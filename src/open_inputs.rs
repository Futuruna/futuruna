//! Open keyed inputs (`docs/rfcs/open-keyed-inputs.md`).
//!
//! A field declared `@ field(open = True)` holds a `List` of a type with a
//! key field (`@ field(key = True)`) that the case never gives whole: the
//! integrator gives the entries the law asks for. The law reads it only
//! through `lookup(record.field, key)`, which gives the entry with that key.
//! An entry the case does not give is unknown in a partial case, a need at
//! `field[key_field=value]`, and an error naming the key in a complete one.
//! On a closed list `lookup` gives `Some(entry)` or `None`.

use super::*;
use crate::constructor_scope::constructor_display_name;
use serde_json::Value as JsonValue;

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

/// One step of a concrete input path.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PathStep {
    /// A record field, a map key, `$variant` or `$values`.
    Field(String),
    /// A list or set element by its position.
    Index(usize),
    /// `[field=value]`: the entry of an open collection whose key `field`
    /// holds `value` (a JSON string or integer).
    Key { field: String, value: JsonValue },
}

/// The step that names the entry of an open collection by its key:
/// `[day="2027-02-20"]`, the value written as JSON.
pub(crate) fn key_selector(field: &str, value: &JsonValue) -> String {
    format!("[{field}={value}]")
}

/// Split a concrete input path (`accounts[2].balances[day="2027-02-20"]`)
/// into its steps; the empty path is the input itself.
pub(crate) fn parse_input_path(path: &str) -> Option<Vec<PathStep>> {
    let mut steps = Vec::new();
    let mut rest = path;
    while !rest.is_empty() {
        if let Some(inner) = rest.strip_prefix('[') {
            let digits = inner
                .find(|character: char| !character.is_ascii_digit())
                .unwrap_or(inner.len());
            if digits > 0 && inner[digits..].starts_with(']') {
                steps.push(PathStep::Index(inner[..digits].parse().ok()?));
                rest = &inner[digits + 1..];
                continue;
            }
            let equals = inner.find('=')?;
            let field = &inner[..equals];
            if field.is_empty() || field.contains(['.', '[', ']']) {
                return None;
            }
            let text = &inner[equals + 1..];
            let mut values = serde_json::Deserializer::from_str(text).into_iter::<JsonValue>();
            let value = values.next()?.ok()?;
            if !(value.is_string() || value.is_i64()) {
                return None;
            }
            rest = text[values.byte_offset()..].strip_prefix(']')?;
            steps.push(PathStep::Key {
                field: field.to_string(),
                value,
            });
            continue;
        }
        if let Some(after) = rest.strip_prefix('.') {
            rest = after;
        } else if !steps.is_empty() {
            return None;
        }
        let end = rest.find(['.', '[']).unwrap_or(rest.len());
        if end == 0 {
            return None;
        }
        steps.push(PathStep::Field(rest[..end].to_string()));
        rest = &rest[end..];
    }
    Some(steps)
}

