//! `runa cases`: a conformance suite generated from the law
//! (`docs/rfcs/conformance-cases.md`). For every rule clause an entry
//! reaches, find an entry input that fires it, and for every threshold in a
//! clause's guard, inputs just below, at and just above it. A case is a base
//! input (a scenario's, a seed's, or the minimal one) with a few input leaves
//! moved to where a Z3 witness of the rule's arguments says; what a case
//! covers and its expected result come from running it.
use super::*;
use runa_rule_reachability::cases::{
    case_catalogue, Anchor, ArgumentLeaf, CaseCatalogue, ClauseStatus, Goal, InputDomain, Witness,
};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::sync::Arc;

pub(crate) struct CasesOptions {
    pub(crate) entry: Option<String>,
    pub(crate) out: String,
    pub(crate) seeds: Vec<String>,
    pub(crate) scenarios: Vec<String>,
    /// `--rejected`: result variants that are rejections, besides a
    /// `Result`'s `Err`.
    pub(crate) rejected: Vec<String>,
}

/// Probes allowed while searching for one clause or one boundary.
const PROBES_PER_TARGET: usize = 120;
/// Rounds of witness and lift for one base call.
const LIFT_ROUNDS: usize = 3;
/// Calls of the rule tried in each base.
const CALLS_PER_BASE: usize = 2;
/// Passes over the clauses: a case found in one pass may call a rule no
/// earlier case called.
const PASSES: usize = 3;

#[derive(Serialize, Clone)]
struct Cover {
    rule: String,
    clause: String,
    location: String,
    cites: Vec<String>,
}

#[derive(Serialize)]
struct CaseFile {
    id: String,
    entry: String,
    kind: String,
    covers: Vec<Cover>,
    input: JsonValue,
    expected: JsonValue,
}

#[derive(Serialize)]
struct ClauseCoverage {
    rule: String,
    clause: String,
    location: String,
    cites: Vec<String>,
    covered_by: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    witness: Option<JsonValue>,
}

#[derive(Serialize)]
struct ThresholdCoverage {
    rule: String,
    clause: String,
    threshold: usize,
    guard: String,
    below: Option<String>,
    at: Option<String>,
    above: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

#[derive(Serialize)]
struct CaseCounts {
    clause: usize,
    boundary: usize,
    scenario: usize,
    rejected: usize,
}

/// Clauses of one kind (cited or uncited): how many a case covers, of how
/// many reached, and how many of those no result rests on (reported, still
/// counted: the provenance misses some dependence, so coverage understates).
#[derive(Serialize)]
struct ClauseCount {
    covered: usize,
    clauses: usize,
    not_in_result: usize,
}

/// A clause a technical rejection rests on.
#[derive(Serialize, Clone)]
struct ClauseName {
    rule: String,
    clause: String,
    location: String,
}

/// A rejection the search found that rests on no cited clause (a
/// technical bound, an overflow guard): it is not a case.
#[derive(Serialize)]
struct TechnicalRejection {
    /// The id its case would have had.
    id: String,
    /// The kind of case the search was looking for.
    searched_as: String,
    /// The clauses the rejection rests on, none of them cited.
    rests_on: Vec<ClauseName>,
}

#[derive(Serialize)]
struct CoverageFile {
    entry: String,
    /// `cited rule clauses: X of Y covered`.
    summary: String,
    /// The rule clauses with `@ cite`: the law the suite tests.
    cited: ClauseCount,
    /// The rule clauses without `@ cite`, reported apart.
    uncited: ClauseCount,
    /// The result variants counted as rejections.
    rejections: Vec<String>,
    cases: CaseCounts,
    clauses: Vec<ClauseCoverage>,
    thresholds: Vec<ThresholdCoverage>,
    /// Rejections resting only on uncited clauses, which are not cases.
    technical: Vec<TechnicalRejection>,
}

/// A segment of an input path: `events[0].kind.$variant`.
#[derive(Clone, Debug, PartialEq)]
enum Segment {
    Field(String),
    Index(usize),
}

fn parse_path(path: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    for part in path.split('.').filter(|part| !part.is_empty()) {
        let mut rest = part;
        if let Some(open) = rest.find('[') {
            if open > 0 {
                segments.push(Segment::Field(rest[..open].to_string()));
            }
            rest = &rest[open..];
            while let Some(stripped) = rest.strip_prefix('[') {
                let Some(close) = stripped.find(']') else {
                    break;
                };
                if let Ok(index) = stripped[..close].parse() {
                    segments.push(Segment::Index(index));
                }
                rest = &stripped[close + 1..];
            }
        } else {
            segments.push(Segment::Field(rest.to_string()));
        }
    }
    segments
}

fn json_at<'a>(value: &'a JsonValue, path: &[Segment]) -> Option<&'a JsonValue> {
    path.iter()
        .try_fold(value, |current, segment| match segment {
            Segment::Field(name) => current.as_object()?.get(name),
            Segment::Index(index) => current.as_array()?.get(*index),
        })
}

fn set_json_at(value: &mut JsonValue, path: &[Segment], new: JsonValue) -> bool {
    let Some((last, parents)) = path.split_last() else {
        *value = new;
        return true;
    };
    let mut current = value;
    for segment in parents {
        current = match segment {
            Segment::Field(name) => match current.as_object_mut().and_then(|o| o.get_mut(name)) {
                Some(next) => next,
                None => return false,
            },
            Segment::Index(index) => match current.as_array_mut().and_then(|a| a.get_mut(*index)) {
                Some(next) => next,
                None => return false,
            },
        };
    }
    match (last, current) {
        (Segment::Field(name), JsonValue::Object(object)) => {
            object.insert(name.clone(), new);
            true
        }
        (Segment::Index(index), JsonValue::Array(items)) if *index < items.len() => {
            items[*index] = new;
            true
        }
        _ => false,
    }
}

/// The value of a plain-JSON argument (`ProbeArg::value`) at a dotted
/// path.
fn argument_at<'a>(value: &'a JsonValue, path: &str) -> Option<&'a JsonValue> {
    json_at(value, &parse_path(path))
}

fn join(path: &str, part: &str) -> String {
    if path.is_empty() {
        part.to_string()
    } else {
        format!("{}.{}", path, part)
    }
}

/// Where a run's argument and a witness differ: `(path, witness value)`,
/// outermost first; a different variant is one difference.
fn differences(run: &JsonValue, model: &JsonValue, path: &str, out: &mut Vec<(String, JsonValue)>) {
    if run == model {
        return;
    }
    match (run, model) {
        (JsonValue::Object(run_fields), JsonValue::Object(model_fields))
            if run_fields.get("$variant") == model_fields.get("$variant") =>
        {
            for (key, model_value) in model_fields {
                if key == "$variant" {
                    continue;
                }
                match run_fields.get(key) {
                    Some(run_value) => differences(run_value, model_value, &join(path, key), out),
                    None => out.push((join(path, key), model_value.clone())),
                }
            }
        }
        (JsonValue::Number(left), JsonValue::Number(right)) if left.as_i64() == right.as_i64() => {}
        _ => out.push((path.to_string(), model.clone())),
    }
}

/// The input path of the value at argument path `at`, from a leaf under it
/// that is an input leaf unchanged: the leaf's input path without the part
/// below `at`. An `Option`'s payload has the option's own input path; a
/// positional field `i` is `$values[i]` in the input.
fn input_path_of(at: &str, leaf_path: &str, leaf_input: &str) -> Option<Vec<Segment>> {
    let below = if at.is_empty() {
        leaf_path
    } else if leaf_path == at {
        ""
    } else {
        leaf_path.strip_prefix(&format!("{}.", at))?
    };
    let mut input = parse_path(leaf_input);
    for part in below.split('.').filter(|part| !part.is_empty()).rev() {
        if input.last() == Some(&Segment::Field(part.to_string())) {
            input.pop();
        } else if let Ok(position) = part.parse::<usize>() {
            let positional = input.len() >= 2
                && input[input.len() - 1] == Segment::Index(position)
                && input[input.len() - 2] == Segment::Field("$values".to_string());
            if positional {
                input.truncate(input.len() - 2);
            }
            // Otherwise an `Option`'s payload: no segment of its own.
        } else {
            return None;
        }
    }
    Some(input)
}

/// One run of an input, as the search reads it.
struct Run {
    result: Result<JsonValue, String>,
    /// Whether the result is a rejection.
    rejected: bool,
    /// The clauses the run covers: those its result rests on (of a
    /// rejection, with the conditions that chose it).
    covered: Vec<usize>,
    calls: Vec<RunCall>,
    /// The first value of the input or result outside the input domain: the
    /// run is then no case (its `result` is that error) and covers nothing.
    outside: Option<String>,
}

struct RunCall {
    family: usize,
    args: Vec<calculate::ProbeArg>,
}

impl Run {
    fn calls_of(&self, family: usize) -> impl Iterator<Item = &RunCall> {
        self.calls.iter().filter(move |call| call.family == family)
    }

    fn ok(&self) -> bool {
        self.result.is_ok()
    }
}

struct Base {
    input: JsonValue,
    run: Rc<Run>,
}

