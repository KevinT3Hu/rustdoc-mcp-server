use markdown_builder::{CodeBlock, ListBuilder, Markdown};
use rustdoc_types::{
    AssocItemConstraintKind, Crate, GenericArg, GenericArgs, GenericBound, GenericParamDefKind,
    Generics, Id, Item, ItemEnum, PreciseCapturingArg, Term, TraitBoundModifier, Type,
};
use tracing::debug;

fn find_parent_impl(krate: &Crate, id: Id) -> Option<&Item> {
    krate.index.values().find(|item| {
        if let ItemEnum::Impl(impl_) = &item.inner {
            impl_.items.contains(&id)
        } else {
            false
        }
    })
}

fn format_impl_header(impl_: &rustdoc_types::Impl) -> String {
    let mut s = String::from("impl");
    s.push_str(&format_generics(&impl_.generics));
    s.push(' ');

    if let Some(trait_) = &impl_.trait_ {
        s.push_str(&format_path_like(&trait_.path, trait_.args.as_deref()));
        s.push_str(" for ");
    }

    s.push_str(&format_type(&impl_.for_));
    s
}

pub fn generate_item_markdown(item: &Item, krate: &Crate) -> String {
    let mut doc = Markdown::new();

    let name = item
        .name
        .as_deref()
        .or(match &item.inner {
            ItemEnum::Use(u) => Some(u.name.as_str()),
            _ => None,
        })
        .unwrap_or("<unnamed>");
    let kind = get_item_kind(item);

    doc.header1(format!("{kind} {name}"));

    if let Some(parent) = find_parent_impl(krate, item.id)
        && let ItemEnum::Impl(impl_) = &parent.inner
    {
        let cb = format_impl_header(impl_).to_code_block_with_language("rust");
        doc.paragraph(cb);
    }

    // Signature / Definition
    let definition = format_item_definition(item);
    if !definition.is_empty() {
        let cb = definition.to_code_block_with_language("rust");
        doc.paragraph(cb);
    }

    // Documentation
    if let Some(docs) = &item.docs {
        doc.header2("Documentation");
        doc.paragraph(docs);
    }

    // Specific details based on kind
    match &item.inner {
        ItemEnum::Struct(s) => {
            if let rustdoc_types::StructKind::Plain { fields, .. } = &s.kind
                && !fields.is_empty()
            {
                doc.header2("Fields");
                let mut field_list = ListBuilder::new();
                for field_id in fields {
                    if let Some(field) = krate.index.get(field_id)
                        && let ItemEnum::StructField(ty) = &field.inner
                    {
                        let field_name = field.name.as_deref().unwrap_or("_");
                        let field_type = format_type(ty);

                        let mut line = format!("`{field_name}: {field_type}`");
                        if let Some(d) = &field.docs {
                            let short = d.lines().next().unwrap_or("").trim();
                            if !short.is_empty() {
                                use std::fmt::Write;
                                write!(&mut line, " - {short}").ok();
                            }
                        }
                        field_list = field_list.append(line);
                    }
                }

                doc.list(field_list.unordered());
            }
        }
        ItemEnum::Enum(e) => {
            if !e.variants.is_empty() {
                doc.header2("Variants");
                let mut variant_list = ListBuilder::new();
                for variant_id in &e.variants {
                    if let Some(variant) = krate.index.get(variant_id) {
                        let variant_name = variant.name.as_deref().unwrap_or("_");

                        let mut line = format!("`{variant_name}`");

                        if let ItemEnum::Variant(v) = &variant.inner {
                            match &v.kind {
                                rustdoc_types::VariantKind::Tuple(types) => {
                                    if !types.is_empty() {
                                        line.push_str("(...)");
                                    }
                                }
                                rustdoc_types::VariantKind::Struct { .. } => {
                                    line.push_str(" { ... }");
                                }
                                rustdoc_types::VariantKind::Plain => {}
                            }
                        }

                        if let Some(d) = &variant.docs {
                            let short = d.lines().next().unwrap_or("").trim();
                            if !short.is_empty() {
                                use std::fmt::Write;
                                write!(&mut line, " - {short}").ok();
                            }
                        }
                        variant_list = variant_list.append(line);
                    }
                }

                doc.list(variant_list.unordered());
            }
        }
        _ => {}
    }

    doc.render()
}

