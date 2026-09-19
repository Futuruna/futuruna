//! Exhaustiveness evidence from exact checked owners, not constructor spellings
//! or the legacy frontend's more permissive match-coverage approximation.

use super::*;

impl<'program, 'query> CheckedClassificationProducer<'program, 'query> {
    pub(super) fn checked_match_is_exhaustive(
        &self,
        site: &ExprSiteId,
        arms: &[MatchArm],
        allow_bare_fielded_tag: bool,
    ) -> bool {
        if arms
            .iter()
            .any(|arm| arm.guard.is_none() && pattern_is_irrefutable(&arm.pat))
        {
            return true;
        }
        let Some(CheckedExpressionType::Resolved(ty)) = self
            .resolutions
            .expressions
            .get(&child_site(site, 0))
            .map(|resolution| &resolution.resolved_type)
        else {
            return false;
        };
        if self.scalar_kind(ty) == Some(ScalarKind::Boolean) {
            let mut covered = BTreeSet::new();
            for (ordinal, arm) in arms.iter().enumerate() {
                if arm.guard.is_none() {
                    if let Some(value) = self.checked_boolean_pattern(site, ordinal, &arm.pat) {
                        covered.insert(value);
                    }
                }
            }
            return covered.len() == 2;
        }
        let Some(constructors) = closed_match_constructors(&self.index, self.resolutions, ty)
        else {
            return false;
        };
        let mut covered = BTreeSet::new();
        for (ordinal, arm) in arms.iter().enumerate() {
            if arm.guard.is_some() {
                continue;
            }
            let mut pattern = &arm.pat;
            while let Pat::As(inner, _) = pattern {
                pattern = inner;
            }
            let children = match pattern {
                Pat::Con(_, children) => children.iter().collect::<Vec<_>>(),
                Pat::NamedCon(_, fields) => fields.iter().map(|(_, pat)| pat).collect(),
                _ => continue,
            };
            // Refutable fields do not cover a whole constructor, even if every
            // outer tag appears somewhere. Nested alternatives stay residual.
            if !children.iter().all(|pat| pattern_is_irrefutable(pat)) {
                continue;
            }
            let Some(resolution) = self
                .resolutions
                .constructor_patterns
                .get(&checked_pattern_site(site, &[ordinal as u32]))
            else {
                return false;
            };
            let constructor = &resolution.constructor;
            if !constructors.contains(constructor)
                || resolution.source_fields.len() != children.len()
                || matches!(pattern, Pat::NamedCon(_, _))
                    && constructor.layout != CheckedConstructorLayout::Named
                || matches!(pattern, Pat::Con(_, _))
                    && children.len() != constructor.fields.len()
                    && !(children.is_empty() && allow_bare_fielded_tag)
                || resolution
                    .source_fields
                    .iter()
                    .any(|field| constructor.fields.get(field.field_index) != Some(field))
            {
                return false;
            }
            covered.insert(constructor.clone());
        }
        covered == constructors
    }