struct Search<'a> {
    filename: &'a str,
    contract: &'a calculate::CalculationContract,
    prepared: calculate::PreparedEntry,
    catalogue: CaseCatalogue,
    /// Catalogue clause by canonical file and line.
    places: BTreeMap<(PathBuf, usize), usize>,
    /// Catalogue family by scope, name and arity.
    family_names: BTreeMap<(Option<String>, String, usize), usize>,
    covers: Vec<Cover>,
    runs: BTreeMap<String, Rc<Run>>,
    probes: usize,
    bases: Vec<Base>,
    cases: Vec<CaseFile>,
    case_by_input: BTreeMap<String, usize>,
    covered_by: Vec<Vec<String>>,
    /// Result variants that are rejections.
    rejections: Vec<String>,
    /// Clauses that fired in some run the entry rejected, off the path the
    /// rejection rests on.
    fired_when_rejected: BTreeSet<usize>,
    /// Clauses that fired in some run the entry accepted, off the path its
    /// result rests on.
    fired_when_accepted: BTreeSet<usize>,
    /// Clauses some run's result (in the input domain) rests on.
    in_some_result: BTreeSet<usize>,
    /// Clauses some run outside the input domain would have covered, with
    /// the first value outside it.
    fired_outside_domain: BTreeMap<usize, String>,
    /// Set when a run outside the input domain reached the boundary searched.
    reached_outside: Option<String>,
    /// Why a clause has no case: (reason, detail, witness).
    notes: BTreeMap<usize, (String, String, Option<JsonValue>)>,
    /// Rejections found that rest on no cited clause, which are no cases.
    technical: Vec<TechnicalRejection>,
    technical_by_input: BTreeSet<String>,
    /// Clauses some technical rejection rests on.
    technical_clauses: BTreeSet<usize>,
}

fn canonical(path: &str, relative_to: &Path) -> PathBuf {
    std::fs::canonicalize(path)
        .or_else(|_| std::fs::canonicalize(relative_to.join(path)))
        .unwrap_or_else(|_| PathBuf::from(path))
}

/// `path` relative to `base`, with `..` where it leaves it.
fn relative_path(path: &Path, base: &Path) -> String {
    let path_parts: Vec<_> = path.components().collect();
    let base_parts: Vec<_> = base.components().collect();
    let common = path_parts
        .iter()
        .zip(&base_parts)
        .take_while(|(left, right)| left == right)
        .count();
    let mut relative = PathBuf::new();
    for _ in common..base_parts.len() {
        relative.push("..");
    }
    for part in &path_parts[common..] {
        relative.push(part);
    }
    relative.to_string_lossy().into_owned()
}

fn id_part(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c == '/' || c.is_whitespace() {
                '_'
            } else {
                c
            }
        })
        .collect()
}

impl<'a> Search<'a> {
    fn run(&mut self, input: &JsonValue) -> Rc<Run> {
        let key = input.to_string();
        if let Some(run) = self.runs.get(&key) {
            return run.clone();
        }
        self.probes += 1;
        let probe = self.prepared.probe(input, &self.rejections);
        let root = Path::new(self.filename)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let place_of = |place: &Option<(String, usize)>| {
            place
                .as_ref()
                .and_then(|(file, line)| self.places.get(&(canonical(file, &root), *line)))
                .copied()
        };
        let mut fired: Vec<usize> = probe
            .fired
            .iter()
            .filter_map(|clause| place_of(&clause.place))
            .collect();
        fired.sort();
        fired.dedup();
        // A run covers the clauses its result rests on, not every clause it
        // evaluated.
        let mut covered: Vec<usize> = probe
            .rests_on
            .iter()
            .filter_map(|clause| place_of(&probe.fired[*clause].place))
            .collect();
        covered.sort();
        covered.dedup();
        let calls = probe
            .calls
            .into_iter()
            .filter_map(|call| {
                let by_clause = call
                    .clause
                    .and_then(|clause| place_of(&probe.fired[clause].place))
                    .map(|clause| self.catalogue.clauses[clause].family);
                let family = by_clause.or_else(|| {
                    self.family_names
                        .get(&(call.scope.clone(), call.name.clone(), call.arity))
                        .copied()
                })?;
                Some(RunCall {
                    family,
                    args: call.args,
                })
            })
            .collect();
        let rejected = probe.rejected;
        // An input an integrator cannot send, or a result the SDK cannot
        // hold, is no case: what it covers does not count. Its calls stay,
        // for the search to move on from it.
        let outside = self.contract.input_domain_error(input).or_else(|| {
            let result = probe.result.as_ref().ok()?;
            let why = self.contract.output_domain_error(result)?;
            Some(format!("the result: {}", why))
        });
        if let Some(why) = &outside {
            for clause in &covered {
                self.fired_outside_domain
                    .entry(*clause)
                    .or_insert_with(|| why.clone());
            }
            let run = Rc::new(Run {
                result: Err(format!("outside the input domain: {}", why)),
                rejected,
                covered,
                calls,
                outside,
            });
            self.runs.insert(key, run.clone());
            return run;
        }
        if probe.result.is_ok() {
            let off_path = fired.iter().filter(|clause| !covered.contains(clause));
            if rejected {
                self.fired_when_rejected.extend(off_path);
            } else {
                self.fired_when_accepted.extend(off_path);
            }
            self.in_some_result.extend(covered.iter().copied());
        }
        let run = Rc::new(Run {
            result: probe.result,
            rejected,
            covered,
            calls,
            outside: None,
        });
        self.runs.insert(key, run.clone());
        run
    }

    /// Whether a clause cites the law (`@ cite`).
    fn cited(&self, clause: usize) -> bool {
        !self.covers[clause].cites.is_empty()
    }

    /// Whether a clause is reachable but no result rests on it: it fired
    /// in a run the entry accepted, and no case covers it, and no run's
    /// result (in the input domain or outside it) rested on it.
    fn not_in_result(&self, clause: usize) -> bool {
        self.covered_by[clause].is_empty()
            && self.fired_when_accepted.contains(&clause)
            && !self.in_some_result.contains(&clause)
            && !self.fired_outside_domain.contains_key(&clause)
    }

    /// Write `input` as a case unless an equal input already is one;
    /// returns the case's id. A rejected result is kind `rejected`, and is
    /// a case only if a cited clause is on the path it rests on: a
    /// rejection resting only on uncited clauses is technical, listed in
    /// `technical` instead (the error says so).
    fn keep(&mut self, input: &JsonValue, id: String, kind: &str) -> Result<String, String> {
        let key = input.to_string();
        if let Some(index) = self.case_by_input.get(&key) {
            return Ok(self.cases[*index].id.clone());
        }
        let run = self.run(input);
        let expected = run.result.clone()?;
        if run.rejected && !run.covered.iter().any(|clause| self.cited(*clause)) {
            let rests_on: Vec<ClauseName> = run
                .covered
                .iter()
                .map(|clause| {
                    let cover = &self.covers[*clause];
                    ClauseName {
                        rule: cover.rule.clone(),
                        clause: cover.clause.clone(),
                        location: cover.location.clone(),
                    }
                })
                .collect();
            let names = rests_on
                .iter()
                .map(|name| format!("{}.{}", name.rule, name.clause))
                .collect::<Vec<_>>()
                .join(", ");
            let why = if names.is_empty() {
                "technical: the input found is rejected, and no cited clause is on the path the rejection rests on".to_string()
            } else {
                format!(
                    "technical: the input found is rejected only by clauses without `@ cite` ({})",
                    names
                )
            };
            if self.technical_by_input.insert(key) {
                self.technical_clauses.extend(run.covered.iter().copied());
                self.technical.push(TechnicalRejection {
                    id,
                    searched_as: kind.to_string(),
                    rests_on,
                });
                self.bases.push(Base {
                    input: input.clone(),
                    run,
                });
            }
            return Err(why);
        }
        let mut id = id;
        if self.cases.iter().any(|case| case.id == id) {
            let mut suffix = 2;
            while self
                .cases
                .iter()
                .any(|case| case.id == format!("{}-{}", id, suffix))
            {
                suffix += 1;
            }
            id = format!("{}-{}", id, suffix);
        }
        for clause in &run.covered {
            self.covered_by[*clause].push(id.clone());
        }
        self.case_by_input.insert(key, self.cases.len());
        self.cases.push(CaseFile {
            id: id.clone(),
            entry: self.contract.entry.clone(),
            kind: if run.rejected { "rejected" } else { kind }.to_string(),
            covers: run
                .covered
                .iter()
                .map(|clause| self.covers[*clause].clone())
                .collect(),
            input: input.clone(),
            expected,
        });
        self.bases.push(Base {
            input: input.clone(),
            run,
        });
        Ok(id)
    }

    fn add_base(&mut self, input: JsonValue, origin: &str) -> bool {
        let run = self.run(&input);
        if let Err(error) = &run.result {
            eprintln!("runa cases: {} is not used: {}", origin, error);
            return false;
        }
        if !self.bases.iter().any(|base| base.input == input) {
            self.bases.push(Base { input, run });
        }
        true
    }

    fn rule_of(&self, clause: usize) -> String {
        self.catalogue.families[self.catalogue.clauses[clause].family].qualified()
    }

    /// Whether a run of `input` covers `clause`: its result rests on it.
    fn fires(&mut self, input: &JsonValue, clause: usize) -> bool {
        let run = self.run(input);
        run.ok() && run.covered.contains(&clause)
    }

    /// The contract type at an input path, reading sum types' variants from
    /// `input`.
    fn type_at(
        &self,
        input: &JsonValue,
        path: &[Segment],
    ) -> Option<calculate::CalculationTypeRef> {
        use calculate::CalculationTypeRef as T;
        let mut ty = self.contract.input.clone();
        let mut value = Some(input);
        let mut position = 0;
        while position < path.len() {
            if let T::Optional { item } = ty {
                ty = *item;
                continue;
            }
            let segment = &path[position];
            ty = match (&ty, segment) {
                (T::List { item } | T::Set { item }, Segment::Index(_)) => (**item).clone(),
                (T::Named { name, .. }, Segment::Field(field)) => {
                    let definition = self.contract.definition(name)?;
                    let variant = if definition.variants.len() == 1 {
                        &definition.variants[0]
                    } else {
                        let chosen = value?.get("$variant")?.as_str()?;
                        definition
                            .variants
                            .iter()
                            .find(|variant| variant.name == chosen)?
                    };
                    if field == "$values" {
                        let Segment::Index(index) = path.get(position + 1)? else {
                            return None;
                        };
                        value =
                            value.and_then(|value| json_at(value, &path[position..position + 2]));
                        position += 2;
                        ty = variant.fields.get(*index)?.ty.clone();
                        continue;
                    }
                    variant
                        .fields
                        .iter()
                        .find(|candidate| &candidate.name == field)?
                        .ty
                        .clone()
                }
                _ => return None,
            };
            value = value.and_then(|value| json_at(value, std::slice::from_ref(segment)));
            position += 1;
        }
        Some(ty)
    }