fn get_item_kind(item: &Item) -> &'static str {
    match &item.inner {
        ItemEnum::Module(_) => "Module",
        ItemEnum::ExternCrate { .. } => "Extern Crate",
        ItemEnum::Union(_) => "Union",
        ItemEnum::Struct(_) => "Struct",
        ItemEnum::StructField(_) => "Field",
        ItemEnum::Enum(_) => "Enum",
        ItemEnum::Variant(_) => "Variant",
        ItemEnum::Function(_) => "Function",
        ItemEnum::Trait(_) => "Trait",
        ItemEnum::TraitAlias(_) => "Trait Alias",
        ItemEnum::Impl(_) => "Impl",
        ItemEnum::TypeAlias(_) => "Type Alias",
        ItemEnum::Static(_) => "Static",
        ItemEnum::Macro(_) => "Macro",
        ItemEnum::ProcMacro(_) => "Proc Macro",
        ItemEnum::Primitive(_) => "Primitive",
        ItemEnum::AssocConst { .. } => "Assoc Constant",
        ItemEnum::AssocType { .. } => "Assoc Type",
        ItemEnum::Use(_) => "Use",
        ItemEnum::ExternType => "Extern Type",
        ItemEnum::Constant {
            type_: _,
            const_: _,
        } => "Constant",
    }
}

fn format_generic_bound(bound: &GenericBound) -> String {
    match bound {
        GenericBound::TraitBound {
            trait_,
            generic_params,
            modifier,
        } => {
            let mut s = String::new();
            if !generic_params.is_empty() {
                s.push_str("for<");
                let params: Vec<String> = generic_params
                    .iter()
                    .map(|p| {
                        if let GenericParamDefKind::Lifetime { outlives } = &p.kind {
                            if outlives.is_empty() {
                                p.name.clone()
                            } else {
                                format!("{}: {}", p.name, outlives.join(" + "))
                            }
                        } else {
                            p.name.clone()
                        }
                    })
                    .collect();
                s.push_str(&params.join(", "));
                s.push_str("> ");
            }

            match modifier {
                TraitBoundModifier::None => {}
                TraitBoundModifier::Maybe => s.push('?'),
                TraitBoundModifier::MaybeConst => s.push_str("~const "),
            }

            s.push_str(&format_path_like(&trait_.path, trait_.args.as_deref()));
            s
        }
        GenericBound::Outlives(l) => l.clone(),
        GenericBound::Use(precise_capturing_args) => {
            format_precise_capturing_args(precise_capturing_args)
        }
    }
}

fn format_generics(generics: &Generics) -> String {
    let mut params = Vec::new();
    for param in &generics.params {
        match &param.kind {
            GenericParamDefKind::Lifetime { outlives } => {
                let mut s = param.name.clone();
                if !outlives.is_empty() {
                    s.push_str(": ");
                    s.push_str(&outlives.join(" + "));
                }
                params.push(s);
            }
            GenericParamDefKind::Type {
                bounds,
                default,
                is_synthetic,
            } => {
                if *is_synthetic {
                    continue;
                }
                let mut s = param.name.clone();
                if !bounds.is_empty() {
                    s.push_str(": ");
                    let bounds: Vec<String> = bounds.iter().map(format_generic_bound).collect();
                    s.push_str(&bounds.join(" + "));
                }
                if let Some(ty) = default {
                    s.push_str(" = ");
                    s.push_str(&format_type(ty));
                }
                params.push(s);
            }
            GenericParamDefKind::Const { type_, default } => {
                let mut s = format!("const {}: {}", param.name, format_type(type_));
                if let Some(expr) = default {
                    s.push_str(" = ");
                    s.push_str(expr);
                }
                params.push(s);
            }
        }
    }

    if params.is_empty() {
        return String::new();
    }

    format!("<{}>", params.join(", "))
}