/// A key's JSON value: the text of a `String` key, the number of an `Int`.
fn key_json(key: &Value) -> Option<JsonValue> {
    match key {
        Value::Str(text) => Some(JsonValue::String(text.clone())),
        Value::Int(number) => Some(JsonValue::from(*number)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Checks
// ---------------------------------------------------------------------------

/// The named type a `List(T)` holds, as declared.
fn list_element(ty: &Ty) -> Option<&str> {
    let Ty::App(head, arguments) = ty else {
        return None;
    };
    if !matches!(head.as_ref(), Ty::Name(name) if name == "List") || arguments.len() != 1 {
        return None;
    }
    match &arguments[0] {
        Ty::Name(name) => Some(name),
        Ty::App(head, _) => match head.as_ref() {
            Ty::Name(name) => Some(name),
            _ => None,
        },
        _ => None,
    }
}

impl TypeChecker {
    /// Record the open fields and the key field a type declares.
    pub(super) fn register_open_and_key_fields(
        &mut self,
        name: &str,
        variants: &[Variant],
        field_meta: Option<&TypeFieldMeta>,
    ) {
        self.key_fields.remove(name);
        self.open_fields.remove(name);
        let Some(meta) = field_meta else {
            return;
        };
        if let Some(key) = meta.marked("key").next() {
            self.key_fields.insert(name.to_string(), key.field.clone());
        }
        for member in meta.marked("open") {
            let Some(ty) = variants
                .iter()
                .find(|variant| variant.name == member.variant)
                .and_then(|variant| {
                    variant
                        .fields
                        .iter()
                        .find(|field| field.name == member.field)
                })
                .map(|field| field.ty.clone())
            else {
                continue;
            };
            for owner in [name, member.variant.as_str()] {
                self.open_fields
                    .entry(owner.to_string())
                    .or_default()
                    .insert(member.field.clone(), ty.clone());
            }
        }
    }

    /// The declared type of `field` when it is an open field of the type
    /// (or variant) `owner` names.
    pub(super) fn open_field(&self, owner: &str, field: &str) -> Option<&Ty> {
        let owner = Self::canonical_nominal_owner(owner).unwrap_or_else(|| owner.to_string());
        self.open_fields.get(&owner)?.get(field)
    }

    /// `@ field(open = True)` is written `True` or `False` on a field whose
    /// type is a `List` of a type with a key field.
    pub(super) fn check_open_field_declarations(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            let Stmt::TypeDecl(TypeDecl::ADT {
                name,
                variants,
                field_meta: Some(field_meta),
                ..
            }) = stmt
            else {
                continue;
            };
            if let Some(value) = field_meta
                .type_default
                .as_ref()
                .and_then(|default| default.value("open"))
            {
                self.error_at_expr(
                    value,
                    format!("`open` belongs to the field that holds a collection, not to the type `{name}`; write `@ field(open = True)` on the field"),
                );
            }
            for member in &field_meta.members {
                let Some(value) = member.meta.value("open") else {
                    continue;
                };
                match member.meta.flag("open") {
                    Some(true) => {}
                    Some(false) => continue,
                    None => {
                        self.error_at_expr(
                            value,
                            "`@ field` key `open` is written `open = True` or `open = False`"
                                .to_string(),
                        );
                        continue;
                    }
                }
                let binding = if &member.variant == name {
                    format!("{name}::{}", member.field)
                } else {
                    format!("{name}::{}::{}", member.variant, member.field)
                };
                let Some(ty) = variants
                    .iter()
                    .find(|variant| variant.name == member.variant)
                    .and_then(|variant| {
                        variant
                            .fields
                            .iter()
                            .find(|field| field.name == member.field)
                    })
                    .map(|field| &field.ty)
                else {
                    continue;
                };
                let problem = match list_element(ty) {
                    None => Some(format!(
                        "an open collection is a `List` of a type with a key field (`@ field(key = True)`), and `{binding}` holds `{ty}`"
                    )),
                    Some(element) if !self.key_fields.contains_key(element) => Some(format!(
                        "the law looks an entry of an open collection up by its key, and `{element}` declares no key field; mark one `@ field(key = True)`"
                    )),
                    Some(_) => None,
                };
                if let Some(problem) = problem {
                    self.error_at_expr(
                        value,
                        format!("`@ field(open = True)` on `{binding}`: {problem}"),
                    );
                }
            }
        }
    }

    /// `lookup(collection, key)`: a `List` of a type with a key field, and a
    /// key of that field's type. The collection may be an open field: this is
    /// the one place it is read.
    pub(super) fn check_lookup_call(&mut self, expr: &Expr, args: &[Expr]) {
        let collection = &args[0];
        if matches!(collection.kind, ExprKind::Field(_, _)) {
            self.lookup_collections
                .insert(collection as *const Expr as usize);
        }
        let Some(collection_type) = self.ordinary_expression_type(collection) else {
            return;
        };
        let Some(element) = Self::applied_type_argument(&collection_type, "List", 0) else {
            self.error_at_expr(
                collection,
                format!("`lookup` finds an entry of a `List` of a type with a key field (`@ field(key = True)`) by its key; got `{collection_type}`"),
            );
            return;
        };
        let owner = Self::canonical_nominal_owner(&element).unwrap_or_else(|| element.clone());
        let Some(key_field) = self.key_fields.get(&owner).cloned() else {
            self.error_at_expr(
                expr,
                format!("`lookup` finds an entry by its key, and `{element}` declares no key field; mark one `@ field(key = True)`"),
            );
            return;
        };
        let expected = self
            .type_field_types
            .get(&owner)
            .and_then(|fields| fields.get(&key_field))
            .cloned();
        let actual = self.ordinary_expression_type(&args[1]);
        if let (Some(expected), Some(actual)) = (expected, actual) {
            let primitive = |name: &str| -> String {
                match self.scalar_primitive(name) {
                    Some(primitive) => primitive.to_string(),
                    None if self.is_unit_type(name) => "Int".to_string(),
                    None => name.to_string(),
                }
            };
            if expected != actual && primitive(&expected) != primitive(&actual) {
                self.error_at_expr(
                    &args[1],
                    format!("`lookup` in a list of `{element}` takes the key `{key_field}`, a `{expected}`; got `{actual}`"),
                );
            }
        }
    }

    /// An open field is read only as the collection of a `lookup`: the case
    /// never gives all of its entries, so no other read has a true result.
    pub(super) fn check_open_field_read(
        &mut self,
        expression: &Expr,
        receiver_type: Option<&str>,
        field: &str,
    ) {
        if self
            .lookup_collections
            .contains(&(expression as *const Expr as usize))
        {
            return;
        }
        let owner = match receiver_type {
            Some(receiver_type) => {
                if self.open_field(receiver_type, field).is_none() {
                    return;
                }
                Self::canonical_nominal_owner(receiver_type)
                    .unwrap_or_else(|| receiver_type.to_string())
            }
            None => {
                let Some(owner) = self
                    .open_fields
                    .iter()
                    .find(|(_, fields)| fields.contains_key(field))
                    .map(|(owner, _)| owner.clone())
                else {
                    return;
                };
                self.error_at_expr(
                    expression,
                    format!("`{field}` may be the open collection of `{owner}`, and the type of what it is read from is not known here; give it a type. An open collection is read only through `lookup(<record>.{field}, key)`"),
                );
                return;
            }
        };
        self.error_at_expr(
            expression,
            format!("`{field}` of `{owner}` is an open collection (`@ field(open = True)`): the case gives only the entries the law asks for, so it is never read whole. Look an entry up by its key with `lookup(<record>.{field}, key)`"),
        );
    }
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

/// The open fields and key fields of a program's types, by constructor.
#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeOpenInputs {
    /// Constructor (record or variant) name -> open field -> the type of
    /// its entries.
    open: HashMap<String, HashMap<String, String>>,
    /// Type or constructor name -> the key field of its values.
    keys: HashMap<String, String>,
}

impl RuntimeOpenInputs {
    pub(crate) fn register(
        &mut self,
        name: &str,
        variants: &[Variant],
        field_meta: Option<&TypeFieldMeta>,
    ) {
        let Some(meta) = field_meta else {
            return;
        };
        if let Some(key) = meta.marked("key").next() {
            self.keys.insert(name.to_string(), key.field.clone());
            for variant in variants {
                self.keys.insert(variant.name.clone(), key.field.clone());
            }
        }
        for member in meta.marked("open") {
            let element = variants
                .iter()
                .find(|variant| variant.name == member.variant)
                .and_then(|variant| {
                    variant
                        .fields
                        .iter()
                        .find(|field| field.name == member.field)
                })
                .and_then(|field| list_element(&field.ty));
            if let Some(element) = element {
                self.open
                    .entry(member.variant.clone())
                    .or_default()
                    .insert(member.field.clone(), element.to_string());
            }
        }
    }

    fn open_element(&self, constructor: &str, field: &str) -> Option<&str> {
        self.open
            .get(constructor_display_name(constructor).as_ref())?
            .get(field)
            .map(String::as_str)
    }

    fn key(&self, name: &str) -> Option<&str> {
        self.keys
            .get(constructor_display_name(name).as_ref())
            .map(String::as_str)
    }
}

/// The open collection a `lookup` reads: its field, the type of its
/// entries, and its input path in a partial case.
pub(crate) struct OpenCollection {
    field: String,
    element: String,
    path: Option<String>,
}

impl Interpreter {
    /// `lookup(collection, key)`, with the collection unevaluated: a field
    /// read names the open collection it may be.
    pub(crate) fn eval_lookup(&mut self, collection: &Expr, key: &Expr, env: &Env) -> Value {
        let (list, open) = match &collection.kind {
            ExprKind::Field(base, field) => {
                let receiver = explain::strip(self.eval(base, env));
                if self.calculation_failed() {
                    return Value::Unit;
                }
                if self.partial_mode.is_some() {
                    if let Some(paths) = partial::unknown_paths(&receiver, false) {
                        // A variant whose tag is unknown may lack the field.
                        self.partial_fallible(&paths);
                        return self.partial_unknown_read(paths);
                    }
                }
                match crate::query::named_fields(&receiver) {
                    Some((constructor, fields)) => {
                        let Some(value) = fields
                            .iter()
                            .find(|(name, _)| name == field)
                            .map(|(_, value)| value.clone())
                        else {
                            return self.panic_or_ground_fail(format!(
                                "value has no field or method `{field}`"
                            ));
                        };
                        if self.partial_mode.is_some() {
                            self.partial_field_read(&receiver, field, &value);
                        }
                        let open =
                            self.open_inputs
                                .open_element(constructor, field)
                                .map(|element| OpenCollection {
                                    field: field.clone(),
                                    element: element.to_string(),
                                    path: self
                                        .tracked_path(&receiver)
                                        .map(|path| partial::join_path(&path, field)),
                                });
                        (value, open)
                    }
                    None => (self.eval(collection, env), None),
                }
            }
            _ => (self.eval(collection, env), None),
        };
        if self.calculation_failed() {
            return Value::Unit;
        }
        let key = self.eval(key, env);
        if self.calculation_failed() {
            return Value::Unit;
        }
        self.lookup_entry(list, key, open)
    }

    /// The entry of `list` whose key is `key`: the entry itself in an open
    /// collection, `Some(entry)` or `None` in a closed one.
    pub(crate) fn lookup_entry(
        &mut self,
        list: Value,
        key: Value,
        open: Option<OpenCollection>,
    ) -> Value {
        let list = explain::strip(list);
        let key = explain::strip(key);
        if let Some(unknown) = self.partial_need(&[&key], true) {
            return unknown;
        }
        if let Some(unknown) = self.partial_need(&[&list], false) {
            return unknown;
        }
        let Some(items) = crate::query::list_items(&list) else {
            return self.panic_or_ground_fail(format!(
                "`lookup` finds an entry of a list, got {}",
                self.runtime_value_type_label(&list)
            ));
        };
        let key_field = open
            .as_ref()
            .and_then(|open| self.open_inputs.key(&open.element))
            .or_else(|| {
                items.iter().find_map(|item| {
                    crate::query::named_fields(item)
                        .and_then(|(constructor, _)| self.open_inputs.key(constructor))
                })
            })
            .map(str::to_string);
        let Some(key_field) = key_field else {
            if items.is_empty() {
                return Value::Constructor("None".into(), Vec::new().into());
            }
            return self.panic_or_ground_fail(
                "`lookup` finds an entry by its key field, and the list's entries declare none (`@ field(key = True)`)",
            );
        };
        let Some(wanted) = key_json(&key) else {
            return self.panic_or_ground_fail(format!(
                "`lookup` takes a `String` or `Int` key, got {}",
                self.runtime_value_type_label(&key)
            ));
        };
        let strict = self.partial_mode == Some(partial::PartialMode::Strict);
        // Keys not known yet: such an entry may be the one.
        let mut unknown_keys = BTreeSet::new();
        let mut found = None;
        for item in &items {
            let entry = explain::strip(item.clone());
            if let Value::Unknown(paths) = &entry {
                if strict {
                    return self.partial_unknown_read((**paths).clone());
                }
                unknown_keys.extend(paths.iter().cloned());
                continue;
            }
            let Some(entry_key) = crate::query::named_fields(&entry).and_then(|(_, fields)| {
                fields
                    .iter()
                    .find(|(name, _)| *name == key_field)
                    .map(|(_, value)| explain::strip(value.clone()))
            }) else {
                return self.panic_or_ground_fail(format!(
                    "`lookup`: an entry has no key field `{key_field}`"
                ));
            };
            if self.partial_mode.is_some() {
                self.partial_field_read(&entry, &key_field, &entry_key);
            }
            match &entry_key {
                Value::Unknown(paths) => {
                    if strict {
                        return self.partial_unknown_read((**paths).clone());
                    }
                    unknown_keys.extend(paths.iter().cloned());
                }
                known if values_equal(known, &key) => {
                    found = Some(item.clone());
                    break;
                }
                _ => {}
            }
        }
        if !unknown_keys.is_empty() {
            // In an open collection the entry may be missing once the
            // unknown keys are known: a completion can fail.
            if open.is_some() {
                self.partial_fallible(&unknown_keys);
            }
            return self.partial_unknown_read(unknown_keys);
        }
        match (found, open) {
            (Some(entry), Some(_)) => entry,
            (Some(entry), None) => Value::Constructor("Some".into(), vec![entry].into()),
            (None, None) => Value::Constructor("None".into(), Vec::new().into()),
            (None, Some(open)) => match (open.path, self.partial_mode) {
                (Some(path), Some(_)) => {
                    // The entry is a need of its own; a case that stays
                    // without it fails, so it is fallible too.
                    let paths =
                        BTreeSet::from([format!("{path}{}", key_selector(&key_field, &wanted))]);
                    self.partial_fallible(&paths);
                    self.partial_unknown_read(paths)
                }
                _ => self.panic_or_ground_fail(format!(
                    "`lookup`: `{}` has no entry whose `{key_field}` is {wanted}. It is an open collection (`@ field(open = True)`): the case gives each entry the law looks up, so give this one",
                    open.field
                )),
            },
        }
    }
}
