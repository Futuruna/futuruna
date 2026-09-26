//! Frontend name checks for authored type annotations. Rust paths and opaque
//! Rust imports remain the backend's responsibility; this is not an ABI proof.

use super::*;

#[derive(Clone, Default)]
pub(super) struct AnnotationEnvironment {
    pub(super) parameters: BTreeSet<String>,
    rust_names: BTreeSet<String>,
    opaque_rust_types: bool,
}

impl AnnotationEnvironment {
    fn rust_use(&mut self, tree: &syn::UseTree) {
        match tree {
            syn::UseTree::Path(path) => self.rust_use(&path.tree),
            syn::UseTree::Name(name) => {
                self.rust_names.insert(name.ident.to_string());
            }
            syn::UseTree::Rename(name) => {
                self.rust_names.insert(name.rename.to_string());
            }
            syn::UseTree::Group(group) => {
                for item in &group.items {
                    self.rust_use(item);
                }
            }
            syn::UseTree::Glob(_) => self.opaque_rust_types = true,
        }
    }

    fn rust_items(&mut self, items: &[syn::Item]) {
        for item in items {
            let name = match item {
                syn::Item::Struct(item) => Some(&item.ident),
                syn::Item::Enum(item) => Some(&item.ident),
                syn::Item::Union(item) => Some(&item.ident),
                syn::Item::Type(item) => Some(&item.ident),
                syn::Item::Trait(item) => Some(&item.ident),
                syn::Item::TraitAlias(item) => Some(&item.ident),
                syn::Item::Use(item) => {
                    self.rust_use(&item.tree);
                    None
                }
                // A macro invocation may create names unavailable to this
                // frontend. Defining a macro alone does not create types.
                syn::Item::Macro(item) if !item.mac.path.is_ident("macro_rules") => {
                    self.opaque_rust_types = true;
                    None
                }
                _ => None,
            };
            if let Some(name) = name {
                self.rust_names.insert(name.to_string());
            }
        }
    }
}

impl TypeChecker {
    fn unknown_annotation_type(&mut self, name: &str) {
        // Reparse only erroneous source to index real type positions. Value
        // identifiers, strings and Rust bodies must not capture this error's
        // location. A source is indexed once even if it has several errors.
        if self
            .annotation_source_spans
            .as_ref()
            .is_none_or(|(source, _)| source != &self.source_text)
        {
            let mut parser =
                Parser::new(Lexer::new(&self.source_text).tokenize(), &self.source_text);
            parser.type_name_spans = Some(Vec::new());
            let mut spans = BTreeMap::new();
            if parser.parse_program().is_ok() {
                for (name, span) in parser.type_name_spans.unwrap() {
                    spans.entry(name).or_insert(span);
                }
            }
            self.annotation_source_spans = Some((self.source_text.clone(), spans));
        }
        let message = format!("unknown type `{name}` in annotation");
        if let Some(span) = self
            .annotation_source_spans
            .as_ref()
            .and_then(|(_, spans)| spans.get(name))
            .copied()
        {
            self.error_at_span(span, message);
        } else {
            self.error(message);
        }
    }

    pub(super) fn collect_rust_annotation_names(&mut self, statement: &Stmt) {
        match statement {
            Stmt::Use(path) => {
                if let Ok(item) = syn::parse_str::<syn::ItemUse>(&format!("use {path};")) {
                    self.annotation_environment.rust_use(&item.tree);
                }
            }
            Stmt::RustBlock(code) => match syn::parse_file(code) {
                Ok(file) => self.annotation_environment.rust_items(&file.items),
                // Embedded Rust can also be a block of statements. Let rustc
                // resolve types when its contents cannot be indexed as items.
                Err(_) => self.annotation_environment.opaque_rust_types = true,
            },
            _ => {}
        }
    }