fn format_item_definition(item: &Item) -> String {
    let name = item.name.as_deref().unwrap_or("");
    match &item.inner {
        ItemEnum::Function(f) => {
            let mut s = String::new();

            if f.header.is_const {
                s.push_str("const ");
            }
            if f.header.is_async {
                s.push_str("async ");
            }
            if f.header.is_unsafe {
                s.push_str("unsafe ");
            }

            s.push_str("fn ");
            s.push_str(name);
            s.push_str(&format_generics(&f.generics));
            debug!(
                "Formatting function: {} with generics: {:?}",
                name, f.generics
            );
            s.push('(');
            let args: Vec<String> = f
                .sig
                .inputs
                .iter()
                .map(|(name, ty)| format!("{}: {}", name, format_type(ty)))
                .collect();
            s.push_str(&args.join(", "));
            s.push(')');

            if let Some(output) = &f.sig.output {
                s.push_str(" -> ");
                s.push_str(&format_type(output));
            }
            s
        }
        ItemEnum::Struct(s) => {
            let mut def = format!("struct {}{}", name, format_generics(&s.generics));
            match &s.kind {
                rustdoc_types::StructKind::Unit => {
                    def.push(';');
                }
                rustdoc_types::StructKind::Tuple(_) => {
                    def.push_str("(/* ... */);");
                }
                rustdoc_types::StructKind::Plain { .. } => {
                    def.push_str(" { ... }");
                }
            }
            def
        }
        ItemEnum::Union(u) => format!("union {}{} {{ ... }}", name, format_generics(&u.generics)),
        ItemEnum::Enum(e) => format!("enum {}{}", name, format_generics(&e.generics)),
        ItemEnum::Trait(t) => format!("trait {}{}", name, format_generics(&t.generics)),
        ItemEnum::TypeAlias(t) => {
            format!(
                "type {}{} = {};",
                name,
                format_generics(&t.generics),
                format_type(&t.type_)
            )
        }
        ItemEnum::Constant { type_, const_: _ } => {
            format!("const {}: {} = ...;", name, format_type(type_))
        }
        ItemEnum::Static(st) => {
            format!("static {}: {} = ...;", name, format_type(&st.type_))
        }
        ItemEnum::Use(u) => format!("use {};", u.source),
        _ => String::new(),
    }
}

fn format_type(ty: &Type) -> String {
    match ty {
        Type::ResolvedPath(p) => format_path_like(&p.path, p.args.as_deref()),
        Type::Primitive(p) => p.clone(),
        Type::Tuple(types) => {
            let types: Vec<String> = types.iter().map(format_type).collect();
            format!("({})", types.join(", "))
        }
        Type::Slice(ty) => format!("[{}]", format_type(ty)),
        Type::Array { type_, len } => format!("[{}; {}]", format_type(type_), len),
        Type::BorrowedRef {
            lifetime,
            is_mutable,
            type_,
        } => {
            let mut s = String::from("&");
            if let Some(l) = lifetime {
                s.push_str(l);
                s.push(' ');
            }
            if *is_mutable {
                s.push_str("mut ");
            }
            s.push_str(&format_type(type_));
            s
        }
        Type::RawPointer { is_mutable, type_ } => {
            let mut s = String::from("*");
            s.push_str(if *is_mutable { "mut " } else { "const " });
            s.push_str(&format_type(type_));
            s
        }
        Type::Generic(name) => name.clone(),
        Type::ImplTrait(bounds) => {
            let bounds: Vec<String> = bounds.iter().map(format_generic_bound).collect();
            format!("impl {}", bounds.join(" + "))
        }
        Type::DynTrait(dyn_trait) => {
            let mut s = String::from("dyn ");
            if let Some(lifetime) = &dyn_trait.lifetime {
                s.push_str(lifetime);
                s.push_str(" + ");
            }
            let traits: Vec<String> = dyn_trait
                .traits
                .iter()
                .map(|t| format_path_like(&t.trait_.path, t.trait_.args.as_deref()))
                .collect();
            s.push_str(&traits.join(" + "));
            s
        }
        // Fallback for others
        _ => "_".to_string(),
    }
}

