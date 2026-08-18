//! Deterministic semantic module interfaces for incremental compilation.
//!
//! A module's source hash identifies its implementation. Its semantic interface
//! identifies the surface that can affect importers. Keeping those identities
//! separate lets a body-only edit rebuild the changed module without forcing
//! every transitive importer through type checking and code generation again.

use super::*;
use quote::ToTokens;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const SEMANTIC_INTERFACE_SCHEMA: &str = "futuruna.semantic-interface.v1";

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticImportKind {
    Flat,
    Qualified,
    ContentAddressed,
    CargoDependency,
    RustUse,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemanticImport {
    pub kind: SemanticImportKind,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_module: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemanticParameter {
    pub name: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub ty: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub inout: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemanticCallable {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub name: String,
    pub parameters: Vec<SemanticParameter>,
    #[serde(rename = "return_type", skip_serializing_if = "Option::is_none")]
    pub return_ty: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signature_details: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemanticField {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemanticVariant {
    pub name: String,
    pub positional: bool,
    pub fields: Vec<SemanticField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemanticTypeDeclaration {
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<SemanticParameter>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<SemanticVariant>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemanticBinding {
    pub kind: String,
    pub name: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub ty: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemanticAnnotation {
    pub name: String,
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemanticCalculationContract {
    pub entry: String,
    pub schema_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemanticMetaReference {
    pub role: String,
    pub binding: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub qualified_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub static_value: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub role_types: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemanticMetaContract {
    pub kind: String,
    pub label: String,
    pub text_hash: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<SemanticMetaReference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symbols: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticModuleInterface {
    pub schema: String,
    pub schema_version: u32,
    pub imports: Vec<SemanticImport>,
    pub exports: Vec<String>,
    pub callables: Vec<SemanticCallable>,
    pub types: Vec<SemanticTypeDeclaration>,
    pub bindings: Vec<SemanticBinding>,
    pub annotations: Vec<SemanticAnnotation>,
    pub calculations: Vec<SemanticCalculationContract>,
    pub metadata: Vec<SemanticMetaContract>,
}

impl SemanticModuleInterface {
    pub fn hash(&self) -> String {
        let canonical = serde_json::to_vec(self).expect("semantic interface serializes");
        sha256_hex(&canonical)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticModuleInterfaceEnvelope {
    pub interface_hash: String,
    pub interface: SemanticModuleInterface,
}

impl From<SemanticModuleInterface> for SemanticModuleInterfaceEnvelope {
    fn from(interface: SemanticModuleInterface) -> Self {
        let interface_hash = interface.hash();
        Self {
            interface_hash,
            interface,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticModuleGraphEntry {
    pub path: String,
    pub content_hash: String,
    pub interface_hash: String,
    pub dependency_hash: String,
    pub imports: Vec<String>,
    pub interface: SemanticModuleInterface,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticModuleGraph {
    pub schema: String,
    pub schema_version: u32,
    pub root: String,
    pub root_dependency_hash: String,
    pub modules: Vec<SemanticModuleGraphEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticDependencyNode {
    pub interface_hash: String,
    pub imports: BTreeSet<String>,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn hash_segment(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

fn canonical_expr(expr: &Expr) -> String {
    format!("{:?}", strip_spans_expr(expr))
}

fn canonical_condition_hash(expr: &Expr) -> String {
    sha256_hex(canonical_expr(expr).as_bytes())
}

fn semantic_param(param: &Param) -> SemanticParameter {
    SemanticParameter {
        name: param.name.clone(),
        ty: param.ty.as_ref().map(ToString::to_string),
        inout: param.inout,
    }
}

fn infer_semantic_expr_type(
    checker: &TypeChecker,
    expr: &Expr,
    locals: &BTreeMap<String, String>,
) -> Option<String> {
    checker
        .infer_expr_type_name_with_locals(expr, locals)
        .or_else(|| match &expr.kind {
            ExprKind::BinOp(operator, left, right) => {
                if matches!(
                    operator.as_str(),
                    "==" | "!=" | "<" | "<=" | ">" | ">=" | "&&" | "||"
                ) {
                    return Some("Bool".to_string());
                }
                let left = infer_semantic_expr_type(checker, left, locals)?;
                let right = infer_semantic_expr_type(checker, right, locals)?;
                if operator == "+" && (left == "String" || right == "String") {
                    Some("String".to_string())
                } else if left == "Float" || right == "Float" {
                    Some("Float".to_string())
                } else if left == right {
                    Some(left)
                } else {
                    None
                }
            }
            ExprKind::UnOp(operator, value) => {
                if operator == "!" || operator == "not" {
                    Some("Bool".to_string())
                } else {
                    infer_semantic_expr_type(checker, value, locals)
                }
            }
            ExprKind::Block(statements) => {
                let mut block_locals = locals.clone();
                let mut result = None;
                for statement in statements {
                    result = match statement {
                        Stmt::Expr(value) => {
                            infer_semantic_expr_type(checker, value, &block_locals)
                        }
                        Stmt::Bind(Pat::Var(name), explicit, value) => {
                            let ty = explicit.as_ref().map(ToString::to_string).or_else(|| {
                                infer_semantic_expr_type(checker, value, &block_locals)
                            });
                            if let Some(ty) = &ty {
                                block_locals.insert(name.clone(), ty.clone());
                            }
                            ty
                        }
                        _ => None,
                    };
                }
                result
            }
            ExprKind::If(_, then_value, else_value) => {
                let then_ty = infer_semantic_expr_type(checker, then_value, locals);
                let else_ty = infer_semantic_expr_type(checker, else_value, locals);
                (then_ty == else_ty).then_some(then_ty).flatten()
            }
            ExprKind::Pipe(_, callable) => infer_semantic_expr_type(checker, callable, locals),
            ExprKind::Try(value) => infer_semantic_expr_type(checker, value, locals),
            _ => None,
        })
}

fn inferred_callable_return(
    checker: &TypeChecker,
    owner: Option<&str>,
    params: &[Param],
    explicit: Option<&Ty>,
    body: &Expr,
) -> Option<String> {
    explicit.map(ToString::to_string).or_else(|| {
        let mut locals = BTreeMap::new();
        if let Some(owner) = owner {
            locals.insert("self".to_string(), owner.to_string());
        }
        for param in params {
            if let Some(ty) = &param.ty {
                if let Some(type_name) = TypeChecker::type_name_from_ty(ty) {
                    locals.insert(param.name.clone(), type_name);
                }
            }
        }
        infer_semantic_expr_type(checker, body, &locals)
    })
}

fn collect_function_callable(
    checker: &TypeChecker,
    defn: &Defn,
    kind: &str,
    owner: Option<&str>,
    callables: &mut Vec<SemanticCallable>,
) {
    match defn {
        Defn::Fn {
            name,
            params,
            ret_ty,
            effects,
            body,
        } => callables.push(SemanticCallable {
            kind: kind.to_string(),
            owner: owner.map(str::to_string),
            name: name.clone(),
            parameters: params.iter().map(semantic_param).collect(),
            return_ty: inferred_callable_return(checker, owner, params, ret_ty.as_ref(), body),
            effects: effects.clone(),
            signature_details: Vec::new(),
        }),
        Defn::Actor {
            name,
            state_param,
            handlers,
        } => {
            callables.push(SemanticCallable {
                kind: "actor_constructor".to_string(),
                owner: owner.map(str::to_string),
                name: name.clone(),
                parameters: vec![semantic_param(state_param)],
                return_ty: Some(name.clone()),
                effects: Vec::new(),
                signature_details: Vec::new(),
            });
            for handler in handlers {
                callables.push(SemanticCallable {
                    kind: "actor_handler".to_string(),
                    owner: Some(name.clone()),
                    name: format!("{:?}", handler.msg_pat),
                    parameters: Vec::new(),
                    return_ty: None,
                    effects: Vec::new(),
                    signature_details: Vec::new(),
                });
            }
        }
        Defn::Module { .. } => {}
    }
}

fn rule_head(rule: &Rule) -> Option<&Expr> {
    match rule {
        Rule::Clause { head, .. } | Rule::Default { head, .. } | Rule::Exception { head, .. } => {
            Some(head)
        }
        Rule::ReactiveScope { .. } => None,
    }
}

fn rule_value(rule: &Rule) -> Option<&Expr> {
    match rule {
        Rule::Clause {
            body: Some(body), ..
        } => Some(body),
        Rule::Default { value, .. } | Rule::Exception { value, .. } => Some(value),
        Rule::Clause { body: None, .. } | Rule::ReactiveScope { .. } => None,
    }
}

fn semantic_rule_parameters(head: &Expr) -> Vec<SemanticParameter> {
    let ExprKind::App(_, args) = &head.kind else {
        return Vec::new();
    };
    args.iter()
        .enumerate()
        .map(|(index, arg)| {
            let (inner, ty) = TypeChecker::typed_rule_arg_parts(arg)
                .map(|(inner, ty)| (inner, Some(ty.to_string())))
                .unwrap_or((arg, None));
            SemanticParameter {
                name: TypeChecker::rule_head_param_name(inner)
                    .unwrap_or_else(|| format!("_{}", index + 1)),
                ty,
                inout: false,
            }
        })
        .collect()
}

fn collect_rule_callables(
    checker: &TypeChecker,
    stmts: &[Stmt],
    owner: Option<&str>,
    callables: &mut Vec<SemanticCallable>,
) {
    let mut seen = BTreeSet::new();
    for stmt in stmts {
        let Stmt::Rule(rule) = stmt else {
            continue;
        };
        let Some((name, arity)) = TypeChecker::rule_name_arity(rule) else {
            continue;
        };
        if !seen.insert((name.clone(), arity)) {
            continue;
        }
        let return_ty = owner
            .and_then(|owner| checker.scoped_member_return_type(owner, &name))
            .or_else(|| checker.rule_return_types.get(&name).cloned())
            .or_else(|| {
                let head = rule_head(rule)?;
                let locals = checker.rule_head_local_types(head);
                infer_semantic_expr_type(checker, rule_value(rule)?, &locals)
            });
        callables.push(SemanticCallable {
            kind: if owner.is_some() {
                "rule_scope_rule".to_string()
            } else {
                "rule".to_string()
            },
            owner: owner.map(str::to_string),
            name,
            parameters: rule_head(rule)
                .map(semantic_rule_parameters)
                .unwrap_or_default(),
            return_ty,
            effects: Vec::new(),
            signature_details: Vec::new(),
        });
    }
}

fn semantic_variant(variant: &Variant) -> SemanticVariant {
    SemanticVariant {
        name: variant.name.clone(),
        positional: variant.positional,
        fields: variant
            .fields
            .iter()
            .map(|field| SemanticField {
                name: field.name.clone(),
                ty: field.ty.to_string(),
            })
            .collect(),
        from_type: variant.from_type.clone(),
    }
}

fn collect_type_interface(
    checker: &TypeChecker,
    decl: &TypeDecl,
    callables: &mut Vec<SemanticCallable>,
    types: &mut Vec<SemanticTypeDeclaration>,
) {
    match decl {
        TypeDecl::ADT {
            name,
            params,
            variants,
            methods,
            except_from,
        } => {
            types.push(SemanticTypeDeclaration {
                kind: "adt".to_string(),
                name: name.clone(),
                parameters: params.iter().map(semantic_param).collect(),
                variants: variants.iter().map(semantic_variant).collect(),
                details: except_from
                    .as_ref()
                    .map(|(source, excluded)| {
                        vec![format!("except:{}:{}", source, excluded.join(","))]
                    })
                    .unwrap_or_default(),
            });
            for method in methods {
                collect_function_callable(checker, method, "method", Some(name), callables);
            }
        }
        TypeDecl::WhenType {
            name,
            condition,
            variants,
            except_from,
        } => {
            let mut details = vec![format!("condition:{}", canonical_condition_hash(condition))];
            if let Some((source, excluded)) = except_from {
                details.push(format!("except:{}:{}", source, excluded.join(",")));
            }
            types.push(SemanticTypeDeclaration {
                kind: "conditional_type".to_string(),
                name: name.clone(),
                parameters: Vec::new(),
                variants: variants.iter().map(semantic_variant).collect(),
                details,
            });
        }
        TypeDecl::EffectDecl { name, ops } => {
            types.push(SemanticTypeDeclaration {
                kind: "effect".to_string(),
                name: name.clone(),
                parameters: Vec::new(),
                variants: Vec::new(),
                details: Vec::new(),
            });
            for (op_name, params, ret_ty) in ops {
                callables.push(SemanticCallable {
                    kind: "effect_operation".to_string(),
                    owner: Some(name.clone()),
                    name: op_name.clone(),
                    parameters: params.iter().map(semantic_param).collect(),
                    return_ty: ret_ty.as_ref().map(ToString::to_string),
                    effects: vec![name.clone()],
                    signature_details: Vec::new(),
                });
            }
        }
        TypeDecl::TraitDecl {
            name,
            params,
            methods,
        } => {
            types.push(SemanticTypeDeclaration {
                kind: "trait".to_string(),
                name: name.clone(),
                parameters: params.iter().map(semantic_param).collect(),
                variants: Vec::new(),
                details: Vec::new(),
            });
            for method in methods {
                callables.push(SemanticCallable {
                    kind: "trait_method".to_string(),
                    owner: Some(name.clone()),
                    name: method.name.clone(),
                    parameters: method.params.iter().map(semantic_param).collect(),
                    return_ty: method.ret_ty.as_ref().map(ToString::to_string),
                    effects: Vec::new(),
                    signature_details: Vec::new(),
                });
            }
        }
        TypeDecl::ImplBlock {
            trait_name,
            for_type,
            methods,
        } => {
            let owner = format!("{} for {}", trait_name, for_type);
            types.push(SemanticTypeDeclaration {
                kind: "trait_impl".to_string(),
                name: owner.clone(),
                parameters: Vec::new(),
                variants: Vec::new(),
                details: vec![trait_name.clone(), for_type.clone()],
            });
            for method in methods {
                collect_function_callable(checker, method, "impl_method", Some(&owner), callables);
            }
        }
        TypeDecl::RuleScope { name, params, body } => {
            types.push(SemanticTypeDeclaration {
                kind: "rule_scope".to_string(),
                name: name.clone(),
                parameters: params.iter().map(semantic_param).collect(),
                variants: Vec::new(),
                details: Vec::new(),
            });
            collect_rule_callables(checker, body, Some(name), callables);
            for stmt in body {
                if let Stmt::Defn(defn) = stmt {
                    collect_function_callable(
                        checker,
                        defn,
                        "rule_scope_function",
                        Some(name),
                        callables,
                    );
                }
            }
        }
    }
}

fn collect_pattern_names(pattern: &Pat, names: &mut Vec<String>) {
    match pattern {
        Pat::Var(name) => names.push(name.clone()),
        Pat::Con(_, fields) => {
            for field in fields {
                collect_pattern_names(field, names);
            }
        }
        Pat::NamedCon(_, fields) => {
            for (_, field) in fields {
                collect_pattern_names(field, names);
            }
        }
        Pat::As(inner, name) => {
            collect_pattern_names(inner, names);
            names.push(name.clone());
        }
        Pat::Wild | Pat::Lit(_) => {}
    }
}

fn rust_signature_callable(
    signature: &syn::Signature,
    kind: &str,
    owner: Option<&str>,
) -> SemanticCallable {
    let parameters = signature
        .inputs
        .iter()
        .map(|input| match input {
            syn::FnArg::Receiver(receiver) => SemanticParameter {
                name: "self".to_string(),
                ty: Some(receiver.to_token_stream().to_string()),
                inout: receiver.mutability.is_some(),
            },
            syn::FnArg::Typed(parameter) => SemanticParameter {
                name: parameter.pat.to_token_stream().to_string(),
                ty: Some(parameter.ty.to_token_stream().to_string()),
                inout: false,
            },
        })
        .collect();
    let return_ty = match &signature.output {
        syn::ReturnType::Default => None,
        syn::ReturnType::Type(_, ty) => Some(ty.to_token_stream().to_string()),
    };
    let mut effects = Vec::new();
    if signature.constness.is_some() {
        effects.push("const".to_string());
    }
    if signature.asyncness.is_some() {
        effects.push("async".to_string());
    }
    if signature.unsafety.is_some() {
        effects.push("unsafe".to_string());
    }
    if let Some(abi) = &signature.abi {
        effects.push(format!("extern:{}", abi.to_token_stream()));
    }
    SemanticCallable {
        kind: kind.to_string(),
        owner: owner.map(str::to_string),
        name: signature.ident.to_string(),
        parameters,
        return_ty,
        effects,
        signature_details: vec![signature.to_token_stream().to_string()],
    }
}

fn nested_owner(owner: Option<&str>, member: &str) -> String {
    owner
        .map(|owner| format!("{}::{}", owner, member))
        .unwrap_or_else(|| member.to_string())
}

fn collect_rust_items(
    items: &[syn::Item],
    owner: Option<&str>,
    callables: &mut Vec<SemanticCallable>,
) {
    for item in items {
        match item {
            syn::Item::Fn(function) => callables.push(rust_signature_callable(
                &function.sig,
                "rust_function",
                owner,
            )),
            syn::Item::Impl(implementation) => {
                let impl_owner = nested_owner(
                    owner,
                    &format!("impl {}", implementation.self_ty.to_token_stream()),
                );
                for item in &implementation.items {
                    if let syn::ImplItem::Fn(function) = item {
                        callables.push(rust_signature_callable(
                            &function.sig,
                            "rust_impl_method",
                            Some(&impl_owner),
                        ));
                    }
                }
            }
            syn::Item::Trait(trait_decl) => {
                let trait_owner = nested_owner(owner, &trait_decl.ident.to_string());
                for item in &trait_decl.items {
                    if let syn::TraitItem::Fn(function) = item {
                        callables.push(rust_signature_callable(
                            &function.sig,
                            "rust_trait_method",
                            Some(&trait_owner),
                        ));
                    }
                }
            }
            syn::Item::ForeignMod(foreign) => {
                let foreign_owner = nested_owner(owner, &foreign.abi.to_token_stream().to_string());
                for item in &foreign.items {
                    if let syn::ForeignItem::Fn(function) = item {
                        callables.push(rust_signature_callable(
                            &function.sig,
                            "rust_foreign_function",
                            Some(&foreign_owner),
                        ));
                    }
                }
            }
            syn::Item::Mod(module) => {
                if let Some((_, items)) = &module.content {
                    let module_owner = nested_owner(owner, &module.ident.to_string());
                    collect_rust_items(items, Some(&module_owner), callables);
                }
            }
            _ => {}
        }
    }
}

fn collect_rust_block_callables(
    code: &str,
    owner: Option<&str>,
    callables: &mut Vec<SemanticCallable>,
) {
    if let Ok(file) = syn::parse_file(code) {
        collect_rust_items(&file.items, owner, callables);
    }
}

fn collect_statement_interfaces(
    checker: &TypeChecker,
    stmts: &[Stmt],
    owner: Option<&str>,
    callables: &mut Vec<SemanticCallable>,
    types: &mut Vec<SemanticTypeDeclaration>,
    bindings: &mut Vec<SemanticBinding>,
) {
    collect_rule_callables(checker, stmts, owner, callables);
    for stmt in stmts {
        match stmt {
            Stmt::Defn(Defn::Module { name, body }) => {
                collect_statement_interfaces(checker, body, Some(name), callables, types, bindings)
            }
            Stmt::Defn(defn) => {
                collect_function_callable(checker, defn, "function", owner, callables)
            }
            Stmt::TypeDecl(decl) => collect_type_interface(checker, decl, callables, types),
            Stmt::Bind(pattern, explicit_ty, value) => {
                let inferred = explicit_ty
                    .as_ref()
                    .map(ToString::to_string)
                    .or_else(|| checker.infer_expr_type_name(value));
                let mut names = Vec::new();
                collect_pattern_names(pattern, &mut names);
                for name in names {
                    bindings.push(SemanticBinding {
                        kind: "binding".to_string(),
                        name,
                        ty: inferred.clone(),
                    });
                }
            }
            Stmt::StreamBind(name, value) => bindings.push(SemanticBinding {
                kind: "stream".to_string(),
                name: name.clone(),
                ty: checker.infer_expr_type_name(value),
            }),
            Stmt::Invariant { name, .. } => bindings.push(SemanticBinding {
                kind: "invariant".to_string(),
                name: name.clone(),
                ty: Some("Bool".to_string()),
            }),
            Stmt::Rule(Rule::ReactiveScope { name, body }) => {
                bindings.push(SemanticBinding {
                    kind: "reactive_scope".to_string(),
                    name: name.clone(),
                    ty: None,
                });
                for child in body {
                    match child {
                        Stmt::Bind(Pat::Var(child_name), explicit_ty, value) => {
                            bindings.push(SemanticBinding {
                                kind: format!("reactive_scope_binding:{}", name),
                                name: child_name.clone(),
                                ty: explicit_ty
                                    .as_ref()
                                    .map(ToString::to_string)
                                    .or_else(|| checker.infer_expr_type_name(value)),
                            });
                        }
                        Stmt::StreamBind(child_name, value) => bindings.push(SemanticBinding {
                            kind: format!("reactive_scope_stream:{}", name),
                            name: child_name.clone(),
                            ty: checker.infer_expr_type_name(value),
                        }),
                        _ => {}
                    }
                }
            }
            Stmt::RustBlock(code) => collect_rust_block_callables(code, owner, callables),
            Stmt::Rule(_)
            | Stmt::Use(_)
            | Stmt::Import(_)
            | Stmt::QualifiedImport(_, _)
            | Stmt::HashImport(_, _)
            | Stmt::Depend(_, _)
            | Stmt::Annot(_, _)
            | Stmt::MonadicBind(_, _, _)
            | Stmt::For(_, _, _)
            | Stmt::While(_, _)
            | Stmt::Send(_, _)
            | Stmt::StreamSub(_, _)
            | Stmt::Prove { .. }
            | Stmt::Explore(_)
            | Stmt::Assert(_, _)
            | Stmt::Retract(_, _)
            | Stmt::Abort
            | Stmt::Expr(_) => {}
        }
    }
}

fn resolved_import(path: &str, source_dir: Option<&str>) -> Option<String> {
    let source_dir = source_dir?;
    let resolved = TypeChecker::resolve_tc_import(path, source_dir)?;
    Some(
        std::fs::canonicalize(&resolved)
            .unwrap_or_else(|_| Path::new(&resolved).to_path_buf())
            .to_string_lossy()
            .to_string(),
    )
}

fn collect_imports(stmts: &[Stmt], source_dir: Option<&str>) -> Vec<SemanticImport> {
    let mut imports = Vec::new();
    for stmt in stmts {
        let import = match stmt {
            Stmt::Import(path) => Some(SemanticImport {
                kind: SemanticImportKind::Flat,
                path: path.clone(),
                alias: None,
                selection: None,
                resolved_module: resolved_import(path, source_dir),
            }),
            Stmt::QualifiedImport(alias, path) => Some(SemanticImport {
                kind: SemanticImportKind::Qualified,
                path: path.clone(),
                alias: Some(alias.clone()),
                selection: None,
                resolved_module: resolved_import(path, source_dir),
            }),
            Stmt::HashImport(hash, path) => Some(SemanticImport {
                kind: SemanticImportKind::ContentAddressed,
                path: path.clone(),
                alias: None,
                selection: Some(hash.clone()),
                resolved_module: resolved_import(path, source_dir),
            }),
            Stmt::Depend(name, version) => Some(SemanticImport {
                kind: SemanticImportKind::CargoDependency,
                path: name.clone(),
                alias: None,
                selection: Some(version.clone()),
                resolved_module: None,
            }),
            Stmt::Use(path) => Some(SemanticImport {
                kind: SemanticImportKind::RustUse,
                path: path.clone(),
                alias: None,
                selection: None,
                resolved_module: None,
            }),
            _ => None,
        };
        if let Some(import) = import {
            imports.push(import);
        }
    }
    imports.sort();
    imports.dedup();
    imports
}

fn collect_annotations(stmts: &[Stmt]) -> Vec<SemanticAnnotation> {
    let mut annotations = stmts
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Annot(name, _) if name == "print" || name == "comptime" => None,
            Stmt::Annot(name, args) => Some(SemanticAnnotation {
                name: name.clone(),
                arguments: args.iter().map(canonical_expr).collect(),
            }),
            _ => None,
        })
        .collect::<Vec<_>>();
    annotations.sort();
    annotations.dedup();
    annotations
}

fn collect_metadata(
    source: &str,
    stmts: &[Stmt],
    source_dir: Option<String>,
) -> Result<Vec<SemanticMetaContract>, Vec<Diagnostic>> {
    if !source.contains("--@") {
        return Ok(Vec::new());
    }
    let index = scan_meta_comments_with_dir_from_stmts(source, stmts, source_dir);
    if !index.diagnostics.is_empty() {
        return Err(index
            .diagnostics
            .iter()
            .map(|diagnostic| {
                Diagnostic::error(format!(
                    "metadata line {}: {}",
                    diagnostic.line, diagnostic.message
                ))
            })
            .collect());
    }
    let mut metadata = index
        .anchors
        .iter()
        .map(|anchor| {
            let mut references = anchor
                .references
                .iter()
                .map(|reference| SemanticMetaReference {
                    role: reference.role.clone(),
                    binding: reference.binding_name.clone(),
                    qualified_type: reference.qualified_type.clone(),
                    static_value: reference.static_value.clone(),
                    role_types: reference.meta_role_types.iter().cloned().collect(),
                })
                .collect::<Vec<_>>();
            references.sort();
            references.dedup();
            let mut symbols = index
                .spans
                .iter()
                .filter(|span| span.label == anchor.label)
                .flat_map(|span| span.symbols.iter())
                .map(|symbol| format!("{}:{}", symbol.kind, symbol.name))
                .collect::<Vec<_>>();
            symbols.sort();
            symbols.dedup();
            SemanticMetaContract {
                kind: anchor.kind.clone(),
                label: anchor.label.clone(),
                text_hash: sha256_hex(anchor.text.as_bytes()),
                references,
                symbols,
            }
        })
        .collect::<Vec<_>>();
    let anchored_labels = index
        .anchors
        .iter()
        .map(|anchor| anchor.label.as_str())
        .collect::<BTreeSet<_>>();
    metadata.extend(
        index
            .spans
            .iter()
            .filter(|span| !anchored_labels.contains(span.label.as_str()))
            .map(|span| {
                let mut symbols = span
                    .symbols
                    .iter()
                    .map(|symbol| format!("{}:{}", symbol.kind, symbol.name))
                    .collect::<Vec<_>>();
                symbols.sort();
                symbols.dedup();
                SemanticMetaContract {
                    kind: "span".to_string(),
                    label: span.label.clone(),
                    text_hash: sha256_hex(b""),
                    references: Vec::new(),
                    symbols,
                }
            }),
    );
    metadata.sort();
    metadata.dedup();
    Ok(metadata)
}

/// Build the semantic surface of one source module. Imported declarations are
/// available to inference and checking, but only declarations authored in
/// `stmts` are recorded in the returned interface.
fn build_semantic_module_interface_with_checker(
    checker: &TypeChecker,
    stmts: &[Stmt],
    source: &str,
    source_dir: Option<String>,
) -> Result<SemanticModuleInterface, Vec<Diagnostic>> {
    let calculations = if stmts
        .iter()
        .any(|stmt| matches!(stmt, Stmt::Annot(name, _) if name == "calculate"))
    {
        match calculate::extract_calculation_contracts_with_checker(
            stmts,
            source,
            source_dir.clone(),
            checker,
        ) {
            Ok(contracts) => contracts
                .into_iter()
                .map(|contract| SemanticCalculationContract {
                    entry: contract.entry,
                    schema_hash: contract.schema_hash,
                })
                .collect(),
            Err(diagnostics) => return Err(diagnostics),
        }
    } else {
        Vec::new()
    };

    let mut callables = Vec::new();
    let mut types = Vec::new();
    let mut bindings = Vec::new();
    collect_statement_interfaces(
        checker,
        stmts,
        None,
        &mut callables,
        &mut types,
        &mut bindings,
    );
    callables.sort();
    callables.dedup();
    types.sort();
    types.dedup();
    bindings.sort();
    bindings.dedup();
    let mut calculations = calculations;
    calculations.sort();
    calculations.dedup();

    Ok(SemanticModuleInterface {
        schema: SEMANTIC_INTERFACE_SCHEMA.to_string(),
        schema_version: 1,
        imports: collect_imports(stmts, source_dir.as_deref()),
        exports: TypeChecker::exported_names_from_stmts(stmts)
            .into_iter()
            .collect(),
        callables,
        types,
        bindings,
        annotations: collect_annotations(stmts),
        calculations,
        metadata: collect_metadata(source, stmts, source_dir)?,
    })
}

/// Build the semantic surface of one source module. Imported declarations are
/// available to inference and checking, but only declarations authored in
/// `stmts` are recorded in the returned interface.
pub fn build_semantic_module_interface(
    stmts: &[Stmt],
    source: &str,
    source_dir: Option<String>,
    use_prelude: bool,
) -> Result<SemanticModuleInterface, Vec<Diagnostic>> {
    let program = if use_prelude {
        prepend_prelude(parse_prelude(), stmts)
    } else {
        stmts.to_vec()
    };
    let mut checker = TypeChecker::new();
    checker.source_dir = source_dir.clone();
    checker.source_text = source.to_string();
    checker.collect_declarations(&program);
    checker.infer_rule_return_types(&program);
    checker.infer_top_level_binding_types(&program);
    checker.check_program(&program);
    if !checker.diagnostics.is_empty() {
        return Err(checker.diagnostics);
    }
    build_semantic_module_interface_with_checker(&checker, stmts, source, source_dir)
}

struct PendingGraphEntry {
    path: String,
    content_hash: String,
    imports: Vec<String>,
    interface: SemanticModuleInterface,
}

fn canonical_module_path(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}

fn collect_semantic_graph_module(
    path: &Path,
    source: &str,
    stmts: &[Stmt],
    checker: &TypeChecker,
    visited: &mut BTreeSet<String>,
    entries: &mut BTreeMap<String, PendingGraphEntry>,
) -> Result<(), Vec<Diagnostic>> {
    let canonical_path = canonical_module_path(path);
    if !visited.insert(canonical_path.clone()) {
        return Ok(());
    }
    let source_dir = Path::new(&canonical_path)
        .parent()
        .map(|parent| parent.to_string_lossy().to_string());
    let interface =
        build_semantic_module_interface_with_checker(checker, stmts, source, source_dir)?;
    let imports = interface
        .imports
        .iter()
        .filter(|import| {
            matches!(
                import.kind,
                SemanticImportKind::Flat
                    | SemanticImportKind::Qualified
                    | SemanticImportKind::ContentAddressed
            )
        })
        .filter_map(|import| import.resolved_module.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    entries.insert(
        canonical_path.clone(),
        PendingGraphEntry {
            path: canonical_path,
            content_hash: sha256_hex(source.as_bytes()),
            imports: imports.clone(),
            interface,
        },
    );

    for imported in imports {
        let imported_path = Path::new(&imported);
        let module = match parse_source_module_file_cached(imported_path) {
            Ok(module) => module,
            Err(error) => return Err(vec![Diagnostic::error(error)]),
        };
        collect_semantic_graph_module(
            imported_path,
            module.source(),
            module.statements(),
            checker,
            visited,
            entries,
        )?;
    }
    Ok(())
}

/// Build deterministic local-interface and transitive-dependency hashes for a
/// complete import graph using one whole-graph type-checking pass.
pub fn build_semantic_module_graph(
    root_path: &Path,
    source: &str,
    root_stmts: &[Stmt],
    use_prelude: bool,
) -> Result<SemanticModuleGraph, Vec<Diagnostic>> {
    let program = if use_prelude {
        prepend_prelude(parse_prelude(), root_stmts)
    } else {
        root_stmts.to_vec()
    };
    let root = canonical_module_path(root_path);
    let mut checker = TypeChecker::new();
    checker.source_dir = Path::new(&root)
        .parent()
        .map(|parent| parent.to_string_lossy().to_string());
    checker.source_text = source.to_string();
    checker.collect_declarations(&program);
    checker.infer_rule_return_types(&program);
    checker.infer_top_level_binding_types(&program);
    checker.check_program(&program);
    if !checker.diagnostics.is_empty() {
        return Err(checker.diagnostics);
    }

    let mut pending = BTreeMap::new();
    collect_semantic_graph_module(
        Path::new(&root),
        source,
        root_stmts,
        &checker,
        &mut BTreeSet::new(),
        &mut pending,
    )?;
    let dependency_nodes = pending
        .iter()
        .map(|(path, entry)| {
            (
                path.clone(),
                SemanticDependencyNode {
                    interface_hash: entry.interface.hash(),
                    imports: entry.imports.iter().cloned().collect(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let dependency_hashes = semantic_dependency_hashes(&dependency_nodes)
        .map_err(|error| vec![Diagnostic::error(error)])?;
    let modules = pending
        .into_values()
        .map(|entry| SemanticModuleGraphEntry {
            interface_hash: entry.interface.hash(),
            dependency_hash: dependency_hashes[&entry.path].clone(),
            path: entry.path,
            content_hash: entry.content_hash,
            imports: entry.imports,
            interface: entry.interface,
        })
        .collect::<Vec<_>>();
    Ok(SemanticModuleGraph {
        schema: "futuruna.semantic-module-graph.v1".to_string(),
        schema_version: 1,
        root: root.clone(),
        root_dependency_hash: dependency_hashes[&root].clone(),
        modules,
    })
}

struct TarjanState {
    next_index: usize,
    indices: BTreeMap<String, usize>,
    lowlinks: BTreeMap<String, usize>,
    stack: Vec<String>,
    on_stack: BTreeSet<String>,
    components: Vec<Vec<String>>,
}

impl TarjanState {
    fn new() -> Self {
        Self {
            next_index: 0,
            indices: BTreeMap::new(),
            lowlinks: BTreeMap::new(),
            stack: Vec::new(),
            on_stack: BTreeSet::new(),
            components: Vec::new(),
        }
    }

    fn visit(&mut self, module: &str, nodes: &BTreeMap<String, SemanticDependencyNode>) {
        let index = self.next_index;
        self.next_index += 1;
        self.indices.insert(module.to_string(), index);
        self.lowlinks.insert(module.to_string(), index);
        self.stack.push(module.to_string());
        self.on_stack.insert(module.to_string());

        let imports = nodes[module].imports.iter().cloned().collect::<Vec<_>>();
        for imported in imports {
            if !self.indices.contains_key(&imported) {
                self.visit(&imported, nodes);
                let imported_lowlink = self.lowlinks[&imported];
                let lowlink = self.lowlinks[module].min(imported_lowlink);
                self.lowlinks.insert(module.to_string(), lowlink);
            } else if self.on_stack.contains(&imported) {
                let imported_index = self.indices[&imported];
                let lowlink = self.lowlinks[module].min(imported_index);
                self.lowlinks.insert(module.to_string(), lowlink);
            }
        }

        if self.lowlinks[module] == self.indices[module] {
            let mut component = Vec::new();
            loop {
                let member = self.stack.pop().expect("Tarjan stack contains root");
                self.on_stack.remove(&member);
                let done = member == module;
                component.push(member);
                if done {
                    break;
                }
            }
            component.sort();
            self.components.push(component);
        }
    }
}

fn component_hash(
    component: usize,
    components: &[Vec<String>],
    component_by_module: &BTreeMap<String, usize>,
    nodes: &BTreeMap<String, SemanticDependencyNode>,
    memo: &mut BTreeMap<usize, String>,
) -> String {
    if let Some(hash) = memo.get(&component) {
        return hash.clone();
    }
    let mut hasher = Sha256::new();
    hash_segment(&mut hasher, b"futuruna.semantic-dependencies.v1");
    for module in &components[component] {
        hash_segment(&mut hasher, module.as_bytes());
        hash_segment(&mut hasher, nodes[module].interface_hash.as_bytes());
        for imported in &nodes[module].imports {
            hash_segment(&mut hasher, module.as_bytes());
            hash_segment(&mut hasher, imported.as_bytes());
            let imported_component = component_by_module[imported];
            if imported_component == component {
                hash_segment(&mut hasher, b"internal-cycle-edge");
            } else {
                let imported_hash = component_hash(
                    imported_component,
                    components,
                    component_by_module,
                    nodes,
                    memo,
                );
                hash_segment(&mut hasher, imported_hash.as_bytes());
            }
        }
    }
    let hash = format!("{:x}", hasher.finalize());
    memo.insert(component, hash.clone());
    hash
}

/// Compute the transitive semantic dependency fingerprint for every module.
/// Strongly connected components are hashed as one unit, so cycles terminate
/// deterministically and a public change in any member invalidates every member.
pub fn semantic_dependency_hashes(
    nodes: &BTreeMap<String, SemanticDependencyNode>,
) -> Result<BTreeMap<String, String>, String> {
    for (module, node) in nodes {
        for imported in &node.imports {
            if !nodes.contains_key(imported) {
                return Err(format!(
                    "semantic dependency graph module `{}` imports missing module `{}`",
                    module, imported
                ));
            }
        }
    }

    let mut tarjan = TarjanState::new();
    for module in nodes.keys() {
        if !tarjan.indices.contains_key(module) {
            tarjan.visit(module, nodes);
        }
    }
    tarjan.components.sort();
    let mut component_by_module = BTreeMap::new();
    for (component, members) in tarjan.components.iter().enumerate() {
        for member in members {
            component_by_module.insert(member.clone(), component);
        }
    }

    let mut memo = BTreeMap::new();
    let mut hashes = BTreeMap::new();
    for module in nodes.keys() {
        let component = component_by_module[module];
        let hash = component_hash(
            component,
            &tarjan.components,
            &component_by_module,
            nodes,
            &mut memo,
        );
        hashes.insert(module.clone(), hash);
    }
    Ok(hashes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Vec<Stmt> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        parser.parse_program().expect("test source parses")
    }

    fn interface(source: &str) -> SemanticModuleInterface {
        build_semantic_module_interface(&parse(source), source, None, false)
            .unwrap_or_else(|diagnostics| panic!("interface diagnostics: {diagnostics:?}"))
    }

    fn node(hash: &str, imports: &[&str]) -> SemanticDependencyNode {
        SemanticDependencyNode {
            interface_hash: hash.to_string(),
            imports: imports.iter().map(|name| (*name).to_string()).collect(),
        }
    }

    #[test]
    fn body_only_edits_preserve_interface_but_signature_edits_change_it() {
        let first = interface("> add(value: Int) -> Int { value + 1 }");
        let body_edit = interface("> add(value: Int) -> Int { value + 2 }");
        let signature_edit = interface("> add(value: Int) -> String { \"changed\" }");

        assert_eq!(first.hash(), body_edit.hash());
        assert_ne!(first.hash(), signature_edit.hash());
    }

    #[test]
    fn inferred_and_raw_rust_return_types_participate_in_the_interface() {
        let inferred_int = interface("> answer() { 1 + 1 }");
        let inferred_string = interface("> answer() { \"answer\" }");
        assert_eq!(inferred_int.callables[0].return_ty.as_deref(), Some("Int"));
        assert_eq!(
            inferred_string.callables[0].return_ty.as_deref(),
            Some("String")
        );
        assert_ne!(inferred_int.hash(), inferred_string.hash());

        let rust_first = interface("@ rust {\nfn external(value: i64) -> i64 { value + 1 }\n}\n");
        let rust_body_edit =
            interface("@ rust {\nfn external(value: i64) -> i64 { value + 2 }\n}\n");
        let rust_signature_edit =
            interface("@ rust {\nfn external(value: f64) -> f64 { value + 2.0 }\n}\n");
        assert_eq!(rust_first.hash(), rust_body_edit.hash());
        assert_ne!(rust_first.hash(), rust_signature_edit.hash());
    }

    #[test]
    fn rule_parameter_names_inferred_returns_and_type_layouts_are_hashed() {
        let first = interface("# Input(amount: Int)\n| calculate(input: Input) -> input.amount\n");
        let renamed =
            interface("# Input(amount: Int)\n| calculate(facts: Input) -> facts.amount\n");
        let relaid =
            interface("# Input(amount: String)\n| calculate(input: Input) -> input.amount\n");

        assert_ne!(first.hash(), renamed.hash());
        assert_ne!(first.hash(), relaid.hash());
        assert_eq!(first.callables[0].return_ty.as_deref(), Some("Int"));
    }

    #[test]
    fn import_resolution_is_part_of_the_local_interface() {
        let first = interface("@ import ./first\n");
        let second = interface("@ import ./second\n");
        assert_ne!(first.hash(), second.hash());
        assert_eq!(first.imports[0].path, "./first");
        assert_eq!(second.imports[0].path, "./second");
    }

    #[test]
    fn diamond_dependency_hashes_invalidate_only_on_semantic_changes() {
        let graph = BTreeMap::from([
            ("leaf".to_string(), node("leaf-v1", &[])),
            ("left".to_string(), node("left-v1", &["leaf"])),
            ("right".to_string(), node("right-v1", &["leaf"])),
            ("root".to_string(), node("root-v1", &["left", "right"])),
        ]);
        let first = semantic_dependency_hashes(&graph).expect("diamond hashes");

        let mut body_only = graph.clone();
        body_only.get_mut("leaf").unwrap().interface_hash = "leaf-v1".to_string();
        assert_eq!(
            first,
            semantic_dependency_hashes(&body_only).expect("same interface hashes")
        );

        let mut changed = graph;
        changed.get_mut("leaf").unwrap().interface_hash = "leaf-v2".to_string();
        let second = semantic_dependency_hashes(&changed).expect("changed diamond hashes");
        for module in ["leaf", "left", "right", "root"] {
            assert_ne!(first[module], second[module], "{module} must invalidate");
        }
    }

    #[test]
    fn cyclic_dependency_hashes_are_deterministic_and_invalidate_together() {
        let graph = BTreeMap::from([
            ("a".to_string(), node("a-v1", &["b"])),
            ("b".to_string(), node("b-v1", &["a"])),
            ("root".to_string(), node("root-v1", &["a"])),
        ]);
        let first = semantic_dependency_hashes(&graph).expect("cycle hashes");
        let repeated = semantic_dependency_hashes(&graph).expect("repeated cycle hashes");
        assert_eq!(first, repeated);
        assert_eq!(first["a"], first["b"]);

        let mut changed = graph;
        changed.get_mut("b").unwrap().interface_hash = "b-v2".to_string();
        let second = semantic_dependency_hashes(&changed).expect("changed cycle hashes");
        assert_ne!(first["a"], second["a"]);
        assert_ne!(first["b"], second["b"]);
        assert_ne!(first["root"], second["root"]);
    }

    #[test]
    fn dependency_graph_rejects_missing_modules() {
        let graph = BTreeMap::from([("root".to_string(), node("root-v1", &["missing"]))]);
        let error = semantic_dependency_hashes(&graph).expect_err("missing import rejected");
        assert!(error.contains("missing module `missing`"));
    }

    #[test]
    fn calculation_and_metadata_contract_changes_are_hashed() {
        let first = interface(
            r#"
# Input(amount: Int)
# Result(amount: Int)
# Shape = Circle | Triangle
= source_shape = Circle
--@label:calculation::source:source_shape--
----
Original source text
----
--@begin:calculation--
@ calculate("Readable calculation")
| calculate(input: Input) -> Result(amount = input.amount)
--@end:calculation--
"#,
        );
        let changed_text = interface(
            r#"
# Input(amount: Int)
# Result(amount: Int)
# Shape = Circle | Triangle
= source_shape = Circle
--@label:calculation::source:source_shape--
----
Changed source text
----
--@begin:calculation--
@ calculate("Readable calculation")
| calculate(input: Input) -> Result(amount = input.amount)
--@end:calculation--
"#,
        );
        let changed_label = interface(
            r#"
# Input(amount: Int)
# Result(amount: Int)
# Shape = Circle | Triangle
= source_shape = Circle
--@label:calculation::source:source_shape--
----
Original source text
----
--@begin:calculation--
@ calculate("Renamed calculation")
| calculate(input: Input) -> Result(amount = input.amount)
--@end:calculation--
"#,
        );

        assert_eq!(first.calculations.len(), 1);
        assert_eq!(first.metadata.len(), 1);
        assert_ne!(first.hash(), changed_text.hash());
        assert_ne!(first.hash(), changed_label.hash());
    }

    #[test]
    fn real_module_graph_preserves_dependency_hash_for_dependency_body_edits() {
        let temp = std::env::temp_dir().join(format!(
            "futuruna-semantic-interface-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("graph")
        ));
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).expect("create graph fixture");
        let root_path = temp.join("root.runa");
        let dependency_path = temp.join("dependency.runa");
        let root_source = "@ import ./dependency\n| total(value: Int) -> amount(value)\n";
        std::fs::write(&root_path, root_source).expect("write graph root");
        std::fs::write(&dependency_path, "| amount(value: Int) -> value + 1\n")
            .expect("write graph dependency");
        let root_stmts = parse(root_source);

        let first = build_semantic_module_graph(&root_path, root_source, &root_stmts, false)
            .unwrap_or_else(|diagnostics| panic!("first graph diagnostics: {diagnostics:?}"));
        std::fs::write(&dependency_path, "| amount(value: Int) -> value + 2\n")
            .expect("edit dependency body");
        let body_edit = build_semantic_module_graph(&root_path, root_source, &root_stmts, false)
            .unwrap_or_else(|diagnostics| panic!("body graph diagnostics: {diagnostics:?}"));

        assert_eq!(first.root_dependency_hash, body_edit.root_dependency_hash);
        let first_dependency = first
            .modules
            .iter()
            .find(|module| module.path.ends_with("dependency.runa"))
            .expect("first dependency entry");
        let edited_dependency = body_edit
            .modules
            .iter()
            .find(|module| module.path.ends_with("dependency.runa"))
            .expect("edited dependency entry");
        assert_ne!(
            first_dependency.content_hash,
            edited_dependency.content_hash
        );
        assert_eq!(
            first_dependency.interface_hash,
            edited_dependency.interface_hash
        );

        std::fs::write(&dependency_path, "| amount(number: Int) -> number + 2\n")
            .expect("edit dependency signature");
        let signature_edit =
            build_semantic_module_graph(&root_path, root_source, &root_stmts, false)
                .unwrap_or_else(|diagnostics| {
                    panic!("signature graph diagnostics: {diagnostics:?}")
                });
        assert_ne!(
            first.root_dependency_hash,
            signature_edit.root_dependency_hash
        );

        std::fs::remove_dir_all(&temp).expect("remove graph fixture");
    }
}
