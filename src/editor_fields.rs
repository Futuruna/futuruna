//! Editor field queries borrow the checker's lexical scopes and nominal
//! schemas. They neither execute a document nor establish program validity.

use crate::*;

/// A statically known field available on the receiver at an editor position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorField {
    pub owner: String,
    pub name: String,
    pub type_name: String,
}

pub(super) struct FieldQuery {
    source: String,
    member_end: usize,
    fields: Vec<EditorField>,
}

impl TypeChecker {
    /// Find record fields at the end of a parsed member expression, measured
    /// in the compiler's character offsets. `statements` excludes the prelude.
    /// The caller may replace an unfinished member name before parsing; errors
    /// elsewhere do not turn these editor hints into a successful type check.
    pub fn editor_fields_at(
        statements: &[Stmt],
        source_dir: Option<String>,
        source: &str,
        member_end: usize,
    ) -> Vec<EditorField> {
        let program = prepend_prelude(parse_prelude(), statements);
        let mut checker = Self::new();
        checker.source_dir = source_dir;
        checker.source_text = source.to_string();
        checker.install_constructor_prepass(&program);
        checker.collect_declarations(&program);
        checker.prepare_rule_dispatch_metadata(&program);
        checker.infer_top_level_binding_types(&program);
        checker.editor_field_query = Some(FieldQuery {
            source: source.to_string(),
            member_end,
            fields: Vec::new(),
        });
        // Check only authored statements here: prelude/import spans belong to
        // other documents and cannot identify this editor position.
        checker.check_stmt_sequence(statements);
        checker.editor_field_query.take().unwrap().fields
    }

    pub(super) fn record_editor_field_query(&mut self, expression: &Expr) {
        let Some(query) = &self.editor_field_query else {
            return;
        };
        let ExprKind::Field(receiver, _) = &expression.kind else {
            return;
        };
        if expression.span.end != query.member_end || self.source_text != query.source {
            return;
        }

        // An untyped inner binder must hide an outer type as well as its name.
        // Reuse the existing inference tombstone instead of guessing its type.
        let mut locals = BTreeMap::new();
        for (names, types) in self.scopes.iter().zip(&self.var_types) {
            for name in names {
                locals.insert(
                    name.clone(),
                    types
                        .get(name)
                        .cloned()
                        .unwrap_or_else(|| CHECKED_UNTYPED_SHADOW_TYPE_TOMBSTONE.to_string()),
                );
            }
        }
        let fields = self
            .editor_receiver_type(receiver, &locals)
            .and_then(|type_name| {
                let owner = Self::canonical_nominal_owner(&type_name)?;
                let names = self.type_fields.get(&owner)?;
                let receiver_type = CanonicalDispatchValue::plain(type_name.clone());
                Some(
                    names
                        .iter()
                        .filter_map(|name| {
                            // Only fields uniform across the receiver's variants are
                            // safe without additional pattern-narrowing evidence.
                            self.canonical_uniform_field_type(&receiver_type, name).map(
                                |field_type| EditorField {
                                    owner: type_name.clone(),
                                    name: name.clone(),
                                    type_name: field_type,
                                },
                            )
                        })
                        .collect(),
                )
            })
            .unwrap_or_default();
        self.editor_field_query.as_mut().unwrap().fields = fields;
    }

    fn editor_receiver_type(
        &self,
        expression: &Expr,
        locals: &BTreeMap<String, String>,
    ) -> Option<String> {
        if let ExprKind::Field(base, field) = &expression.kind {
            let base_type = self.editor_receiver_type(base, locals)?;
            return self
                .canonical_uniform_field_type(&CanonicalDispatchValue::plain(base_type), field);
        }
        self.infer_expr_type_name_with_locals(expression, locals)
    }
}
