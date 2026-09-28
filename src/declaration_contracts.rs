//! Declaration contracts: constructor names that belong to the language and
//! trait implementations that must match their trait.

use super::*;

/// Constructors owned by the prelude (`Option`, `Result`, `Pair`,
/// `ProgramReference`) or by the runtime list representation (`Nil`, `Cons`).
/// A declaration of the owning type itself replaces the prelude type; the
/// list constructors have no user-declarable owner.
const RESERVED_CONSTRUCTORS: &[(&str, Option<&str>)] = &[
    ("None", Some("Option")),
    ("Some", Some("Option")),
    ("Ok", Some("Result")),
    ("Err", Some("Result")),
    ("Pair", Some("Pair")),
    ("ProgramSymbolReference", Some("ProgramReference")),
    ("ProgramTypeReference", Some("ProgramReference")),
    ("ProgramMemberReference", Some("ProgramReference")),
    ("Nil", None),
    ("Cons", None),
];

/// Built-in type names. A variant with one of these names would read as a
/// type alias, which Futuruna does not have.
const BUILTIN_TYPE_NAMES: &[&str] = &[
    "Int", "Float", "String", "Bool", "Char", "Unit", "List", "Map", "Set", "Tuple", "Option",
    "Result", "Stream",
];

/// Types that accept trait implementations without a Futuruna declaration.
const BUILTIN_IMPL_TYPES: &[&str] = &[
    "Int", "Float", "String", "Bool", "Char", "Unit", "List", "Map", "Set",
];

impl TypeChecker {
    /// Span of a declaration name recorded by the parser, keyed as
    /// `variant:Name`, `impl:Trait:Type` or `impl:Trait:Type:method`.
    fn declaration_name_span(&mut self, key: &str) -> Option<Span> {
        if self
            .declaration_source_spans
            .as_ref()
            .is_none_or(|(source, _)| source != &self.source_text)
        {
            let mut parser =
                Parser::new(Lexer::new(&self.source_text).tokenize(), &self.source_text);
            parser.declaration_name_spans = Some(Vec::new());
            let mut spans = BTreeMap::new();
            if parser.parse_program().is_ok() {
                for (name, span) in parser.declaration_name_spans.unwrap() {
                    spans.entry(name).or_insert(span);
                }
            }
            self.declaration_source_spans = Some((self.source_text.clone(), spans));
        }
        self.declaration_source_spans
            .as_ref()
            .and_then(|(_, spans)| spans.get(key))
            .copied()
    }

    fn declaration_error(&mut self, key: &str, message: String) {
        match self.declaration_name_span(key) {
            Some(span) => self.error_at_span(span, message),
            None => self.error(message),
        }
    }

    /// Variant names may not reuse a prelude or list constructor owned by a
    /// different type, nor a built-in type name.
    pub(super) fn check_variant_names(&mut self, owner: &str, variants: &[Variant]) {
        for variant in variants {
            if variant
                .from_type
                .as_deref()
                .is_some_and(|from| from != "__maybe_include")
            {
                continue;
            }
            let name = variant.name.as_str();
            let key = format!("variant:{name}");
            if let Some((_, reserved_owner)) = RESERVED_CONSTRUCTORS
                .iter()
                .find(|(reserved, _)| *reserved == name)
            {
                match reserved_owner {
                    Some(reserved_owner) if *reserved_owner == owner => {}
                    Some(reserved_owner) => self.declaration_error(
                        &key,
                        format!(
                            "constructor `{name}` belongs to the built-in `{reserved_owner}` type; give this variant of `{owner}` a different name"
                        ),
                    ),
                    None => self.declaration_error(
                        &key,
                        format!(
                            "constructor `{name}` is reserved for built-in lists; give this variant of `{owner}` a different name"
                        ),
                    ),
                }
                continue;
            }
            if name != owner && BUILTIN_TYPE_NAMES.contains(&name) {
                self.declaration_error(
                    &key,
                    format!(
                        "variant `{name}` of `{owner}` reuses the built-in type name `{name}`; Futuruna has no type aliases, so use `{name}` directly or wrap it in a record such as `# {owner}(value: {name})`"
                    ),
                );
            }
        }
    }

    fn trait_type_matches(trait_ty: &Ty, impl_ty: &Ty, for_type: &str) -> bool {
        fn substitute(ty: &Ty, for_type: &str) -> Ty {
            match ty {
                Ty::Name(name) if name == "Self" => Ty::Name(for_type.to_string()),
                Ty::App(head, arguments) => Ty::App(
                    Box::new(substitute(head, for_type)),
                    arguments
                        .iter()
                        .map(|argument| substitute(argument, for_type))
                        .collect(),
                ),
                Ty::Arrow(input, output) => Ty::Arrow(
                    Box::new(substitute(input, for_type)),
                    Box::new(substitute(output, for_type)),
                ),
                Ty::Ref(inner) => Ty::Ref(Box::new(substitute(inner, for_type))),
                Ty::MutRef(inner) => Ty::MutRef(Box::new(substitute(inner, for_type))),
                Ty::Shared(inner) => Ty::Shared(Box::new(substitute(inner, for_type))),
                Ty::Optional(inner) => Ty::Optional(Box::new(substitute(inner, for_type))),
                other => other.clone(),
            }
        }
        let expected = substitute(trait_ty, for_type);
        let actual = substitute(impl_ty, for_type);
        Self::canonical_explore_ty_name(&expected) == Self::canonical_explore_ty_name(&actual)
    }