    fn checked_boolean_pattern(
        &self,
        site: &ExprSiteId,
        ordinal: usize,
        mut pattern: &Pat,
    ) -> Option<bool> {
        while let Pat::As(inner, _) = pattern {
            pattern = inner;
        }
        match pattern {
            Pat::Lit(Literal::Bool(value)) => Some(*value),
            Pat::Con(_, children) if children.is_empty() => {
                let resolution = self
                    .resolutions
                    .constructor_patterns
                    .get(&checked_pattern_site(site, &[ordinal as u32]))?;
                let constructor = &resolution.constructor;
                if !matches!(&constructor.owner, CheckedDataTypeId::Intrinsic { canonical_name }
                    if canonical_name.as_ref() == "Bool")
                    || !constructor.fields.is_empty()
                    || !resolution.source_fields.is_empty()
                {
                    return None;
                }
                match constructor.variant.as_ref() {
                    "False" => Some(false),
                    "True" => Some(true),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

/// A catalogue of observed constructors is not a closed universe: entries
/// can be absent or ambiguous. Reconcile *every* declaration variant with
/// its checked identity. Composed, open and polymorphic schemas are not
/// handled by this first-match normalization.
fn closed_match_constructors(
    index: &CheckedExploreSemanticIndex<'_>,
    resolutions: &CheckedResolutionArtifacts,
    ty: &Ty,
) -> Option<BTreeSet<CheckedConstructorIdentity>> {
    let Ty::Name(name) = ty else {
        return None;
    };
    if index.conditional_type_names.contains(name) {
        return None;
    }
    let owner = resolutions.data_type_identities.get(name.as_str())?;
    let CheckedDataTypeId::Declared(model_owner) = owner else {
        return None;
    };
    let declaration_id = resolutions
        .data_owner_to_analysis_occurrence
        .get(model_owner)?;
    let crate::TypeDecl::ADT {
        name: declared_name,
        params,
        variants,
        except_from,
        ..
    } = index.type_declarations.get(declaration_id).copied()?
    else {
        return None;
    };
    if declared_name != name
        || !params.is_empty()
        || except_from.is_some()
        || variants.is_empty()
        || variants.len() > 256
    {
        return None;
    }
    let mut constructors = BTreeSet::new();
    for (variant_index, variant) in variants.iter().enumerate() {
        match variant.from_type.as_deref() {
            None => {}
            Some("__maybe_include")
                if !crate::checked_explore_maybe_include_resolves_as_type(
                    resolutions,
                    declaration_id,
                    &variant.name,
                ) => {}
            _ => return None,
        }
        let constructor = resolutions
            .constructor_identities
            .get(&(name.as_str().into(), variant.name.as_str().into()))?;
        let layout = if variant.positional || variant.fields.is_empty() {
            CheckedConstructorLayout::Positional
        } else {
            CheckedConstructorLayout::Named
        };
        if &constructor.owner != owner
            || constructor.owner_type.as_ref() != name
            || constructor.variant.as_ref() != variant.name
            || constructor.variant_index != variant_index
            || constructor.layout != layout
            || constructor.fields.len() != variant.fields.len()
            || constructor
                .fields
                .iter()
                .zip(&variant.fields)
                .enumerate()
                .any(|(field_index, (identity, field))| {
                    &identity.owner != owner
                        || identity.variant_index != variant_index
                        || identity.field_index != field_index
                        || identity.name.as_ref() != field.name
                })
            || !constructors.insert(constructor.as_ref().clone())
        {
            return None;
        }
    }
    Some(constructors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::explore::relational_classification_capsule::ClassificationLaneStatus;
    use crate::{Lexer, Parser, TypeCheckArtifacts, TypeChecker};

    fn artifacts(definition: &str) -> TypeCheckArtifacts {
        let source = format!("{definition}\n? explore checked_matches {{\n    from {{\n        vary before in range(0, 300)\n        given context = ()\n    }}\n    transition after = before + 1\n    find cases = matches of q(before) > 0\n}}\n");
        let statements = Parser::new(Lexer::new(&source).tokenize(), &source)
            .parse_program()
            .unwrap();
        TypeChecker::check_with_explore_artifacts(&statements, None, &source)
    }

    fn graph(definition: &str) -> std::sync::Arc<FrozenClassificationProgram> {
        let artifacts = artifacts(definition);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        artifacts
            .checked_exploration_query(0)
            .unwrap()
            .classification_program()
    }

    #[test]
    fn checked_exhaustive_matches_use_checked_adt_boolean_and_binder_identities() {
        for definition in [
            "# Choice = Empty | Present(Int)\n> q(x: Int) -> Int { match Present(x) { | Empty -> 0 | Present(y) -> y } }",
            "# Choice = Empty | Present(Int)\n> q(x: Int) -> Int { match Present(x) { | Present(y) if y > 7 -> y + 1 | Empty -> 0 | Present(y) -> y } }",
            "# Record(value: Int, other: Int)\n> q(x: Int) -> Int { match Record(x, 2) { | Record(value: y) -> y } }",
            "> q(x: Int) -> Int { match x > 7 { | True -> x | False -> 0 } }",
        ] {
            let graph = graph(definition);
            assert!(graph.lane_manifest().iter().all(|lane| lane.status == ClassificationLaneStatus::Lowered), "{definition}: {:?}", graph.lane_manifest());
            assert!(graph.validate_identity());
        }
        let original = graph("# Choice = Empty | Present(Int)\n> q(x: Int) -> Int { match Present(x) { | Empty -> 0 | Present(y) -> y } }");
        let renamed = graph("# Choice = Empty | Present(Int)\n> q(income: Int) -> Int { match Present(income) { | Empty -> 0 | Present(value) -> value } }");
        assert_eq!(original.graph_root(), renamed.graph_root());
    }

    #[test]
    fn checked_exhaustive_matches_do_not_promote_partial_or_refutable_coverage() {
        let mut residuals_checked = 0;
        for definition in [
            "# Choice = Empty | Present(Int)\n> q(x: Int) -> Int { match Present(x) { | Empty -> 0 } }",
            "# Choice = Empty | Present(Int)\n> q(x: Int) -> Int { match Present(x) { | Present(y) if y > 0 -> y | Empty -> 0 } }",
            "# Choice = Empty | Present(Int)\n> q(x: Int) -> Int { match Present(x) { | Empty -> 0 | Present(7) -> 1 } }",
            "# Inner = First | Second\n# Outer = Absent | Wrapped(Inner)\n> q(x: Int) -> Int { match Wrapped(First) { | Absent -> 0 | Wrapped(First) -> 1 } }",
            "> q(x: Int) -> Int { match x > 7 { | True if x > 10 -> 1 | False -> 0 } }",
        ] {
            let artifacts = artifacts(definition);
            // The frontend may already reject a missing arm. If it accepts
            // one, the proof producer must independently retain a residual.
            if !artifacts.diagnostics.is_empty() {
                continue;
            }
            let graph = artifacts.checked_exploration_query(0).unwrap().classification_program();
            residuals_checked += 1;
            assert!(graph.lane_manifest().iter().any(|lane| matches!(lane.lane, ClassificationSemanticLane::Find(_)) && lane.status == ClassificationLaneStatus::Residual), "{definition}");
        }
        assert!(
            residuals_checked >= 2,
            "exercise the proof check independently of frontend rejection"
        );
    }

    #[test]
    fn checked_exhaustive_matches_require_the_entire_exact_constructor_catalogue() {
        let artifacts = artifacts("# Choice = Empty | Present(Int)\n> q(x: Int) -> Int { x }");
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let index = CheckedExploreSemanticIndex::build(&artifacts.analysis_program);
        let ty = Ty::Name("Choice".into());
        let original = &artifacts.checked_resolutions;
        assert_eq!(
            closed_match_constructors(&index, original, &ty)
                .unwrap()
                .len(),
            2
        );
        for mutation in 0..5 {
            let mut resolutions = original.clone();
            let key = ("Choice".into(), "Present".into());
            let mut constructor = resolutions
                .constructor_identities
                .remove(&key)
                .unwrap()
                .as_ref()
                .clone();
            match mutation {
                0 => {} // Missing metadata is not a smaller closed universe.
                1 => {
                    constructor.owner = CheckedDataTypeId::Intrinsic {
                        canonical_name: "Choice".into(),
                    }
                }
                2 => constructor.variant_index = 0,
                3 => constructor.fields[0].field_index = 1,
                4 => constructor.layout = CheckedConstructorLayout::Named,
                _ => unreachable!(),
            }
            if mutation != 0 {
                resolutions
                    .constructor_identities
                    .insert(key, constructor.into());
            }
            assert!(
                closed_match_constructors(&index, &resolutions, &ty).is_none(),
                "mutation {mutation}"
            );
        }
        let mut evolving_index = CheckedExploreSemanticIndex::build(&artifacts.analysis_program);
        evolving_index
            .conditional_type_names
            .insert("Choice".into());
        assert!(closed_match_constructors(&evolving_index, original, &ty).is_none());

        // Type inclusion is not just another nullary variant. Even if the
        // existing catalogue looks complete, composition needs its own proof.
        for declaration in [
            "# Base = A | B\n# Choice = Base | C",
            "# Base = A | B\n# Choice = Base EXCEPT B",
        ] {
            let composed = self::artifacts(&format!("{declaration}\n> q(x: Int) -> Int {{ x }}"));
            assert!(
                composed.diagnostics.is_empty(),
                "{:?}",
                composed.diagnostics
            );
            let index = CheckedExploreSemanticIndex::build(&composed.analysis_program);
            assert!(
                closed_match_constructors(&index, &composed.checked_resolutions, &ty).is_none()
            );
        }
    }
}