    /// The plainest input value of a type.
    fn minimal(&self, ty: &calculate::CalculationTypeRef, depth: usize) -> JsonValue {
        use calculate::CalculationTypeRef as T;
        if depth > 8 {
            return JsonValue::Null;
        }
        match ty {
            T::Primitive { name, .. } => match name.as_str() {
                "Int" => JsonValue::from(0),
                "Float" => JsonValue::from(0.0),
                "Bool" => JsonValue::Bool(false),
                "Char" => JsonValue::from("a"),
                _ => JsonValue::from(""),
            },
            T::List { .. } | T::Set { .. } => JsonValue::Array(Vec::new()),
            T::Map { .. } => JsonValue::Object(JsonMap::new()),
            T::Named { name, .. } => {
                let Some(definition) = self.contract.definition(name) else {
                    return JsonValue::Null;
                };
                let product = definition.variants.len() == 1
                    && definition.variants[0].name == definition.name
                    && !definition.variants[0].positional;
                if product {
                    let variant = &definition.variants[0];
                    let is_date = definition.name == "Date"
                        && variant
                            .fields
                            .iter()
                            .map(|field| field.name.as_str())
                            .eq(["year", "month", "day"]);
                    if is_date {
                        return serde_json::json!({ "year": 2000, "month": 1, "day": 1 });
                    }
                    let mut object = JsonMap::new();
                    for field in &variant.fields {
                        object.insert(field.name.clone(), self.minimal(&field.ty, depth + 1));
                    }
                    return JsonValue::Object(object);
                }
                let Some(variant) = definition
                    .variants
                    .iter()
                    .find(|variant| variant.fields.is_empty())
                    .or(definition.variants.first())
                else {
                    return JsonValue::Null;
                };
                self.variant_value(variant, depth)
            }
            _ => JsonValue::Null,
        }
    }

    fn variant_value(&self, variant: &calculate::CalculationVariant, depth: usize) -> JsonValue {
        let mut object = JsonMap::new();
        object.insert("$variant".into(), JsonValue::from(variant.name.clone()));
        if variant.positional {
            object.insert(
                "$values".into(),
                JsonValue::Array(
                    variant
                        .fields
                        .iter()
                        .map(|field| self.minimal(&field.ty, depth + 1))
                        .collect(),
                ),
            );
        } else {
            for field in &variant.fields {
                object.insert(field.name.clone(), self.minimal(&field.ty, depth + 1));
            }
        }
        JsonValue::Object(object)
    }

    /// A plain-JSON value (`ProbeArg::value`, a witness) as the input
    /// writes a value of `ty`.
    fn to_input(
        &self,
        value: &JsonValue,
        ty: &calculate::CalculationTypeRef,
        depth: usize,
    ) -> JsonValue {
        use calculate::CalculationTypeRef as T;
        if depth > 16 {
            return JsonValue::Null;
        }
        let variant_of = |value: &JsonValue| {
            value
                .get("$variant")
                .and_then(JsonValue::as_str)
                .map(str::to_string)
        };
        match ty {
            T::Optional { item } => match variant_of(value).as_deref() {
                Some("None") => JsonValue::Null,
                Some("Some") => value
                    .get("0")
                    .map(|inner| self.to_input(inner, item, depth + 1))
                    .unwrap_or(JsonValue::Null),
                _ if value.is_null() => JsonValue::Null,
                _ => self.to_input(value, item, depth + 1),
            },
            T::List { item } | T::Set { item } => match value.as_array() {
                Some(items) => JsonValue::Array(
                    items
                        .iter()
                        .map(|element| self.to_input(element, item, depth + 1))
                        .collect(),
                ),
                None => JsonValue::Array(Vec::new()),
            },
            T::Named { name, .. } => {
                let Some(definition) = self.contract.definition(name) else {
                    return value.clone();
                };
                let product = definition.variants.len() == 1
                    && definition.variants[0].name == definition.name
                    && !definition.variants[0].positional;
                let chosen = variant_of(value);
                let variant = if product {
                    &definition.variants[0]
                } else {
                    match definition
                        .variants
                        .iter()
                        .find(|variant| Some(&variant.name) == chosen.as_ref())
                    {
                        Some(variant) => variant,
                        None => return self.minimal(ty, depth),
                    }
                };
                let field_value = |index: usize, field: &calculate::CalculationField| {
                    let key = if variant.positional {
                        index.to_string()
                    } else {
                        field.name.clone()
                    };
                    match value.get(&key) {
                        Some(inner) => self.to_input(inner, &field.ty, depth + 1),
                        None => self.minimal(&field.ty, depth + 1),
                    }
                };
                let mut object = JsonMap::new();
                if !product {
                    object.insert("$variant".into(), JsonValue::from(variant.name.clone()));
                }
                if variant.positional {
                    object.insert(
                        "$values".into(),
                        JsonValue::Array(
                            variant
                                .fields
                                .iter()
                                .enumerate()
                                .map(|(index, field)| field_value(index, field))
                                .collect(),
                        ),
                    );
                } else {
                    for (index, field) in variant.fields.iter().enumerate() {
                        object.insert(field.name.clone(), field_value(index, field));
                    }
                }
                JsonValue::Object(object)
            }
            _ => value.clone(),
        }
    }

    /// Every input path holding `null` whose type is an `Option`.
    fn null_paths(&self, input: &JsonValue) -> Vec<Vec<Segment>> {
        fn walk(value: &JsonValue, path: &mut Vec<Segment>, out: &mut Vec<Vec<Segment>>) {
            match value {
                JsonValue::Null => out.push(path.clone()),
                JsonValue::Object(object) => {
                    for (key, inner) in object {
                        path.push(Segment::Field(key.clone()));
                        walk(inner, path, out);
                        path.pop();
                    }
                }
                JsonValue::Array(items) => {
                    for (index, inner) in items.iter().enumerate() {
                        path.push(Segment::Index(index));
                        walk(inner, path, out);
                        path.pop();
                    }
                }
                _ => {}
            }
        }
        let mut out = Vec::new();
        walk(input, &mut Vec::new(), &mut out);
        out.retain(|path| {
            matches!(
                self.type_at(input, path),
                Some(calculate::CalculationTypeRef::Optional { .. })
            )
        });
        out
    }

