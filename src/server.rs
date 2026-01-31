use std::env::current_dir;

use crate::types::*;
use crate::workspace::Workspace;
use crate::{
    index::{CrateIndex, get_item_kind},
    markdown::generate_item_markdown,
};

use anyhow::Result;
use rmcp::{
    ServerHandler,
    handler::server::{
        tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
use rustdoc_types::ItemEnum;
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub struct RustDocMCPServer {
    workspace: Workspace,
    index: CrateIndex,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl RustDocMCPServer {
    pub fn new(cwd: Option<String>) -> Result<Self, String> {
        let cwd = match cwd {
            Some(dir) => dir,
            None => current_dir()
                .map_err(|e| e.to_string())?
                .to_str()
                .ok_or("Failed to convert path to string".to_string())?
                .to_string(),
        };

        if !Workspace::has_nightly_toolchain() {
            return Err("Rust nightly toolchain is required but not found. Please install it with `rustup toolchain install nightly`.".to_string());
        }

        let workspace =
            Workspace::load(&cwd).map_err(|e| format!("Failed to load workspace: {}", e))?;

        let index = CrateIndex::new(workspace.clone());

        Ok(Self {
            workspace,
            index,
            tool_router: Self::tool_router(),
        })
    }

    #[tool(description = "Returns a list of all dependencies available in the current project.")]
    pub async fn list_deps(&self) -> Result<Json<ListDepsResult>, String> {
        info!("Listing dependencies...");
        let deps: Vec<String> = self
            .workspace
            .get_dependencies()
            .iter()
            .map(|p| p.name.to_string())
            .collect();
        debug!("Found dependencies: {:?}", deps);
        Ok(Json(ListDepsResult { dependencies: deps }))
    }

    #[tool(description = "Lists the root items of a specific crate.")]
    pub async fn list_crate_items(
        &self,
        args: Parameters<ListCrateItemsArgs>,
    ) -> Result<Json<ListCrateItemsResult>, String> {
        let crate_name = &args.0.crate_name;
        info!("Listing items for crate: {}", crate_name);

        self.index
            .ensure_loaded(crate_name)
            .await
            .map_err(|e| e.to_string())?;

        let krate_ref = self
            .index
            .get_crate(crate_name)
            .ok_or("Failed to load crate".to_string())?;

        let root_id = &krate_ref.krate.root;
        let root_item = krate_ref
            .krate
            .index
            .get(root_id)
            .ok_or("Root item missing".to_string())?;

        debug!("Root item: {:?}", root_item);

        let mut items = Vec::new();
        if let ItemEnum::Module(m) = &root_item.inner {
            for item_id in &m.items {
                if let Some(child) = krate_ref.krate.index.get(item_id) {
                    debug!("Found child item: {:?}", child);
                    let name = if let Some(name) = &child.name {
                        Some(name.clone())
                    } else if let ItemEnum::Use(use_item) = &child.inner {
                        Some(use_item.name.clone())
                    } else {
                        None
                    };

                    if let Some(name) = name {
                        items.push(ItemSummary {
                            name,
                            kind: get_item_kind(child),
                        });
                    }
                }
            }
        }

        info!("Found {} items in crate root", items.len());
        debug!("Items: {:?}", items);

        Ok(Json(ListCrateItemsResult { items }))
    }

    #[tool(description = "Returns the documentation for a specific item (e.g., std::vec::Vec).")]
    pub async fn get_docs(&self, args: Parameters<GetDocsArgs>) -> Result<String, String> {
        let path = &args.0.path;
        info!("Getting docs for path: {}", path);

        let parts: Vec<&str> = path.split("::").collect();
        if parts.is_empty() {
            return Err("Invalid path".to_string());
        }
        let crate_name = parts[0];

        self.index
            .ensure_loaded(crate_name)
            .await
            .map_err(|e| e.to_string())?;

        let krate_ref = self
            .index
            .get_crate(crate_name)
            .ok_or("Failed to load crate".to_string())?;

        let id = krate_ref
            .path_to_id
            .get(path)
            .ok_or(format!("Item not found: {}", path))?;

        debug!("Found item ID: {:?}", id);

        let item = krate_ref
            .krate
            .index
            .get(id)
            .ok_or("Item index missing".to_string())?;

        let docs = generate_item_markdown(item, &krate_ref.krate);

        Ok(docs)
    }

    #[tool(description = "Performs a fuzzy search across the index for items matching the query.")]
    pub async fn search_docs(
        &self,
        Parameters(args): Parameters<SearchDocsArgs>,
    ) -> Result<Json<SearchDocsResult>, String> {
        info!(
            "Searching docs for query: '{}' in crate: {:?}",
            args.query, args.crate_name
        );
        let matches = self
            .index
            .search(&args.query, args.crate_name.as_deref())
            .await
            .map_err(|e| e.to_string())?;

        info!("Found {} matches", matches.len());
        debug!("Matches: {:?}", matches);

        Ok(Json(SearchDocsResult { matches }))
    }

    #[tool(description = "Returns a summary of all public items within a specific module.")]
    pub async fn get_module(
        &self,
        args: Parameters<GetModuleArgs>,
    ) -> Result<Json<GetModuleResult>, String> {
        let path = &args.0.path;
        info!("Getting module info for path: {}", path);

        let parts: Vec<&str> = path.split("::").collect();
        if parts.is_empty() {
            return Err("Invalid path".to_string());
        }
        let crate_name = parts[0];

        self.index
            .ensure_loaded(crate_name)
            .await
            .map_err(|e| e.to_string())?;

        let krate_ref = self
            .index
            .get_crate(crate_name)
            .ok_or("Failed to load crate".to_string())?;

        let id = krate_ref
            .path_to_id
            .get(path)
            .ok_or(format!("Module not found: {}", path))?;
        let item = krate_ref
            .krate
            .index
            .get(id)
            .ok_or("Item index missing".to_string())?;

        if let rustdoc_types::ItemEnum::Module(m) = &item.inner {
            let mut children = Vec::new();
            for item_id in &m.items {
                if let Some(child) = krate_ref.krate.index.get(item_id) {
                    let name = if let Some(name) = &child.name {
                        Some(name.clone())
                    } else if let rustdoc_types::ItemEnum::Use(use_item) = &child.inner {
                        Some(use_item.name.clone())
                    } else {
                        None
                    };

                    if let Some(name) = name {
                        children.push(ItemSummary {
                            name,
                            kind: get_item_kind(child),
                        });
                    }
                }
            }

            info!("Found {} items in module", children.len());
            debug!("Module items: {:?}", children);

            Ok(Json(GetModuleResult { items: children }))
        } else {
            Err(format!("Item at {} is not a module", path))
        }
    }
}

const SERVER_INSTRUCTIONS: &str = include_str!("../server_instructions.md");

#[tool_handler]
impl ServerHandler for RustDocMCPServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(SERVER_INSTRUCTIONS.to_string()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
