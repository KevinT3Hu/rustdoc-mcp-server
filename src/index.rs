use std::collections::HashMap;
use std::sync::Arc;

use crate::types::ItemSummary;
use anyhow::{Context, Result};
use dashmap::DashMap;
use rustdoc_types::{Crate, Id, Item, ItemEnum};
use strsim::jaro_winkler;
use tokio::fs;
use tracing::{debug, info, instrument};

use crate::doc_gen::DocGenerator;
use crate::workspace::Workspace;

#[derive(Debug, Clone)]
pub struct LoadedCrate {
    pub krate: Crate,
    pub path_to_id: HashMap<String, Id>,
}

#[derive(Debug, Clone)]
pub struct CrateIndex {
    /// Cache of loaded crates: crate_name -> LoadedCrate
    crates: Arc<DashMap<String, LoadedCrate>>,
    workspace: Workspace,
}

impl CrateIndex {
    pub fn new(workspace: Workspace) -> Self {
        Self {
            crates: Arc::new(DashMap::new()),
            workspace,
        }
    }

    /// Ensures the documentation for the given crate is loaded.
    #[instrument(skip(self))]
    pub async fn ensure_loaded(&self, crate_name: &str) -> Result<()> {
        if self.crates.contains_key(crate_name) {
            debug!("Crate {} is already loaded", crate_name);
            return Ok(());
        }

        info!("Ensuring docs loaded for crate: {}", crate_name);

        let target_dir = self.workspace.metadata.target_directory.as_std_path();
        let json_path = target_dir
            .join("doc")
            .join(format!("{}.json", crate_name.replace('-', "_")));

        debug!("Expected JSON path: {:?}", json_path);

        if !json_path.exists() {
            debug!("JSON not found, generating docs for {}", crate_name);
            let package = self.workspace.packages.get(crate_name).or_else(|| {
                self.workspace
                    .packages
                    .iter()
                    .find(|(k, _)| k.replace('-', "_") == crate_name)
                    .map(|(_, v)| v)
            });

            if let Some(pkg) = package {
                let features = self
                    .workspace
                    .metadata
                    .resolve
                    .as_ref()
                    .and_then(|resolve| {
                        resolve
                            .nodes
                            .iter()
                            .find(|node| node.id == pkg.id)
                            .map(|node| {
                                node.features
                                    .iter()
                                    .map(|f| f.to_string())
                                    .collect::<Vec<_>>()
                            })
                    });

                DocGenerator::generate(
                    &pkg.name,
                    features.as_deref(),
                    self.workspace.root.to_str().unwrap(),
                    target_dir,
                )
                .await?;
            } else {
                DocGenerator::generate(
                    crate_name,
                    None,
                    self.workspace.root.to_str().unwrap(),
                    target_dir,
                )
                .await?;
            }
        }

        info!("Reading rustdoc JSON from {:?}", json_path);
        let content = fs::read_to_string(&json_path)
            .await
            .context("Failed to read rustdoc JSON")?;
        let krate: Crate =
            serde_json::from_str(&content).context("Failed to parse rustdoc JSON")?;

        let path_to_id = self.build_path_map(&krate, crate_name);

        self.crates
            .insert(crate_name.to_string(), LoadedCrate { krate, path_to_id });
        info!("Crate {} loaded successfully", crate_name);
        Ok(())
    }

    fn build_path_map(&self, krate: &Crate, crate_name: &str) -> HashMap<String, Id> {
        debug!("Building path map for crate: {}", crate_name);
        let mut map = HashMap::new();

        // Traverse `index` starting from root.
        let root_id = &krate.root;
        if let Some(root_item) = krate.index.get(root_id) {
            self.traverse_item(krate, root_item, crate_name.to_string(), &mut map);
        }

        info!("Indexed {} paths for crate {}", map.len(), crate_name);

        map
    }