fn format_path_like(name: &str, args: Option<&GenericArgs>) -> String {
    let mut s = name.to_string();
    if let Some(args) = args {
        match args {
            GenericArgs::AngleBracketed { args, constraints } => {
                if !args.is_empty() || !constraints.is_empty() {
                    s.push('<');
                    let mut params = Vec::new();
                    for arg in args {
                        match arg {
                            GenericArg::Lifetime(l) => params.push(l.clone()),
                            GenericArg::Type(t) => params.push(format_type(t)),
                            GenericArg::Const(c) => params.push(format!("const {}", c.expr)),
                            GenericArg::Infer => params.push("_".to_string()),
                        }
                    }
                    for binding in constraints {
                        match &binding.binding {
                            AssocItemConstraintKind::Equality(term) => {
                                let rhs = match term {
                                    Term::Type(t) => format_type(t),
                                    Term::Constant(c) => c.expr.clone(),
                                };
                                params.push(format!("{} = {}", binding.name, rhs));
                            }
                            AssocItemConstraintKind::Constraint(bounds) => {
                                let bound_str = bounds
                                    .iter()
                                    .map(format_generic_bound)
                                    .collect::<Vec<_>>()
                                    .join(" + ");
                                params.push(format!("{}: {}", binding.name, bound_str));
                            }
                        }
                    }
                    s.push_str(&params.join(", "));
                    s.push('>');
                }
            }
            GenericArgs::Parenthesized { inputs, output } => {
                s.push('(');
                let inputs: Vec<String> = inputs.iter().map(format_type).collect();
                s.push_str(&inputs.join(", "));
                s.push(')');
                if let Some(out) = output {
                    s.push_str(" -> ");
                    s.push_str(&format_type(out));
                }
            }
            GenericArgs::ReturnTypeNotation => s.push_str("(..)"),
        }
    }
    s
}

fn format_precise_capturing_args(precise_capturing_args: &[PreciseCapturingArg]) -> String {
    let args_str = precise_capturing_args
        .iter()
        .map(|arg| match arg {
            PreciseCapturingArg::Lifetime(name) | PreciseCapturingArg::Param(name) => name.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("use<{args_str}>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustdoc_types::{Crate, Generics, Id, Item, ItemEnum, Span, StructKind, Visibility};
    use std::collections::HashMap;

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

    fn create_dummy_crate() -> Crate {
        Crate {
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
        }
    }

    #[test]
    fn test_format_type_primitive() {
        let ty = Type::Primitive("i32".to_string());
        assert_eq!(format_type(&ty), "i32");
    }

    #[test]
    fn test_format_type_tuple() {
        let ty = Type::Tuple(vec![
            Type::Primitive("i32".to_string()),
            Type::Primitive("String".to_string()),
        ]);
        assert_eq!(format_type(&ty), "(i32, String)");
    }

    #[test]
    fn test_format_type_slice() {
        let ty = Type::Slice(Box::new(Type::Primitive("u8".to_string())));
        assert_eq!(format_type(&ty), "[u8]");
    }

    #[test]
    fn test_generate_struct_markdown() {
        let krate = create_dummy_crate();
        let item = create_dummy_item(
            "MyStruct",
            ItemEnum::Struct(rustdoc_types::Struct {
                generics: Generics {
                    params: vec![],
                    where_predicates: vec![],
                },
                kind: StructKind::Plain {
                    fields: vec![],
                    has_stripped_fields: false,
                },
                impls: vec![],
            }),
        );

        let md = generate_item_markdown(&item, &krate);
        assert!(md.contains("# Struct MyStruct"));
        assert!(md.contains("struct MyStruct { ... }"));
    }

    #[test]
    fn test_generate_enum_markdown() {
        let krate = create_dummy_crate();
        let item = create_dummy_item(
            "MyEnum",
            ItemEnum::Enum(rustdoc_types::Enum {
                generics: Generics {
                    params: vec![],
                    where_predicates: vec![],
                },
                variants: vec![],
                impls: vec![],
                has_stripped_variants: false,
            }),
        );

        let md = generate_item_markdown(&item, &krate);
        assert!(md.contains("# Enum MyEnum"));
        assert!(md.contains("enum MyEnum"));
    }

    #[test]
    fn test_generate_function_markdown() {
        let krate = create_dummy_crate();
        let item = create_dummy_item(
            "my_fn",
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
                    inputs: vec![("arg1".to_string(), Type::Primitive("i32".to_string()))],
                    output: Some(Type::Primitive("bool".to_string())),
                    is_c_variadic: false,
                },
            }),
        );

        let md = generate_item_markdown(&item, &krate);
        assert!(md.contains("# Function my_fn"));
        assert!(md.contains("fn my_fn(arg1: i32) -> bool"));
    }
}