    /// The `nth` call of `family` in `run`, else its first.
    fn call<'r>(run: &'r Run, family: usize, nth: usize) -> Option<&'r RunCall> {
        run.calls_of(family)
            .nth(nth)
            .or_else(|| run.calls_of(family).next())
    }

    /// Move `input` towards a witness of the rule's arguments: each
    /// argument leaf the witness changes is set at its input path, or
    /// searched for through an integer input it is computed from, or an
    /// `Option` that is `None` is given at an input `null` of its type.
    /// Stops once `done` holds for a run.
    fn lift(
        &mut self,
        mut input: JsonValue,
        family: usize,
        nth: usize,
        model: &[Option<JsonValue>],
        budget: usize,
        done: &dyn Fn(&mut Self, &JsonValue) -> bool,
    ) -> Option<JsonValue> {
        let parameters: Vec<String> = self.catalogue.families[family]
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect();
        for _ in 0..LIFT_ROUNDS {
            if done(self, &input) {
                return Some(input);
            }
            let run = self.run(&input);
            let Some(call) = Self::call(&run, family, nth) else {
                return None;
            };
            let mut diffs = Vec::new();
            for (position, value) in model.iter().enumerate() {
                let (Some(value), Some(argument)) = (value, call.args.get(position)) else {
                    continue;
                };
                let mut found = Vec::new();
                differences(&argument.value, value, "", &mut found);
                diffs.extend(
                    found
                        .into_iter()
                        .map(|(path, target)| (position, path, target)),
                );
            }
            if diffs.is_empty() {
                return None;
            }
            let mut changed = false;
            for (position, at, target) in diffs {
                if self.probes > budget {
                    return None;
                }
                let run = self.run(&input);
                let Some(call) = Self::call(&run, family, nth) else {
                    return None;
                };
                let argument = &call.args[position];
                // Already there after an earlier move.
                if argument_at(&argument.value, &at) == Some(&target) {
                    continue;
                }
                if let Some(moved) =
                    self.move_leaf(&input, family, nth, position, &at, &target, budget)
                {
                    input = moved;
                    changed = true;
                    continue;
                }
                let current = argument_at(&argument.value, &at);
                let is_none = current
                    .and_then(|value| value.get("$variant"))
                    .and_then(JsonValue::as_str)
                    == Some("None");
                if is_none {
                    if let Some(moved) = self.give_option(
                        &input,
                        family,
                        nth,
                        position,
                        &at,
                        &target,
                        &parameters[position],
                        budget,
                    ) {
                        input = moved;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        done(self, &input).then_some(input)
    }

    /// Set the input behind one argument value: through a leaf at or under
    /// `at` that is an input leaf unchanged, or, for an integer, by
    /// searching an integer input it is computed from.
    #[allow(clippy::too_many_arguments)]
    fn move_leaf(
        &mut self,
        input: &JsonValue,
        family: usize,
        nth: usize,
        position: usize,
        at: &str,
        target: &JsonValue,
        budget: usize,
    ) -> Option<JsonValue> {
        let run = self.run(input);
        let call = Self::call(&run, family, nth)?;
        let leaves = &call.args[position].leaves;
        // An input leaf unchanged at or under `at`.
        for leaf in leaves {
            let Some(leaf_input) = &leaf.input else {
                continue;
            };
            let Some(path) = input_path_of(at, &leaf.path, leaf_input) else {
                continue;
            };
            let ty = self.type_at(input, &path)?;
            let mut moved = input.clone();
            if set_json_at(&mut moved, &path, self.to_input(target, &ty, 0)) && moved != *input {
                return Some(moved);
            }
        }
        // An integer computed from integer inputs.
        let target = target.as_i64()?;
        let leaf = leaves.iter().find(|leaf| leaf.path == at)?;
        let sources: Vec<Vec<Segment>> = leaf
            .inputs
            .iter()
            .map(|path| parse_path(path))
            .filter(|path| json_at(input, path).is_some_and(JsonValue::is_i64))
            .take(3)
            .collect();
        for source in sources {
            if let Some(found) =
                self.solve(input, &source, family, nth, position, at, target, budget)
            {
                return Some(found);
            }
        }
        None
    }

    /// Search the integer input at `source` until the argument leaf equals
    /// `target`: secant steps from the current value, widening a step that
    /// changes nothing (rounding), then bisection of a bracket.
    #[allow(clippy::too_many_arguments)]
    fn solve(
        &mut self,
        input: &JsonValue,
        source: &[Segment],
        family: usize,
        nth: usize,
        position: usize,
        at: &str,
        target: i64,
        budget: usize,
    ) -> Option<JsonValue> {
        let observe = |search: &mut Self, value: i128| -> Option<(JsonValue, i128)> {
            let value = i64::try_from(value).ok()?;
            let mut moved = input.clone();
            set_json_at(&mut moved, source, JsonValue::from(value));
            let run = search.run(&moved);
            let call = Self::call(&run, family, nth)?;
            let leaf = argument_at(&call.args[position].value, at)?.as_i64()?;
            Some((moved, leaf as i128))
        };
        let target = target as i128;
        let mut x0 = json_at(input, source)?.as_i64()? as i128;
        let (_, mut y0) = observe(self, x0)?;
        if y0 == target {
            return None;
        }
        let mut step = target - y0;
        let mut x1 = x0 + step;
        let mut low: Option<(i128, i128)> = None;
        let mut high: Option<(i128, i128)> = None;
        for _ in 0..40 {
            if self.probes > budget {
                return None;
            }
            let Some((moved, y1)) = observe(self, x1) else {
                return None;
            };
            if y1 == target {
                return Some(moved);
            }
            for (x, y) in [(x0, y0), (x1, y1)] {
                if y < target {
                    low = Some((x, y));
                } else {
                    high = Some((x, y));
                }
            }
            if let (Some((xl, _)), Some((xh, _))) = (low, high) {
                if (xl - xh).abs() <= 1 {
                    return None;
                }
            }
            if y1 == y0 {
                step = step.saturating_mul(10);
                x1 = x0 + step;
                continue;
            }
            let next = x1 + (target - y1) * (x1 - x0) / (y1 - y0);
            let next = match (low, high) {
                (Some((xl, _)), Some((xh, _))) if !((next > xl.min(xh)) && (next < xl.max(xh))) => {
                    (xl + xh) / 2
                }
                _ if next == x1 => x1 + if target > y1 { 1 } else { -1 },
                _ => next,
            };
            x0 = x1;
            y0 = y1;
            x1 = next;
        }
        None
    }

    /// Give an `Option` argument that is `None` the witness's value at an
    /// input `null` of an `Option` type: those whose name is the
    /// parameter's (or the argument path's last part) first.
    #[allow(clippy::too_many_arguments)]
    fn give_option(
        &mut self,
        input: &JsonValue,
        family: usize,
        nth: usize,
        position: usize,
        at: &str,
        target: &JsonValue,
        parameter: &str,
        budget: usize,
    ) -> Option<JsonValue> {
        let name = at
            .rsplit('.')
            .next()
            .filter(|part| !part.is_empty())
            .unwrap_or(parameter);
        let mut candidates = self.null_paths(input);
        candidates.sort_by_key(|path| {
            let last = path.iter().rev().find_map(|segment| match segment {
                Segment::Field(field) => Some(field.as_str()),
                Segment::Index(_) => None,
            });
            !(last == Some(name) || last == Some(parameter))
        });
        for path in candidates.into_iter().take(8) {
            if self.probes > budget {
                return None;
            }
            let ty = self.type_at(input, &path)?;
            let mut moved = input.clone();
            if !set_json_at(&mut moved, &path, self.to_input(target, &ty, 0)) {
                continue;
            }
            let run = self.run(&moved);
            let Some(call) = Self::call(&run, family, nth) else {
                continue;
            };
            let now = argument_at(&call.args[position].value, at)
                .and_then(|value| value.get("$variant"))
                .and_then(JsonValue::as_str);
            if run.ok() && now == target.get("$variant").and_then(JsonValue::as_str) {
                return Some(moved);
            }
        }
        None
    }

    /// The bases' calls of a family, at most `CALLS_PER_BASE` per base:
    /// (base, which call of the family).
    fn base_calls(&self, family: usize) -> Vec<(usize, usize)> {
        let mut found = Vec::new();
        for (base, entry) in self.bases.iter().enumerate() {
            let mut seen: Vec<String> = Vec::new();
            for (nth, call) in entry.run.calls_of(family).enumerate() {
                let key = call
                    .args
                    .iter()
                    .map(|arg| arg.value.to_string())
                    .collect::<Vec<_>>()
                    .join("|");
                if seen.contains(&key) {
                    continue;
                }
                seen.push(key);
                found.push((base, nth));
                if seen.len() >= CALLS_PER_BASE {
                    break;
                }
            }
        }
        found
    }

    /// The list elements a call's arguments come from: each list's input
    /// path and the element's index, innermost list first, in the order the
    /// arguments' leaves name them.
    fn element_sources(&self, input: &JsonValue, call: &RunCall) -> Vec<(Vec<Segment>, usize)> {
        let mut found: Vec<(Vec<Segment>, usize)> = Vec::new();
        for argument in &call.args {
            for leaf in &argument.leaves {
                for path in leaf.input.iter().chain(&leaf.inputs) {
                    let path = parse_path(path);
                    for at in (0..path.len()).rev() {
                        let Segment::Index(index) = path[at] else {
                            continue;
                        };
                        let list = path[..at].to_vec();
                        let is_list = matches!(
                            self.type_at(input, &list),
                            Some(calculate::CalculationTypeRef::List { .. })
                        );
                        if is_list && !found.iter().any(|(l, i)| *l == list && *i == index) {
                            found.push((list, index));
                        }
                    }
                }
            }
        }
        found
    }

    /// The contract field at an input path that ends in a field name.
    fn field_at(&self, input: &JsonValue, path: &[Segment]) -> Option<calculate::CalculationField> {
        let (Segment::Field(name), parent) = path.split_last()? else {
            return None;
        };
        let mut ty = self.type_at(input, parent)?;
        while let calculate::CalculationTypeRef::Optional { item } = ty {
            ty = *item;
        }
        let calculate::CalculationTypeRef::Named {
            name: type_name, ..
        } = ty
        else {
            return None;
        };
        let definition = self.contract.definition(&type_name)?;
        let variant = if definition.variants.len() == 1 {
            &definition.variants[0]
        } else {
            let chosen = json_at(input, parent)?.get("$variant")?.as_str()?;
            definition
                .variants
                .iter()
                .find(|variant| variant.name == chosen)?
        };
        variant
            .fields
            .iter()
            .find(|field| &field.name == name)
            .cloned()
    }

    /// When a call's arguments come from a list element (an event, a
    /// position), a new element of that list carrying the witness: a copy of
    /// the element with its own key, appended to the base, moved to the
    /// witness as `lift` moves a base, then every leaf the witness did not
    /// set put back to its plainest value where the case still holds. A
    /// closed list's key is derived from `case_id`; an open list's key only
    /// from the witness, never invented. Kept only when the run decodes, is
    /// not rejected and `done` holds; otherwise why not.
    #[allow(clippy::too_many_arguments)]
    fn add_element(
        &mut self,
        base: usize,
        nth: usize,
        family: usize,
        model: &[Option<JsonValue>],
        case_id: &str,
        budget: usize,
        done: &dyn Fn(&mut Self, &JsonValue) -> bool,
    ) -> Result<JsonValue, String> {
        use calculate::CalculationTypeRef as T;
        let accepted = |search: &mut Self, input: &JsonValue| {
            let run = search.run(input);
            run.ok() && !run.rejected && done(search, input)
        };
        let input = self.bases[base].input.clone();
        let run = self.bases[base].run.clone();
        let Some(call) = Self::call(&run, family, nth) else {
            return Err("no case calls the rule".into());
        };
        let sources = self.element_sources(&input, call);
        if sources.is_empty() {
            return Err("its arguments come from no list element".into());
        }
        let mut why = String::new();
        for (list, index) in sources.into_iter().take(2) {
            if self.probes > budget {
                break;
            }
            let list_text = segments_text(&list);
            let Some(JsonValue::Array(items)) = json_at(&input, &list) else {
                continue;
            };
            let Some(T::List { item }) = self.type_at(&input, &list) else {
                continue;
            };
            let key = match item.as_ref() {
                T::Named { name, .. } => self
                    .contract
                    .definition(name)
                    .and_then(|definition| definition.key.clone()),
                _ => None,
            };
            let open = self.field_at(&input, &list).is_some_and(|field| field.open);
            let mut element = items[index].clone();
            if let Some(key) = &key {
                let key_path = [Segment::Index(index), Segment::Field(key.clone())];
                let existing: Vec<&JsonValue> =
                    items.iter().filter_map(|item| item.get(key)).collect();
                let value = if open {
                    // Only a key the witness gives: one an argument reads.
                    let mut path = list.clone();
                    path.extend(key_path.iter().cloned());
                    let path = segments_text(&path);
                    let given = call
                        .args
                        .iter()
                        .enumerate()
                        .find_map(|(position, argument)| {
                            let leaf = argument
                                .leaves
                                .iter()
                                .find(|leaf| leaf.input.as_deref() == Some(path.as_str()))?;
                            argument_at(model.get(position)?.as_ref()?, &leaf.path).cloned()
                        });
                    let ty = {
                        let mut at = list.clone();
                        at.extend(key_path.iter().cloned());
                        self.type_at(&input, &at)
                    };
                    match (given, ty) {
                        (Some(value), Some(ty)) => {
                            let value = self.to_input(&value, &ty, 0);
                            if existing.contains(&&value) {
                                why = format!("a new entry of the open `{}` needs a key, and the witness gives only one it already has", list_text);
                                continue;
                            }
                            value
                        }
                        _ => {
                            why = format!(
                                "a new entry of the open `{}` needs a key, and the witness gives none",
                                list_text
                            );
                            continue;
                        }
                    }
                } else {
                    if !matches!(element.get(key), Some(JsonValue::String(_))) {
                        why = format!("the key of `{}` is not text", list_text);
                        continue;
                    }
                    let mut id = case_id.to_string();
                    let mut suffix = 2;
                    while existing
                        .iter()
                        .any(|value| value.as_str() == Some(id.as_str()))
                    {
                        id = format!("{}-{}", case_id, suffix);
                        suffix += 1;
                    }
                    JsonValue::from(id)
                };
                set_json_at(&mut element, &key_path[1..], value);
            }
            // The witness values whose input leaf is in the element go on the
            // copy before it runs: a copy equal to the element would give the
            // rule the same arguments, which its memo answers unlogged.
            let copied = element.clone();
            let mut element_path = list.clone();
            element_path.push(Segment::Index(index));
            for (position, argument) in call.args.iter().enumerate() {
                let Some(Some(target)) = model.get(position) else {
                    continue;
                };
                let mut diffs = Vec::new();
                differences(&argument.value, target, "", &mut diffs);
                for (at, value) in diffs {
                    for leaf in &argument.leaves {
                        let Some(path) = leaf
                            .input
                            .as_ref()
                            .and_then(|leaf_input| input_path_of(&at, &leaf.path, leaf_input))
                        else {
                            continue;
                        };
                        let (Some(relative), Some(ty)) = (
                            path.strip_prefix(element_path.as_slice()),
                            self.type_at(&input, &path),
                        ) else {
                            continue;
                        };
                        if set_json_at(&mut element, relative, self.to_input(&value, &ty, 0)) {
                            break;
                        }
                    }
                }
            }
            let mut appended = input.clone();
            let mut grown = items.clone();
            grown.push(element.clone());
            let new_index = items.len();
            set_json_at(&mut appended, &list, JsonValue::Array(grown));
            let mut prefix = list.clone();
            prefix.push(Segment::Index(new_index));
            let run = self.run(&appended);
            if let Err(error) = &run.result {
                why = format!("a new element of `{}` is refused: {}", list_text, error);
                continue;
            }
            let reads_new = |call: &RunCall| {
                call.args.iter().any(|argument| {
                    argument.leaves.iter().any(|leaf| {
                        leaf.input
                            .iter()
                            .chain(&leaf.inputs)
                            .any(|path| parse_path(path).starts_with(&prefix))
                    })
                })
            };
            let Some(new_nth) = run.calls_of(family).position(reads_new) else {
                why = format!(
                    "the rule is not called with a new element of `{}`",
                    list_text
                );
                continue;
            };
            let Some(found) = self.lift(appended, family, new_nth, model, budget, &accepted) else {
                why = format!(
                    "a new element of `{}` moved to the witness does not fire it, or the entry rejects it",
                    list_text
                );
                continue;
            };
            return Ok(self.minimise_element(
                found,
                &prefix,
                &copied,
                key.as_deref(),
                budget,
                &accepted,
            ));
        }
        Err(why)
    }

    /// Put each leaf of the new element at `prefix` that still has the
    /// copied element's value (`copied`), except its key, back to its
    /// plainest value where `done` still holds.
    fn minimise_element(
        &mut self,
        mut input: JsonValue,
        prefix: &[Segment],
        copied: &JsonValue,
        key: Option<&str>,
        budget: usize,
        done: &dyn Fn(&mut Self, &JsonValue) -> bool,
    ) -> JsonValue {
        let Some(element) = json_at(&input, prefix).cloned() else {
            return input;
        };
        let mut leaves = Vec::new();
        element_leaves(&element, &mut Vec::new(), &mut leaves);
        for relative in leaves {
            if self.probes > budget {
                break;
            }
            if key.is_some_and(|key| relative == [Segment::Field(key.to_string())]) {
                continue;
            }
            if json_at(&element, &relative) != json_at(copied, &relative) {
                continue;
            }
            let mut path = prefix.to_vec();
            path.extend(relative);
            let Some(ty) = self.type_at(&input, &path) else {
                continue;
            };
            let plain = self.minimal(&ty, 0);
            if json_at(&input, &path) == Some(&plain) {
                continue;
            }
            let mut moved = input.clone();
            if set_json_at(&mut moved, &path, plain) && done(self, &moved) {
                input = moved;
            }
        }
        input
    }

    /// The anchors of a run's call of a family (each argument's value, and
    /// the leaves no input moves), and what the input domain asks of the
    /// argument leaves that are input leaves unchanged: an integer input a
    /// safe integer, the parts of an input date a day on the calendar.
    fn anchors(
        &self,
        input: &JsonValue,
        call: &RunCall,
    ) -> (Vec<(usize, JsonValue, BTreeSet<String>)>, Vec<InputDomain>) {
        const PARTS: [&str; 3] = ["year", "month", "day"];
        let anchors = call
            .args
            .iter()
            .enumerate()
            .map(|(position, argument)| {
                let mut fixed = BTreeSet::new();
                collect_leaf_paths(&argument.value, "", &mut fixed);
                for leaf in &argument.leaves {
                    if !leaf.inputs.is_empty() {
                        fixed.remove(&leaf.path);
                    }
                }
                (position, argument.value.clone(), fixed)
            })
            .collect();
        let mut domain = Vec::new();
        let mut dates: BTreeMap<String, (Vec<Segment>, [Option<ArgumentLeaf>; 3])> =
            BTreeMap::new();
        for (position, argument) in call.args.iter().enumerate() {
            for leaf in &argument.leaves {
                let Some(leaf_input) = &leaf.input else {
                    continue;
                };
                let path = parse_path(leaf_input);
                // A date is one fact: its leaf is the whole input date.
                if json_at(input, &path).is_some_and(JsonValue::is_object)
                    && self.is_date_at(input, &path)
                {
                    domain.push(InputDomain::Date(PARTS.map(|part| {
                        Ok(ArgumentLeaf {
                            parameter: position,
                            path: join(&leaf.path, part),
                        })
                    })));
                    continue;
                }
                if !json_at(input, &path).is_some_and(JsonValue::is_i64) {
                    continue;
                }
                let argument_leaf = ArgumentLeaf {
                    parameter: position,
                    path: leaf.path.clone(),
                };
                let date_part = match path.split_last() {
                    Some((Segment::Field(name), parent)) => PARTS
                        .iter()
                        .position(|part| part == name)
                        .filter(|_| self.is_date_at(input, parent)),
                    _ => None,
                };
                match date_part {
                    Some(part) => {
                        let parent = path[..path.len() - 1].to_vec();
                        dates
                            .entry(segments_text(&parent))
                            .or_insert_with(|| (parent, [None, None, None]))
                            .1[part] = Some(argument_leaf);
                    }
                    None => domain.push(InputDomain::Int(argument_leaf)),
                }
            }
        }
        // A part no argument leaf is keeps the input's value.
        for (parent, parts) in dates.into_values() {
            let mut resolved = Vec::new();
            for (part, leaf) in PARTS.iter().zip(parts) {
                resolved.push(match leaf {
                    Some(leaf) => Ok(leaf),
                    None => {
                        let mut at = parent.clone();
                        at.push(Segment::Field(part.to_string()));
                        match json_at(input, &at).and_then(JsonValue::as_i64) {
                            Some(value) => Err(value),
                            None => break,
                        }
                    }
                });
            }
            if let Ok(parts) = <[_; 3]>::try_from(resolved) {
                domain.push(InputDomain::Date(parts));
            }
        }
        (anchors, domain)
    }

    /// Whether the input value at `path` is a `Date` of the SDK.
    fn is_date_at(&self, input: &JsonValue, path: &[Segment]) -> bool {
        let mut ty = self.type_at(input, path);
        while let Some(calculate::CalculationTypeRef::Optional { item }) = ty {
            ty = Some(*item);
        }
        ty.is_some_and(|ty| calculate::input_domain::is_date_type(self.contract, &ty))
    }

    /// Find an input that fires `clause`.
    fn find_clause(&mut self, clause: usize) -> Result<Option<JsonValue>, String> {
        let family = self.catalogue.clauses[clause].family;
        let budget = self.probes + PROBES_PER_TARGET;
        let analysable = matches!(
            self.catalogue.clauses[clause].status,
            ClauseStatus::Analysable | ClauseStatus::Unknown(_)
        );
        let calls = self.base_calls(family);
        if calls.is_empty() {
            self.notes.insert(
                clause,
                (
                    "no-entry-input".into(),
                    format!(
                        "no case calls `{}`",
                        self.catalogue.families[family].qualified()
                    ),
                    None,
                ),
            );
            return Ok(None);
        }
        let target = move |search: &mut Self, input: &JsonValue| search.fires(input, clause);
        let mut models = Vec::new();
        for (base, nth) in calls {
            if self.probes > budget {
                break;
            }
            let input = self.bases[base].input.clone();
            let run = self.bases[base].run.clone();
            let Some(call) = Self::call(&run, family, nth) else {
                continue;
            };
            if analysable {
                let (anchors, domain) = self.anchors(&input, call);
                let anchors: Vec<Anchor> = anchors
                    .iter()
                    .map(|(parameter, value, fixed)| Anchor {
                        parameter: *parameter,
                        value,
                        fixed,
                    })
                    .collect();
                match self
                    .catalogue
                    .witness(clause, Goal::Fires, &anchors, &domain)?
                {
                    Witness::Model(model) => {
                        let witness = self.witness_json(family, &model);
                        if let Some(found) =
                            self.lift(input.clone(), family, nth, &model, budget, &target)
                        {
                            return Ok(Some(found));
                        }
                        models.push((base, nth, model));
                        self.notes.insert(
                            clause,
                            (
                                "no-entry-input".into(),
                                "Z3 found arguments that fire the clause, but no input moved to them reached the rule with them".into(),
                                Some(witness),
                            ),
                        );
                    }
                    Witness::OutsideDomain(model) => {
                        let witness = self.witness_json(family, &model);
                        let found_arguments = self.notes.get(&clause).is_some_and(
                            |(reason, _, witness)| reason == "no-entry-input" && witness.is_some(),
                        );
                        if !found_arguments {
                            self.notes.insert(
                                clause,
                                (
                                    "outside-input-domain".into(),
                                    "Z3 found arguments that fire the clause only with an input outside the input domain (an Int beyond ±(2^53 − 1), or a day not on the calendar)".into(),
                                    Some(witness),
                                ),
                            );
                        }
                    }
                    Witness::Unsat => {
                        self.notes.entry(clause).or_insert((
                            "no-entry-input".into(),
                            "Z3 found no arguments that fire the clause".into(),
                            None,
                        ));
                    }
                    Witness::Unknown(reason) => {
                        self.notes
                            .insert(clause, ("solver-unknown".into(), reason, None));
                    }
                }
            }
            if let Some(found) = self.neighbourhood(&input, &run, family, nth, budget, &target) {
                return Ok(Some(found));
            }
        }
        // A new element of the list the arguments come from.
        let budget = self.probes + PROBES_PER_TARGET;
        let id = self.clause_case_id(clause);
        for (base, nth, model) in models.into_iter().take(CALLS_PER_BASE) {
            match self.add_element(base, nth, family, &model, &id, budget, &target) {
                Ok(found) => return Ok(Some(found)),
                Err(why) => {
                    if let Some(note) = self.notes.get_mut(&clause) {
                        if !note.1.contains(&why) {
                            note.1 = format!("{}; {}", note.1, why);
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    /// The id of the case found for `clause`.
    fn clause_case_id(&self, clause: usize) -> String {
        format!(
            "{}.{}.clause",
            id_part(&self.rule_of(clause)),
            id_part(&self.catalogue.clauses[clause].key)
        )
    }

    /// The id of the case found for a side of a threshold.
    fn boundary_case_id(&self, clause: usize, threshold: usize, side: &str) -> String {
        format!(
            "{}.{}.t{}.boundary-{}",
            id_part(&self.rule_of(clause)),
            id_part(&self.catalogue.clauses[clause].key),
            threshold + 1,
            side
        )
    }

    fn witness_json(&self, family: usize, model: &[Option<JsonValue>]) -> JsonValue {
        let mut object = JsonMap::new();
        for (parameter, value) in self.catalogue.families[family].parameters.iter().zip(model) {
            if let Some(value) = value {
                object.insert(parameter.name.clone(), value.clone());
            }
        }
        JsonValue::Object(object)
    }

    /// Without a witness: around the inputs the rule's arguments come
    /// from, flip each Bool, switch each variant, set each `Option` to
    /// `null`, and give each input `null` of an `Option` type its plainest
    /// value.
    fn neighbourhood(
        &mut self,
        input: &JsonValue,
        run: &Run,
        family: usize,
        nth: usize,
        budget: usize,
        target: &dyn Fn(&mut Self, &JsonValue) -> bool,
    ) -> Option<JsonValue> {
        let call = Self::call(run, family, nth)?;
        let mut sources: Vec<String> = Vec::new();
        for argument in &call.args {
            for leaf in &argument.leaves {
                for path in leaf.inputs.iter().chain(leaf.input.iter()) {
                    if !sources.contains(path) {
                        sources.push(path.clone());
                    }
                }
            }
        }
        let mut mutations: Vec<JsonValue> = Vec::new();
        for source in &sources {
            let path = parse_path(source);
            if let Some((Segment::Field(last), parent)) = path.split_last() {
                if last == "$variant" {
                    if let Some(calculate::CalculationTypeRef::Named { name, .. }) =
                        self.type_at(input, parent)
                    {
                        if let Some(definition) = self.contract.definition(&name) {
                            for variant in &definition.variants {
                                let mut moved = input.clone();
                                if set_json_at(&mut moved, parent, self.variant_value(variant, 0))
                                    && moved != *input
                                {
                                    mutations.push(moved);
                                }
                            }
                        }
                    }
                    continue;
                }
            }
            if let Some(JsonValue::Bool(value)) = json_at(input, &path) {
                let mut moved = input.clone();
                set_json_at(&mut moved, &path, JsonValue::Bool(!value));
                mutations.push(moved);
            }
            // The innermost `Option` around the source, set to `null`.
            for end in (1..=path.len()).rev() {
                if let Some(calculate::CalculationTypeRef::Optional { .. }) =
                    self.type_at(input, &path[..end])
                {
                    let mut moved = input.clone();
                    if set_json_at(&mut moved, &path[..end], JsonValue::Null) && moved != *input {
                        mutations.push(moved);
                    }
                    break;
                }
            }
        }
        for path in self.null_paths(input) {
            if let Some(calculate::CalculationTypeRef::Optional { item }) =
                self.type_at(input, &path)
            {
                let mut moved = input.clone();
                if set_json_at(&mut moved, &path, self.minimal(&item, 0)) {
                    mutations.push(moved);
                }
            }
        }
        for moved in mutations {
            if self.probes > budget {
                return None;
            }
            if target(self, &moved) {
                return Some(moved);
            }
        }
        None
    }

    /// Find an input whose call of the clause's rule has the threshold's
    /// `lhs - rhs` at `offset`; `case_id` is the id its case will have.
    fn find_boundary(
        &mut self,
        clause: usize,
        threshold: usize,
        offset: i64,
        case_id: &str,
    ) -> Result<Result<JsonValue, String>, String> {
        let family = self.catalogue.clauses[clause].family;
        let budget = self.probes + PROBES_PER_TARGET;
        self.reached_outside = None;
        let reads = self.catalogue.clauses[clause].thresholds[threshold]
            .reads
            .clone();
        let calls = self.base_calls(family);
        if calls.is_empty() {
            return Ok(Err(format!(
                "no case calls `{}`",
                self.catalogue.families[family].qualified()
            )));
        }
        let mut last = String::from("no input moved to Z3's arguments reached the rule with them");
        let mut models = Vec::new();
        // Set when some base's arguments lie only outside the input domain.
        let mut outside = None;
        for (base, nth) in calls {
            if self.probes > budget {
                break;
            }
            let input = self.bases[base].input.clone();
            let run = self.bases[base].run.clone();
            let Some(call) = Self::call(&run, family, nth) else {
                continue;
            };
            let (anchors, domain) = self.anchors(&input, call);
            let anchors: Vec<Anchor> = anchors
                .iter()
                .map(|(parameter, value, fixed)| Anchor {
                    parameter: *parameter,
                    value,
                    fixed,
                })
                .collect();
            let model = match self.catalogue.witness(
                clause,
                Goal::Threshold {
                    index: threshold,
                    offset,
                },
                &anchors,
                &domain,
            )? {
                Witness::Model(model) => model,
                Witness::OutsideDomain(model) => {
                    outside = Some(format!(
                        "outside the input domain: Z3's arguments at this offset are {}",
                        self.witness_json(family, &model)
                    ));
                    continue;
                }
                Witness::Unsat => {
                    last = "Z3 found no arguments at this offset where the rule reaches the clause"
                        .into();
                    continue;
                }
                Witness::Unknown(reason) => {
                    last = reason;
                    continue;
                }
            };
            let at_offset = Self::at_offset(family, reads.clone(), model.clone());
            if let Some(found) = self.lift(input, family, nth, &model, budget, &at_offset) {
                return Ok(Ok(found));
            }
            models.push((base, nth, model));
        }
        if models.is_empty() {
            if let Some(outside) = outside {
                return Ok(Err(outside));
            }
        }
        // A new element of the list the arguments come from.
        let budget = self.probes + PROBES_PER_TARGET;
        for (base, nth, model) in models.into_iter().take(CALLS_PER_BASE) {
            let at_offset = Self::at_offset(family, reads.clone(), model.clone());
            match self.add_element(base, nth, family, &model, case_id, budget, &at_offset) {
                Ok(found) => return Ok(Ok(found)),
                Err(why) if !last.contains(&why) => last = format!("{}; {}", last, why),
                Err(_) => {}
            }
        }
        Ok(Err(match self.reached_outside.take() {
            Some(why) => format!(
                "outside the input domain: the search reached this offset only with an input outside it ({})",
                why
            ),
            None => last,
        }))
    }

    /// Whether some call of `family` in a run has the model's values in
    /// every argument leaf a threshold reads, and the result shows what the
    /// family decided: it is not rejected, or the rejection rests on one of
    /// the family's clauses. (A Boolean family's `false` fall-through is no
    /// clause and leaves no provenance, so an accepted result is taken as
    /// showing it; the guards of `not-in-result` clauses get no boundaries.)
    fn at_offset(
        family: usize,
        reads: Vec<(usize, String)>,
        model: Vec<Option<JsonValue>>,
    ) -> impl Fn(&mut Self, &JsonValue) -> bool {
        move |search: &mut Self, input: &JsonValue| -> bool {
            let run = search.run(input);
            let observable = !run.rejected
                || run
                    .covered
                    .iter()
                    .any(|clause| search.catalogue.clauses[*clause].family == family);
            let reached = observable
                && run.calls_of(family).any(|call| {
                    reads
                        .iter()
                        .all(|(parameter, path)| match &model.get(*parameter) {
                            Some(Some(value)) => {
                                argument_at(&call.args[*parameter].value, path)
                                    == argument_at(value, path)
                            }
                            _ => true,
                        })
                });
            if reached {
                if let Some(why) = &run.outside {
                    search.reached_outside.get_or_insert_with(|| why.clone());
                }
            }
            run.ok() && reached
        }
    }
}

/// An input path as explanations write it: `events[2].kind`.
fn segments_text(path: &[Segment]) -> String {
    let mut text = String::new();
    for segment in path {
        match segment {
            Segment::Field(name) => {
                if !text.is_empty() {
                    text.push('.');
                }
                text.push_str(name);
            }
            Segment::Index(index) => text.push_str(&format!("[{}]", index)),
        }
    }
    text
}

/// The leaves of an input element a new element may set back to its
/// plainest value: scalars, lists (whole) and dates (whole), not variant
/// tags.
fn element_leaves(value: &JsonValue, path: &mut Vec<Segment>, out: &mut Vec<Vec<Segment>>) {
    match value {
        JsonValue::Object(object) => {
            let is_date = object.len() == 3
                && ["year", "month", "day"]
                    .iter()
                    .all(|field| object.contains_key(*field));
            if is_date {
                out.push(path.clone());
                return;
            }
            for (key, inner) in object {
                if key == "$variant" {
                    continue;
                }
                path.push(Segment::Field(key.clone()));
                element_leaves(inner, path, out);
                path.pop();
            }
        }
        JsonValue::Null => {}
        _ => out.push(path.clone()),
    }
}

fn collect_leaf_paths(value: &JsonValue, path: &str, out: &mut BTreeSet<String>) {
    match value {
        JsonValue::Object(object) => {
            for (key, inner) in object {
                if key == "$variant" {
                    out.insert(join(path, key));
                } else {
                    collect_leaf_paths(inner, &join(path, key), out);
                }
            }
        }
        JsonValue::Array(_) | JsonValue::Null => {}
        _ => {
            out.insert(path.to_string());
        }
    }
}

/// The arguments of every call of `entry` a scenario makes, as inputs.
fn scenario_inputs(
    path: &str,
    entry: &str,
    contract: &calculate::CalculationContract,
    use_prelude: bool,
) -> Vec<JsonValue> {
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("runa cases: cannot read {}: {}", path, error);
            return Vec::new();
        }
    };
    let mut parser = Parser::new(Lexer::new(&source).tokenize(), &source);
    let user_stmts = match parser.parse_program() {
        Ok(statements) => statements,
        Err(error) => {
            display_error_in(&source, &error, path);
            return Vec::new();
        }
    };
    let stmts = if use_prelude {
        prepend_prelude(parse_prelude(), &user_stmts)
    } else {
        user_stmts
    };
    let artifacts = type_check_artifacts(&stmts, &source, path);
    if print_type_check_diagnostics(&artifacts.diagnostics, &source, path) {
        return Vec::new();
    }
    let stmts = artifacts.program().clone();
    let mut interpreter = Interpreter::for_checked(&artifacts);
    interpreter.source_dir = source_dir_for(path);
    interpreter.suppress_output = true;
    let mut env = interpreter.default_env();
    interpreter.begin_call_capture(entry);
    if let Err(diagnostic) =
        interpreter.run_program_with_diagnostics(&stmts, &mut env, Path::new(path), &source)
    {
        eprintln!(
            "runa cases: scenario {} failed; the calls it made before are used:\n{}",
            path,
            diagnostic.display(&source, path, false)
        );
    }
    let calls = interpreter.finish_call_capture();
    let mut inputs = Vec::new();
    for arguments in calls {
        let [argument] = arguments.as_slice() else {
            continue;
        };
        match contract.encode_partial_input(argument) {
            Ok(input) if !inputs.contains(&input) => inputs.push(input),
            Ok(_) => {}
            Err(error) => eprintln!("runa cases: a call in {} is not used: {}", path, error),
        }
    }
    inputs
}

/// The ready cases of a `runa call` input document.
fn seed_inputs(path: &str) -> Vec<(String, JsonValue)> {
    let document = std::fs::read_to_string(path)
        .map_err(|error| error.to_string())
        .and_then(|text| {
            serde_json::from_str::<JsonValue>(&text).map_err(|error| error.to_string())
        });
    let document = match document {
        Ok(document) => document,
        Err(error) => {
            eprintln!("runa cases: cannot read seed {}: {}", path, error);
            return Vec::new();
        }
    };
    document["cases"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|case| {
            case["input_status"]
                .as_str()
                .is_none_or(|status| status == "ready")
        })
        .filter_map(|case| Some((case["case_id"].as_str()?.to_string(), case["input"].clone())))
        .collect()
}

pub(crate) fn run_cases(filename: &str, use_prelude: bool, options: &CasesOptions) {
    let source = std::fs::read_to_string(filename).unwrap_or_else(|error| {
        eprintln!("error: cannot read {}: {}", filename, error);
        std::process::exit(1);
    });
    let mut parser = Parser::new(Lexer::new(&source).tokenize(), &source);
    let user_stmts = parser.parse_program().unwrap_or_else(|error| {
        display_error_in(&source, &error, filename);
        std::process::exit(1);
    });
    let stmts = if use_prelude {
        prepend_prelude(parse_prelude(), &user_stmts)
    } else {
        user_stmts
    };
    let artifacts = type_check_artifacts(&stmts, &source, filename);
    if print_type_check_diagnostics(&artifacts.diagnostics, &source, filename) {
        std::process::exit(1);
    }
    let program: Arc<[Stmt]> = artifacts.program().clone();
    let contract =
        select_calculation_contract(&artifacts.calculation_contracts, options.entry.as_deref())
            .unwrap_or_else(|error| {
                eprintln!("error: {}", error);
                std::process::exit(1);
            })
            .clone();
    let rejections = rejection_variants(&contract, &options.rejected).unwrap_or_else(|error| {
        eprintln!("error: {}", error);
        std::process::exit(1);
    });
    let interpreter =
        initialized_calculation_interpreter(&program, &artifacts, filename, &contract.entry);
    let reached = reached_rule_families(&interpreter.calculation_callable_reachability());
    drop(interpreter);
    let catalogue = case_catalogue(&program, filename, &source, &reached).unwrap_or_else(|error| {
        eprintln!("error: runa cases cannot analyse the rules: {}", error);
        std::process::exit(1);
    });

    // Scenario arguments are encoded while the scenario's program is current.
    let scenario_inputs: Vec<(String, Vec<JsonValue>)> = options
        .scenarios
        .iter()
        .map(|path| {
            (
                path.clone(),
                scenario_inputs(path, &contract.entry, &contract, use_prelude),
            )
        })
        .collect();
    if let Some(elaboration) = &artifacts.constructor_elaboration {
        elaboration.install();
    }

    let root_file = canonical(filename, Path::new("."));
    let root = root_file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let mut prepared = calculate::PreparedEntry::new(
        contract.clone(),
        program.clone(),
        source_dir_for(filename),
        artifacts.constructor_elaboration.clone(),
    )
    .with_root_file(root_file.clone());
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut places = BTreeMap::new();
    let mut covers = Vec::new();
    for (index, clause) in catalogue.clauses.iter().enumerate() {
        let file = canonical(&clause.file, &cwd);
        places.insert((file.clone(), clause.line), index);
        let family = &catalogue.families[clause.family];
        covers.push(Cover {
            rule: family.qualified(),
            clause: clause.key.clone(),
            location: format!("{}:{}", relative_path(&file, &root), clause.line),
            cites: prepared.clause_cites(
                &file.to_string_lossy(),
                clause.line,
                family.scope.as_deref(),
                &family.name,
            ),
        });
    }
    let family_names = catalogue
        .families
        .iter()
        .enumerate()
        .map(|(index, family)| {
            (
                (family.scope.clone(), family.name.clone(), family.arity),
                index,
            )
        })
        .collect();
    let clause_count = catalogue.clauses.len();
    let mut search = Search {
        filename,
        contract: &contract,
        prepared,
        catalogue,
        places,
        family_names,
        covers,
        runs: BTreeMap::new(),
        probes: 0,
        bases: Vec::new(),
        cases: Vec::new(),
        case_by_input: BTreeMap::new(),
        covered_by: vec![Vec::new(); clause_count],
        rejections,
        fired_when_rejected: BTreeSet::new(),
        fired_when_accepted: BTreeSet::new(),
        in_some_result: BTreeSet::new(),
        fired_outside_domain: BTreeMap::new(),
        reached_outside: None,
        notes: BTreeMap::new(),
        technical: Vec::new(),
        technical_by_input: BTreeSet::new(),
        technical_clauses: BTreeSet::new(),
    };

    // Bases: scenario inputs (each also a case), seeds, the minimal input.
    for (path, inputs) in &scenario_inputs {
        let stem = Path::new(path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let stem = stem
            .trim_end_matches(".runa")
            .trim_end_matches(".scenario")
            .to_string();
        for (position, input) in inputs.iter().enumerate() {
            let origin = format!("call {} of scenario {}", position + 1, path);
            if search.run(input).ok() {
                let _ = search.keep(
                    input,
                    format!("scenario.{}.{}", id_part(&stem), position + 1),
                    "scenario",
                );
            } else {
                search.add_base(input.clone(), &origin);
            }
        }
    }
    let mut seeds: Vec<(String, JsonValue)> = options
        .seeds
        .iter()
        .flat_map(|path| seed_inputs(path))
        .collect();
    seeds.push(("minimal".to_string(), search.minimal(&contract.input, 0)));
    for (case_id, input) in seeds {
        if !search.add_base(input.clone(), &format!("seed `{}`", case_id)) {
            continue;
        }
        let run = search.run(&input);
        if run
            .covered
            .iter()
            .any(|clause| search.covered_by[*clause].is_empty())
        {
            let _ = search.keep(&input, format!("seed.{}", id_part(&case_id)), "clause");
        }
    }

    // Clauses no case covers yet.
    for _ in 0..PASSES {
        let mut found_any = false;
        for clause in 0..clause_count {
            // A clause only technical rejections reach is not searched again.
            if !search.covered_by[clause].is_empty()
                || search.technical_clauses.contains(&clause)
                || matches!(
                    search.catalogue.clauses[clause].status,
                    ClauseStatus::Unreachable(_)
                )
            {
                continue;
            }
            let found = search.find_clause(clause).unwrap_or_else(|error| {
                eprintln!("error: {}", error);
                std::process::exit(1);
            });
            if let Some(input) = found {
                let id = search.clause_case_id(clause);
                if search.keep(&input, id, "clause").is_ok() {
                    found_any = true;
                }
            }
        }
        if !found_any {
            break;
        }
    }

    // Thresholds of the guards of cited clauses: below, at and above.
    let mut thresholds = Vec::new();
    for clause in 0..clause_count {
        if !search.cited(clause)
            || search.not_in_result(clause)
            || matches!(
                search.catalogue.clauses[clause].status,
                ClauseStatus::Unreachable(_)
            )
        {
            continue;
        }
        for threshold in 0..search.catalogue.clauses[clause].thresholds.len() {
            let mut sides: [Option<String>; 3] = [None, None, None];
            let mut details = Vec::new();
            for (side, (offset, name)) in [(-1, "below"), (0, "at"), (1, "above")]
                .into_iter()
                .enumerate()
            {
                let id = search.boundary_case_id(clause, threshold, name);
                let found = search
                    .find_boundary(clause, threshold, offset, &id)
                    .unwrap_or_else(|error| {
                        eprintln!("error: {}", error);
                        std::process::exit(1);
                    });
                match found {
                    Ok(input) => match search.keep(&input, id, &format!("boundary-{}", name)) {
                        Ok(id) => sides[side] = Some(id),
                        Err(reason) => details.push(format!("{}: {}", name, reason)),
                    },
                    Err(reason) => details.push(format!("{}: {}", name, reason)),
                }
            }
            let [below, at, above] = sides;
            thresholds.push(ThresholdCoverage {
                rule: search.rule_of(clause),
                clause: search.catalogue.clauses[clause].key.clone(),
                threshold: threshold + 1,
                guard: search.catalogue.clauses[clause].thresholds[threshold]
                    .text
                    .clone(),
                below,
                at,
                above,
                detail: (!details.is_empty()).then(|| details.join("; ")),
            });
        }
    }

    // Coverage.
    let mut clauses = Vec::new();
    let mut cited = ClauseCount {
        covered: 0,
        clauses: 0,
        not_in_result: 0,
    };
    let mut uncited = ClauseCount {
        covered: 0,
        clauses: 0,
        not_in_result: 0,
    };
    for clause in 0..clause_count {
        let cover = search.covers[clause].clone();
        let covered_by = search.covered_by[clause].clone();
        let count = if search.cited(clause) {
            &mut cited
        } else {
            &mut uncited
        };
        let not_in_result = search.not_in_result(clause);
        count.clauses += 1;
        if not_in_result {
            count.not_in_result += 1;
        }
        let (reason, detail, witness) = if !covered_by.is_empty() {
            count.covered += 1;
            (None, None, None)
        } else if search.technical_clauses.contains(&clause) {
            (
                Some("technical".to_string()),
                Some(
                    "only rejections that rest on no cited clause reach it; technical checks are not in the suite (see `technical`)"
                        .to_string(),
                ),
                None,
            )
        } else if let Some(why) = search.fired_outside_domain.get(&clause) {
            (
                Some("outside-input-domain".to_string()),
                Some(format!(
                    "it fired only in inputs outside the input domain, which no integrator can send ({})",
                    why
                )),
                search
                    .notes
                    .get(&clause)
                    .and_then(|(_, _, witness)| witness.clone()),
            )
        } else if not_in_result {
            (
                Some("not-in-result".to_string()),
                Some(
                    "it fired, but no run's result rests on it by provenance: the entry evaluates it and gives nothing computed from it, or decides on it in a builtin provenance does not see (still counted)"
                        .to_string(),
                ),
                None,
            )
        } else if search.fired_when_rejected.contains(&clause) {
            (
                Some("rejected".to_string()),
                Some(
                    "it fired only in inputs the entry rejects, and no rejection rests on it"
                        .to_string(),
                ),
                search
                    .notes
                    .get(&clause)
                    .and_then(|(_, _, witness)| witness.clone()),
            )
        } else {
            match &search.catalogue.clauses[clause].status {
                ClauseStatus::Unreachable(detail) => {
                    (Some("unreachable".to_string()), Some(detail.clone()), None)
                }
                ClauseStatus::NotAnalysable(detail) => (
                    Some("not-analysable".to_string()),
                    Some(detail.clone()),
                    None,
                ),
                ClauseStatus::Unknown(detail) => (
                    Some("solver-unknown".to_string()),
                    Some(detail.clone()),
                    None,
                ),
                ClauseStatus::Analysable => match search.notes.get(&clause) {
                    Some((reason, detail, witness)) => {
                        (Some(reason.clone()), Some(detail.clone()), witness.clone())
                    }
                    None => (Some("no-entry-input".to_string()), None, None),
                },
            }
        };
        clauses.push(ClauseCoverage {
            rule: cover.rule,
            clause: cover.clause,
            location: cover.location,
            cites: cover.cites,
            covered_by,
            reason,
            detail,
            witness,
        });
    }
    let summary = format!(
        "cited rule clauses: {} of {} covered, of which {} not in the result (reported, still counted)",
        cited.covered, cited.clauses, cited.not_in_result
    );

    // Write the suite.
    let out = Path::new(&options.out);
    if let Err(error) = std::fs::create_dir_all(out) {
        eprintln!("error: cannot create {}: {}", options.out, error);
        std::process::exit(1);
    }
    let written: BTreeSet<String> = search
        .cases
        .iter()
        .map(|case| format!("{}.case.json", case.id))
        .collect();
    if let Ok(entries) = std::fs::read_dir(out) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".case.json") && !written.contains(&name) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    let write = |name: &str, value: &dyn erased_json::Json| {
        let path = out.join(name);
        let text = value.pretty() + "\n";
        if let Err(error) = std::fs::write(&path, text) {
            eprintln!("error: cannot write {}: {}", path.display(), error);
            std::process::exit(1);
        }
    };
    for case in &search.cases {
        write(&format!("{}.case.json", case.id), case);
    }
    let count = |kind: &str| {
        search
            .cases
            .iter()
            .filter(|case| case.kind.starts_with(kind))
            .count()
    };
    let counts = CaseCounts {
        clause: count("clause"),
        boundary: count("boundary"),
        scenario: count("scenario"),
        rejected: count("rejected"),
    };
    let line = format!(
        "runa cases: {}; {} cases ({} clause, {} boundary, {} scenario, {} rejected) in {}; uncited rule clauses: {} of {} covered, of which {} not in the result; technical rejections not in the suite: {}",
        summary,
        search.cases.len(),
        counts.clause,
        counts.boundary,
        counts.scenario,
        counts.rejected,
        options.out,
        uncited.covered,
        uncited.clauses,
        uncited.not_in_result,
        search.technical.len(),
    );
    write(
        "coverage.json",
        &CoverageFile {
            entry: contract.entry.clone(),
            summary,
            cited,
            uncited,
            rejections: search.rejections.clone(),
            cases: counts,
            clauses,
            thresholds,
            technical: search.technical,
        },
    );
    println!("{}", line);
}

/// The result variants that are rejections: a `Result`'s `Err`, and each
/// `--rejected` variant, which must be one of the result type's.
fn rejection_variants(
    contract: &calculate::CalculationContract,
    named: &[String],
) -> Result<Vec<String>, String> {
    let definition = match &contract.output {
        calculate::CalculationTypeRef::Named { name, .. } => contract.definition(name),
        _ => None,
    };
    let variants: Vec<&str> = definition
        .map(|definition| {
            definition
                .variants
                .iter()
                .map(|variant| variant.name.as_str())
                .collect()
        })
        .unwrap_or_default();
    let mut rejections = Vec::new();
    let is_result = definition
        .is_some_and(|definition| definition.name == "Result" && variants == ["Ok", "Err"]);
    if is_result {
        rejections.push("Err".to_string());
    }
    for name in named {
        if !variants.contains(&name.as_str()) || variants.len() < 2 {
            return Err(format!(
                "--rejected `{}` is not a variant of the result of `{}`{}",
                name,
                contract.entry,
                if variants.len() < 2 {
                    String::new()
                } else {
                    format!(" (its variants: {})", variants.join(", "))
                }
            ));
        }
        if !rejections.contains(name) {
            rejections.push(name.clone());
        }
    }
    Ok(rejections)
}

/// Pretty JSON of any serializable value behind one `dyn`.
mod erased_json {
    pub(super) trait Json {
        fn pretty(&self) -> String;
    }

    impl<T: serde::Serialize> Json for T {
        fn pretty(&self) -> String {
            serde_json::to_string_pretty(self).expect("case files are JSON")
        }
    }
}