    pub(super) fn check_annotation_type(&mut self, ty: &Ty) {
        match ty {
            Ty::Name(name) => {
                if !self.types.contains(name)
                    && !self.annotation_environment.parameters.contains(name)
                    && !self.annotation_environment.rust_names.contains(name)
                    && !self.annotation_environment.opaque_rust_types
                    && !name.contains("::")
                    // These are native surface types without constructors in
                    // the legacy declaration catalog.
                    && !matches!(name.as_str(), "Nat" | "Map" | "Set" | "ProgramReference")
                {
                    self.unknown_annotation_type(name);
                }
            }
            Ty::App(constructor, args) => {
                self.check_annotation_type(constructor);
                for argument in args {
                    self.check_annotation_type(argument);
                }
            }
            Ty::Arrow(input, output) => {
                self.check_annotation_type(input);
                self.check_annotation_type(output);
            }
            Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner) | Ty::Optional(inner) => {
                self.check_annotation_type(inner);
            }
            // Lowercase variables and holes are the existing generic syntax.
            Ty::Var(_) | Ty::Unit | Ty::Hole => {}
        }
    }

    pub(super) fn check_parameter_annotations(&mut self, parameters: &[Param]) {
        for parameter in parameters {
            if let Some(ty) = &parameter.ty {
                self.check_annotation_type(ty);
            }
        }
    }

    pub(super) fn check_rule_pattern_annotations(&mut self, pattern: &Expr) {
        if let Some((inner, type_name)) = Self::typed_rule_arg_parts(pattern) {
            if let Ok(ty) = parse_type_annotation(type_name) {
                self.check_annotation_type(&ty);
            }
            self.check_rule_pattern_annotations(inner);
        } else {
            match &pattern.kind {
                ExprKind::App(_, arguments)
                | ExprKind::Tuple(arguments)
                | ExprKind::List(arguments) => {
                    for argument in arguments {
                        self.check_rule_pattern_annotations(argument);
                    }
                }
                _ => {}
            }
        }
    }

    fn check_definition_annotations(&mut self, definition: &Defn) {
        match definition {
            Defn::Fn { params, ret_ty, .. } => {
                self.check_parameter_annotations(params);
                if let Some(ty) = ret_ty {
                    self.check_annotation_type(ty);
                }
            }
            Defn::Actor { state_param, .. } => {
                self.check_parameter_annotations(std::slice::from_ref(state_param));
            }
            Defn::Module { .. } => {} // The ordinary checker enters its scope.
        }
    }

    fn check_variant_annotations(&mut self, variants: &[Variant]) {
        for variant in variants {
            for field in &variant.fields {
                self.check_annotation_type(&field.ty);
            }
        }
    }

    pub(super) fn check_statement_annotations(&mut self, statement: &Stmt) {
        match statement {
            Stmt::Defn(definition) => self.check_definition_annotations(definition),
            Stmt::TypeDecl(TypeDecl::ADT {
                params,
                variants,
                methods,
                ..
            }) => {
                self.annotation_environment.parameters.extend(
                    params
                        .iter()
                        .filter(|param| param.ty.is_none())
                        .map(|param| param.name.clone()),
                );
                self.annotation_environment.parameters.insert("Self".into());
                self.check_parameter_annotations(params);
                self.check_variant_annotations(variants);
                for method in methods {
                    self.check_definition_annotations(method);
                }
            }
            Stmt::TypeDecl(TypeDecl::WhenType { variants, .. }) => {
                self.check_variant_annotations(variants);
            }
            Stmt::TypeDecl(TypeDecl::TraitDecl {
                params, methods, ..
            }) => {
                self.annotation_environment.parameters.extend(
                    params
                        .iter()
                        .filter(|param| param.ty.is_none())
                        .map(|param| param.name.clone()),
                );
                self.annotation_environment.parameters.insert("Self".into());
                self.check_parameter_annotations(params);
                for method in methods {
                    self.check_parameter_annotations(&method.params);
                    if let Some(ty) = &method.ret_ty {
                        self.check_annotation_type(ty);
                    }
                }
            }
            Stmt::TypeDecl(TypeDecl::ImplBlock { methods, .. }) => {
                self.annotation_environment.parameters.insert("Self".into());
                for method in methods {
                    self.check_definition_annotations(method);
                }
            }
            Stmt::TypeDecl(TypeDecl::EffectDecl { ops, .. }) => {
                for (_, parameters, result) in ops {
                    self.check_parameter_annotations(parameters);
                    if let Some(ty) = result {
                        self.check_annotation_type(ty);
                    }
                }
            }
            Stmt::TypeDecl(TypeDecl::RuleScope { params, .. }) => {
                self.check_parameter_annotations(params);
            }
            Stmt::Bind(_, Some(ty), _) | Stmt::MonadicBind(_, Some(ty), _) => {
                self.check_annotation_type(ty);
            }
            _ => {}
        }
    }
}