    /// Every `# impl Trait for Type` names a declared trait and type, provides
    /// each trait method without a default body, provides nothing else, and
    /// restates each method's signature: parameter count, `self` position,
    /// declared parameter types and result type.
    pub(super) fn check_trait_impls(&mut self) {
        let mut errors: Vec<(String, String)> = Vec::new();
        let impls = self
            .impl_method_signatures
            .iter()
            .map(|(key, methods)| (key.clone(), methods.clone()))
            .collect::<Vec<_>>();
        for ((trait_name, for_type), methods) in impls {
            let impl_key = format!("impl:{trait_name}:{for_type}");
            let qualified = |name: &str| name.contains("::") || name.contains('.');
            if !qualified(&for_type)
                && !self.types.contains(&for_type)
                && !BUILTIN_IMPL_TYPES.contains(&for_type.as_str())
            {
                errors.push((
                    impl_key.clone(),
                    format!("`# impl {trait_name} for {for_type}` names unknown type `{for_type}`"),
                ));
            }
            let Some(trait_methods) = self.trait_method_signatures.get(&trait_name).cloned() else {
                if !qualified(&trait_name) {
                    errors.push((
                        impl_key.clone(),
                        format!(
                            "`# impl {trait_name} for {for_type}` names unknown trait `{trait_name}`; declare it with `# trait {trait_name} {{ ... }}`"
                        ),
                    ));
                }
                continue;
            };
            let missing = trait_methods
                .iter()
                .filter(|method| method.default_body.is_none())
                .filter(|method| !methods.iter().any(|(name, _, _)| name == &method.name))
                .map(|method| method.name.as_str())
                .collect::<Vec<_>>();
            if !missing.is_empty() {
                errors.push((
                    impl_key.clone(),
                    format!(
                        "`# impl {trait_name} for {for_type}` is missing method{}: {}",
                        if missing.len() == 1 { "" } else { "s" },
                        missing.join(", ")
                    ),
                ));
            }
            for (method_name, params, ret_ty) in &methods {
                let method_key = format!("{impl_key}:{method_name}");
                let Some(required) = trait_methods
                    .iter()
                    .find(|method| &method.name == method_name)
                else {
                    errors.push((
                        method_key,
                        format!(
                            "`# impl {trait_name} for {for_type}` defines `{method_name}`, which is not a method of trait `{trait_name}`"
                        ),
                    ));
                    continue;
                };
                let trait_shape = Self::method_receiver_shape(&required.params);
                let impl_shape = Self::method_receiver_shape(params);
                if impl_shape == MethodReceiverShape::Invalid {
                    errors.push((
                        method_key,
                        format!(
                            "`# impl {trait_name} for {for_type}` method `{method_name}` uses `self` in an invalid position; `self` must be the first parameter"
                        ),
                    ));
                    continue;
                }
                match (trait_shape, impl_shape) {
                    (MethodReceiverShape::SelfFirst, MethodReceiverShape::None) => {
                        errors.push((
                            method_key,
                            format!(
                                "`# impl {trait_name} for {for_type}` method `{method_name}` must declare `self` as its first parameter to match the trait receiver"
                            ),
                        ));
                        continue;
                    }
                    (MethodReceiverShape::None, MethodReceiverShape::SelfFirst) => {
                        errors.push((
                            method_key,
                            format!(
                                "`# impl {trait_name} for {for_type}` method `{method_name}` must not declare `self`; the trait method is not a receiver method"
                            ),
                        ));
                        continue;
                    }
                    _ => {}
                }
                if params.len() != required.params.len() {
                    errors.push((
                        method_key,
                        format!(
                            "`# impl {trait_name} for {for_type}` method `{method_name}` takes {} parameter{} but trait `{trait_name}` declares {}",
                            params.len(),
                            if params.len() == 1 { "" } else { "s" },
                            required.params.len()
                        ),
                    ));
                    continue;
                }
                for (expected, actual) in required.params.iter().zip(params) {
                    let Some(expected_ty) = &expected.ty else {
                        continue;
                    };
                    let matches = actual.ty.as_ref().is_some_and(|actual_ty| {
                        Self::trait_type_matches(expected_ty, actual_ty, &for_type)
                    });
                    if !matches {
                        errors.push((
                            method_key.clone(),
                            format!(
                                "`# impl {trait_name} for {for_type}` method `{method_name}` parameter `{}` must have type `{}` as declared by trait `{trait_name}`",
                                actual.name,
                                Self::canonical_explore_ty_name(expected_ty)
                            ),
                        ));
                    }
                }
                if let Some(expected_ty) = &required.ret_ty {
                    let matches = ret_ty.as_ref().is_some_and(|actual_ty| {
                        Self::trait_type_matches(expected_ty, actual_ty, &for_type)
                    });
                    if !matches {
                        errors.push((
                            method_key,
                            format!(
                                "`# impl {trait_name} for {for_type}` method `{method_name}` must return `{}` as declared by trait `{trait_name}`",
                                Self::canonical_explore_ty_name(expected_ty)
                            ),
                        ));
                    }
                }
            }
        }
        for (key, message) in errors {
            self.declaration_error(&key, message);
        }
    }
}