    fn traverse_item(
        &self,
        krate: &Crate,
        item: &Item,
        current_path: String,
        map: &mut HashMap<String, Id>,
    ) {
        map.insert(current_path.clone(), item.id);

        match &item.inner {
            ItemEnum::Module(m) => {
                for item_id in &m.items {
                    if let Some(child) = krate.index.get(item_id)
                        && let Some(name) = &child.name
                    {
                        let child_path = format!("{}::{}", current_path, name);
                        self.traverse_item(krate, child, child_path, map);
                    }
                }
            }
            ItemEnum::Struct(s) => {
                let mut add_field = |field_id: &Id| {
                    if let Some(field) = krate.index.get(field_id)
                        && let Some(name) = &field.name
                    {
                        let field_path = format!("{}::{}", current_path, name);
                        map.insert(field_path, field.id);
                    }
                };

                match &s.kind {
                    rustdoc_types::StructKind::Unit => {}
                    rustdoc_types::StructKind::Tuple(ids) => {
                        for field_id in ids.iter().flatten() {
                            add_field(field_id);
                        }
                    }
                    rustdoc_types::StructKind::Plain { fields, .. } => {
                        for field_id in fields {
                            add_field(field_id);
                        }
                    }
                }
                for impl_id in &s.impls {
                    if let Some(impl_item) = krate.index.get(impl_id)
                        && let ItemEnum::Impl(i) = &impl_item.inner
                    {
                        for item_id in &i.items {
                            if let Some(item) = krate.index.get(item_id)
                                && let Some(name) = &item.name
                            {
                                let item_path = format!("{}::{}", current_path, name);
                                map.insert(item_path, item.id);
                            }
                        }
                    }
                }
            }
            ItemEnum::Enum(e) => {
                for variant_id in &e.variants {
                    if let Some(variant) = krate.index.get(variant_id)
                        && let Some(name) = &variant.name
                    {
                        let variant_path = format!("{}::{}", current_path, name);
                        map.insert(variant_path, variant.id);
                    }
                }
                for impl_id in &e.impls {
                    if let Some(impl_item) = krate.index.get(impl_id)
                        && let ItemEnum::Impl(i) = &impl_item.inner
                    {
                        for item_id in &i.items {
                            if let Some(item) = krate.index.get(item_id)
                                && let Some(name) = &item.name
                            {
                                let item_path = format!("{}::{}", current_path, name);
                                map.insert(item_path, item.id);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    pub fn get_crate(
        &self,
        crate_name: &str,
    ) -> Option<dashmap::mapref::one::Ref<'_, String, LoadedCrate>> {
        self.crates.get(crate_name)
    }

    pub async fn search(&self, query: &str, crate_name: Option<&str>) -> Result<Vec<ItemSummary>> {
        debug!(
            "Searching index for '{}' (crate scope: {:?})",
            query, crate_name
        );
        if let Some(name) = crate_name {
            self.ensure_loaded(name).await?;
        }

        let mut matches = Vec::new();

        for entry in self.crates.iter() {
            let krate_name = entry.key();
            if let Some(target) = crate_name
                && krate_name != target
            {
                continue;
            }

            let loaded_crate = entry.value();
            for (path, id) in &loaded_crate.path_to_id {
                let score = jaro_winkler(query, path);
                if score > 0.8 || path.contains(query) {
                    let kind = loaded_crate
                        .krate
                        .index
                        .get(id)
                        .map(get_item_kind)
                        .unwrap_or_else(|| "unknown".to_string());
                    matches.push((path.clone(), kind, score));
                }
            }
        }

        debug!(
            "Found {} potential matches before sorting/truncating",
            matches.len()
        );

        matches.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        matches.truncate(20);

        Ok(matches
            .into_iter()
            .map(|(name, kind, _)| ItemSummary { name, kind })
            .collect())
    }
}

pub fn get_item_kind(item: &rustdoc_types::Item) -> String {
    use rustdoc_types::ItemEnum::*;
    match &item.inner {
        Module(_) => "module",
        ExternCrate { .. } => "extern_crate",
        Use(_) => "import",
        Union(_) => "union",
        Struct(_) => "struct",
        StructField(_) => "struct_field",
        Enum(_) => "enum",
        Variant(_) => "variant",
        Function(_) => "function",
        TypeAlias(_) => "type_alias",
        Trait(_) => "trait",
        TraitAlias(_) => "trait_alias",
        Impl(_) => "impl",
        Static(_) => "static",
        Macro(_) => "macro",
        ProcMacro(_) => "proc_macro",
        Primitive(_) => "primitive",
        AssocConst { .. } => "assoc_const",
        AssocType { .. } => "assoc_type",
        _ => "other",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustdoc_types::{Crate, Generics, Id, Item, ItemEnum, Span, Visibility};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn create_dummy_metadata() -> cargo_metadata::Metadata {
        serde_json::from_str(
            r#"{
            "packages": [],
            "workspace_members": [],
            "workspace_default_members": [],
            "resolve": null,
            "target_directory": "/tmp",
            "version": 1,
            "workspace_root": "/tmp"
        }"#,
        )
        .unwrap()
    }

    fn create_dummy_workspace() -> Workspace {
        Workspace {
            root: PathBuf::from("/tmp"),
            metadata: create_dummy_metadata(),
            packages: HashMap::new(),
        }
    }

    fn create_dummy_item(name: &str, inner: ItemEnum) -> Item {
        let id_val = name.len() as u32;
        Item {
            id: Id(id_val),
            crate_id: 0,
            name: Some(name.to_string()),
            span: Some(Span {
                filename: Default::default(),
                begin: (0, 0),
                end: (0, 0),
            }),
            visibility: Visibility::Public,
            docs: None,
            links: HashMap::new(),
            attrs: Vec::new(),
            deprecation: None,
            inner,
        }
    }

    #[test]
    fn test_get_item_kind() {
        let item = create_dummy_item(
            "test",
            ItemEnum::Struct(rustdoc_types::Struct {
                generics: Generics {
                    params: vec![],
                    where_predicates: vec![],
                },
                kind: rustdoc_types::StructKind::Unit,
                impls: vec![],
            }),
        );
        assert_eq!(get_item_kind(&item), "struct");

        let item = create_dummy_item(
            "test",
            ItemEnum::Function(rustdoc_types::Function {
                generics: Generics {
                    params: vec![],
                    where_predicates: vec![],
                },
                header: rustdoc_types::FunctionHeader {
                    is_const: false,
                    is_unsafe: false,
                    is_async: false,
                    abi: rustdoc_types::Abi::Rust,
                },
                has_body: true,
                sig: rustdoc_types::FunctionSignature {
                    inputs: vec![],
                    output: None,
                    is_c_variadic: false,
                },
            }),
        );
        assert_eq!(get_item_kind(&item), "function");
    }

    #[tokio::test]
    async fn test_search_docs() {
        let workspace = create_dummy_workspace();
        let index = CrateIndex::new(workspace);

        // Manually populate the index
        let mut krate = Crate {
            root: Id(0),
            crate_version: None,
            includes_private: false,
            index: HashMap::new(),
            paths: HashMap::new(),
            external_crates: HashMap::new(),
            format_version: 0,
            target: rustdoc_types::Target {
                triple: "x86_64-unknown-linux-gnu".to_string(),
                target_features: vec![],
            },
        };

        let item1 = create_dummy_item(
            "Vec",
            ItemEnum::Struct(rustdoc_types::Struct {
                generics: Generics {
                    params: vec![],
                    where_predicates: vec![],
                },
                kind: rustdoc_types::StructKind::Unit,
                impls: vec![],
            }),
        );
        krate.index.insert(item1.id.clone(), item1);

        let item2 = create_dummy_item(
            "String",
            ItemEnum::Struct(rustdoc_types::Struct {
                generics: Generics {
                    params: vec![],
                    where_predicates: vec![],
                },
                kind: rustdoc_types::StructKind::Unit,
                impls: vec![],
            }),
        );
        krate.index.insert(item2.id.clone(), item2);

        let mut path_to_id = HashMap::new();
        // Since we used len() as ID, Vec -> 3, String -> 6
        path_to_id.insert("std::vec::Vec".to_string(), Id(3));
        path_to_id.insert("std::string::String".to_string(), Id(6));

        index.crates.insert(
            "std".to_string(),
            LoadedCrate {
                krate: krate.clone(),
                path_to_id,
            },
        );

        // Add an empty "other" crate
        let other_krate = Crate {
            root: Id(0),
            crate_version: None,
            includes_private: false,
            index: HashMap::new(),
            paths: HashMap::new(),
            external_crates: HashMap::new(),
            format_version: 0,
            target: rustdoc_types::Target {
                triple: "x86_64-unknown-linux-gnu".to_string(),
                target_features: vec![],
            },
        };

        index.crates.insert(
            "other".to_string(),
            LoadedCrate {
                krate: other_krate,
                path_to_id: HashMap::new(),
            },
        );

        // Test exact match
        let results = index.search("Vec", None).await.unwrap();
        assert!(results.iter().any(|r| r.name == "std::vec::Vec"));

        // Test fuzzy match
        let results = index.search("std::string::Strng", None).await.unwrap();
        assert!(results.iter().any(|r| r.name == "std::string::String"));

        // Test crate filtering
        let results = index.search("Vec", Some("std")).await.unwrap();
        assert!(!results.is_empty());

        let results = index.search("Vec", Some("other")).await.unwrap();
        assert!(results.is_empty());
    }
}
