use calamine::{open_workbook_auto, Data, Reader, SheetVisible};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_PATH_ID: AtomicU64 = AtomicU64::new(0);

fn runa() -> &'static str {
    env!("CARGO_BIN_EXE_runa")
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/calculation/tax.calculate.runa")
}

fn temp_path(extension: &str) -> PathBuf {
    let unique_id = NEXT_TEMP_PATH_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "futuruna-calculate-{}-{}-{}.{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos(),
        unique_id,
        extension
    ))
}

fn run(args: &[&str]) -> Output {
    Command::new(runa())
        .args(args)
        .output()
        .expect("run runa calculation command")
}

fn parse_stdout(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "invalid JSON: {error}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn schema_exposes_reachable_types_metadata_and_fingerprint() {
    let fixture = fixture();
    let output = run(&["schema", fixture.to_str().expect("fixture path")]);
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let schema = parse_stdout(&output);
    assert_eq!(schema["schema"], "futuruna.calculate.v1");
    assert_eq!(schema["entry"], "calculate_tax");
    assert_eq!(schema["label"], "Household tax calculation");
    assert_eq!(schema["input"]["name"], "TaxInput");
    assert_eq!(schema["output"]["name"], "TaxResult");
    assert_eq!(schema["schema_hash"].as_str().expect("hash").len(), 64);

    let definitions = schema["definitions"].as_array().expect("definitions");
    assert!(definitions
        .iter()
        .any(|definition| definition["name"] == "Child"));
    assert!(definitions
        .iter()
        .any(|definition| definition["name"] == "FilingStatus"));
    assert_eq!(schema["metadata"][0]["role"], "source");
    assert_eq!(schema["metadata"][0]["binding"], "tax_source");
    assert_eq!(schema["metadata"][0]["data"]["name"], "SourceInfo");
    assert!(schema["metadata"][0]["symbols"]
        .as_array()
        .expect("metadata symbols")
        .iter()
        .any(|symbol| symbol == "calculate_tax"));

    let field_metadata = schema["field_metadata"].as_array().expect("field metadata");
    assert_eq!(field_metadata.len(), 5);
    let monthly_income = field_metadata
        .iter()
        .find(|metadata| metadata["path"] == "monthly_income")
        .expect("monthly income metadata");
    assert_eq!(monthly_income["label"], "Monthly income");
    assert_eq!(
        monthly_income["question"],
        "What is your income before tax each month?"
    );
    assert_eq!(monthly_income["unit"], "currency/month");
    assert_eq!(monthly_income["sources"][0]["binding"], "tax_source");
    assert_eq!(
        monthly_income["sources"][0]["data"]["arguments"][0]["value"]["value"],
        "https://example.invalid/tax"
    );
    assert!(field_metadata
        .iter()
        .any(|metadata| metadata["path"] == "children.age"));
}

#[test]
fn schema_preserves_localized_keyword_field_spelling() {
    let path = temp_path("runa");
    std::fs::write(
        &path,
        "@ sprog da\n\
# Input(omfang: Heltal)\n\
# Resultat(værdi: Heltal)\n\
# Beregningsfelt(path: ProgramReference, label: Tekst)\n\
# Metarolle(a) = Field(value: a)\n\
# Beregningsmeta(a) = Beregningsmeta(attachments: a)\n\
# impl MetaRole for Metarolle {}\n\
# impl Meta for Beregningsmeta {}\n\
= felt = Beregningsfelt(path = refof(Input::omfang), label = \"Beregningens omfang\")\n\
= metadata = Beregningsmeta(attachments = (Field(value = felt),))\n\
--@label:beregn::meta:metadata--\n\
@ calculate\n\
| beregn(input: Input) -> Resultat(værdi = input.omfang)\n",
    )
    .expect("write localized keyword calculation source");

    let output = run(&["schema", path.to_str().expect("source path")]);
    std::fs::remove_file(&path).ok();
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let schema = parse_stdout(&output);
    let input = schema["definitions"]
        .as_array()
        .expect("definitions")
        .iter()
        .find(|definition| definition["name"] == "Input")
        .expect("Input definition");
    assert_eq!(input["variants"][0]["fields"][0]["name"], "omfang");
    let field = schema["field_metadata"]
        .as_array()
        .expect("field metadata")
        .iter()
        .find(|field| field["path"] == "omfang")
        .expect("localized field metadata");
    assert_eq!(field["label"], "Beregningens omfang");
}

#[test]
fn schema_expands_typed_aggregate_meta_into_field_metadata() {
    let path = temp_path("runa");
    std::fs::write(
        &path,
        "# Input(monthly_income: Int, deduction: Int)\n\
# Result(value: Int)\n\
# CalculationField(path: String, label: String, question: String?, help: String?, unit: String?)\n\
# CalculationMetaRole(a) = Field(value: a) | Source(value: a)\n\
# CalculationMeta(a) = CalculationMeta(fields: List(CalculationField), attachments: a)\n\
# impl MetaRole for CalculationMetaRole {}\n\
# impl Meta for CalculationMeta {}\n\
# SourceInfo(url: String)\n\
= source = SourceInfo(url = \"https://example.invalid/tax\")\n\
= calculation_meta = CalculationMeta(fields = [\n\
    CalculationField(path = \"monthly_income\", label = \"Monthly income\", question = Some(\"What do you earn each month?\"), help = None, unit = Some(\"currency/month\"))\n\
], attachments = (\n\
    Field(value = CalculationField(path = \"deduction\", label = \"Deduction\", question = Some(\"What may be deducted?\"), help = None, unit = Some(\"currency/year\"))),\n\
    Source(value = source),\n\
))\n\
--@label:calculate::meta:calculation_meta--\n\
@ calculate\n\
| calculate(input: Input) -> Result(value = input.monthly_income * 12 - input.deduction)\n",
    )
    .expect("write aggregate calculation metadata source");

    let output = run(&["schema", path.to_str().expect("source path")]);
    std::fs::remove_file(&path).ok();
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let schema = parse_stdout(&output);
    let fields = schema["field_metadata"].as_array().expect("field metadata");
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0]["path"], "deduction");
    assert_eq!(fields[0]["label"], "Deduction");
    assert_eq!(
        fields[0]["binding"],
        "calculation_meta.attachments[0].value"
    );
    assert_eq!(fields[0]["sources"].as_array().expect("sources").len(), 1);
    assert_eq!(fields[0]["sources"][0]["role"], "source");
    assert_eq!(fields[0]["sources"][0]["binding"], "source");
    assert_eq!(
        fields[0]["sources"][0]["attachment_path"],
        "calculation_meta.attachments[1]"
    );
    assert_eq!(fields[1]["path"], "monthly_income");
    assert_eq!(fields[1]["label"], "Monthly income");
    assert_eq!(fields[1]["binding"], "calculation_meta.fields[0]");
}

#[test]
fn schema_lowers_structural_pathof_field_metadata_to_canonical_paths() {
    let path = temp_path("runa");
    std::fs::write(
        &path,
        "# Child(age: Int)\n\
# Income = Wage(amount: Int) | Business(profit: Int)\n\
# Input(children: List(Child), income: Income)\n\
# Result(value: Int)\n\
# CalculationField(path: String, label: String)\n\
# CalculationMeta(fields: List(CalculationField))\n\
# impl Meta for CalculationMeta {}\n\
= calculation_meta = CalculationMeta(fields = [\n\
    CalculationField(path = pathof(Input::children::age), label = \"Child age\"),\n\
    CalculationField(path = pathof(Input::income::$variant), label = \"Income kind\"),\n\
    CalculationField(path = pathof(Input::income::Wage::amount), label = \"Wage amount\")\n\
])\n\
--@label:calculate::meta:calculation_meta--\n\
@ calculate\n\
| calculate(input: Input) -> Result(value = 0)\n",
    )
    .expect("write pathof calculation metadata source");

    let output = run(&["schema", path.to_str().expect("source path")]);
    std::fs::remove_file(&path).ok();
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let schema = parse_stdout(&output);
    let fields = schema["field_metadata"].as_array().expect("field metadata");
    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0]["path"], "children.age");
    assert_eq!(fields[1]["path"], "income.$variant");
    assert_eq!(fields[2]["path"], "income.Wage.amount");
}

#[test]
fn schema_projects_typed_relative_field_metadata_and_prefers_exact_overrides() {
    let path = temp_path("runa");
    std::fs::write(
        &path,
        "# Child(age: Int)\n\
# Household(primary: Child, dependents: List(Child))\n\
# FilingStatus = Active(child: Child) | Inactive\n\
# Input(first: Household, second: Household, filing_status: FilingStatus, children_by_name: Map(String, Child), unique_children: Set(Child))\n\
# Result(value: Int)\n\
# RelativeField(path: ProgramReference, label: String)\n\
# RelativeMeta(fields: List(RelativeField))\n\
# ExactField(path: String, label: String)\n\
# ExactMeta(fields: List(ExactField))\n\
# impl Meta for RelativeMeta {}\n\
# impl Meta for ExactMeta {}\n\
= child_meta = RelativeMeta(fields = [\n\
    RelativeField(path = refof(Child::age), label = \"Child age\")\n\
])\n\
--@label:Child::meta:child_meta--\n\
= exact_meta = ExactMeta(fields = [\n\
    ExactField(path = pathof(Input::first::primary::age), label = \"Primary child age\")\n\
])\n\
--@label:calculate::meta:exact_meta--\n\
@ calculate\n\
| calculate(input: Input) -> Result(value = 0)\n",
    )
    .expect("write relative calculation metadata source");

    let output = run(&["schema", path.to_str().expect("source path")]);
    std::fs::remove_file(&path).ok();
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let schema = parse_stdout(&output);
    let fields = schema["field_metadata"].as_array().expect("field metadata");
    let expected_paths = [
        "children_by_name.age",
        "filing_status.Active.child.age",
        "first.dependents.age",
        "first.primary.age",
        "second.dependents.age",
        "second.primary.age",
        "unique_children.age",
    ];
    assert_eq!(fields.len(), expected_paths.len());
    for expected in expected_paths {
        assert!(
            fields.iter().any(|field| field["path"] == expected),
            "missing projected metadata for {expected}: {fields:?}"
        );
    }
    let exact = fields
        .iter()
        .find(|field| field["path"] == "first.primary.age")
        .expect("exact metadata");
    assert_eq!(exact["label"], "Primary child age");
    assert_eq!(exact["anchor"], "calculate");
    let projected = fields
        .iter()
        .find(|field| field["path"] == "second.primary.age")
        .expect("projected metadata");
    assert_eq!(projected["label"], "Child age");
    assert_eq!(projected["anchor"], "Child");
}

#[test]
fn schema_exact_field_metadata_supersedes_multiple_relative_candidates() {
    let path = temp_path("runa");
    std::fs::write(
        &path,
        "# Child(age: Int)\n\
# Input(child: Child)\n\
# Result(value: Int)\n\
# RelativeField(path: ProgramReference, label: String)\n\
# RelativeMeta(fields: List(RelativeField))\n\
# ExactField(path: String, label: String)\n\
# ExactMeta(fields: List(ExactField))\n\
# impl Meta for RelativeMeta {}\n\
# impl Meta for ExactMeta {}\n\
= relative_meta = RelativeMeta(fields = [\n\
    RelativeField(path = refof(Child::age), label = \"Relative age one\"),\n\
    RelativeField(path = refof(Child::age), label = \"Relative age two\")\n\
])\n\
--@label:Child::meta:relative_meta--\n\
= exact_meta = ExactMeta(fields = [\n\
    ExactField(path = pathof(Input::child::age), label = \"Exact age\")\n\
])\n\
--@label:calculate::meta:exact_meta--\n\
@ calculate\n\
| calculate(input: Input) -> Result(value = input.child.age)\n",
    )
    .expect("write exact metadata precedence source");

    let output = run(&["schema", path.to_str().expect("source path")]);
    std::fs::remove_file(&path).ok();
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let schema = parse_stdout(&output);
    let fields = schema["field_metadata"].as_array().expect("field metadata");
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0]["path"], "child.age");
    assert_eq!(fields[0]["label"], "Exact age");
}

#[test]
fn schema_rejects_ambiguous_or_non_structural_typed_field_metadata() {
    let duplicate_path = temp_path("runa");
    std::fs::write(
        &duplicate_path,
        "# Child(age: Int)\n\
# Input(child: Child)\n\
# Result(value: Int)\n\
# RelativeField(path: ProgramReference, label: String)\n\
# RelativeMeta(fields: List(RelativeField))\n\
# impl Meta for RelativeMeta {}\n\
= child_meta = RelativeMeta(fields = [\n\
    RelativeField(path = refof(Child::age), label = \"Age one\"),\n\
    RelativeField(path = refof(Child::age), label = \"Age two\")\n\
])\n\
--@label:Child::meta:child_meta--\n\
@ calculate\n\
| calculate(input: Input) -> Result(value = 0)\n",
    )
    .expect("write ambiguous relative metadata source");
    let duplicate = run(&[
        "schema",
        duplicate_path.to_str().expect("duplicate source path"),
    ]);
    std::fs::remove_file(&duplicate_path).ok();
    assert!(!duplicate.status.success());
    let duplicate_stderr = String::from_utf8_lossy(&duplicate.stderr);
    assert!(
        duplicate_stderr.contains("duplicate field metadata for `child.age`"),
        "stderr:\n{duplicate_stderr}"
    );

    let symbol_path = temp_path("runa");
    std::fs::write(
        &symbol_path,
        "# Input(value: Int)\n\
# Result(value: Int)\n\
# RelativeField(path: ProgramReference, label: String)\n\
# RelativeMeta(fields: List(RelativeField))\n\
# impl Meta for RelativeMeta {}\n\
= invalid_meta = RelativeMeta(fields = [\n\
    RelativeField(path = refof(calculate), label = \"Invalid\")\n\
])\n\
--@label:calculate::meta:invalid_meta--\n\
@ calculate\n\
| calculate(input: Input) -> Result(value = input.value)\n",
    )
    .expect("write non-structural typed metadata source");
    let symbol = run(&["schema", symbol_path.to_str().expect("symbol source path")]);
    std::fs::remove_file(&symbol_path).ok();
    assert!(!symbol.status.success());
    let symbol_stderr = String::from_utf8_lossy(&symbol.stderr);
    assert!(
        symbol_stderr.contains(
            "field `path` must be a string or a structural `refof(Type::member)` reference"
        ),
        "stderr:\n{symbol_stderr}"
    );

    let optional_composite_path = temp_path("runa");
    std::fs::write(
        &optional_composite_path,
        "# Child(age: Int)\n\
# Input(child: Child?)\n\
# Result(value: Int)\n\
# RelativeField(path: ProgramReference, label: String)\n\
# RelativeMeta(fields: List(RelativeField))\n\
# impl Meta for RelativeMeta {}\n\
= child_meta = RelativeMeta(fields = [\n\
    RelativeField(path = refof(Child::age), label = \"Child age\")\n\
])\n\
--@label:Child::meta:child_meta--\n\
@ calculate\n\
| calculate(input: Input) -> Result(value = 0)\n",
    )
    .expect("write optional composite metadata source");
    let optional_composite = run(&[
        "schema",
        optional_composite_path
            .to_str()
            .expect("optional composite source path"),
    ]);
    std::fs::remove_file(&optional_composite_path).ok();
    assert!(!optional_composite.status.success());
    let optional_stderr = String::from_utf8_lossy(&optional_composite.stderr);
    assert!(
        optional_stderr.contains("targets unknown input path `Child::age`"),
        "stderr:\n{optional_stderr}"
    );
}

#[test]
fn schema_checks_pathof_against_plain_imported_types() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/calculation/pathof-import.calculate.runa");
    let output = run(&["schema", fixture.to_str().expect("fixture path")]);
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let schema = parse_stdout(&output);
    let fields = schema["field_metadata"].as_array().expect("field metadata");
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0]["path"], "children.age");
    assert_eq!(fields[1]["path"], "income.ImportedWage.amount");
}

#[test]
fn schema_projects_type_anchored_field_metadata_from_plain_imports() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/calculation-relative-meta/model.calculate.runa");
    let output = run(&["schema", fixture.to_str().expect("fixture path")]);
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let schema = parse_stdout(&output);
    let fields = schema["field_metadata"].as_array().expect("field metadata");
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0]["path"], "children.age");
    assert_eq!(fields[1]["path"], "primary.age");
    assert!(fields
        .iter()
        .all(|field| field["binding"] == "imported_child_meta.fields[0]"));
}

#[test]
fn schema_expands_computed_typed_aggregate_meta_into_field_metadata() {
    let path = temp_path("runa");
    std::fs::write(
        &path,
        "# Input(first: Int, second: Int, third: Int)\n\
# Result(value: Int)\n\
# CalculationField(path: String, label: String)\n\
# CalculationMeta(fields: List(CalculationField))\n\
# FieldSeed(path: String, label: String)\n\
# impl Meta for CalculationMeta {}\n\
| field_from_seed(seed: FieldSeed) -> CalculationField(path = seed.path, label = seed.label)\n\
| fields_from_seed(seed: FieldSeed) -> [field_from_seed(seed)]\n\
= direct_fields = map([\n\
    FieldSeed(path = \"first\", label = \"First value\"),\n\
    FieldSeed(path = \"second\", label = \"Second value\")\n\
], field_from_seed)\n\
= nested_fields = flat_map([\n\
    FieldSeed(path = \"third\", label = \"Third value\")\n\
], fields_from_seed)\n\
= calculation_meta = CalculationMeta(fields = concat(direct_fields, nested_fields))\n\
--@label:calculate::meta:calculation_meta--\n\
@ calculate\n\
| calculate(input: Input) -> Result(value = input.first + input.second + input.third)\n",
    )
    .expect("write computed aggregate calculation metadata source");

    let output = run(&["schema", path.to_str().expect("source path")]);
    std::fs::remove_file(&path).ok();
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let schema = parse_stdout(&output);
    let fields = schema["field_metadata"].as_array().expect("field metadata");
    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0]["path"], "first");
    assert_eq!(fields[0]["label"], "First value");
    assert_eq!(fields[1]["path"], "second");
    assert_eq!(fields[1]["label"], "Second value");
    assert_eq!(fields[2]["path"], "third");
    assert_eq!(fields[2]["label"], "Third value");
}

#[test]
fn schema_rejects_unknown_paths_from_computed_typed_aggregate_meta() {
    let path = temp_path("runa");
    std::fs::write(
        &path,
        "# Input(amount: Int)\n\
# Result(value: Int)\n\
# CalculationField(path: String, label: String)\n\
# CalculationMeta(fields: List(CalculationField))\n\
# FieldSeed(path: String, label: String)\n\
# impl Meta for CalculationMeta {}\n\
| field_from_seed(seed: FieldSeed) -> CalculationField(path = seed.path, label = seed.label)\n\
= computed_fields = map([\n\
    FieldSeed(path = \"not_an_input_path\", label = \"Invalid value\")\n\
], field_from_seed)\n\
= calculation_meta = CalculationMeta(fields = computed_fields)\n\
--@label:calculate::meta:calculation_meta--\n\
@ calculate\n\
| calculate(input: Input) -> Result(value = input.amount)\n",
    )
    .expect("write invalid computed aggregate calculation metadata source");

    let output = run(&["schema", path.to_str().expect("source path")]);
    std::fs::remove_file(&path).ok();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("targets unknown input path `not_an_input_path`"),
        "stderr:\n{stderr}"
    );
}

#[test]
fn schema_scopes_sources_through_nested_typed_metadata() {
    let path = temp_path("runa");
    std::fs::write(
        &path,
        "# Input(first: Int, second: Int)\n\
# Result(value: Int)\n\
# CalculationField(path: String, label: String)\n\
# CalculationMetaRole(a) = Source(value: a)\n\
# FieldMeta(a) = FieldMeta(fields: List(CalculationField), attachments: a)\n\
# CalculationMeta(a) = CalculationMeta(parts: a)\n\
# impl MetaRole for CalculationMetaRole {}\n\
# impl Meta for CalculationMeta {}\n\
# SourceInfo(url: String)\n\
= first_source = SourceInfo(url = \"https://example.invalid/first\")\n\
= second_source = SourceInfo(url = \"https://example.invalid/second\")\n\
= first_meta = FieldMeta(\n\
    fields = [CalculationField(path = \"first\", label = \"First\")],\n\
    attachments = Source(value = first_source)\n\
)\n\
= second_meta = FieldMeta(\n\
    fields = [CalculationField(path = \"second\", label = \"Second\")],\n\
    attachments = Source(value = second_source)\n\
)\n\
= calculation_meta = CalculationMeta(parts = (first_meta, second_meta))\n\
--@label:calculate::meta:calculation_meta--\n\
@ calculate\n\
| calculate(input: Input) -> Result(value = input.first + input.second)\n",
    )
    .expect("write nested calculation metadata source");

    let output = run(&["schema", path.to_str().expect("source path")]);
    std::fs::remove_file(&path).ok();
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let schema = parse_stdout(&output);
    let fields = schema["field_metadata"].as_array().expect("field metadata");
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0]["path"], "first");
    assert_eq!(fields[0]["sources"].as_array().expect("sources").len(), 1);
    assert_eq!(fields[0]["sources"][0]["binding"], "first_source");
    assert_eq!(
        fields[0]["sources"][0]["attachment_path"],
        "calculation_meta.parts[0].attachments"
    );
    assert_eq!(fields[1]["path"], "second");
    assert_eq!(fields[1]["sources"].as_array().expect("sources").len(), 1);
    assert_eq!(fields[1]["sources"][0]["binding"], "second_source");
    assert_eq!(
        fields[1]["sources"][0]["attachment_path"],
        "calculation_meta.parts[1].attachments"
    );
}

#[test]
fn schema_resolves_metadata_from_plain_imports() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/meta-imports/model.calculate.runa");
    let output = run(&["schema", fixture.to_str().expect("fixture path")]);
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let schema = parse_stdout(&output);
    let metadata = schema["metadata"].as_array().expect("metadata");
    assert_eq!(metadata.len(), 2);
    for reference in metadata {
        assert_eq!(reference["type"], "SourceInfo");
        assert!(reference["value"]
            .as_str()
            .expect("imported metadata value")
            .starts_with("SourceInfo("));
        assert!(reference.get("definition_file").is_none());
    }
}

#[test]
fn schema_collects_typed_field_metadata_anchors_from_plain_imports() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/calculation-imported-meta/model.calculate.runa");
    let output = run(&["schema", fixture.to_str().expect("fixture path")]);
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let schema = parse_stdout(&output);
    let fields = schema["field_metadata"].as_array().expect("field metadata");
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0]["path"], "amount");
    assert_eq!(fields[0]["label"], "Imported amount");
    assert_eq!(fields[0]["binding"], "imported_calculation_meta.felter[0]");
    assert_eq!(fields[0]["sources"][0]["binding"], "imported_source");
}

#[test]
fn schema_rejects_matching_invalid_field_metadata_from_plain_imports() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/calculation-imported-meta/invalid.calculate.runa");
    let output = run(&["schema", fixture.to_str().expect("fixture path")]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("targets unknown input path `not_an_input_path`"),
        "stderr:\n{stderr}"
    );
}

#[test]
fn schema_rejects_duplicate_local_and_imported_field_metadata() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/calculation-imported-meta/duplicate.calculate.runa");
    let output = run(&["schema", fixture.to_str().expect("fixture path")]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("duplicate field metadata for `amount`"),
        "stderr:\n{stderr}"
    );
}

#[test]
fn json_batch_invokes_conditions_and_exceptions() {
    let fixture = fixture();
    let input_path = temp_path("json");
    let template = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--format",
        "json",
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(template.status.success());

    let mut input: Value =
        serde_json::from_slice(&std::fs::read(&input_path).expect("read generated JSON template"))
            .expect("template JSON");
    input["cases"][0]["input"] = serde_json::json!({
        "monthly_income": 50_000,
        "filing_status": { "$variant": "Married" },
        "deduction": 12_000,
        "children": [{ "name": "Ada", "age": 7 }]
    });
    std::fs::write(
        &input_path,
        serde_json::to_vec_pretty(&input).expect("encode input"),
    )
    .expect("write input");

    let output = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&input_path).ok();
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result = parse_stdout(&output);
    assert_eq!(
        result["diagnostics"].as_array().expect("diagnostics").len(),
        0
    );
    assert_eq!(result["results"][0]["result"]["annual_income"], 600_000);
    assert_eq!(result["results"][0]["result"]["taxable_income"], 588_000);
    assert_eq!(result["results"][0]["result"]["annual_tax"], 117_600);
}

#[test]
fn calculation_output_encodes_empty_list_literals() {
    let source_path = temp_path("runa");
    let input_path = temp_path("json");
    std::fs::write(
        &source_path,
        "# EmptyListInput(marker: Int)\n\
# EmptyListResult(marker: Int, items: List(Int))\n\
\n\
@ calculate\n\
| calculate_empty_list(input: EmptyListInput) -> EmptyListResult(marker = input.marker, items = [])\n",
    )
    .expect("write empty-list calculation");

    let template = run(&[
        "template",
        source_path.to_str().expect("source path"),
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(
        template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&template.stderr)
    );

    let output = run(&[
        "call",
        source_path.to_str().expect("source path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&source_path).ok();
    std::fs::remove_file(&input_path).ok();
    assert!(
        output.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let result = parse_stdout(&output);
    assert_eq!(
        result["results"][0]["result"]["items"],
        serde_json::json!([])
    );
}

#[test]
fn stale_json_template_fails_closed_with_both_hashes() {
    let fixture = fixture();
    let input_path = temp_path("json");
    let template = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(template.status.success());
    let mut input: Value = serde_json::from_slice(&std::fs::read(&input_path).expect("template"))
        .expect("template JSON");
    input["$futuruna"]["schema_hash"] = Value::String("stale".to_string());
    std::fs::write(&input_path, serde_json::to_vec_pretty(&input).unwrap()).unwrap();

    let output = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&input_path).ok();
    assert!(!output.status.success());
    let result = parse_stdout(&output);
    let message = result["diagnostics"][0]["message"]
        .as_str()
        .expect("stale diagnostic");
    assert!(message.contains("stale calculation template"));
    assert!(message.contains("stale"));
    assert!(message.contains(
        result["$futuruna"]["schema_hash"]
            .as_str()
            .expect("expected hash")
    ));
    assert!(result["results"].as_array().expect("results").is_empty());
}

#[test]
fn toml_template_round_trips_optional_and_nested_values() {
    let fixture = fixture();
    let input_path = temp_path("toml");
    let template = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--format",
        "toml",
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(
        template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&template.stderr)
    );
    let source = std::fs::read_to_string(&input_path).expect("TOML template");
    assert!(source.contains("[cases.input.filing_status]"));
    assert!(!source.contains("deduction"));

    let output = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&input_path).ok();
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result = parse_stdout(&output);
    assert_eq!(result["results"][0]["result"]["annual_tax"], 0);
}

#[test]
fn xlsx_template_round_trips_and_output_has_result_sheets() {
    let fixture = fixture();
    let input_path = temp_path("xlsx");
    let output_path = temp_path("xlsx");
    let template = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--format",
        "xlsx",
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(
        template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&template.stderr)
    );

    {
        let mut workbook = open_workbook_auto(&input_path).expect("input workbook");
        for expected in [
            "_futuruna",
            "_tables",
            "_columns",
            "_choices",
            "cases",
            "children",
        ] {
            assert!(
                workbook.sheet_names().iter().any(|name| name == expected),
                "missing generated sheet {expected}"
            );
        }
        assert_workbook_visibility(
            &workbook,
            &["_futuruna", "_tables", "_columns", "_choices"],
            "cases",
        );
        assert_eq!(
            workbook_title(&mut workbook, "cases"),
            "Household tax calculation"
        );
        assert_eq!(
            workbook_title(&mut workbook, "children"),
            "Household tax calculation - Children"
        );
        assert_eq!(
            workbook_headers(&mut workbook, "cases"),
            ["case_id", "Monthly income", "Filing status", "Deduction"]
        );
        assert_eq!(
            workbook_headers(&mut workbook, "children"),
            ["case_id", "item_id", "position", "Child name", "Child age"]
        );
        let metadata = workbook
            .worksheet_range("_futuruna")
            .expect("workbook metadata");
        let label = metadata
            .rows()
            .skip(1)
            .find(|row| row.first().map(ToString::to_string).as_deref() == Some("label"))
            .and_then(|row| row.get(1))
            .map(ToString::to_string);
        assert_eq!(label.as_deref(), Some("Household tax calculation"));
        let columns = workbook
            .worksheet_range("_columns")
            .expect("column metadata");
        let monthly_income = columns
            .rows()
            .skip(1)
            .find(|row| row.get(9).map(ToString::to_string).as_deref() == Some("monthly_income"))
            .expect("monthly income column metadata");
        assert_eq!(
            monthly_income.get(2).map(ToString::to_string).as_deref(),
            Some("monthly_income")
        );
        assert_eq!(
            monthly_income.get(10).map(ToString::to_string).as_deref(),
            Some("Monthly income")
        );
        assert_eq!(
            monthly_income.get(11).map(ToString::to_string).as_deref(),
            Some("What is your income before tax each month?")
        );
        assert!(monthly_income
            .get(14)
            .map(ToString::to_string)
            .expect("source metadata")
            .contains("tax_source"));
    }

    edit_workbook(&input_path, |sheets| {
        set_workbook_cell(sheets, "cases", 1, 1, Data::String("50000".to_string()));
        set_workbook_cell(sheets, "cases", 1, 2, Data::String("Married".to_string()));
        set_workbook_cell(sheets, "cases", 1, 3, Data::String("12000".to_string()));
        workbook_sheet_mut(sheets, "children").push(vec![
            Data::String("case-1".to_string()),
            Data::String("child-1".to_string()),
            Data::Int(1),
            Data::String("Ada".to_string()),
            Data::Int(7),
        ]);
    });

    let call = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        input_path.to_str().expect("input path"),
        "--output",
        output_path.to_str().expect("output path"),
    ]);
    assert!(
        call.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&call.stderr)
    );

    let mut workbook = open_workbook_auto(&output_path).expect("result workbook");
    assert_workbook_visibility(&workbook, &["_futuruna"], "results");
    assert!(workbook.sheet_names().iter().any(|name| name == "results"));
    assert!(workbook
        .sheet_names()
        .iter()
        .any(|name| name == "result_values"));
    assert!(workbook
        .sheet_names()
        .iter()
        .any(|name| name == "diagnostics"));
    let results = workbook.worksheet_range("results").expect("results sheet");
    assert_eq!(
        results.get((1, 0)).map(ToString::to_string).as_deref(),
        Some("case-1")
    );
    assert!(results
        .get((1, 1))
        .map(ToString::to_string)
        .expect("result JSON")
        .contains("\"annual_tax\":117600"));
    let result_values = workbook
        .worksheet_range("result_values")
        .expect("result values sheet");
    let annual_tax = result_values
        .rows()
        .skip(1)
        .find(|row| row.get(1).map(ToString::to_string).as_deref() == Some("/annual_tax"))
        .expect("annual tax result value");
    assert_eq!(
        annual_tax.get(2).map(ToString::to_string).as_deref(),
        Some("number")
    );
    assert_eq!(
        annual_tax.get(4).map(ToString::to_string).as_deref(),
        Some("117600")
    );

    std::fs::remove_file(&input_path).ok();
    std::fs::remove_file(&output_path).ok();
}

#[test]
fn xlsx_template_hydrates_populated_json_cases() {
    let fixture = fixture();
    let json_path = temp_path("json");
    let xlsx_path = temp_path("xlsx");
    let template = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--format",
        "json",
        "--output",
        json_path.to_str().expect("JSON path"),
    ]);
    assert!(
        template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&template.stderr)
    );
    let mut input: Value =
        serde_json::from_slice(&std::fs::read(&json_path).expect("JSON template"))
            .expect("template JSON");
    input["cases"][0]["case_id"] = Value::String("hydrated-case".to_string());
    input["cases"][0]["input"] = serde_json::json!({
        "monthly_income": 50_000,
        "filing_status": { "$variant": "Married" },
        "deduction": 12_000,
        "children": [{ "name": "Ada", "age": 7 }]
    });
    std::fs::write(
        &json_path,
        serde_json::to_vec_pretty(&input).expect("encode populated input"),
    )
    .expect("write populated input");

    let hydrated = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--input",
        json_path.to_str().expect("JSON path"),
        "--format",
        "xlsx",
        "--output",
        xlsx_path.to_str().expect("XLSX path"),
    ]);
    assert!(
        hydrated.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&hydrated.stderr)
    );

    {
        let mut workbook = open_workbook_auto(&xlsx_path).expect("hydrated workbook");
        let cases = workbook.worksheet_range("cases").expect("cases sheet");
        assert_eq!(
            cases.get((2, 0)).map(ToString::to_string).as_deref(),
            Some("hydrated-case")
        );
        assert_eq!(
            cases.get((2, 1)).map(ToString::to_string).as_deref(),
            Some("50000")
        );
        assert_eq!(
            cases.get((2, 2)).map(ToString::to_string).as_deref(),
            Some("Married")
        );
        assert_eq!(
            cases.get((2, 3)).map(ToString::to_string).as_deref(),
            Some("12000")
        );
        let children = workbook
            .worksheet_range("children")
            .expect("children sheet");
        assert_eq!(
            children.get((2, 0)).map(ToString::to_string).as_deref(),
            Some("hydrated-case")
        );
        assert_eq!(
            children.get((2, 3)).map(ToString::to_string).as_deref(),
            Some("Ada")
        );
        assert_eq!(
            children.get((2, 4)).map(ToString::to_string).as_deref(),
            Some("7")
        );
    }

    let call = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        xlsx_path.to_str().expect("XLSX path"),
    ]);
    std::fs::remove_file(&json_path).ok();
    std::fs::remove_file(&xlsx_path).ok();
    assert!(
        call.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&call.stderr)
    );
    let result = parse_stdout(&call);
    assert_eq!(result["results"][0]["case_id"], "hydrated-case");
    assert_eq!(result["results"][0]["result"]["annual_tax"], 117_600);
}

#[test]
fn xlsx_long_choice_sets_use_hidden_validation_ranges() {
    let source_path = temp_path("runa");
    let input_path = temp_path("xlsx");
    let choices: Vec<String> = (1..=8)
        .map(|index| format!("ValidationChoiceWithEnoughCharactersNumber{index:02}"))
        .collect();
    let source = format!(
        "# LongChoice = {}\n\
# LongChoiceInput(choice: LongChoice)\n\
# LongChoiceResult(choice: LongChoice)\n\
\n\
@ calculate\n\
| calculate_long_choice(input: LongChoiceInput) -> LongChoiceResult(choice = input.choice)\n",
        choices.join(" | ")
    );
    std::fs::write(&source_path, source).expect("write long-choice calculation");

    let template = run(&[
        "template",
        source_path.to_str().expect("source path"),
        "--format",
        "xlsx",
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(
        template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&template.stderr)
    );

    let mut workbook = open_workbook_auto(&input_path).expect("input workbook");
    assert_workbook_visibility(
        &workbook,
        &["_futuruna", "_tables", "_columns", "_choices"],
        "cases",
    );
    let choice_cells: Vec<String> = workbook
        .worksheet_range("_choices")
        .expect("choice metadata")
        .rows()
        .skip(1)
        .filter_map(|row| row.get(1))
        .map(ToString::to_string)
        .collect();
    assert_eq!(choice_cells, choices);

    std::fs::remove_file(&source_path).ok();
    std::fs::remove_file(&input_path).ok();
}

#[test]
fn personskatteloven_xlsx_boundary_round_trips_source_fact_cases() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/danish-income-tax/personskat.calculate.runa");
    let input_path = temp_path("xlsx");
    let template = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--format",
        "xlsx",
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(
        template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&template.stderr)
    );

    {
        let mut workbook = open_workbook_auto(&input_path).expect("input workbook");
        assert_workbook_visibility(
            &workbook,
            &["_futuruna", "_tables", "_columns", "_choices"],
            "cases",
        );
        let workbook_metadata = workbook
            .worksheet_range("_futuruna")
            .expect("Personskatteloven workbook metadata");
        let calculation_label = workbook_metadata
            .rows()
            .skip(1)
            .find(|row| row.first().map(ToString::to_string).as_deref() == Some("label"))
            .and_then(|row| row.get(1))
            .map(ToString::to_string);
        assert_eq!(calculation_label.as_deref(), Some("Dansk personskat"));
        let case_headers = workbook_headers(&mut workbook, "cases");
        for expected in [
            "Skatteår",
            "Bopælskommune",
            "Årlig bruttoløn",
            "Etablerings- eller iværksætterkonto",
            "Faktisk indskud på etableringskonto",
            "Faktisk indskud på iværksætterkonto",
            "Ægtefælle",
            "Samlevende med ægtefællen ved årets udløb",
            "Ægtefællens årlige bruttoløn",
            "Ægtefællens renteudgifter",
            "Befordringsfradrag",
            "Afstand til folkepensionsalderen",
            "Selvstændigt overskud før VSL § 22 b",
            "Renteudgifter i virksomhedsoverskuddet",
            "Kurstab i virksomhedsoverskuddet",
            "Renteindtægter i virksomhedsoverskuddet",
            "Udbytteindtægter i virksomhedsoverskuddet",
            "Kursgevinster i virksomhedsoverskuddet",
            "Udelukkede afståelsesindkomster",
            "Valg af årsfradrag for livrenter",
            "Ønsket opfyldningsfradrag for livrenter",
            "§ 15 A-fradrag i aktieindkomst",
            "Ønsket pensionsfradrag i aktieindkomst",
            "Dato for meddelelse om aktieindkomstfradrag",
            "Dato for omgørelse af aktieindkomstfradrag",
            "Aldersstatus for personfradrag",
            "Kirkeskat",
            "Årets renteindtægter",
            "Årets renteudgifter",
            "Driftsresultat fra bolig eller fritidsejendom",
            "Ejendomstype for driftsresultatet",
            "Ejendommens beliggenhed",
            "Erhvervsmæssig udlejning",
            "Særlige ejerboligbetingelser opfyldt",
            "Årets overskud eller underskud fra ejendommen",
            "Kursgevinster og kurstab",
            "Udlejning eller fremleje af helårsbolig",
            "Fradragsmetode for udlejningen",
            "Samlet lejeindtægt før fradrag",
            "Samordning med langtidsudlejning",
            "Personen, som beregningen vedrører",
            "Din ægtefælles identifikation",
            "Din folkepensionsalder",
            "Samlevende ægtefælles folkepensionsalder",
            "Skattemæssigt hjemsted for pensionistnedslag",
            "Årsopgørelse",
            "Ordinært aktieår",
            "Fremført tab på markedsaktier",
        ] {
            assert!(
                case_headers.iter().any(|header| header == expected),
                "missing human Personskatteloven input label {expected}"
            );
        }
        let column_metadata = workbook
            .worksheet_range("_columns")
            .expect("column metadata");
        let metadata_headers = column_metadata.rows().next().expect("metadata headers");
        let sheet_column = metadata_headers
            .iter()
            .position(|cell| cell.to_string() == "sheet")
            .expect("sheet metadata column");
        let input_path_column = metadata_headers
            .iter()
            .position(|cell| cell.to_string() == "input_path")
            .expect("input_path metadata column");
        let case_column_count = column_metadata
            .rows()
            .skip(1)
            .filter(|row| {
                row.get(sheet_column)
                    .is_some_and(|cell| cell.to_string() == "cases")
            })
            .count();
        assert_eq!(case_headers.len(), case_column_count + 1);
        let employer_benefits_path =
            "lønmodtager.personlig_indkomst.ordinære_forhold.arbejdsgiverydelser";
        let employer_benefits_sheet =
            workbook_collection_sheet_name(&mut workbook, employer_benefits_path);
        let employer_benefit_paths = workbook_column_paths(&mut workbook, &employer_benefits_sheet);
        for expected in [
            "identifikation",
            "indkomstår",
            "ydelse.$variant",
            "ydelse.DirekteArbejdsgiverbetaltGruppeliv.præmie_før_arbejdsmarkedsbidrag_kroner",
            "ydelse.GruppelivSomUadskiltDelAfPbl19Ordning.personlig_indkomst_efter_indeholdt_arbejdsmarkedsbidrag_kroner",
            "ydelse.NaturalieEfterArbejdsmarkedsbidragslovensPar2Stk2.art",
            "ydelse.NaturalieEfterArbejdsmarkedsbidragslovensPar2Stk2.skattepligtig_værdi_kroner",
            "ydelse.UklassificeretArbejdsgiverbetaltYdelse.beskrivelse",
            "ydelse.UklassificeretArbejdsgiverbetaltYdelse.beløb_kroner",
        ] {
            assert!(
                employer_benefit_paths.iter().any(|path| path == expected),
                "missing canonical ordinary employer-benefit path {expected} on {employer_benefits_sheet}"
            );
        }
        let employer_benefit_headers = workbook_headers(&mut workbook, &employer_benefits_sheet);
        for expected in [
            "Ydelsens identifikation",
            "Ydelsens indkomstår",
            "Arbejdsgiverbetalt ydelse",
            "Direkte arbejdsgiverbetalt gruppeliv før AM-bidrag",
            "Gruppeliv i PBL § 19-ordning efter AM-bidrag",
            "Naturaliets art",
            "Naturaliets skattepligtige værdi",
            "Beskrivelse af uklassificeret ydelse",
            "Uklassificeret arbejdsgiverydelse",
        ] {
            assert!(
                employer_benefit_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human ordinary employer-benefit label {expected} on {employer_benefits_sheet}"
            );
        }
        let businesses_path =
            "lønmodtager.personlig_indkomst.ordinære_forhold.virksomheder_uden_virksomhedsordning";
        let businesses_sheet = workbook_collection_sheet_name(&mut workbook, businesses_path);
        for expected in ["identifikation", "indkomstår"] {
            assert!(
                workbook_column_paths(&mut workbook, &businesses_sheet)
                    .iter()
                    .any(|path| path == expected),
                "missing canonical ordinary-business path {expected} on {businesses_sheet}"
            );
        }
        for expected in ["Virksomhedens identifikation", "Virksomhedens indkomstår"] {
            assert!(
                workbook_headers(&mut workbook, &businesses_sheet)
                    .iter()
                    .any(|header| header == expected),
                "missing human ordinary-business label {expected} on {businesses_sheet}"
            );
        }
        let business_revenues_path = "lønmodtager.personlig_indkomst.ordinære_forhold.virksomheder_uden_virksomhedsordning.indtægter";
        let business_revenues_sheet =
            workbook_collection_sheet_name(&mut workbook, business_revenues_path);
        assert_eq!(
            workbook_column_paths(&mut workbook, &business_revenues_sheet),
            ["identifikation", "art", "beløb_kroner"]
        );
        for expected in [
            "Indtægtspostens identifikation",
            "Indtægtspostens skattemæssige art",
            "Virksomhedsindtægt",
        ] {
            assert!(
                workbook_headers(&mut workbook, &business_revenues_sheet)
                    .iter()
                    .any(|header| header == expected),
                "missing human ordinary-business revenue label {expected} on {business_revenues_sheet}"
            );
        }
        let business_expenses_path = "lønmodtager.personlig_indkomst.ordinære_forhold.virksomheder_uden_virksomhedsordning.udgifter";
        let business_expenses_sheet =
            workbook_collection_sheet_name(&mut workbook, business_expenses_path);
        assert_eq!(
            workbook_column_paths(&mut workbook, &business_expenses_sheet),
            ["identifikation", "afgrænsning", "beløb_kroner"]
        );
        for expected in [
            "Udgiftspostens identifikation",
            "Udgiftens afgrænsning efter PSL § 3, stk. 2, nr. 1",
            "Virksomhedsudgift",
        ] {
            assert!(
                workbook_headers(&mut workbook, &business_expenses_sheet)
                    .iter()
                    .any(|header| header == expected),
                "missing human ordinary-business expense label {expected} on {business_expenses_sheet}"
            );
        }
        let business_travel_path = "lønmodtager.erhvervsbefordring.sager";
        let business_travel_sheet =
            workbook_collection_sheet_name(&mut workbook, business_travel_path);
        let business_travel_paths = workbook_column_paths(&mut workbook, &business_travel_sheet);
        for expected in [
            "identifikation",
            "rækkefølge_i_indkomståret",
            "godtgørende_arbejdsgiver_identifikation",
            "køretøj",
            "befordring.art",
            "befordring.kilometer_i_sagen",
            "godtgørelsesforhold.udbetalt_godtgørelse_kroner",
        ] {
            assert!(
                business_travel_paths.iter().any(|path| path == expected),
                "missing canonical § 9 B source-fact path {expected} on {business_travel_sheet}"
            );
        }
        let business_travel_headers = workbook_headers(&mut workbook, &business_travel_sheet);
        for expected in [
            "case_id",
            "item_id",
            "position",
            "Kørselssag",
            "Rækkefølge i indkomståret",
            "Godtgørende arbejdsgiver",
            "Køretøj til erhvervskørsel",
            "Kilometer i denne erhvervskørsel",
            "Udbetalt kørselsgodtgørelse",
        ] {
            assert!(
                business_travel_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human § 9 B input label {expected} on {business_travel_sheet}"
            );
        }
        let dividend_path = "aktieavance.udbytter";
        let dividend_sheet = workbook_collection_sheet_name(&mut workbook, dividend_path);
        assert_eq!(
            workbook_title(&mut workbook, &dividend_sheet),
            "Dansk personskat - Udbytter og udlodninger"
        );
        let dividend_paths = workbook_column_paths(&mut workbook, &dividend_sheet);
        for expected in [
            "identifikation",
            "udlodder",
            "modtager",
            "aktiv.$variant",
            "aktiv.PersonskatAndelsbevis.forrentning",
            "beløb_kroner",
            "par13a_kildefakta.$variant",
            "par13a_kildefakta.AblPar13AUdbytteForMarkedsaktieEfterPar12.markedsstatus",
            "par13a_kildefakta.AblPar13AUdbytteForMarkedsaktieEfterPar12.aktivklassifikation.indkomstår",
            "par13a_kildefakta.AblPar13AUdbytteForMarkedsaktieEfterPar12.aktivklassifikation.aktiv",
            "par13a_kildefakta.AblPar13AUdbytteForMarkedsaktieEfterPar12.aktivklassifikation.par17_modprøve.næringsstatus",
            "par13a_kildefakta.AblPar13AUdbytteForMarkedsaktieEfterPar12.aktivklassifikation.investeringsklassifikation.$variant",
            "par13a_kildefakta.AblPar13AUdbytteForAktieOmfattetAfPar44.par44_input.identifikation",
            "par13a_kildefakta.AblPar13AUdbytteForAktieOmfattetAfPar44.par44_input.beholdningsfakta.egen_kursværdi_pr_31_december_2005_kroner",
            "par13a_kildefakta.AblPar13AUdbytteForAktieOmfattetAfPar44.par44_input.historisk_undtagelsesstatus",
        ] {
            assert!(
                dividend_paths.iter().any(|path| path == expected),
                "missing canonical dividend source-fact path {expected} on {dividend_sheet}"
            );
        }
        let dividend_headers = workbook_headers(&mut workbook, &dividend_sheet);
        for expected in [
            "case_id",
            "item_id",
            "position",
            "Udlodningens identifikation",
            "Udlodderens retlige type",
            "Din retlige modtagerstatus",
            "Aktiv bag udlodningen",
            "Andelsbevisets forrentning",
            "Modtaget udlodning",
            "Udbyttets grundlag efter ABL § 13 A",
            "Udbytteaktiens markedsstatus",
            "Indkomstår for ABL-klassifikationen",
            "Aktivets ABL-kategori",
            "Næring med køb og salg af aktier",
            "Investeringsaktivets klassifikationsgrundlag",
            "Den historiske akties identifikation",
            "Egen børsnoteret beholdning den 31. december 2005",
            "Historisk undtagelse efter § 2 c eller § 2 e",
        ] {
            assert!(
                dividend_headers.iter().any(|header| header == expected),
                "missing human dividend input label {expected} on {dividend_sheet}"
            );
        }
        let property_tax_path = "ejendomsskatter.ejendomme";
        let property_tax_sheet = workbook_collection_sheet_name(&mut workbook, property_tax_path);
        assert_eq!(
            workbook_title(&mut workbook, &property_tax_sheet),
            "Dansk personskat - Ejendomme med ejendomsskatter"
        );
        let property_tax_paths = workbook_column_paths(&mut workbook, &property_tax_sheet);
        for expected in [
            "ordinært_grundlag.identifikation",
            "ordinært_grundlag.kommune",
            "ordinært_grundlag.kategori",
            "ordinært_grundlag.ejendomsværdi_kroner",
            "ordinært_grundlag.grundværdi_kroner",
            "ordinært_grundlag.ejendomsværdiskatteperiode.$variant",
            "ordinært_grundlag.grundskyldsperiode.$variant",
            "ordinært_grundlag.ejerandel_basispoint",
            "nedslagsfakta.ejerskabshistorik.oprindelig_erhvervelsesdato.år",
            "nedslagsfakta.pensionistsuccession.$variant",
            "overgangsomfang.vurderingskategori",
            "overgangsvurderinger.rabat.$variant",
        ] {
            assert!(
                property_tax_paths.iter().any(|path| path == expected),
                "missing canonical property-tax source-fact path {expected} on {property_tax_sheet}"
            );
        }
        let residence_municipality_choices =
            workbook_column_choices(&mut workbook, "cases", "lønmodtager.kommune");
        let property_municipality_choices = workbook_column_choices(
            &mut workbook,
            &property_tax_sheet,
            "ordinært_grundlag.kommune",
        );
        assert_eq!(residence_municipality_choices.len(), 98);
        assert_eq!(
            property_municipality_choices,
            residence_municipality_choices
        );
        for expected in [
            "København",
            "Frederiksberg",
            "HøjeTaastrup",
            "FaaborgMidtfyn",
            "Aarhus",
            "RingkøbingSkjern",
            "Læsø",
            "Ærø",
        ] {
            assert!(
                residence_municipality_choices
                    .iter()
                    .any(|choice| choice == expected),
                "missing municipality choice {expected}"
            );
        }
        let property_tax_headers = workbook_headers(&mut workbook, &property_tax_sheet);
        for expected in [
            "Ejendommens identifikation",
            "Ejendommens kommune",
            "Ejendomskategori",
            "Vurderet ejendomsværdi",
            "Vurderet grundværdi",
            "Periode med ejendomsværdiskat",
            "Periode med grundskyld",
            "Registreret ejerandel",
            "Oprindeligt erhvervelsesår i ejerforløbet",
            "Længstlevendes rådighed over ejendommen",
            "Vurderingskategori for overgangsregler",
            "Kildegrundlag for skatterabat",
        ] {
            assert!(
                property_tax_headers.iter().any(|header| header == expected),
                "missing human property-tax input label {expected} on {property_tax_sheet}"
            );
        }
        let cfc_path = "cfc.poster";
        let cfc_sheet = workbook_collection_sheet_name(&mut workbook, cfc_path);
        assert_eq!(
            workbook_title(&mut workbook, &cfc_sheet),
            "Dansk personskat - CFC-forhold"
        );
        let cfc_paths = workbook_column_paths(&mut workbook, &cfc_sheet);
        for expected in [
            "$variant",
            "PersonskatCfcEfterLigningslov16H.fakta.udenlandsk_indkomstskat_kroner",
            "PersonskatCfcEfterLigningslov16H.fakta.selskabets_samlede_skattepligtige_indkomst_stk4_5_kroner",
            "PersonskatCfcEfterLigningslov16IStk6Og7.fakta.selskabets_afkast_via_fond_kroner",
            "PersonskatCfcEfterLigningslov16IStk6Og7.fakta.ret_til_merafkast_basispoint",
        ] {
            assert!(
                cfc_paths.iter().any(|path| path == expected),
                "missing canonical CFC source-fact path {expected} on {cfc_sheet}"
            );
        }
        let cfc_headers = workbook_headers(&mut workbook, &cfc_sheet);
        for expected in [
            "CFC-regelgrundlag",
            "Selskabets udenlandske indkomstskat",
            "Selskabets samlede skattepligtige indkomst",
            "Selskabets afkast via fonden",
            "Personens ret til merafkastet",
        ] {
            assert!(
                cfc_headers.iter().any(|header| header == expected),
                "missing human CFC input label {expected} on {cfc_sheet}"
            );
        }
        let pbl53a_path = "kapitalindkomst.pbl53a.ordninger";
        let pbl53a_sheet = workbook_collection_sheet_name(&mut workbook, pbl53a_path);
        assert_eq!(
            workbook_title(&mut workbook, &pbl53a_sheet),
            "Dansk personskat - Pensionsordninger efter PBL § 53 A"
        );
        let pbl53a_paths = workbook_column_paths(&mut workbook, &pbl53a_sheet);
        for expected in [
            "identifikation",
            "skatteyder_identifikation",
            "omfangsfakta.oprettelsesdato.år",
            "omfangsfakta.oprindelig_rettighedshaver_identifikation",
            "omfangsfakta.kapitalværdi_ved_oprettelsen_kroner",
            "omfangsfakta.repræsenteret_kontraktdel.$variant",
            "omfangsfakta.repræsenteret_kontraktdel.Pbl53ADelSkabtVedKontraktændring.ændringsidentifikation",
            "omfangsfakta.overgangsvalgfristfakta.$variant",
            "omfangsfakta.overgangsvalgfristfakta.Pbl53ASenereArvUnderFuldSkattepligt.arvedato.år",
            "omfangsfakta.overgangsvalgfristfakta.Pbl53ASenereArvUnderFuldSkattepligt.fuld_skattepligtig_på_arvedatoen",
            "omfangsfakta.produkt.$variant",
            "omfangsfakta.afsnit_i_valg.$variant",
            "omfangsfakta.institutionsfinansiering.samlet_drift_løn_og_pension_kroner",
            "omfangsfakta.par53b_oprettelsesposition.$variant",
            "afkastforløbsåbning.$variant",
            "afkastforløbsåbning.Pbl53ADokumenteretTidligereAfkasthistorik.seneste_indkomstår",
            "afkastforløbsåbning.Pbl53ADokumenteretTidligereAfkasthistorik.metodetilstand",
        ] {
            assert!(
                pbl53a_paths.iter().any(|path| path == expected),
                "missing canonical PBL § 53 A source-fact path {expected} on {pbl53a_sheet}"
            );
        }
        let pbl53a_headers = workbook_headers(&mut workbook, &pbl53a_sheet);
        for expected in [
            "§ 53 A-ordningens identifikation",
            "Skatteyder på § 53 A-ordningen",
            "Ordningens oprettelsesdato - år",
            "Oprindelig rettighedshaver til ordningen",
            "Kapitalværdi ved ordningens oprettelse",
            "Kontraktdel repræsenteret ved ordningen",
            "Kontraktændring som skabte den repræsenterede del",
            "Fristgrundlag for overgangsvalg efter PBL §§ 53 A eller 53 B",
            "Arvedato for det senere overgangsvalg - år",
            "Fuld dansk skattepligt ved den senere arv",
            "Ordningens faktiske produkttype",
            "Afkald på beskatning efter PBL afsnit I",
            "Institutionens samlede drift, løn og pension",
            "Skattepligt og skattemæssigt hjemsted ved oprettelsen",
            "Historik før det første angivne afkastår",
            "Seneste indkomstår før afkastforløbet",
            "Bindende afkastmetode før afkastforløbet",
        ] {
            assert!(
                pbl53a_headers.iter().any(|header| header == expected),
                "missing human PBL § 53 A input label {expected} on {pbl53a_sheet}"
            );
        }
        let pbl53a_contract_changes_path =
            "kapitalindkomst.pbl53a.ordninger.omfangsfakta.kontraktændringer";
        let pbl53a_contract_changes_sheet =
            workbook_collection_sheet_name(&mut workbook, pbl53a_contract_changes_path);
        assert_eq!(
            workbook_title(&mut workbook, &pbl53a_contract_changes_sheet),
            "Dansk personskat - Ændringer af den historiske pensionskontrakt"
        );
        let pbl53a_contract_change_paths =
            workbook_column_paths(&mut workbook, &pbl53a_contract_changes_sheet);
        for expected in [
            "identifikation",
            "ændringsdato.år",
            "virkningstidspunkt.dato.år",
            "virkningstidspunkt.rækkefølge_på_dagen",
            "kapitalværdi_på_virkningstidspunktet.$variant",
            "kapitalværdi_på_virkningstidspunktet.Pbl53AHeleOrdningensKapitalværdi.kroner",
            "kapitalværdi_på_virkningstidspunktet.Pbl53ANyeDelsKapitalværdi.kroner",
            "forhåndsaftale.$variant",
            "forhåndsaftale.Pbl53ADokumenteretForhåndsaftale.bindende_fra_dato.år",
            "forhåndsaftale.Pbl53ADokumenteretForhåndsaftale.vilkår_fuldt_fastlagt",
            "forhåndsaftale.Pbl53ADokumenteretForhåndsaftale.indtræder_uden_nyt_valg",
            "art.$variant",
            "art.Pbl53AÅrligOpsparingspræmieForhøjet.forhøjelse_kroner",
            "art.Pbl53AÅrligOpsparingspræmieForhøjet.grundlag",
            "art.Pbl53AForsikringGenoptagetEfterMisligholdelse.nye_helbredsoplysninger_krævet",
            "art.Pbl53ASelvvalgtOverflytningTilNyUdbyder.modtagende_ordnings_oprettelsesdato.år",
            "art.Pbl53AAndenKontraktændring.beskrivelse",
        ] {
            assert!(
                pbl53a_contract_change_paths
                    .iter()
                    .any(|path| path == expected),
                "missing canonical PBL § 53 A contract-change path {expected} on {pbl53a_contract_changes_sheet}"
            );
        }
        let pbl53a_contract_change_headers =
            workbook_headers(&mut workbook, &pbl53a_contract_changes_sheet);
        for expected in [
            "Kontraktændringens identifikation",
            "Dato hvor kontraktændringen blev aftalt - år",
            "Dato hvor kontraktændringen fik virkning - år",
            "Kontraktændringens rækkefølge på dagen",
            "Kapitalværdiens rækkevidde ved kontraktændringen",
            "Hele ordningens kapitalværdi ved kontraktændringen",
            "Den nye kontraktdels kapitalværdi ved ændringen",
            "Forhåndsaftale om kontraktændringen",
            "Dato for bindende forhåndsaftale - år",
            "Kontraktændringens art",
            "Årlig forhøjelse af opsparingspræmien",
            "Nye helbredsoplysninger ved genoptagelsen",
            "Oprettelsesdato for modtagende ordning - år",
            "Beskrivelse af anden kontraktændring",
        ] {
            assert!(
                pbl53a_contract_change_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human PBL § 53 A contract-change label {expected} on {pbl53a_contract_changes_sheet}"
            );
        }
        let pbl53a_acquisitions_path = "kapitalindkomst.pbl53a.ordninger.omfangsfakta.erhvervelser";
        let pbl53a_acquisitions_sheet =
            workbook_collection_sheet_name(&mut workbook, pbl53a_acquisitions_path);
        assert_eq!(
            workbook_title(&mut workbook, &pbl53a_acquisitions_sheet),
            "Dansk personskat - Senere erhvervelser af ordningen"
        );
        assert_eq!(
            workbook_column_paths(&mut workbook, &pbl53a_acquisitions_sheet),
            [
                "identifikation",
                "tidspunkt.dato.år",
                "tidspunkt.dato.måned",
                "tidspunkt.dato.dag",
                "tidspunkt.rækkefølge_på_dagen",
                "overdrager_identifikation",
                "erhverver_identifikation",
                "kapitalværdi_på_erhvervelsestidspunktet_kroner",
                "måde",
            ]
        );
        for expected in [
            "Erhvervelsens identifikation",
            "Dato for senere erhvervelse - år",
            "Dato for senere erhvervelse - måned",
            "Dato for senere erhvervelse - dag",
            "Erhvervelsens rækkefølge på dagen",
            "Rettighedshaver før erhvervelsen",
            "Rettighedshaver efter erhvervelsen",
            "Kapitalværdi ved erhvervelsen",
            "Erhvervelsesmåden",
        ] {
            assert!(
                workbook_headers(&mut workbook, &pbl53a_acquisitions_sheet)
                    .iter()
                    .any(|header| header == expected),
                "missing human PBL § 53 A acquisition label {expected} on {pbl53a_acquisitions_sheet}"
            );
        }
        let pbl53a_elections_path = "kapitalindkomst.pbl53a.ordninger.omfangsfakta.overgangsvalg";
        let pbl53a_elections_sheet =
            workbook_collection_sheet_name(&mut workbook, pbl53a_elections_path);
        assert_eq!(
            workbook_title(&mut workbook, &pbl53a_elections_sheet),
            "Dansk personskat - Meddelelser om bindende overgangsvalg"
        );
        assert_eq!(
            workbook_column_paths(&mut workbook, &pbl53a_elections_sheet),
            [
                "beslutningsdato.år",
                "beslutningsdato.måned",
                "beslutningsdato.dag",
                "modtagelsesdato.år",
                "modtagelsesdato.måned",
                "modtagelsesdato.dag",
                "mål",
                "modtager",
                "ønsket_virkning",
            ]
        );
        for expected in [
            "Beslutningsdato for overgangsvalget - år",
            "Modtagelsesdato for overgangsvalget - år",
            "Valgt pensionsbeskatningsregel",
            "Modtager af overgangsvalget",
            "Ønsket virkning af overgangsvalget",
        ] {
            assert!(
                workbook_headers(&mut workbook, &pbl53a_elections_sheet)
                    .iter()
                    .any(|header| header == expected),
                "missing human PBL § 53 A election label {expected} on {pbl53a_elections_sheet}"
            );
        }
        let pbl53a_legacy_forms_path =
            "kapitalindkomst.pbl53a.ordninger.omfangsfakta.historiske_blanket49020_indsendelser";
        let pbl53a_legacy_forms_sheet =
            workbook_collection_sheet_name(&mut workbook, pbl53a_legacy_forms_path);
        assert_eq!(
            workbook_title(&mut workbook, &pbl53a_legacy_forms_sheet),
            "Dansk personskat - Historiske indsendelser af blanket 49.020"
        );
        assert_eq!(
            workbook_column_paths(&mut workbook, &pbl53a_legacy_forms_sheet),
            [
                "indsendelsesdato.år",
                "indsendelsesdato.måned",
                "indsendelsesdato.dag",
                "modtagelsesdato.år",
                "modtagelsesdato.måned",
                "modtagelsesdato.dag",
                "udgave",
                "modtager",
                "påberåbelse",
                "ønsket_virkning",
            ]
        );
        for expected in [
            "Indsendelsesdato for historisk blanket 49.020 - år",
            "Modtagelsesdato for historisk blanket 49.020 - år",
            "Udgave af blanket 49.020",
            "Modtager af blanket 49.020",
            "Skatteyderens påberåbelse af den historiske blanket",
            "Ønsket virkning af den historiske blanket",
        ] {
            assert!(
                workbook_headers(&mut workbook, &pbl53a_legacy_forms_sheet)
                    .iter()
                    .any(|header| header == expected),
                "missing human historical PBL § 53 A form label {expected} on {pbl53a_legacy_forms_sheet}"
            );
        }
        let pbl53a_opening_losses_path = "kapitalindkomst.pbl53a.ordninger.afkastforløbsåbning.Pbl53ADokumenteretTidligereAfkasthistorik.fremførte_negative_afkast";
        let pbl53a_opening_losses_sheet =
            workbook_collection_sheet_name(&mut workbook, pbl53a_opening_losses_path);
        assert_eq!(
            workbook_title(&mut workbook, &pbl53a_opening_losses_sheet),
            "Dansk personskat - Fremførte negative afkast ved afkastforløbets begyndelse"
        );
        assert_eq!(
            workbook_column_paths(&mut workbook, &pbl53a_opening_losses_sheet),
            ["opstået_indkomstår", "resterende_kroner"]
        );
        for expected in [
            "Oprindelsesår for fremført negativt afkast",
            "Resterende fremført negativt afkast",
        ] {
            assert!(
                workbook_headers(&mut workbook, &pbl53a_opening_losses_sheet)
                    .iter()
                    .any(|header| header == expected),
                "missing human PBL § 53 A opening-loss label {expected} on {pbl53a_opening_losses_sheet}"
            );
        }
        let pbl53a_coverages_path = "kapitalindkomst.pbl53a.ordninger.omfangsfakta.produkt.Pbl53ALivsforsikringsprodukt.vilkår.dækninger";
        let pbl53a_coverages_sheet =
            workbook_collection_sheet_name(&mut workbook, pbl53a_coverages_path);
        assert_eq!(
            workbook_title(&mut workbook, &pbl53a_coverages_sheet),
            "Dansk personskat - Livsforsikringens dækninger"
        );
        assert_eq!(
            workbook_column_paths(&mut workbook, &pbl53a_coverages_sheet),
            ["value"]
        );
        for expected in [
            "case_id",
            "parent_id",
            "item_id",
            "position",
            "Livsforsikringens dækninger",
        ] {
            assert!(
                workbook_headers(&mut workbook, &pbl53a_coverages_sheet)
                    .iter()
                    .any(|header| header == expected),
                "missing human PBL § 53 A coverage label {expected} on {pbl53a_coverages_sheet}"
            );
        }
        let pbl53a_years_path = "kapitalindkomst.pbl53a.ordninger.afkastår";
        let pbl53a_years_sheet = workbook_collection_sheet_name(&mut workbook, pbl53a_years_path);
        assert_eq!(
            workbook_title(&mut workbook, &pbl53a_years_sheet),
            "Dansk personskat - Årlige afkastfakta for § 53 A-ordningen"
        );
        let pbl53a_year_paths = workbook_column_paths(&mut workbook, &pbl53a_years_sheet);
        for expected in [
            "indkomstår",
            "afkastgrundlag.$variant",
            "afkastgrundlag.Pbl53AAfkastEfterPal.afkast_efter_pal_par3_til_5_kroner",
            "afkastgrundlag.Pbl53AAlternativtKapitalværdiAfkast.kalenderårets_primo_depotværdi_kroner",
            "afkastgrundlag.Pbl53AAlternativtKapitalværdiAfkast.kalenderårets_ultimo_depotværdi_kroner",
            "pensionsudbyder_opgjorde_afkast_efter_pal",
            "skattepligtsstatus_ved_årets_begyndelse",
            "sikkerhedsstatus_ved_årets_begyndelse",
            "afkastfordeling.$variant",
            "afkastfordeling.Pbl53AFlereBerettigedeVedAfkastperiodensUdgang.rettighedsperiodereference.$variant",
            "afkastfordeling.Pbl53AFlereBerettigedeVedAfkastperiodensUdgang.rettighedsperiodereference.Pbl53ARettighedsperiodeFraErhvervelse.erhvervelsesidentifikation",
            "afkastfordeling.Pbl53AFlereBerettigedeVedAfkastperiodensUdgang.samlet_indestående_ved_afkastperiodens_udgang_kroner",
        ] {
            assert!(
                pbl53a_year_paths.iter().any(|path| path == expected),
                "missing canonical PBL § 53 A annual source-fact path {expected} on {pbl53a_years_sheet}"
            );
        }
        let pbl53a_year_headers = workbook_headers(&mut workbook, &pbl53a_years_sheet);
        for expected in [
            "Indkomstår for afkastet",
            "Opgørelsesgrundlag for årets § 53 A-afkast",
            "Afkast opgjort efter PAL §§ 3-5",
            "Depotværdi ved kalenderårets begyndelse",
            "Depotværdi ved kalenderårets udgang",
            "Pensionsudbyderen har opgjort årets PAL-afkast",
            "Skattepligt ved årets begyndelse",
            "Sikkerhedsstillelse ved årets begyndelse",
            "Fordeling af afkastet",
            "Rettighedsperiodens begyndelse",
            "Erhvervelse, der begyndte rettighedsperioden",
            "Samlet indestående ved afkastperiodens udgang",
        ] {
            assert!(
                pbl53a_year_headers.iter().any(|header| header == expected),
                "missing human PBL § 53 A annual label {expected} on {pbl53a_years_sheet}"
            );
        }
        let pbl53a_boundaries_path = "kapitalindkomst.pbl53a.ordninger.afkastår.grænsehændelser";
        let pbl53a_boundaries_sheet =
            workbook_collection_sheet_name(&mut workbook, pbl53a_boundaries_path);
        assert_eq!(
            workbook_title(&mut workbook, &pbl53a_boundaries_sheet),
            "Dansk personskat - Daterede ændringer i skattepligt eller sikkerhed"
        );
        let pbl53a_boundary_paths = workbook_column_paths(&mut workbook, &pbl53a_boundaries_sheet);
        for expected in [
            "identifikation",
            "tidspunkt.dato.år",
            "tidspunkt.dato.måned",
            "tidspunkt.dato.dag",
            "tidspunkt.rækkefølge_på_dagen",
            "depotværdi_kroner",
            "art",
        ] {
            assert!(
                pbl53a_boundary_paths.iter().any(|path| path == expected),
                "missing canonical PBL § 53 A boundary path {expected} on {pbl53a_boundaries_sheet}"
            );
        }
        let input_choice_values: Vec<String> = workbook
            .worksheet_range("_choices")
            .expect("input choice metadata")
            .rows()
            .skip(1)
            .filter_map(|row| row.get(1))
            .map(ToString::to_string)
            .collect();
        for expected in [
            "Pbl53ASkattepligtIndtræder",
            "Pbl53ASkattepligtOphører",
            "Pbl53ASikkerhedsstillelseEtableres",
            "Pbl53ASikkerhedsstillelseOphører",
            "Pbl53ARettighedsperiodeFraOprettelsen",
            "Pbl53ARettighedsperiodeFraErhvervelse",
        ] {
            assert!(
                input_choice_values.iter().any(|value| value == expected),
                "missing caller-facing PBL § 53 A boundary choice {expected}"
            );
        }
        for derived in ["Pbl53ARettighedErhverves", "Pbl53ARettighedAfstås"] {
            assert!(
                input_choice_values.iter().all(|value| value != derived),
                "derived PBL § 53 A ownership boundary {derived} must not be caller-facing"
            );
        }
        assert!(
            input_choice_values
                .iter()
                .all(|value| value != "oprettelse"),
            "the internal initial PBL § 53 A rights-period key must not be caller-facing"
        );
        let pbl53a_shares_path = "kapitalindkomst.pbl53a.ordninger.afkastår.afkastfordeling.Pbl53AFlereBerettigedeVedAfkastperiodensUdgang.andele";
        let pbl53a_shares_sheet = workbook_collection_sheet_name(&mut workbook, pbl53a_shares_path);
        assert_eq!(
            workbook_title(&mut workbook, &pbl53a_shares_sheet),
            "Dansk personskat - Indeståender for flere berettigede"
        );
        assert_eq!(
            workbook_column_paths(&mut workbook, &pbl53a_shares_sheet),
            [
                "identifikation",
                "indestående_ved_afkastperiodens_udgang_kroner"
            ]
        );
        let pbl53a_events_path = "kapitalindkomst.pbl53a.ordninger.hændelser";
        let pbl53a_events_sheet = workbook_collection_sheet_name(&mut workbook, pbl53a_events_path);
        assert_eq!(
            workbook_title(&mut workbook, &pbl53a_events_sheet),
            "Dansk personskat - Hændelser på § 53 A-ordningen"
        );
        let pbl53a_event_paths = workbook_column_paths(&mut workbook, &pbl53a_events_sheet);
        for expected in [
            "$variant",
            "Pbl53AIndbetaling.fakta.identifikation",
            "Pbl53AIndbetaling.fakta.tidspunkt.dato.år",
            "Pbl53AIndbetaling.fakta.tidspunkt.dato.måned",
            "Pbl53AIndbetaling.fakta.tidspunkt.dato.dag",
            "Pbl53AIndbetaling.fakta.tidspunkt.rækkefølge_på_dagen",
            "Pbl53AIndbetaling.fakta.beløb_kroner",
            "Pbl53AIndbetaling.fakta.indbetaler.$variant",
            "Pbl53AUdbetaling.fakta.bruttoudbetaling_kroner",
            "Pbl53AUdbetaling.fakta.art.$variant",
            "Pbl53AUdbetaling.fakta.art.Pbl53AUdbetalingFraOrdningen.pbl20_stk6_kildefakta.indkomstskat_betalt_til_staten_kroner",
            "Pbl53AUdbetaling.fakta.art.Pbl53AUdbetalingTilAfkastskat.afkast_indkomstår",
            "Pbl53AUdbetaling.fakta.art.Pbl53AUdbetalingTilAfkastskat.dokumenteret_endelig_afkastskat_kroner",
            "Pbl53AUdbetaling.fakta.art.Pbl53AUdbetalingTilAfkastskat.dokumentationsdato.år",
        ] {
            assert!(
                pbl53a_event_paths.iter().any(|path| path == expected),
                "missing canonical PBL § 53 A event path {expected} on {pbl53a_events_sheet}"
            );
        }
        let pbl53a_event_headers = workbook_headers(&mut workbook, &pbl53a_events_sheet);
        for expected in [
            "Type § 53 A-hændelse",
            "Indbetalingens identifikation",
            "Indbetalingens år",
            "Indbetalingens måned",
            "Indbetalingens dag",
            "Indbetalt beløb",
            "Hvem foretog indbetalingen",
            "Udbetalingens modtager",
            "Bruttoudbetaling",
            "Type § 53 A-udbetaling",
            "Indkomstskat betalt til den anden stat",
            "Indkomstår for den dækkede afkastskat",
            "Dokumenteret endelig afkastskat",
            "Modsvarende sikkerhedsstillelse",
        ] {
            assert!(
                pbl53a_event_headers.iter().any(|header| header == expected),
                "missing human PBL § 53 A event label {expected} on {pbl53a_events_sheet}"
            );
        }
        let pension_path = "lønmodtager.pension.pbl18_indbetalinger";
        let pension_sheet = workbook_collection_sheet_name(&mut workbook, pension_path);
        assert_eq!(
            workbook_title(&mut workbook, &pension_sheet),
            "Dansk personskat - Pensionsindbetalinger efter PBL § 18"
        );
        assert_eq!(
            workbook_headers(&mut workbook, &pension_sheet),
            [
                "case_id",
                "item_id",
                "position",
                "Pensionsindbetaling",
                "Type pensionsordning",
                "Hvem foretog indbetalingen",
                "Person med fradragsretten",
                "Indbetalt pensionsbeløb",
                "Indbetalingens forfaldsår",
                "Faktisk betalingsår",
                "Betalt senest den justerede 1. april",
                "Tilbagebetaling efter PBL § 22 E",
                "Fradragsplacering for § 15 A-ordning",
                "Valgt afståelsesår for § 15 A-fradrag",
                "§ 15 A-indbetaling foretaget rettidigt",
                "AM-bidrag af arbejdsgiverindbetalingen",
                "Fordeling af livrentefradraget",
                "År for kapitalindskuddet",
                "Kapitalindskud på livrenten",
                "Første forfaldsår i kort indbetalingsperiode",
                "Samlet aftalt beløb i den korte periode",
                "Efterfølgende tilsvarende pensionsordning",
                "Samlet indbetalingsperiode",
                "Første år med pensionsforhøjelsen",
                "Samlet pensionsforhøjelse",
                "Grundlag for indeksordningen",
                "Indeksordningens form",
                "Indekskontrakt efter pristalsloven",
                "Tidligere forfaldne beløb uden fradrag",
                "Særligt pensionsgrundlag",
                "Ophørspension for indbetalingen",
                "Virksomhedsafståelse for indbetalingen",
                "Sportspension for indbetalingen",
                "Personkredsen i PBL § 54 er opfyldt",
                "Afgiftspligt for hele ordningen er indtrådt",
                "Udenlandsk overførsel med bevaret tidligere fradrag",
            ]
        );
        let index_contributions_path = "lønmodtager.pension.pbl18_indbetalinger.indeksvalg.fradragsvalgte_kontraktbidrag_kroner";
        let index_contributions_sheet =
            workbook_collection_sheet_name(&mut workbook, index_contributions_path);
        assert_eq!(
            workbook_title(&mut workbook, &index_contributions_sheet),
            "Dansk personskat - Valgte indekskontraktbidrag"
        );
        assert_eq!(
            workbook_headers(&mut workbook, &index_contributions_sheet),
            [
                "case_id",
                "parent_id",
                "item_id",
                "position",
                "Valgte indekskontraktbidrag",
            ]
        );
        for (path, expected_title, expected_headers) in [
            (
                "lønmodtager.pension.pbl15b_årsgrundlag.indkomstposter",
                "Dansk personskat - Indkomstposter for sportspension",
                vec![
                    "Indkomstpost for sportspension",
                    "År for retserhvervelse af indkomsten",
                    "Indkomst fra aktiviteten",
                    "Indkomstens forbindelse til sportsudøvelse",
                ],
            ),
            (
                "lønmodtager.pension.pbl15b_årsgrundlag.ordninger",
                "Dansk personskat - Sportspensionsordning",
                vec![
                    "Sportspensionsordningens identifikation",
                    "Sportspensionens oprettelsesår",
                    "Sportspensionens art",
                    "Sportspensionsindehaverens fødselsår",
                    "Påtegnet som sportspension",
                    "Plan for tidlig udbetaling fra sportspensionen",
                    "Første år i rateforsikringsplanen",
                    "Aftalt forsikringssum til tidlige rater",
                    "Første år i rateopsparingsplanen",
                    "Metode for beregning af rateopsparingsrater",
                ],
            ),
            (
                "lønmodtager.pension.pbl15b_årsgrundlag.tidligere_indbetalinger",
                "Dansk personskat - Tidligere indbetalinger på sportspension",
                vec![
                    "Tidligere sportspensionsindbetaling",
                    "År for tidligere sportspensionsindbetaling",
                    "Sportspension for tidligere indbetaling",
                    "Tidligere indbetalt beløb på sportspension",
                    "Arbejdsmarkedsbidrag i tidligere indbetaling",
                ],
            ),
            (
                "lønmodtager.pension.pbl15b_årsgrundlag.rateudbetalinger",
                "Dansk personskat - Tidlige rater fra sportspensionen",
                vec![
                    "Den tidlige rates identifikation",
                    "Sportspension for den tidlige rate",
                    "Indkomstår for den tidlige rate",
                    "Udbetalt tidlig rate",
                    "Beregningsgrundlag for den tidlige rate",
                    "Opsparingens værdi ved årets begyndelse",
                    "Amortisationsrente for annuitetsraten",
                ],
            ),
            (
                "lønmodtager.pension.øvrige_pbl20_årsgrundlag.udbetalinger",
                "Dansk personskat - Øvrige pensionsudbetalinger efter PBL § 20",
                vec![
                    "Pensionsudbetalingens identifikation",
                    "Indkomstår for pensionsudbetalingen",
                    "Pensionsordningens art efter PBL § 20",
                    "Retten til pensionsudbetalingen",
                    "Udbetalingens art efter ligningslovens § 9 L",
                    "Pensionsudbetaling før fritagelser",
                    "Dokumenteret del uden fradrags- eller bortseelsesret",
                ],
            ),
        ] {
            let sheet = workbook_collection_sheet_name(&mut workbook, path);
            assert_eq!(workbook_title(&mut workbook, &sheet), expected_title);
            let headers = workbook_headers(&mut workbook, &sheet);
            for expected in expected_headers {
                assert!(
                    headers.iter().any(|header| header == expected),
                    "missing human § 15 B input label {expected} on {sheet}"
                );
            }
        }
        let canonical_input_paths = column_metadata
            .rows()
            .skip(1)
            .filter_map(|row| row.get(input_path_column))
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        for expected in [
            "aktieavance.udbytter.identifikation",
            "aktieavance.udbytter.udlodder",
            "aktieavance.udbytter.modtager",
            "aktieavance.udbytter.aktiv.$variant",
            "aktieavance.udbytter.aktiv.PersonskatAndelsbevis.forrentning",
            "aktieavance.udbytter.beløb_kroner",
            "aktieavance.ordinært_aktieår.$variant",
            "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb.hændelser.AblOrdinærAfståelse.vilkår.boligret.$variant",
            "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb.hændelser.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.udsteder.$variant",
            "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb.hændelser.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.udsteder.AblPar15Kapitalselskabsudsteder.sel_input.selskabsform",
            "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb.hændelser.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.udsteder.AblPar15Kapitalselskabsudsteder.sel_input.hjemmehørende_i_danmark",
            "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb.hændelser.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.udsteder.AblPar15Foreningsudsteder.sel_input.enhed",
            "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb.hændelser.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.udsteder.AblPar15Foreningsudsteder.sel_input.hjemmehørende_i_danmark",
            "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb.hændelser.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.udsteder.AblPar15Foreningsudsteder.sel_input.omfattet_af_par3_undtagelse",
            "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb.hændelser.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.udsteder.AblPar15Foreningsudsteder.sel_input.omfattet_af_fondsbeskatningsloven",
            "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb.hændelser.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.værdipapirstatus",
            "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb.hændelser.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.fakta.lejlighed_har_tjent_til_bolig_mens_skattefrihedsbetingelser_var_opfyldt_i_ejertiden",
            "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb.hændelser.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.fakta.grundforhold.$variant",
            "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb.hændelser.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.afståelsesform.$variant",
            "lønmodtager.ligningsfradrag.befordring.$variant",
            "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.arbejdsdage",
            "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.arbejdsgiverbetalt_befordring",
            "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.ligningslov9d.$variant",
            "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.ligningslov9d.MedLigningslov9D.input.befordringsudgifter.dokumenteret_faktisk_udgift_kroner",
            "lønmodtager.erhvervsbefordring.sager.identifikation",
            "lønmodtager.erhvervsbefordring.sager.rækkefølge_i_indkomståret",
            "lønmodtager.erhvervsbefordring.sager.godtgørende_arbejdsgiver_identifikation",
            "lønmodtager.erhvervsbefordring.sager.køretøj",
            "lønmodtager.erhvervsbefordring.sager.befordring.kilometer_i_sagen",
            "lønmodtager.erhvervsbefordring.sager.godtgørelsesforhold.udbetalt_godtgørelse_kroner",
            "lønmodtager.pension.pensionsalder_status",
            "lønmodtager.pension.pbl18_selvstændig_overskud.skattepligtigt_overskud_før_vsl22b_kroner",
            "lønmodtager.pension.pbl18_livrentevalg.$variant",
            "lønmodtager.pension.pbl18_livrentevalg.Pbl18Grundbeløbsvalg.ønsket_fradrag_kroner",
            "lønmodtager.pension.aktiepensionsfradrag_valg.$variant",
            "lønmodtager.pension.aktiepensionsfradrag_valg.MedAktiepensionsfradragIAktieindkomst.ønsket_fradrag_kroner",
            "lønmodtager.pension.aktiepensionsfradrag_valg.MedAktiepensionsfradragIAktieindkomst.meddelelse_dato_yyyymmdd",
            "lønmodtager.pension.aktiepensionsfradrag_valg.MedAktiepensionsfradragIAktieindkomst.omgørelse_dato_yyyymmdd",
            "lønmodtager.pension.pbl18_indbetalinger.identifikation",
            "lønmodtager.pension.pbl18_indbetalinger.ordning",
            "lønmodtager.pension.pbl18_indbetalinger.betaling.beløb_kroner",
            "lønmodtager.pension.pbl18_indbetalinger.betaling.arbejdsmarkedsbidrag_kroner",
            "lønmodtager.pension.pbl18_indbetalinger.fordelingsforløb.$variant",
            "lønmodtager.pension.pbl18_indbetalinger.indeksvalg.fradragsvalgte_kontraktbidrag_kroner",
            "lønmodtager.pension.pbl18_indbetalinger.indeksordningsgrundlag.$variant",
            "lønmodtager.pension.pbl18_indbetalinger.indeksordningsgrundlag.Pbl18Par15Indeksordning.fakta.form",
            "lønmodtager.pension.pbl18_indbetalinger.indeksordningsgrundlag.Pbl18Par15Indeksordning.fakta.indekskontrakt_efter_pristalsloven",
            "lønmodtager.pension.pbl18_indbetalinger.særligt_ordningsgrundlag.$variant",
            "lønmodtager.pension.pbl18_indbetalinger.særligt_ordningsgrundlag.Pbl18Par15AIndbetalingsgrundlag.ordning_identifikation",
            "lønmodtager.pension.pbl18_indbetalinger.særligt_ordningsgrundlag.Pbl18Par15AIndbetalingsgrundlag.afståelse_identifikation",
            "lønmodtager.pension.pbl18_indbetalinger.særligt_ordningsgrundlag.Pbl18Par15BIndbetalingsgrundlag.ordning_identifikation",
            "lønmodtager.pension.pbl15a_årsgrundlag.afståelser.fortjenestegrundlag.afskrivningslovsposter.identifikation",
            "lønmodtager.pension.pbl15a_årsgrundlag.afståelser.fortjenestegrundlag.afskrivningslovsposter.kilde.$variant",
            "lønmodtager.pension.pbl15a_årsgrundlag.afståelser.fortjenestegrundlag.ejendomsavance.$variant",
            "lønmodtager.pension.pbl15a_årsgrundlag.afståelser.fortjenestegrundlag.aktieavance.$variant",
            "lønmodtager.pension.pbl15a_årsgrundlag.afståelser.fortjenestegrundlag.kursgevinstposter.identifikation",
            "lønmodtager.pension.pbl15a_årsgrundlag.afståelser.fortjenestegrundlag.kursgevinstposter.input.rolle",
            "lønmodtager.pension.pbl15a_årsgrundlag.ordninger.identifikation",
            "lønmodtager.pension.pbl15a_årsgrundlag.ordninger.oprettelsesafståelse_identifikation",
            "lønmodtager.pension.pbl15a_årsgrundlag.kvalifikationsår.indkomstår",
            "lønmodtager.pension.pbl15a_årsgrundlag.tidligere_indbetalinger.beløb_kroner",
            "lønmodtager.pension.pbl15b_årsgrundlag.indkomstposter.beløb_kroner",
            "lønmodtager.pension.pbl15b_årsgrundlag.indkomstposter.kilde",
            "lønmodtager.pension.pbl15b_årsgrundlag.ordninger.identifikation",
            "lønmodtager.pension.pbl15b_årsgrundlag.ordninger.udbetalingsplan.$variant",
            "lønmodtager.pension.pbl15b_årsgrundlag.ordninger.udbetalingsplan.Pbl15BRateopsparingsplan.metode",
            "lønmodtager.pension.pbl15b_årsgrundlag.rateudbetalinger.udbetalt_kroner",
            "lønmodtager.pension.pbl15b_årsgrundlag.rateudbetalinger.beregningsfakta.$variant",
            "lønmodtager.pension.pbl15b_årsgrundlag.tidligere_indbetalinger.arbejdsmarkedsbidrag_kroner",
            "lønmodtager.pension.øvrige_pbl20_årsgrundlag.udbetalinger.bruttoudbetaling_kroner",
            "lønmodtager.pension.øvrige_pbl20_årsgrundlag.udbetalinger.udbetalingsret.$variant",
            "lønmodtager.pension.øvrige_pbl20_årsgrundlag.udbetalinger.ligningslov9l_art",
            "kapitalindkomst.renter.renteindtægter_kroner",
            "kapitalindkomst.renter.renteudgifter_kroner",
            "kapitalindkomst.renter.næringsstatus",
            "kapitalindkomst.renter.ligningslov6.$variant",
            "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.kurstab_kroner",
            "kapitalindkomst.renter.ligningslov6a.$variant",
            "kapitalindkomst.renter.ligningslov6a.MedLigningslov6AFradrag.input.arbejderboliger_beløb_kroner",
            "kapitalindkomst.pbl53a.ordninger.identifikation",
            "kapitalindkomst.pbl53a.ordninger.skatteyder_identifikation",
            "kapitalindkomst.pbl53a.ordninger.omfangsfakta.oprettelsesdato.år",
            "kapitalindkomst.pbl53a.ordninger.omfangsfakta.produkt.$variant",
            "kapitalindkomst.pbl53a.ordninger.omfangsfakta.afsnit_i_valg.$variant",
            "kapitalindkomst.pbl53a.ordninger.omfangsfakta.par53b_oprettelsesposition.$variant",
            "kapitalindkomst.pbl53a.ordninger.afkastforløbsåbning.$variant",
            "kapitalindkomst.pbl53a.ordninger.afkastforløbsåbning.Pbl53ADokumenteretTidligereAfkasthistorik.seneste_indkomstår",
            "kapitalindkomst.pbl53a.ordninger.afkastforløbsåbning.Pbl53ADokumenteretTidligereAfkasthistorik.metodetilstand",
            "kapitalindkomst.pbl53a.ordninger.afkastforløbsåbning.Pbl53ADokumenteretTidligereAfkasthistorik.fremførte_negative_afkast.opstået_indkomstår",
            "kapitalindkomst.pbl53a.ordninger.afkastforløbsåbning.Pbl53ADokumenteretTidligereAfkasthistorik.fremførte_negative_afkast.resterende_kroner",
            "kapitalindkomst.pbl53a.ordninger.afkastår.indkomstår",
            "kapitalindkomst.pbl53a.ordninger.afkastår.afkastgrundlag.$variant",
            "kapitalindkomst.pbl53a.ordninger.afkastår.afkastgrundlag.Pbl53AAfkastEfterPal.afkast_efter_pal_par3_til_5_kroner",
            "kapitalindkomst.pbl53a.ordninger.afkastår.afkastgrundlag.Pbl53AAlternativtKapitalværdiAfkast.kalenderårets_primo_depotværdi_kroner",
            "kapitalindkomst.pbl53a.ordninger.afkastår.pensionsudbyder_opgjorde_afkast_efter_pal",
            "kapitalindkomst.pbl53a.ordninger.afkastår.skattepligtsstatus_ved_årets_begyndelse",
            "kapitalindkomst.pbl53a.ordninger.afkastår.grænsehændelser.tidspunkt.dato.år",
            "kapitalindkomst.pbl53a.ordninger.afkastår.grænsehændelser.depotværdi_kroner",
            "kapitalindkomst.pbl53a.ordninger.afkastår.afkastfordeling.$variant",
            "kapitalindkomst.pbl53a.ordninger.afkastår.afkastfordeling.Pbl53AFlereBerettigedeVedAfkastperiodensUdgang.rettighedsperiodereference.$variant",
            "kapitalindkomst.pbl53a.ordninger.afkastår.afkastfordeling.Pbl53AFlereBerettigedeVedAfkastperiodensUdgang.rettighedsperiodereference.Pbl53ARettighedsperiodeFraErhvervelse.erhvervelsesidentifikation",
            "kapitalindkomst.pbl53a.ordninger.afkastår.afkastfordeling.Pbl53AFlereBerettigedeVedAfkastperiodensUdgang.andele.indestående_ved_afkastperiodens_udgang_kroner",
            "kapitalindkomst.pbl53a.ordninger.hændelser.$variant",
            "kapitalindkomst.pbl53a.ordninger.hændelser.Pbl53AIndbetaling.fakta.identifikation",
            "kapitalindkomst.pbl53a.ordninger.hændelser.Pbl53AIndbetaling.fakta.tidspunkt.dato.år",
            "kapitalindkomst.pbl53a.ordninger.hændelser.Pbl53AIndbetaling.fakta.indbetaler.$variant",
            "kapitalindkomst.pbl53a.ordninger.hændelser.Pbl53AIndbetaling.fakta.par53b_udenlandsk_skattebehandling.$variant",
            "kapitalindkomst.pbl53a.ordninger.hændelser.Pbl53AUdbetaling.fakta.art.$variant",
            "kapitalindkomst.pbl53a.ordninger.hændelser.Pbl53AUdbetaling.fakta.art.Pbl53AUdbetalingFraOrdningen.pbl20_stk6_kildefakta.indkomstskat_betalt_til_staten_kroner",
            "kapitalindkomst.pbl53a.ordninger.hændelser.Pbl53AUdbetaling.fakta.art.Pbl53AUdbetalingTilAfkastskat.dokumentationsdato.år",
            "kapitalindkomst.ejendomsdrift.$variant",
            "kapitalindkomst.ejendomsdrift.MedEjendomsdriftEfterPar4Nr6.fakta.kategori",
            "kapitalindkomst.ejendomsdrift.MedEjendomsdriftEfterPar4Nr6.fakta.beliggenhed",
            "kapitalindkomst.ejendomsdrift.MedEjendomsdriftEfterPar4Nr6.fakta.erhvervsmæssigt_udlejet",
            "kapitalindkomst.ejendomsdrift.MedEjendomsdriftEfterPar4Nr6.fakta.særlige_betingelser_for_nr6_til_nr8_opfyldt",
            "kapitalindkomst.ejendomsdrift.MedEjendomsdriftEfterPar4Nr6.fakta.overskud_eller_underskud_kroner",
            "kapitalindkomst.ejendomsavance.$variant",
            "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.eget_fremført_tab.$variant",
            "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.eget_fremført_tab.MedFremførtEjendomstab.tab_kroner",
            "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.ægtefælles_fremførte_tab.$variant",
            "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.gift_samlevende_ved_indkomstårets_udgang",
            "kapitalindkomst.kursgevinst.$variant",
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.skatteyder_identifikation",
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.ægtefælles_skatteyder_identifikation",
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.fordringer.identifikation",
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.fordringer.kilde.fordringsart",
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.fordringer.position_primo.$variant",
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.fordringer.hændelser.$variant",
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.obligationsbaserede_minimumsbeviser.identifikation",
            "kapitalindkomst.fremleje.$variant",
            "kapitalindkomst.fremleje.MedFremlejeEfterLigningslov15Q.fakta.metode",
            "kapitalindkomst.fremleje.MedFremlejeEfterLigningslov15Q.fakta.bruttolejeindtægt_kroner",
            "kapitalindkomst.fremleje.MedFremlejeEfterLigningslov15Q.fakta.stk4_samordning.$variant",
            "cfc.poster.$variant",
            "cfc.poster.PersonskatCfcEfterLigningslov16H.fakta.udenlandsk_indkomstskat_kroner",
            "cfc.poster.PersonskatCfcEfterLigningslov16H.fakta.selskabets_cfc_indkomst_stk4_og_sel32_stk5_kroner",
            "cfc.poster.PersonskatCfcEfterLigningslov16IStk6Og7.fakta.selskabets_afkast_via_fond_kroner",
            "cfc.poster.PersonskatCfcEfterLigningslov16IStk6Og7.fakta.fremført_negativt_merafkast_kroner",
            "skatteforhold.$variant",
            "underskudsforhold.$variant",
            "underskudsforhold.MedUnderskudshistorik.egne_tidligere_underskud_kroner",
            "underskudsforhold.MedUnderskudshistorik.aktuelt_underskud_ikke_rummet_i_tidligere_indkomst_eller_skat",
            "ægtefælle.$variant",
            "ægtefælle.MedÆgtefælle.fakta.lønmodtager.bruttoløn_kroner",
            "ægtefælle.MedÆgtefælle.fakta.kapitalindkomst.renter.renteudgifter_kroner",
            "ægtefælle.MedÆgtefælle.samlevende_ved_indkomstårets_udløb",
            "årsopgørelse.$variant",
            "årsopgørelse.MedÅrsopgørelse.kreditter.a_skat_og_am_indeholdt_kroner",
        ] {
            assert!(
                canonical_input_paths.iter().any(|path| path == expected),
                "missing typed Personskatteloven input column {expected}"
            );
        }
        assert!(!canonical_input_paths
            .iter()
            .any(|path| path == "lønmodtager.ligningsmæssige_fradrag_kroner"));
        assert!(!canonical_input_paths
            .iter()
            .any(|path| path.contains("aftrapningsindkomst_kroner")));
        assert!(!canonical_input_paths
            .iter()
            .any(|path| path.contains("ligningslov9d_resultat")));
        assert!(!canonical_input_paths
            .iter()
            .any(|path| path.contains("ligningslov9b_resultat")));
        assert!(!canonical_input_paths
            .iter()
            .any(|path| path.contains("dansk_selskabsskat17_stk1_af_samlet_indkomst_kroner")));
        assert!(!canonical_input_paths
            .iter()
            .any(|path| path == "skatteforhold.SærligeSkatteforhold.forhold.cfc_indkomst_kroner"));
        assert!(!canonical_input_paths
            .iter()
            .any(|path| path.contains("øvrig_aktieindkomst_kroner")));
        assert!(!canonical_input_paths
            .iter()
            .any(|path| path.contains("fradragsforhold.personrolle")));
        assert!(!canonical_input_paths
            .iter()
            .any(|path| path.contains("ægtefælle_skattepligtig_indkomst_kroner")));
        assert!(!canonical_input_paths
            .iter()
            .any(|path| path.contains("overført_skatteværdi_kroner")));
        assert!(!canonical_input_paths
            .iter()
            .any(|path| path.contains("overført_nedslag_kroner")));
        assert!(!canonical_input_paths.iter().any(|path| {
            path == "kapitalindkomst.fremleje.MedFremlejeEfterLigningslov15Q.fakta.stk4_samordning.MedSamordningMedLigningslov15P.indkomstårets_dage"
        }));
        assert_eq!(
            workbook_headers(&mut workbook, "kapitalindkomst_omkostninger"),
            [
                "case_id",
                "item_id",
                "position",
                "Omkostningens identifikation",
                "År hvor omkostningen blev anvendt",
                "Omkostningens formål",
                "Omkostningsart efter ligningslovens § 17 C",
                "Omkostningens næringsstatus",
                "Kapitalindkomstomkostningens beløb"
            ]
        );
        let own_property_sheet = workbook_collection_sheet_name(
            &mut workbook,
            "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.egne_afståelser",
        );
        let spouse_property_sheet = workbook_collection_sheet_name(
            &mut workbook,
            "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.ægtefælles_afståelser",
        );
        for sheet in [&own_property_sheet, &spouse_property_sheet] {
            let property_paths = workbook_column_paths(&mut workbook, sheet);
            for expected in [
                "identifikation",
                "afståelsesdato.år",
                "afståelsesdato.måned",
                "afståelsesdato.dag",
                "afståelse",
                "erhvervet_som_led_i_næring",
                "kontant_anskaffelsessum_kroner",
                "par5_fakta.anskaffelsesdato.år",
                "par5_fakta.anskaffelsesdato.måned",
                "par5_fakta.anskaffelsesdato.dag",
                "par5_fakta.anskaffelsesgrundlag.$variant",
                "par5_fakta.anskaffelsesgrundlag.EblPar4Stk3TredjePktAnskaffelsesgrundlag.tab_efter_ejendomsværdi_par4_stk3_nr1_eller_2_kroner",
                "par5_fakta.fordeling.ejerandel_promille",
                "par5_fakta.fordeling.afståelsesomfang.$variant",
                "par5_fakta.fordeling.afståelsesomfang.EblPar5DelAfEjendommen.afstået_del_anskaffelsessum_før_par5_kroner",
                "par5_fakta.fordeling.afståelsesomfang.EblPar5DelAfEjendommen.hele_ejendommens_anskaffelsessum_før_par5_kroner",
                "par5_fakta.fordeling.afståelsesomfang.EblPar5DelAfEjendommen.ikke_boligdelens_anskaffelsessum_før_par5_kroner",
                "par5_fakta.stk6_overførsel.$variant",
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.ejendomskategori",
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.tillægsparcelværdi_kroner",
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.teknisk_værdi_kroner",
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.afstået_jord_anskaffelsessum_før_overførsel_kroner",
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.samlet_jord_anskaffelsessum_før_overførsel_kroner",
                "par5_fakta.reguleringsvalg.$variant",
                "par5_fakta.reguleringsvalg.EblPar5AMedIndeksering.kategori",
                "kontant_afståelsessum_kroner",
                "par11_stk2_valg.$variant",
                "par11_stk2_valg.EblPar11Stk2MedNytGenanbringelsesvalg.valg.fakta.investering.erhvervsmæssigt_anskaffelsesgrundlag_kroner",
                "par11_stk2_valg.EblPar11Stk2MedNytGenanbringelsesvalg.valg.fakta.begæring.$variant",
                "par11_stk2_valg.EblPar11Stk2MedNytGenanbringelsesvalg.valg.hjemmel.$variant",
                "ejendomstype.$variant",
                "ejendomstype.EblAndenFastEjendom.genanbringelse.$variant",
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.afståelsesindkomstår",
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.erhvervsfortjeneste_før_par6_stk2_kroner",
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.erhvervsmæssigt_anskaffelsesgrundlag_kroner",
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.erhvervsanvendelse.EblPar6AUdlejetTilKontrolleretSelskab.bestemmende_indflydelse_består",
                "ejendomstype.EblBoligejendom.fakta.ejendomsart.$variant",
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.$variant",
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.$variant",
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.afståelsesindkomstår",
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.erhvervsfortjeneste_før_par6_stk2_kroner",
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.erhvervsmæssigt_anskaffelsesgrundlag_kroner",
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.placering.EblPar6AEjendomIUdlandet.begæringsforhold.$variant",
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.anvendelsesændring.ændringsdato.år",
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.anvendelsesændring.ændringsdato.måned",
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.anvendelsesændring.ændringsdato.dag",
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.kategori.$variant",
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.bolig_anskaffelsessum_kroner",
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.genanbringelsesforhold.$variant",
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.genanbringelsesforhold.EblPar9GenanbringelseEfterStk4.genanbringelse.$variant",
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.genanbringelsesforhold.EblPar9GenanbringelseEfterStk4.boligandelsændring.ændringsdato.år",
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.genanbringelsesforhold.EblPar9GenanbringelseEfterStk4.boligandelsændring.ændringsdato.måned",
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.genanbringelsesforhold.EblPar9GenanbringelseEfterStk4.boligandelsændring.ændringsdato.dag",
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.genanbringelsesforhold.EblPar9GenanbringelseEfterStk4.boligandelsændring.boligandel_ved_genanbringelsen_promille",
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.genanbringelsesforhold.EblPar9GenanbringelseEfterStk4.boligandelsændring.boligandel_efter_ændringen_promille",
                "par6d_valg.$variant",
                "par6d_valg.EblMedPar6DValg.fakta.valgt_udskudt_fortjeneste_kroner",
                "par6d_valg.EblMedPar6DValg.fakta.årligt_beløb_kroner",
                "par6d_valg.EblMedPar6DValg.fakta.fordelingsår",
                "par6d_valg.EblMedPar6DValg.fakta.erhververen_erhvervede_ejendommen_som_led_i_næring",
                "par6d_valg.EblMedPar6DValg.fakta.overdragerens_anvendelse.EblPar6DUdlejetTilKontrolleretSelskab.fakta.ejeren_er_fysisk_person",
                "par6d_valg.EblMedPar6DValg.fakta.erhververens_anvendelse_ved_overdragelsen.EblPar6DUdlejetTilKontrolleretSelskab.fakta.ejeren_er_fysisk_person",
                "par6d_valg.EblMedPar6DValg.fakta.sælgerpantebrev.identifikation",
                "par6d_valg.EblMedPar6DValg.fakta.meddelelse.$variant",
                "par6d_valg.EblMedPar6DValg.fakta.ejendomsplacering.$variant",
                "par6d_valg.EblMedPar6DValg.fakta.afståelsesårets_hændelse.$variant",
                "par6d_valg.EblMedPar6DValg.fakta.afståelsesårets_hændelse.EblPar6DPantebrevAfståetEllerIndfriet.afståelses_eller_indfrielsesprovenu_kroner",
            ] {
                assert!(
                    property_paths.iter().any(|path| path == expected),
                    "missing canonical property source-fact path {expected} on {sheet}"
                );
            }
            let property_headers = workbook_headers(&mut workbook, sheet);
            let mut unique_property_headers = property_headers.clone();
            unique_property_headers.sort();
            unique_property_headers.dedup();
            assert_eq!(
                unique_property_headers.len(),
                property_headers.len(),
                "duplicate human property headers on {sheet}"
            );
            for expected in [
                "Ejendom",
                "Afståelsesår",
                "Anskaffelsesår",
                "Kontant anskaffelsessum",
                "Kontant afståelsessum",
                "Anskaffelsesgrundlag",
                "Ejerandel",
                "Hele eller en del af ejendommen",
                "Anskaffelsessum for den afståede del",
                "Anskaffelsessum for hele ejendommen",
                "Anskaffelsessum uden boligdel",
                "Overførsel efter EBL § 5, stk. 6",
                "Ejendomskategori for § 5, stk. 6",
                "Tillægsparcelværdi",
                "Teknisk værdi",
                "Ejendomskategori for § 5 A",
                "Indeksering efter § 5 A",
                "Ejendomstype",
                "Ny genanbringelse efter ekspropriation",
                "Erhvervsmæssigt grundlag for ny genanbringelse",
                "Begæring om ny genanbringelse",
                "Lovgrundlag for den nye genanbringelse",
                "Tidligere genanbringelse på ejendommen",
                "Bestemmende indflydelse består",
                "Boligejendommens art",
                "Genanbringelse ved boligejendom",
                "Lovgrundlag for genanbringelse (§ 8)",
                "Oprindelig erhvervsfortjeneste",
                "Geninvesteringsår",
                "§ 8-anvendelsesændring: år",
                "§ 8-anvendelsesændring: måned",
                "§ 8-anvendelsesændring: dag",
                "Udenlandsk geninvestering i tilladt område",
                "Begæringsforløb for udenlandsk geninvestering",
                "Oplysninger og driftsbudget ved fraflytning",
                "Genanbringelse ved blandet ejendom",
                "Lovgrundlag for genanbringelse (§ 9)",
                "§ 9-boligandelsændring: år",
                "§ 9-boligandelsændring: måned",
                "§ 9-boligandelsændring: dag",
                "Boligandel ved genanbringelsen",
                "Boligandel efter ændringen",
                "Fordeling af ejendomsfortjeneste",
                "Fortjeneste fordelt til senere år",
                "Årligt beskattet fortjeneste",
                "Antal fordelingsår",
                "Køberens erhvervelse af ejendommen",
                "Sælgerens brug af ejendommen",
                "Køberens brug af ejendommen",
                "Udlejer er en fysisk person (sælger)",
                "Udlejer er en fysisk person (køber)",
                "Sælgerpantebrev",
                "Sælgerpantebrevets kontantværdi",
                "Meddelelse om fordelingsvalget",
                "Ejendommens beliggenhed",
                "Hændelse i afståelsesåret",
                "Provenu ved afståelse eller indfrielse",
            ] {
                assert!(
                    property_headers.iter().any(|header| header == expected),
                    "missing human property input label {expected} on {sheet}"
                );
            }
        }
        for property_path in [
            "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.egne_afståelser",
            "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.ægtefælles_afståelser",
        ] {
            let expense_path =
                format!("{property_path}.par5_fakta.vedligeholdelses_og_forbedringsudgifter");
            let expense_sheet = workbook_collection_sheet_name(&mut workbook, &expense_path);
            let expense_paths = workbook_column_paths(&mut workbook, &expense_sheet);
            for expected in [
                "afholdelsesdato.år",
                "afholdelsesdato.måned",
                "afholdelsesdato.dag",
                "fuldførelsesår",
                "beløb_for_afstået_del_kroner",
                "status",
            ] {
                assert!(
                    expense_paths.iter().any(|path| path == expected),
                    "missing canonical § 5 expense path {expected} on {expense_sheet}"
                );
            }

            let reduction_path = format!("{property_path}.par5_fakta.nedsættelser");
            let reduction_sheet = workbook_collection_sheet_name(&mut workbook, &reduction_path);
            let reduction_paths = workbook_column_paths(&mut workbook, &reduction_sheet);
            for expected in ["indkomstår", "beløb_for_afstået_del_kroner", "grund"] {
                assert!(
                    reduction_paths.iter().any(|path| path == expected),
                    "missing canonical § 5 reduction path {expected} on {reduction_sheet}"
                );
            }

            let milk_quota_path = format!("{property_path}.par5_fakta.mælkekvoter");
            let milk_quota_sheet = workbook_collection_sheet_name(&mut workbook, &milk_quota_path);
            let milk_quota_paths = workbook_column_paths(&mut workbook, &milk_quota_sheet);
            for expected in [
                "identifikation",
                "anskaffelsesdato.år",
                "anskaffelsesdato.måned",
                "anskaffelsesdato.dag",
                "oprindelige_enheder",
                "disponerede_enheder",
                "anskaffelsesgrundlag.$variant",
                "anskaffelsesgrundlag.EblPar5MælkekvoteKøbt.vederlag_kroner",
                "anskaffelsesgrundlag.EblPar5MælkekvoteVederlagsfritTildelt.beskattet_værdi_ved_tildeling_kroner",
                "disposition.$variant",
                "disposition.EblPar5MælkekvoteAfstået.afståelsesdato.år",
                "disposition.EblPar5MælkekvoteAfstået.vederlag_kroner",
                "disposition.EblPar5MælkekvoteUdløbet.udløbsdato.år",
                "disposition.EblPar5MælkekvoteToldetUdenErstatning.toldningsdato.år",
            ] {
                assert!(
                    milk_quota_paths.iter().any(|path| path == expected),
                    "missing canonical milk-quota path {expected} on {milk_quota_sheet}"
                );
            }
            let milk_quota_headers = workbook_headers(&mut workbook, &milk_quota_sheet);
            for expected in [
                "Mælkekvote",
                "Anskaffelsesår",
                "Oprindelige kvoteenheder",
                "Disponerede kvoteenheder",
                "Køb eller vederlagsfri tildeling",
                "Købsvederlag",
                "Disposition over mælkekvoten",
                "Afståelsesår",
                "Afståelsesvederlag",
                "Udløbsår",
                "Toldningsår",
            ] {
                assert!(
                    milk_quota_headers.iter().any(|header| header == expected),
                    "missing human milk-quota label {expected} on {milk_quota_sheet}"
                );
            }

            let par6d_schedule_path = format!(
                "{property_path}.par6d_valg.EblMedPar6DValg.fakta.sælgerpantebrev.afdragsplan"
            );
            let par6d_schedule_sheet =
                workbook_collection_sheet_name(&mut workbook, &par6d_schedule_path);
            assert_eq!(
                workbook_headers(&mut workbook, &par6d_schedule_sheet),
                [
                    "case_id",
                    "parent_id",
                    "item_id",
                    "position",
                    "År efter afståelsen",
                    "Forfaldende hovedstol"
                ]
            );

            let par6d_years_path = format!(
                "{property_path}.par6d_valg.EblMedPar6DValg.fakta.efterfølgende_årsforhold"
            );
            let par6d_years_sheet =
                workbook_collection_sheet_name(&mut workbook, &par6d_years_path);
            let par6d_year_paths = workbook_column_paths(&mut workbook, &par6d_years_sheet);
            for expected in ["indkomstår"] {
                assert!(
                    par6d_year_paths.iter().any(|path| path == expected),
                    "missing canonical EBL § 6 D annual path {expected} on {par6d_years_sheet}"
                );
            }
            let par6d_year_headers = workbook_headers(&mut workbook, &par6d_years_sheet);
            for expected in ["Indkomstår"] {
                assert!(
                    par6d_year_headers.iter().any(|header| header == expected),
                    "missing human EBL § 6 D annual label {expected} on {par6d_years_sheet}"
                );
            }

            let par6d_posts_path = format!("{par6d_years_path}.forløbsposter");
            let par6d_posts_sheet =
                workbook_collection_sheet_name(&mut workbook, &par6d_posts_path);
            let par6d_post_paths = workbook_column_paths(&mut workbook, &par6d_posts_sheet);
            for expected in [
                "$variant",
                "EblPar6DOrdinærtHovedstolsafdrag.fordringsdel_identifikation",
                "EblPar6DOrdinærtHovedstolsafdrag.betalt_kroner",
                "EblPar6DFremrykningshændelse.hændelse.$variant",
                "EblPar6DFremrykningshændelse.hændelse.EblPar6DPantebrevAfståetEllerIndfriet.afståelses_eller_indfrielsesprovenu_kroner",
            ] {
                assert!(
                    par6d_post_paths.iter().any(|path| path == expected),
                    "missing canonical EBL § 6 D ordered-post path {expected} on {par6d_posts_sheet}"
                );
            }
            let par6d_post_headers = workbook_headers(&mut workbook, &par6d_posts_sheet);
            for expected in [
                "Posttype",
                "Betalt fordringsdel",
                "Faktisk ordinært hovedstolsafdrag",
                "Hændelse i indkomståret",
                "Provenu ved afståelse eller indfrielse",
            ] {
                assert!(
                    par6d_post_headers.iter().any(|header| header == expected),
                    "missing human EBL § 6 D ordered-post label {expected} on {par6d_posts_sheet}"
                );
            }
        }
        let kgl_seller_note_path =
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.sælgerpantebreve";
        let kgl_seller_note_sheet =
            workbook_collection_sheet_name(&mut workbook, kgl_seller_note_path);
        let kgl_seller_note_paths = workbook_column_paths(&mut workbook, &kgl_seller_note_sheet);
        for expected in [
            "sælgerpantebrev_identifikation",
            "oprindelig_skatteyder_identifikation",
            "skatteyderfakta.udøver_næring_ved_køb_og_salg_af_fordringer",
            "skatteyderfakta.fordringen_erhvervet_uden_for_fordringsnæring",
            "skatteyderfakta.fordringen_erhvervet_som_vederlag_for_leverede_varer_eller_tjenesteydelser",
            "skatteyderfakta.fordringen_erhvervet_i_direkte_tilknytning_til_erhvervsmæssig_drift",
            "skatteyderfakta.debitor_omfattet_af_tabsbegrænsningen_i_kgl_par14_stk2",
            "skatteyderfakta.renter_eller_gevinster_fritaget_efter_dobbeltbeskatningsoverenskomst",
        ] {
            assert!(
                kgl_seller_note_paths.iter().any(|path| path == expected),
                "missing canonical KGL seller-note path {expected} on {kgl_seller_note_sheet}"
            );
        }
        let kgl_seller_note_headers = workbook_headers(&mut workbook, &kgl_seller_note_sheet);
        for expected in [
            "Sælgerpantebrev",
            "Sælgerpantebrevets oprindelige skatteyder",
            "Næring med fordringer",
            "Fordring uden for næring",
            "Vederlag for varer eller ydelser",
            "Direkte tilknytning til virksomheden",
            "Kontrolleret eller nærtstående debitor",
            "Fritagelse efter skatteaftale",
        ] {
            assert!(
                kgl_seller_note_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human KGL seller-note label {expected} on {kgl_seller_note_sheet}"
            );
        }
        let kgl_debt_path = "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.gældsposter";
        let kgl_debt_sheet = workbook_collection_sheet_name(&mut workbook, kgl_debt_path);
        let kgl_debt_paths = workbook_column_paths(&mut workbook, &kgl_debt_sheet);
        for expected in [
            "identifikation",
            "beløb.gældens_værdi_ved_påtagelse_kroner",
            "beløb.gældens_værdi_ved_frigørelse_eller_indfrielse_kroner",
            "beløb.fordringens_værdi_for_kreditor_kroner",
            "frigørelsesart",
            "erhvervsforhold",
            "valuta",
            "selskabsfakta.$variant",
            "gældsordning.$variant",
            "gældsordning.KglFrivilligKreditorordning.fakta.ordningsidentifikation",
            "gældsordning.KglFrivilligKreditorordning.fakta.alle_usikrede_krav_oplyst",
            "vedrører_ikke_indbetalt_selskabskapital",
            "par22_hændelse.$variant",
        ] {
            assert!(
                kgl_debt_paths.iter().any(|path| path == expected),
                "missing canonical KGL debt source-fact path {expected} on {kgl_debt_sheet}"
            );
        }
        for forbidden in ["indkomstår", "behandling"] {
            assert!(
                !kgl_debt_paths.iter().any(|path| path == forbidden),
                "derived KGL debt field {forbidden} leaked into {kgl_debt_sheet}"
            );
        }
        let kgl_debt_headers = workbook_headers(&mut workbook, &kgl_debt_sheet);
        for expected in [
            "Gældens identifikation",
            "Gældens værdi ved påtagelsen",
            "Gældens værdi ved frigørelse eller indfrielse",
            "Fordringens værdi for kreditor",
            "Hvordan gælden ophørte eller blev reduceret",
            "Gældens forbindelse til finansieringsnæring",
            "Gældens valuta og regulering",
            "Nuværende eller tidligere selskabsgæld",
            "Gældsordningens dokumenterede form",
            "Den frivillige kreditorordnings identifikation",
            "Alle usikrede krav er oplyst",
            "Ikke indbetalt selskabskapital",
            "Særlig lånehændelse efter KGL § 22",
        ] {
            assert!(
                kgl_debt_headers.iter().any(|header| header == expected),
                "missing human KGL debt label {expected} on {kgl_debt_sheet}"
            );
        }
        let kgl_annual_claim_path =
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.fordringer";
        let kgl_annual_claim_sheet =
            workbook_collection_sheet_name(&mut workbook, kgl_annual_claim_path);
        let kgl_annual_claim_paths = workbook_column_paths(&mut workbook, &kgl_annual_claim_sheet);
        for expected in [
            "identifikation",
            "kilde.fordringsart",
            "kilde.markedsfakta.$variant",
            "kilde.debitorrelation",
            "kilde.næringsforhold",
            "kilde.erhvervelsesgrundlag",
            "kilde.dba_status",
            "kilde.opgørelsesprincip",
            "position_primo.$variant",
            "position_primo.KglÅrsnettoVidereførtPositionPrimo.fra_indkomstår",
            "position_primo.KglÅrsnettoVidereførtPositionPrimo.skattemæssig_værdi_kroner",
        ] {
            assert!(
                kgl_annual_claim_paths.iter().any(|path| path == expected),
                "missing annual KGL claim source path {expected} on {kgl_annual_claim_sheet}"
            );
        }
        for forbidden in ["rå_netto_kroner", "kursgevinstlov_resultat"] {
            assert!(
                !kgl_annual_claim_paths.iter().any(|path| path == forbidden),
                "derived annual KGL field {forbidden} leaked into {kgl_annual_claim_sheet}"
            );
        }
        let kgl_annual_claim_headers = workbook_headers(&mut workbook, &kgl_annual_claim_sheet);
        for expected in [
            "Fordringens identifikation",
            "Fordringens art",
            "Handel på reguleret marked",
            "Relationen til debitor",
            "Fordrings- eller finansieringsnæring",
            "Hvordan fordringen blev erhvervet",
            "Dobbeltbeskatningsoverenskomst",
            "Opgørelsesprincip for fordringen",
            "Fordringens position ved årets begyndelse",
            "Skattemæssig værdi ved årets begyndelse",
        ] {
            assert!(
                kgl_annual_claim_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human annual KGL claim label {expected} on {kgl_annual_claim_sheet}"
            );
        }
        let kgl_annual_claim_events_path = format!("{kgl_annual_claim_path}.hændelser");
        let kgl_annual_claim_events_sheet =
            workbook_collection_sheet_name(&mut workbook, &kgl_annual_claim_events_path);
        let kgl_annual_claim_event_paths =
            workbook_column_paths(&mut workbook, &kgl_annual_claim_events_sheet);
        for expected in [
            "$variant",
            "KglÅrsnettoAnskaffelse.anskaffelsessum_kroner",
            "KglÅrsnettoAfståelse.afståelsessum_kroner",
            "KglÅrsnettoUltimoværdi.værdi_kroner",
        ] {
            assert!(
                kgl_annual_claim_event_paths
                    .iter()
                    .any(|path| path == expected),
                "missing annual KGL event path {expected} on {kgl_annual_claim_events_sheet}"
            );
        }
        let kgl_annual_claim_event_headers =
            workbook_headers(&mut workbook, &kgl_annual_claim_events_sheet);
        for expected in [
            "Fordringens hændelse",
            "Fordringens anskaffelsessum",
            "Fordringens afståelsessum",
            "Fordringens værdi ved årets udgang",
        ] {
            assert!(
                kgl_annual_claim_event_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human annual KGL event label {expected} on {kgl_annual_claim_events_sheet}"
            );
        }
        let kgl_abl22_path = "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.obligationsbaserede_minimumsbeviser";
        let kgl_abl22_sheet = workbook_collection_sheet_name(&mut workbook, kgl_abl22_path);
        let kgl_abl22_headers = workbook_headers(&mut workbook, &kgl_abl22_sheet);
        for expected in [
            "Minimumsbevisets identifikation",
            "Minimumsbevisets position ved årets begyndelse",
            "Minimumsbevisets anskaffelsessum fra tidligere år",
        ] {
            assert!(
                kgl_abl22_headers.iter().any(|header| header == expected),
                "missing human ABL §22 annual label {expected} on {kgl_abl22_sheet}"
            );
        }
        let kgl_voluntary_claim_path =
            format!("{kgl_debt_path}.gældsordning.KglFrivilligKreditorordning.fakta.krav");
        let kgl_voluntary_claim_sheet =
            workbook_collection_sheet_name(&mut workbook, &kgl_voluntary_claim_path);
        let kgl_voluntary_claim_paths =
            workbook_column_paths(&mut workbook, &kgl_voluntary_claim_sheet);
        for expected in [
            "krav_identifikation",
            "kreditor_identifikation",
            "samlet_krav_kroner",
            "værdi_af_tilstrækkelig_sikkerhed_kroner",
            "deltagelse.$variant",
            "deltagelse.KglKreditorTiltrådtFrivilligOrdning.aftalt_restkrav_kroner",
            "deltagelse.KglKreditorUdenforFrivilligOrdning.småkravsgrundlag.$variant",
            "deltagelse.KglKreditorUdenforFrivilligOrdning.småkravsgrundlag.KglUdeladtKravDokumenteretSomSmåkrav.afgørelsesreference",
            "deltagelse.KglKreditorUdenforFrivilligOrdning.småkravsgrundlag.KglUdeladtKravDokumenteretSomIkkeSmåkrav.afgørelsesreference",
        ] {
            assert!(
                kgl_voluntary_claim_paths
                    .iter()
                    .any(|path| path == expected),
                "missing KGL voluntary-arrangement source path {expected} on {kgl_voluntary_claim_sheet}"
            );
        }
        for forbidden in ["vurdering", "deltagende_andel_basispoint"] {
            assert!(
                !kgl_voluntary_claim_paths
                    .iter()
                    .any(|path| path == forbidden),
                "derived KGL §24 field {forbidden} leaked into {kgl_voluntary_claim_sheet}"
            );
        }
        let kgl_voluntary_claim_headers =
            workbook_headers(&mut workbook, &kgl_voluntary_claim_sheet);
        for expected in [
            "Kreditorkravets identifikation",
            "Kreditorens identifikation",
            "Kreditors samlede krav før ordningen",
            "Værdi af tilstrækkelig sikkerhed",
            "Kreditors faktiske deltagelse",
            "Aftalt restkrav efter ordningen",
            "Dokumentation for et udeladt krav",
            "Reference, som dokumenterer småkravet",
            "Reference, som dokumenterer et væsentligt krav",
        ] {
            assert!(
                kgl_voluntary_claim_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human KGL §24 label {expected} on {kgl_voluntary_claim_sheet}"
            );
        }
        let kgl_disposition_path =
            format!("{kgl_seller_note_path}.dispositioner_efter_ebl_forløbet");
        let kgl_disposition_sheet =
            workbook_collection_sheet_name(&mut workbook, &kgl_disposition_path);
        let kgl_disposition_paths = workbook_column_paths(&mut workbook, &kgl_disposition_sheet);
        for expected in [
            "indkomstår",
            "berørt_tranche_identifikation",
            "art.$variant",
            "art.KglSælgerpantebrevEjerskifte.modtagers_tranche_identifikation",
            "berørt_hovedstol_kroner",
            "afståelses_eller_indfrielsessum_kroner",
        ] {
            assert!(
                kgl_disposition_paths.iter().any(|path| path == expected),
                "missing canonical post-EBL KGL disposition path {expected} on {kgl_disposition_sheet}"
            );
        }
        let kgl_disposition_headers = workbook_headers(&mut workbook, &kgl_disposition_sheet);
        for expected in [
            "Dispositionens indkomstår",
            "Berørt del af sælgerpantebrevet",
            "Disposition med restfordringen",
            "Ny del efter ejerskifte",
            "Berørt hovedstol",
            "Modtaget beløb",
        ] {
            assert!(
                kgl_disposition_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human post-EBL KGL disposition label {expected} on {kgl_disposition_sheet}"
            );
        }
        let kgl_par32_current_contracts_path = "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.par32_kontraktforløb.MedPar32Kontraktforløb.aktuelt_år.kontrakter";
        let kgl_par32_current_contracts_sheet =
            workbook_collection_sheet_name(&mut workbook, kgl_par32_current_contracts_path);
        let kgl_par32_current_contract_paths =
            workbook_column_paths(&mut workbook, &kgl_par32_current_contracts_sheet);
        for expected in [
            "identifikation",
            "rækkefølge_i_indkomståret",
            "kontrakt.aftaleart.$variant",
            "kontrakt.har_modgående_kontrakt_eller_forretning",
            "kontrakt.værdi_primo_kroner",
            "kontrakt.værdi_ultimo_kroner",
            "kontrakt.anskaffelsessum_kroner",
            "kontrakt.afståelsesværdi_kroner",
            "kontrakt.anskaffet_i_indkomståret",
            "kontrakt.realiseret_i_indkomståret",
            "kontrakt.udøver_næring_ved_køb_og_salg_af_finansielle_kontrakter",
            "relationsfakta.$variant",
            "relationsfakta.KglPar32KildeTilknyttetAblAktiv.aktieaktiv_identifikation",
            "underliggende.$variant",
        ] {
            assert!(
                kgl_par32_current_contract_paths
                    .iter()
                    .any(|path| path == expected),
                "missing KGL §32 source path {expected} on {kgl_par32_current_contracts_sheet}"
            );
        }
        for forbidden in [
            "kursgevinstlov_resultat",
            "relation",
            "aktiegrundlag",
            "fremførsel",
        ] {
            assert!(
                !kgl_par32_current_contract_paths
                    .iter()
                    .any(|path| path == forbidden),
                "derived KGL §32 field {forbidden} leaked into {kgl_par32_current_contracts_sheet}"
            );
        }
        let kgl_par32_current_contract_headers =
            workbook_headers(&mut workbook, &kgl_par32_current_contracts_sheet);
        for expected in [
            "Kontraktens identifikation",
            "Kontraktens rækkefølge i indkomståret",
            "Kontraktens dokumenterede aftaleart",
            "Modgående kontrakt eller forretning",
            "Kontraktens anskaffelsessum",
            "Kontraktens afståelsesværdi",
            "Kontraktens dokumenterede tilknytning",
            "Tilknyttet ABL-aktiv",
            "Kontraktens underliggende aktiv",
        ] {
            assert!(
                kgl_par32_current_contract_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human KGL §32 label {expected} on {kgl_par32_current_contracts_sheet}"
            );
        }
        let kgl_par32_history_path = "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.par32_kontraktforløb.MedPar32Kontraktforløb.tidligere_år";
        let kgl_par32_history_sheet =
            workbook_collection_sheet_name(&mut workbook, kgl_par32_history_path);
        let kgl_par32_history_paths =
            workbook_column_paths(&mut workbook, &kgl_par32_history_sheet);
        for expected in [
            "fakta.indkomstår",
            "fakta.valg.tabsprioritet",
            "fakta.valg.fast_ejendomstabsprioritet",
            "fakta.valg.aktiemodregningsvalg.omfang",
            "fakta.valg.aktiemodregningsvalg.beløb.$variant",
            "gift_og_samlevende_ved_indkomstårets_udgang",
        ] {
            assert!(
                kgl_par32_history_paths.iter().any(|path| path == expected),
                "missing historical KGL §32 source path {expected} on {kgl_par32_history_sheet}"
            );
        }
        let kgl_par32_history_contracts_path = format!("{kgl_par32_history_path}.fakta.kontrakter");
        let kgl_par32_history_contracts_sheet =
            workbook_collection_sheet_name(&mut workbook, &kgl_par32_history_contracts_path);
        let kgl_par32_history_contract_paths =
            workbook_column_paths(&mut workbook, &kgl_par32_history_contracts_sheet);
        assert_eq!(
            kgl_par32_history_contract_paths, kgl_par32_current_contract_paths,
            "current and historical KGL §32 contracts must expose the same source-fact columns"
        );
        let special_asset_paths =
            workbook_column_paths(&mut workbook, "aktieavance_særlige_aktiver");
        for expected in [
            "identifikation",
            "kilde.$variant",
            "kilde.PersonskatAktieaktivEfterPar17.fakta.skattepligtsgrundlag",
            "kilde.PersonskatAktieaktivEfterPar17.fakta.instrument",
            "kilde.PersonskatØvrigtAktieaktiv.input.aktiv",
            "kilde.PersonskatØvrigtAktieaktiv.par17_modprøvekilde.$variant",
            "kilde.PersonskatØvrigtAktieaktiv.par17_modprøvekilde.MedPar17Modprøvekilde.fakta.næringsstatus",
            "kilde.PersonskatØvrigtAktieaktiv.input.investeringsklassifikation.$variant",
            "kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.identifikation",
            "kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.opgørelsesår",
            "kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.overdragelse.overdragelsesår",
            "kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.overdragelse.parterne_har_valgt_ordningen",
            "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.identifikation",
            "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.opgørelsesår",
            "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.fraflytningsår",
            "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.ophørsgrund",
            "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.kontekstgrundlag.$variant",
            "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.tilflytning.$variant",
            "markedsstatus",
        ] {
            assert!(
                special_asset_paths.iter().any(|path| path == expected),
                "missing canonical source-level ABL input path {expected}"
            );
        }
        for forbidden in [
            "input.aktiv",
            "input.par17_modprøve.næringsstatus",
            "input.par17_modprøve.erhvervelsesstatus",
            "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.aktieindkomstkontekst.indkomstår",
            "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.aktieindkomstkontekst.øvrig_egen_aktieindkomst_kroner",
            "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.aktieindkomstkontekst.ægtefælles_aktieindkomst_kroner",
            "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.aktieindkomstkontekst.samlevende_med_ægtefælle_ved_indkomstårets_udløb",
        ] {
            assert!(
                !special_asset_paths.iter().any(|path| path == forbidden),
                "legacy caller-selected ABL § 17 path {forbidden} leaked into the canonical workbook"
            );
        }
        assert_eq!(
            workbook_title(&mut workbook, "aktieavance_særlige_aktiver"),
            "Dansk personskat - Andre aktiver efter aktieavancebeskatningsloven"
        );
        let special_asset_headers = workbook_headers(&mut workbook, "aktieavance_særlige_aktiver");
        for expected in [
            "Det særlige aktivs identifikation",
            "Det særlige aktivs retsgrundlag",
            "§ 17-kildens indkomstår",
            "Skattepligtsgrundlag efter ABL §§ 6-7",
            "Næring ved køb og salg af aktier",
            "Instrument prøvet efter ABL § 17",
            "Aktivet erhvervet som led i næring",
            "§ 17-aktivets afståelsessum",
            "§ 17-aktivets anskaffelsessum",
            "§ 17-modprøve for investeringsaktiv",
            "Det øvrige aktivs ABL-kategori",
            "Det særlige aktivs investeringsklassifikation",
            "Medarbejderejeordningens identifikation",
            "Opgørelsesår for medarbejderejeordningen",
            "År for overdragelsen til medarbejderejevirksomheden",
            "Parterne har valgt §§ 35 H-35 K",
            "Fraflytterskatteforløbets identifikation",
            "Opgørelsesår for fraflytterskatten",
            "Indkomstår for fraflytningen",
            "Grund til ophør af dansk beskatningsret",
            "Grundlag for fraflytningsårets aktieindkomstkontekst",
            "Tilbageflytning efter ABL § 39 B",
            "Det særlige aktivs markedsstatus",
        ] {
            assert!(
                special_asset_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human special-asset label {expected}"
            );
        }
        let par35_events_path = "aktieavance.særlige_aktiver.kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.hændelsesposter";
        let par35_events_sheet = workbook_collection_sheet_name(&mut workbook, par35_events_path);
        assert_eq!(
            workbook_title(&mut workbook, &par35_events_sheet),
            "Dansk personskat - Hændelser i overdragerskatteforløbet"
        );
        let par35_event_paths = workbook_column_paths(&mut workbook, &par35_events_sheet);
        for expected in [
            "rækkefølge_i_indkomståret",
            "hændelse.$variant",
            "hændelse.AblPar35HændelseAfståelse.data.hændelsesidentifikation",
            "hændelse.AblPar35HændelseAfståelse.data.indkomstår",
        ] {
            assert!(
                par35_event_paths.iter().any(|path| path == expected),
                "missing ordered § 35 source-event path {expected} on {par35_events_sheet}"
            );
        }
        let par35_event_headers = workbook_headers(&mut workbook, &par35_events_sheet);
        for expected in [
            "Hændelsens rækkefølge i indkomståret",
            "Hændelse i overdragerskatteforløbet",
            "Afståelsens identifikation",
            "Afståelsens indkomstår",
        ] {
            assert!(
                par35_event_headers.iter().any(|header| header == expected),
                "missing human § 35 event label {expected} on {par35_events_sheet}"
            );
        }
        let par35_parties_path = "aktieavance.særlige_aktiver.kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.overdragelse.partier";
        let par35_parties_sheet = workbook_collection_sheet_name(&mut workbook, par35_parties_path);
        assert_eq!(
            workbook_title(&mut workbook, &par35_parties_sheet),
            "Dansk personskat - Overdragne aktiepartier"
        );
        let par35_party_paths = workbook_column_paths(&mut workbook, &par35_parties_sheet);
        for expected in [
            "identifikation",
            "selskabsidentifikation",
            "erhvervelsesrækkefølge",
            "skattemæssig_anskaffelsessum_kroner",
        ] {
            assert!(
                par35_party_paths.iter().any(|path| path == expected),
                "missing § 35 transferred-lot path {expected} on {par35_parties_sheet}"
            );
        }
        let par37_shares_path = "aktieavance.særlige_aktiver.kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.aktier";
        let par37_shares_sheet = workbook_collection_sheet_name(&mut workbook, par37_shares_path);
        assert_eq!(
            workbook_title(&mut workbook, &par37_shares_sheet),
            "Dansk personskat - Aktiepartier ved fraflytningen"
        );
        let par37_share_paths = workbook_column_paths(&mut workbook, &par37_shares_sheet);
        for expected in [
            "identifikation",
            "selskabsidentifikation",
            "erhvervelsesdato.år",
            "handelsværdi_ved_ophør_kroner",
            "skattemæssig_anskaffelsessum_kroner",
            "opgørelseskilde.$variant",
            "aktivgrundlag.AblPar38SærligtAktiv.fakta.markedsstatus",
            "princip",
            "henstandsvalg",
        ] {
            assert!(
                par37_share_paths.iter().any(|path| path == expected),
                "missing § 37-40 departure-lot path {expected} on {par37_shares_sheet}"
            );
        }
        let par37_share_headers = workbook_headers(&mut workbook, &par37_shares_sheet);
        for expected in [
            "Fraflytteraktiens identifikation",
            "Fraflytteraktiens selskab",
            "Aktiepartiets erhvervelsesår",
            "Handelsværdi ved fraflytningen",
            "Skattemæssig anskaffelsessum ved fraflytningen",
            "Opgørelsesmetode ved fraflytningen",
            "Det særlige fraflytteraktivs markedsstatus",
            "Realisations- eller lagerprincip",
            "Valg om henstand for aktiepartiet",
        ] {
            assert!(
                par37_share_headers.iter().any(|header| header == expected),
                "missing human § 37-40 departure-lot label {expected} on {par37_shares_sheet}"
            );
        }
        let par37_events_path = "aktieavance.særlige_aktiver.kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.hændelsesposter";
        let par37_events_sheet = workbook_collection_sheet_name(&mut workbook, par37_events_path);
        assert_eq!(
            workbook_title(&mut workbook, &par37_events_sheet),
            "Dansk personskat - Hændelser i fraflytterskatteforløbet"
        );
        let par37_event_headers = workbook_headers(&mut workbook, &par37_events_sheet);
        for expected in [
            "Fraflytterskattehændelsens identifikation",
            "Fraflytterskattehændelsens indkomstår",
            "Hændelsens rækkefølge i indkomståret",
            "Hændelse efter ABL §§ 39 A-40",
        ] {
            assert!(
                par37_event_headers.iter().any(|header| header == expected),
                "missing human § 37-40 event label {expected} on {par37_events_sheet}"
            );
        }
        let par39b_values_path = "aktieavance.særlige_aktiver.kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.tilflytning.TilflytningEfterPar39B.tilflytningsværdier";
        let par39b_values_sheet = workbook_collection_sheet_name(&mut workbook, par39b_values_path);
        assert_eq!(
            workbook_title(&mut workbook, &par39b_values_sheet),
            "Dansk personskat - Handelsværdier ved tilbageflytningen"
        );
        let par39b_value_headers = workbook_headers(&mut workbook, &par39b_values_sheet);
        for expected in [
            "Aktieparti ved tilbageflytningen",
            "Handelsværdi ved tilbageflytningen",
        ] {
            assert!(
                par39b_value_headers.iter().any(|header| header == expected),
                "missing human § 39 B value label {expected} on {par39b_values_sheet}"
            );
        }
        let ordinary_investment_certificates_path =
            "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.investeringsbeviser";
        let ordinary_investment_certificates_sheet =
            workbook_collection_sheet_name(&mut workbook, ordinary_investment_certificates_path);
        assert_eq!(
            workbook_title(&mut workbook, &ordinary_investment_certificates_sheet),
            "Dansk personskat - Investeringsbeviser efter ABL § 13 A"
        );
        let ordinary_investment_certificate_paths =
            workbook_column_paths(&mut workbook, &ordinary_investment_certificates_sheet);
        for expected in [
            "indkomstår",
            "art",
            "afståelsessum_kroner",
            "anskaffelsessum_kroner",
            "oplysningsstatus",
        ] {
            assert!(
                ordinary_investment_certificate_paths
                    .iter()
                    .any(|path| path == expected),
                "missing canonical § 13 A investment-certificate path {expected}"
            );
        }
        let ordinary_investment_certificate_headers =
            workbook_headers(&mut workbook, &ordinary_investment_certificates_sheet);
        for expected in [
            "Investeringsbevisets indkomstår",
            "Investeringsbevisets art",
            "Investeringsbevisets afståelsessum",
            "Investeringsbevisets anskaffelsessum",
            "Oplysningsstatus for investeringsbeviset",
        ] {
            assert!(
                ordinary_investment_certificate_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human § 13 A investment-certificate label {expected}"
            );
        }
        let ordinary_holdings_path =
            "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb";
        let ordinary_events_path = format!("{ordinary_holdings_path}.hændelser");
        let ordinary_events_sheet =
            workbook_collection_sheet_name(&mut workbook, &ordinary_events_path);
        let ordinary_event_headers = workbook_headers(&mut workbook, &ordinary_events_sheet);
        for expected in [
            "Kildedata til tabsbegrænsning efter ABL § 5 A",
            "Tidsmæssigt grundlag for ABL § 5 A",
            "Skatteydergrundlag for ABL § 5 A",
            "Tilsvarende udbytte på præferenceaktier",
            "Præferenceudbytte allerede anvendt",
            "Boligret efter ABL § 15",
            "Værdipapir med boligret",
            "Udsteder af værdipapiret med boligret",
            "Kapitalselskabets registrerede form",
            "Kapitalselskabets skattemæssige hjemsted",
            "Foreningens juridiske type",
            "Foreningens skattemæssige hjemsted",
            "Undtagelse efter selskabsskattelovens § 3",
            "Foreningen er omfattet af fondsbeskatningsloven",
            "Værdipapirets ABL-status",
            "Udstederens ejendom med flere boliger",
            "Boligbrug i kvalificerende periode",
            "Grundareal knyttet til lejligheden",
            "Grundbetingelse efter EBL § 8",
            "Afståelsesform for boligret",
            "Udloddet i endeligt opløsningsår",
        ] {
            assert!(
                ordinary_event_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human ABL § 15 input label {expected} on {ordinary_events_sheet}"
            );
        }
        let par5a_facts_path = format!(
            "{ordinary_events_path}.AblOrdinærAfståelse.par5a_kildefakta.AblOrdinærPar5AKildefakta.fakta"
        );
        let par5a_dividends_path = format!("{par5a_facts_path}.ejertidsudbytter");
        let par5a_dividends_sheet =
            workbook_collection_sheet_name(&mut workbook, &par5a_dividends_path);
        assert_eq!(
            workbook_title(&mut workbook, &par5a_dividends_sheet),
            "Dansk personskat - Udbytter modtaget i ejertiden"
        );
        let par5a_dividend_headers = workbook_headers(&mut workbook, &par5a_dividends_sheet);
        for expected in [
            "Udbyttets art efter ABL § 5 A",
            "Skattefrit udbytte af de afståede aktier",
            "Udbytte med dobbeltbeskatningslempelse",
            "Opnået dobbeltbeskatningslempelse",
            "Betalt skat i den fremmede stat",
        ] {
            assert!(
                par5a_dividend_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human ABL § 5 A dividend label {expected} on {par5a_dividends_sheet}"
            );
        }
        let par5a_group_amounts_path = format!("{par5a_facts_path}.koncernbeløb");
        let par5a_group_amounts_sheet =
            workbook_collection_sheet_name(&mut workbook, &par5a_group_amounts_path);
        assert_eq!(
            workbook_title(&mut workbook, &par5a_group_amounts_sheet),
            "Dansk personskat - Koncernbeløb efter ABL § 5 A, stk. 3"
        );
        let par5a_group_amount_headers =
            workbook_headers(&mut workbook, &par5a_group_amounts_sheet);
        for expected in [
            "Koncernbeløbets art",
            "Yderens relation til det tabsgivende selskab",
            "Modtagerens relation til det tabsgivende selskab",
            "Personens kontrol over yder og modtager",
            "Tilskud eller præferenceudbytte mellem koncernselskaber",
        ] {
            assert!(
                par5a_group_amount_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human ABL § 5 A group-amount label {expected} on {par5a_group_amounts_sheet}"
            );
        }

        let choices = workbook
            .worksheet_range("_choices")
            .expect("choice metadata");
        assert!(choices
            .rows()
            .flatten()
            .any(|cell| cell.to_string() == "AblNæringsaktiePar17"));
        assert!(choices
            .rows()
            .flatten()
            .any(|cell| cell.to_string() == "MedEjendomsdriftEfterPar4Nr6"));

        let metadata = workbook
            .worksheet_range("_columns")
            .expect("column metadata");
        let annual_assessment_row = metadata
            .rows()
            .skip(1)
            .find(|row| {
                row.get(2).map(ToString::to_string).as_deref()
                    == Some("årsopgørelse.MedÅrsopgørelse.overført_restskat_mv_kroner")
            })
            .expect("annual assessment payload metadata");
        assert!(annual_assessment_row
            .get(8)
            .map(ToString::to_string)
            .expect("variant guard")
            .contains("MedÅrsopgørelse"));
        let commuting_row = metadata
            .rows()
            .skip(1)
            .find(|row| {
                row.get(2).map(ToString::to_string).as_deref()
                    == Some("lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.arbejdsdage")
            })
            .expect("commuting payload metadata");
        assert!(commuting_row
            .get(8)
            .map(ToString::to_string)
            .expect("commuting variant guard")
            .contains("MedBefordringsfradrag"));
        let metadata_headers = metadata.rows().next().expect("column metadata headers");
        let sources_column = metadata_headers
            .iter()
            .position(|cell| cell.to_string() == "sources")
            .expect("sources metadata column");
        let label_column = metadata_headers
            .iter()
            .position(|cell| cell.to_string() == "label")
            .expect("label metadata column");
        let question_column = metadata_headers
            .iter()
            .position(|cell| cell.to_string() == "question")
            .expect("question metadata column");
        for (path, expected_label, expected_question_fragment) in [
            (
                "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb.hændelser.AblOrdinærAfståelse.par5a_kildefakta.$variant",
                "Kildedata til tabsbegrænsning efter ABL § 5 A",
                "Kræver afståelsen oplysninger",
            ),
            (
                "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb.hændelser.AblOrdinærAfståelse.par5a_kildefakta.AblOrdinærPar5AKildefakta.fakta.anvendelsesgrundlag",
                "Tidsmæssigt grundlag for ABL § 5 A",
                "afståelse den 24. november 2010",
            ),
            (
                "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb.hændelser.AblOrdinærAfståelse.par5a_kildefakta.AblOrdinærPar5AKildefakta.fakta.skatteydergrundlag",
                "Skatteydergrundlag for ABL § 5 A",
                "Hvilken skatteyderkategori",
            ),
            (
                "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb.hændelser.AblOrdinærAfståelse.par5a_kildefakta.AblOrdinærPar5AKildefakta.fakta.ejertidsudbytter.AblPar5ASkattefritUdbytteAfPågældendeAktier.beløb_kroner",
                "Skattefrit udbytte af de afståede aktier",
                "Hvor stort var det skattefrie udbytte",
            ),
            (
                "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb.hændelser.AblOrdinærAfståelse.par5a_kildefakta.AblOrdinærPar5AKildefakta.fakta.koncernbeløb.beløb_kroner",
                "Tilskud eller præferenceudbytte mellem koncernselskaber",
                "Hvor stort var koncernbeløbet",
            ),
            (
                "lønmodtager.erhvervsbefordring.sager.identifikation",
                "Kørselssag",
                "entydig identifikation",
            ),
            (
                "lønmodtager.erhvervsbefordring.sager.godtgørende_arbejdsgiver_identifikation",
                "Godtgørende arbejdsgiver",
                "Hvilken arbejdsgiver",
            ),
            (
                "lønmodtager.erhvervsbefordring.sager.rækkefølge_i_indkomståret",
                "Rækkefølge i indkomståret",
                "kronologiske rækkefølge",
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.arbejdsgiverbetalt_befordring",
                "Arbejdsgiverbetalt transport",
                "Hvilken form",
            ),
            (
                "lønmodtager.pension.pbl15a_årsgrundlag.afståelser.identifikation",
                "Virksomhedsafståelse",
                "entydig identifikation",
            ),
            (
                "lønmodtager.pension.pbl15a_årsgrundlag.afståelser.afståelsesdato.år",
                "År for virksomhedsafståelsen",
                "I hvilket år",
            ),
            (
                "lønmodtager.pension.pbl15a_årsgrundlag.afståelser.passiv_kapital.seneste_tre_regnskabsperioder.startdato.år",
                "Regnskabsperiodens startår",
                "begyndte regnskabsperioden",
            ),
            (
                "lønmodtager.pension.pbl15a_årsgrundlag.afståelser.passiv_kapital.seneste_tre_regnskabsperioder.selskabsregnskaber.selskab.ejerforhold.direkte_ejerandel_basispoint",
                "Direkte ejerandel i selskabet",
                "direkte kapitalandel",
            ),
            (
                "lønmodtager.pension.pbl15a_årsgrundlag.afståelser.passiv_kapital.seneste_tre_regnskabsperioder.selskabsregnskaber.selskab.ejerforhold.indirekte_ejerveje.identifikation",
                "Ejervejens identifikation",
                "entydig identifikation",
            ),
            (
                "lønmodtager.pension.pbl15a_årsgrundlag.afståelser.passiv_kapital.seneste_tre_regnskabsperioder.selskabsregnskaber.selskab.ejerforhold.indirekte_ejerveje.ejerandele_gennem_kæden_basispoint",
                "Ejerandel i hvert led",
                "dette led",
            ),
            (
                "lønmodtager.pension.pbl15a_årsgrundlag.afståelser.passiv_kapital.selskabsaktiver_på_overdragelsestidspunktet.selskabets_aktiver_før_ejerandel.handelsværdi_kroner",
                "Selskabsaktivets fulde værdi ved overdragelsen",
                "fulde handelsværdi",
            ),
            (
                "lønmodtager.pension.pbl15a_årsgrundlag.ordninger.identifikation",
                "Ophørspensionsordning",
                "entydig identifikation",
            ),
            (
                "lønmodtager.pension.pbl15a_årsgrundlag.ordninger.oprettelsesafståelse_identifikation",
                "Afståelse, der oprettede ophørspensionen",
                "Hvilken virksomheds- eller aktieafståelse",
            ),
            (
                "lønmodtager.pension.pbl15a_årsgrundlag.kvalifikationsår.indkomstår",
                "Kvalifikationsår",
                "Hvilket indkomstår",
            ),
            (
                "lønmodtager.pension.pbl15a_årsgrundlag.tidligere_indbetalinger.beløb_kroner",
                "Tidligere indbetalt beløb",
                "Hvor stort et beløb",
            ),
            (
                "lønmodtager.pension.pbl15b_årsgrundlag.indkomstposter.beløb_kroner",
                "Indkomst fra aktiviteten",
                "Hvor stor var indkomsten",
            ),
            (
                "lønmodtager.pension.pbl15b_årsgrundlag.ordninger.identifikation",
                "Sportspensionsordningens identifikation",
                "Hvilken entydig identifikation",
            ),
            (
                "lønmodtager.pension.pbl15b_årsgrundlag.ordninger.udbetalingsplan.$variant",
                "Plan for tidlig udbetaling fra sportspensionen",
                "ingen tidlig udbetalingsplan",
            ),
            (
                "lønmodtager.pension.pbl15b_årsgrundlag.rateudbetalinger.udbetalt_kroner",
                "Udbetalt tidlig rate",
                "Hvor stort et beløb",
            ),
            (
                "lønmodtager.pension.øvrige_pbl20_årsgrundlag.udbetalinger.bruttoudbetaling_kroner",
                "Pensionsudbetaling før fritagelser",
                "samlede pensionsudbetaling",
            ),
            (
                "lønmodtager.pension.pbl15b_årsgrundlag.tidligere_indbetalinger.arbejdsmarkedsbidrag_kroner",
                "Arbejdsmarkedsbidrag i tidligere indbetaling",
                "Hvor meget af den tidligere indbetaling",
            ),
        ] {
            let row = metadata
                .rows()
                .skip(1)
                .find(|row| {
                    row.get(input_path_column)
                        .map(ToString::to_string)
                        .as_deref()
                        == Some(path)
                })
                .unwrap_or_else(|| panic!("missing human field metadata for {path}"));
            assert_eq!(
                row.get(label_column).map(ToString::to_string).as_deref(),
                Some(expected_label)
            );
            assert!(
                row.get(question_column)
                    .map(ToString::to_string)
                    .unwrap_or_default()
                    .contains(expected_question_fragment),
                "missing human interview question for {path}"
            );
        }
        for (path, expected_title) in [
            (
                "lønmodtager.pension.pbl15a_årsgrundlag.afståelser.fortjenestegrundlag.afskrivningslovsposter",
                "Dansk personskat - Afskrivningslovens afståelser",
            ),
            (
                "lønmodtager.pension.pbl15a_årsgrundlag.afståelser.fortjenestegrundlag.kursgevinstposter",
                "Dansk personskat - Kursgevinster og kurstab ved afståelsen",
            ),
        ] {
            let sheet = workbook_collection_sheet_name(&mut workbook, path);
            assert_eq!(workbook_title(&mut workbook, &sheet), expected_title);
        }
        for path in [
            "lønmodtager.pension.pbl15a_årsgrundlag.afståelser.grundlag.$variant",
            "lønmodtager.pension.pbl15a_årsgrundlag.afståelser.fortjenestegrundlag.ejendomsavance.$variant",
            "lønmodtager.pension.pbl15a_årsgrundlag.afståelser.fortjenestegrundlag.aktieavance.$variant",
            "lønmodtager.pension.pbl15a_årsgrundlag.afståelser.fortjenestegrundlag.afskrivningslovsposter.identifikation",
            "lønmodtager.pension.pbl15a_årsgrundlag.afståelser.fortjenestegrundlag.afskrivningslovsposter.kilde.$variant",
            "lønmodtager.pension.pbl15a_årsgrundlag.afståelser.fortjenestegrundlag.kursgevinstposter.identifikation",
            "lønmodtager.pension.pbl15a_årsgrundlag.afståelser.fortjenestegrundlag.kursgevinstposter.input.rolle",
        ] {
            let row = metadata
                .rows()
                .skip(1)
                .find(|row| {
                    row.get(input_path_column)
                        .map(ToString::to_string)
                        .as_deref()
                        == Some(path)
                })
                .unwrap_or_else(|| panic!("missing explicit § 15 A source-boundary metadata for {path}"));
            assert!(
                row.get(label_column)
                    .map(ToString::to_string)
                    .is_some_and(|label| !label.trim().is_empty()),
                "missing explicit human label for § 15 A input {path}"
            );
            assert!(
                row.get(question_column)
                    .map(ToString::to_string)
                    .is_some_and(|question| !question.trim().is_empty()),
                "§ 15 A source-boundary input {path} lacks an explicit interview question"
            );
        }
        let pbl15a_period_start_row = metadata
            .rows()
            .skip(1)
            .find(|row| {
                row.get(input_path_column)
                    .map(ToString::to_string)
                    .as_deref()
                    == Some("lønmodtager.pension.pbl15a_årsgrundlag.afståelser.passiv_kapital.seneste_tre_regnskabsperioder.startdato.år")
            })
            .expect("§ 15 A accounting-period metadata");
        let pbl15a_period_sources = pbl15a_period_start_row
            .get(sources_column)
            .map(ToString::to_string)
            .expect("§ 15 A accounting-period sources");
        for expected in [
            "pensionsbeskatningsloven_lbk1243_par15a",
            "skat_pbl15a_betingelser_vejledning",
        ] {
            assert!(
                pbl15a_period_sources.contains(expected),
                "missing § 15 A accounting-period source {expected}"
            );
        }
        let spouse_source_row = metadata
            .rows()
            .skip(1)
            .find(|row| {
                row.get(input_path_column)
                    .map(ToString::to_string)
                    .as_deref()
                    == Some("ægtefælle.$variant")
            })
            .expect("spouse source-boundary metadata");
        let spouse_sources = spouse_source_row
            .get(sources_column)
            .map(ToString::to_string)
            .expect("spouse sources");
        for expected in [
            "personskatteloven_2021_1284",
            "personskat_personfradrag_aendring_2023_1564",
        ] {
            assert!(
                spouse_sources.contains(expected),
                "missing spouse-transfer source {expected}"
            );
        }
        let wage_source_row = metadata
            .rows()
            .skip(1)
            .find(|row| {
                row.get(input_path_column)
                    .map(ToString::to_string)
                    .as_deref()
                    == Some("lønmodtager.bruttoløn_kroner")
            })
            .expect("wage source-boundary metadata");
        assert!(
            !wage_source_row
                .get(sources_column)
                .map(ToString::to_string)
                .unwrap_or_default()
                .contains("personskat_personfradrag_aendring_2023_1564"),
            "person-fradrag amendment leaked onto unrelated wage metadata"
        );
        let business_travel_row = metadata
            .rows()
            .skip(1)
            .find(|row| {
                row.get(input_path_column)
                    .map(ToString::to_string)
                    .as_deref()
                    == Some("lønmodtager.erhvervsbefordring.sager.godtgørelsesforhold.udbetalt_godtgørelse_kroner")
            })
            .expect("business-travel reimbursement metadata");
        assert_eq!(
            business_travel_row
                .get(label_column)
                .map(ToString::to_string)
                .as_deref(),
            Some("Udbetalt kørselsgodtgørelse")
        );
        assert!(business_travel_row
            .get(question_column)
            .map(ToString::to_string)
            .expect("business-travel interview question")
            .contains("Hvor meget kørselsgodtgørelse"));
        let business_travel_sources = business_travel_row
            .get(sources_column)
            .map(ToString::to_string)
            .expect("business-travel sources");
        for expected in [
            "ligningsloven_lbk1500_par9b",
            "skatteraadet_bek1333_par4_par14",
            "ligningsloven_juridisk_vejledning_par9b_godtgørelse",
        ] {
            assert!(
                business_travel_sources.contains(expected),
                "missing business-travel source {expected}"
            );
        }
        let establishment_account_row = metadata
            .rows()
            .skip(1)
            .find(|row| {
                row.get(input_path_column)
                    .map(ToString::to_string)
                    .as_deref()
                    == Some("lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.faktisk_iværksætterkontoindskud_kroner")
            })
            .expect("entrepreneur-account metadata");
        assert_eq!(
            establishment_account_row
                .get(label_column)
                .map(ToString::to_string)
                .as_deref(),
            Some("Faktisk indskud på iværksætterkonto")
        );
        let establishment_account_sources = establishment_account_row
            .get(sources_column)
            .map(ToString::to_string)
            .expect("entrepreneur-account sources");
        for expected in [
            "etableringskontoloven_lbk1307_par1_par4",
            "personskatteloven_lbk1284_par3",
        ] {
            assert!(
                establishment_account_sources.contains(expected),
                "missing entrepreneur-account source {expected}"
            );
        }
        let pension_history_row = metadata
            .rows()
            .skip(1)
            .find(|row| {
                row.get(input_path_column)
                    .map(ToString::to_string)
                    .as_deref()
                    == Some("lønmodtager.pension.pbl15a_årsgrundlag.tidligere_indbetalinger.beløb_kroner")
            })
            .expect("§ 15 A pension-history metadata");
        let pension_history_sources = pension_history_row
            .get(sources_column)
            .map(ToString::to_string)
            .expect("§ 15 A pension-history sources");
        for expected in [
            "pensionsbeskatningsloven_lbk1243_par15a",
            "skatteministeriet_pbl15a_beløbsgrænser_2026",
            "skat_pbl15a_ophørspension_vejledning",
            "skat_pbl15a_fradragstidspunkt_vejledning",
            "skat_pbl15a_flere_afståelser_vejledning",
        ] {
            assert!(
                pension_history_sources.contains(expected),
                "missing § 15 A pension source {expected}"
            );
        }
        let sportspension_history_row = metadata
            .rows()
            .skip(1)
            .find(|row| {
                row.get(input_path_column)
                    .map(ToString::to_string)
                    .as_deref()
                    == Some("lønmodtager.pension.pbl15b_årsgrundlag.tidligere_indbetalinger.arbejdsmarkedsbidrag_kroner")
            })
            .expect("§ 15 B sportspension-history metadata");
        let sportspension_history_sources = sportspension_history_row
            .get(sources_column)
            .map(ToString::to_string)
            .expect("§ 15 B sportspension-history sources");
        for expected in [
            "pensionsbeskatningsloven_lbk1243_par15b",
            "skatteministeriet_pbl15b_satser_2010_til_2017",
            "skatteministeriet_pbl15b_satser_2018_til_2024",
            "skatteministeriet_pbl15b_satser_2025_til_2026",
        ] {
            assert!(
                sportspension_history_sources.contains(expected),
                "missing § 15 B sportspension source {expected}"
            );
        }
        let pbl53a_row = metadata
            .rows()
            .skip(1)
            .find(|row| {
                row.get(input_path_column)
                    .map(ToString::to_string)
                    .as_deref()
                    == Some("kapitalindkomst.pbl53a.ordninger.identifikation")
            })
            .expect("PBL § 53 A metadata");
        let pbl53a_sources = pbl53a_row
            .get(sources_column)
            .map(ToString::to_string)
            .expect("PBL § 53 A sources");
        for expected in [
            "pensionsbeskatningsloven_lbk1243_par53a",
            "pensionsbeskatningsloven_lov569_par1_nr11_og_par6_stk1",
            "aftaleloven_lbk193_par6",
            "pensionsbeskatningsloven_lsf229_1992_par6",
            "pensionsbeskatningsloven_historisk_lbk1120_par53a_stk3",
            "pensionsbeskatningsloven_lov313_par9_nr3_og_par19_stk3",
            "pensionsbeskatningsloven_lov1534_par1_nr37_og_par11_stk7",
            "pensionsbeskatningsloven_lbk1243_par53b",
            "pensionsbeskatningsloven_lbk1243_par20",
            "personskatteloven_lbk1284_par3",
            "personskatteloven_lbk1284_par4",
            "arbejdsmarkedsbidragsloven_lbk121_par2",
            "skat_juridisk_vejledning_pbl53a_indbetalinger",
            "skat_juridisk_vejledning_pbl53a_udbetalinger",
            "skat_juridisk_vejledning_pbl53a_afkast",
            "skat_juridisk_vejledning_pbl53a_overgang",
            "skat_juridisk_vejledning_pbl53a_overgangsvalg",
            "skat_juridisk_vejledning_pbl53a_ordningstyper",
            "skat_juridisk_vejledning_pbl53b_omfang",
            "skat_skm2025_658_lsr_pbl53a_blanketvalg",
            "skat_skm2013_481_sr_pbl53a_produktændring",
            "skat_skm2023_406_lsr_pbl53a_overflytning",
            "pensionsbeskatningsloven_lsf24_2007_par53a_stk5",
        ] {
            assert!(
                pbl53a_sources.contains(expected),
                "missing PBL § 53 A source {expected}"
            );
        }
        let table_metadata = workbook.worksheet_range("_tables").expect("table metadata");
        let table_metadata_headers = table_metadata
            .rows()
            .next()
            .expect("table metadata headers");
        let table_path_column = table_metadata_headers
            .iter()
            .position(|cell| cell.to_string() == "path")
            .expect("table path metadata column");
        let table_sources_column = table_metadata_headers
            .iter()
            .position(|cell| cell.to_string() == "sources")
            .expect("table sources metadata column");
        let kgl_par32_source_row = table_metadata
            .rows()
            .skip(1)
            .find(|row| {
                row.get(table_path_column)
                    .map(ToString::to_string)
                    .as_deref()
                    == Some(kgl_par32_current_contracts_path)
            })
            .expect("KGL §32 source metadata");
        let kgl_par32_sources = kgl_par32_source_row
            .get(table_sources_column)
            .map(ToString::to_string)
            .expect("KGL §32 sources");
        for expected in [
            "kursgevinstloven_lbk1176_par32",
            "kursgevinstloven_juridisk_vejledning_par32",
            "kursgevinstloven_lov1563_par4_par8",
            "aktieavancebeskatningsloven_lbk1098_par17_til_par22",
        ] {
            assert!(
                kgl_par32_sources.contains(expected),
                "missing KGL §32 source {expected}"
            );
        }
        let par37_to_40_source_row = table_metadata
            .rows()
            .skip(1)
            .find(|row| {
                row.get(table_path_column)
                    .map(ToString::to_string)
                    .as_deref()
                    == Some("aktieavance.særlige_aktiver.kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.aktier")
            })
            .expect("ABL §§ 37-40 source metadata");
        let par37_to_40_sources = par37_to_40_source_row
            .get(table_sources_column)
            .map(ToString::to_string)
            .expect("ABL §§ 37-40 sources");
        for expected in [
            "aktieavancebeskatningsloven_lbk1098_par37",
            "aktieavancebeskatningsloven_lbk1098_par38",
            "aktieavancebeskatningsloven_lbk1098_par39",
            "aktieavancebeskatningsloven_lbk1098_par39a",
            "aktieavancebeskatningsloven_lbk1098_par39b",
            "aktieavancebeskatningsloven_lbk1098_par40",
            "aktieavancebeskatningsloven_juridisk_vejledning_par37",
            "aktieavancebeskatningsloven_juridisk_vejledning_par38_personkreds",
            "aktieavancebeskatningsloven_juridisk_vejledning_par38_opgoerelse",
            "aktieavancebeskatningsloven_juridisk_vejledning_par39",
            "aktieavancebeskatningsloven_juridisk_vejledning_par39a_beholdningsoversigt",
            "aktieavancebeskatningsloven_juridisk_vejledning_par39a_henstandssaldo",
            "aktieavancebeskatningsloven_juridisk_vejledning_par39a_prioritet",
            "aktieavancebeskatningsloven_juridisk_vejledning_par39a_alle_afstået",
            "aktieavancebeskatningsloven_juridisk_vejledning_par39a_årsoplysninger",
            "aktieavancebeskatningsloven_juridisk_vejledning_par39b",
        ] {
            assert!(
                par37_to_40_sources.contains(expected),
                "missing ABL §§ 37-40 source {expected}"
            );
        }
        let par5a_source_row = metadata
            .rows()
            .skip(1)
            .find(|row| {
                row.get(input_path_column)
                    .map(ToString::to_string)
                    .as_deref()
                    == Some("aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb.hændelser.AblOrdinærAfståelse.par5a_kildefakta.$variant")
            })
            .expect("ABL § 5 A source metadata");
        let par5a_sources = par5a_source_row
            .get(sources_column)
            .map(ToString::to_string)
            .expect("ABL § 5 A sources");
        assert!(
            par5a_sources.contains("aktieavancebeskatningsloven_lbk1098_par5a"),
            "missing official ABL § 5 A source"
        );
        let dividend_row = metadata
            .rows()
            .skip(1)
            .find(|row| {
                row.get(input_path_column)
                    .map(ToString::to_string)
                    .as_deref()
                    == Some("aktieavance.udbytter.beløb_kroner")
            })
            .expect("dividend source metadata");
        let dividend_sources = dividend_row
            .get(sources_column)
            .map(ToString::to_string)
            .expect("dividend sources");
        for expected in [
            "ligningsloven_par16a_lbk1500",
            "ligningsloven_par16a_lov1755",
        ] {
            assert!(
                dividend_sources.contains(expected),
                "missing dividend source {expected}"
            );
        }
        let property_income_row = metadata
            .rows()
            .skip(1)
            .find(|row| {
                row.get(2).map(ToString::to_string).as_deref()
                    == Some("kapitalindkomst.ejendomsdrift.$variant")
            })
            .expect("property-income metadata");
        let property_income_sources = property_income_row
            .get(sources_column)
            .map(ToString::to_string)
            .expect("property-income sources");
        for expected in [
            "personskatteloven_lov679_par4_nr6",
            "personskatteloven_lov615_par4_nr6",
            "ejendomsskatteloven_lov678_par3",
            "ejendomsskatteloven_lov615_par3_aendring",
        ] {
            assert!(
                property_income_sources.contains(expected),
                "missing property-income source {expected}"
            );
        }
    }

    edit_workbook(&input_path, |sheets| {
        let fill_wage_case = |sheets: &mut [(String, Vec<Vec<Data>>)],
                              row: usize,
                              case_id: &str| {
            for (header, value) in [
                    ("case_id", Data::String(case_id.to_string())),
                    ("lønmodtager.skatteår", Data::String("2026".to_string())),
                    (
                        "lønmodtager.kommune",
                        Data::String("København".to_string()),
                    ),
                    (
                        "lønmodtager.bruttoløn_kroner",
                        Data::String("600000".to_string()),
                    ),
                    (
                        "lønmodtager.personlig_indkomst.etableringskonto.$variant",
                        Data::String("UdenEtableringskontoindskud".to_string()),
                    ),
                    (
                        "lønmodtager.ligningsfradrag.befordring.$variant",
                        Data::String("UdenBefordringsfradrag".to_string()),
                    ),
                    (
                        "lønmodtager.pension.pensionsalder_status",
                        Data::String("Ll9lMereEnd15ÅrFørFolkepension".to_string()),
                    ),
                    (
                        "lønmodtager.pension.pbl18_selvstændig_overskud.skattepligtigt_overskud_før_vsl22b_kroner",
                        Data::String("0".to_string()),
                    ),
                    (
                        "lønmodtager.pension.pbl18_selvstændig_overskud.renteudgifter_kroner",
                        Data::String("0".to_string()),
                    ),
                    (
                        "lønmodtager.pension.pbl18_selvstændig_overskud.kurstab_kroner",
                        Data::String("0".to_string()),
                    ),
                    (
                        "lønmodtager.pension.pbl18_selvstændig_overskud.renteindtægter_kroner",
                        Data::String("0".to_string()),
                    ),
                    (
                        "lønmodtager.pension.pbl18_selvstændig_overskud.udbytteindtægter_kroner",
                        Data::String("0".to_string()),
                    ),
                    (
                        "lønmodtager.pension.pbl18_selvstændig_overskud.kursgevinster_kroner",
                        Data::String("0".to_string()),
                    ),
                    (
                        "lønmodtager.pension.pbl18_selvstændig_overskud.udelukkede_afståelsesindkomster_kroner",
                        Data::String("0".to_string()),
                    ),
                    (
                        "lønmodtager.pension.pbl18_livrentevalg.$variant",
                        Data::String("Pbl18FordeltFradrag".to_string()),
                    ),
                    (
                        "lønmodtager.pension.aktiepensionsfradrag_valg.$variant",
                        Data::String("UdenAktiepensionsfradragIAktieindkomst".to_string()),
                    ),
                    (
                        "lønmodtager.personfradrag_alder_status",
                        Data::String("Fyldt18EllerGift".to_string()),
                    ),
                    ("lønmodtager.betaler_kirkeskat", Data::Bool(false)),
                    (
                        "kapitalindkomst.renter.renteindtægter_kroner",
                        Data::String("0".to_string()),
                    ),
                    (
                        "kapitalindkomst.renter.renteudgifter_kroner",
                        Data::String("0".to_string()),
                    ),
                    (
                        "kapitalindkomst.renter.næringsstatus",
                        Data::String("IkkeNæring".to_string()),
                    ),
                    (
                        "kapitalindkomst.renter.ligningslov6.$variant",
                        Data::String("UdenLigningslov6Kurstab".to_string()),
                    ),
                    (
                        "kapitalindkomst.renter.ligningslov6a.$variant",
                        Data::String("UdenLigningslov6AFradrag".to_string()),
                    ),
                    (
                        "kapitalindkomst.virksomhedskapital.selvstændig_beskatningsordning.$variant",
                        Data::String(
                            "UdenVirksomhedsEllerKapitalafkastordning".to_string(),
                        ),
                    ),
                    (
                        "kapitalindkomst.virksomhedskapital.medarbejderaktier.$variant",
                        Data::String(
                            "UdenKapitalafkastEfterVirksomhedsskattelov22C".to_string(),
                        ),
                    ),
                    (
                        "kapitalindkomst.ejendomsdrift.$variant",
                        Data::String("UdenEjendomsdriftEfterPar4Nr6".to_string()),
                    ),
                    (
                        "kapitalindkomst.ejendomsavance.$variant",
                        Data::String("UdenEjendomsavance".to_string()),
                    ),
                    (
                        "kapitalindkomst.kursgevinst.$variant",
                        Data::String("UdenKursgevinst".to_string()),
                    ),
                    (
                        "kapitalindkomst.fremleje.$variant",
                        Data::String("UdenFremlejeEfterLigningslov15Q".to_string()),
                    ),
                    (
                        "skatteforhold.$variant",
                        Data::String("StandardSkatteforhold".to_string()),
                    ),
                    (
                        "aktieavance.ordinært_aktieår.$variant",
                        Data::String("UdenOrdinærtAktieår".to_string()),
                    ),
                    (
                        "udenlandske_sociale_bidrag.$variant",
                        Data::String(
                            "UdenUdenlandskeSocialeBidragEfterLigningslov8M".to_string(),
                        ),
                    ),
                    (
                        "underskudsforhold.$variant",
                        Data::String("StandardUnderskudsforhold".to_string()),
                    ),
                    (
                        "ægtefælle.$variant",
                        Data::String("UdenÆgtefælle".to_string()),
                    ),
                    (
                        "ejendomsskatter.person.ejer_folkepensionsalder.$variant",
                        Data::String("EjskFolkepensionsalderIkkeOpnået".to_string()),
                    ),
                    (
                        "ejendomsskatter.person.samlevende_ægtefælles_folkepensionsalder.$variant",
                        Data::String("EjskFolkepensionsalderIkkeOpnået".to_string()),
                    ),
                    (
                        "ejendomsskatter.person.skattemæssigt_hjemsted.$variant",
                        Data::String(
                            "EjskFuldtSkattepligtigEfterKildeskattelovensPar1".to_string(),
                        ),
                    ),
                    (
                        "ejendomsskatter.person.egen_udbytteindkomst_kroner",
                        Data::String("0".to_string()),
                    ),
                    (
                        "ejendomsskatter.person.ægtefælles_udbytteindkomst_kroner",
                        Data::String("0".to_string()),
                    ),
                    (
                        "årsopgørelse.$variant",
                        Data::String("UdenÅrsopgørelse".to_string()),
                    ),
                ] {
                    set_workbook_cell_by_header(sheets, "cases", row, header, value);
                }
        };

        fill_wage_case(sheets, 1, "personskat-årsopgørelse-2026");
        fill_wage_case(sheets, 2, "personskat-abl-personlig-2026");
        fill_wage_case(sheets, 3, "personskat-renter-befordring-2026");
        fill_wage_case(sheets, 4, "personskat-ebl5-kildefakta-2026");
        fill_wage_case(sheets, 5, "personskat-ebl6d-historisk-2026");
        fill_wage_case(sheets, 6, "personskat-ebl11-genanbringelse-2026");
        fill_wage_case(sheets, 7, "personskat-fremleje-2026");
        fill_wage_case(sheets, 8, "personskat-ejendomsdrift-2026");
        fill_wage_case(sheets, 9, "personskat-erhvervsbefordring-2026");
        fill_wage_case(sheets, 10, "personskat-pbl53a-2026");
        fill_wage_case(sheets, 11, "personskat-pbl53a-overdrager-2026");
        fill_wage_case(sheets, 12, "personskat-aegtefaelleoverfoersler-2025");
        fill_wage_case(sheets, 13, "personskat-ejendomsskatter-2025");
        fill_wage_case(sheets, 14, "personskat-kgl-gaeld-2026");
        fill_wage_case(sheets, 15, "personskat-kgl-frivillig-ordning-2026");
        fill_wage_case(sheets, 16, "personskat-udbytte-2026");
        fill_wage_case(sheets, 17, "personskat-etableringskonto-2026");
        fill_wage_case(sheets, 18, "personskat-par35-medarbejdereje-2026");
        fill_wage_case(sheets, 19, "personskat-par37-40-fraflytning-2026");
        fill_wage_case(sheets, 20, "personskat-kgl-par32-historik-2026");
        fill_wage_case(sheets, 21, "personskat-kgl-par32-abl17-2026");
        fill_wage_case(sheets, 22, "personskat-par37-40-aegtefaelle-2026");
        fill_wage_case(sheets, 23, "personskat-par37-40-modstridende-kontekst-2026");
        let fill_par32_case = |sheets: &mut [(String, Vec<Vec<Data>>)],
                               row: usize,
                               skatteyder_identifikation: &str| {
            for (header, value) in [
                (
                    "kapitalindkomst.kursgevinst.$variant",
                    Data::String("MedKursgevinst".to_string()),
                ),
                (
                    "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.skatteyder_identifikation",
                    Data::String(skatteyder_identifikation.to_string()),
                ),
                (
                    "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.par32_kontraktforløb.$variant",
                    Data::String("MedPar32Kontraktforløb".to_string()),
                ),
                (
                    "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.par32_kontraktforløb.MedPar32Kontraktforløb.aktuelt_år.indkomstår",
                    Data::Int(2026),
                ),
                (
                    "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.par32_kontraktforløb.MedPar32Kontraktforløb.aktuelt_år.valg.tabsprioritet",
                    Data::String("KglPar32AlmindeligeTabFørst".to_string()),
                ),
                (
                    "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.par32_kontraktforløb.MedPar32Kontraktforløb.aktuelt_år.valg.fast_ejendomstabsprioritet",
                    Data::String("KglPar32SælgertabFørst".to_string()),
                ),
                (
                    "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.par32_kontraktforløb.MedPar32Kontraktforløb.aktuelt_år.valg.aktiemodregningsvalg.omfang",
                    Data::String("KglPar32IngenAktiemodregning".to_string()),
                ),
                (
                    "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.par32_kontraktforløb.MedPar32Kontraktforløb.aktuelt_år.valg.aktiemodregningsvalg.beløb.$variant",
                    Data::String("KglPar32MaksimalAktiemodregning".to_string()),
                ),
            ] {
                set_workbook_cell_by_header(sheets, "cases", row, header, value);
            }
        };
        fill_par32_case(sheets, 20, "par32-historik-person");
        fill_par32_case(sheets, 21, "par32-abl17-person");

        let par32_current_contracts_path = "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.par32_kontraktforløb.MedPar32Kontraktforløb.aktuelt_år.kontrakter";
        let par32_current_contracts_sheet =
            workbook_collection_sheet_name_from_rows(sheets, par32_current_contracts_path);
        let fill_par32_contract = |sheets: &mut [(String, Vec<Vec<Data>>)],
                                   sheet: &str,
                                   row: usize,
                                   case_id: &str,
                                   parent_id: Option<&str>,
                                   contract_id: &str,
                                   position: i64,
                                   anskaffelsessum_kroner: i64,
                                   afståelsesværdi_kroner: i64,
                                   udøver_næring: bool,
                                   relationsvariant: &str,
                                   abl_reference: Option<&str>,
                                   underliggende_variant: &str| {
            for (header, value) in [
                ("case_id", Data::String(case_id.to_string())),
                ("item_id", Data::String(contract_id.to_string())),
                ("position", Data::Int(position)),
                ("identifikation", Data::String(contract_id.to_string())),
                ("rækkefølge_i_indkomståret", Data::Int(position)),
                (
                    "kontrakt.aftaleart.$variant",
                    Data::String("KglPar30IngenUndtagelsesaftale".to_string()),
                ),
                (
                    "kontrakt.har_modgående_kontrakt_eller_forretning",
                    Data::Bool(false),
                ),
                ("kontrakt.værdi_primo_kroner", Data::Int(0)),
                ("kontrakt.værdi_ultimo_kroner", Data::Int(0)),
                (
                    "kontrakt.anskaffelsessum_kroner",
                    Data::Int(anskaffelsessum_kroner),
                ),
                (
                    "kontrakt.afståelsesværdi_kroner",
                    Data::Int(afståelsesværdi_kroner),
                ),
                ("kontrakt.anskaffet_i_indkomståret", Data::Bool(true)),
                ("kontrakt.realiseret_i_indkomståret", Data::Bool(true)),
                (
                    "kontrakt.udøver_næring_ved_køb_og_salg_af_finansielle_kontrakter",
                    Data::Bool(udøver_næring),
                ),
                (
                    "relationsfakta.$variant",
                    Data::String(relationsvariant.to_string()),
                ),
                (
                    "underliggende.$variant",
                    Data::String(underliggende_variant.to_string()),
                ),
            ] {
                set_workbook_cell_by_header(sheets, sheet, row, header, value);
            }
            if let Some(parent_id) = parent_id {
                set_workbook_cell_by_header(
                    sheets,
                    sheet,
                    row,
                    "parent_id",
                    Data::String(parent_id.to_string()),
                );
            }
            if let Some(abl_reference) = abl_reference {
                set_workbook_cell_by_header(
                    sheets,
                    sheet,
                    row,
                    "relationsfakta.KglPar32KildeTilknyttetAblAktiv.aktieaktiv_identifikation",
                    Data::String(abl_reference.to_string()),
                );
            }
        };
        fill_par32_contract(
            sheets,
            &par32_current_contracts_sheet,
            1,
            "personskat-kgl-par32-historik-2026",
            None,
            "par32-gevinst-a-2026",
            1,
            10_000,
            14_000,
            false,
            "KglPar32KildeUdenSærligRelation",
            None,
            "KglPar32KildeIkkeAktiebaseret",
        );
        fill_par32_contract(
            sheets,
            &par32_current_contracts_sheet,
            2,
            "personskat-kgl-par32-historik-2026",
            None,
            "par32-gevinst-b-2026",
            2,
            5_000,
            7_000,
            false,
            "KglPar32KildeUdenSærligRelation",
            None,
            "KglPar32KildeIkkeAktiebaseret",
        );
        fill_par32_contract(
            sheets,
            &par32_current_contracts_sheet,
            3,
            "personskat-kgl-par32-abl17-2026",
            None,
            "par32-abl17-tab-2026",
            1,
            20_000,
            11_000,
            true,
            "KglPar32KildeTilknyttetAblAktiv",
            Some("par32-abl17-aktiv"),
            "KglPar32KildeEnkeltaktieFraAblReference",
        );

        let par32_history_path = "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.par32_kontraktforløb.MedPar32Kontraktforløb.tidligere_år";
        let par32_history_sheet =
            workbook_collection_sheet_name_from_rows(sheets, par32_history_path);
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-kgl-par32-historik-2026".to_string()),
            ),
            ("item_id", Data::String("par32-historik-2025".to_string())),
            ("position", Data::Int(1)),
            ("fakta.indkomstår", Data::Int(2025)),
            (
                "fakta.valg.tabsprioritet",
                Data::String("KglPar32AlmindeligeTabFørst".to_string()),
            ),
            (
                "fakta.valg.fast_ejendomstabsprioritet",
                Data::String("KglPar32SælgertabFørst".to_string()),
            ),
            (
                "fakta.valg.aktiemodregningsvalg.omfang",
                Data::String("KglPar32IngenAktiemodregning".to_string()),
            ),
            (
                "fakta.valg.aktiemodregningsvalg.beløb.$variant",
                Data::String("KglPar32MaksimalAktiemodregning".to_string()),
            ),
            (
                "aktieavance.ordinært_aktieår.$variant",
                Data::String("UdenOrdinærtAktieår".to_string()),
            ),
            (
                "gift_og_samlevende_ved_indkomstårets_udgang",
                Data::Bool(false),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &par32_history_sheet, 1, header, value);
        }
        let par32_history_contracts_path = format!("{par32_history_path}.fakta.kontrakter");
        let par32_history_contracts_sheet =
            workbook_collection_sheet_name_from_rows(sheets, &par32_history_contracts_path);
        fill_par32_contract(
            sheets,
            &par32_history_contracts_sheet,
            1,
            "personskat-kgl-par32-historik-2026",
            Some("par32-historik-2025"),
            "par32-tab-2025",
            1,
            20_000,
            10_000,
            false,
            "KglPar32KildeUdenSærligRelation",
            None,
            "KglPar32KildeIkkeAktiebaseret",
        );

        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-kgl-par32-abl17-2026".to_string()),
            ),
            ("item_id", Data::String("par32-abl17-aktiv".to_string())),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String("par32-abl17-aktiv".to_string()),
            ),
            (
                "kilde.$variant",
                Data::String("PersonskatAktieaktivEfterPar17".to_string()),
            ),
            (
                "kilde.PersonskatAktieaktivEfterPar17.fakta.indkomstår",
                Data::Int(2026),
            ),
            (
                "kilde.PersonskatAktieaktivEfterPar17.fakta.skattepligtsgrundlag",
                Data::String("AblPar7PersonEfterKildeskatteloven".to_string()),
            ),
            (
                "kilde.PersonskatAktieaktivEfterPar17.fakta.næringsstatus",
                Data::String("AblPar17UdøverNæringVedKøbOgSalgAfAktier".to_string()),
            ),
            (
                "kilde.PersonskatAktieaktivEfterPar17.fakta.instrument",
                Data::String("AblPar17AlmindeligAktie".to_string()),
            ),
            (
                "kilde.PersonskatAktieaktivEfterPar17.fakta.erhvervelsesstatus",
                Data::String("AblPar17ErhvervetSomLedINæringsvej".to_string()),
            ),
            (
                "kilde.PersonskatAktieaktivEfterPar17.fakta.afståelsessum_kroner",
                Data::Int(35_000),
            ),
            (
                "kilde.PersonskatAktieaktivEfterPar17.fakta.anskaffelsessum_kroner",
                Data::Int(30_000),
            ),
            (
                "markedsstatus",
                Data::String("AblOptagetTilHandelPåReguleretMarked".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "aktieavance_særlige_aktiver", 4, header, value);
        }
        for (header, value) in [
            (
                "lønmodtager.personlig_indkomst.etableringskonto.$variant",
                Data::String("MedEtableringskontoindskud".to_string()),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.indkomstår",
                Data::Int(2026),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.fase.$variant",
                Data::String("EtblFørEtablering".to_string()),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.fuldt_skattepligtig",
                Data::Bool(true),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.skattemæssigt_hjemmehørende_i_andet_land_efter_dbo",
                Data::Bool(false),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.senest_indkomståret_efter_folkepensionsalderen",
                Data::Bool(true),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.tidligere_indskud_fuldt_hævet",
                Data::Bool(true),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.indskud_foretaget_i_indskudsåret",
                Data::Bool(true),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.betingelser_for_undladt_indskud_efter_par4_stk2_opfyldt",
                Data::Bool(false),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.faktisk_indskudsplacering.placering",
                Data::String("EtblSærligIndlånskonto".to_string()),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.faktisk_indskudsplacering.pengeinstitut_omfattet_af_par4_stk1",
                Data::Bool(true),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.faktisk_indskudsplacering.etableringskonto_og_iværksætterkonto_ført_særskilt",
                Data::Bool(true),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.faktisk_indskudsplacering.konto_korrekt_betegnet",
                Data::Bool(true),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.faktisk_indskudsplacering.navn_adresse_og_personnummer_påført",
                Data::Bool(true),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.faktisk_indskudsplacering.kontantkonto_og_depot_i_samme_pengeinstitut",
                Data::Bool(true),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.kontant_løn_kroner",
                Data::Int(600_000),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.skatteværdi_af_frit_ophold_og_andre_goder_kroner",
                Data::Int(0),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.skattepligtige_arbejdsgivergodtgørelser_kroner",
                Data::Int(0),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.ligningslov9_til_9d_fradrag_kroner",
                Data::Int(0),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.skattepligtigt_virksomhedsoverskud_efter_vsl22b_fradrag_kroner",
                Data::Int(0),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.vsl22b_henlæggelse_kroner",
                Data::Int(0),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.renteudgifter_og_kurstab_kroner",
                Data::Int(0),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.rente_udbytte_og_kursgevinst_kroner",
                Data::Int(0),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.forskudsafskrivning_efter_al29_kroner",
                Data::Int(0),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.faktisk_etableringskontoindskud_kroner",
                Data::Int(0),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.faktisk_iværksætterkontoindskud_kroner",
                Data::Int(30_000),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.undladt_etableringskontoindskud_efter_par4_stk2_kroner",
                Data::Int(0),
            ),
            (
                "lønmodtager.personlig_indkomst.etableringskonto.MedEtableringskontoindskud.input.undladt_iværksætterkontoindskud_efter_par4_stk2_kroner",
                Data::Int(0),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "cases", 17, header, value);
        }
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-udbytte-2026".to_string()),
            ),
            ("item_id", Data::String("udbytte-1".to_string())),
            ("position", Data::Int(1)),
            (
                "Udlodningens identifikation",
                Data::String("udbytte-1".to_string()),
            ),
            (
                "Udlodderens retlige type",
                Data::String("Ll16AAlmindeligtSelskab".to_string()),
            ),
            (
                "Din retlige modtagerstatus",
                Data::String("Ll16AAktuelAktionær".to_string()),
            ),
            (
                "Aktiv bag udlodningen",
                Data::String("PersonskatAlmindeligAktie".to_string()),
            ),
            ("Modtaget udlodning", Data::Int(12_000)),
            (
                "Udbyttets grundlag efter ABL § 13 A",
                Data::String("AblPar13AUdbytteUdenForModregningsgrundlag".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "aktieavance_udbytter", 1, header, value);
        }
        for row in [14, 15] {
            for (header, value) in [
                (
                    "kapitalindkomst.kursgevinst.$variant",
                    Data::String("MedKursgevinst".to_string()),
                ),
                (
                    "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.skatteyder_identifikation",
                    Data::String("Borger".to_string()),
                ),
                (
                    "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.par32_kontraktforløb.$variant",
                    Data::String("UdenPar32Kontraktforløb".to_string()),
                ),
            ] {
                set_workbook_cell_by_header(sheets, "cases", row, header, value);
            }
        }
        for (header, value) in [
            ("lønmodtager.skatteår", Data::String("2025".to_string())),
            (
                "lønmodtager.kommune",
                Data::String("Frederiksberg".to_string()),
            ),
            (
                "aktieavance.ordinært_aktieår.$variant",
                Data::String("MedOrdinærtAktieår".to_string()),
            ),
            (
                "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.indkomstår",
                Data::Int(2025),
            ),
            (
                "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.fremført_tab_efter_par13a_kroner",
                Data::Int(0),
            ),
            (
                "ægtefælle.$variant",
                Data::String("MedÆgtefælle".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.skatteår",
                Data::String("2025".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.kommune",
                Data::String("Frederiksberg".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.bruttoløn_kroner",
                Data::String("0".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.personlig_indkomst.etableringskonto.$variant",
                Data::String("UdenEtableringskontoindskud".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.ligningsfradrag.befordring.$variant",
                Data::String("UdenBefordringsfradrag".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.pension.pensionsalder_status",
                Data::String("Ll9lMereEnd15ÅrFørFolkepension".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.pension.pbl18_selvstændig_overskud.skattepligtigt_overskud_før_vsl22b_kroner",
                Data::String("0".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.pension.pbl18_selvstændig_overskud.renteudgifter_kroner",
                Data::String("0".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.pension.pbl18_selvstændig_overskud.kurstab_kroner",
                Data::String("0".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.pension.pbl18_selvstændig_overskud.renteindtægter_kroner",
                Data::String("0".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.pension.pbl18_selvstændig_overskud.udbytteindtægter_kroner",
                Data::String("0".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.pension.pbl18_selvstændig_overskud.kursgevinster_kroner",
                Data::String("0".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.pension.pbl18_selvstændig_overskud.udelukkede_afståelsesindkomster_kroner",
                Data::String("0".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.pension.pbl18_livrentevalg.$variant",
                Data::String("Pbl18FordeltFradrag".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.pension.aktiepensionsfradrag_valg.$variant",
                Data::String("UdenAktiepensionsfradragIAktieindkomst".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.personfradrag_alder_status",
                Data::String("Fyldt18EllerGift".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.betaler_kirkeskat",
                Data::Bool(false),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.kapitalindkomst.renter.renteindtægter_kroner",
                Data::String("0".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.kapitalindkomst.renter.renteudgifter_kroner",
                Data::String("39617".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.kapitalindkomst.renter.næringsstatus",
                Data::String("IkkeNæring".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.kapitalindkomst.renter.ligningslov6.$variant",
                Data::String("UdenLigningslov6Kurstab".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.kapitalindkomst.renter.ligningslov6a.$variant",
                Data::String("UdenLigningslov6AFradrag".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.kapitalindkomst.virksomhedskapital.selvstændig_beskatningsordning.$variant",
                Data::String("UdenVirksomhedsEllerKapitalafkastordning".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.kapitalindkomst.virksomhedskapital.medarbejderaktier.$variant",
                Data::String("UdenKapitalafkastEfterVirksomhedsskattelov22C".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.kapitalindkomst.ejendomsdrift.$variant",
                Data::String("UdenEjendomsdriftEfterPar4Nr6".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.kapitalindkomst.ejendomsavance.$variant",
                Data::String("UdenEjendomsavance".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.kapitalindkomst.kursgevinst.$variant",
                Data::String("UdenKursgevinst".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.kapitalindkomst.fremleje.$variant",
                Data::String("UdenFremlejeEfterLigningslov15Q".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.aktieavance.ordinært_aktieår.$variant",
                Data::String("UdenOrdinærtAktieår".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.udenlandske_sociale_bidrag.$variant",
                Data::String("UdenUdenlandskeSocialeBidragEfterLigningslov8M".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.skatteforhold.$variant",
                Data::String("StandardSkatteforhold".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.underskudsforhold.$variant",
                Data::String("StandardUnderskudsforhold".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.samlevende_ved_indkomstårets_udløb",
                Data::Bool(true),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "cases", 12, header, value);
        }
        copy_workbook_data_row(sheets, "cases", 12, 22);
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-par37-40-aegtefaelle-2026".to_string()),
            ),
            ("lønmodtager.skatteår", Data::String("2026".to_string())),
            ("lønmodtager.kommune", Data::String("København".to_string())),
            (
                "aktieavance.ordinært_aktieår.$variant",
                Data::String("UdenOrdinærtAktieår".to_string()),
            ),
            (
                "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.indkomstår",
                Data::Empty,
            ),
            (
                "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.fremført_tab_efter_par13a_kroner",
                Data::Empty,
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.skatteår",
                Data::String("2026".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.kommune",
                Data::String("København".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.kapitalindkomst.renter.renteudgifter_kroner",
                Data::String("0".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "cases", 22, header, value);
        }
        let par13a_investment_certificates_path =
            "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.investeringsbeviser";
        let par13a_investment_certificates_sheet =
            workbook_collection_sheet_name_from_rows(sheets, par13a_investment_certificates_path);
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-aegtefaelleoverfoersler-2025".to_string()),
            ),
            ("item_id", Data::String("par13a-tab-bevis".to_string())),
            ("position", Data::Int(1)),
            ("indkomstår", Data::Int(2025)),
            (
                "art",
                Data::String("AblPar21AktiebaseretMinimumsbevis".to_string()),
            ),
            ("afståelsessum_kroner", Data::Int(50_000)),
            ("anskaffelsessum_kroner", Data::Int(100_000)),
            (
                "oplysningsstatus",
                Data::String("AblOplystRettidigt".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(
                sheets,
                &par13a_investment_certificates_sheet,
                1,
                header,
                value,
            );
        }
        let spouse_dividend_path = "ægtefælle.MedÆgtefælle.fakta.aktieavance.udbytter";
        let spouse_dividend_sheet =
            workbook_collection_sheet_name_from_rows(sheets, spouse_dividend_path);
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-aegtefaelleoverfoersler-2025".to_string()),
            ),
            (
                "item_id",
                Data::String("par13a-aegtefaelle-udbytte".to_string()),
            ),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String("par13a-aegtefaelle-udbytte".to_string()),
            ),
            (
                "udlodder",
                Data::String("Ll16AAlmindeligtSelskab".to_string()),
            ),
            (
                "modtager",
                Data::String("Ll16AAktuelAktionær".to_string()),
            ),
            (
                "aktiv.$variant",
                Data::String("PersonskatAlmindeligAktie".to_string()),
            ),
            ("beløb_kroner", Data::Int(12_000)),
            (
                "par13a_kildefakta.$variant",
                Data::String("AblPar13AUdbytteForMarkedsaktieEfterPar12".to_string()),
            ),
            (
                "par13a_kildefakta.AblPar13AUdbytteForMarkedsaktieEfterPar12.markedsstatus",
                Data::String("AblOptagetTilHandelPåReguleretMarked".to_string()),
            ),
            (
                "par13a_kildefakta.AblPar13AUdbytteForMarkedsaktieEfterPar12.aktivklassifikation.indkomstår",
                Data::Int(2025),
            ),
            (
                "par13a_kildefakta.AblPar13AUdbytteForMarkedsaktieEfterPar12.aktivklassifikation.aktiv",
                Data::String("AblOrdinærAktiePar12Til15".to_string()),
            ),
            (
                "par13a_kildefakta.AblPar13AUdbytteForMarkedsaktieEfterPar12.aktivklassifikation.par17_modprøve.næringsstatus",
                Data::String("AblPar17UdøverIkkeNæringVedKøbOgSalgAfAktier".to_string()),
            ),
            (
                "par13a_kildefakta.AblPar13AUdbytteForMarkedsaktieEfterPar12.aktivklassifikation.par17_modprøve.erhvervelsesstatus",
                Data::String("AblPar17IkkeErhvervetSomLedINæringsvej".to_string()),
            ),
            (
                "par13a_kildefakta.AblPar13AUdbytteForMarkedsaktieEfterPar12.aktivklassifikation.investeringsklassifikation.$variant",
                Data::String("AblIngenInvesteringsklassifikation".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &spouse_dividend_sheet, 1, header, value);
        }
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-par37-40-aegtefaelle-2026".to_string()),
            ),
            (
                "item_id",
                Data::String("par37-40-aegtefaelle-udbytte".to_string()),
            ),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String("par37-40-aegtefaelle-udbytte".to_string()),
            ),
            (
                "udlodder",
                Data::String("Ll16AAlmindeligtSelskab".to_string()),
            ),
            ("modtager", Data::String("Ll16AAktuelAktionær".to_string())),
            (
                "aktiv.$variant",
                Data::String("PersonskatAlmindeligAktie".to_string()),
            ),
            ("beløb_kroner", Data::Int(50_000)),
            (
                "par13a_kildefakta.$variant",
                Data::String("AblPar13AUdbytteUdenForModregningsgrundlag".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &spouse_dividend_sheet, 2, header, value);
        }
        set_workbook_cell_by_header(
            sheets,
            "cases",
            13,
            "lønmodtager.skatteår",
            Data::String("2025".to_string()),
        );
        let property_tax_sheet =
            workbook_collection_sheet_name_from_rows(sheets, "ejendomsskatter.ejendomme");
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-ejendomsskatter-2025".to_string()),
            ),
            ("item_id", Data::String("ejerbolig-1".to_string())),
            ("position", Data::Int(1)),
            (
                "ordinært_grundlag.identifikation",
                Data::String("ejerbolig-1".to_string()),
            ),
            (
                "ordinært_grundlag.kommune",
                Data::String("København".to_string()),
            ),
            (
                "ordinært_grundlag.kategori",
                Data::String("EjskEnBoligenhed".to_string()),
            ),
            (
                "ordinært_grundlag.beliggenhed",
                Data::String("EjskDanmark".to_string()),
            ),
            (
                "ordinært_grundlag.erhvervsmæssigt_udlejet",
                Data::Bool(false),
            ),
            (
                "ordinært_grundlag.særlige_betingelser_for_nr6_til_nr8_opfyldt",
                Data::Bool(true),
            ),
            (
                "ordinært_grundlag.ejendomsværdi_kroner",
                Data::Int(3_710_000),
            ),
            ("ordinært_grundlag.grundværdi_kroner", Data::Int(3_363_000)),
            ("ordinært_grundlag.produktionsjord", Data::Bool(false)),
            (
                "ordinært_grundlag.ejendomsværdiskatteperiode.$variant",
                Data::String("EjendomsskatFraOgMed".to_string()),
            ),
            (
                "ordinært_grundlag.ejendomsværdiskatteperiode.EjendomsskatFraOgMed.dato.år",
                Data::Int(2025),
            ),
            (
                "ordinært_grundlag.ejendomsværdiskatteperiode.EjendomsskatFraOgMed.dato.måned",
                Data::Int(8),
            ),
            (
                "ordinært_grundlag.ejendomsværdiskatteperiode.EjendomsskatFraOgMed.dato.dag",
                Data::Int(1),
            ),
            (
                "ordinært_grundlag.grundskyldsperiode.$variant",
                Data::String("EjendomsskatFraOgMed".to_string()),
            ),
            (
                "ordinært_grundlag.grundskyldsperiode.EjendomsskatFraOgMed.dato.år",
                Data::Int(2025),
            ),
            (
                "ordinært_grundlag.grundskyldsperiode.EjendomsskatFraOgMed.dato.måned",
                Data::Int(8),
            ),
            (
                "ordinært_grundlag.grundskyldsperiode.EjendomsskatFraOgMed.dato.dag",
                Data::Int(1),
            ),
            ("ordinært_grundlag.ejerandel_basispoint", Data::Int(5_000)),
            (
                "nedslagsfakta.ejerskabshistorik.oprindelig_erhvervelsesdato.år",
                Data::Int(2025),
            ),
            (
                "nedslagsfakta.ejerskabshistorik.oprindelig_erhvervelsesdato.måned",
                Data::Int(8),
            ),
            (
                "nedslagsfakta.ejerskabshistorik.oprindelig_erhvervelsesdato.dag",
                Data::Int(1),
            ),
            (
                "nedslagsfakta.boliganvendelse",
                Data::String("EjskHelårsbolig".to_string()),
            ),
            ("nedslagsfakta.selvstændige_boligenheder", Data::Int(1)),
            (
                "nedslagsfakta.ejendomsform",
                Data::String("EjskIkkeEjerlejlighed".to_string()),
            ),
            (
                "nedslagsfakta.fredet_og_omfattet_af_ligningslovens_par15k",
                Data::Bool(false),
            ),
            (
                "nedslagsfakta.par24_beregningsgrundlag.$variant",
                Data::String("EjskPar24SammeVærdiSomPar13".to_string()),
            ),
            (
                "nedslagsfakta.pensionistsuccession.$variant",
                Data::String("EjskIngenPensionistsuccession".to_string()),
            ),
            (
                "overgangsomfang.vurderingskategori",
                Data::String("EjskEjerboligEfterEjendomsvurderingslovensPar3Stk1Nr1".to_string()),
            ),
            (
                "overgangsomfang.ejerkreds",
                Data::String("EjskKunFysiskeEjere".to_string()),
            ),
            (
                "overgangsvurderinger.rabat.$variant",
                Data::String("EjskIngenRabatvurderingerOplyst".to_string()),
            ),
            (
                "overgangsvurderinger.stigningsbegrænsning.$variant",
                Data::String("EjskIngenStigningsvurderingerOplyst".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &property_tax_sheet, 1, header, value);
        }
        let business_travel_sheet = workbook_collection_sheet_name_from_rows(
            sheets,
            "lønmodtager.erhvervsbefordring.sager",
        );
        for (row, item_id, employer_id, kilometres, reimbursement) in [
            (
                1,
                "arbejdsgiver-a-kørsel-1",
                "arbejdsgiver-a",
                19_500,
                76_830,
            ),
            (2, "arbejdsgiver-b-kørsel-1", "arbejdsgiver-b", 1_000, 3_940),
            (3, "arbejdsgiver-a-kørsel-2", "arbejdsgiver-a", 1_000, 3_510),
        ] {
            for (header, value) in [
                (
                    "case_id",
                    Data::String("personskat-erhvervsbefordring-2026".to_string()),
                ),
                ("item_id", Data::String(item_id.to_string())),
                ("position", Data::Int(row as i64)),
                ("identifikation", Data::String(item_id.to_string())),
                ("rækkefølge_i_indkomståret", Data::Int(row as i64)),
                (
                    "godtgørende_arbejdsgiver_identifikation",
                    Data::String(employer_id.to_string()),
                ),
                ("køretøj", Data::String("Ll9BEgenBil".to_string())),
                (
                    "befordring.art",
                    Data::String("Ll9BMellemArbejdspladser".to_string()),
                ),
                ("befordring.kilometer_i_sagen", Data::Int(kilometres)),
                (
                    "befordring.tres_dages_forhold.arbejdsdage_til_samme_arbejdsplads_inklusive_aktuel_dag_i_forudgående_12_måneder",
                    Data::Int(0),
                ),
                (
                    "befordring.tres_dages_forhold.sammenhængende_arbejdsdage_siden_sidst_på_arbejdspladsen",
                    Data::Int(0),
                ),
                (
                    "befordring.tres_dages_forhold.mange_forskellige_arbejdspladser",
                    Data::Bool(false),
                ),
                (
                    "befordring.tres_dages_forhold.ikke_sandsynligt_over_60_dage",
                    Data::Bool(false),
                ),
                (
                    "befordring.tres_dages_forhold.skriftligt_kørselsregnskabspålæg_aktivt",
                    Data::Bool(false),
                ),
                (
                    "befordring.tres_dages_forhold.kørselsregnskab_dokumenterer_erhvervsmæssig_befordring",
                    Data::Bool(false),
                ),
                (
                    "udgifter.har_afholdt_befordringsudgifter",
                    Data::Bool(true),
                ),
                (
                    "udgifter.dokumenterede_faktiske_kørselsudgifter_eksklusive_bro_tunnel_kroner",
                    Data::Int(4_000),
                ),
                (
                    "udgifter.dokumenterede_bro_tunnel_udgifter_kroner",
                    Data::Int(500),
                ),
                ("kundeopsøgende_aktivitet", Data::Bool(false)),
                (
                    "antal_arbejdsgivere_som_befordringen_vedrører_på_en_gang",
                    Data::Int(1),
                ),
                (
                    "godtgørelsesforhold.udbetalt_godtgørelse_kroner",
                    Data::Int(reimbursement),
                ),
                (
                    "godtgørelsesforhold.form",
                    Data::String("Ll9BKilometerafregnet".to_string()),
                ),
                (
                    "godtgørelsesforhold.arbejdsgiver_har_kontrolleret_kilometer",
                    Data::Bool(true),
                ),
                (
                    "godtgørelsesforhold.bogføringsbilag_opfylder_par6",
                    Data::Bool(true),
                ),
                (
                    "godtgørelsesforhold.modregnet_i_forud_aftalt_bruttoløn",
                    Data::Bool(false),
                ),
                (
                    "godtgørelsesforhold.firmabil_stillet_til_rådighed",
                    Data::Bool(false),
                ),
                (
                    "godtgørelsesforhold.dokumenteret_kørsel_i_eget_køretøj",
                    Data::Bool(true),
                ),
                (
                    "godtgørelsesforhold.fuldt_vederlag_betalt_for_firmabilskørsel_for_anden_arbejdsgiver",
                    Data::Bool(false),
                ),
                (
                    "godtgørelsesforhold.overskydende_beløb_behandlet_som_løn_ved_endelig_opgørelse",
                    Data::Bool(true),
                ),
                (
                    "godtgørelsesforhold.eventuel_godtgørelse_valgt_medregnet_i_indkomsten",
                    Data::Bool(false),
                ),
            ] {
                set_workbook_cell_by_header(sheets, &business_travel_sheet, row, header, value);
            }
        }
        let pbl53a_sheet =
            workbook_collection_sheet_name_from_rows(sheets, "kapitalindkomst.pbl53a.ordninger");
        for (
            row,
            case_id,
            identifikation,
            skatteyder_identifikation,
            oprindelig_rettighedshaver_identifikation,
            kapitalværdi_ved_oprettelsen_kroner,
            produkt,
        ) in [
            (
                1,
                "personskat-pbl53a-2026",
                "livsforsikring-pal",
                "person-1",
                "tidligere-ejer",
                100_000,
                "Pbl53ALivsforsikringsprodukt",
            ),
            (
                2,
                "personskat-pbl53a-2026",
                "pensionskasse-negativ",
                "person-1",
                "person-1",
                100_000,
                "Pbl53APensionskasseprodukt",
            ),
            (
                3,
                "personskat-pbl53a-2026",
                "pengeinstitut-halv-andel",
                "person-1",
                "person-1",
                190_000,
                "Pbl53APengeEllerKreditinstitutprodukt",
            ),
            (
                4,
                "personskat-pbl53a-overdrager-2026",
                "afstået-pengeinstitut",
                "tidligere-ejer",
                "tidligere-ejer",
                100_000,
                "Pbl53APengeEllerKreditinstitutprodukt",
            ),
        ] {
            for (header, value) in [
                ("case_id", Data::String(case_id.to_string())),
                ("item_id", Data::String(identifikation.to_string())),
                (
                    "position",
                    Data::Int(if row == 4 { 1 } else { row as i64 }),
                ),
                ("identifikation", Data::String(identifikation.to_string())),
                (
                    "skatteyder_identifikation",
                    Data::String(skatteyder_identifikation.to_string()),
                ),
                (
                    "omfangsfakta.oprettelsesdato.år",
                    Data::Int(if row == 1 { 1990 } else { 2020 }),
                ),
                (
                    "omfangsfakta.oprettelsesdato.måned",
                    Data::Int(1),
                ),
                ("omfangsfakta.oprettelsesdato.dag", Data::Int(1)),
                (
                    "omfangsfakta.oprindelig_rettighedshaver_identifikation",
                    Data::String(oprindelig_rettighedshaver_identifikation.to_string()),
                ),
                (
                    "omfangsfakta.kapitalværdi_ved_oprettelsen_kroner",
                    Data::Int(kapitalværdi_ved_oprettelsen_kroner),
                ),
                (
                    "omfangsfakta.repræsenteret_kontraktdel.$variant",
                    Data::String("Pbl53AHeleKontrakten".to_string()),
                ),
                (
                    "omfangsfakta.overgangsvalgfristfakta.$variant",
                    Data::String(
                        if row == 1 {
                            "Pbl53ASenereArvUnderFuldSkattepligt"
                        } else {
                            "Pbl53AIntetOvergangsvalgfristgrundlag"
                        }
                        .to_string(),
                    ),
                ),
                (
                    "omfangsfakta.produkt.$variant",
                    Data::String(produkt.to_string()),
                ),
                (
                    "omfangsfakta.afsnit_i_valg.$variant",
                    Data::String("Pbl53AIntetAfkaldPåAfsnitI".to_string()),
                ),
                (
                    "omfangsfakta.institutionsfinansiering.samlet_drift_løn_og_pension_kroner",
                    Data::Int(1_000_000),
                ),
                (
                    "omfangsfakta.institutionsfinansiering.statsligt_finansieret_drift_løn_og_pension_kroner",
                    Data::Int(0),
                ),
                (
                    "omfangsfakta.par53b_oprettelsesposition.$variant",
                    Data::String("Pbl53BOprettetUnderDanskSkattepligtOgHjemsted".to_string()),
                ),
                (
                    "afkastforløbsåbning.$variant",
                    Data::String("Pbl53AIngenTidligereAfkasthistorik".to_string()),
                ),
            ] {
                set_workbook_cell_by_header(sheets, &pbl53a_sheet, row, header, value);
            }
            match row {
                1 => {
                    for (header, value) in [
                        (
                            "omfangsfakta.overgangsvalgfristfakta.Pbl53ASenereArvUnderFuldSkattepligt.arvedato.år",
                            Data::Int(2024),
                        ),
                        (
                            "omfangsfakta.overgangsvalgfristfakta.Pbl53ASenereArvUnderFuldSkattepligt.arvedato.måned",
                            Data::Int(3),
                        ),
                        (
                            "omfangsfakta.overgangsvalgfristfakta.Pbl53ASenereArvUnderFuldSkattepligt.arvedato.dag",
                            Data::Int(15),
                        ),
                        (
                            "omfangsfakta.overgangsvalgfristfakta.Pbl53ASenereArvUnderFuldSkattepligt.oplysningsfrist.år",
                            Data::Int(2025),
                        ),
                        (
                            "omfangsfakta.overgangsvalgfristfakta.Pbl53ASenereArvUnderFuldSkattepligt.oplysningsfrist.måned",
                            Data::Int(7),
                        ),
                        (
                            "omfangsfakta.overgangsvalgfristfakta.Pbl53ASenereArvUnderFuldSkattepligt.oplysningsfrist.dag",
                            Data::Int(1),
                        ),
                        (
                            "omfangsfakta.overgangsvalgfristfakta.Pbl53ASenereArvUnderFuldSkattepligt.fuld_skattepligtig_på_arvedatoen",
                            Data::Bool(true),
                        ),
                        (
                            "omfangsfakta.overgangsvalgfristfakta.Pbl53ASenereArvUnderFuldSkattepligt.tidligere_ejer_fuld_skattepligtig_i_ejerperioden",
                            Data::Bool(false),
                        ),
                        (
                            "omfangsfakta.overgangsvalgfristfakta.Pbl53ASenereArvUnderFuldSkattepligt.tidligere_ejer_havde_valgt_afsnit_iia",
                            Data::Bool(false),
                        ),
                        (
                            "omfangsfakta.produkt.Pbl53ALivsforsikringsprodukt.ejer_identifikation",
                            Data::String("person-1".to_string()),
                        ),
                        (
                            "omfangsfakta.produkt.Pbl53ALivsforsikringsprodukt.forsikret_identifikation",
                            Data::String("person-1".to_string()),
                        ),
                        (
                            "omfangsfakta.produkt.Pbl53ALivsforsikringsprodukt.kapitel1fakta.$variant",
                            Data::String("Pbl53AIkkeOmfattetAfKapitel1".to_string()),
                        ),
                        (
                            "omfangsfakta.produkt.Pbl53ALivsforsikringsprodukt.vilkår.aftalt_udløbsdato.år",
                            Data::Int(2050),
                        ),
                        (
                            "omfangsfakta.produkt.Pbl53ALivsforsikringsprodukt.vilkår.aftalt_udløbsdato.måned",
                            Data::Int(1),
                        ),
                        (
                            "omfangsfakta.produkt.Pbl53ALivsforsikringsprodukt.vilkår.aftalt_udløbsdato.dag",
                            Data::Int(1),
                        ),
                        (
                            "omfangsfakta.produkt.Pbl53ALivsforsikringsprodukt.vilkår.første_policedag_efter_fyldte_80_år.år",
                            Data::Int(2060),
                        ),
                        (
                            "omfangsfakta.produkt.Pbl53ALivsforsikringsprodukt.vilkår.første_policedag_efter_fyldte_80_år.måned",
                            Data::Int(1),
                        ),
                        (
                            "omfangsfakta.produkt.Pbl53ALivsforsikringsprodukt.vilkår.første_policedag_efter_fyldte_80_år.dag",
                            Data::Int(1),
                        ),
                        (
                            "omfangsfakta.produkt.Pbl53ALivsforsikringsprodukt.direktørsikkerhed.$variant",
                            Data::String("Pbl53AIngenDirektørsikkerhed".to_string()),
                        ),
                    ] {
                        set_workbook_cell_by_header(
                            sheets,
                            &pbl53a_sheet,
                            row,
                            header,
                            value,
                        );
                    }
                }
                2 => {
                    for (header, value) in [
                        (
                            "omfangsfakta.produkt.Pbl53APensionskasseprodukt.pensionsberettiget_identifikation",
                            Data::String("person-1".to_string()),
                        ),
                        (
                            "omfangsfakta.produkt.Pbl53APensionskasseprodukt.kapitel1fakta.$variant",
                            Data::String("Pbl53AIkkeOmfattetAfKapitel1".to_string()),
                        ),
                        (
                            "omfangsfakta.produkt.Pbl53APensionskasseprodukt.karakteristika.selvstændig_juridisk_person",
                            Data::Bool(true),
                        ),
                        (
                            "omfangsfakta.produkt.Pbl53APensionskasseprodukt.karakteristika.uafhængig_af_arbejdsgiver",
                            Data::Bool(true),
                        ),
                        (
                            "omfangsfakta.produkt.Pbl53APensionskasseprodukt.karakteristika.midler_afsondret_fra_berettigedes_formue",
                            Data::Bool(true),
                        ),
                        (
                            "omfangsfakta.produkt.Pbl53APensionskasseprodukt.karakteristika.vedtægter_fastlægger_pensionsydelser",
                            Data::Bool(true),
                        ),
                        (
                            "omfangsfakta.produkt.Pbl53APensionskasseprodukt.direktørsikkerhed.$variant",
                            Data::String("Pbl53AIngenDirektørsikkerhed".to_string()),
                        ),
                    ] {
                        set_workbook_cell_by_header(
                            sheets,
                            &pbl53a_sheet,
                            row,
                            header,
                            value,
                        );
                    }
                }
                3 | 4 => {
                    for (header, value) in [
                        (
                            "omfangsfakta.produkt.Pbl53APengeEllerKreditinstitutprodukt.kontohaver_identifikation",
                            Data::String("person-1".to_string()),
                        ),
                        (
                            "omfangsfakta.produkt.Pbl53APengeEllerKreditinstitutprodukt.kapitel1fakta.$variant",
                            Data::String("Pbl53AIkkeOmfattetAfKapitel1".to_string()),
                        ),
                        (
                            "omfangsfakta.produkt.Pbl53APengeEllerKreditinstitutprodukt.institutionssted.$variant",
                            Data::String("Pbl53ADanskInstitution".to_string()),
                        ),
                        (
                            "omfangsfakta.produkt.Pbl53APengeEllerKreditinstitutprodukt.karakteristika.standardiseret_lovreguleret_pensionsprodukt",
                            Data::Bool(true),
                        ),
                        (
                            "omfangsfakta.produkt.Pbl53APengeEllerKreditinstitutprodukt.karakteristika.pensionsmidler_adskilt_fra_øvrig_formue",
                            Data::Bool(true),
                        ),
                        (
                            "omfangsfakta.produkt.Pbl53APengeEllerKreditinstitutprodukt.karakteristika.kan_disponeres_som_almindelig_bankkonto",
                            Data::Bool(false),
                        ),
                    ] {
                        set_workbook_cell_by_header(
                            sheets,
                            &pbl53a_sheet,
                            row,
                            header,
                            value,
                        );
                    }
                }
                _ => unreachable!(),
            }
        }
        let pbl53a_contract_changes_sheet = workbook_collection_sheet_name_from_rows(
            sheets,
            "kapitalindkomst.pbl53a.ordninger.omfangsfakta.kontraktændringer",
        );
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-pbl53a-2026".to_string()),
            ),
            ("parent_id", Data::String("livsforsikring-pal".to_string())),
            (
                "item_id",
                Data::String("livsforsikring-pal-valuta-2014".to_string()),
            ),
            ("position", Data::Int(1)),
            ("identifikation", Data::String("valuta-2014".to_string())),
            ("ændringsdato.år", Data::Int(2014)),
            ("ændringsdato.måned", Data::Int(6)),
            ("ændringsdato.dag", Data::Int(1)),
            ("virkningstidspunkt.dato.år", Data::Int(2014)),
            ("virkningstidspunkt.dato.måned", Data::Int(6)),
            ("virkningstidspunkt.dato.dag", Data::Int(1)),
            ("virkningstidspunkt.rækkefølge_på_dagen", Data::Int(1)),
            (
                "kapitalværdi_på_virkningstidspunktet.$variant",
                Data::String("Pbl53AKapitalværdiIkkeRelevant".to_string()),
            ),
            (
                "forhåndsaftale.$variant",
                Data::String("Pbl53AIngenDokumenteretForhåndsaftale".to_string()),
            ),
            (
                "art.$variant",
                Data::String("Pbl53AValutaÆndret".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &pbl53a_contract_changes_sheet, 1, header, value);
        }
        let pbl53a_acquisitions_sheet = workbook_collection_sheet_name_from_rows(
            sheets,
            "kapitalindkomst.pbl53a.ordninger.omfangsfakta.erhvervelser",
        );
        for (
            row,
            case_id,
            parent_id,
            item_id,
            identifikation,
            år,
            måned,
            dag,
            rækkefølge_på_dagen,
            overdrager,
            erhverver,
            kapitalværdi,
            måde,
        ) in [
            (
                1,
                "personskat-pbl53a-2026",
                "livsforsikring-pal",
                "livsforsikring-pal-erhvervelse-1",
                "arv-2024",
                2024,
                3,
                15,
                1,
                "tidligere-ejer",
                "person-1",
                200_000,
                "Pbl53AErhvervetVedArv",
            ),
            (
                2,
                "personskat-pbl53a-overdrager-2026",
                "afstået-pengeinstitut",
                "afstået-pengeinstitut-erhvervelse-1",
                "køb-1-juni",
                2026,
                6,
                1,
                2,
                "tidligere-ejer",
                "person-1",
                130_000,
                "Pbl53AErhvervetPåAndenMåde",
            ),
        ] {
            for (header, value) in [
                ("case_id", Data::String(case_id.to_string())),
                ("parent_id", Data::String(parent_id.to_string())),
                ("item_id", Data::String(item_id.to_string())),
                ("position", Data::Int(1)),
                ("identifikation", Data::String(identifikation.to_string())),
                ("tidspunkt.dato.år", Data::Int(år)),
                ("tidspunkt.dato.måned", Data::Int(måned)),
                ("tidspunkt.dato.dag", Data::Int(dag)),
                (
                    "tidspunkt.rækkefølge_på_dagen",
                    Data::Int(rækkefølge_på_dagen),
                ),
                (
                    "overdrager_identifikation",
                    Data::String(overdrager.to_string()),
                ),
                (
                    "erhverver_identifikation",
                    Data::String(erhverver.to_string()),
                ),
                (
                    "kapitalværdi_på_erhvervelsestidspunktet_kroner",
                    Data::Int(kapitalværdi),
                ),
                ("måde", Data::String(måde.to_string())),
            ] {
                set_workbook_cell_by_header(sheets, &pbl53a_acquisitions_sheet, row, header, value);
            }
        }
        let pbl53a_elections_sheet = workbook_collection_sheet_name_from_rows(
            sheets,
            "kapitalindkomst.pbl53a.ordninger.omfangsfakta.overgangsvalg",
        );
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-pbl53a-2026".to_string()),
            ),
            ("parent_id", Data::String("livsforsikring-pal".to_string())),
            (
                "item_id",
                Data::String("livsforsikring-pal-overgangsvalg-1".to_string()),
            ),
            ("position", Data::Int(1)),
            ("beslutningsdato.år", Data::Int(2024)),
            ("beslutningsdato.måned", Data::Int(4)),
            ("beslutningsdato.dag", Data::Int(1)),
            ("modtagelsesdato.år", Data::Int(2024)),
            ("modtagelsesdato.måned", Data::Int(4)),
            ("modtagelsesdato.dag", Data::Int(3)),
            ("mål", Data::String("Pbl53AValgAfPar53A".to_string())),
            (
                "modtager",
                Data::String("Pbl53AValgMeddeltSkattestyrelsen".to_string()),
            ),
            (
                "ønsket_virkning",
                Data::String("Pbl53AValgVirkningFraModtagelse".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &pbl53a_elections_sheet, 1, header, value);
        }
        let pbl53a_legacy_forms_sheet = workbook_collection_sheet_name_from_rows(
            sheets,
            "kapitalindkomst.pbl53a.ordninger.omfangsfakta.historiske_blanket49020_indsendelser",
        );
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-pbl53a-2026".to_string()),
            ),
            ("parent_id", Data::String("livsforsikring-pal".to_string())),
            (
                "item_id",
                Data::String("livsforsikring-pal-blanket49020-1".to_string()),
            ),
            ("position", Data::Int(1)),
            ("indsendelsesdato.år", Data::Int(2024)),
            ("indsendelsesdato.måned", Data::Int(4)),
            ("indsendelsesdato.dag", Data::Int(2)),
            ("modtagelsesdato.år", Data::Int(2024)),
            ("modtagelsesdato.måned", Data::Int(4)),
            ("modtagelsesdato.dag", Data::Int(2)),
            (
                "udgave",
                Data::String("Pbl53ANyBlanket49020MedValgfelt".to_string()),
            ),
            (
                "modtager",
                Data::String("Pbl53AValgMeddeltSkattestyrelsen".to_string()),
            ),
            (
                "påberåbelse",
                Data::String("Pbl53AValgEfterPar53AEllerPar53BPåberåbt".to_string()),
            ),
            (
                "ønsket_virkning",
                Data::String("Pbl53AValgVirkningFraModtagelse".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &pbl53a_legacy_forms_sheet, 1, header, value);
        }
        let pbl53a_coverages_sheet = workbook_collection_sheet_name_from_rows(
            sheets,
            "kapitalindkomst.pbl53a.ordninger.omfangsfakta.produkt.Pbl53ALivsforsikringsprodukt.vilkår.dækninger",
        );
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-pbl53a-2026".to_string()),
            ),
            ("parent_id", Data::String("livsforsikring-pal".to_string())),
            (
                "item_id",
                Data::String("livsforsikring-pal-dækning-1".to_string()),
            ),
            ("position", Data::Int(1)),
            (
                "value",
                Data::String("Pbl53AAndenLivsforsikringsdækning".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &pbl53a_coverages_sheet, 1, header, value);
        }
        let pbl53a_years_path = "kapitalindkomst.pbl53a.ordninger.afkastår";
        let pbl53a_years_sheet =
            workbook_collection_sheet_name_from_rows(sheets, pbl53a_years_path);
        for (
            row,
            case_id,
            parent_id,
            item_id,
            position,
            year,
            basis,
            pal_return,
            calendar_opening,
            calendar_closing,
            provider_used_pal,
            tax_status,
            allocation_variant,
            rights_period_origin,
            total_balance,
        ) in [
            (
                1,
                "personskat-pbl53a-2026",
                "livsforsikring-pal",
                "livsforsikring-pal-2025",
                1,
                2025,
                "Pbl53AAfkastEfterPal",
                Some(-6_000),
                None,
                None,
                true,
                "Pbl53ASkattepligtigVedÅretsBegyndelse",
                "Pbl53AAfledtEnkeltRettighedshaver",
                None,
                None,
            ),
            (
                2,
                "personskat-pbl53a-2026",
                "livsforsikring-pal",
                "livsforsikring-pal-2026",
                2,
                2026,
                "Pbl53AAfkastEfterPal",
                Some(28_000),
                None,
                None,
                true,
                "Pbl53ASkattepligtigVedÅretsBegyndelse",
                "Pbl53AAfledtEnkeltRettighedshaver",
                None,
                None,
            ),
            (
                3,
                "personskat-pbl53a-2026",
                "pensionskasse-negativ",
                "pensionskasse-negativ-2025",
                1,
                2025,
                "Pbl53AAlternativtKapitalværdiAfkast",
                None,
                Some(100_000),
                Some(95_000),
                false,
                "Pbl53ASkattepligtigVedÅretsBegyndelse",
                "Pbl53AAfledtEnkeltRettighedshaver",
                None,
                None,
            ),
            (
                4,
                "personskat-pbl53a-2026",
                "pensionskasse-negativ",
                "pensionskasse-negativ-2026",
                2,
                2026,
                "Pbl53AAlternativtKapitalværdiAfkast",
                None,
                Some(140_000),
                Some(132_000),
                false,
                "Pbl53ASkattepligtigVedÅretsBegyndelse",
                "Pbl53AAfledtEnkeltRettighedshaver",
                None,
                None,
            ),
            (
                5,
                "personskat-pbl53a-2026",
                "pengeinstitut-halv-andel",
                "pengeinstitut-halv-andel-2026",
                1,
                2026,
                "Pbl53AAlternativtKapitalværdiAfkast",
                None,
                Some(190_000),
                Some(227_000),
                false,
                "Pbl53AIkkeSkattepligtigVedÅretsBegyndelse",
                "Pbl53AFlereBerettigedeVedAfkastperiodensUdgang",
                Some("Pbl53ARettighedsperiodeFraOprettelsen"),
                Some(400_000),
            ),
            (
                6,
                "personskat-pbl53a-overdrager-2026",
                "afstået-pengeinstitut",
                "afstået-pengeinstitut-2026",
                1,
                2026,
                "Pbl53AAlternativtKapitalværdiAfkast",
                None,
                Some(100_000),
                Some(160_000),
                false,
                "Pbl53ASkattepligtigVedÅretsBegyndelse",
                "Pbl53AAfledtEnkeltRettighedshaver",
                None,
                None,
            ),
        ] {
            for (header, value) in [
                ("case_id", Data::String(case_id.to_string())),
                ("parent_id", Data::String(parent_id.to_string())),
                ("item_id", Data::String(item_id.to_string())),
                ("position", Data::Int(position)),
                ("indkomstår", Data::Int(year)),
                ("afkastgrundlag.$variant", Data::String(basis.to_string())),
                (
                    "pensionsudbyder_opgjorde_afkast_efter_pal",
                    Data::Bool(provider_used_pal),
                ),
                (
                    "skattepligtsstatus_ved_årets_begyndelse",
                    Data::String(tax_status.to_string()),
                ),
                (
                    "sikkerhedsstatus_ved_årets_begyndelse",
                    Data::String("Pbl53ASikkerhedIkkeRelevant".to_string()),
                ),
                (
                    "afkastfordeling.$variant",
                    Data::String(allocation_variant.to_string()),
                ),
            ] {
                set_workbook_cell_by_header(sheets, &pbl53a_years_sheet, row, header, value);
            }
            if let Some(value) = pal_return {
                set_workbook_cell_by_header(
                    sheets,
                    &pbl53a_years_sheet,
                    row,
                    "afkastgrundlag.Pbl53AAfkastEfterPal.afkast_efter_pal_par3_til_5_kroner",
                    Data::Int(value),
                );
            }
            if let Some(value) = calendar_opening {
                set_workbook_cell_by_header(
                    sheets,
                    &pbl53a_years_sheet,
                    row,
                    "afkastgrundlag.Pbl53AAlternativtKapitalværdiAfkast.kalenderårets_primo_depotværdi_kroner",
                    Data::Int(value),
                );
            }
            if let Some(value) = calendar_closing {
                set_workbook_cell_by_header(
                    sheets,
                    &pbl53a_years_sheet,
                    row,
                    "afkastgrundlag.Pbl53AAlternativtKapitalværdiAfkast.kalenderårets_ultimo_depotværdi_kroner",
                    Data::Int(value),
                );
            }
            if let Some(value) = rights_period_origin {
                set_workbook_cell_by_header(
                    sheets,
                    &pbl53a_years_sheet,
                    row,
                    "afkastfordeling.Pbl53AFlereBerettigedeVedAfkastperiodensUdgang.rettighedsperiodereference.$variant",
                    Data::String(value.to_string()),
                );
            }
            if let Some(value) = total_balance {
                set_workbook_cell_by_header(
                    sheets,
                    &pbl53a_years_sheet,
                    row,
                    "afkastfordeling.Pbl53AFlereBerettigedeVedAfkastperiodensUdgang.samlet_indestående_ved_afkastperiodens_udgang_kroner",
                    Data::Int(value),
                );
            }
        }
        let pbl53a_boundaries_path = "kapitalindkomst.pbl53a.ordninger.afkastår.grænsehændelser";
        let pbl53a_boundaries_sheet =
            workbook_collection_sheet_name_from_rows(sheets, pbl53a_boundaries_path);
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-pbl53a-2026".to_string()),
            ),
            (
                "parent_id",
                Data::String("pengeinstitut-halv-andel-2026".to_string()),
            ),
            (
                "item_id",
                Data::String("skattepligt-indtræder-2026".to_string()),
            ),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String("skattepligt-indtræder-2026".to_string()),
            ),
            ("tidspunkt.dato.år", Data::Int(2026)),
            ("tidspunkt.dato.måned", Data::Int(3)),
            ("tidspunkt.dato.dag", Data::Int(1)),
            ("tidspunkt.rækkefølge_på_dagen", Data::Int(1)),
            ("depotværdi_kroner", Data::Int(200_000)),
            (
                "art",
                Data::String("Pbl53ASkattepligtIndtræder".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &pbl53a_boundaries_sheet, 1, header, value);
        }
        let pbl53a_shares_path = "kapitalindkomst.pbl53a.ordninger.afkastår.afkastfordeling.Pbl53AFlereBerettigedeVedAfkastperiodensUdgang.andele";
        let pbl53a_shares_sheet =
            workbook_collection_sheet_name_from_rows(sheets, pbl53a_shares_path);
        for (row, person, balance) in [(1, "person-1", 200_000), (2, "person-2", 200_000)] {
            for (header, value) in [
                (
                    "case_id",
                    Data::String("personskat-pbl53a-2026".to_string()),
                ),
                (
                    "parent_id",
                    Data::String("pengeinstitut-halv-andel-2026".to_string()),
                ),
                ("item_id", Data::String(person.to_string())),
                ("position", Data::Int(row as i64)),
                ("identifikation", Data::String(person.to_string())),
                (
                    "indestående_ved_afkastperiodens_udgang_kroner",
                    Data::Int(balance),
                ),
            ] {
                set_workbook_cell_by_header(sheets, &pbl53a_shares_sheet, row, header, value);
            }
        }
        let pbl53a_events_path = "kapitalindkomst.pbl53a.ordninger.hændelser";
        let pbl53a_events_sheet =
            workbook_collection_sheet_name_from_rows(sheets, pbl53a_events_path);
        for (row, case_id, parent_id, item_id, måned, beløb, indbetaler) in [
            (
                1,
                "personskat-pbl53a-2026",
                "livsforsikring-pal",
                "arbejdsgiver-indbetaling-2026",
                3,
                60_000,
                "Pbl53ANuværendeArbejdsgiver",
            ),
            (
                2,
                "personskat-pbl53a-overdrager-2026",
                "afstået-pengeinstitut",
                "indbetaling-før-overdragelse",
                3,
                10_000,
                "Pbl53AEjeren",
            ),
        ] {
            for (header, value) in [
                ("case_id".to_string(), Data::String(case_id.to_string())),
                (
                    "parent_id".to_string(),
                    Data::String(parent_id.to_string()),
                ),
                ("item_id".to_string(), Data::String(item_id.to_string())),
                ("position".to_string(), Data::Int(1)),
                (
                    format!("{pbl53a_events_path}.$variant"),
                    Data::String("Pbl53AIndbetaling".to_string()),
                ),
                (
                    format!("{pbl53a_events_path}.Pbl53AIndbetaling.fakta.identifikation"),
                    Data::String(item_id.to_string()),
                ),
                (
                    format!("{pbl53a_events_path}.Pbl53AIndbetaling.fakta.tidspunkt.dato.år"),
                    Data::Int(2026),
                ),
                (
                    format!("{pbl53a_events_path}.Pbl53AIndbetaling.fakta.tidspunkt.dato.måned"),
                    Data::Int(måned),
                ),
                (
                    format!("{pbl53a_events_path}.Pbl53AIndbetaling.fakta.tidspunkt.dato.dag"),
                    Data::Int(1),
                ),
                (
                    format!(
                        "{pbl53a_events_path}.Pbl53AIndbetaling.fakta.tidspunkt.rækkefølge_på_dagen"
                    ),
                    Data::Int(1),
                ),
                (
                    format!("{pbl53a_events_path}.Pbl53AIndbetaling.fakta.beløb_kroner"),
                    Data::Int(beløb),
                ),
                (
                    format!("{pbl53a_events_path}.Pbl53AIndbetaling.fakta.periode"),
                    Data::String("Pbl53AIndbetaltMensOrdningenErOmfattet".to_string()),
                ),
                (
                    format!("{pbl53a_events_path}.Pbl53AIndbetaling.fakta.indbetaler.$variant"),
                    Data::String(indbetaler.to_string()),
                ),
                (
                    format!("{pbl53a_events_path}.Pbl53AIndbetaling.fakta.ejerens_fradragsstatus"),
                    Data::String("Pbl53AUdenFradragsEllerBortseelsesret".to_string()),
                ),
                (
                    format!("{pbl53a_events_path}.Pbl53AIndbetaling.fakta.par53b_udenlandsk_skattebehandling.$variant"),
                    Data::String("Pbl53BIkkeForetagetIUdenlandsperioden".to_string()),
                ),
            ] {
                set_workbook_cell_by_header(
                    sheets,
                    &pbl53a_events_sheet,
                    row,
                    &header,
                    value,
                );
            }
        }
        for (header, value) in [
            (
                "kapitalindkomst.ejendomsdrift.$variant",
                Data::String("MedEjendomsdriftEfterPar4Nr6".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsdrift.MedEjendomsdriftEfterPar4Nr6.fakta.kategori",
                Data::String("EjskLandzoneOver5000M2".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsdrift.MedEjendomsdriftEfterPar4Nr6.fakta.beliggenhed",
                Data::String("EjskDanmark".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsdrift.MedEjendomsdriftEfterPar4Nr6.fakta.erhvervsmæssigt_udlejet",
                Data::Bool(false),
            ),
            (
                "kapitalindkomst.ejendomsdrift.MedEjendomsdriftEfterPar4Nr6.fakta.særlige_betingelser_for_nr6_til_nr8_opfyldt",
                Data::Bool(true),
            ),
            (
                "kapitalindkomst.ejendomsdrift.MedEjendomsdriftEfterPar4Nr6.fakta.overskud_eller_underskud_kroner",
                Data::Int(25_000),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "cases", 8, header, value);
        }
        for (header, value) in [
            (
                "kapitalindkomst.fremleje.$variant",
                Data::String("MedFremlejeEfterLigningslov15Q".to_string()),
            ),
            (
                "kapitalindkomst.fremleje.MedFremlejeEfterLigningslov15Q.fakta.rolle",
                Data::String("PersonskatFremlejendeLejer".to_string()),
            ),
            (
                "kapitalindkomst.fremleje.MedFremlejeEfterLigningslov15Q.fakta.udlejningsform",
                Data::String("Ll15QVærelserIHelårsbolig".to_string()),
            ),
            (
                "kapitalindkomst.fremleje.MedFremlejeEfterLigningslov15Q.fakta.boligstatus",
                Data::String("Ll15QHelårsbolig".to_string()),
            ),
            (
                "kapitalindkomst.fremleje.MedFremlejeEfterLigningslov15Q.fakta.indberetningsstatus",
                Data::String("Ll15QIndberettetEfterSkatteindberetningslov43".to_string()),
            ),
            (
                "kapitalindkomst.fremleje.MedFremlejeEfterLigningslov15Q.fakta.metode",
                Data::String("Ll15QStk1Bundfradrag".to_string()),
            ),
            (
                "kapitalindkomst.fremleje.MedFremlejeEfterLigningslov15Q.fakta.bruttolejeindtægt_kroner",
                Data::Int(60_000),
            ),
            (
                "kapitalindkomst.fremleje.MedFremlejeEfterLigningslov15Q.fakta.faktiske_udgifter_kroner",
                Data::Int(0),
            ),
            (
                "kapitalindkomst.fremleje.MedFremlejeEfterLigningslov15Q.fakta.tidligere_anvendt_par15p_stk3",
                Data::Bool(false),
            ),
            (
                "kapitalindkomst.fremleje.MedFremlejeEfterLigningslov15Q.fakta.stk4_samordning.$variant",
                Data::String("UdenSamordningMedLigningslov15P".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "cases", 7, header, value);
        }
        for (header, value) in [
            (
                "aktieavance.ordinært_aktieår.$variant",
                Data::String("MedOrdinærtAktieår".to_string()),
            ),
            (
                "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.indkomstår",
                Data::Int(2026),
            ),
            (
                "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.fremført_tab_efter_par13a_kroner",
                Data::Int(0),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "cases", 3, header, value);
        }
        let ordinary_holdings_path =
            "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb";
        let ordinary_events_path = format!("{ordinary_holdings_path}.hændelser");
        let ordinary_holdings_sheet =
            workbook_collection_sheet_name_from_rows(sheets, ordinary_holdings_path);
        let ordinary_events_sheet =
            workbook_collection_sheet_name_from_rows(sheets, &ordinary_events_path);
        for (header, value) in [
            (
                "case_id".to_string(),
                Data::String("personskat-renter-befordring-2026".to_string()),
            ),
            (
                "item_id".to_string(),
                Data::String("boligret-beholdning-1".to_string()),
            ),
            ("position".to_string(), Data::Int(1)),
            (
                format!("{ordinary_holdings_path}.position_primo.selskabsidentifikation"),
                Data::String("DK-BOLIGRET-1".to_string()),
            ),
            (
                format!("{ordinary_holdings_path}.position_primo.kapitalmængde.$variant"),
                Data::String("AblAktiekapitalUdenPålydendeVærdi".to_string()),
            ),
            (
                format!("{ordinary_holdings_path}.position_primo.kapitalmængde.AblAktiekapitalUdenPålydendeVærdi.antal_aktier"),
                Data::Int(10),
            ),
            (
                format!("{ordinary_holdings_path}.position_primo.anskaffelsessum_kroner"),
                Data::Int(10_000),
            ),
        ] {
            set_workbook_cell_by_header(
                sheets,
                &ordinary_holdings_sheet,
                1,
                &header,
                value,
            );
        }
        for (header, value) in [
            (
                "case_id".to_string(),
                Data::String("personskat-renter-befordring-2026".to_string()),
            ),
            (
                "parent_id".to_string(),
                Data::String("boligret-beholdning-1".to_string()),
            ),
            (
                "item_id".to_string(),
                Data::String("boligret-afståelse-1".to_string()),
            ),
            ("position".to_string(), Data::Int(1)),
            (
                format!("{ordinary_events_path}.$variant"),
                Data::String("AblOrdinærAfståelse".to_string()),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.kapitalmængde.$variant"),
                Data::String("AblAktiekapitalUdenPålydendeVærdi".to_string()),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.kapitalmængde.AblAktiekapitalUdenPålydendeVærdi.antal_aktier"),
                Data::Int(10),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.afståelsessum_kroner"),
                Data::Int(15_000),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.par5a_kildefakta.$variant"),
                Data::String("AblOrdinærIngenPar5AFaktaPåkrævet".to_string()),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.markedsstatus"),
                Data::String("AblIkkeOptagetTilHandel".to_string()),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.har_tidligere_været_optaget_til_handel"),
                Data::Bool(false),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.hovedaktionæraktier"),
                Data::Bool(false),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.afståede_aktiers_handelsværdi_kroner"),
                Data::Int(0),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.beholdte_aktiers_handelsværdi_kroner"),
                Data::Int(0),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.oplysningsstatus"),
                Data::String("AblOplystRettidigt".to_string()),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.boligret.$variant"),
                Data::String("AblBoligretEfterPar15".to_string()),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.udsteder.$variant"),
                Data::String("AblPar15Kapitalselskabsudsteder".to_string()),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.udsteder.AblPar15Kapitalselskabsudsteder.sel_input.selskabsform"),
                Data::String("Sel1Stk1Nr1IndregistreretAktieselskab".to_string()),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.udsteder.AblPar15Kapitalselskabsudsteder.sel_input.hjemmehørende_i_danmark"),
                Data::Bool(true),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.værdipapirstatus"),
                Data::String(
                    "AblPar15VærdipapirOmfattetAfAktieavancebeskatningsloven".to_string(),
                ),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.fakta.værdipapir_forbundet_med_brugsret_til_beboelseslejlighed"),
                Data::Bool(true),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.fakta.udsteder_ejer_direkte_ejendom_med_flere_beboelseslejligheder"),
                Data::Bool(true),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.fakta.lejlighed_har_tjent_til_bolig_mens_skattefrihedsbetingelser_var_opfyldt_i_ejertiden"),
                Data::Bool(true),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.fakta.grundforhold.$variant"),
                Data::String("EblPar8Stk4UdenBestemtGrundareal".to_string()),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.afståelsesform.$variant"),
                Data::String("AblPar15AlmindeligAfståelse".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &ordinary_events_sheet, 1, &header, value);
        }
        for (header, value) in [
            (
                "case_id".to_string(),
                Data::String("personskat-renter-befordring-2026".to_string()),
            ),
            (
                "item_id".to_string(),
                Data::String("par5a-beholdning-1".to_string()),
            ),
            ("position".to_string(), Data::Int(2)),
            (
                format!("{ordinary_holdings_path}.position_primo.selskabsidentifikation"),
                Data::String("DK-PAR5A-1".to_string()),
            ),
            (
                format!("{ordinary_holdings_path}.position_primo.kapitalmængde.$variant"),
                Data::String("AblAktiekapitalUdenPålydendeVærdi".to_string()),
            ),
            (
                format!("{ordinary_holdings_path}.position_primo.kapitalmængde.AblAktiekapitalUdenPålydendeVærdi.antal_aktier"),
                Data::Int(10),
            ),
            (
                format!("{ordinary_holdings_path}.position_primo.anskaffelsessum_kroner"),
                Data::Int(30_000),
            ),
        ] {
            set_workbook_cell_by_header(
                sheets,
                &ordinary_holdings_sheet,
                2,
                &header,
                value,
            );
        }
        let par5a_facts_path = format!(
            "{ordinary_events_path}.AblOrdinærAfståelse.par5a_kildefakta.AblOrdinærPar5AKildefakta.fakta"
        );
        for (header, value) in [
            (
                "case_id".to_string(),
                Data::String("personskat-renter-befordring-2026".to_string()),
            ),
            (
                "parent_id".to_string(),
                Data::String("par5a-beholdning-1".to_string()),
            ),
            (
                "item_id".to_string(),
                Data::String("par5a-afståelse-1".to_string()),
            ),
            ("position".to_string(), Data::Int(1)),
            (
                format!("{ordinary_events_path}.$variant"),
                Data::String("AblOrdinærAfståelse".to_string()),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.kapitalmængde.$variant"),
                Data::String("AblAktiekapitalUdenPålydendeVærdi".to_string()),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.kapitalmængde.AblAktiekapitalUdenPålydendeVærdi.antal_aktier"),
                Data::Int(10),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.afståelsessum_kroner"),
                Data::Int(0),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.par5a_kildefakta.$variant"),
                Data::String("AblOrdinærPar5AKildefakta".to_string()),
            ),
            (
                format!("{par5a_facts_path}.anvendelsesgrundlag"),
                Data::String("AblPar5AAfståelseDen24November2010EllerSenere".to_string()),
            ),
            (
                format!("{par5a_facts_path}.skatteydergrundlag"),
                Data::String("AblPar5APersonSkattepligtigEfterPar7".to_string()),
            ),
            (
                format!("{par5a_facts_path}.præferenceposition.modtaget_tilsvarende_udbytte_kroner"),
                Data::Int(0),
            ),
            (
                format!("{par5a_facts_path}.præferenceposition.allerede_anvendt_til_tabsreduktion_kroner"),
                Data::Int(0),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.markedsstatus"),
                Data::String("AblIkkeOptagetTilHandel".to_string()),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.har_tidligere_været_optaget_til_handel"),
                Data::Bool(false),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.hovedaktionæraktier"),
                Data::Bool(false),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.afståede_aktiers_handelsværdi_kroner"),
                Data::Int(0),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.beholdte_aktiers_handelsværdi_kroner"),
                Data::Int(0),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.oplysningsstatus"),
                Data::String("AblOplystRettidigt".to_string()),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.boligret.$variant"),
                Data::String("AblUdenBoligret".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &ordinary_events_sheet, 2, &header, value);
        }
        let par5a_dividends_path = format!("{par5a_facts_path}.ejertidsudbytter");
        let par5a_dividends_sheet =
            workbook_collection_sheet_name_from_rows(sheets, &par5a_dividends_path);
        for (header, value) in [
            (
                "case_id".to_string(),
                Data::String("personskat-renter-befordring-2026".to_string()),
            ),
            (
                "parent_id".to_string(),
                Data::String("par5a-afståelse-1".to_string()),
            ),
            (
                "item_id".to_string(),
                Data::String("par5a-udbytte-1".to_string()),
            ),
            ("position".to_string(), Data::Int(1)),
            (
                format!("{par5a_dividends_path}.$variant"),
                Data::String("AblPar5ASkattefritUdbytteAfPågældendeAktier".to_string()),
            ),
            (
                format!("{par5a_dividends_path}.AblPar5ASkattefritUdbytteAfPågældendeAktier.beløb_kroner"),
                Data::Int(12_000),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &par5a_dividends_sheet, 1, &header, value);
        }
        for (header, value) in [
            (
                "lønmodtager.ligningsfradrag.befordring.$variant",
                Data::String("MedBefordringsfradrag".to_string()),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.arbejdsdage",
                Data::Int(203),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.daglige_befordringskilometer",
                Data::Int(60),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.bopæl_i_yderkommune_eller_lille_ø",
                Data::Bool(false),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.befordringsformål",
                Data::String("IndtægtsgivendeArbejdsplads".to_string()),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.modtaget_skattefri_befordringsgodtgørelse_for_strækning",
                Data::Bool(false),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.modtaget_uddannelsesbefordringsrabat_eller_godtgørelse_for_strækning",
                Data::Bool(false),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.ligningslov9d.$variant",
                Data::String("UdenLigningslov9D".to_string()),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.fradrag_udelukket_folketingshverv_m_v",
                Data::Bool(false),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.arbejdsgiverbetalt_befordring",
                Data::String("UdenArbejdsgiverbetaltBefordring".to_string()),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.broer.storebælt_bil_motorcykel_passager",
                Data::Int(0),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.broer.storebælt_kollektiv_passager",
                Data::Int(0),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.broer.øresund_bil_motorcykel_passager",
                Data::Int(0),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.broer.øresund_kollektiv_passager",
                Data::Int(0),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.broer.dokumenteret_og_afholdt_af_skattepligtige",
                Data::Bool(false),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.særlig_transport.faktisk_dokumenteret_udgift_kroner",
                Data::Int(0),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.særlig_transport.geografiske_forhold_tidsforbrug_økonomisk_rimelighed_kræver_transporten",
                Data::Bool(false),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "cases", 3, header, value);
        }
        for (header, value) in [
            (
                "kapitalindkomst.ejendomsavance.$variant",
                Data::String("MedEjendomsavance".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.eget_fremført_tab.$variant",
                Data::String("MedFremførtEjendomstab".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.eget_fremført_tab.MedFremførtEjendomstab.fra_indkomstår",
                Data::Int(2025),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.eget_fremført_tab.MedFremførtEjendomstab.tab_kroner",
                Data::Int(25_000),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.ægtefælles_fremførte_tab.$variant",
                Data::String("UdenFremførtEjendomstab".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.gift_samlevende_ved_indkomstårets_udgang",
                Data::Bool(true),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "cases", 3, header, value);
        }
        for (header, value) in [
            (
                "kapitalindkomst.ejendomsavance.$variant",
                Data::String("MedEjendomsavance".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.eget_fremført_tab.$variant",
                Data::String("UdenFremførtEjendomstab".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.ægtefælles_fremførte_tab.$variant",
                Data::String("UdenFremførtEjendomstab".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.gift_samlevende_ved_indkomstårets_udgang",
                Data::Bool(false),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "cases", 4, header, value);
        }
        for (header, value) in [
            (
                "kapitalindkomst.ejendomsavance.$variant",
                Data::String("MedEjendomsavance".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.eget_fremført_tab.$variant",
                Data::String("UdenFremførtEjendomstab".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.ægtefælles_fremførte_tab.$variant",
                Data::String("UdenFremførtEjendomstab".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.gift_samlevende_ved_indkomstårets_udgang",
                Data::Bool(false),
            ),
            (
                "kapitalindkomst.kursgevinst.$variant",
                Data::String("MedKursgevinst".to_string()),
            ),
            (
                "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.skatteyder_identifikation",
                Data::String("Sælger".to_string()),
            ),
            (
                "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.par32_kontraktforløb.$variant",
                Data::String("UdenPar32Kontraktforløb".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "cases", 5, header, value);
        }
        for (header, value) in [
            (
                "kapitalindkomst.ejendomsavance.$variant",
                Data::String("MedEjendomsavance".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.eget_fremført_tab.$variant",
                Data::String("UdenFremførtEjendomstab".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.ægtefælles_fremførte_tab.$variant",
                Data::String("UdenFremførtEjendomstab".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.gift_samlevende_ved_indkomstårets_udgang",
                Data::Bool(false),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "cases", 6, header, value);
        }
        let own_property_sheet = workbook_collection_sheet_name_from_rows(
            sheets,
            "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.egne_afståelser",
        );
        let spouse_property_sheet = workbook_collection_sheet_name_from_rows(
            sheets,
            "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.ægtefælles_afståelser",
        );
        for (row, item_id, identification, acquisition, disposal) in [
            (
                1,
                "egen-fortjeneste-1",
                "egen-fortjeneste",
                1_000_000,
                1_200_000,
            ),
            (2, "eget-tab-1", "eget-tab", 500_000, 450_000),
        ] {
            for (header, value) in [
                (
                    "case_id",
                    Data::String("personskat-renter-befordring-2026".to_string()),
                ),
                ("item_id", Data::String(item_id.to_string())),
                ("position", Data::Int(row as i64)),
                ("identifikation", Data::String(identification.to_string())),
                ("afståelsesdato.år", Data::Int(2026)),
                ("afståelsesdato.måned", Data::Int(12)),
                ("afståelsesdato.dag", Data::Int(31)),
                (
                    "afståelse",
                    Data::String("EblAlmindeligAfståelse".to_string()),
                ),
                ("erhvervet_som_led_i_næring", Data::Bool(false)),
                ("kontant_anskaffelsessum_kroner", Data::Int(acquisition)),
                ("gæld_kursværdi_ved_anskaffelse_kroner", Data::Int(0)),
                ("par5_fakta.anskaffelsesdato.år", Data::Int(2026)),
                ("par5_fakta.anskaffelsesdato.måned", Data::Int(1)),
                ("par5_fakta.anskaffelsesdato.dag", Data::Int(1)),
                (
                    "par5_fakta.anskaffelsesgrundlag.$variant",
                    Data::String("EblPar4AlmindeligtAnskaffelsesgrundlag".to_string()),
                ),
                ("par5_fakta.fordeling.ejerandel_promille", Data::Int(1000)),
                (
                    "par5_fakta.fordeling.afståelsesomfang.$variant",
                    Data::String("EblPar5HeleEjendommen".to_string()),
                ),
                (
                    "par5_fakta.stk6_overførsel.$variant",
                    Data::String("EblPar5UdenStk6Overførsel".to_string()),
                ),
                (
                    "par5_fakta.reguleringsvalg.$variant",
                    Data::String("EblPar5UdenIndeksering".to_string()),
                ),
                ("par4_stk8_anskaffelse_udeladt_kroner", Data::Int(0)),
                ("kontant_afståelsessum_kroner", Data::Int(disposal)),
                ("overdragne_gældsposter_kursværdi_kroner", Data::Int(0)),
                ("par4_stk8_afståelsesværdi_udeladt_kroner", Data::Int(0)),
                (
                    "par11_stk2_valg.$variant",
                    Data::String("EblPar11Stk2IngenNyGenanbringelse".to_string()),
                ),
                (
                    "ejendomstype.$variant",
                    Data::String("EblAndenFastEjendom".to_string()),
                ),
                (
                    "par6d_valg.$variant",
                    Data::String("EblUdenPar6DValg".to_string()),
                ),
            ] {
                set_workbook_cell_by_header(sheets, &own_property_sheet, row, header, value);
            }
        }
        set_workbook_cell_by_header(
            sheets,
            &own_property_sheet,
            2,
            "ejendomstype.EblAndenFastEjendom.genanbringelse.$variant",
            Data::String("EblUdenAktivGenanbringelse".to_string()),
        );
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-ebl5-kildefakta-2026".to_string()),
            ),
            ("item_id", Data::String("ebl5-landbrug-1".to_string())),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String("landbrug-med-mælk-og-stk6".to_string()),
            ),
            ("afståelsesdato.år", Data::Int(2026)),
            ("afståelsesdato.måned", Data::Int(12)),
            ("afståelsesdato.dag", Data::Int(31)),
            (
                "afståelse",
                Data::String("EblAlmindeligAfståelse".to_string()),
            ),
            ("erhvervet_som_led_i_næring", Data::Bool(false)),
            ("kontant_anskaffelsessum_kroner", Data::Int(200_000)),
            ("gæld_kursværdi_ved_anskaffelse_kroner", Data::Int(0)),
            ("par5_fakta.anskaffelsesdato.år", Data::Int(1985)),
            ("par5_fakta.anskaffelsesdato.måned", Data::Int(1)),
            ("par5_fakta.anskaffelsesdato.dag", Data::Int(1)),
            (
                "par5_fakta.anskaffelsesgrundlag.$variant",
                Data::String("EblPar4Stk3Nr1Anskaffelsesgrundlag".to_string()),
            ),
            ("par5_fakta.fordeling.ejerandel_promille", Data::Int(500)),
            (
                "par5_fakta.fordeling.afståelsesomfang.$variant",
                Data::String("EblPar5DelAfEjendommen".to_string()),
            ),
            (
                "par5_fakta.fordeling.afståelsesomfang.EblPar5DelAfEjendommen.afstået_del_anskaffelsessum_før_par5_kroner",
                Data::Int(200_000),
            ),
            (
                "par5_fakta.fordeling.afståelsesomfang.EblPar5DelAfEjendommen.hele_ejendommens_anskaffelsessum_før_par5_kroner",
                Data::Int(800_000),
            ),
            (
                "par5_fakta.fordeling.afståelsesomfang.EblPar5DelAfEjendommen.ikke_boligdelens_anskaffelsessum_før_par5_kroner",
                Data::Int(800_000),
            ),
            (
                "par5_fakta.stk6_overførsel.$variant",
                Data::String("EblPar5MedStk6Overførsel".to_string()),
            ),
            (
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.ejendomskategori",
                Data::String("EblPar5Stk6Landbrugsejendom".to_string()),
            ),
            (
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.tillægsparcelværdi_kroner",
                Data::Int(500_000),
            ),
            (
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.teknisk_værdi_kroner",
                Data::Int(600_000),
            ),
            (
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.afskrivninger_på_vurderingsomfattede_bygninger_kroner",
                Data::Int(100_000),
            ),
            (
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.stk4_nr2_afskrivninger_på_vurderingsomfattede_nedrevne_bygninger_kroner",
                Data::Int(50_000),
            ),
            (
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.nedskrevet_værdi_af_vurderingsomfattede_bygninger_kroner",
                Data::Int(150_000),
            ),
            (
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.afstået_jord_anskaffelsessum_før_overførsel_kroner",
                Data::Int(200_000),
            ),
            (
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.samlet_jord_anskaffelsessum_før_overførsel_kroner",
                Data::Int(800_000),
            ),
            (
                "par5_fakta.reguleringsvalg.$variant",
                Data::String("EblPar5UdenIndeksering".to_string()),
            ),
            ("par4_stk8_anskaffelse_udeladt_kroner", Data::Int(0)),
            ("kontant_afståelsessum_kroner", Data::Int(500_000)),
            ("overdragne_gældsposter_kursværdi_kroner", Data::Int(0)),
            ("par4_stk8_afståelsesværdi_udeladt_kroner", Data::Int(0)),
            (
                "par11_stk2_valg.$variant",
                Data::String("EblPar11Stk2IngenNyGenanbringelse".to_string()),
            ),
            (
                "ejendomstype.$variant",
                Data::String("EblLandbrugSkovNaturEllerBlandetEjendom".to_string()),
            ),
            (
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.kategori.$variant",
                Data::String("EblPar9Landbrugsejendom".to_string()),
            ),
            (
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.bolig_har_tjent_til_bolig_for_ejer_eller_husstand",
                Data::Bool(false),
            ),
            (
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.boligbetingelser_opfyldt_i_ejerperioden",
                Data::Bool(false),
            ),
            (
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.grundbetingelse",
                Data::String("EblPar8GrundbetingelseIkkeOpfyldt".to_string()),
            ),
            (
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.bolig_anskaffelsessum_kroner",
                Data::Int(0),
            ),
            (
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.bolig_afståelsessum_kroner",
                Data::Int(0),
            ),
            (
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.skadeforhold.$variant",
                Data::String("EblPar9IngenVæsentligSkade".to_string()),
            ),
            (
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.genanbringelsesforhold.$variant",
                Data::String("EblPar9IngenGenanbringelseEfterStk4".to_string()),
            ),
            (
                "par6d_valg.$variant",
                Data::String("EblUdenPar6DValg".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &own_property_sheet, 3, header, value);
        }
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-ebl6d-historisk-2026".to_string()),
            ),
            ("item_id", Data::String("ebl6d-salg-2025".to_string())),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String("erhvervsejendom-solgt-2025".to_string()),
            ),
            ("afståelsesdato.år", Data::Int(2025)),
            ("afståelsesdato.måned", Data::Int(12)),
            ("afståelsesdato.dag", Data::Int(31)),
            (
                "afståelse",
                Data::String("EblAlmindeligAfståelse".to_string()),
            ),
            ("erhvervet_som_led_i_næring", Data::Bool(false)),
            ("kontant_anskaffelsessum_kroner", Data::Int(25_000_000)),
            ("gæld_kursværdi_ved_anskaffelse_kroner", Data::Int(0)),
            ("par5_fakta.anskaffelsesdato.år", Data::Int(2025)),
            ("par5_fakta.anskaffelsesdato.måned", Data::Int(1)),
            ("par5_fakta.anskaffelsesdato.dag", Data::Int(1)),
            (
                "par5_fakta.anskaffelsesgrundlag.$variant",
                Data::String("EblPar4AlmindeligtAnskaffelsesgrundlag".to_string()),
            ),
            ("par5_fakta.fordeling.ejerandel_promille", Data::Int(1000)),
            (
                "par5_fakta.fordeling.afståelsesomfang.$variant",
                Data::String("EblPar5HeleEjendommen".to_string()),
            ),
            (
                "par5_fakta.stk6_overførsel.$variant",
                Data::String("EblPar5UdenStk6Overførsel".to_string()),
            ),
            (
                "par5_fakta.reguleringsvalg.$variant",
                Data::String("EblPar5UdenIndeksering".to_string()),
            ),
            ("par4_stk8_anskaffelse_udeladt_kroner", Data::Int(0)),
            ("kontant_afståelsessum_kroner", Data::Int(30_000_000)),
            ("overdragne_gældsposter_kursværdi_kroner", Data::Int(0)),
            ("par4_stk8_afståelsesværdi_udeladt_kroner", Data::Int(0)),
            (
                "par11_stk2_valg.$variant",
                Data::String("EblPar11Stk2IngenNyGenanbringelse".to_string()),
            ),
            (
                "ejendomstype.$variant",
                Data::String("EblAndenFastEjendom".to_string()),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.$variant",
                Data::String("EblUdenAktivGenanbringelse".to_string()),
            ),
            (
                "par6d_valg.$variant",
                Data::String("EblMedPar6DValg".to_string()),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.valgt_udskudt_fortjeneste_kroner",
                Data::Int(3_000_000),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.årligt_beløb_kroner",
                Data::Int(300_000),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.fordelingsår",
                Data::Int(10),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.erhververen_erhvervede_ejendommen_som_led_i_næring",
                Data::Bool(false),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.partrelation.overdragerens_kapitalandel_i_erhververen_promille",
                Data::Int(0),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.partrelation.overdragerens_stemmeandel_i_erhververen_promille",
                Data::Int(0),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.partrelation.erhververens_kapitalandel_i_overdrageren_promille",
                Data::Int(0),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.partrelation.erhververens_stemmeandel_i_overdrageren_promille",
                Data::Int(0),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.partrelation.parterne_er_nærtstående_efter_ligningslovens_par2_stk2",
                Data::Bool(false),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.partrelation.parterne_har_aftale_om_fælles_bestemmende_indflydelse",
                Data::Bool(false),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.partrelation.parterne_er_under_fælles_bestemmende_indflydelse",
                Data::Bool(false),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.overdragerens_anvendelse.$variant",
                Data::String(
                    "EblPar6DEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed".to_string(),
                ),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.erhververens_anvendelse_ved_overdragelsen.$variant",
                Data::String(
                    "EblPar6DEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed".to_string(),
                ),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.sælgerpantebrev.identifikation",
                Data::String("sælgerpantebrev-2025".to_string()),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.sælgerpantebrev.fordring_mod_erhververen",
                Data::Bool(true),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.sælgerpantebrev.pant_i_den_overdragne_ejendom",
                Data::Bool(true),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.sælgerpantebrev.kontantværdi_kroner",
                Data::Int(3_000_000),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.sælgerpantebrev.hovedstol_kroner",
                Data::Int(3_750_000),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.sælgerpantebrev.løbetid_år",
                Data::Int(10),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.meddelelse.$variant",
                Data::String("EblPar6DRettidigMeddelelse".to_string()),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.meddelelse.EblPar6DRettidigMeddelelse.indhold.kopi_af_sælgerpantebrev_vedlagt",
                Data::Bool(true),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.meddelelse.EblPar6DRettidigMeddelelse.indhold.oplyst_skattepligtig_fortjeneste_efter_par6_kroner",
                Data::Int(4_990_000),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.meddelelse.EblPar6DRettidigMeddelelse.indhold.oplyst_valgt_udskudt_fortjeneste_kroner",
                Data::Int(3_000_000),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.meddelelse.EblPar6DRettidigMeddelelse.indhold.oplyst_årligt_beløb_kroner",
                Data::Int(300_000),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.meddelelse.EblPar6DRettidigMeddelelse.indhold.oplyst_antal_år",
                Data::Int(10),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.sælgers_skatteforhold.skattepligtsstatus",
                Data::String("EblPar6DFuldtSkattepligtigTilDanmark".to_string()),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.sælgers_skatteforhold.hjemland_omfattet_af_nordisk_overenskomst_eller_eu_inddrivelsesdirektiv",
                Data::Bool(true),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.sælgers_skatteforhold.sikkerhed.$variant",
                Data::String("EblPar6DIngenSikkerhed".to_string()),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.ejendomsplacering.$variant",
                Data::String("EblPar6DEjendomIDanmark".to_string()),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.afståelsesårets_hændelse.$variant",
                Data::String("EblPar6DIngenFremrykningshændelse".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &own_property_sheet, 4, header, value);
        }
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-ebl11-genanbringelse-2026".to_string()),
            ),
            ("item_id", Data::String("ebl11-salg-2026".to_string())),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String("eksproprieret-genanbringelsesejendom".to_string()),
            ),
            ("afståelsesdato.år", Data::Int(2026)),
            ("afståelsesdato.måned", Data::Int(12)),
            ("afståelsesdato.dag", Data::Int(31)),
            (
                "afståelse",
                Data::String("EblEkspropriationserstatning".to_string()),
            ),
            ("erhvervet_som_led_i_næring", Data::Bool(false)),
            ("kontant_anskaffelsessum_kroner", Data::Int(1_000_000)),
            ("gæld_kursværdi_ved_anskaffelse_kroner", Data::Int(0)),
            ("par5_fakta.anskaffelsesdato.år", Data::Int(2026)),
            ("par5_fakta.anskaffelsesdato.måned", Data::Int(1)),
            ("par5_fakta.anskaffelsesdato.dag", Data::Int(1)),
            (
                "par5_fakta.anskaffelsesgrundlag.$variant",
                Data::String("EblPar4AlmindeligtAnskaffelsesgrundlag".to_string()),
            ),
            ("par5_fakta.fordeling.ejerandel_promille", Data::Int(1000)),
            (
                "par5_fakta.fordeling.afståelsesomfang.$variant",
                Data::String("EblPar5HeleEjendommen".to_string()),
            ),
            (
                "par5_fakta.stk6_overførsel.$variant",
                Data::String("EblPar5UdenStk6Overførsel".to_string()),
            ),
            (
                "par5_fakta.reguleringsvalg.$variant",
                Data::String("EblPar5UdenIndeksering".to_string()),
            ),
            ("par4_stk8_anskaffelse_udeladt_kroner", Data::Int(0)),
            ("kontant_afståelsessum_kroner", Data::Int(1_500_000)),
            ("overdragne_gældsposter_kursværdi_kroner", Data::Int(0)),
            ("par4_stk8_afståelsesværdi_udeladt_kroner", Data::Int(0)),
            (
                "par11_stk2_valg.$variant",
                Data::String("EblPar11Stk2IngenNyGenanbringelse".to_string()),
            ),
            (
                "ejendomstype.$variant",
                Data::String("EblAndenFastEjendom".to_string()),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.$variant",
                Data::String("EblMedGenanbringelseEfterPar6A".to_string()),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.afståelsesindkomstår",
                Data::Int(2025),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.erhvervsfortjeneste_før_par6_stk2_kroner",
                Data::Int(200_000),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.regulering.tillæg_for_genanbragt_del_kroner",
                Data::Int(0),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.regulering.nedslag_for_genanbragt_del_kroner",
                Data::Int(0),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.afstået_ejendoms_erhvervsanvendelse.$variant",
                Data::String(
                    "EblPar6AEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed".to_string(),
                ),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.indkomstår",
                Data::Int(2026),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.erhvervsmæssigt_anskaffelsesgrundlag_kroner",
                Data::Int(1_000_000),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.ejendomsstatus",
                Data::String("EblPar6AEjendomOmfattetAfLovenIkkePar8".to_string()),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.erhvervsanvendelse.$variant",
                Data::String(
                    "EblPar6AEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed".to_string(),
                ),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.placering.$variant",
                Data::String("EblPar6AEjendomIDanmark".to_string()),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.begæring.$variant",
                Data::String(
                    "EblPar6ABegæringVedRettidigAfgivelseEfterSkattekontrollovensPar2"
                        .to_string(),
                ),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.ejerskab.$variant",
                Data::String("EblPar6ASammeSkattepligtige".to_string()),
            ),
            (
                "par6d_valg.$variant",
                Data::String("EblUdenPar6DValg".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &own_property_sheet, 5, header, value);
        }
        let own_par6d_schedule_path = "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.egne_afståelser.par6d_valg.EblMedPar6DValg.fakta.sælgerpantebrev.afdragsplan";
        let own_par6d_schedule_sheet =
            workbook_collection_sheet_name_from_rows(sheets, own_par6d_schedule_path);
        for year in 1_i64..=10 {
            for (header, value) in [
                (
                    "case_id",
                    Data::String("personskat-ebl6d-historisk-2026".to_string()),
                ),
                ("parent_id", Data::String("ebl6d-salg-2025".to_string())),
                ("item_id", Data::String(format!("ebl6d-afdrag-{year}"))),
                ("position", Data::Int(year)),
                ("år_efter_afståelsen", Data::Int(year)),
                ("hovedstol_forfalder_kroner", Data::Int(375_000)),
            ] {
                set_workbook_cell_by_header(
                    sheets,
                    &own_par6d_schedule_sheet,
                    year as usize,
                    header,
                    value,
                );
            }
        }
        let own_par6d_years_path = "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.egne_afståelser.par6d_valg.EblMedPar6DValg.fakta.efterfølgende_årsforhold";
        let own_par6d_years_sheet =
            workbook_collection_sheet_name_from_rows(sheets, own_par6d_years_path);
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-ebl6d-historisk-2026".to_string()),
            ),
            ("parent_id", Data::String("ebl6d-salg-2025".to_string())),
            ("item_id", Data::String("ebl6d-år-2026".to_string())),
            ("position", Data::Int(1)),
            ("indkomstår", Data::Int(2026)),
        ] {
            set_workbook_cell_by_header(sheets, &own_par6d_years_sheet, 1, header, value);
        }
        let own_par6d_posts_path = "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.egne_afståelser.par6d_valg.EblMedPar6DValg.fakta.efterfølgende_årsforhold.forløbsposter";
        let own_par6d_posts_sheet =
            workbook_collection_sheet_name_from_rows(sheets, own_par6d_posts_path);
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-ebl6d-historisk-2026".to_string()),
            ),
            ("parent_id", Data::String("ebl6d-år-2026".to_string())),
            (
                "item_id",
                Data::String("ebl6d-år-2026-afdrag-1".to_string()),
            ),
            ("position", Data::Int(1)),
            (
                "$variant",
                Data::String("EblPar6DOrdinærtHovedstolsafdrag".to_string()),
            ),
            (
                "EblPar6DOrdinærtHovedstolsafdrag.fordringsdel_identifikation",
                Data::String("sælgerpantebrev-2025".to_string()),
            ),
            (
                "EblPar6DOrdinærtHovedstolsafdrag.betalt_kroner",
                Data::Int(375_000),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &own_par6d_posts_sheet, 1, header, value);
        }
        let kgl_seller_note_sheet = workbook_collection_sheet_name_from_rows(
            sheets,
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.sælgerpantebreve",
        );
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-ebl6d-historisk-2026".to_string()),
            ),
            (
                "item_id",
                Data::String("kgl-sælgerpantebrev-2025".to_string()),
            ),
            ("position", Data::Int(1)),
            (
                "sælgerpantebrev_identifikation",
                Data::String("sælgerpantebrev-2025".to_string()),
            ),
            (
                "oprindelig_skatteyder_identifikation",
                Data::String("Sælger".to_string()),
            ),
            (
                "skatteyderfakta.udøver_næring_ved_køb_og_salg_af_fordringer",
                Data::Bool(false),
            ),
            (
                "skatteyderfakta.fordringen_erhvervet_uden_for_fordringsnæring",
                Data::Bool(true),
            ),
            (
                "skatteyderfakta.fordringen_erhvervet_som_vederlag_for_leverede_varer_eller_tjenesteydelser",
                Data::Bool(false),
            ),
            (
                "skatteyderfakta.fordringen_erhvervet_i_direkte_tilknytning_til_erhvervsmæssig_drift",
                Data::Bool(false),
            ),
            (
                "skatteyderfakta.debitor_omfattet_af_tabsbegrænsningen_i_kgl_par14_stk2",
                Data::Bool(false),
            ),
            (
                "skatteyderfakta.renter_eller_gevinster_fritaget_efter_dobbeltbeskatningsoverenskomst",
                Data::Bool(false),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &kgl_seller_note_sheet, 1, header, value);
        }
        let kgl_debt_sheet = workbook_collection_sheet_name_from_rows(
            sheets,
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.gældsposter",
        );
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-kgl-gaeld-2026".to_string()),
            ),
            ("item_id", Data::String("usd-laan-1".to_string())),
            ("position", Data::Int(1)),
            ("identifikation", Data::String("USD-lån".to_string())),
            (
                "beløb.gældens_værdi_ved_påtagelse_kroner",
                Data::Int(100_000),
            ),
            (
                "beløb.gældens_værdi_ved_frigørelse_eller_indfrielse_kroner",
                Data::Int(97_000),
            ),
            (
                "beløb.fordringens_værdi_for_kreditor_kroner",
                Data::Int(97_000),
            ),
            (
                "frigørelsesart",
                Data::String("KglGældOrdinærIndfrielse".to_string()),
            ),
            (
                "erhvervsforhold",
                Data::String("KglGældUdenFinansieringsnæring".to_string()),
            ),
            ("valuta", Data::String("KglGældFremmedValuta".to_string())),
            (
                "selskabsfakta.$variant",
                Data::String("KglIngenPar21Stk2Selskabsgæld".to_string()),
            ),
            (
                "gældsordning.$variant",
                Data::String("KglIngenDokumenteretGældsordning".to_string()),
            ),
            ("vedrører_ikke_indbetalt_selskabskapital", Data::Bool(false)),
            (
                "par22_hændelse.$variant",
                Data::String("KglIngenPar22Hændelse".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &kgl_debt_sheet, 1, header, value);
        }
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-kgl-frivillig-ordning-2026".to_string()),
            ),
            ("item_id", Data::String("frivillig-gaeld-1".to_string())),
            ("position", Data::Int(1)),
            ("identifikation", Data::String("hovedkrav-82".to_string())),
            (
                "beløb.gældens_værdi_ved_påtagelse_kroner",
                Data::Int(820_800),
            ),
            (
                "beløb.gældens_værdi_ved_frigørelse_eller_indfrielse_kroner",
                Data::Int(150_000),
            ),
            (
                "beløb.fordringens_værdi_for_kreditor_kroner",
                Data::Int(200_000),
            ),
            (
                "frigørelsesart",
                Data::String("KglGældEftergivelse".to_string()),
            ),
            (
                "erhvervsforhold",
                Data::String("KglGældUdenFinansieringsnæring".to_string()),
            ),
            ("valuta", Data::String("KglGældDanskeKroner".to_string())),
            (
                "selskabsfakta.$variant",
                Data::String("KglIngenPar21Stk2Selskabsgæld".to_string()),
            ),
            (
                "gældsordning.$variant",
                Data::String("KglFrivilligKreditorordning".to_string()),
            ),
            (
                "gældsordning.KglFrivilligKreditorordning.fakta.ordningsidentifikation",
                Data::String("skm2017-10-moenster".to_string()),
            ),
            (
                "gældsordning.KglFrivilligKreditorordning.fakta.alle_usikrede_krav_oplyst",
                Data::Bool(true),
            ),
            ("vedrører_ikke_indbetalt_selskabskapital", Data::Bool(false)),
            (
                "par22_hændelse.$variant",
                Data::String("KglIngenPar22Hændelse".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &kgl_debt_sheet, 2, header, value);
        }
        let kgl_voluntary_claim_sheet = workbook_collection_sheet_name_from_rows(
            sheets,
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.gældsposter.gældsordning.KglFrivilligKreditorordning.fakta.krav",
        );
        for (row, item_id, claim_id, creditor_id, amount, participation, remainder) in [
            (
                1,
                "frivillig-gaeld-1-krav-1",
                "hovedkrav-82",
                "hovedkreditor",
                820_800,
                "KglKreditorTiltrådtFrivilligOrdning",
                Some(150_000),
            ),
            (
                2,
                "frivillig-gaeld-1-krav-2",
                "småkrav-347",
                "kreditor-347",
                34_700,
                "KglKreditorUdenforFrivilligOrdning",
                None,
            ),
            (
                3,
                "frivillig-gaeld-1-krav-3",
                "småkrav-717",
                "kreditor-717",
                71_700,
                "KglKreditorUdenforFrivilligOrdning",
                None,
            ),
            (
                4,
                "frivillig-gaeld-1-krav-4",
                "småkrav-636",
                "kreditor-636",
                63_600,
                "KglKreditorUdenforFrivilligOrdning",
                None,
            ),
            (
                5,
                "frivillig-gaeld-1-krav-5",
                "småkrav-92",
                "kreditor-92",
                9_200,
                "KglKreditorUdenforFrivilligOrdning",
                None,
            ),
        ] {
            for (header, value) in [
                (
                    "case_id",
                    Data::String("personskat-kgl-frivillig-ordning-2026".to_string()),
                ),
                ("parent_id", Data::String("frivillig-gaeld-1".to_string())),
                ("item_id", Data::String(item_id.to_string())),
                ("position", Data::Int(row as i64)),
                ("krav_identifikation", Data::String(claim_id.to_string())),
                (
                    "kreditor_identifikation",
                    Data::String(creditor_id.to_string()),
                ),
                ("samlet_krav_kroner", Data::Int(amount)),
                ("værdi_af_tilstrækkelig_sikkerhed_kroner", Data::Int(0)),
                (
                    "deltagelse.$variant",
                    Data::String(participation.to_string()),
                ),
            ] {
                set_workbook_cell_by_header(sheets, &kgl_voluntary_claim_sheet, row, header, value);
            }
            if let Some(remainder) = remainder {
                set_workbook_cell_by_header(
                    sheets,
                    &kgl_voluntary_claim_sheet,
                    row,
                    "deltagelse.KglKreditorTiltrådtFrivilligOrdning.aftalt_restkrav_kroner",
                    Data::Int(remainder),
                );
            } else {
                for (header, value) in [
                    (
                        "deltagelse.KglKreditorUdenforFrivilligOrdning.småkravsgrundlag.$variant",
                        Data::String(
                            "KglUdeladtKravDokumenteretSomSmåkrav".to_string(),
                        ),
                    ),
                    (
                        "deltagelse.KglKreditorUdenforFrivilligOrdning.småkravsgrundlag.KglUdeladtKravDokumenteretSomSmåkrav.afgørelsesreference",
                        Data::String("SKM2017.10.SR".to_string()),
                    ),
                ] {
                    set_workbook_cell_by_header(
                        sheets,
                        &kgl_voluntary_claim_sheet,
                        row,
                        header,
                        value,
                    );
                }
            }
        }
        let own_milk_quota_sheet = workbook_collection_sheet_name_from_rows(
            sheets,
            "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.egne_afståelser.par5_fakta.mælkekvoter",
        );
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-ebl5-kildefakta-2026".to_string()),
            ),
            ("parent_id", Data::String("ebl5-landbrug-1".to_string())),
            ("item_id", Data::String("mælkekvote-1".to_string())),
            ("position", Data::Int(1)),
            ("identifikation", Data::String("mælk-2000".to_string())),
            ("anskaffelsesdato.år", Data::Int(2000)),
            ("anskaffelsesdato.måned", Data::Int(1)),
            ("anskaffelsesdato.dag", Data::Int(1)),
            ("oprindelige_enheder", Data::Int(100)),
            ("disponerede_enheder", Data::Int(100)),
            (
                "anskaffelsesgrundlag.$variant",
                Data::String("EblPar5MælkekvoteKøbt".to_string()),
            ),
            (
                "anskaffelsesgrundlag.EblPar5MælkekvoteKøbt.vederlag_kroner",
                Data::Int(80_000),
            ),
            (
                "disposition.$variant",
                Data::String("EblPar5MælkekvoteAfstået".to_string()),
            ),
            (
                "disposition.EblPar5MælkekvoteAfstået.afståelsesdato.år",
                Data::Int(2010),
            ),
            (
                "disposition.EblPar5MælkekvoteAfstået.afståelsesdato.måned",
                Data::Int(6),
            ),
            (
                "disposition.EblPar5MælkekvoteAfstået.afståelsesdato.dag",
                Data::Int(1),
            ),
            (
                "disposition.EblPar5MælkekvoteAfstået.vederlag_kroner",
                Data::Int(120_000),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &own_milk_quota_sheet, 1, header, value);
        }
        for (header, value) in [
            (
                "ejendomstype.$variant",
                Data::String("EblBoligejendom".to_string()),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.ejendomsart.$variant",
                Data::String("EblPar8Ejerlejlighed".to_string()),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.tjent_til_bolig_eller_privat_formål",
                Data::Bool(true),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.betingelser_opfyldt_i_ejerperioden",
                Data::Bool(true),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.afståelsesforhold.$variant",
                Data::String("EblPar8AlmindeligAfståelse".to_string()),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.$variant",
                Data::String("EblPar8GenanbringelseEfterStk5".to_string()),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.$variant",
                Data::String("EblMedGenanbringelseEfterPar6A".to_string()),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.afståelsesindkomstår",
                Data::Int(2025),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.erhvervsfortjeneste_før_par6_stk2_kroner",
                Data::Int(190_000),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.regulering.tillæg_for_genanbragt_del_kroner",
                Data::Int(0),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.regulering.nedslag_for_genanbragt_del_kroner",
                Data::Int(0),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.afstået_ejendoms_erhvervsanvendelse.$variant",
                Data::String(
                    "EblPar6AEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed".to_string(),
                ),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.indkomstår",
                Data::Int(2026),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.erhvervsmæssigt_anskaffelsesgrundlag_kroner",
                Data::Int(1_000_000),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.ejendomsstatus",
                Data::String("EblPar6AEjendomOmfattetAfLovenIkkePar8".to_string()),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.erhvervsanvendelse.$variant",
                Data::String(
                    "EblPar6AEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed".to_string(),
                ),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.placering.$variant",
                Data::String("EblPar6AEjendomIDanmark".to_string()),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.begæring.$variant",
                Data::String(
                    "EblPar6ABegæringVedRettidigAfgivelseEfterSkattekontrollovensPar2"
                        .to_string(),
                ),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.ejerskab.$variant",
                Data::String("EblPar6ASammeSkattepligtige".to_string()),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.anvendelsesændring.ændringsdato.år",
                Data::Int(2026),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.anvendelsesændring.ændringsdato.måned",
                Data::Int(6),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.anvendelsesændring.ændringsdato.dag",
                Data::Int(1),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &own_property_sheet, 1, header, value);
        }
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-renter-befordring-2026".to_string()),
            ),
            ("item_id", Data::String("ægtefælles-tab-1".to_string())),
            ("position", Data::Int(1)),
            ("identifikation", Data::String("ægtefælles-tab".to_string())),
            ("afståelsesdato.år", Data::Int(2026)),
            ("afståelsesdato.måned", Data::Int(12)),
            ("afståelsesdato.dag", Data::Int(31)),
            (
                "afståelse",
                Data::String("EblAlmindeligAfståelse".to_string()),
            ),
            ("erhvervet_som_led_i_næring", Data::Bool(false)),
            ("kontant_anskaffelsessum_kroner", Data::Int(300_000)),
            ("gæld_kursværdi_ved_anskaffelse_kroner", Data::Int(0)),
            ("par5_fakta.anskaffelsesdato.år", Data::Int(2026)),
            ("par5_fakta.anskaffelsesdato.måned", Data::Int(1)),
            ("par5_fakta.anskaffelsesdato.dag", Data::Int(1)),
            (
                "par5_fakta.anskaffelsesgrundlag.$variant",
                Data::String("EblPar4AlmindeligtAnskaffelsesgrundlag".to_string()),
            ),
            ("par5_fakta.fordeling.ejerandel_promille", Data::Int(1000)),
            (
                "par5_fakta.fordeling.afståelsesomfang.$variant",
                Data::String("EblPar5HeleEjendommen".to_string()),
            ),
            (
                "par5_fakta.stk6_overførsel.$variant",
                Data::String("EblPar5UdenStk6Overførsel".to_string()),
            ),
            (
                "par5_fakta.reguleringsvalg.$variant",
                Data::String("EblPar5UdenIndeksering".to_string()),
            ),
            ("par4_stk8_anskaffelse_udeladt_kroner", Data::Int(0)),
            ("kontant_afståelsessum_kroner", Data::Int(270_000)),
            ("overdragne_gældsposter_kursværdi_kroner", Data::Int(0)),
            ("par4_stk8_afståelsesværdi_udeladt_kroner", Data::Int(0)),
            (
                "par11_stk2_valg.$variant",
                Data::String("EblPar11Stk2IngenNyGenanbringelse".to_string()),
            ),
            (
                "ejendomstype.$variant",
                Data::String("EblAndenFastEjendom".to_string()),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.$variant",
                Data::String("EblUdenAktivGenanbringelse".to_string()),
            ),
            (
                "par6d_valg.$variant",
                Data::String("EblUdenPar6DValg".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &spouse_property_sheet, 1, header, value);
        }
        for (header, value) in [
            (
                "kapitalindkomst.renter.renteindtægter_kroner",
                Data::Int(20_000),
            ),
            (
                "kapitalindkomst.renter.renteudgifter_kroner",
                Data::Int(5_000),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.$variant",
                Data::String("MedLigningslov6Kurstab".to_string()),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.indkomstår",
                Data::Int(2026),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.kontantlån_optaget_i_realkreditinstitut_før_19_maj_1993",
                Data::Bool(true),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.indfrielse_sker_ved_realkreditlån",
                Data::Bool(true),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.nyt_lån_optaget_før_1_januar_1996",
                Data::Bool(true),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.nyt_lån_mindst_samme_løbetid",
                Data::Bool(true),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.transaktioner_inden_for_1_år",
                Data::Bool(true),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.lånetilbud_før_indfrielse_hvis_indfrielse_før_optagelse",
                Data::Bool(true),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.kurstab_kroner",
                Data::Int(24_000),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.samlet_antal_terminer",
                Data::Int(12),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.forfaldne_terminer_i_indkomståret",
                Data::Int(1),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.ekstraordinær_indfrielse_af_nyt_lån",
                Data::Bool(false),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.stk4_omlægning_undtager_stk3",
                Data::Bool(false),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.stk3_nedsættelse_basispoint",
                Data::Int(0),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.debitorskifte_i_året",
                Data::Bool(false),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.debitordage_for_skattepligtig",
                Data::Int(0),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.dage_i_overdragelsesår",
                Data::Int(0),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.kurstab_medregnes_efter_kursgevinstloven",
                Data::Bool(false),
            ),
            (
                "kapitalindkomst.renter.ligningslov6a.$variant",
                Data::String("MedLigningslov6AFradrag".to_string()),
            ),
            (
                "kapitalindkomst.renter.ligningslov6a.MedLigningslov6AFradrag.input.indkomstår",
                Data::Int(2026),
            ),
            (
                "kapitalindkomst.renter.ligningslov6a.MedLigningslov6AFradrag.input.skattepligtig_person",
                Data::Bool(true),
            ),
            (
                "kapitalindkomst.renter.ligningslov6a.MedLigningslov6AFradrag.input.arbejderboliger_beløb_kroner",
                Data::Int(1_000),
            ),
            (
                "kapitalindkomst.renter.ligningslov6a.MedLigningslov6AFradrag.input.arbejderboliger_betalt",
                Data::Bool(true),
            ),
            (
                "kapitalindkomst.renter.ligningslov6a.MedLigningslov6AFradrag.input.statshusmandsbrug_jordrente_beløb_kroner",
                Data::Int(0),
            ),
            (
                "kapitalindkomst.renter.ligningslov6a.MedLigningslov6AFradrag.input.statshusmandsbrug_jordrente_betalt",
                Data::Bool(false),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "cases", 3, header, value);
        }
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-renter-befordring-2026".to_string()),
            ),
            ("item_id", Data::String("kapitalomkostning-1".to_string())),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String("dokumenteret-kapitalomkostning-1".to_string()),
            ),
            ("anvendelsesår", Data::Int(2026)),
            (
                "anvendelse",
                Data::String("Par4Stk2ErhverveKapitalindkomst".to_string()),
            ),
            (
                "omkostningsart",
                Data::String("Ll17CAndenOmkostning".to_string()),
            ),
            ("næringsstatus", Data::String("IkkeNæring".to_string())),
            ("beløb_kroner", Data::Int(2_000)),
        ] {
            set_workbook_cell_by_header(sheets, "kapitalindkomst_omkostninger", 1, header, value);
        }
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-abl-personlig-2026".to_string()),
            ),
            ("item_id", Data::String("abl-par17-1".to_string())),
            ("position", Data::Int(1)),
            ("identifikation", Data::String("abl-par17-1".to_string())),
            (
                "kilde.$variant",
                Data::String("PersonskatAktieaktivEfterPar17".to_string()),
            ),
            (
                "kilde.PersonskatAktieaktivEfterPar17.fakta.indkomstår",
                Data::Int(2026),
            ),
            (
                "kilde.PersonskatAktieaktivEfterPar17.fakta.skattepligtsgrundlag",
                Data::String("AblPar7PersonEfterKildeskatteloven".to_string()),
            ),
            (
                "kilde.PersonskatAktieaktivEfterPar17.fakta.næringsstatus",
                Data::String("AblPar17UdøverNæringVedKøbOgSalgAfAktier".to_string()),
            ),
            (
                "kilde.PersonskatAktieaktivEfterPar17.fakta.instrument",
                Data::String("AblPar17AlmindeligAktie".to_string()),
            ),
            (
                "kilde.PersonskatAktieaktivEfterPar17.fakta.erhvervelsesstatus",
                Data::String("AblPar17ErhvervetSomLedINæringsvej".to_string()),
            ),
            (
                "kilde.PersonskatAktieaktivEfterPar17.fakta.afståelsessum_kroner",
                Data::Int(37_000),
            ),
            (
                "kilde.PersonskatAktieaktivEfterPar17.fakta.anskaffelsessum_kroner",
                Data::Int(30_000),
            ),
            (
                "markedsstatus",
                Data::String("AblIkkeOptagetTilHandel".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "aktieavance_særlige_aktiver", 1, header, value);
        }
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-par35-medarbejdereje-2026".to_string()),
            ),
            ("item_id", Data::String("par35-forloeb-1".to_string())),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String("personskat-par35-2026".to_string()),
            ),
            (
                "kilde.$variant",
                Data::String("PersonskatMedarbejderejeordningEfterPar35G".to_string()),
            ),
            (
                "kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.identifikation",
                Data::String("personskat-par35-2026".to_string()),
            ),
            (
                "kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.opgørelsesår",
                Data::Int(2026),
            ),
            (
                "kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.overdragelse.overdragelsesår",
                Data::Int(2026),
            ),
            (
                "kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.overdragelse.overdrager_er_fysisk_person",
                Data::Bool(true),
            ),
            (
                "kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.overdragelse.hjemsted",
                Data::String("AblPar35GDanmark".to_string()),
            ),
            (
                "kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.overdragelse.dansk_virksomhed_omfattet_af_sel_par1_stk1_nr2j",
                Data::Bool(true),
            ),
            (
                "kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.overdragelse.udenlandsk_virksomhed_svarer_til_sel_par1_stk1_nr2j",
                Data::Bool(false),
            ),
            (
                "kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.overdragelse.udenlandsk_virksomhed_opfylder_erhvervsvirksomhedslov_kap5c",
                Data::Bool(false),
            ),
            (
                "kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.overdragelse.udenlandsk_virksomhed_forpligter_sig_til_overdragerskat",
                Data::Bool(false),
            ),
            (
                "kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.overdragelse.udenlandsk_virksomhed_forpligter_sig_til_årsoplysninger",
                Data::Bool(false),
            ),
            (
                "kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.overdragelse.aktier_opfylder_par34_stk1_nr3",
                Data::Bool(true),
            ),
            (
                "kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.overdragelse.parterne_har_valgt_ordningen",
                Data::Bool(true),
            ),
            (
                "kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.overdragelse.meddelelse_rettidig",
                Data::Bool(true),
            ),
            (
                "kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.overdragelse.beholdningsoversigt_vedlagt",
                Data::Bool(true),
            ),
            (
                "kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.overdragelse.saldo_vedlagt",
                Data::Bool(true),
            ),
            (
                "kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.overdragelse.land_omfattet_af_inddrivelsesbistand",
                Data::Bool(true),
            ),
            (
                "kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.overdragelse.sikkerhedsform",
                Data::String("AblPar35GIngenSikkerhed".to_string()),
            ),
            (
                "kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.overdragelse.sikkerhed_står_i_passende_forhold",
                Data::Bool(false),
            ),
            (
                "markedsstatus",
                Data::String("AblIkkeOptagetTilHandel".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "aktieavance_særlige_aktiver", 2, header, value);
        }
        let par35_parties_path = "aktieavance.særlige_aktiver.kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.overdragelse.partier";
        let par35_parties_sheet =
            workbook_collection_sheet_name_from_rows(sheets, par35_parties_path);
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-par35-medarbejdereje-2026".to_string()),
            ),
            ("parent_id", Data::String("par35-forloeb-1".to_string())),
            ("item_id", Data::String("par35-parti-1".to_string())),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String("personskat-par35-negativt-parti".to_string()),
            ),
            (
                "selskabsidentifikation",
                Data::String("DK-PERSONSKAT-PAR35".to_string()),
            ),
            ("aktieserie", Data::String("ordinær".to_string())),
            ("erhvervelsesrækkefølge", Data::Int(1)),
            ("antal", Data::Int(100)),
            ("skattemæssig_anskaffelsessum_kroner", Data::Int(-50_000)),
            ("handelsværdi_kroner", Data::Int(100_000)),
        ] {
            set_workbook_cell_by_header(sheets, &par35_parties_sheet, 1, header, value);
        }
        let par35_events_path = "aktieavance.særlige_aktiver.kilde.PersonskatMedarbejderejeordningEfterPar35G.fakta.hændelsesposter";
        let par35_events_sheet =
            workbook_collection_sheet_name_from_rows(sheets, par35_events_path);
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-par35-medarbejdereje-2026".to_string()),
            ),
            ("parent_id", Data::String("par35-forloeb-1".to_string())),
            ("item_id", Data::String("par35-haendelse-1".to_string())),
            ("position", Data::Int(1)),
            ("rækkefølge_i_indkomståret", Data::Int(1)),
            (
                "hændelse.$variant",
                Data::String("AblPar35HændelseAfståelse".to_string()),
            ),
            (
                "hændelse.AblPar35HændelseAfståelse.data.hændelsesidentifikation",
                Data::String("personskat-par35-salg-2026".to_string()),
            ),
            (
                "hændelse.AblPar35HændelseAfståelse.data.selskabsidentifikation",
                Data::String("DK-PERSONSKAT-PAR35".to_string()),
            ),
            (
                "hændelse.AblPar35HændelseAfståelse.data.aktieserie",
                Data::String("ordinær".to_string()),
            ),
            (
                "hændelse.AblPar35HændelseAfståelse.data.antal",
                Data::Int(100),
            ),
            (
                "hændelse.AblPar35HændelseAfståelse.data.afståelsessum_kroner",
                Data::Int(120_000),
            ),
            (
                "hændelse.AblPar35HændelseAfståelse.data.indkomstår",
                Data::Int(2026),
            ),
            (
                "hændelse.AblPar35HændelseAfståelse.data.anden_betalt_skat_kroner",
                Data::Int(0),
            ),
            (
                "hændelse.AblPar35HændelseAfståelse.data.godkendt_fradrag_efter_ligningslov_par33_kroner",
                Data::Int(0),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &par35_events_sheet, 1, header, value);
        }
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-par37-40-fraflytning-2026".to_string()),
            ),
            ("item_id", Data::String("par37-40-forloeb-1".to_string())),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String("personskat-fraflytning-2026".to_string()),
            ),
            (
                "kilde.$variant",
                Data::String("PersonskatFraflytteraktierEfterPar37Til40".to_string()),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.identifikation",
                Data::String("personskat-fraflytning-2026".to_string()),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.opgørelsesår",
                Data::Int(2026),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.fraflytningsår",
                Data::Int(2026),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.ophørsgrund",
                Data::String(
                    "AblPar38OphørAfSkattepligtEfterKildeskattelovPar1".to_string(),
                ),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.tilknytning",
                Data::String("AblPar38MindstSyvÅrIndenForSenesteTiÅr".to_string()),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.kontekstgrundlag.$variant",
                Data::String("AfledPar37Til40KontekstFraPersonskat".to_string()),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.indberetning.oplysninger_efter_skattekontrollov_par2",
                Data::String("AblPar39RettidigOrdinærFrist".to_string()),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.indberetning.beholdningsoversigt_efter_par39a",
                Data::String("AblPar39RettidigOrdinærFrist".to_string()),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.bopæl.oprindeligt_fraflytningsland",
                Data::String(
                    "AblPar39LandOmfattetAfNordiskOverenskomstEllerEuDirektiv".to_string(),
                ),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.bopæl.aktuelt_land",
                Data::String(
                    "AblPar39LandOmfattetAfNordiskOverenskomstEllerEuDirektiv".to_string(),
                ),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.bopæl.frigivelse_af_sikkerhed_anmodet",
                Data::Bool(false),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.sikkerhed.$variant",
                Data::String("AblPar39IngenSikkerhedStillet".to_string()),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.tilflytning.$variant",
                Data::String("IngenTilflytningEfterPar39B".to_string()),
            ),
            (
                "markedsstatus",
                Data::String("AblIkkeOptagetTilHandel".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "aktieavance_særlige_aktiver", 3, header, value);
        }
        copy_workbook_data_row(sheets, "aktieavance_særlige_aktiver", 3, 5);
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-par37-40-aegtefaelle-2026".to_string()),
            ),
            (
                "item_id",
                Data::String("par37-40-aegtefaelle-1".to_string()),
            ),
            (
                "identifikation",
                Data::String("personskat-fraflytning-aegtefaelle-2026".to_string()),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.identifikation",
                Data::String("personskat-fraflytning-aegtefaelle-2026".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "aktieavance_særlige_aktiver", 5, header, value);
        }
        copy_workbook_data_row(sheets, "aktieavance_særlige_aktiver", 3, 6);
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-par37-40-modstridende-kontekst-2026".to_string()),
            ),
            (
                "item_id",
                Data::String("par37-40-modstridende-kontekst-1".to_string()),
            ),
            (
                "identifikation",
                Data::String("personskat-fraflytning-modstridende-2026".to_string()),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.identifikation",
                Data::String("personskat-fraflytning-modstridende-2026".to_string()),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.kontekstgrundlag.$variant",
                Data::String("HistoriskPar37Til40Aktieindkomstkontekst".to_string()),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.kontekstgrundlag.HistoriskPar37Til40Aktieindkomstkontekst.kontekst.indkomstår",
                Data::Int(2026),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.kontekstgrundlag.HistoriskPar37Til40Aktieindkomstkontekst.kontekst.øvrig_egen_aktieindkomst_kroner",
                Data::Int(999_999),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.kontekstgrundlag.HistoriskPar37Til40Aktieindkomstkontekst.kontekst.ægtefælles_aktieindkomst_kroner",
                Data::Int(999_999),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.kontekstgrundlag.HistoriskPar37Til40Aktieindkomstkontekst.kontekst.samlevende_med_ægtefælle_ved_indkomstårets_udløb",
                Data::Bool(true),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "aktieavance_særlige_aktiver", 6, header, value);
        }
        let par37_shares_path = "aktieavance.særlige_aktiver.kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.aktier";
        let par37_shares_sheet =
            workbook_collection_sheet_name_from_rows(sheets, par37_shares_path);
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-par37-40-fraflytning-2026".to_string()),
            ),
            ("parent_id", Data::String("par37-40-forloeb-1".to_string())),
            ("item_id", Data::String("par37-40-parti-1".to_string())),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String("personskat-fraflytteraktie".to_string()),
            ),
            (
                "selskabsidentifikation",
                Data::String("DK-PERSONSKAT-FRAFLYTNING".to_string()),
            ),
            ("aktieserie", Data::String("ordinær".to_string())),
            ("erhvervelsesdato.år", Data::Int(2020)),
            ("erhvervelsesdato.måned", Data::Int(4)),
            ("erhvervelsesdato.dag", Data::Int(5)),
            ("erhvervelsesrækkefølge", Data::Int(1)),
            ("antal", Data::Int(100)),
            ("handelsværdi_ved_ophør_kroner", Data::Int(200_000)),
            ("skattemæssig_anskaffelsessum_kroner", Data::Int(100_000)),
            (
                "beskatningsstatus",
                Data::String("AblPar38AktieOmfattetAfDanskBeskatning".to_string()),
            ),
            (
                "par44_status",
                Data::String("AblPar38IkkeHistoriskPar44Aktie".to_string()),
            ),
            (
                "opgørelseskilde.$variant",
                Data::String("AblPar37Til40OpgørelseEfterPar23Til29Og46".to_string()),
            ),
            (
                "aktivgrundlag.$variant",
                Data::String("AblPar38OrdinærAktieEfterPar12Til15".to_string()),
            ),
            (
                "aktivgrundlag.AblPar38OrdinærAktieEfterPar12Til15.fakta.markedsstatus",
                Data::String("AblIkkeOptagetTilHandel".to_string()),
            ),
            (
                "aktivgrundlag.AblPar38OrdinærAktieEfterPar12Til15.fakta.har_tidligere_været_optaget_til_handel",
                Data::Bool(false),
            ),
            (
                "aktivgrundlag.AblPar38OrdinærAktieEfterPar12Til15.fakta.oplysningsstatus",
                Data::String("AblOplysningsbetingelseIkkeOpfyldt".to_string()),
            ),
            (
                "aktivgrundlag.AblPar38OrdinærAktieEfterPar12Til15.fakta.par5a_kildefakta.$variant",
                Data::String("AblOrdinærIngenPar5AFaktaPåkrævet".to_string()),
            ),
            (
                "princip",
                Data::String("AblPar23Realisationsprincip".to_string()),
            ),
            (
                "henstandsvalg",
                Data::String("AblPar37Til40HenstandSøges".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &par37_shares_sheet, 1, header, value);
        }
        copy_workbook_data_row(sheets, &par37_shares_sheet, 1, 2);
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-par37-40-aegtefaelle-2026".to_string()),
            ),
            (
                "parent_id",
                Data::String("par37-40-aegtefaelle-1".to_string()),
            ),
            (
                "item_id",
                Data::String("par37-40-aegtefaelle-parti-1".to_string()),
            ),
            (
                "identifikation",
                Data::String("personskat-fraflytteraktie-aegtefaelle".to_string()),
            ),
            (
                "selskabsidentifikation",
                Data::String("DK-PERSONSKAT-FRAFLYTNING-AEGTEFAELLE".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &par37_shares_sheet, 2, header, value);
        }
        copy_workbook_data_row(sheets, &par37_shares_sheet, 1, 3);
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-par37-40-modstridende-kontekst-2026".to_string()),
            ),
            (
                "parent_id",
                Data::String("par37-40-modstridende-kontekst-1".to_string()),
            ),
            (
                "item_id",
                Data::String("par37-40-modstridende-parti-1".to_string()),
            ),
            (
                "identifikation",
                Data::String("personskat-fraflytteraktie-modstridende".to_string()),
            ),
            (
                "selskabsidentifikation",
                Data::String("DK-PERSONSKAT-FRAFLYTNING-MODSTRIDENDE".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &par37_shares_sheet, 3, header, value);
        }
        set_workbook_cell_by_header(
            sheets,
            "cases",
            1,
            "årsopgørelse.$variant",
            Data::String("MedÅrsopgørelse".to_string()),
        );
        for (header, value) in [
            (
                "årsopgørelse.MedÅrsopgørelse.overført_restskat_mv_kroner",
                "1000",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.øvrig_pensionsbeskatningsafgift_kroner",
                "500",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.a_skat_og_am_indeholdt_kroner",
                "0",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.par68_indbetalt_kroner",
                "0",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.b_skat_betalt_kroner",
                "0",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.udbytteskat_modregningsberettiget_kroner",
                "0",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.frivillig_indbetaling_par59_kroner",
                "0",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.virksomhedsordning_beløb_kroner",
                "0",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.personskattelov_par8a_stk5_beløb_kroner",
                "0",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.afskrivningslov_acontoskat_kroner",
                "0",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.am_lov_par6_beløb_kroner",
                "0",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.seniornedslag_kroner",
                "0",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.energiafgiftskompensation_kroner",
                "0",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.tilbagebetalt_par55_kroner",
                "0",
            ),
        ] {
            set_workbook_cell_by_header(
                sheets,
                "cases",
                1,
                header,
                Data::String(value.to_string()),
            );
        }
    });

    {
        let mut workbook = open_workbook_auto(&input_path).expect("populated Personskat workbook");
        let current_contracts_path = "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.par32_kontraktforløb.MedPar32Kontraktforløb.aktuelt_år.kontrakter";
        let current_contracts_sheet =
            workbook_collection_sheet_name(&mut workbook, current_contracts_path);
        let current_contracts = workbook
            .worksheet_range(&current_contracts_sheet)
            .expect("current KGL §32 contracts");
        let current_rows: Vec<_> = current_contracts.rows().skip(2).collect();
        assert_eq!(current_rows.len(), 3);
        for (row, case_id, item_id, position) in [
            (
                current_rows[0],
                "personskat-kgl-par32-historik-2026",
                "par32-gevinst-a-2026",
                "1",
            ),
            (
                current_rows[1],
                "personskat-kgl-par32-historik-2026",
                "par32-gevinst-b-2026",
                "2",
            ),
            (
                current_rows[2],
                "personskat-kgl-par32-abl17-2026",
                "par32-abl17-tab-2026",
                "1",
            ),
        ] {
            assert_eq!(row[0].to_string(), case_id);
            assert_eq!(row[1].to_string(), item_id);
            assert_eq!(row[2].to_string(), position);
        }
        let linked_asset_column = current_contracts
            .rows()
            .nth(1)
            .expect("KGL §32 contract headers")
            .iter()
            .position(|cell| cell.to_string() == "Tilknyttet ABL-aktiv")
            .expect("linked ABL asset column");
        assert_eq!(
            current_rows[2][linked_asset_column].to_string(),
            "par32-abl17-aktiv"
        );

        let history_path = "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.par32_kontraktforløb.MedPar32Kontraktforløb.tidligere_år";
        let history_sheet = workbook_collection_sheet_name(&mut workbook, history_path);
        let history = workbook
            .worksheet_range(&history_sheet)
            .expect("KGL §32 history");
        assert_eq!(
            history.get((2, 0)).map(ToString::to_string).as_deref(),
            Some("personskat-kgl-par32-historik-2026")
        );
        assert_eq!(
            history.get((2, 1)).map(ToString::to_string).as_deref(),
            Some("par32-historik-2025")
        );
        assert_eq!(
            history.get((2, 2)).map(ToString::to_string).as_deref(),
            Some("1")
        );

        let history_contracts_path = format!("{history_path}.fakta.kontrakter");
        let history_contracts_sheet =
            workbook_collection_sheet_name(&mut workbook, &history_contracts_path);
        let history_contracts = workbook
            .worksheet_range(&history_contracts_sheet)
            .expect("historical KGL §32 contracts");
        assert_eq!(
            history_contracts
                .get((2, 0))
                .map(ToString::to_string)
                .as_deref(),
            Some("personskat-kgl-par32-historik-2026")
        );
        assert_eq!(
            history_contracts
                .get((2, 1))
                .map(ToString::to_string)
                .as_deref(),
            Some("par32-historik-2025")
        );
        assert_eq!(
            history_contracts
                .get((2, 2))
                .map(ToString::to_string)
                .as_deref(),
            Some("par32-tab-2025")
        );
        assert_eq!(
            history_contracts
                .get((2, 3))
                .map(ToString::to_string)
                .as_deref(),
            Some("1")
        );
    }

    let output = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&input_path).ok();
    assert!(
        output.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let result = parse_stdout(&output);
    assert!(result["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .is_empty());

    let json_input_path = temp_path("json");
    let json_template = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--format",
        "json",
        "--output",
        json_input_path.to_str().expect("JSON input path"),
    ]);
    assert!(
        json_template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&json_template.stderr)
    );
    let mut json_input: Value = serde_json::from_slice(
        &std::fs::read(&json_input_path).expect("read generated Personskat JSON template"),
    )
    .expect("Personskat JSON template");
    json_input["cases"][0]["case_id"] = Value::String("personskat-abl-personlig-2026".into());
    json_input["cases"][0]["input"] = serde_json::json!({
        "lønmodtager": {
            "skatteår": 2026,
            "kommune": { "$variant": "København" },
            "bruttoløn_kroner": 600_000,
            "personlig_indkomst": {
                "etableringskonto": { "$variant": "UdenEtableringskontoindskud" },
                "ordinære_forhold": {
                    "arbejdsgiverydelser": [],
                    "virksomheder_uden_virksomhedsordning": []
                }
            },
            "erhvervsbefordring": { "sager": [] },
            "ligningsfradrag": {
                "befordring": { "$variant": "UdenBefordringsfradrag" }
            },
            "pension": {
                "pensionsalder_status": {
                    "$variant": "Ll9lMereEnd15ÅrFørFolkepension"
                },
                "pbl18_indbetalinger": [],
                "pbl18_selvstændig_overskud": {
                    "skattepligtigt_overskud_før_vsl22b_kroner": 0,
                    "renteudgifter_kroner": 0,
                    "kurstab_kroner": 0,
                    "renteindtægter_kroner": 0,
                    "udbytteindtægter_kroner": 0,
                    "kursgevinster_kroner": 0,
                    "udelukkede_afståelsesindkomster_kroner": 0
                },
                "pbl18_livrentevalg": {
                    "$variant": "Pbl18FordeltFradrag"
                },
                "pbl15a_årsgrundlag": {
                    "afståelser": [],
                    "ordninger": [],
                    "kvalifikationsår": [],
                    "tidligere_indbetalinger": []
                },
                "pbl15b_årsgrundlag": {
                    "indkomstposter": [],
                    "ordninger": [],
                    "tidligere_indbetalinger": [],
                    "rateudbetalinger": []
                },
                "øvrige_pbl20_årsgrundlag": {
                    "udbetalinger": []
                },
                "aktiepensionsfradrag_valg": {
                    "$variant": "UdenAktiepensionsfradragIAktieindkomst"
                }
            },
            "personfradrag_alder_status": { "$variant": "Fyldt18EllerGift" },
            "betaler_kirkeskat": false
        },
        "kapitalindkomst": {
            "renter": {
                "renteindtægter_kroner": 0,
                "renteudgifter_kroner": 0,
                "næringsstatus": { "$variant": "IkkeNæring" },
                "ligningslov6": { "$variant": "UdenLigningslov6Kurstab" },
                "ligningslov6a": { "$variant": "UdenLigningslov6AFradrag" }
            },
            "pbl53a": { "ordninger": [] },
            "ejendomsavance": { "$variant": "UdenEjendomsavance" },
            "ejendomsdrift": { "$variant": "UdenEjendomsdriftEfterPar4Nr6" },
            "kursgevinst": { "$variant": "UdenKursgevinst" },
            "fremleje": { "$variant": "UdenFremlejeEfterLigningslov15Q" },
            "omkostninger": []
        },
        "aktieavance": {
            "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
            "særlige_aktiver": [{
                "identifikation": "abl-par17-1",
                "kilde": {
                    "$variant": "PersonskatAktieaktivEfterPar17",
                    "fakta": {
                        "indkomstår": 2026,
                        "skattepligtsgrundlag": {
                            "$variant": "AblPar7PersonEfterKildeskatteloven"
                        },
                        "næringsstatus": {
                            "$variant": "AblPar17UdøverNæringVedKøbOgSalgAfAktier"
                        },
                        "instrument": { "$variant": "AblPar17AlmindeligAktie" },
                        "erhvervelsesstatus": {
                            "$variant": "AblPar17ErhvervetSomLedINæringsvej"
                        },
                        "afståelsessum_kroner": 37_000,
                        "anskaffelsessum_kroner": 30_000
                    }
                },
                "markedsstatus": { "$variant": "AblIkkeOptagetTilHandel" }
            }],
            "udbytter": []
        },
        "udenlandske_sociale_bidrag": {
            "$variant": "UdenUdenlandskeSocialeBidragEfterLigningslov8M"
        },
        "cfc": { "poster": [] },
        "ejendomsskatter": {
            "person": {
                "ejer_folkepensionsalder": {
                    "$variant": "EjskFolkepensionsalderIkkeOpnået"
                },
                "samlevende_ægtefælles_folkepensionsalder": {
                    "$variant": "EjskFolkepensionsalderIkkeOpnået"
                },
                "skattemæssigt_hjemsted": {
                    "$variant": "EjskFuldtSkattepligtigEfterKildeskattelovensPar1"
                },
                "egen_udbytteindkomst_kroner": 0,
                "ægtefælles_udbytteindkomst_kroner": 0,
                "egne_historiske_aktielønshændelser": [],
                "ægtefælles_historiske_aktielønshændelser": []
            },
            "ejendomme": []
        },
        "skatteforhold": { "$variant": "StandardSkatteforhold" },
        "underskudsforhold": { "$variant": "StandardUnderskudsforhold" },
        "ægtefælle": { "$variant": "UdenÆgtefælle" },
        "årsopgørelse": { "$variant": "UdenÅrsopgørelse" }
    });
    let mut interest_case = json_input["cases"][0].clone();
    interest_case["case_id"] = Value::String("personskat-renter-befordring-2026".into());
    let reinvested_home = serde_json::json!({
        "$variant": "EblBoligejendom",
        "fakta": {
            "ejendomsart": { "$variant": "EblPar8Ejerlejlighed" },
            "tjent_til_bolig_eller_privat_formål": true,
            "betingelser_opfyldt_i_ejerperioden": true,
            "afståelsesforhold": { "$variant": "EblPar8AlmindeligAfståelse" },
            "genanbringelsesforhold": {
                "$variant": "EblPar8GenanbringelseEfterStk5",
                "genanbringelse": {
                    "$variant": "EblMedGenanbringelseEfterPar6A",
                    "fakta": {
                        "oprindelig_fortjeneste": {
                            "afståelsesindkomstår": 2025,
                            "erhvervsfortjeneste_før_par6_stk2_kroner": 190_000,
                            "regulering": {
                                "tillæg_for_genanbragt_del_kroner": 0,
                                "nedslag_for_genanbragt_del_kroner": 0
                            }
                        },
                        "afstået_ejendoms_erhvervsanvendelse": {
                            "$variant": "EblPar6AEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed"
                        },
                        "investering": {
                            "indkomstår": 2026,
                            "erhvervsmæssigt_anskaffelsesgrundlag_kroner": 1_000_000,
                            "ejendomsstatus": {
                                "$variant": "EblPar6AEjendomOmfattetAfLovenIkkePar8"
                            },
                            "erhvervsanvendelse": {
                                "$variant": "EblPar6AEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed"
                            },
                            "placering": {
                                "$variant": "EblPar6AEjendomIDanmark"
                            }
                        },
                        "begæring": {
                            "$variant": "EblPar6ABegæringVedRettidigAfgivelseEfterSkattekontrollovensPar2"
                        },
                        "ejerskab": {
                            "$variant": "EblPar6ASammeSkattepligtige"
                        }
                    }
                },
                "anvendelsesændring": {
                    "ændringsdato": { "år": 2026, "måned": 6, "dag": 1 }
                }
            }
        }
    });
    interest_case["input"]["kapitalindkomst"] = serde_json::json!({
        "renter": {
            "renteindtægter_kroner": 20_000,
            "renteudgifter_kroner": 5_000,
            "næringsstatus": { "$variant": "IkkeNæring" },
            "ligningslov6": {
                "$variant": "MedLigningslov6Kurstab",
                "input": {
                    "indkomstår": 2026,
                    "kontantlån_optaget_i_realkreditinstitut_før_19_maj_1993": true,
                    "indfrielse_sker_ved_realkreditlån": true,
                    "nyt_lån_optaget_før_1_januar_1996": true,
                    "nyt_lån_mindst_samme_løbetid": true,
                    "transaktioner_inden_for_1_år": true,
                    "lånetilbud_før_indfrielse_hvis_indfrielse_før_optagelse": true,
                    "kurstab_kroner": 24_000,
                    "samlet_antal_terminer": 12,
                    "forfaldne_terminer_i_indkomståret": 1,
                    "ekstraordinær_indfrielse_af_nyt_lån": false,
                    "stk4_omlægning_undtager_stk3": false,
                    "stk3_nedsættelse_basispoint": 0,
                    "debitorskifte_i_året": false,
                    "debitordage_for_skattepligtig": 0,
                    "dage_i_overdragelsesår": 0,
                    "kurstab_medregnes_efter_kursgevinstloven": false
                }
            },
            "ligningslov6a": {
                "$variant": "MedLigningslov6AFradrag",
                "input": {
                    "indkomstår": 2026,
                    "skattepligtig_person": true,
                    "arbejderboliger_beløb_kroner": 1_000,
                    "arbejderboliger_betalt": true,
                    "statshusmandsbrug_jordrente_beløb_kroner": 0,
                    "statshusmandsbrug_jordrente_betalt": false
                }
            }
        },
        "pbl53a": { "ordninger": [] },
        "ejendomsdrift": { "$variant": "UdenEjendomsdriftEfterPar4Nr6" },
        "ejendomsavance": {
            "$variant": "MedEjendomsavance",
            "fakta": {
                "egne_afståelser": [{
                    "identifikation": "egen-fortjeneste",
                    "afståelsesdato": { "år": 2026, "måned": 12, "dag": 31 },
                    "afståelse": { "$variant": "EblAlmindeligAfståelse" },
                    "erhvervet_som_led_i_næring": false,
                    "kontant_anskaffelsessum_kroner": 1_000_000,
                    "gæld_kursværdi_ved_anskaffelse_kroner": 0,
                    "par5_fakta": {
                        "anskaffelsesdato": { "år": 2026, "måned": 1, "dag": 1 },
                        "anskaffelsesgrundlag": { "$variant": "EblPar4AlmindeligtAnskaffelsesgrundlag" },
                        "fordeling": {
                            "ejerandel_promille": 1000,
                            "afståelsesomfang": { "$variant": "EblPar5HeleEjendommen" }
                        },
                        "vedligeholdelses_og_forbedringsudgifter": [],
                        "nedsættelser": [],
                        "mælkekvoter": [],
                        "stk6_overførsel": { "$variant": "EblPar5UdenStk6Overførsel" },
                        "reguleringsvalg": { "$variant": "EblPar5UdenIndeksering" }
                    },
                    "par4_stk8_anskaffelse_udeladt_kroner": 0,
                    "kontant_afståelsessum_kroner": 1_200_000,
                    "overdragne_gældsposter_kursværdi_kroner": 0,
                    "par4_stk8_afståelsesværdi_udeladt_kroner": 0,
                    "par11_stk2_valg": { "$variant": "EblPar11Stk2IngenNyGenanbringelse" },
                    "par6d_valg": { "$variant": "EblUdenPar6DValg" },
                    "ejendomstype": reinvested_home
                }, {
                    "identifikation": "eget-tab",
                    "afståelsesdato": { "år": 2026, "måned": 12, "dag": 31 },
                    "afståelse": { "$variant": "EblAlmindeligAfståelse" },
                    "erhvervet_som_led_i_næring": false,
                    "kontant_anskaffelsessum_kroner": 500_000,
                    "gæld_kursværdi_ved_anskaffelse_kroner": 0,
                    "par5_fakta": {
                        "anskaffelsesdato": { "år": 2026, "måned": 1, "dag": 1 },
                        "anskaffelsesgrundlag": { "$variant": "EblPar4AlmindeligtAnskaffelsesgrundlag" },
                        "fordeling": {
                            "ejerandel_promille": 1000,
                            "afståelsesomfang": { "$variant": "EblPar5HeleEjendommen" }
                        },
                        "vedligeholdelses_og_forbedringsudgifter": [],
                        "nedsættelser": [],
                        "mælkekvoter": [],
                        "stk6_overførsel": { "$variant": "EblPar5UdenStk6Overførsel" },
                        "reguleringsvalg": { "$variant": "EblPar5UdenIndeksering" }
                    },
                    "par4_stk8_anskaffelse_udeladt_kroner": 0,
                    "kontant_afståelsessum_kroner": 450_000,
                    "overdragne_gældsposter_kursværdi_kroner": 0,
                    "par4_stk8_afståelsesværdi_udeladt_kroner": 0,
                    "par11_stk2_valg": { "$variant": "EblPar11Stk2IngenNyGenanbringelse" },
                    "par6d_valg": { "$variant": "EblUdenPar6DValg" },
                    "ejendomstype": {
                        "$variant": "EblAndenFastEjendom",
                        "genanbringelse": { "$variant": "EblUdenAktivGenanbringelse" }
                    }
                }],
                "egne_skadegenopførelser": [],
                "eget_fremført_tab": {
                    "$variant": "MedFremførtEjendomstab",
                    "fra_indkomstår": 2025,
                    "tab_kroner": 25_000
                },
                "ægtefælles_afståelser": [{
                    "identifikation": "ægtefælles-tab",
                    "afståelsesdato": { "år": 2026, "måned": 12, "dag": 31 },
                    "afståelse": { "$variant": "EblAlmindeligAfståelse" },
                    "erhvervet_som_led_i_næring": false,
                    "kontant_anskaffelsessum_kroner": 300_000,
                    "gæld_kursværdi_ved_anskaffelse_kroner": 0,
                    "par5_fakta": {
                        "anskaffelsesdato": { "år": 2026, "måned": 1, "dag": 1 },
                        "anskaffelsesgrundlag": { "$variant": "EblPar4AlmindeligtAnskaffelsesgrundlag" },
                        "fordeling": {
                            "ejerandel_promille": 1000,
                            "afståelsesomfang": { "$variant": "EblPar5HeleEjendommen" }
                        },
                        "vedligeholdelses_og_forbedringsudgifter": [],
                        "nedsættelser": [],
                        "mælkekvoter": [],
                        "stk6_overførsel": { "$variant": "EblPar5UdenStk6Overførsel" },
                        "reguleringsvalg": { "$variant": "EblPar5UdenIndeksering" }
                    },
                    "par4_stk8_anskaffelse_udeladt_kroner": 0,
                    "kontant_afståelsessum_kroner": 270_000,
                    "overdragne_gældsposter_kursværdi_kroner": 0,
                    "par4_stk8_afståelsesværdi_udeladt_kroner": 0,
                    "par11_stk2_valg": { "$variant": "EblPar11Stk2IngenNyGenanbringelse" },
                    "par6d_valg": { "$variant": "EblUdenPar6DValg" },
                    "ejendomstype": {
                        "$variant": "EblAndenFastEjendom",
                        "genanbringelse": { "$variant": "EblUdenAktivGenanbringelse" }
                    }
                }],
                "ægtefælles_skadegenopførelser": [],
                "ægtefælles_fremførte_tab": { "$variant": "UdenFremførtEjendomstab" },
                "gift_samlevende_ved_indkomstårets_udgang": true
            }
        },
        "kursgevinst": { "$variant": "UdenKursgevinst" },
        "fremleje": { "$variant": "UdenFremlejeEfterLigningslov15Q" },
        "omkostninger": [{
            "identifikation": "dokumenteret-kapitalomkostning-1",
            "anvendelsesår": 2026,
            "anvendelse": { "$variant": "Par4Stk2ErhverveKapitalindkomst" },
            "omkostningsart": { "$variant": "Ll17CAndenOmkostning" },
            "næringsstatus": { "$variant": "IkkeNæring" },
            "beløb_kroner": 2_000
        }]
    });
    interest_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": {
            "$variant": "MedOrdinærtAktieår",
            "input": {
                "indkomstår": 2026,
                "hændelsesforløb": [{
                    "position_primo": {
                        "selskabsidentifikation": "DK-BOLIGRET-1",
                        "kapitalmængde": {
                            "$variant": "AblAktiekapitalUdenPålydendeVærdi",
                            "antal_aktier": 10
                        },
                        "anskaffelsessum_kroner": 10_000
                    },
                    "hændelser": [{
                        "$variant": "AblOrdinærAfståelse",
                        "kapitalmængde": {
                            "$variant": "AblAktiekapitalUdenPålydendeVærdi",
                            "antal_aktier": 10
                        },
                        "afståelsessum_kroner": 15_000,
                        "par5a_kildefakta": {
                            "$variant": "AblOrdinærIngenPar5AFaktaPåkrævet"
                        },
                        "vilkår": {
                            "markedsstatus": { "$variant": "AblIkkeOptagetTilHandel" },
                            "har_tidligere_været_optaget_til_handel": false,
                            "hovedaktionæraktier": false,
                            "afståede_aktiers_handelsværdi_kroner": 0,
                            "beholdte_aktiers_handelsværdi_kroner": 0,
                            "oplysningsstatus": { "$variant": "AblOplystRettidigt" },
                            "boligret": {
                                "$variant": "AblBoligretEfterPar15",
                                "udsteder": {
                                    "$variant": "AblPar15Kapitalselskabsudsteder",
                                    "sel_input": {
                                        "selskabsform": {
                                            "$variant": "Sel1Stk1Nr1IndregistreretAktieselskab"
                                        },
                                        "hjemmehørende_i_danmark": true
                                    }
                                },
                                "værdipapirstatus": {
                                    "$variant": "AblPar15VærdipapirOmfattetAfAktieavancebeskatningsloven"
                                },
                                "fakta": {
                                    "værdipapir_forbundet_med_brugsret_til_beboelseslejlighed": true,
                                    "udsteder_ejer_direkte_ejendom_med_flere_beboelseslejligheder": true,
                                    "lejlighed_har_tjent_til_bolig_mens_skattefrihedsbetingelser_var_opfyldt_i_ejertiden": true,
                                    "grundforhold": {
                                        "$variant": "EblPar8Stk4UdenBestemtGrundareal"
                                    }
                                },
                                "afståelsesform": {
                                    "$variant": "AblPar15AlmindeligAfståelse"
                                }
                            }
                        }
                    }]
                }, {
                    "position_primo": {
                        "selskabsidentifikation": "DK-PAR5A-1",
                        "kapitalmængde": {
                            "$variant": "AblAktiekapitalUdenPålydendeVærdi",
                            "antal_aktier": 10
                        },
                        "anskaffelsessum_kroner": 30_000
                    },
                    "hændelser": [{
                        "$variant": "AblOrdinærAfståelse",
                        "kapitalmængde": {
                            "$variant": "AblAktiekapitalUdenPålydendeVærdi",
                            "antal_aktier": 10
                        },
                        "afståelsessum_kroner": 0,
                        "par5a_kildefakta": {
                            "$variant": "AblOrdinærPar5AKildefakta",
                            "fakta": {
                                "anvendelsesgrundlag": {
                                    "$variant": "AblPar5AAfståelseDen24November2010EllerSenere"
                                },
                                "skatteydergrundlag": {
                                    "$variant": "AblPar5APersonSkattepligtigEfterPar7"
                                },
                                "ejertidsudbytter": [{
                                    "$variant": "AblPar5ASkattefritUdbytteAfPågældendeAktier",
                                    "beløb_kroner": 12_000
                                }],
                                "præferenceposition": {
                                    "modtaget_tilsvarende_udbytte_kroner": 0,
                                    "allerede_anvendt_til_tabsreduktion_kroner": 0
                                },
                                "koncernbeløb": []
                            }
                        },
                        "vilkår": {
                            "markedsstatus": { "$variant": "AblIkkeOptagetTilHandel" },
                            "har_tidligere_været_optaget_til_handel": false,
                            "hovedaktionæraktier": false,
                            "afståede_aktiers_handelsværdi_kroner": 0,
                            "beholdte_aktiers_handelsværdi_kroner": 0,
                            "oplysningsstatus": { "$variant": "AblOplystRettidigt" },
                            "boligret": { "$variant": "AblUdenBoligret" }
                        }
                    }]
                }],
                "investeringsbeviser": [],
                "fremført_tab_efter_par13a_kroner": 0
            }
        },
        "særlige_aktiver": [],
        "udbytter": []
    });
    interest_case["input"]["lønmodtager"]["ligningsfradrag"] = serde_json::json!({
        "befordring": {
            "$variant": "MedBefordringsfradrag",
            "fakta": {
                "arbejdsdage": 203,
                "daglige_befordringskilometer": 60,
                "bopæl_i_yderkommune_eller_lille_ø": false,
                "befordringsformål": { "$variant": "IndtægtsgivendeArbejdsplads" },
                "modtaget_skattefri_befordringsgodtgørelse_for_strækning": false,
                "modtaget_uddannelsesbefordringsrabat_eller_godtgørelse_for_strækning": false,
                "ligningslov9d": { "$variant": "UdenLigningslov9D" },
                "fradrag_udelukket_folketingshverv_m_v": false,
                "arbejdsgiverbetalt_befordring": {
                    "$variant": "UdenArbejdsgiverbetaltBefordring"
                },
                "broer": {
                    "storebælt_bil_motorcykel_passager": 0,
                    "storebælt_kollektiv_passager": 0,
                    "øresund_bil_motorcykel_passager": 0,
                    "øresund_kollektiv_passager": 0,
                    "dokumenteret_og_afholdt_af_skattepligtige": false
                },
                "særlig_transport": {
                    "faktisk_dokumenteret_udgift_kroner": 0,
                    "geografiske_forhold_tidsforbrug_økonomisk_rimelighed_kræver_transporten": false
                }
            }
        }
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(interest_case);
    let mut ebl5_case = json_input["cases"][0].clone();
    ebl5_case["case_id"] = Value::String("personskat-ebl5-kildefakta-2026".into());
    ebl5_case["input"]["kapitalindkomst"] = serde_json::json!({
        "renter": {
            "renteindtægter_kroner": 0,
            "renteudgifter_kroner": 0,
            "næringsstatus": { "$variant": "IkkeNæring" },
            "ligningslov6": { "$variant": "UdenLigningslov6Kurstab" },
            "ligningslov6a": { "$variant": "UdenLigningslov6AFradrag" }
        },
        "pbl53a": { "ordninger": [] },
        "ejendomsdrift": { "$variant": "UdenEjendomsdriftEfterPar4Nr6" },
        "ejendomsavance": {
            "$variant": "MedEjendomsavance",
            "fakta": {
                "egne_afståelser": [{
                    "identifikation": "landbrug-med-mælk-og-stk6",
                    "afståelsesdato": { "år": 2026, "måned": 12, "dag": 31 },
                    "afståelse": { "$variant": "EblAlmindeligAfståelse" },
                    "erhvervet_som_led_i_næring": false,
                    "kontant_anskaffelsessum_kroner": 200_000,
                    "gæld_kursværdi_ved_anskaffelse_kroner": 0,
                    "par5_fakta": {
                        "anskaffelsesdato": { "år": 1985, "måned": 1, "dag": 1 },
                        "anskaffelsesgrundlag": {
                            "$variant": "EblPar4Stk3Nr1Anskaffelsesgrundlag"
                        },
                        "fordeling": {
                            "ejerandel_promille": 500,
                            "afståelsesomfang": {
                                "$variant": "EblPar5DelAfEjendommen",
                                "afstået_del_anskaffelsessum_før_par5_kroner": 200_000,
                                "hele_ejendommens_anskaffelsessum_før_par5_kroner": 800_000,
                                "ikke_boligdelens_anskaffelsessum_før_par5_kroner": 800_000
                            }
                        },
                        "vedligeholdelses_og_forbedringsudgifter": [],
                        "nedsættelser": [],
                        "mælkekvoter": [{
                            "identifikation": "mælk-2000",
                            "anskaffelsesdato": { "år": 2000, "måned": 1, "dag": 1 },
                            "oprindelige_enheder": 100,
                            "disponerede_enheder": 100,
                            "anskaffelsesgrundlag": {
                                "$variant": "EblPar5MælkekvoteKøbt",
                                "vederlag_kroner": 80_000
                            },
                            "disposition": {
                                "$variant": "EblPar5MælkekvoteAfstået",
                                "afståelsesdato": { "år": 2010, "måned": 6, "dag": 1 },
                                "vederlag_kroner": 120_000
                            }
                        }],
                        "stk6_overførsel": {
                            "$variant": "EblPar5MedStk6Overførsel",
                            "fakta": {
                                "ejendomskategori": {
                                    "$variant": "EblPar5Stk6Landbrugsejendom"
                                },
                                "tillægsparcelværdi_kroner": 500_000,
                                "teknisk_værdi_kroner": 600_000,
                                "afskrivninger_på_vurderingsomfattede_bygninger_kroner": 100_000,
                                "stk4_nr2_afskrivninger_på_vurderingsomfattede_nedrevne_bygninger_kroner": 50_000,
                                "nedskrevet_værdi_af_vurderingsomfattede_bygninger_kroner": 150_000,
                                "afstået_jord_anskaffelsessum_før_overførsel_kroner": 200_000,
                                "samlet_jord_anskaffelsessum_før_overførsel_kroner": 800_000
                            }
                        },
                        "reguleringsvalg": { "$variant": "EblPar5UdenIndeksering" }
                    },
                    "par4_stk8_anskaffelse_udeladt_kroner": 0,
                    "kontant_afståelsessum_kroner": 500_000,
                    "overdragne_gældsposter_kursværdi_kroner": 0,
                    "par4_stk8_afståelsesværdi_udeladt_kroner": 0,
                    "par11_stk2_valg": { "$variant": "EblPar11Stk2IngenNyGenanbringelse" },
                    "par6d_valg": { "$variant": "EblUdenPar6DValg" },
                    "ejendomstype": {
                        "$variant": "EblLandbrugSkovNaturEllerBlandetEjendom",
                        "fakta": {
                            "kategori": { "$variant": "EblPar9Landbrugsejendom" },
                            "bolig_har_tjent_til_bolig_for_ejer_eller_husstand": false,
                            "boligbetingelser_opfyldt_i_ejerperioden": false,
                            "grundbetingelse": {
                                "$variant": "EblPar8GrundbetingelseIkkeOpfyldt"
                            },
                            "bolig_anskaffelsessum_kroner": 0,
                            "bolig_afståelsessum_kroner": 0,
                            "skadeforhold": {
                                "$variant": "EblPar9IngenVæsentligSkade"
                            },
                            "genanbringelsesforhold": {
                                "$variant": "EblPar9IngenGenanbringelseEfterStk4"
                            }
                        }
                    }
                }],
                "egne_skadegenopførelser": [],
                "eget_fremført_tab": {
                    "$variant": "UdenFremførtEjendomstab"
                },
                "ægtefælles_afståelser": [],
                "ægtefælles_skadegenopførelser": [],
                "ægtefælles_fremførte_tab": {
                    "$variant": "UdenFremførtEjendomstab"
                },
                "gift_samlevende_ved_indkomstårets_udgang": false
            }
        },
        "kursgevinst": { "$variant": "UdenKursgevinst" },
        "fremleje": { "$variant": "UdenFremlejeEfterLigningslov15Q" },
        "omkostninger": []
    });
    ebl5_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": [],
        "udbytter": []
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(ebl5_case);
    let par6d_schedule = (1..=10)
        .map(|year| {
            serde_json::json!({
                "år_efter_afståelsen": year,
                "hovedstol_forfalder_kroner": 375_000
            })
        })
        .collect::<Vec<_>>();
    let mut ebl6d_case = json_input["cases"][0].clone();
    ebl6d_case["case_id"] = Value::String("personskat-ebl6d-historisk-2026".into());
    let par6d_election = serde_json::json!({
        "$variant": "EblMedPar6DValg",
        "fakta": {
            "valgt_udskudt_fortjeneste_kroner": 3_000_000,
            "årligt_beløb_kroner": 300_000,
            "fordelingsår": 10,
            "erhververen_erhvervede_ejendommen_som_led_i_næring": false,
            "partrelation": {
                "overdragerens_kapitalandel_i_erhververen_promille": 0,
                "overdragerens_stemmeandel_i_erhververen_promille": 0,
                "erhververens_kapitalandel_i_overdrageren_promille": 0,
                "erhververens_stemmeandel_i_overdrageren_promille": 0,
                "parterne_er_nærtstående_efter_ligningslovens_par2_stk2": false,
                "parterne_har_aftale_om_fælles_bestemmende_indflydelse": false,
                "parterne_er_under_fælles_bestemmende_indflydelse": false
            },
            "overdragerens_anvendelse": {
                "$variant": "EblPar6DEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed"
            },
            "erhververens_anvendelse_ved_overdragelsen": {
                "$variant": "EblPar6DEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed"
            },
            "sælgerpantebrev": {
                "identifikation": "sælgerpantebrev-2025",
                "fordring_mod_erhververen": true,
                "pant_i_den_overdragne_ejendom": true,
                "kontantværdi_kroner": 3_000_000,
                "hovedstol_kroner": 3_750_000,
                "løbetid_år": 10,
                "afdragsplan": par6d_schedule
            },
            "meddelelse": {
                "$variant": "EblPar6DRettidigMeddelelse",
                "indhold": {
                    "kopi_af_sælgerpantebrev_vedlagt": true,
                    "oplyst_skattepligtig_fortjeneste_efter_par6_kroner": 4_990_000,
                    "oplyst_valgt_udskudt_fortjeneste_kroner": 3_000_000,
                    "oplyst_årligt_beløb_kroner": 300_000,
                    "oplyst_antal_år": 10
                }
            },
            "sælgers_skatteforhold": {
                "skattepligtsstatus": {
                    "$variant": "EblPar6DFuldtSkattepligtigTilDanmark"
                },
                "hjemland_omfattet_af_nordisk_overenskomst_eller_eu_inddrivelsesdirektiv": true,
                "sikkerhed": { "$variant": "EblPar6DIngenSikkerhed" }
            },
            "ejendomsplacering": {
                "$variant": "EblPar6DEjendomIDanmark"
            },
            "afståelsesårets_hændelse": {
                "$variant": "EblPar6DIngenFremrykningshændelse"
            },
            "efterfølgende_årsforhold": [{
                "indkomstår": 2026,
                "forløbsposter": [{
                    "$variant": "EblPar6DOrdinærtHovedstolsafdrag",
                    "fordringsdel_identifikation": "sælgerpantebrev-2025",
                    "betalt_kroner": 375_000
                }]
            }]
        }
    });
    ebl6d_case["input"]["kapitalindkomst"] = serde_json::json!({
        "renter": {
            "renteindtægter_kroner": 0,
            "renteudgifter_kroner": 0,
            "næringsstatus": { "$variant": "IkkeNæring" },
            "ligningslov6": { "$variant": "UdenLigningslov6Kurstab" },
            "ligningslov6a": { "$variant": "UdenLigningslov6AFradrag" }
        },
        "pbl53a": { "ordninger": [] },
        "ejendomsdrift": { "$variant": "UdenEjendomsdriftEfterPar4Nr6" },
        "ejendomsavance": {
            "$variant": "MedEjendomsavance",
            "fakta": {
                "egne_afståelser": [{
                    "identifikation": "erhvervsejendom-solgt-2025",
                    "afståelsesdato": { "år": 2025, "måned": 12, "dag": 31 },
                    "afståelse": { "$variant": "EblAlmindeligAfståelse" },
                    "erhvervet_som_led_i_næring": false,
                    "kontant_anskaffelsessum_kroner": 25_000_000,
                    "gæld_kursværdi_ved_anskaffelse_kroner": 0,
                    "par5_fakta": {
                        "anskaffelsesdato": { "år": 2025, "måned": 1, "dag": 1 },
                        "anskaffelsesgrundlag": {
                            "$variant": "EblPar4AlmindeligtAnskaffelsesgrundlag"
                        },
                        "fordeling": {
                            "ejerandel_promille": 1000,
                            "afståelsesomfang": { "$variant": "EblPar5HeleEjendommen" }
                        },
                        "vedligeholdelses_og_forbedringsudgifter": [],
                        "nedsættelser": [],
                        "mælkekvoter": [],
                        "stk6_overførsel": { "$variant": "EblPar5UdenStk6Overførsel" },
                        "reguleringsvalg": { "$variant": "EblPar5UdenIndeksering" }
                    },
                    "par4_stk8_anskaffelse_udeladt_kroner": 0,
                    "kontant_afståelsessum_kroner": 30_000_000,
                    "overdragne_gældsposter_kursværdi_kroner": 0,
                    "par4_stk8_afståelsesværdi_udeladt_kroner": 0,
                    "par11_stk2_valg": { "$variant": "EblPar11Stk2IngenNyGenanbringelse" },
                    "par6d_valg": par6d_election,
                    "ejendomstype": {
                        "$variant": "EblAndenFastEjendom",
                        "genanbringelse": { "$variant": "EblUdenAktivGenanbringelse" }
                    }
                }],
                "egne_skadegenopførelser": [],
                "eget_fremført_tab": { "$variant": "UdenFremførtEjendomstab" },
                "ægtefælles_afståelser": [],
                "ægtefælles_skadegenopførelser": [],
                "ægtefælles_fremførte_tab": { "$variant": "UdenFremførtEjendomstab" },
                "gift_samlevende_ved_indkomstårets_udgang": false
            }
        },
        "kursgevinst": {
            "$variant": "MedKursgevinst",
            "fakta": {
                "skatteyder_identifikation": "Sælger",
                "ægtefælles_skatteyder_identifikation": null,
                "sælgerpantebreve": [{
                    "sælgerpantebrev_identifikation": "sælgerpantebrev-2025",
                    "oprindelig_skatteyder_identifikation": "Sælger",
                    "skatteyderfakta": {
                        "udøver_næring_ved_køb_og_salg_af_fordringer": false,
                        "fordringen_erhvervet_uden_for_fordringsnæring": true,
                        "fordringen_erhvervet_som_vederlag_for_leverede_varer_eller_tjenesteydelser": false,
                        "fordringen_erhvervet_i_direkte_tilknytning_til_erhvervsmæssig_drift": false,
                        "debitor_omfattet_af_tabsbegrænsningen_i_kgl_par14_stk2": false,
                        "renter_eller_gevinster_fritaget_efter_dobbeltbeskatningsoverenskomst": false
                    },
                    "dispositioner_efter_ebl_forløbet": []
                }],
                "gældsposter": [],
                "øvrige_instrumenter": {
                    "fordringer": [],
                    "obligationsbaserede_minimumsbeviser": []
                },
                "par32_kontraktforløb": {
                    "$variant": "UdenPar32Kontraktforløb"
                }
            }
        },
        "fremleje": { "$variant": "UdenFremlejeEfterLigningslov15Q" },
        "omkostninger": []
    });
    ebl6d_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": [],
        "udbytter": []
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(ebl6d_case);
    let mut ebl11_case = json_input["cases"][0].clone();
    ebl11_case["case_id"] = Value::String("personskat-ebl11-genanbringelse-2026".into());
    ebl11_case["input"]["kapitalindkomst"] = serde_json::json!({
        "renter": {
            "renteindtægter_kroner": 0,
            "renteudgifter_kroner": 0,
            "næringsstatus": { "$variant": "IkkeNæring" },
            "ligningslov6": { "$variant": "UdenLigningslov6Kurstab" },
            "ligningslov6a": { "$variant": "UdenLigningslov6AFradrag" }
        },
        "pbl53a": { "ordninger": [] },
        "ejendomsdrift": { "$variant": "UdenEjendomsdriftEfterPar4Nr6" },
        "ejendomsavance": {
            "$variant": "MedEjendomsavance",
            "fakta": {
                "egne_afståelser": [{
                    "identifikation": "eksproprieret-genanbringelsesejendom",
                    "afståelsesdato": { "år": 2026, "måned": 12, "dag": 31 },
                    "afståelse": { "$variant": "EblEkspropriationserstatning" },
                    "erhvervet_som_led_i_næring": false,
                    "kontant_anskaffelsessum_kroner": 1_000_000,
                    "gæld_kursværdi_ved_anskaffelse_kroner": 0,
                    "par5_fakta": {
                        "anskaffelsesdato": { "år": 2026, "måned": 1, "dag": 1 },
                        "anskaffelsesgrundlag": {
                            "$variant": "EblPar4AlmindeligtAnskaffelsesgrundlag"
                        },
                        "fordeling": {
                            "ejerandel_promille": 1000,
                            "afståelsesomfang": { "$variant": "EblPar5HeleEjendommen" }
                        },
                        "vedligeholdelses_og_forbedringsudgifter": [],
                        "nedsættelser": [],
                        "mælkekvoter": [],
                        "stk6_overførsel": { "$variant": "EblPar5UdenStk6Overførsel" },
                        "reguleringsvalg": { "$variant": "EblPar5UdenIndeksering" }
                    },
                    "par4_stk8_anskaffelse_udeladt_kroner": 0,
                    "kontant_afståelsessum_kroner": 1_500_000,
                    "overdragne_gældsposter_kursværdi_kroner": 0,
                    "par4_stk8_afståelsesværdi_udeladt_kroner": 0,
                    "par11_stk2_valg": {
                        "$variant": "EblPar11Stk2IngenNyGenanbringelse"
                    },
                    "par6d_valg": { "$variant": "EblUdenPar6DValg" },
                    "ejendomstype": {
                        "$variant": "EblAndenFastEjendom",
                        "genanbringelse": {
                            "$variant": "EblMedGenanbringelseEfterPar6A",
                            "fakta": {
                                "oprindelig_fortjeneste": {
                                    "afståelsesindkomstår": 2025,
                                    "erhvervsfortjeneste_før_par6_stk2_kroner": 200_000,
                                    "regulering": {
                                        "tillæg_for_genanbragt_del_kroner": 0,
                                        "nedslag_for_genanbragt_del_kroner": 0
                                    }
                                },
                                "afstået_ejendoms_erhvervsanvendelse": {
                                    "$variant": "EblPar6AEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed"
                                },
                                "investering": {
                                    "indkomstår": 2026,
                                    "erhvervsmæssigt_anskaffelsesgrundlag_kroner": 1_000_000,
                                    "ejendomsstatus": {
                                        "$variant": "EblPar6AEjendomOmfattetAfLovenIkkePar8"
                                    },
                                    "erhvervsanvendelse": {
                                        "$variant": "EblPar6AEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed"
                                    },
                                    "placering": { "$variant": "EblPar6AEjendomIDanmark" }
                                },
                                "begæring": {
                                    "$variant": "EblPar6ABegæringVedRettidigAfgivelseEfterSkattekontrollovensPar2"
                                },
                                "ejerskab": { "$variant": "EblPar6ASammeSkattepligtige" }
                            }
                        }
                    }
                }],
                "egne_skadegenopførelser": [],
                "eget_fremført_tab": { "$variant": "UdenFremførtEjendomstab" },
                "ægtefælles_afståelser": [],
                "ægtefælles_skadegenopførelser": [],
                "ægtefælles_fremførte_tab": { "$variant": "UdenFremførtEjendomstab" },
                "gift_samlevende_ved_indkomstårets_udgang": false
            }
        },
        "kursgevinst": { "$variant": "UdenKursgevinst" },
        "fremleje": { "$variant": "UdenFremlejeEfterLigningslov15Q" },
        "omkostninger": []
    });
    ebl11_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": [],
        "udbytter": []
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(ebl11_case);
    let mut fremleje_case = json_input["cases"][0].clone();
    fremleje_case["case_id"] = Value::String("personskat-fremleje-2026".into());
    fremleje_case["input"]["kapitalindkomst"] = serde_json::json!({
        "renter": {
            "renteindtægter_kroner": 0,
            "renteudgifter_kroner": 0,
            "næringsstatus": { "$variant": "IkkeNæring" },
            "ligningslov6": { "$variant": "UdenLigningslov6Kurstab" },
            "ligningslov6a": { "$variant": "UdenLigningslov6AFradrag" }
        },
        "pbl53a": { "ordninger": [] },
        "ejendomsdrift": { "$variant": "UdenEjendomsdriftEfterPar4Nr6" },
        "ejendomsavance": { "$variant": "UdenEjendomsavance" },
        "kursgevinst": { "$variant": "UdenKursgevinst" },
        "fremleje": {
            "$variant": "MedFremlejeEfterLigningslov15Q",
            "fakta": {
                "rolle": { "$variant": "PersonskatFremlejendeLejer" },
                "udlejningsform": { "$variant": "Ll15QVærelserIHelårsbolig" },
                "boligstatus": { "$variant": "Ll15QHelårsbolig" },
                "indberetningsstatus": {
                    "$variant": "Ll15QIndberettetEfterSkatteindberetningslov43"
                },
                "metode": { "$variant": "Ll15QStk1Bundfradrag" },
                "bruttolejeindtægt_kroner": 60_000,
                "faktiske_udgifter_kroner": 0,
                "tidligere_anvendt_par15p_stk3": false,
                "stk4_samordning": {
                    "$variant": "UdenSamordningMedLigningslov15P"
                }
            }
        },
        "omkostninger": []
    });
    fremleje_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": [],
        "udbytter": []
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(fremleje_case);
    let mut property_income_case = json_input["cases"][0].clone();
    property_income_case["case_id"] = Value::String("personskat-ejendomsdrift-2026".into());
    property_income_case["input"]["kapitalindkomst"]["ejendomsdrift"] = serde_json::json!({
        "$variant": "MedEjendomsdriftEfterPar4Nr6",
        "fakta": {
            "kategori": { "$variant": "EjskLandzoneOver5000M2" },
            "beliggenhed": { "$variant": "EjskDanmark" },
            "erhvervsmæssigt_udlejet": false,
            "særlige_betingelser_for_nr6_til_nr8_opfyldt": true,
            "overskud_eller_underskud_kroner": 25_000
        }
    });
    property_income_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": [],
        "udbytter": []
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(property_income_case);
    let mut business_travel_case = json_input["cases"][0].clone();
    business_travel_case["case_id"] = Value::String("personskat-erhvervsbefordring-2026".into());
    let business_travel_a = serde_json::json!({
        "identifikation": "arbejdsgiver-a-kørsel-1",
        "rækkefølge_i_indkomståret": 1,
        "godtgørende_arbejdsgiver_identifikation": "arbejdsgiver-a",
        "køretøj": { "$variant": "Ll9BEgenBil" },
        "befordring": {
            "art": { "$variant": "Ll9BMellemArbejdspladser" },
            "kilometer_i_sagen": 19_500,
            "tres_dages_forhold": {
                "arbejdsdage_til_samme_arbejdsplads_inklusive_aktuel_dag_i_forudgående_12_måneder": 0,
                "sammenhængende_arbejdsdage_siden_sidst_på_arbejdspladsen": 0,
                "mange_forskellige_arbejdspladser": false,
                "ikke_sandsynligt_over_60_dage": false,
                "skriftligt_kørselsregnskabspålæg_aktivt": false,
                "kørselsregnskab_dokumenterer_erhvervsmæssig_befordring": false
            }
        },
        "udgifter": {
            "har_afholdt_befordringsudgifter": true,
            "dokumenterede_faktiske_kørselsudgifter_eksklusive_bro_tunnel_kroner": 4_000,
            "dokumenterede_bro_tunnel_udgifter_kroner": 500
        },
        "kundeopsøgende_aktivitet": false,
        "antal_arbejdsgivere_som_befordringen_vedrører_på_en_gang": 1,
        "godtgørelsesforhold": {
            "udbetalt_godtgørelse_kroner": 76_830,
            "form": { "$variant": "Ll9BKilometerafregnet" },
            "arbejdsgiver_har_kontrolleret_kilometer": true,
            "bogføringsbilag_opfylder_par6": true,
            "modregnet_i_forud_aftalt_bruttoløn": false,
            "firmabil_stillet_til_rådighed": false,
            "dokumenteret_kørsel_i_eget_køretøj": true,
            "fuldt_vederlag_betalt_for_firmabilskørsel_for_anden_arbejdsgiver": false,
            "overskydende_beløb_behandlet_som_løn_ved_endelig_opgørelse": true,
            "eventuel_godtgørelse_valgt_medregnet_i_indkomsten": false
        }
    });
    let mut business_travel_b = business_travel_a.clone();
    business_travel_b["identifikation"] = Value::String("arbejdsgiver-b-kørsel-1".into());
    business_travel_b["rækkefølge_i_indkomståret"] = Value::from(2);
    business_travel_b["godtgørende_arbejdsgiver_identifikation"] =
        Value::String("arbejdsgiver-b".into());
    business_travel_b["befordring"]["kilometer_i_sagen"] = Value::from(1_000);
    business_travel_b["godtgørelsesforhold"]["udbetalt_godtgørelse_kroner"] = Value::from(3_940);
    let mut business_travel_a_second = business_travel_a.clone();
    business_travel_a_second["identifikation"] = Value::String("arbejdsgiver-a-kørsel-2".into());
    business_travel_a_second["rækkefølge_i_indkomståret"] = Value::from(3);
    business_travel_a_second["befordring"]["kilometer_i_sagen"] = Value::from(1_000);
    business_travel_a_second["godtgørelsesforhold"]["udbetalt_godtgørelse_kroner"] =
        Value::from(3_510);
    business_travel_case["input"]["lønmodtager"]["erhvervsbefordring"] = serde_json::json!({
        "sager": [business_travel_a, business_travel_b, business_travel_a_second]
    });
    business_travel_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": [],
        "udbytter": []
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(business_travel_case);
    let mut pbl53a_case = json_input["cases"][0].clone();
    pbl53a_case["case_id"] = Value::String("personskat-pbl53a-2026".into());
    pbl53a_case["input"]["kapitalindkomst"]["pbl53a"] = serde_json::json!({
        "ordninger": [
            {
                "identifikation": "livsforsikring-pal",
                "skatteyder_identifikation": "person-1",
                "omfangsfakta": {
                    "oprettelsesdato": { "år": 1990, "måned": 1, "dag": 1 },
                    "oprindelig_rettighedshaver_identifikation": "tidligere-ejer",
                    "kapitalværdi_ved_oprettelsen_kroner": 100_000,
                    "repræsenteret_kontraktdel": { "$variant": "Pbl53AHeleKontrakten" },
                    "kontraktændringer": [
                        {
                            "identifikation": "valuta-2014",
                            "ændringsdato": { "år": 2014, "måned": 6, "dag": 1 },
                            "virkningstidspunkt": {
                                "dato": { "år": 2014, "måned": 6, "dag": 1 },
                                "rækkefølge_på_dagen": 1
                            },
                            "kapitalværdi_på_virkningstidspunktet": {
                                "$variant": "Pbl53AKapitalværdiIkkeRelevant"
                            },
                            "forhåndsaftale": {
                                "$variant": "Pbl53AIngenDokumenteretForhåndsaftale"
                            },
                            "art": { "$variant": "Pbl53AValutaÆndret" }
                        }
                    ],
                    "erhvervelser": [
                        {
                            "identifikation": "arv-2024",
                            "tidspunkt": {
                                "dato": { "år": 2024, "måned": 3, "dag": 15 },
                                "rækkefølge_på_dagen": 1
                            },
                            "overdrager_identifikation": "tidligere-ejer",
                            "erhverver_identifikation": "person-1",
                            "kapitalværdi_på_erhvervelsestidspunktet_kroner": 200_000,
                            "måde": { "$variant": "Pbl53AErhvervetVedArv" }
                        }
                    ],
                    "overgangsvalgfristfakta": {
                        "$variant": "Pbl53ASenereArvUnderFuldSkattepligt",
                        "arvedato": { "år": 2024, "måned": 3, "dag": 15 },
                        "oplysningsfrist": { "år": 2025, "måned": 7, "dag": 1 },
                        "fuld_skattepligtig_på_arvedatoen": true,
                        "tidligere_ejer_fuld_skattepligtig_i_ejerperioden": false,
                        "tidligere_ejer_havde_valgt_afsnit_iia": false
                    },
                    "overgangsvalg": [
                        {
                            "beslutningsdato": { "år": 2024, "måned": 4, "dag": 1 },
                            "modtagelsesdato": { "år": 2024, "måned": 4, "dag": 3 },
                            "mål": { "$variant": "Pbl53AValgAfPar53A" },
                            "modtager": { "$variant": "Pbl53AValgMeddeltSkattestyrelsen" },
                            "ønsket_virkning": { "$variant": "Pbl53AValgVirkningFraModtagelse" }
                        }
                    ],
                    "historiske_blanket49020_indsendelser": [
                        {
                            "indsendelsesdato": { "år": 2024, "måned": 4, "dag": 2 },
                            "modtagelsesdato": { "år": 2024, "måned": 4, "dag": 2 },
                            "udgave": { "$variant": "Pbl53ANyBlanket49020MedValgfelt" },
                            "modtager": { "$variant": "Pbl53AValgMeddeltSkattestyrelsen" },
                            "påberåbelse": { "$variant": "Pbl53AValgEfterPar53AEllerPar53BPåberåbt" },
                            "ønsket_virkning": { "$variant": "Pbl53AValgVirkningFraModtagelse" }
                        }
                    ],
                    "produkt": {
                        "$variant": "Pbl53ALivsforsikringsprodukt",
                        "ejer_identifikation": "person-1",
                        "forsikret_identifikation": "person-1",
                        "kapitel1fakta": { "$variant": "Pbl53AIkkeOmfattetAfKapitel1" },
                        "vilkår": {
                            "dækninger": [
                                { "$variant": "Pbl53AAndenLivsforsikringsdækning" }
                            ],
                            "aftalt_udløbsdato": { "år": 2050, "måned": 1, "dag": 1 },
                            "første_policedag_efter_fyldte_80_år": { "år": 2060, "måned": 1, "dag": 1 }
                        },
                        "direktørsikkerhed": { "$variant": "Pbl53AIngenDirektørsikkerhed" }
                    },
                    "afsnit_i_valg": { "$variant": "Pbl53AIntetAfkaldPåAfsnitI" },
                    "institutionsfinansiering": {
                        "samlet_drift_løn_og_pension_kroner": 1_000_000,
                        "statsligt_finansieret_drift_løn_og_pension_kroner": 0
                    },
                    "par53b_oprettelsesposition": {
                        "$variant": "Pbl53BOprettetUnderDanskSkattepligtOgHjemsted"
                    }
                },
                "afkastforløbsåbning": {
                    "$variant": "Pbl53AIngenTidligereAfkasthistorik"
                },
                "afkastår": [
                    {
                        "indkomstår": 2025,
                        "afkastgrundlag": {
                            "$variant": "Pbl53AAfkastEfterPal",
                            "afkast_efter_pal_par3_til_5_kroner": -6_000
                        },
                        "pensionsudbyder_opgjorde_afkast_efter_pal": true,
                        "skattepligtsstatus_ved_årets_begyndelse": {
                            "$variant": "Pbl53ASkattepligtigVedÅretsBegyndelse"
                        },
                        "sikkerhedsstatus_ved_årets_begyndelse": {
                            "$variant": "Pbl53ASikkerhedIkkeRelevant"
                        },
                        "grænsehændelser": [],
                        "afkastfordeling": {
                            "$variant": "Pbl53AAfledtEnkeltRettighedshaver"
                        }
                    },
                    {
                        "indkomstår": 2026,
                        "afkastgrundlag": {
                            "$variant": "Pbl53AAfkastEfterPal",
                            "afkast_efter_pal_par3_til_5_kroner": 28_000
                        },
                        "pensionsudbyder_opgjorde_afkast_efter_pal": true,
                        "skattepligtsstatus_ved_årets_begyndelse": {
                            "$variant": "Pbl53ASkattepligtigVedÅretsBegyndelse"
                        },
                        "sikkerhedsstatus_ved_årets_begyndelse": {
                            "$variant": "Pbl53ASikkerhedIkkeRelevant"
                        },
                        "grænsehændelser": [],
                        "afkastfordeling": {
                            "$variant": "Pbl53AAfledtEnkeltRettighedshaver"
                        }
                    }
                ],
                "hændelser": [
                    {
                        "$variant": "Pbl53AIndbetaling",
                        "fakta": {
                            "identifikation": "arbejdsgiver-indbetaling-2026",
                            "tidspunkt": {
                                "dato": { "år": 2026, "måned": 3, "dag": 1 },
                                "rækkefølge_på_dagen": 1
                            },
                            "beløb_kroner": 60_000,
                            "periode": {
                                "$variant": "Pbl53AIndbetaltMensOrdningenErOmfattet"
                            },
                            "indbetaler": {
                                "$variant": "Pbl53ANuværendeArbejdsgiver"
                            },
                            "ejerens_fradragsstatus": {
                                "$variant": "Pbl53AUdenFradragsEllerBortseelsesret"
                            },
                            "par53b_udenlandsk_skattebehandling": {
                                "$variant": "Pbl53BIkkeForetagetIUdenlandsperioden"
                            }
                        }
                    }
                ]
            },
            {
                "identifikation": "pensionskasse-negativ",
                "skatteyder_identifikation": "person-1",
                "omfangsfakta": {
                    "oprettelsesdato": { "år": 2020, "måned": 1, "dag": 1 },
                    "oprindelig_rettighedshaver_identifikation": "person-1",
                    "kapitalværdi_ved_oprettelsen_kroner": 100_000,
                    "repræsenteret_kontraktdel": { "$variant": "Pbl53AHeleKontrakten" },
                    "kontraktændringer": [],
                    "erhvervelser": [],
                    "overgangsvalgfristfakta": {
                        "$variant": "Pbl53AIntetOvergangsvalgfristgrundlag"
                    },
                    "overgangsvalg": [],
                    "historiske_blanket49020_indsendelser": [],
                    "produkt": {
                        "$variant": "Pbl53APensionskasseprodukt",
                        "pensionsberettiget_identifikation": "person-1",
                        "kapitel1fakta": { "$variant": "Pbl53AIkkeOmfattetAfKapitel1" },
                        "karakteristika": {
                            "selvstændig_juridisk_person": true,
                            "uafhængig_af_arbejdsgiver": true,
                            "midler_afsondret_fra_berettigedes_formue": true,
                            "vedtægter_fastlægger_pensionsydelser": true
                        },
                        "direktørsikkerhed": { "$variant": "Pbl53AIngenDirektørsikkerhed" }
                    },
                    "afsnit_i_valg": { "$variant": "Pbl53AIntetAfkaldPåAfsnitI" },
                    "institutionsfinansiering": {
                        "samlet_drift_løn_og_pension_kroner": 1_000_000,
                        "statsligt_finansieret_drift_løn_og_pension_kroner": 0
                    },
                    "par53b_oprettelsesposition": {
                        "$variant": "Pbl53BOprettetUnderDanskSkattepligtOgHjemsted"
                    }
                },
                "afkastforløbsåbning": {
                    "$variant": "Pbl53AIngenTidligereAfkasthistorik"
                },
                "afkastår": [
                    {
                        "indkomstår": 2025,
                        "afkastgrundlag": {
                            "$variant": "Pbl53AAlternativtKapitalværdiAfkast",
                            "kalenderårets_primo_depotværdi_kroner": 100_000,
                            "kalenderårets_ultimo_depotværdi_kroner": 95_000
                        },
                        "pensionsudbyder_opgjorde_afkast_efter_pal": false,
                        "skattepligtsstatus_ved_årets_begyndelse": {
                            "$variant": "Pbl53ASkattepligtigVedÅretsBegyndelse"
                        },
                        "sikkerhedsstatus_ved_årets_begyndelse": {
                            "$variant": "Pbl53ASikkerhedIkkeRelevant"
                        },
                        "grænsehændelser": [],
                        "afkastfordeling": {
                            "$variant": "Pbl53AAfledtEnkeltRettighedshaver"
                        }
                    },
                    {
                        "indkomstår": 2026,
                        "afkastgrundlag": {
                            "$variant": "Pbl53AAlternativtKapitalværdiAfkast",
                            "kalenderårets_primo_depotværdi_kroner": 140_000,
                            "kalenderårets_ultimo_depotværdi_kroner": 132_000
                        },
                        "pensionsudbyder_opgjorde_afkast_efter_pal": false,
                        "skattepligtsstatus_ved_årets_begyndelse": {
                            "$variant": "Pbl53ASkattepligtigVedÅretsBegyndelse"
                        },
                        "sikkerhedsstatus_ved_årets_begyndelse": {
                            "$variant": "Pbl53ASikkerhedIkkeRelevant"
                        },
                        "grænsehændelser": [],
                        "afkastfordeling": {
                            "$variant": "Pbl53AAfledtEnkeltRettighedshaver"
                        }
                    }
                ],
                "hændelser": []
            },
            {
                "identifikation": "pengeinstitut-halv-andel",
                "skatteyder_identifikation": "person-1",
                "omfangsfakta": {
                    "oprettelsesdato": { "år": 2020, "måned": 1, "dag": 1 },
                    "oprindelig_rettighedshaver_identifikation": "person-1",
                    "kapitalværdi_ved_oprettelsen_kroner": 190_000,
                    "repræsenteret_kontraktdel": { "$variant": "Pbl53AHeleKontrakten" },
                    "kontraktændringer": [],
                    "erhvervelser": [],
                    "overgangsvalgfristfakta": {
                        "$variant": "Pbl53AIntetOvergangsvalgfristgrundlag"
                    },
                    "overgangsvalg": [],
                    "historiske_blanket49020_indsendelser": [],
                    "produkt": {
                        "$variant": "Pbl53APengeEllerKreditinstitutprodukt",
                        "kontohaver_identifikation": "person-1",
                        "kapitel1fakta": { "$variant": "Pbl53AIkkeOmfattetAfKapitel1" },
                        "institutionssted": { "$variant": "Pbl53ADanskInstitution" },
                        "karakteristika": {
                            "standardiseret_lovreguleret_pensionsprodukt": true,
                            "pensionsmidler_adskilt_fra_øvrig_formue": true,
                            "kan_disponeres_som_almindelig_bankkonto": false
                        }
                    },
                    "afsnit_i_valg": { "$variant": "Pbl53AIntetAfkaldPåAfsnitI" },
                    "institutionsfinansiering": {
                        "samlet_drift_løn_og_pension_kroner": 1_000_000,
                        "statsligt_finansieret_drift_løn_og_pension_kroner": 0
                    },
                    "par53b_oprettelsesposition": {
                        "$variant": "Pbl53BOprettetUnderDanskSkattepligtOgHjemsted"
                    }
                },
                "afkastforløbsåbning": {
                    "$variant": "Pbl53AIngenTidligereAfkasthistorik"
                },
                "afkastår": [
                    {
                        "indkomstår": 2026,
                        "afkastgrundlag": {
                            "$variant": "Pbl53AAlternativtKapitalværdiAfkast",
                            "kalenderårets_primo_depotværdi_kroner": 190_000,
                            "kalenderårets_ultimo_depotværdi_kroner": 227_000
                        },
                        "pensionsudbyder_opgjorde_afkast_efter_pal": false,
                        "skattepligtsstatus_ved_årets_begyndelse": {
                            "$variant": "Pbl53AIkkeSkattepligtigVedÅretsBegyndelse"
                        },
                        "sikkerhedsstatus_ved_årets_begyndelse": {
                            "$variant": "Pbl53ASikkerhedIkkeRelevant"
                        },
                        "grænsehændelser": [
                            {
                                "identifikation": "skattepligt-indtræder-2026",
                                "tidspunkt": {
                                    "dato": { "år": 2026, "måned": 3, "dag": 1 },
                                    "rækkefølge_på_dagen": 1
                                },
                                "depotværdi_kroner": 200_000,
                                "art": { "$variant": "Pbl53ASkattepligtIndtræder" }
                            }
                        ],
                        "afkastfordeling": {
                            "$variant": "Pbl53AFlereBerettigedeVedAfkastperiodensUdgang",
                            "rettighedsperiodereference": {
                                "$variant": "Pbl53ARettighedsperiodeFraOprettelsen"
                            },
                            "samlet_indestående_ved_afkastperiodens_udgang_kroner": 400_000,
                            "andele": [
                                { "identifikation": "person-1", "indestående_ved_afkastperiodens_udgang_kroner": 200_000 },
                                { "identifikation": "person-2", "indestående_ved_afkastperiodens_udgang_kroner": 200_000 }
                            ]
                        }
                    }
                ],
                "hændelser": []
            }
        ]
    });
    pbl53a_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": [],
        "udbytter": []
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(pbl53a_case.clone());
    let mut historical_form_case = pbl53a_case.clone();
    historical_form_case["case_id"] =
        Value::String("personskat-pbl53a-historisk-blanket-2026".into());
    let mut historical_form_order =
        historical_form_case["input"]["kapitalindkomst"]["pbl53a"]["ordninger"][0].clone();
    historical_form_order["identifikation"] =
        Value::String("livsforsikring-historisk-blanket".into());
    historical_form_order["omfangsfakta"]["oprindelig_rettighedshaver_identifikation"] =
        Value::String("person-1".into());
    historical_form_order["omfangsfakta"]["erhvervelser"] = serde_json::json!([]);
    historical_form_order["omfangsfakta"]["overgangsvalgfristfakta"] = serde_json::json!({
        "$variant": "Pbl53ASenereIndtrådtFuldSkattepligt",
        "indtrædelsesdato": { "år": 2020, "måned": 7, "dag": 1 },
        "oplysningsfrist": { "år": 2021, "måned": 7, "dag": 1 }
    });
    historical_form_order["omfangsfakta"]["overgangsvalg"] = serde_json::json!([]);
    historical_form_order["omfangsfakta"]["historiske_blanket49020_indsendelser"] = serde_json::json!([
        {
            "indsendelsesdato": { "år": 2021, "måned": 4, "dag": 30 },
            "modtagelsesdato": { "år": 2021, "måned": 4, "dag": 30 },
            "udgave": { "$variant": "Pbl53ATidligereBlanket49020UdenValgfelt" },
            "modtager": { "$variant": "Pbl53AValgMeddeltSkattestyrelsen" },
            "påberåbelse": { "$variant": "Pbl53AValgEfterPar53AEllerPar53BPåberåbt" },
            "ønsket_virkning": { "$variant": "Pbl53AValgVirkningFraModtagelse" }
        }
    ]);
    historical_form_case["input"]["kapitalindkomst"]["pbl53a"]["ordninger"] =
        serde_json::json!([historical_form_order]);
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(historical_form_case.clone());
    let mut rejected_historical_form_case = historical_form_case;
    rejected_historical_form_case["case_id"] =
        Value::String("personskat-pbl53a-ny-blanket-uden-maal-2026".into());
    rejected_historical_form_case["input"]["kapitalindkomst"]["pbl53a"]["ordninger"][0]
        ["omfangsfakta"]["historiske_blanket49020_indsendelser"][0]["udgave"] =
        serde_json::json!({ "$variant": "Pbl53ANyBlanket49020MedValgfelt" });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(rejected_historical_form_case);
    let mut material_contract_change_case = pbl53a_case.clone();
    material_contract_change_case["case_id"] =
        Value::String("personskat-pbl53a-material-kontraktændring-2026".into());
    let mut material_contract_change_order =
        material_contract_change_case["input"]["kapitalindkomst"]["pbl53a"]["ordninger"][0].clone();
    material_contract_change_order["identifikation"] =
        Value::String("livsforsikring-material-kontraktændring".into());
    material_contract_change_order["omfangsfakta"]["oprindelig_rettighedshaver_identifikation"] =
        Value::String("person-1".into());
    material_contract_change_order["omfangsfakta"]["erhvervelser"] = serde_json::json!([]);
    material_contract_change_order["omfangsfakta"]["overgangsvalgfristfakta"] =
        serde_json::json!({ "$variant": "Pbl53AIntetOvergangsvalgfristgrundlag" });
    material_contract_change_order["omfangsfakta"]["overgangsvalg"] = serde_json::json!([]);
    material_contract_change_order["omfangsfakta"]["historiske_blanket49020_indsendelser"] =
        serde_json::json!([]);
    material_contract_change_order["omfangsfakta"]["kontraktændringer"] = serde_json::json!([
        {
            "identifikation": "markedsrente-2013",
            "ændringsdato": { "år": 2013, "måned": 6, "dag": 1 },
            "virkningstidspunkt": {
                "dato": { "år": 2013, "måned": 6, "dag": 1 },
                "rækkefølge_på_dagen": 1
            },
            "kapitalværdi_på_virkningstidspunktet": {
                "$variant": "Pbl53AHeleOrdningensKapitalværdi",
                "kroner": 140_000
            },
            "forhåndsaftale": {
                "$variant": "Pbl53AIngenDokumenteretForhåndsaftale"
            },
            "art": { "$variant": "Pbl53AGennemsnitsrenteSkiftetTilMarkedsrente" }
        }
    ]);
    material_contract_change_case["input"]["kapitalindkomst"]["pbl53a"]["ordninger"] =
        serde_json::json!([material_contract_change_order]);
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(material_contract_change_case.clone());
    let mut unsupported_contract_change_case = material_contract_change_case;
    unsupported_contract_change_case["case_id"] =
        Value::String("personskat-pbl53a-uafklaret-kontraktændring-2026".into());
    unsupported_contract_change_case["input"]["kapitalindkomst"]["pbl53a"]["ordninger"][0]
        ["omfangsfakta"]["kontraktændringer"][0]["art"] = serde_json::json!({
        "$variant": "Pbl53AAndenKontraktændring",
        "beskrivelse": "Sammensat ændring uden direkte klassifikation"
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(unsupported_contract_change_case);
    let mut spouse_case = json_input["cases"][0].clone();
    spouse_case["case_id"] = Value::String("personskat-aegtefaelleoverfoersler-2025".into());
    spouse_case["input"]["lønmodtager"]["skatteår"] = serde_json::json!(2025);
    spouse_case["input"]["lønmodtager"]["kommune"] =
        serde_json::json!({ "$variant": "Frederiksberg" });
    spouse_case["input"]["aktieavance"]["særlige_aktiver"] = serde_json::json!([]);
    spouse_case["input"]["aktieavance"]["ordinært_aktieår"] = serde_json::json!({
        "$variant": "MedOrdinærtAktieår",
        "input": {
            "indkomstår": 2025,
            "hændelsesforløb": [],
            "investeringsbeviser": [{
                "indkomstår": 2025,
                "art": { "$variant": "AblPar21AktiebaseretMinimumsbevis" },
                "afståelsessum_kroner": 50_000,
                "anskaffelsessum_kroner": 100_000,
                "oplysningsstatus": { "$variant": "AblOplystRettidigt" }
            }],
            "fremført_tab_efter_par13a_kroner": 0
        }
    });
    spouse_case["input"]["ægtefælle"] = serde_json::json!({
        "$variant": "MedÆgtefælle",
        "fakta": {
            "lønmodtager": {
                "skatteår": 2025,
                "kommune": { "$variant": "Frederiksberg" },
                "bruttoløn_kroner": 0,
                "personlig_indkomst": {
                    "etableringskonto": { "$variant": "UdenEtableringskontoindskud" },
                    "ordinære_forhold": {
                        "arbejdsgiverydelser": [],
                        "virksomheder_uden_virksomhedsordning": []
                    }
                },
                "erhvervsbefordring": { "sager": [] },
                "ligningsfradrag": {
                    "befordring": { "$variant": "UdenBefordringsfradrag" }
                },
                "pension": {
                    "pensionsalder_status": {
                        "$variant": "Ll9lMereEnd15ÅrFørFolkepension"
                    },
                    "pbl18_indbetalinger": [],
                    "pbl18_selvstændig_overskud": {
                        "skattepligtigt_overskud_før_vsl22b_kroner": 0,
                        "renteudgifter_kroner": 0,
                        "kurstab_kroner": 0,
                        "renteindtægter_kroner": 0,
                        "udbytteindtægter_kroner": 0,
                        "kursgevinster_kroner": 0,
                        "udelukkede_afståelsesindkomster_kroner": 0
                    },
                    "pbl18_livrentevalg": { "$variant": "Pbl18FordeltFradrag" },
                    "pbl15a_årsgrundlag": {
                        "afståelser": [],
                        "ordninger": [],
                        "kvalifikationsår": [],
                        "tidligere_indbetalinger": []
                    },
                    "pbl15b_årsgrundlag": {
                        "indkomstposter": [],
                        "ordninger": [],
                        "tidligere_indbetalinger": [],
                        "rateudbetalinger": []
                    },
                    "øvrige_pbl20_årsgrundlag": { "udbetalinger": [] },
                    "aktiepensionsfradrag_valg": {
                        "$variant": "UdenAktiepensionsfradragIAktieindkomst"
                    }
                },
                "personfradrag_alder_status": { "$variant": "Fyldt18EllerGift" },
                "betaler_kirkeskat": false
            },
            "kapitalindkomst": {
                "renter": {
                    "renteindtægter_kroner": 0,
                    "renteudgifter_kroner": 39_617,
                    "næringsstatus": { "$variant": "IkkeNæring" },
                    "ligningslov6": { "$variant": "UdenLigningslov6Kurstab" },
                    "ligningslov6a": { "$variant": "UdenLigningslov6AFradrag" }
                },
                "virksomhedskapital": {
                    "selvstændig_beskatningsordning": {
                        "$variant": "UdenVirksomhedsEllerKapitalafkastordning"
                    },
                    "medarbejderaktier": {
                        "$variant": "UdenKapitalafkastEfterVirksomhedsskattelov22C"
                    }
                },
                "pbl53a": { "ordninger": [] },
                "ejendomsdrift": { "$variant": "UdenEjendomsdriftEfterPar4Nr6" },
                "ejendomsavance": { "$variant": "UdenEjendomsavance" },
                "kursgevinst": { "$variant": "UdenKursgevinst" },
                "fremleje": { "$variant": "UdenFremlejeEfterLigningslov15Q" },
                "omkostninger": []
            },
            "aktieavance": {
                "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
                "særlige_aktiver": [],
                "udbytter": [{
                    "identifikation": "par13a-aegtefaelle-udbytte",
                    "udlodder": { "$variant": "Ll16AAlmindeligtSelskab" },
                    "modtager": { "$variant": "Ll16AAktuelAktionær" },
                    "aktiv": { "$variant": "PersonskatAlmindeligAktie" },
                    "beløb_kroner": 12_000,
                    "par13a_kildefakta": {
                        "$variant": "AblPar13AUdbytteForMarkedsaktieEfterPar12",
                        "markedsstatus": {
                            "$variant": "AblOptagetTilHandelPåReguleretMarked"
                        },
                        "aktivklassifikation": {
                            "indkomstår": 2025,
                            "aktiv": { "$variant": "AblOrdinærAktiePar12Til15" },
                            "par17_modprøve": {
                                "næringsstatus": {
                                    "$variant": "AblPar17UdøverIkkeNæringVedKøbOgSalgAfAktier"
                                },
                                "erhvervelsesstatus": {
                                    "$variant": "AblPar17IkkeErhvervetSomLedINæringsvej"
                                }
                            },
                            "investeringsklassifikation": {
                                "$variant": "AblIngenInvesteringsklassifikation"
                            }
                        }
                    }
                }]
            },
            "udenlandske_sociale_bidrag": {
                "$variant": "UdenUdenlandskeSocialeBidragEfterLigningslov8M"
            },
            "cfc": { "poster": [] },
            "skatteforhold": { "$variant": "StandardSkatteforhold" },
            "underskudsforhold": { "$variant": "StandardUnderskudsforhold" }
        },
        "samlevende_ved_indkomstårets_udløb": true
    });
    let par37_spouse_relationship = spouse_case["input"]["ægtefælle"].clone();
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(spouse_case);
    let mut property_tax_case = json_input["cases"][0].clone();
    property_tax_case["case_id"] = Value::String("personskat-ejendomsskatter-2025".into());
    property_tax_case["input"]["lønmodtager"]["skatteår"] = serde_json::json!(2025);
    property_tax_case["input"]["aktieavance"]["særlige_aktiver"] = serde_json::json!([]);
    property_tax_case["input"]["ejendomsskatter"]["ejendomme"] = serde_json::json!([{
        "ordinært_grundlag": {
            "identifikation": "ejerbolig-1",
            "kommune": { "$variant": "København" },
            "kategori": { "$variant": "EjskEnBoligenhed" },
            "beliggenhed": { "$variant": "EjskDanmark" },
            "erhvervsmæssigt_udlejet": false,
            "særlige_betingelser_for_nr6_til_nr8_opfyldt": true,
            "ejendomsværdi_kroner": 3_710_000,
            "grundværdi_kroner": 3_363_000,
            "produktionsjord": false,
            "ejendomsværdiskatteperiode": {
                "$variant": "EjendomsskatFraOgMed",
                "dato": { "år": 2025, "måned": 8, "dag": 1 }
            },
            "grundskyldsperiode": {
                "$variant": "EjendomsskatFraOgMed",
                "dato": { "år": 2025, "måned": 8, "dag": 1 }
            },
            "ejerandel_basispoint": 5_000
        },
        "nedslagsfakta": {
            "ejerskabshistorik": {
                "oprindelig_erhvervelsesdato": { "år": 2025, "måned": 8, "dag": 1 },
                "ejerskifter": []
            },
            "boliganvendelse": { "$variant": "EjskHelårsbolig" },
            "selvstændige_boligenheder": 1,
            "ejendomsform": { "$variant": "EjskIkkeEjerlejlighed" },
            "fredet_og_omfattet_af_ligningslovens_par15k": false,
            "par24_beregningsgrundlag": {
                "$variant": "EjskPar24SammeVærdiSomPar13"
            },
            "pensionistsuccession": { "$variant": "EjskIngenPensionistsuccession" },
            "udenlandske_ejendomsskatter": []
        },
        "overgangsomfang": {
            "vurderingskategori": {
                "$variant": "EjskEjerboligEfterEjendomsvurderingslovensPar3Stk1Nr1"
            },
            "ejerkreds": { "$variant": "EjskKunFysiskeEjere" }
        },
        "overgangsvurderinger": {
            "rabat": { "$variant": "EjskIngenRabatvurderingerOplyst" },
            "stigningsbegrænsning": {
                "$variant": "EjskIngenStigningsvurderingerOplyst"
            }
        }
    }]);
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(property_tax_case);
    let mut debt_case = json_input["cases"][0].clone();
    debt_case["case_id"] = Value::String("personskat-kgl-gaeld-2026".into());
    debt_case["input"]["kapitalindkomst"]["kursgevinst"] = serde_json::json!({
        "$variant": "MedKursgevinst",
        "fakta": {
            "skatteyder_identifikation": "Borger",
            "ægtefælles_skatteyder_identifikation": null,
            "sælgerpantebreve": [],
            "gældsposter": [{
                "identifikation": "USD-lån",
                "beløb": {
                    "gældens_værdi_ved_påtagelse_kroner": 100_000,
                    "gældens_værdi_ved_frigørelse_eller_indfrielse_kroner": 97_000,
                    "fordringens_værdi_for_kreditor_kroner": 97_000
                },
                "frigørelsesart": { "$variant": "KglGældOrdinærIndfrielse" },
                "erhvervsforhold": { "$variant": "KglGældUdenFinansieringsnæring" },
                "valuta": { "$variant": "KglGældFremmedValuta" },
                "selskabsfakta": { "$variant": "KglIngenPar21Stk2Selskabsgæld" },
                "gældsordning": { "$variant": "KglIngenDokumenteretGældsordning" },
                "vedrører_ikke_indbetalt_selskabskapital": false,
                "par22_hændelse": { "$variant": "KglIngenPar22Hændelse" }
            }],
            "øvrige_instrumenter": {
                "fordringer": [],
                "obligationsbaserede_minimumsbeviser": []
            },
            "par32_kontraktforløb": {
                "$variant": "UdenPar32Kontraktforløb"
            }
        }
    });
    debt_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": [],
        "udbytter": []
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(debt_case);
    let mut annual_claim_case = json_input["cases"][0].clone();
    annual_claim_case["case_id"] = Value::String("personskat-kgl-aarsnetto-fordring-2026".into());
    annual_claim_case["input"]["kapitalindkomst"]["kursgevinst"] = serde_json::json!({
        "$variant": "MedKursgevinst",
        "fakta": {
            "skatteyder_identifikation": "Borger",
            "ægtefælles_skatteyder_identifikation": null,
            "sælgerpantebreve": [],
            "gældsposter": [],
            "øvrige_instrumenter": {
                "fordringer": [{
                    "identifikation": "privat-fordring-2026",
                    "kilde": {
                        "fordringsart": { "$variant": "KglÅrsnettoPengefordring" },
                        "markedsfakta": {
                            "$variant": "KglÅrsnettoIkkeOptagetPåReguleretMarked"
                        },
                        "debitorrelation": { "$variant": "KglÅrsnettoUafhængigDebitor" },
                        "næringsforhold": { "$variant": "KglÅrsnettoIkkeNæringsdrivende" },
                        "erhvervelsesgrundlag": {
                            "$variant": "KglÅrsnettoAlmindeligErhvervelse"
                        },
                        "dba_status": { "$variant": "KglÅrsnettoIngenDbaBegrænsning" },
                        "opgørelsesprincip": { "$variant": "KglRealisationsprincip" }
                    },
                    "position_primo": { "$variant": "KglÅrsnettoIngenPositionPrimo" },
                    "hændelser": [
                        {
                            "$variant": "KglÅrsnettoAnskaffelse",
                            "anskaffelsessum_kroner": 10_000
                        },
                        {
                            "$variant": "KglÅrsnettoAfståelse",
                            "afståelsessum_kroner": 13_000
                        }
                    ]
                }],
                "obligationsbaserede_minimumsbeviser": []
            },
            "par32_kontraktforløb": {
                "$variant": "UdenPar32Kontraktforløb"
            }
        }
    });
    annual_claim_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": [],
        "udbytter": []
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(annual_claim_case);
    let mut voluntary_arrangement_case = json_input["cases"][0].clone();
    voluntary_arrangement_case["case_id"] =
        Value::String("personskat-kgl-frivillig-ordning-2026".into());
    voluntary_arrangement_case["input"]["kapitalindkomst"]["kursgevinst"] = serde_json::json!({
        "$variant": "MedKursgevinst",
        "fakta": {
            "skatteyder_identifikation": "Borger",
            "ægtefælles_skatteyder_identifikation": null,
            "sælgerpantebreve": [],
            "gældsposter": [{
                "identifikation": "hovedkrav-82",
                "beløb": {
                    "gældens_værdi_ved_påtagelse_kroner": 820_800,
                    "gældens_værdi_ved_frigørelse_eller_indfrielse_kroner": 150_000,
                    "fordringens_værdi_for_kreditor_kroner": 200_000
                },
                "frigørelsesart": { "$variant": "KglGældEftergivelse" },
                "erhvervsforhold": { "$variant": "KglGældUdenFinansieringsnæring" },
                "valuta": { "$variant": "KglGældDanskeKroner" },
                "selskabsfakta": { "$variant": "KglIngenPar21Stk2Selskabsgæld" },
                "gældsordning": {
                    "$variant": "KglFrivilligKreditorordning",
                    "fakta": {
                        "ordningsidentifikation": "skm2017-10-moenster",
                        "alle_usikrede_krav_oplyst": true,
                        "krav": [
                            {
                                "krav_identifikation": "hovedkrav-82",
                                "kreditor_identifikation": "hovedkreditor",
                                "samlet_krav_kroner": 820_800,
                                "værdi_af_tilstrækkelig_sikkerhed_kroner": 0,
                                "deltagelse": {
                                    "$variant": "KglKreditorTiltrådtFrivilligOrdning",
                                    "aftalt_restkrav_kroner": 150_000
                                }
                            },
                            {
                                "krav_identifikation": "småkrav-347",
                                "kreditor_identifikation": "kreditor-347",
                                "samlet_krav_kroner": 34_700,
                                "værdi_af_tilstrækkelig_sikkerhed_kroner": 0,
                                "deltagelse": {
                                    "$variant": "KglKreditorUdenforFrivilligOrdning",
                                    "småkravsgrundlag": {
                                        "$variant": "KglUdeladtKravDokumenteretSomSmåkrav",
                                        "afgørelsesreference": "SKM2017.10.SR"
                                    }
                                }
                            },
                            {
                                "krav_identifikation": "småkrav-717",
                                "kreditor_identifikation": "kreditor-717",
                                "samlet_krav_kroner": 71_700,
                                "værdi_af_tilstrækkelig_sikkerhed_kroner": 0,
                                "deltagelse": {
                                    "$variant": "KglKreditorUdenforFrivilligOrdning",
                                    "småkravsgrundlag": {
                                        "$variant": "KglUdeladtKravDokumenteretSomSmåkrav",
                                        "afgørelsesreference": "SKM2017.10.SR"
                                    }
                                }
                            },
                            {
                                "krav_identifikation": "småkrav-636",
                                "kreditor_identifikation": "kreditor-636",
                                "samlet_krav_kroner": 63_600,
                                "værdi_af_tilstrækkelig_sikkerhed_kroner": 0,
                                "deltagelse": {
                                    "$variant": "KglKreditorUdenforFrivilligOrdning",
                                    "småkravsgrundlag": {
                                        "$variant": "KglUdeladtKravDokumenteretSomSmåkrav",
                                        "afgørelsesreference": "SKM2017.10.SR"
                                    }
                                }
                            },
                            {
                                "krav_identifikation": "småkrav-92",
                                "kreditor_identifikation": "kreditor-92",
                                "samlet_krav_kroner": 9_200,
                                "værdi_af_tilstrækkelig_sikkerhed_kroner": 0,
                                "deltagelse": {
                                    "$variant": "KglKreditorUdenforFrivilligOrdning",
                                    "småkravsgrundlag": {
                                        "$variant": "KglUdeladtKravDokumenteretSomSmåkrav",
                                        "afgørelsesreference": "SKM2017.10.SR"
                                    }
                                }
                            }
                        ]
                    }
                },
                "vedrører_ikke_indbetalt_selskabskapital": false,
                "par22_hændelse": { "$variant": "KglIngenPar22Hændelse" }
            }],
            "øvrige_instrumenter": {
                "fordringer": [],
                "obligationsbaserede_minimumsbeviser": []
            },
            "par32_kontraktforløb": {
                "$variant": "UdenPar32Kontraktforløb"
            }
        }
    });
    voluntary_arrangement_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": [],
        "udbytter": []
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(voluntary_arrangement_case);
    let mut dividend_case = json_input["cases"][0].clone();
    dividend_case["case_id"] = Value::String("personskat-udbytte-2026".into());
    dividend_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": [],
        "udbytter": [{
            "identifikation": "udbytte-1",
            "udlodder": { "$variant": "Ll16AAlmindeligtSelskab" },
            "modtager": { "$variant": "Ll16AAktuelAktionær" },
            "aktiv": { "$variant": "PersonskatAlmindeligAktie" },
            "beløb_kroner": 12_000,
            "par13a_kildefakta": {
                "$variant": "AblPar13AUdbytteUdenForModregningsgrundlag"
            }
        }]
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(dividend_case);
    let mut establishment_account_case = json_input["cases"][0].clone();
    establishment_account_case["case_id"] =
        Value::String("personskat-etableringskonto-2026".into());
    establishment_account_case["input"]["aktieavance"]["særlige_aktiver"] = serde_json::json!([]);
    establishment_account_case["input"]["lønmodtager"]["personlig_indkomst"] = serde_json::json!({
        "etableringskonto": {
            "$variant": "MedEtableringskontoindskud",
            "input": {
                "indkomstår": 2026,
                "fase": { "$variant": "EtblFørEtablering" },
                "fuldt_skattepligtig": true,
                "skattemæssigt_hjemmehørende_i_andet_land_efter_dbo": false,
                "senest_indkomståret_efter_folkepensionsalderen": true,
                "tidligere_indskud_fuldt_hævet": true,
                "indskud_foretaget_i_indskudsåret": true,
                "betingelser_for_undladt_indskud_efter_par4_stk2_opfyldt": false,
                "faktisk_indskudsplacering": {
                    "placering": { "$variant": "EtblSærligIndlånskonto" },
                    "pengeinstitut_omfattet_af_par4_stk1": true,
                    "etableringskonto_og_iværksætterkonto_ført_særskilt": true,
                    "konto_korrekt_betegnet": true,
                    "navn_adresse_og_personnummer_påført": true,
                    "kontantkonto_og_depot_i_samme_pengeinstitut": true
                },
                "kontant_løn_kroner": 600_000,
                "skatteværdi_af_frit_ophold_og_andre_goder_kroner": 0,
                "skattepligtige_arbejdsgivergodtgørelser_kroner": 0,
                "ligningslov9_til_9d_fradrag_kroner": 0,
                "skattepligtigt_virksomhedsoverskud_efter_vsl22b_fradrag_kroner": 0,
                "vsl22b_henlæggelse_kroner": 0,
                "renteudgifter_og_kurstab_kroner": 0,
                "rente_udbytte_og_kursgevinst_kroner": 0,
                "forskudsafskrivning_efter_al29_kroner": 0,
                "faktisk_etableringskontoindskud_kroner": 0,
                "faktisk_iværksætterkontoindskud_kroner": 30_000,
                "undladt_etableringskontoindskud_efter_par4_stk2_kroner": 0,
                "undladt_iværksætterkontoindskud_efter_par4_stk2_kroner": 0
            }
        },
        "ordinære_forhold": {
            "arbejdsgiverydelser": [],
            "virksomheder_uden_virksomhedsordning": []
        }
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(establishment_account_case);
    let mut par35_case = json_input["cases"][0].clone();
    par35_case["case_id"] = Value::String("personskat-par35-medarbejdereje-2026".into());
    par35_case["input"]["aktieavance"]["særlige_aktiver"] = serde_json::json!([{
        "identifikation": "personskat-par35-2026",
        "kilde": {
            "$variant": "PersonskatMedarbejderejeordningEfterPar35G",
            "fakta": {
                "identifikation": "personskat-par35-2026",
                "opgørelsesår": 2026,
                "overdragelse": {
                    "overdragelsesår": 2026,
                    "overdrager_er_fysisk_person": true,
                    "hjemsted": { "$variant": "AblPar35GDanmark" },
                    "dansk_virksomhed_omfattet_af_sel_par1_stk1_nr2j": true,
                    "udenlandsk_virksomhed_svarer_til_sel_par1_stk1_nr2j": false,
                    "udenlandsk_virksomhed_opfylder_erhvervsvirksomhedslov_kap5c": false,
                    "udenlandsk_virksomhed_forpligter_sig_til_overdragerskat": false,
                    "udenlandsk_virksomhed_forpligter_sig_til_årsoplysninger": false,
                    "aktier_opfylder_par34_stk1_nr3": true,
                    "parterne_har_valgt_ordningen": true,
                    "meddelelse_rettidig": true,
                    "beholdningsoversigt_vedlagt": true,
                    "saldo_vedlagt": true,
                    "land_omfattet_af_inddrivelsesbistand": true,
                    "sikkerhedsform": { "$variant": "AblPar35GIngenSikkerhed" },
                    "sikkerhed_står_i_passende_forhold": false,
                    "partier": [{
                        "identifikation": "personskat-par35-negativt-parti",
                        "selskabsidentifikation": "DK-PERSONSKAT-PAR35",
                        "aktieserie": "ordinær",
                        "erhvervelsesrækkefølge": 1,
                        "antal": 100,
                        "skattemæssig_anskaffelsessum_kroner": -50_000,
                        "handelsværdi_kroner": 100_000
                    }]
                },
                "hændelsesposter": [{
                    "rækkefølge_i_indkomståret": 1,
                    "hændelse": {
                        "$variant": "AblPar35HændelseAfståelse",
                        "data": {
                            "hændelsesidentifikation": "personskat-par35-salg-2026",
                            "selskabsidentifikation": "DK-PERSONSKAT-PAR35",
                            "aktieserie": "ordinær",
                            "antal": 100,
                            "afståelsessum_kroner": 120_000,
                            "indkomstår": 2026,
                            "anden_betalt_skat_kroner": 0,
                            "godkendt_fradrag_efter_ligningslov_par33_kroner": 0
                        }
                    }
                }]
            }
        },
        "markedsstatus": { "$variant": "AblIkkeOptagetTilHandel" }
    }]);
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(par35_case);
    let mut par37_case = json_input["cases"][0].clone();
    par37_case["case_id"] = Value::String("personskat-par37-40-fraflytning-2026".into());
    par37_case["input"]["aktieavance"]["særlige_aktiver"] = serde_json::json!([{
        "identifikation": "personskat-fraflytning-2026",
        "kilde": {
            "$variant": "PersonskatFraflytteraktierEfterPar37Til40",
            "fakta": {
                "identifikation": "personskat-fraflytning-2026",
                "opgørelsesår": 2026,
                "fraflytning": {
                    "fraflytningsår": 2026,
                    "ophørsgrund": {
                        "$variant": "AblPar38OphørAfSkattepligtEfterKildeskattelovPar1"
                    },
                    "tilknytning": {
                        "$variant": "AblPar38MindstSyvÅrIndenForSenesteTiÅr"
                    },
                    "aktier": [{
                        "identifikation": "personskat-fraflytteraktie",
                        "selskabsidentifikation": "DK-PERSONSKAT-FRAFLYTNING",
                        "aktieserie": "ordinær",
                        "erhvervelsesdato": { "år": 2020, "måned": 4, "dag": 5 },
                        "erhvervelsesrækkefølge": 1,
                        "antal": 100,
                        "handelsværdi_ved_ophør_kroner": 200_000,
                        "skattemæssig_anskaffelsessum_kroner": 100_000,
                        "beskatningsstatus": {
                            "$variant": "AblPar38AktieOmfattetAfDanskBeskatning"
                        },
                        "par44_status": {
                            "$variant": "AblPar38IkkeHistoriskPar44Aktie"
                        },
                        "egenskaber": [],
                        "opgørelseskilde": {
                            "$variant": "AblPar37Til40OpgørelseEfterPar23Til29Og46"
                        },
                        "aktivgrundlag": {
                            "$variant": "AblPar38OrdinærAktieEfterPar12Til15",
                            "fakta": {
                                "markedsstatus": { "$variant": "AblIkkeOptagetTilHandel" },
                                "har_tidligere_været_optaget_til_handel": false,
                                "oplysningsstatus": {
                                    "$variant": "AblOplysningsbetingelseIkkeOpfyldt"
                                },
                                "par5a_kildefakta": {
                                    "$variant": "AblOrdinærIngenPar5AFaktaPåkrævet"
                                }
                            }
                        },
                        "princip": { "$variant": "AblPar23Realisationsprincip" },
                        "henstandsvalg": {
                            "$variant": "AblPar37Til40HenstandSøges"
                        }
                    }],
                    "kontekstgrundlag": {
                        "$variant": "AfledPar37Til40KontekstFraPersonskat"
                    },
                    "indberetning": {
                        "oplysninger_efter_skattekontrollov_par2": {
                            "$variant": "AblPar39RettidigOrdinærFrist"
                        },
                        "beholdningsoversigt_efter_par39a": {
                            "$variant": "AblPar39RettidigOrdinærFrist"
                        }
                    },
                    "bopæl": {
                        "oprindeligt_fraflytningsland": {
                            "$variant": "AblPar39LandOmfattetAfNordiskOverenskomstEllerEuDirektiv"
                        },
                        "aktuelt_land": {
                            "$variant": "AblPar39LandOmfattetAfNordiskOverenskomstEllerEuDirektiv"
                        },
                        "frigivelse_af_sikkerhed_anmodet": false
                    },
                    "sikkerhed": { "$variant": "AblPar39IngenSikkerhedStillet" }
                },
                "hændelsesposter": [],
                "tilflytning": { "$variant": "IngenTilflytningEfterPar39B" }
            }
        },
        "markedsstatus": { "$variant": "AblIkkeOptagetTilHandel" }
    }]);
    let mut par37_mixed_case = par37_case.clone();
    par37_mixed_case["case_id"] = Value::String("personskat-par37-40-blandet-slutskat-2026".into());
    par37_mixed_case["input"]["aktieavance"]["særlige_aktiver"][0]["identifikation"] =
        Value::String("personskat-fraflytning-blandet-2026".into());
    par37_mixed_case["input"]["aktieavance"]["særlige_aktiver"][0]["kilde"]["fakta"]
        ["identifikation"] = Value::String("personskat-fraflytning-blandet-2026".into());
    let mut mixed_ordinary = par37_case["input"]["aktieavance"]["særlige_aktiver"][0]["kilde"]
        ["fakta"]["fraflytning"]["aktier"][0]
        .clone();
    mixed_ordinary["identifikation"] = Value::String("blandet-ordinaer".into());
    mixed_ordinary["selskabsidentifikation"] = Value::String("DK-BLANDET-ORDINAER".into());
    mixed_ordinary["handelsværdi_ved_ophør_kroner"] = serde_json::json!(150_000);
    let mut mixed_trading = mixed_ordinary.clone();
    mixed_trading["identifikation"] = Value::String("blandet-naering".into());
    mixed_trading["selskabsidentifikation"] = Value::String("DK-BLANDET-NAERING".into());
    mixed_trading["erhvervelsesrækkefølge"] = serde_json::json!(2);
    mixed_trading["handelsværdi_ved_ophør_kroner"] = serde_json::json!(130_000);
    mixed_trading["aktivgrundlag"] = serde_json::json!({
        "$variant": "AblPar38SærligtAktiv",
        "fakta": {
            "klassifikation": {
                "indkomstår": 2026,
                "aktiv": { "$variant": "AblNæringsaktiePar17" },
                "par17_modprøve": {
                    "næringsstatus": {
                        "$variant": "AblPar17UdøverNæringVedKøbOgSalgAfAktier"
                    },
                    "erhvervelsesstatus": {
                        "$variant": "AblPar17ErhvervetSomLedINæringsvej"
                    }
                },
                "investeringsklassifikation": {
                    "$variant": "AblIngenInvesteringsklassifikation"
                }
            },
            "markedsstatus": { "$variant": "AblIkkeOptagetTilHandel" },
            "koncernintern_konvertibel_eller_tegningsret": false,
            "andelsforening_stiftet_før_22_maj_1987": false,
            "afståelse_sker_for_at_undgå_likvidationsbeskatning": false,
            "årets_netto_med_kgl_par14_23_kroner": 0
        }
    });
    let mut mixed_par19c = mixed_ordinary.clone();
    mixed_par19c["identifikation"] = Value::String("blandet-par19c".into());
    mixed_par19c["selskabsidentifikation"] = Value::String("DK-BLANDET-PAR19C".into());
    mixed_par19c["erhvervelsesrækkefølge"] = serde_json::json!(3);
    mixed_par19c["handelsværdi_ved_ophør_kroner"] = serde_json::json!(120_000);
    mixed_par19c["aktivgrundlag"] = serde_json::json!({
        "$variant": "AblPar38SærligtAktiv",
        "fakta": {
            "klassifikation": {
                "indkomstår": 2026,
                "aktiv": { "$variant": "AblInvesteringsselskabPar19TilKlassifikation" },
                "par17_modprøve": {
                    "næringsstatus": {
                        "$variant": "AblPar17UdøverIkkeNæringVedKøbOgSalgAfAktier"
                    },
                    "erhvervelsesstatus": {
                        "$variant": "AblPar17IkkeErhvervetSomLedINæringsvej"
                    }
                },
                "investeringsklassifikation": {
                    "$variant": "AblPar19BPar19CKlassifikation",
                    "input": {
                        "indkomstår": 2026,
                        "meddelelse": { "$variant": "AblIngenPar19BMeddelelse" },
                        "aktivmasse": {
                            "indkomstår": 2026,
                            "direkte_aktiver": [
                                {
                                    "$variant": "AblDirekteInvesteringsaktiv",
                                    "art": { "$variant": "AblKvalificerendeAktieaktiv" },
                                    "gennemsnitlig_værdi_kroner": 20_000
                                },
                                {
                                    "$variant": "AblDirekteInvesteringsaktiv",
                                    "art": { "$variant": "AblAndetVærdipapir" },
                                    "gennemsnitlig_værdi_kroner": 80_000
                                }
                            ],
                            "ejerposter": []
                        },
                        "oplysninger": { "$variant": "AblPar19BOplysningerIkkeIndsendt" }
                    }
                }
            },
            "markedsstatus": {
                "$variant": "AblOptagetTilHandelPåReguleretMarked"
            },
            "koncernintern_konvertibel_eller_tegningsret": false,
            "andelsforening_stiftet_før_22_maj_1987": false,
            "afståelse_sker_for_at_undgå_likvidationsbeskatning": false,
            "årets_netto_med_kgl_par14_23_kroner": 0
        }
    });
    mixed_par19c["princip"] = serde_json::json!({ "$variant": "AblPar23Lagerprincip" });
    mixed_par19c["henstandsvalg"] =
        serde_json::json!({ "$variant": "AblPar37Til40SkatBetalesStraks" });
    par37_mixed_case["input"]["aktieavance"]["særlige_aktiver"][0]["kilde"]["fakta"]
        ["fraflytning"]["aktier"] = Value::Array(vec![mixed_ordinary, mixed_trading, mixed_par19c]);
    let mut par37_spouse_case = par37_case.clone();
    par37_spouse_case["case_id"] = Value::String("personskat-par37-40-aegtefaelle-2026".into());
    par37_spouse_case["input"]["aktieavance"]["særlige_aktiver"][0]["identifikation"] =
        Value::String("personskat-fraflytning-aegtefaelle-2026".into());
    par37_spouse_case["input"]["aktieavance"]["særlige_aktiver"][0]["kilde"]["fakta"]
        ["identifikation"] = Value::String("personskat-fraflytning-aegtefaelle-2026".into());
    par37_spouse_case["input"]["aktieavance"]["særlige_aktiver"][0]["kilde"]["fakta"]
        ["fraflytning"]["aktier"][0]["identifikation"] =
        Value::String("personskat-fraflytteraktie-aegtefaelle".into());
    par37_spouse_case["input"]["aktieavance"]["særlige_aktiver"][0]["kilde"]["fakta"]
        ["fraflytning"]["aktier"][0]["selskabsidentifikation"] =
        Value::String("DK-PERSONSKAT-FRAFLYTNING-AEGTEFAELLE".into());
    par37_spouse_case["input"]["ægtefælle"] = par37_spouse_relationship;
    par37_spouse_case["input"]["ægtefælle"]["fakta"]["lønmodtager"]["skatteår"] =
        serde_json::json!(2026);
    par37_spouse_case["input"]["ægtefælle"]["fakta"]["lønmodtager"]["kommune"] =
        serde_json::json!({ "$variant": "København" });
    par37_spouse_case["input"]["ægtefælle"]["fakta"]["kapitalindkomst"]["renter"]
        ["renteudgifter_kroner"] = serde_json::json!(0);
    par37_spouse_case["input"]["ægtefælle"]["fakta"]["aktieavance"]["udbytter"] = serde_json::json!([{
        "identifikation": "par37-40-aegtefaelle-udbytte",
        "udlodder": { "$variant": "Ll16AAlmindeligtSelskab" },
        "modtager": { "$variant": "Ll16AAktuelAktionær" },
        "aktiv": { "$variant": "PersonskatAlmindeligAktie" },
        "beløb_kroner": 50_000,
        "par13a_kildefakta": {
            "$variant": "AblPar13AUdbytteUdenForModregningsgrundlag"
        }
    }]);
    let mut par37_conflicting_context_case = par37_case.clone();
    par37_conflicting_context_case["case_id"] =
        Value::String("personskat-par37-40-modstridende-kontekst-2026".into());
    par37_conflicting_context_case["input"]["aktieavance"]["særlige_aktiver"][0]
        ["identifikation"] = Value::String("personskat-fraflytning-modstridende-2026".into());
    par37_conflicting_context_case["input"]["aktieavance"]["særlige_aktiver"][0]["kilde"]
        ["fakta"]["identifikation"] =
        Value::String("personskat-fraflytning-modstridende-2026".into());
    par37_conflicting_context_case["input"]["aktieavance"]["særlige_aktiver"][0]["kilde"]
        ["fakta"]["fraflytning"]["aktier"][0]["identifikation"] =
        Value::String("personskat-fraflytteraktie-modstridende".into());
    par37_conflicting_context_case["input"]["aktieavance"]["særlige_aktiver"][0]["kilde"]
        ["fakta"]["fraflytning"]["aktier"][0]["selskabsidentifikation"] =
        Value::String("DK-PERSONSKAT-FRAFLYTNING-MODSTRIDENDE".into());
    par37_conflicting_context_case["input"]["aktieavance"]["særlige_aktiver"][0]["kilde"]
        ["fakta"]["fraflytning"]["kontekstgrundlag"] = serde_json::json!({
        "$variant": "HistoriskPar37Til40Aktieindkomstkontekst",
        "kontekst": {
            "indkomstår": 2026,
            "øvrig_egen_aktieindkomst_kroner": 999_999,
            "ægtefælles_aktieindkomst_kroner": 999_999,
            "samlevende_med_ægtefælle_ved_indkomstårets_udløb": true
        }
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(par37_case);
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(par37_mixed_case);
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(par37_spouse_case);
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(par37_conflicting_context_case);
    let par32_choices = || {
        serde_json::json!({
            "tabsprioritet": { "$variant": "KglPar32AlmindeligeTabFørst" },
            "fast_ejendomstabsprioritet": {
                "$variant": "KglPar32SælgertabFørst"
            },
            "aktiemodregningsvalg": {
                "omfang": { "$variant": "KglPar32IngenAktiemodregning" },
                "beløb": { "$variant": "KglPar32MaksimalAktiemodregning" }
            }
        })
    };
    let par32_contract = |identifikation: &str,
                          rækkefølge_i_indkomståret: i64,
                          anskaffelsessum_kroner: i64,
                          afståelsesværdi_kroner: i64,
                          udøver_næring: bool,
                          relationsfakta: Value,
                          underliggende: Value| {
        serde_json::json!({
            "identifikation": identifikation,
            "rækkefølge_i_indkomståret": rækkefølge_i_indkomståret,
            "kontrakt": {
                "aftaleart": {
                    "$variant": "KglPar30IngenUndtagelsesaftale"
                },
                "har_modgående_kontrakt_eller_forretning": false,
                "værdi_primo_kroner": 0,
                "værdi_ultimo_kroner": 0,
                "anskaffelsessum_kroner": anskaffelsessum_kroner,
                "afståelsesværdi_kroner": afståelsesværdi_kroner,
                "anskaffet_i_indkomståret": true,
                "realiseret_i_indkomståret": true,
                "udøver_næring_ved_køb_og_salg_af_finansielle_kontrakter":
                    udøver_næring
            },
            "relationsfakta": relationsfakta,
            "underliggende": underliggende
        })
    };
    let par32_kursgevinst = |skatteyder_identifikation: &str,
                             tidligere_år: Vec<Value>,
                             aktuelle_kontrakter: Vec<Value>| {
        serde_json::json!({
            "$variant": "MedKursgevinst",
            "fakta": {
                "skatteyder_identifikation": skatteyder_identifikation,
                "ægtefælles_skatteyder_identifikation": null,
                "sælgerpantebreve": [],
                "gældsposter": [],
                "øvrige_instrumenter": {
                    "fordringer": [],
                    "obligationsbaserede_minimumsbeviser": []
                },
                "par32_kontraktforløb": {
                    "$variant": "MedPar32Kontraktforløb",
                    "tidligere_år": tidligere_år,
                    "aktuelt_år": {
                        "indkomstår": 2026,
                        "kontrakter": aktuelle_kontrakter,
                        "valg": par32_choices()
                    }
                }
            }
        })
    };

    let historisk_par32_tab = par32_contract(
        "par32-tab-2025",
        1,
        20_000,
        10_000,
        false,
        serde_json::json!({ "$variant": "KglPar32KildeUdenSærligRelation" }),
        serde_json::json!({ "$variant": "KglPar32KildeIkkeAktiebaseret" }),
    );
    let par32_historikår = serde_json::json!({
        "fakta": {
            "indkomstår": 2025,
            "kontrakter": [historisk_par32_tab],
            "valg": par32_choices()
        },
        "aktieavance": {
            "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
            "særlige_aktiver": [],
            "udbytter": []
        },
        "gift_og_samlevende_ved_indkomstårets_udgang": false
    });
    let mut par32_history_case = json_input["cases"][0].clone();
    par32_history_case["case_id"] = Value::String("personskat-kgl-par32-historik-2026".into());
    par32_history_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": [],
        "udbytter": []
    });
    par32_history_case["input"]["kapitalindkomst"]["kursgevinst"] = par32_kursgevinst(
        "par32-historik-person",
        vec![par32_historikår],
        vec![
            par32_contract(
                "par32-gevinst-a-2026",
                1,
                10_000,
                14_000,
                false,
                serde_json::json!({ "$variant": "KglPar32KildeUdenSærligRelation" }),
                serde_json::json!({ "$variant": "KglPar32KildeIkkeAktiebaseret" }),
            ),
            par32_contract(
                "par32-gevinst-b-2026",
                2,
                5_000,
                7_000,
                false,
                serde_json::json!({ "$variant": "KglPar32KildeUdenSærligRelation" }),
                serde_json::json!({ "$variant": "KglPar32KildeIkkeAktiebaseret" }),
            ),
        ],
    );
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(par32_history_case);

    let mut par32_abl17_case = json_input["cases"][0].clone();
    par32_abl17_case["case_id"] = Value::String("personskat-kgl-par32-abl17-2026".into());
    par32_abl17_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": [{
            "identifikation": "par32-abl17-aktiv",
            "kilde": {
                "$variant": "PersonskatAktieaktivEfterPar17",
                "fakta": {
                    "indkomstår": 2026,
                    "skattepligtsgrundlag": {
                        "$variant": "AblPar7PersonEfterKildeskatteloven"
                    },
                    "næringsstatus": {
                        "$variant": "AblPar17UdøverNæringVedKøbOgSalgAfAktier"
                    },
                    "instrument": { "$variant": "AblPar17AlmindeligAktie" },
                    "erhvervelsesstatus": {
                        "$variant": "AblPar17ErhvervetSomLedINæringsvej"
                    },
                    "afståelsessum_kroner": 35_000,
                    "anskaffelsessum_kroner": 30_000
                }
            },
            "markedsstatus": {
                "$variant": "AblOptagetTilHandelPåReguleretMarked"
            }
        }],
        "udbytter": []
    });
    par32_abl17_case["input"]["kapitalindkomst"]["kursgevinst"] = par32_kursgevinst(
        "par32-abl17-person",
        vec![],
        vec![par32_contract(
            "par32-abl17-tab-2026",
            1,
            20_000,
            11_000,
            true,
            serde_json::json!({
                "$variant": "KglPar32KildeTilknyttetAblAktiv",
                "aktieaktiv_identifikation": "par32-abl17-aktiv"
            }),
            serde_json::json!({
                "$variant": "KglPar32KildeEnkeltaktieFraAblReference"
            }),
        )],
    );
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(par32_abl17_case);
    for case in json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
    {
        case["input"]["kapitalindkomst"]
            .as_object_mut()
            .expect("Personskat capital-income input")
            .entry("virksomhedskapital".to_string())
            .or_insert_with(|| {
                serde_json::json!({
                    "selvstændig_beskatningsordning": {
                        "$variant": "UdenVirksomhedsEllerKapitalafkastordning"
                    },
                    "medarbejderaktier": {
                        "$variant": "UdenKapitalafkastEfterVirksomhedsskattelov22C"
                    }
                })
            });
    }
    std::fs::write(
        &json_input_path,
        serde_json::to_vec_pretty(&json_input).expect("encode Personskat JSON input"),
    )
    .expect("write Personskat JSON input");
    let hydrated_json_input_path = temp_path("json");
    let mut hydrated_json_input = json_input.clone();
    let mixed_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-par37-40-blandet-slutskat-2026")
        .expect("mixed §§ 37-40 JSON case")
        .clone();
    let annual_claim_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-aarsnetto-fordring-2026")
        .expect("annual KGL claim JSON case")
        .clone();
    hydrated_json_input["cases"] = Value::Array(vec![mixed_case, annual_claim_case]);
    std::fs::write(
        &hydrated_json_input_path,
        serde_json::to_vec_pretty(&hydrated_json_input)
            .expect("encode mixed Personskat JSON input"),
    )
    .expect("write mixed Personskat JSON input");
    let json_output = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        json_input_path.to_str().expect("JSON input path"),
    ]);
    let hydrated_xlsx_path = temp_path("xlsx");
    let hydrate_xlsx = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--input",
        hydrated_json_input_path
            .to_str()
            .expect("mixed JSON input path"),
        "--format",
        "xlsx",
        "--output",
        hydrated_xlsx_path
            .to_str()
            .expect("hydrated XLSX input path"),
    ]);
    assert!(
        hydrate_xlsx.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&hydrate_xlsx.stderr),
        String::from_utf8_lossy(&hydrate_xlsx.stdout)
    );
    let hydrated_xlsx_output = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        hydrated_xlsx_path
            .to_str()
            .expect("hydrated XLSX input path"),
    ]);
    std::fs::remove_file(&json_input_path).ok();
    std::fs::remove_file(&hydrated_json_input_path).ok();
    std::fs::remove_file(&hydrated_xlsx_path).ok();
    assert!(
        json_output.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&json_output.stderr),
        String::from_utf8_lossy(&json_output.stdout)
    );
    assert!(
        hydrated_xlsx_output.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&hydrated_xlsx_output.stderr),
        String::from_utf8_lossy(&hydrated_xlsx_output.stdout)
    );
    let json_result = parse_stdout(&json_output);
    let hydrated_xlsx_result = parse_stdout(&hydrated_xlsx_output);
    let json_annual_claim_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-aarsnetto-fordring-2026")
        .expect("JSON annual KGL claim result");
    let hydrated_annual_claim_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-aarsnetto-fordring-2026")
        .expect("hydrated XLSX annual KGL claim result");
    assert_eq!(
        hydrated_annual_claim_result["result"],
        json_annual_claim_result["result"]
    );
    let annual_claim_trace =
        &hydrated_annual_claim_result["result"]["kapitalindkomst"]["kursgevinst_resultat"];
    assert_eq!(annual_claim_trace["input_gyldigt"], true);
    assert_eq!(
        annual_claim_trace["årets_samlede_netto_efter_par14_kroner"],
        3_000
    );
    assert_eq!(
        annual_claim_trace["øvrige_instrumentresultat"]["fordringsresultater"][0]["forløb"]
            ["rå_netto_kroner"],
        3_000
    );
    assert_eq!(
        annual_claim_trace["øvrige_instrumentresultat"]["fordringsresultater"][0]["forløb"]
            ["position_ultimo"]["$variant"],
        "KglÅrsnettoIngenPositionUltimo"
    );
    assert_eq!(
        hydrated_annual_claim_result["result"]["kapitalindkomst"]["kapitalindkomst_resultat"]
            ["nettokapitalindkomst_kroner"],
        3_000
    );
    let xlsx_par32_history_result = result["results"]
        .as_array()
        .expect("XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-par32-historik-2026")
        .expect("XLSX KGL §32 history result");
    let json_par32_history_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-par32-historik-2026")
        .expect("JSON KGL §32 history result");
    assert_eq!(
        xlsx_par32_history_result["result"],
        json_par32_history_result["result"]
    );
    let par32_history_trace = &xlsx_par32_history_result["result"]["kursgevinst_par32"];
    assert_eq!(
        par32_history_trace["$variant"],
        "BeregnetPar32Kontraktforløb"
    );
    assert_eq!(par32_history_trace["input_gyldigt"], true);
    assert_eq!(
        par32_history_trace["historiske_årsresultater"]
            .as_array()
            .expect("historical KGL §32 annual results")
            .len(),
        1
    );
    let par32_history_current =
        &par32_history_trace["aktuelt_årsresultat"]["kursgevinst"]["venstre"];
    assert_eq!(
        par32_history_current["gyldigt_fremførte_almindelige_tab_kroner"],
        10_000
    );
    assert_eq!(
        par32_history_current["tab_modregnet_i_egne_indkomstårsgevinster_kroner"],
        6_000
    );
    assert_eq!(
        par32_history_current["almindelige_tab_fremført_til_følgende_indkomstår_kroner"],
        4_000
    );
    assert_eq!(
        par32_history_current["netto_kontraktindkomst_efter_par32_kroner"],
        0
    );
    assert_eq!(
        xlsx_par32_history_result["result"]["kapitalindkomst"]["kapitalindkomst_resultat"]
            ["nettokapitalindkomst_kroner"],
        0
    );

    let xlsx_par32_abl17_result = result["results"]
        .as_array()
        .expect("XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-par32-abl17-2026")
        .expect("XLSX KGL §32 ABL §17 result");
    let json_par32_abl17_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-par32-abl17-2026")
        .expect("JSON KGL §32 ABL §17 result");
    assert_eq!(
        xlsx_par32_abl17_result["result"],
        json_par32_abl17_result["result"]
    );
    let par32_abl17_trace = &xlsx_par32_abl17_result["result"]["kursgevinst_par32"];
    assert_eq!(par32_abl17_trace["$variant"], "BeregnetPar32Kontraktforløb");
    assert_eq!(par32_abl17_trace["input_gyldigt"], true);
    let par32_abl17_current = &par32_abl17_trace["aktuelt_årsresultat"]["kursgevinst"]["venstre"];
    assert_eq!(
        par32_abl17_current["direkte_fradragsberettigede_kontrakttab_kroner"],
        9_000
    );
    assert_eq!(
        par32_abl17_current["netto_kontraktindkomst_efter_par32_kroner"],
        -9_000
    );
    let par32_abl17_contract =
        &par32_abl17_trace["aktuelt_årsresultat"]["venstre_afledning"]["kontraktresultater"][0];
    assert_eq!(
        par32_abl17_contract["fakta"]["identifikation"],
        "par32-abl17-tab-2026"
    );
    assert_eq!(
        par32_abl17_contract["relation"]["$variant"],
        "KglPar32AktiekontraktEfterAbl17"
    );
    assert_eq!(par32_abl17_contract["abl_reference_entydig"], true);
    assert_eq!(
        xlsx_par32_abl17_result["result"]["aktieavance"]["personlig_indkomst_kroner"],
        5_000
    );
    assert_eq!(
        xlsx_par32_abl17_result["result"]["kapitalindkomst"]["kapitalindkomst_resultat"]
            ["omklassificeret_personlig_indkomst_kroner"],
        -4_000
    );
    let xlsx_spouse_result = result["results"]
        .as_array()
        .expect("XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-aegtefaelleoverfoersler-2025")
        .expect("XLSX spouse-transfer result");
    let json_spouse_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-aegtefaelleoverfoersler-2025")
        .expect("JSON spouse-transfer result");
    assert_eq!(xlsx_spouse_result["result"], json_spouse_result["result"]);
    let xlsx_property_tax_result = result["results"]
        .as_array()
        .expect("XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-ejendomsskatter-2025")
        .expect("XLSX property-tax result");
    let json_property_tax_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-ejendomsskatter-2025")
        .expect("JSON property-tax result");
    assert_eq!(
        xlsx_property_tax_result["result"],
        json_property_tax_result["result"]
    );
    let xlsx_dividend_result = result["results"]
        .as_array()
        .expect("XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-udbytte-2026")
        .expect("XLSX dividend result");
    let json_dividend_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-udbytte-2026")
        .expect("JSON dividend result");
    assert_eq!(
        xlsx_dividend_result["result"],
        json_dividend_result["result"]
    );
    assert_eq!(
        xlsx_dividend_result["result"]["aktieavance"]["udbytter"][0]["ligningslov16a_resultat"]
            ["hjemmel"]["$variant"],
        "Ll16AStk2Nr1FørstePunktum"
    );
    assert_eq!(
        xlsx_dividend_result["result"]["aktieavance"]["aktieindkomst_kroner"],
        12_000
    );
    assert_eq!(
        xlsx_dividend_result["result"]["endelig_aktieindkomstskat_kroner"],
        3_240
    );
    let xlsx_establishment_account_result = result["results"]
        .as_array()
        .expect("XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-etableringskonto-2026")
        .expect("XLSX entrepreneur-account result");
    let json_establishment_account_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-etableringskonto-2026")
        .expect("JSON entrepreneur-account result");
    assert_eq!(
        xlsx_establishment_account_result["result"],
        json_establishment_account_result["result"]
    );
    let establishment_account_result =
        &xlsx_establishment_account_result["result"]["personlig_indkomst"];
    assert_eq!(establishment_account_result["alle_input_gyldige"], true);
    assert_eq!(
        establishment_account_result["fradrag_i_personlig_indkomst_kroner"],
        30_000
    );
    assert_eq!(
        establishment_account_result["ligningsmæssigt_fradrag_kroner"],
        0
    );
    assert_eq!(
        establishment_account_result["etableringskonto"]["$variant"],
        "BeregnetEtableringskontoindskud"
    );
    assert_eq!(
        establishment_account_result["etableringskonto"]["etableringskontolov_resultat"]
            ["iværksætterkonto_personlig_indkomst_fradrag_kroner"],
        30_000
    );
    assert_eq!(
        establishment_account_result["etableringskonto"]["par3_stk2_nr11_resultat"]
            ["fradrag_i_personlig_indkomst_kroner"],
        30_000
    );
    assert_eq!(
        xlsx_establishment_account_result["result"]["skat"]["personlig_indkomst_efter_am_kroner"],
        522_000
    );
    assert_eq!(
        xlsx_establishment_account_result["result"]["skat"]
            ["almindelig_skattepligtig_indkomst_kroner"],
        455_600
    );
    let xlsx_par35_result = result["results"]
        .as_array()
        .expect("XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-par35-medarbejdereje-2026")
        .expect("XLSX § 35 employee-ownership result");
    let json_par35_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-par35-medarbejdereje-2026")
        .expect("JSON § 35 employee-ownership result");
    assert_eq!(xlsx_par35_result["result"], json_par35_result["result"]);
    assert_eq!(
        xlsx_par35_result["result"]["aktieavance"]["aktieindkomst_kroner"],
        50_000
    );
    assert_eq!(
        xlsx_par35_result["result"]["endelig_aktieindkomstskat_kroner"],
        13_500
    );
    let par35_special_result = &xlsx_par35_result["result"]["aktieavance"]["særlige_resultater"][0];
    assert_eq!(par35_special_result["input_gyldigt"], true);
    assert_eq!(
        par35_special_result["årets_forfaldne_overdragerskat_kroner"],
        22_000
    );
    assert_eq!(
        par35_special_result["årets_betalte_overdragerskat_kroner"],
        0
    );
    let par35_trace = &par35_special_result["par35_forløbsresultat"];
    assert_eq!(par35_trace["input_gyldigt"], true);
    assert_eq!(
        par35_trace["umiddelbar_skattepligtig_gevinst_kroner"],
        50_000
    );
    assert_eq!(par35_trace["årets_nye_forfald_kroner"], 22_000);
    assert_eq!(
        par35_trace["tilstand_ultimo"]["overdragerskat_saldo_kroner"],
        22_000
    );
    assert_eq!(
        par35_trace["tilstand_ultimo"]["beholdning"]
            .as_array()
            .expect("§ 35 closing holdings")
            .len(),
        0
    );
    let par35_event = &par35_trace["hændelsesresultater"][0]["post"];
    assert_eq!(par35_event["rækkefølge_i_indkomståret"], 1);
    assert_eq!(
        par35_event["hændelse"]["$variant"],
        "AblPar35HændelseAfståelse"
    );
    assert_eq!(
        par35_event["hændelse"]["data"]["hændelsesidentifikation"],
        "personskat-par35-salg-2026"
    );
    let xlsx_par37_result = result["results"]
        .as_array()
        .expect("XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-par37-40-fraflytning-2026")
        .expect("XLSX §§ 37-40 departure result");
    let json_par37_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-par37-40-fraflytning-2026")
        .expect("JSON §§ 37-40 departure result");
    assert_eq!(xlsx_par37_result["result"], json_par37_result["result"]);
    assert_eq!(
        xlsx_par37_result["result"]["aktieavance"]["aktieindkomst_kroner"],
        100_000
    );
    assert_eq!(
        xlsx_par37_result["result"]["endelig_aktieindkomstskat_kroner"],
        21_438
    );
    assert_eq!(
        xlsx_par37_result["result"]["positiv_aktieindkomstskat"]["lavt_grundlag_kroner"],
        79_400
    );
    assert_eq!(
        xlsx_par37_result["result"]["positiv_aktieindkomstskat"]["højt_grundlag_kroner"],
        20_600
    );
    assert_eq!(
        xlsx_par37_result["result"]["positiv_aktieindkomstskat"]["slutskat_indgående_skat_kroner"],
        8_652
    );
    assert_eq!(
        xlsx_par37_result["result"]["positiv_aktieindkomstskat"]["samlet_skat_kroner"],
        30_090
    );
    let par37_special_result = &xlsx_par37_result["result"]["aktieavance"]["særlige_resultater"][0];
    assert_eq!(par37_special_result["input_gyldigt"], true);
    assert_eq!(
        par37_special_result["årets_nye_henstand_med_fraflytterskat_kroner"],
        30_090
    );
    assert_eq!(
        par37_special_result["årets_forfaldne_fraflytterskat_kroner"],
        0
    );
    assert_eq!(
        par37_special_result["årets_betalte_fraflytterskat_kroner"],
        0
    );
    assert_eq!(
        par37_special_result["årets_fraflytterskat_saldobortfald_kroner"],
        0
    );
    let par37_trace = &par37_special_result["par37_til40_forløbsresultat"];
    assert_eq!(par37_trace["input_gyldigt"], true);
    assert_eq!(par37_trace["årets_beregnede_fraflytterskat_kroner"], 30_090);
    assert_eq!(par37_trace["årets_nye_henstand_kroner"], 30_090);
    assert_eq!(
        par37_trace["tilstand_ultimo"]["henstandssaldo_kroner"],
        30_090
    );
    assert_eq!(
        par37_trace["tilstand_ultimo"]["beholdning"]
            .as_array()
            .expect("§§ 37-40 closing departure holdings")
            .len(),
        1
    );
    let json_par37_mixed_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-par37-40-blandet-slutskat-2026")
        .expect("JSON mixed §§ 37-40 final-tax result");
    let hydrated_par37_mixed_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-par37-40-blandet-slutskat-2026")
        .expect("hydrated XLSX mixed §§ 37-40 final-tax result");
    assert_eq!(
        hydrated_par37_mixed_result["result"],
        json_par37_mixed_result["result"]
    );
    let mixed_result = &json_par37_mixed_result["result"];
    assert_eq!(mixed_result["aktieavance"]["aktieindkomst_kroner"], 50_000);
    assert_eq!(
        mixed_result["aktieavance"]["personlig_indkomst_kroner"],
        30_000
    );
    assert_eq!(
        mixed_result["aktieavance"]["kapitalindkomst_kroner"],
        20_000
    );
    let mixed_special_result = &mixed_result["aktieavance"]["særlige_resultater"][0];
    assert_eq!(mixed_special_result["input_gyldigt"], true);
    let mixed_source_results = mixed_special_result["kilderesultater"]
        .as_array()
        .expect("mixed §§ 37-40 source results");
    assert_eq!(mixed_source_results.len(), 3);
    assert_eq!(
        mixed_source_results[0]["kildeidentifikation"],
        "blandet-ordinaer"
    );
    assert_eq!(
        mixed_source_results[0]["kildegrundlag"]["$variant"],
        "AblOrdinærtAktiekildegrundlag"
    );
    assert_eq!(
        mixed_source_results[0]["kildegrundlag"]["markedsstatus"]["$variant"],
        "AblIkkeOptagetTilHandel"
    );
    assert_eq!(
        mixed_source_results[1]["kildeidentifikation"],
        "blandet-naering"
    );
    assert_eq!(
        mixed_source_results[1]["kildegrundlag"]["markedsstatus"]["$variant"],
        "AblIkkeOptagetTilHandel"
    );
    assert_eq!(
        mixed_source_results[2]["kildeidentifikation"],
        "blandet-par19c"
    );
    assert_eq!(
        mixed_source_results[2]["kildegrundlag"]["markedsstatus"]["$variant"],
        "AblOptagetTilHandelPåReguleretMarked"
    );
    assert_eq!(
        mixed_special_result["resultater"]
            .as_array()
            .expect("mixed §§ 37-40 annual results")
            .len(),
        3
    );
    assert_eq!(
        mixed_special_result["resultat"]["medregnes_i_skattepligtig_indkomst"],
        false
    );
    let mixed_trace = &mixed_special_result["par37_til40_forløbsresultat"];
    let mixed_departure = &mixed_trace["fraflytningsresultat"];
    assert_eq!(
        mixed_departure["skattekontekst"]["$variant"],
        "AblPar37Til40AfledtSlutskat"
    );
    assert_eq!(
        mixed_departure["umiddelbare_aktieavancebeskatningslov_kilderesultater"]
            .as_array()
            .expect("immediate mixed §§ 37-40 source results")
            .len(),
        1
    );
    assert_eq!(
        mixed_departure["henstandsvalgte_aktieavancebeskatningslov_kilderesultater"]
            .as_array()
            .expect("deferred mixed §§ 37-40 source results")
            .len(),
        2
    );
    assert_eq!(
        mixed_departure["umiddelbare_aktieavancebeskatningslov_resultater"]
            .as_array()
            .expect("immediate mixed §§ 37-40 annual results")
            .len(),
        1
    );
    assert_eq!(
        mixed_departure["henstandsvalgte_aktieavancebeskatningslov_resultater"]
            .as_array()
            .expect("deferred mixed §§ 37-40 annual results")
            .len(),
        2
    );
    let mixed_tax_context = &mixed_departure["skattekontekst"]["kontekst"];
    let mixed_tax_without = mixed_tax_context["skat_uden_fraflytterindkomst_kroner"]
        .as_i64()
        .expect("tax without mixed departure income");
    let mixed_tax_immediate = mixed_tax_context
        ["skat_før_henstandsvalgt_fraflytterindkomst_kroner"]
        .as_i64()
        .expect("tax before deferred mixed departure income");
    let mixed_tax_full = mixed_tax_context["skat_med_hele_fraflytterindkomsten_kroner"]
        .as_i64()
        .expect("tax with all mixed departure income");
    assert_eq!(
        mixed_trace["årets_beregnede_fraflytterskat_kroner"],
        mixed_tax_full - mixed_tax_without
    );
    assert_eq!(mixed_trace["årets_beregnede_fraflytterskat_kroner"], 31_200);
    assert_eq!(
        mixed_trace["årets_nye_henstand_kroner"],
        mixed_tax_full - mixed_tax_immediate
    );
    assert_eq!(
        mixed_trace["årets_skat_til_betaling_ved_fraflytning_kroner"],
        mixed_tax_immediate - mixed_tax_without
    );
    let xlsx_par37_spouse_result = result["results"]
        .as_array()
        .expect("XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-par37-40-aegtefaelle-2026")
        .expect("XLSX §§ 37-40 spouse-context result");
    let json_par37_spouse_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-par37-40-aegtefaelle-2026")
        .expect("JSON §§ 37-40 spouse-context result");
    assert_eq!(
        xlsx_par37_spouse_result["result"],
        json_par37_spouse_result["result"]
    );
    let par37_spouse_trace = &xlsx_par37_spouse_result["result"]["aktieavance"]
        ["særlige_resultater"][0]["par37_til40_forløbsresultat"]["fraflytningsresultat"];
    assert_eq!(
        par37_spouse_trace["fakta"]["aktieindkomstkontekst"]["ægtefælles_aktieindkomst_kroner"],
        50_000
    );
    assert_eq!(
        par37_spouse_trace["fakta"]["aktieindkomstkontekst"]
            ["samlevende_med_ægtefælle_ved_indkomstårets_udløb"],
        true
    );
    assert_eq!(par37_spouse_trace["beregnet_fraflytterskat_kroner"], 27_000);
    let xlsx_par37_conflicting_context_result = result["results"]
        .as_array()
        .expect("XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-par37-40-modstridende-kontekst-2026")
        .expect("XLSX §§ 37-40 conflicting-context result");
    let json_par37_conflicting_context_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-par37-40-modstridende-kontekst-2026")
        .expect("JSON §§ 37-40 conflicting-context result");
    assert_eq!(
        xlsx_par37_conflicting_context_result["result"],
        json_par37_conflicting_context_result["result"]
    );
    assert_eq!(
        xlsx_par37_conflicting_context_result["result"]["aktieavance"]["alle_input_gyldige"],
        false
    );
    assert_eq!(
        xlsx_par37_conflicting_context_result["result"]["aktieavance"]["aktieindkomst_kroner"],
        0
    );
    assert_eq!(
        xlsx_par37_conflicting_context_result["result"]["aktieavance"]["særlige_resultater"][0]
            ["input_gyldigt"],
        false
    );
    let xlsx_debt_result = result["results"]
        .as_array()
        .expect("XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-gaeld-2026")
        .expect("XLSX KGL debt result");
    let json_debt_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-gaeld-2026")
        .expect("JSON KGL debt result");
    assert_eq!(xlsx_debt_result["result"], json_debt_result["result"]);
    assert_eq!(
        xlsx_debt_result["result"]["kapitalindkomst"]["kursgevinst_resultat"]
            ["årets_samlede_netto_efter_par14_kroner"],
        3_000
    );
    assert_eq!(
        xlsx_debt_result["result"]["kapitalindkomst"]["kapitalindkomst_resultat"]
            ["nettokapitalindkomst_kroner"],
        3_000
    );
    let xlsx_voluntary_arrangement_result = result["results"]
        .as_array()
        .expect("XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-frivillig-ordning-2026")
        .expect("XLSX KGL voluntary-arrangement result");
    let json_voluntary_arrangement_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-frivillig-ordning-2026")
        .expect("JSON KGL voluntary-arrangement result");
    assert_eq!(
        xlsx_voluntary_arrangement_result["result"],
        json_voluntary_arrangement_result["result"]
    );
    let voluntary_debt_result = &xlsx_voluntary_arrangement_result["result"]["kapitalindkomst"]
        ["kursgevinst_resultat"]["gældsresultater"][0]["gældsresultat"];
    assert_eq!(voluntary_debt_result["input_gyldigt"], true);
    assert_eq!(
        voluntary_debt_result["behandling"]["$variant"],
        "KglGældBehandlesEfterPar24"
    );
    assert_eq!(voluntary_debt_result["par24_anvendes"], true);
    assert_eq!(
        voluntary_debt_result["frivillig_ordning_resultat"]["vurdering"]["$variant"],
        "KglFrivilligOrdningSamlet"
    );
    assert_eq!(
        voluntary_debt_result["frivillig_ordning_resultat"]["deltagende_andel_basispoint"],
        8_208
    );
    assert_eq!(
        voluntary_debt_result["skattepligtig_gevinst_kroner"],
        50_000
    );
    assert_eq!(
        xlsx_voluntary_arrangement_result["result"]["kapitalindkomst"]["kapitalindkomst_resultat"]
            ["nettokapitalindkomst_kroner"],
        50_000
    );
    assert_eq!(
        xlsx_property_tax_result["result"]["ejendomsskatter"]["samlet_ejendomsværdiskat_øre"],
        315_350
    );
    assert_eq!(
        xlsx_property_tax_result["result"]["ejendomsskatter"]["samlet_grundskyld_øre"],
        285_855
    );
    assert_eq!(
        xlsx_property_tax_result["result"]["ejendomsskatter"]["samlet_ejendomsskat_øre"],
        601_205
    );
    assert_eq!(
        xlsx_property_tax_result["result"]["samlet_skat_inkl_ejendomsskatter_kroner"],
        xlsx_property_tax_result["result"]["samlet_skat_inkl_endelig_aktieindkomstskat_kroner"]
            .as_i64()
            .expect("income and stock tax subtotal")
            + 6_012
    );
    assert_eq!(
        xlsx_spouse_result["result"]["indgående_ægtefælle"]["par13_indkomstfradrag_kroner"],
        39_617
    );
    assert_eq!(
        xlsx_spouse_result["result"]["indgående_ægtefælle"]["personfradrag"]
            ["overført_skatteværdi_kroner"],
        18_875
    );
    assert_eq!(
        xlsx_spouse_result["result"]["indgående_ægtefælle"]["par11_nedslag"]
            ["overført_nedslag_kroner"],
        3_169
    );
    assert_eq!(
        xlsx_spouse_result["result"]["samlet_skat_inkl_endelig_aktieindkomstskat_kroner"],
        184_895
    );
    let spouse_transfer_sender_year =
        &xlsx_spouse_result["result"]["aktieavance"]["ordinært_aktieår"]["resultat"]["årsresultat"];
    assert_eq!(
        spouse_transfer_sender_year["årets_tab_efter_par13a_kroner"],
        50_000
    );
    assert_eq!(
        spouse_transfer_sender_year["tab_overført_til_ægtefælle_kroner"],
        12_000
    );
    assert_eq!(
        spouse_transfer_sender_year["tab_fremført_til_følgende_indkomstår_kroner"],
        38_000
    );
    let spouse_transfer_recipient =
        &xlsx_spouse_result["result"]["ægtefælle"]["grundlag"]["aktieavance"];
    assert_eq!(spouse_transfer_recipient["aktieindkomst_kroner"], 0);
    assert_eq!(
        spouse_transfer_recipient["udbytter"][0]["par13a_kilderesultat"]
            ["udbytte_for_aktie_med_gevinst_efter_par12"],
        true
    );
    let spouse_transfer_recipient_year =
        &spouse_transfer_recipient["ordinært_aktieår"]["resultat"]["årsresultat"];
    assert_eq!(
        spouse_transfer_recipient_year["tab_modtaget_fra_ægtefælle_kroner"],
        12_000
    );
    assert_eq!(
        spouse_transfer_recipient_year["netto_aktieindkomst_fra_ordinære_aktier_kroner"],
        -12_000
    );
    assert_eq!(
        result["results"][1]["result"],
        json_result["results"][0]["result"]
    );
    assert_eq!(
        result["results"][2]["result"],
        json_result["results"][1]["result"]
    );
    assert_eq!(
        result["results"][3]["result"],
        json_result["results"][2]["result"]
    );
    assert_eq!(
        result["results"][4]["result"],
        json_result["results"][3]["result"]
    );
    assert_eq!(
        result["results"][5]["result"],
        json_result["results"][4]["result"]
    );
    assert_eq!(
        result["results"][6]["result"],
        json_result["results"][5]["result"]
    );
    assert_eq!(
        result["results"][7]["result"],
        json_result["results"][6]["result"]
    );
    assert_eq!(
        result["results"][8]["result"],
        json_result["results"][7]["result"]
    );
    assert_eq!(
        result["results"][9]["result"],
        json_result["results"][8]["result"]
    );

    assert_eq!(
        result["results"][0]["result"]["skat"]["samlet_inkl_am_efter_personfradrag_kroner"],
        208_726
    );
    assert_eq!(
        result["results"][0]["result"]["årsopgørelse"]["$variant"],
        "BeregnetÅrsopgørelse"
    );
    assert_eq!(
        result["results"][0]["result"]["årsopgørelse"]["resultat"]["slutskat_med_tillæg_kroner"],
        210_226
    );
    assert_eq!(
        result["results"][0]["result"]["årsopgørelse"]["resultat"]["restskat_kroner"],
        210_226
    );
    assert_eq!(
        result["results"][1]["result"]["aktieavance"]["personlig_indkomst_kroner"],
        7_000
    );
    assert_eq!(
        result["results"][1]["result"]["aktieavance"]["særlige_resultater"][0]["par17_resultat"]
            ["omfattet_af_stk1"],
        true
    );
    assert_eq!(
        result["results"][1]["result"]["aktieavance"]["særlige_resultater"][0]["par17_resultat"]
            ["input"]["skatteyder"]["skattepligtsresultat"]["grundlag"]["$variant"],
        "AblPar7PersonEfterKildeskatteloven"
    );
    assert_eq!(
        result["results"][1]["result"]["aktieavance"]["særlige_resultater"][0]
            ["kgl_par32_kontraktrelation"]["$variant"],
        "KglPar32AktiekontraktEfterAbl17"
    );
    assert_eq!(
        result["results"][1]["result"]["skat"]["øvrig_personlig_indkomst_kroner"],
        7_000
    );
    assert_eq!(
        result["results"][1]["result"]["skat"]["arbejdsmarkedsbidrag_kroner"],
        48_000
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ligningslov6_resultat"]
            ["fradrag_kroner"],
        2_000
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ligningslov6a_resultat"]
            ["fradrag_kroner"],
        1_000
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["kapitalindkomst_resultat"]
            ["nettokapitalindkomst_kroner"],
        75_000
    );
    assert_eq!(
        result["results"][7]["result"]["kapitalindkomst"]["kapitalindkomst_resultat"]
            ["nettokapitalindkomst_kroner"],
        25_000
    );
    assert_eq!(
        result["results"][7]["result"]["kapitalindkomst"]["ejendomsdrift_resultat"]["$variant"],
        "BeregnetEjendomsdriftEfterPar4Nr6"
    );
    assert_eq!(
        result["results"][7]["result"]["kapitalindkomst"]["ejendomsdrift_resultat"]
            ["ejendomsskattelov_input"]["indkomstår"],
        2026
    );
    assert_eq!(
        result["results"][7]["result"]["kapitalindkomst"]["ejendomsdrift_resultat"]
            ["par4_resultat"]["kapitalindkomst_kroner"],
        25_000
    );
    let business_travel_result = &result["results"][8]["result"]["erhvervsbefordring"];
    assert_eq!(business_travel_result["identifikationer_entydige"], true);
    assert_eq!(business_travel_result["rækkefølge_gyldig"], true);
    assert_eq!(business_travel_result["alle_input_gyldige"], true);
    assert_eq!(
        business_travel_result["samlet_skattefri_godtgørelse_kroner"],
        83_880
    );
    assert_eq!(
        business_travel_result["samlet_skattepligtig_godtgørelse_kroner"],
        400
    );
    assert_eq!(
        business_travel_result["samlet_fradrag_i_personlig_indkomst_kroner"],
        0
    );
    assert_eq!(
        business_travel_result["sagsresultater"]
            .as_array()
            .expect("§ 9 B per-case traces")
            .len(),
        3
    );
    for (index, expected_employer, expected_previous, expected_free, expected_taxable) in [
        (0, "arbejdsgiver-a", 0, 76_830, 0),
        (1, "arbejdsgiver-b", 0, 3_940, 0),
        (2, "arbejdsgiver-a", 19_500, 3_110, 400),
    ] {
        let case_result = &business_travel_result["sagsresultater"][index];
        assert_eq!(
            case_result["$variant"],
            "BeregnetErhvervsbefordringEfterLigningslov9B"
        );
        assert_eq!(
            case_result["fakta"]["godtgørende_arbejdsgiver_identifikation"],
            expected_employer
        );
        assert_eq!(
            case_result["ligningslov9b_input"]["befordring"]["kilometerhistorik"]
                ["tidligere_erhvervsmæssige_kilometer_hos_godtgørende_arbejdsgiver_i_indkomståret"],
            expected_previous
        );
        assert_eq!(
            case_result["ligningslov9b_resultat"]["skattefri_godtgørelse_kroner"],
            expected_free
        );
        assert_eq!(
            case_result["ligningslov9b_resultat"]["godtgørelse_personlig_indkomst_kroner"],
            expected_taxable
        );
    }
    assert_eq!(
        business_travel_result["historik_ultimo"]["bil_motorcykelkilometer_pr_arbejdsgiver"]
            ["arbejdsgiver-a"],
        20_500
    );
    assert_eq!(
        business_travel_result["historik_ultimo"]["bil_motorcykelkilometer_pr_arbejdsgiver"]
            ["arbejdsgiver-b"],
        1_000
    );
    assert_eq!(
        result["results"][8]["result"]["skat"]["bruttoløn_kroner"],
        600_400
    );
    assert_eq!(
        result["results"][8]["result"]["skat"]["arbejdsmarkedsbidrag_kroner"],
        48_032
    );
    let pbl53a_result = &result["results"][9]["result"]["kapitalindkomst"]["pbl53a_resultat"];
    assert_eq!(pbl53a_result["identifikationer_entydige"], true);
    assert_eq!(pbl53a_result["alle_input_gyldige"], true);
    assert_eq!(pbl53a_result["personlig_indkomst_kroner"], 60_000);
    assert_eq!(
        pbl53a_result["arbejdsmarkedsbidragspligtig_personlig_indkomst_kroner"],
        60_000
    );
    assert_eq!(
        pbl53a_result["kapitalposter"]
            .as_array()
            .expect("positive PBL § 53 A capital posts")
            .len(),
        2
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["fakta"]["omfangsfakta"]["erhvervelser"]
            .as_array()
            .expect("PBL § 53 A acquisition history")
            .len(),
        1
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["fakta"]["omfangsfakta"]
            ["repræsenteret_kontraktdel"]["$variant"],
        "Pbl53AHeleKontrakten"
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["fakta"]["omfangsfakta"]["kontraktændringer"]
            .as_array()
            .expect("PBL § 53 A contract-change history")
            .len(),
        1
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["fakta"]["omfangsfakta"]["kontraktændringer"][0]
            ["identifikation"],
        "valuta-2014"
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["fakta"]["omfangsfakta"]["kontraktændringer"][0]
            ["art"]["$variant"],
        "Pbl53AValutaÆndret"
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["fakta"]["omfangsfakta"]["erhvervelser"][0]
            ["tidspunkt"]["dato"]["år"],
        2024
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["fakta"]["omfangsfakta"]["erhvervelser"][0]
            ["overdrager_identifikation"],
        "tidligere-ejer"
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["fakta"]["omfangsfakta"]["erhvervelser"][0]
            ["erhverver_identifikation"],
        "person-1"
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["fakta"]["omfangsfakta"]["erhvervelser"][0]
            ["kapitalværdi_på_erhvervelsestidspunktet_kroner"],
        200_000
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["fakta"]["omfangsfakta"]["erhvervelser"][0]["måde"]
            ["$variant"],
        "Pbl53AErhvervetVedArv"
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["fakta"]["omfangsfakta"]["overgangsvalgfristfakta"]
            ["$variant"],
        "Pbl53ASenereArvUnderFuldSkattepligt"
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["fakta"]["omfangsfakta"]["overgangsvalg"]
            .as_array()
            .expect("PBL § 53 A election notices")
            .len(),
        1
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["fakta"]["omfangsfakta"]["overgangsvalg"][0]["mål"]
            ["$variant"],
        "Pbl53AValgAfPar53A"
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["fakta"]["omfangsfakta"]
            ["historiske_blanket49020_indsendelser"]
            .as_array()
            .expect("historical PBL § 53 A form submissions")
            .len(),
        1
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["omfangsresultat"]["overgangsresultat"]
            ["valgresultat"]["historiske_blanketresultater"][0]["tidligere_udgave_uden_valgfelt"],
        false
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["omfangsresultat"]["overgangsresultat"]
            ["valgresultat"]["historiske_blanketresultater"][0]["indsendelse_opfylder_betingelser"],
        false
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["omfangsresultat"]["overgangsresultat"]
            ["valgresultat"]["valg_gyldigt"],
        true
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["omfangsresultat"]["overgangsresultat"]
            ["kontraktændringsresultat"]["fakta_gyldige"],
        true
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["omfangsresultat"]["overgangsresultat"]
            ["kontraktændringsresultat"]["hele_nye_ordninger_antal"],
        0
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["omfangsresultat"]["overgangsresultat"]
            ["moderne_virkningsstart"]["dato"],
        serde_json::json!({ "år": 2024, "måned": 4, "dag": 3 })
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["pensionsbeskatningslov_input"]["indkomstår"],
        2026
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["forløbsresultat"]["input_gyldigt"],
        true
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["forløbsresultat"]["personlig_indkomst_kroner"],
        60_000
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][1]["pensionsbeskatningslov_input"]["metodehistorik"]
            ["$variant"],
        "Pbl53ATidligereAlternativKapitalværdiOpgørelse"
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][1]["pensionsbeskatningslov_resultat"]
            ["metodevalg_bindende_opfyldt"],
        true
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][1]["pensionsbeskatningslov_resultat"]
            ["afkastmetode_tilladt_efter_udbyderopgørelse"],
        true
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][0]["par4_resultat"]["kapitalindkomst_kroner"],
        22_000
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][1]["pensionsbeskatningslov_resultat"]
            ["fremført_negativt_afkast_ultimo_kroner"],
        13_000
    );
    assert_eq!(
        pbl53a_result["ordningsresultater"][2]["par4_resultat"]["kapitalindkomst_kroner"],
        13_500
    );
    assert_eq!(
        result["results"][9]["result"]["kapitalindkomst"]["kapitalindkomst_resultat"]
            ["nettokapitalindkomst_kroner"],
        35_500
    );
    assert_eq!(
        result["results"][9]["result"]["skat"]["bruttoløn_kroner"],
        660_000
    );
    assert_eq!(
        result["results"][9]["result"]["skat"]["arbejdsmarkedsbidrag_kroner"],
        52_800
    );
    assert_eq!(
        result["results"][10]["case_id"],
        "personskat-pbl53a-overdrager-2026"
    );
    let pbl53a_transfer_result =
        &result["results"][10]["result"]["kapitalindkomst"]["pbl53a_resultat"];
    assert_eq!(pbl53a_transfer_result["alle_input_gyldige"], true);
    assert_eq!(
        pbl53a_transfer_result["kapitalposter"]
            .as_array()
            .expect("former-owner PBL § 53 A capital posts")
            .len(),
        1
    );
    assert_eq!(
        pbl53a_transfer_result["kapitalposter"][0]["beløb_kroner"],
        20_000
    );
    let pbl53a_transfer_order = &pbl53a_transfer_result["ordningsresultater"][0];
    assert_eq!(pbl53a_transfer_order["$variant"], "BeregnetPbl53AOrdning");
    assert_eq!(
        pbl53a_transfer_order["pensionsbeskatningslov_input"]["kapitalværdi_primo_kroner"],
        100_000
    );
    assert_eq!(
        pbl53a_transfer_order["pensionsbeskatningslov_input"]["kapitalværdi_ultimo_kroner"],
        130_000
    );
    assert_eq!(
        pbl53a_transfer_order["pensionsbeskatningslov_input"]["indbetalinger_i_året_kroner"],
        10_000
    );
    assert_eq!(
        pbl53a_transfer_order["afkastforløbsresultat"]["årsresultater"][0]["periode_resultat"]
            ["rettighedsperiode_findes_entydigt"],
        true
    );
    assert_eq!(
        pbl53a_transfer_order["afkastforløbsresultat"]["årsresultater"][0]["periode_resultat"]
            ["afkastperiode"]["sluttidspunkt_eksklusiv"],
        serde_json::json!({
            "dato": { "år": 2026, "måned": 6, "dag": 1 },
            "rækkefølge_på_dagen": 2
        })
    );
    let accepted_historical_form_result =
        &json_result["results"][9]["result"]["kapitalindkomst"]["pbl53a_resultat"];
    assert_eq!(accepted_historical_form_result["alle_input_gyldige"], true);
    assert_eq!(
        accepted_historical_form_result["ordningsresultater"][0]["$variant"],
        "BeregnetPbl53AOrdning"
    );
    assert_eq!(
        accepted_historical_form_result["ordningsresultater"][0]["omfangsresultat"]
            ["overgangsresultat"]["valgresultat"]["meddelelsesresultater"]
            .as_array()
            .expect("modern election results for historical-form case")
            .len(),
        0
    );
    assert_eq!(
        accepted_historical_form_result["ordningsresultater"][0]["omfangsresultat"]
            ["overgangsresultat"]["valgresultat"]["historiske_blanketresultater"][0]
            ["afledt_regime"]["$variant"],
        "Pbl53AOvergangsvalgTilPar53A"
    );
    assert_eq!(
        accepted_historical_form_result["ordningsresultater"][0]["omfangsresultat"]
            ["overgangsresultat"]["valgresultat"]["historiske_blanketresultater"][0]
            ["indsendelse_opfylder_betingelser"],
        true
    );
    assert_eq!(
        accepted_historical_form_result["ordningsresultater"][0]["omfangsresultat"]
            ["overgangsresultat"]["moderne_virkningsstart"]["dato"],
        serde_json::json!({ "år": 2021, "måned": 4, "dag": 30 })
    );
    let rejected_historical_form_result =
        &json_result["results"][10]["result"]["kapitalindkomst"]["pbl53a_resultat"];
    assert_eq!(rejected_historical_form_result["alle_input_gyldige"], false);
    assert_eq!(
        rejected_historical_form_result["ordningsresultater"][0]["$variant"],
        "UgyldigtPbl53AGrundlag"
    );
    assert_eq!(
        rejected_historical_form_result["ordningsresultater"][0]["omfangsresultat"]
            ["overgangsresultat"]["valgresultat"]["historiske_blanketresultater"][0]
            ["tidligere_udgave_uden_valgfelt"],
        false
    );
    assert_eq!(
        rejected_historical_form_result["ordningsresultater"][0]["omfangsresultat"]
            ["overgangsresultat"]["valgresultat"]["valg_gyldigt"],
        false
    );
    let material_contract_change_result =
        &json_result["results"][11]["result"]["kapitalindkomst"]["pbl53a_resultat"];
    assert_eq!(material_contract_change_result["alle_input_gyldige"], true);
    assert_eq!(
        material_contract_change_result["ordningsresultater"][0]["$variant"],
        "BeregnetPbl53AOrdning"
    );
    assert_eq!(
        material_contract_change_result["ordningsresultater"][0]["omfangsresultat"]
            ["overgangsresultat"]["kontraktændringsresultat"]["hele_nye_ordninger_antal"],
        1
    );
    assert_eq!(
        material_contract_change_result["ordningsresultater"][0]["omfangsresultat"]
            ["overgangsresultat"]["moderne_virkningsstart"]["dato"],
        serde_json::json!({ "år": 2013, "måned": 6, "dag": 1 })
    );
    assert_eq!(
        material_contract_change_result["ordningsresultater"][0]["omfangsresultat"]
            ["overgangsresultat"]["moderne_afkastvirkningsgrænse"],
        serde_json::json!({
            "$variant": "Pbl53AKendtModerneAfkastvirkningsgrænse",
            "kilde": {
                "$variant": "Pbl53AVirkningFraKontraktændring",
                "ændringsidentifikation": "markedsrente-2013"
            },
            "tidspunkt": {
                "dato": { "år": 2013, "måned": 6, "dag": 1 },
                "rækkefølge_på_dagen": 1
            },
            "kapitalværdi_kroner": 140_000
        })
    );
    let unsupported_contract_change_result =
        &json_result["results"][12]["result"]["kapitalindkomst"]["pbl53a_resultat"];
    assert_eq!(
        unsupported_contract_change_result["alle_input_gyldige"],
        false
    );
    assert_eq!(
        unsupported_contract_change_result["ordningsresultater"][0]["$variant"],
        "UgyldigtPbl53AGrundlag"
    );
    assert_eq!(
        unsupported_contract_change_result["ordningsresultater"][0]["omfangsresultat"]
            ["overgangsresultat"]["kontraktændringsresultat"]["alle_ændringsarter_understøttede"],
        false
    );
    assert_eq!(
        unsupported_contract_change_result["ordningsresultater"][0]["omfangsresultat"]
            ["overgangsresultat"]["kontraktændringsresultat"]["alle_kapitalværdier_entydige"],
        false
    );
    assert_eq!(
        result["results"][2]["result"]["aktieavance"]["aktieindkomst_kroner"],
        -18_000
    );
    assert_eq!(
        result["results"][2]["result"]["aktieavance"]["ordinært_aktieår"]["resultat"]
            ["hændelsesforløbsresultater"][0]["hændelsesresultater"][0]["skattefri_efter_par15"],
        true
    );
    assert_eq!(
        result["results"][2]["result"]["aktieavance"]["ordinært_aktieår"]["resultat"]
            ["hændelsesforløbsresultater"][0]["hændelsesresultater"][0]["par15_resultat"]
            ["$variant"],
        "AblPar15Vurderet"
    );
    assert_eq!(
        result["results"][2]["result"]["aktieavance"]["ordinært_aktieår"]["resultat"]
            ["hændelsesforløbsresultater"][0]["hændelsesresultater"][0]["par15_resultat"]
            ["klassifikation"]["input_gyldigt"],
        true
    );
    assert_eq!(
        result["results"][2]["result"]["aktieavance"]["ordinært_aktieår"]["resultat"]
            ["hændelsesforløbsresultater"][0]["hændelsesresultater"][0]["par15_resultat"]
            ["klassifikation"]["udsteder_selvstændigt_skattesubjekt"],
        true
    );
    assert_eq!(
        result["results"][2]["result"]["aktieavance"]["ordinært_aktieår"]["resultat"]
            ["hændelsesforløbsresultater"][0]["hændelsesresultater"][0]["par15_resultat"]
            ["klassifikation"]["værdipapir_omfattet_af_aktieavancebeskatningsloven"],
        true
    );
    assert_eq!(
        result["results"][2]["result"]["aktieavance"]["ordinært_aktieår"]["resultat"]
            ["hændelsesforløbsresultater"][0]["hændelsesresultater"][0]["par15_resultat"]
            ["ebl_par8_stk4_resultat"]["betingelser_for_skattefrihed_opfyldt"],
        true
    );
    let par5a_event = &result["results"][2]["result"]["aktieavance"]["ordinært_aktieår"]
        ["resultat"]["hændelsesforløbsresultater"][1]["hændelsesresultater"][0];
    assert_eq!(par5a_event["hændelse_gyldig"], true);
    assert_eq!(par5a_event["bruttotab_kroner"], 30_000);
    assert_eq!(par5a_event["tabsreduktion_efter_par5a_kroner"], 12_000);
    assert_eq!(par5a_event["tab_efter_par5a_kroner"], 18_000);
    assert_eq!(
        par5a_event["fradragsberettiget_tab_efter_par13_kroner"],
        18_000
    );
    assert_eq!(
        par5a_event["par5a_resultat"]["input"]["fakta"]["ejertidsudbytter"][0]["beløb_kroner"],
        12_000
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]["$variant"],
        "BeregnetEjendomsavance"
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["par8_par9_behandling"]["$variant"],
        "EblPar8Behandling"
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["par8_par9_behandling"]["resultat"]["stk5_anvendt"],
        true
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["par8_par9_behandling"]["resultat"]
            ["skattefri_fortjeneste_kroner"],
        190_000
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["par8_par9_behandling"]["resultat"]
            ["genanbragt_fortjeneste_til_beskatning_kroner"],
        190_000
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["skattepligtig_fortjeneste_kroner"],
        190_000
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["eget_tabsårsresultat"]["gyldigt_fremført_tab_fra_tidligere_år_kroner"],
        25_000
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["ægtefælles_tabsårsresultat"]["tab_overført_til_ægtefælle_kroner"],
        40_000
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["par4_resultat"]["kapitalindkomst_kroner"],
        65_000
    );
    assert_eq!(
        result["results"][2]["result"]["skat"]["arbejdsmarkedsbidrag_kroner"],
        48_000
    );
    assert_eq!(
        result["results"][2]["result"]["ligningsfradrag"]["samlet_ligningsfradrag_kroner"],
        23_166
    );
    assert_eq!(
        result["results"][2]["result"]["ligningsfradrag"]["befordring"]["$variant"],
        "BeregnetBefordringsfradrag"
    );
    assert_eq!(
        result["results"][2]["result"]["ligningsfradrag"]["befordring"]["ligningslov9c_input"]
            ["aftrapningsindkomst_kroner"],
        600_000
    );
    assert_eq!(
        result["results"][2]["result"]["skat"]["øvrige_ligningsmæssige_fradrag_kroner"],
        23_166
    );
    assert_eq!(
        result["results"][3]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["par5_resultat"]["fast_tillæg_efter_stk1_kroner"],
        41_250
    );
    assert_eq!(
        result["results"][3]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["par5_resultat"]
            ["mælkekvoteforhøjelse_efter_stk3_kroner"],
        20_000
    );
    assert_eq!(
        result["results"][3]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["par5_resultat"]
            ["nedsættelser_efter_stk4_til_8_kroner"],
        30_000
    );
    assert_eq!(
        result["results"][3]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["par5_resultat"]
            ["stk6_overført_til_afstået_jord_kroner"],
        37_500
    );
    assert_eq!(
        result["results"][3]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["reguleret_anskaffelsessum_kroner"],
        268_750
    );
    assert_eq!(
        result["results"][3]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["skattepligtig_fortjeneste_kroner"],
        231_250
    );
    assert_eq!(
        result["results"][4]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["skattepligtig_fortjeneste_før_par6d_kroner"],
        4_990_000
    );
    assert_eq!(
        result["results"][4]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["par6d_resultat"]["valgt_udskudt_fortjeneste_kroner"],
        3_000_000
    );
    assert_eq!(
        result["results"][4]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["eget_tabsårsresultat"]["egne_skattepligtige_fortjenester_kroner"],
        300_000
    );
    assert_eq!(
        result["results"][4]["result"]["kapitalindkomst"]["kapitalindkomst_resultat"]
            ["nettokapitalindkomst_kroner"],
        375_000
    );
    assert_eq!(
        result["results"][4]["result"]["kapitalindkomst"]["kursgevinst_input_gyldigt"],
        true
    );
    assert_eq!(
        result["results"][4]["result"]["kapitalindkomst"]["kursgevinst_resultat"]
            ["årets_samlede_netto_efter_par14_kroner"],
        75_000
    );
    assert_eq!(
        result["results"][4]["result"]["kapitalindkomst"]["kursgevinst_resultat"]
            ["sælgerpantebrevsresultater"][0]["kursgevinstresultat"]["dispositionsresultater"][0]
            ["frigivet_anskaffelsessum_kroner"],
        300_000
    );
    assert_eq!(
        result["results"][4]["result"]["kapitalindkomst"]["kursgevinst_resultat"]
            ["kursgevinstlov_resultater"][0]["netto_efter_kursgevinstloven_kroner"],
        75_000
    );
    assert_eq!(
        result["results"][5]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["par11_stk2_resultat"]["stk2_anvendt"],
        true
    );
    assert_eq!(
        result["results"][5]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["par11_stk2_resultat"]
            ["anskaffelsessumsnedslag_bortfalder_kroner"],
        200_000
    );
    assert_eq!(
        result["results"][5]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["skattepligtig_fortjeneste_kroner"],
        200_000
    );
    assert_eq!(
        result["results"][5]["result"]["kapitalindkomst"]["kapitalindkomst_resultat"]
            ["nettokapitalindkomst_kroner"],
        200_000
    );
    assert_eq!(
        result["results"][6]["result"]["kapitalindkomst"]["fremleje_resultat"]["$variant"],
        "BeregnetFremlejeEfterLigningslov15Q"
    );
    assert_eq!(
        result["results"][6]["result"]["kapitalindkomst"]["fremleje_resultat"]
            ["ligningslov15q_resultat"]["reguleret_bundfradrag_kroner"],
        35_100
    );
    assert_eq!(
        result["results"][6]["result"]["kapitalindkomst"]["kapitalindkomst_resultat"]
            ["nettokapitalindkomst_kroner"],
        14_940
    );
}

#[test]
fn investment_classification_xlsx_expands_payloads_and_round_trips_template() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/danish-income-tax/investeringsklassifikation.calculate.runa");
    let input_path = temp_path("xlsx");
    let template = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--entry",
        "klassificer_investeringsselskab",
        "--format",
        "xlsx",
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(
        template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&template.stderr)
    );

    {
        let mut workbook = open_workbook_auto(&input_path).expect("input workbook");
        let case_paths = workbook_column_paths(&mut workbook, "cases");
        for expected in [
            "meddelelse.$variant",
            "meddelelse.AblPar19BOrdinærMeddelelse.virkningsår",
            "meddelelse.AblPar19BNyoprettetMeddelelse.oprettelsesdato.år",
            "oplysninger.$variant",
        ] {
            assert!(
                case_paths.iter().any(|path| path == expected),
                "missing canonical investment input path {expected}"
            );
        }
        let owner_paths = workbook_column_paths(&mut workbook, "aktivmasse_ejerposter");
        for expected in [
            "$variant",
            "AblEjerpostIPar19B.ejerandel.ejede_kapitalenheder",
            "AblEjerpostIPar21.klassifikationsinput.oplysninger.$variant",
        ] {
            assert!(
                owner_paths.iter().any(|path| path == expected),
                "missing canonical owner input path {expected}"
            );
        }

        let metadata = workbook
            .worksheet_range("_columns")
            .expect("column metadata");
        let notification_row = metadata
            .rows()
            .skip(1)
            .find(|row| {
                row.get(2).map(ToString::to_string).as_deref()
                    == Some("meddelelse.AblPar19BOrdinærMeddelelse.virkningsår")
            })
            .expect("notification payload metadata");
        assert_eq!(
            notification_row.get(5).map(ToString::to_string).as_deref(),
            Some("integer")
        );
        assert!(notification_row
            .get(8)
            .map(ToString::to_string)
            .expect("variant guard")
            .contains("AblPar19BOrdinærMeddelelse"));
    }

    let output = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--entry",
        "klassificer_investeringsselskab",
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&input_path).ok();
    assert!(
        output.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let result = parse_stdout(&output);
    assert!(result["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .is_empty());
    assert_eq!(
        result["results"][0]["result"]["effektiv_status"]["$variant"],
        "AblObligationsbaseretInvesteringsselskabEfterPar19C"
    );
}

#[test]
fn par19_xlsx_derives_nested_fact_tables_and_round_trips_template() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/danish-income-tax/investeringsklassifikation.calculate.runa");
    let input_path = temp_path("xlsx");
    let template = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--entry",
        "klassificer_investeringsselskab_efter_par19",
        "--format",
        "xlsx",
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(
        template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&template.stderr)
    );

    {
        let mut workbook = open_workbook_auto(&input_path).expect("input workbook");
        let tables = workbook.worksheet_range("_tables").expect("table metadata");
        let table_paths = tables
            .rows()
            .skip(1)
            .filter_map(|row| row.first().map(ToString::to_string))
            .collect::<Vec<_>>();
        assert_eq!(table_paths.len(), 4);
        for expected_suffix in [
            ".deltagere_ved_indkomstårets_udgang",
            ".aktivmasse.direkte_aktiver",
            ".aktivmasse.ejerposter",
            ".underliggende_aktiver_efter_direkte_og_indirekte_gennemlysning",
        ] {
            assert!(
                table_paths
                    .iter()
                    .any(|path| path.ends_with(expected_suffix)),
                "missing derived § 19 fact table ending in {expected_suffix}: {table_paths:?}"
            );
        }
    }

    let output = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--entry",
        "klassificer_investeringsselskab_efter_par19",
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&input_path).ok();
    assert!(
        output.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let result = parse_stdout(&output);
    assert!(result["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .is_empty());
    assert_eq!(
        result["results"][0]["result"]["status"]["$variant"],
        "AblPar19IkkeInvesteringsselskab"
    );
}

#[test]
fn xlsx_relational_tables_round_trip_nested_collections_and_isolate_bad_cases() {
    let source_path = temp_path("runa");
    let input_path = temp_path("xlsx");
    std::fs::write(
        &source_path,
        "# Toy(label: String)\n\
# Child(name: String, toys: List(Toy))\n\
# Input(children: List(Child), aliases: List(String), totals: Map(String, Int), flags: Set(Int))\n\
@ calculate\n\
> echo(input: Input) -> Input { input }\n",
    )
    .expect("write relational calculation");
    let template = run(&[
        "template",
        source_path.to_str().expect("source path"),
        "--format",
        "xlsx",
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(
        template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&template.stderr)
    );

    edit_workbook(&input_path, |sheets| {
        set_workbook_cell(sheets, "cases", 1, 0, Data::String("valid".to_string()));
        workbook_sheet_mut(sheets, "cases").push(vec![Data::String("invalid".to_string())]);

        workbook_sheet_mut(sheets, "children").extend([
            vec![
                Data::String("valid".to_string()),
                Data::String("child-valid".to_string()),
                Data::Int(1),
                Data::String("Ada".to_string()),
            ],
            vec![
                Data::String("invalid".to_string()),
                Data::String("child-invalid".to_string()),
                Data::Int(1),
                Data::String("Bad".to_string()),
            ],
        ]);
        workbook_sheet_mut(sheets, "children_toys").extend([
            vec![
                Data::String("valid".to_string()),
                Data::String("child-valid".to_string()),
                Data::String("toy-valid".to_string()),
                Data::Int(1),
                Data::String("Ball".to_string()),
            ],
            vec![
                Data::String("invalid".to_string()),
                Data::String("missing-parent".to_string()),
                Data::String("toy-orphan".to_string()),
                Data::Int(1),
                Data::String("Orphan".to_string()),
            ],
        ]);
        workbook_sheet_mut(sheets, "aliases").extend([
            vec![
                Data::String("valid".to_string()),
                Data::String("alias-valid-2".to_string()),
                Data::Int(2),
                Data::String("B".to_string()),
            ],
            vec![
                Data::String("valid".to_string()),
                Data::String("alias-valid-1".to_string()),
                Data::Int(1),
                Data::String("A".to_string()),
            ],
            vec![
                Data::String("invalid".to_string()),
                Data::String("alias-bad-1".to_string()),
                Data::Int(1),
                Data::String("B".to_string()),
            ],
            vec![
                Data::String("invalid".to_string()),
                Data::String("alias-bad-2".to_string()),
                Data::Int(1),
                Data::String("C".to_string()),
            ],
        ]);
        workbook_sheet_mut(sheets, "totals").extend([
            vec![
                Data::String("valid".to_string()),
                Data::String("total-valid".to_string()),
                Data::String("salary".to_string()),
                Data::Int(10),
            ],
            vec![
                Data::String("invalid".to_string()),
                Data::String("total-bad-1".to_string()),
                Data::String("same".to_string()),
                Data::String("not-an-int".to_string()),
            ],
            vec![
                Data::String("invalid".to_string()),
                Data::String("total-bad-2".to_string()),
                Data::String("same".to_string()),
                Data::Int(2),
            ],
        ]);
        workbook_sheet_mut(sheets, "flags").extend([
            vec![
                Data::String("valid".to_string()),
                Data::String("flag-valid".to_string()),
                Data::Int(7),
            ],
            vec![
                Data::String("invalid".to_string()),
                Data::String("flag-duplicate".to_string()),
                Data::Int(1),
            ],
            vec![
                Data::String("invalid".to_string()),
                Data::String("flag-duplicate".to_string()),
                Data::Int(2),
            ],
        ]);
    });

    let output = run(&[
        "call",
        source_path.to_str().expect("source path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&source_path).ok();
    std::fs::remove_file(&input_path).ok();
    assert!(!output.status.success());
    let result = parse_stdout(&output);
    assert_eq!(result["results"].as_array().expect("results").len(), 1);
    assert_eq!(result["results"][0]["case_id"], "valid");
    assert_eq!(result["results"][0]["result"]["children"][0]["name"], "Ada");
    assert_eq!(
        result["results"][0]["result"]["children"][0]["toys"][0]["label"],
        "Ball"
    );
    assert_eq!(result["results"][0]["result"]["aliases"][0], "A");
    assert_eq!(result["results"][0]["result"]["aliases"][1], "B");
    assert_eq!(result["results"][0]["result"]["totals"]["salary"], 10);
    assert_eq!(result["results"][0]["result"]["flags"][0], 7);
    let diagnostics = result["diagnostics"].as_array().expect("diagnostics");
    for expected in [
        "orphan parent_id",
        "duplicate list position",
        "duplicate map key",
        "duplicate item_id",
        "not an exact signed integer",
    ] {
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic["case_id"] == "invalid"
                && diagnostic["message"]
                    .as_str()
                    .is_some_and(|message| message.contains(expected))
        }));
    }
}

#[test]
fn xlsx_required_nested_product_with_only_empty_collections_round_trips_every_case() {
    let source_path = temp_path("runa");
    let input_path = temp_path("xlsx");
    std::fs::write(
        &source_path,
        "# EmptyCollections(items: List(Int), names: Set(String), values: Map(String, Int))\n\
# EmptyCollectionsInput(marker: Int, nested: EmptyCollections)\n\
@ calculate\n\
> echo_empty_collections(input: EmptyCollectionsInput) -> EmptyCollectionsInput { input }\n",
    )
    .expect("write empty nested collections calculation");
    let template = run(&[
        "template",
        source_path.to_str().expect("source path"),
        "--format",
        "xlsx",
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(
        template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&template.stderr)
    );

    edit_workbook(&input_path, |sheets| {
        set_workbook_cell_by_header(
            sheets,
            "cases",
            1,
            "case_id",
            Data::String("first".to_string()),
        );
        set_workbook_cell_by_header(sheets, "cases", 1, "marker", Data::Int(1));
        set_workbook_cell_by_header(
            sheets,
            "cases",
            2,
            "case_id",
            Data::String("second".to_string()),
        );
        set_workbook_cell_by_header(sheets, "cases", 2, "marker", Data::Int(2));
    });

    let output = run(&[
        "call",
        source_path.to_str().expect("source path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&source_path).ok();
    std::fs::remove_file(&input_path).ok();
    assert!(
        output.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let result = parse_stdout(&output);
    assert!(result["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .is_empty());
    assert_eq!(result["results"].as_array().expect("results").len(), 2);
    for (index, marker) in [1, 2].into_iter().enumerate() {
        assert_eq!(result["results"][index]["result"]["marker"], marker);
        assert_eq!(
            result["results"][index]["result"]["nested"],
            serde_json::json!({ "items": [], "names": [], "values": {} })
        );
    }
}

#[test]
fn xlsx_payload_variants_expand_into_typed_columns_and_child_tables() {
    let source_path = temp_path("runa");
    let input_path = temp_path("xlsx");
    std::fs::write(
        &source_path,
        "# Child(name: String, age: Int)\n\
# Selection = Empty | Fixed(amount: Int) | Pair(Int, String) | Family(label: String, children: List(Child))\n\
# Input(selection: Selection, history: List(Selection))\n\
@ calculate\n\
> echo(input: Input) -> Input { input }\n",
    )
    .expect("write payload-variant calculation");
    let template = run(&[
        "template",
        source_path.to_str().expect("source path"),
        "--format",
        "xlsx",
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(
        template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&template.stderr)
    );

    {
        let mut workbook = open_workbook_auto(&input_path).expect("input workbook");
        assert_eq!(
            workbook_headers(&mut workbook, "cases"),
            [
                "case_id",
                "Selection / Variant",
                "Selection / Fixed / Amount",
                "Selection / Pair / 0",
                "Selection / Pair / 1",
                "Selection / Family / Label",
            ]
        );
        assert_eq!(
            workbook_headers(&mut workbook, "history"),
            [
                "case_id",
                "item_id",
                "position",
                "Variant",
                "Fixed / Amount",
                "Pair / 0",
                "Pair / 1",
                "Family / Label",
            ]
        );
        assert_eq!(
            workbook_headers(&mut workbook, "selection_Family_children"),
            ["case_id", "item_id", "position", "Name", "Age"]
        );
        assert_eq!(
            workbook_headers(&mut workbook, "history_Family_children"),
            ["case_id", "parent_id", "item_id", "position", "Name", "Age",]
        );
    }

    edit_workbook(&input_path, |sheets| {
        set_workbook_cell(sheets, "cases", 1, 1, Data::String("Family".to_string()));
        set_workbook_cell(sheets, "cases", 1, 5, Data::String("primary".to_string()));
        workbook_sheet_mut(sheets, "cases").push(vec![
            Data::String("invalid".to_string()),
            Data::String("Empty".to_string()),
            Data::Int(9),
        ]);
        workbook_sheet_mut(sheets, "cases").push(vec![
            Data::String("inactive-table".to_string()),
            Data::String("Empty".to_string()),
        ]);
        workbook_sheet_mut(sheets, "selection_Family_children").extend([
            vec![
                Data::String("case-1".to_string()),
                Data::String("root-child".to_string()),
                Data::Int(1),
                Data::String("Ada".to_string()),
                Data::Int(7),
            ],
            vec![
                Data::String("inactive-table".to_string()),
                Data::String("inactive-child".to_string()),
                Data::Int(1),
                Data::String("No".to_string()),
                Data::Int(1),
            ],
        ]);
        workbook_sheet_mut(sheets, "history").extend([
            vec![
                Data::String("case-1".to_string()),
                Data::String("history-fixed".to_string()),
                Data::Int(1),
                Data::String("Fixed".to_string()),
                Data::Int(12),
            ],
            vec![
                Data::String("case-1".to_string()),
                Data::String("history-pair".to_string()),
                Data::Int(2),
                Data::String("Pair".to_string()),
                Data::Empty,
                Data::Int(7),
                Data::String("seven".to_string()),
            ],
            vec![
                Data::String("case-1".to_string()),
                Data::String("history-family".to_string()),
                Data::Int(3),
                Data::String("Family".to_string()),
                Data::Empty,
                Data::Empty,
                Data::Empty,
                Data::String("secondary".to_string()),
            ],
        ]);
        workbook_sheet_mut(sheets, "history_Family_children").push(vec![
            Data::String("case-1".to_string()),
            Data::String("history-family".to_string()),
            Data::String("nested-child".to_string()),
            Data::Int(1),
            Data::String("Bo".to_string()),
            Data::Int(10),
        ]);
    });

    let output = run(&[
        "call",
        source_path.to_str().expect("source path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&source_path).ok();
    std::fs::remove_file(&input_path).ok();
    assert!(!output.status.success());
    let result = parse_stdout(&output);
    assert_eq!(result["results"].as_array().expect("results").len(), 1);
    assert_eq!(result["results"][0]["case_id"], "case-1");
    assert_eq!(
        result["results"][0]["result"]["selection"]["$variant"],
        "Family"
    );
    assert_eq!(
        result["results"][0]["result"]["selection"]["children"][0]["name"],
        "Ada"
    );
    assert_eq!(
        result["results"][0]["result"]["history"][0],
        serde_json::json!({ "$variant": "Fixed", "amount": 12 })
    );
    assert_eq!(
        result["results"][0]["result"]["history"][1],
        serde_json::json!({ "$variant": "Pair", "$values": [7, "seven"] })
    );
    assert_eq!(
        result["results"][0]["result"]["history"][2]["children"][0]["name"],
        "Bo"
    );
    assert!(result["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .any(|diagnostic| {
            diagnostic["case_id"] == "invalid"
                && diagnostic["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("input column is inactive"))
        }));
    assert!(result["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .any(|diagnostic| {
            diagnostic["case_id"] == "inactive-table"
                && diagnostic["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("collection table is inactive"))
        }));
}

#[test]
fn xlsx_rejects_tampered_collection_topology() {
    let fixture = fixture();
    let input_path = temp_path("xlsx");
    let template = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--format",
        "xlsx",
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(template.status.success());
    edit_workbook(&input_path, |sheets| {
        set_workbook_cell(
            sheets,
            "_tables",
            1,
            0,
            Data::String("tampered.children".to_string()),
        );
    });

    let output = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&input_path).ok();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("`_tables` metadata does not match the current contract"));
}

#[test]
fn xlsx_rejects_tampered_visible_calculation_title() {
    let fixture = fixture();
    let input_path = temp_path("xlsx");
    let template = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--format",
        "xlsx",
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(template.status.success());
    edit_workbook(&input_path, |sheets| {
        workbook_sheet_mut(sheets, "cases")[0][0] =
            Data::String("Tampered calculation".to_string());
    });

    let output = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&input_path).ok();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("`cases` title is `Tampered calculation`, expected `Household tax calculation`"));
}

#[test]
fn calculate_accepts_one_human_label() {
    let path = temp_path("runa");
    std::fs::write(
        &path,
        "# Input(value: Int)\n@ calculate(\"Income from sailor activities\")\n| calculate(input: Input) -> input.value\n",
    )
    .expect("write labelled calculation source");
    let output = run(&["schema", path.to_str().expect("source path")]);
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let schema = parse_stdout(&output);
    assert_eq!(schema["label"], "Income from sailor activities");

    let workbook_path = temp_path("xlsx");
    let template = run(&[
        "template",
        path.to_str().expect("source path"),
        "--format",
        "xlsx",
        "--output",
        workbook_path.to_str().expect("workbook path"),
    ]);
    assert!(template.status.success());
    let mut workbook = open_workbook_auto(&workbook_path).expect("labelled workbook");
    assert_eq!(
        workbook_title(&mut workbook, "cases"),
        "Income from sailor activities"
    );
    drop(workbook);
    std::fs::remove_file(&workbook_path).ok();

    std::fs::write(
        &path,
        "# Input(value: Int)\n@ calculate(\"Sailor income\")\n| calculate(input: Input) -> input.value\n",
    )
    .expect("rewrite labelled calculation source");
    let changed = run(&["schema", path.to_str().expect("source path")]);
    std::fs::remove_file(&path).ok();
    assert!(changed.status.success());
    let changed_schema = parse_stdout(&changed);
    assert_eq!(changed_schema["label"], "Sailor income");
    assert_ne!(schema["schema_hash"], changed_schema["schema_hash"]);
}

#[test]
fn calculate_rejects_invalid_human_labels() {
    for (annotation, expected) in [
        ("@ calculate(42)", "accepts one human-readable string label"),
        ("@ calculate(\"\")", "label must not be empty"),
        (
            "@ calculate(\"First\", \"Second\")",
            "accepts at most one human-readable string label",
        ),
    ] {
        let path = temp_path("runa");
        std::fs::write(
            &path,
            format!(
                "# Input(value: Int)\n{annotation}\n| calculate(input: Input) -> input.value\n"
            ),
        )
        .expect("write invalid labelled calculation source");
        let output = run(&["check", path.to_str().expect("source path")]);
        std::fs::remove_file(&path).ok();
        assert!(
            !output.status.success(),
            "annotation unexpectedly passed: {annotation}"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "stderr for {annotation}:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn calculation_field_metadata_rejects_unknown_paths_and_duplicates() {
    let unknown_path = temp_path("runa");
    std::fs::write(
        &unknown_path,
        "# Input(value: Int)\n\
# CalculationField(path: String, label: String, question: String, help: String, unit: String)\n\
= bad_field = CalculationField(path = \"missing\", label = \"Missing\", question = \"\", help = \"\", unit = \"\")\n\
--@label:calculate::field:bad_field--\n\
@ calculate\n\
| calculate(input: Input) -> input.value\n",
    )
    .expect("write unknown field metadata source");
    let output = run(&["check", unknown_path.to_str().expect("source path")]);
    std::fs::remove_file(&unknown_path).ok();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown input path `missing`"));

    let duplicate_path = temp_path("runa");
    std::fs::write(
        &duplicate_path,
        "# Input(value: Int)\n\
# CalculationField(path: String, label: String, question: String, help: String, unit: String)\n\
= first_field = CalculationField(path = \"value\", label = \"First\", question = \"\", help = \"\", unit = \"\")\n\
= second_field = CalculationField(path = \"Input.value\", label = \"Second\", question = \"\", help = \"\", unit = \"\")\n\
--@label:calculate::field:first_field::field:second_field--\n\
@ calculate\n\
| calculate(input: Input) -> input.value\n",
    )
    .expect("write duplicate field metadata source");
    let output = run(&["check", duplicate_path.to_str().expect("source path")]);
    std::fs::remove_file(&duplicate_path).ok();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("duplicate field metadata for `value`")
    );
}

#[test]
fn aggregate_calculation_field_metadata_rejects_unknown_paths_and_duplicates() {
    let source = |fields: &str| {
        format!(
            "# Input(value: Int)\n\
# Result(value: Int)\n\
# CalculationField(path: String, label: String, question: String, help: String, unit: String)\n\
# CalculationMeta(fields: List(CalculationField))\n\
# impl Meta for CalculationMeta {{}}\n\
= calculation_meta = CalculationMeta(fields = [{fields}])\n\
--@label:calculate::meta:calculation_meta--\n\
@ calculate\n\
| calculate(input: Input) -> Result(value = input.value)\n"
        )
    };

    let unknown_path = temp_path("runa");
    std::fs::write(
        &unknown_path,
        source(
            "CalculationField(path = \"missing\", label = \"Missing\", question = \"\", help = \"\", unit = \"\")",
        ),
    )
    .expect("write aggregate unknown field metadata source");
    let output = run(&["check", unknown_path.to_str().expect("source path")]);
    std::fs::remove_file(&unknown_path).ok();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("calculation_meta.fields[0]"));
    assert!(stderr.contains("unknown input path `missing`"));

    let duplicate_path = temp_path("runa");
    std::fs::write(
        &duplicate_path,
        source(
            "CalculationField(path = \"value\", label = \"First\", question = \"\", help = \"\", unit = \"\"), CalculationField(path = \"Input.value\", label = \"Second\", question = \"\", help = \"\", unit = \"\")",
        ),
    )
    .expect("write aggregate duplicate field metadata source");
    let output = run(&["check", duplicate_path.to_str().expect("source path")]);
    std::fs::remove_file(&duplicate_path).ok();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("duplicate field metadata for `value`"));
    assert!(stderr.contains("calculation_meta.fields[0]"));
    assert!(stderr.contains("calculation_meta.fields[1]"));
}

#[test]
fn changing_field_metadata_makes_an_xlsx_template_stale() {
    let source_path = temp_path("runa");
    let input_path = temp_path("xlsx");
    let source = |label: &str| {
        format!(
            "# Input(value: Int)\n\
# CalculationField(path: String, label: String, question: String, help: String, unit: String)\n\
= value_field = CalculationField(path = \"value\", label = {label:?}, question = \"What is the value?\", help = \"\", unit = \"\")\n\
--@label:calculate::field:value_field--\n\
@ calculate\n\
| calculate(input: Input) -> input.value\n"
        )
    };
    std::fs::write(&source_path, source("Original label")).expect("write calculation source");
    let template = run(&[
        "template",
        source_path.to_str().expect("source path"),
        "--format",
        "xlsx",
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(
        template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&template.stderr)
    );

    std::fs::write(&source_path, source("Updated label")).expect("update calculation source");
    let output = run(&[
        "call",
        source_path.to_str().expect("source path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&source_path).ok();
    std::fs::remove_file(&input_path).ok();
    assert!(!output.status.success());
    let result = parse_stdout(&output);
    assert!(result["diagnostics"][0]["message"]
        .as_str()
        .expect("diagnostic")
        .contains("stale calculation template"));
}

#[test]
fn changing_calculation_label_makes_an_xlsx_template_stale() {
    let source_path = temp_path("runa");
    let input_path = temp_path("xlsx");
    let source = |label: &str| {
        format!(
            "# Input(value: Int)\n@ calculate({label:?})\n| calculate(input: Input) -> input.value\n"
        )
    };
    std::fs::write(&source_path, source("Original calculation")).expect("write calculation source");
    let template = run(&[
        "template",
        source_path.to_str().expect("source path"),
        "--format",
        "xlsx",
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(template.status.success());

    std::fs::write(&source_path, source("Updated calculation")).expect("update calculation source");
    let output = run(&[
        "call",
        source_path.to_str().expect("source path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&source_path).ok();
    std::fs::remove_file(&input_path).ok();
    assert!(!output.status.success());
    let result = parse_stdout(&output);
    assert!(result["diagnostics"][0]["message"]
        .as_str()
        .expect("diagnostic")
        .contains("stale calculation template"));
}

#[test]
fn typed_function_is_a_calculation_boundary_too() {
    let source_path = temp_path("runa");
    let input_path = temp_path("json");
    std::fs::write(
        &source_path,
        "# Input(value: Int)\n# Output(value: Int)\n@ calculate\n> increment(input: Input) -> Output { Output(value = input.value + 1) }\n",
    )
    .expect("write function calculation");
    let template = run(&[
        "template",
        source_path.to_str().expect("source path"),
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(
        template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&template.stderr)
    );
    let mut input: Value =
        serde_json::from_slice(&std::fs::read(&input_path).expect("template")).unwrap();
    input["cases"][0]["input"]["value"] = 41.into();
    std::fs::write(&input_path, serde_json::to_vec_pretty(&input).unwrap()).unwrap();
    let output = run(&[
        "call",
        source_path.to_str().expect("source path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&source_path).ok();
    std::fs::remove_file(&input_path).ok();
    assert!(output.status.success());
    assert_eq!(parse_stdout(&output)["results"][0]["result"]["value"], 42);
}

#[test]
fn untyped_rule_boundary_is_rejected_by_check() {
    let path = temp_path("runa");
    std::fs::write(
        &path,
        "# Input(value: Int)\n@ calculate\n| calculate(input) -> input\n",
    )
    .expect("write invalid calculation");
    let output = run(&["check", path.to_str().expect("source path")]);
    std::fs::remove_file(&path).ok();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("input needs an explicit type"));
}

#[test]
fn multiple_entries_require_explicit_selection() {
    let path = temp_path("runa");
    std::fs::write(
        &path,
        "# Input(value: Int)\n# Output(value: Int)\n@ calculate\n> first(input: Input) -> Output { Output(value = input.value) }\n@ calculate\n> second(input: Input) -> Output { Output(value = input.value + 1) }\n",
    )
    .expect("write multiple calculations");
    let ambiguous = run(&["schema", path.to_str().expect("source path")]);
    assert!(!ambiguous.status.success());
    assert!(String::from_utf8_lossy(&ambiguous.stderr).contains("multiple calculations"));

    let selected = run(&[
        "schema",
        path.to_str().expect("source path"),
        "--entry",
        "second",
    ]);
    std::fs::remove_file(&path).ok();
    assert!(selected.status.success());
    assert_eq!(parse_stdout(&selected)["entry"], "second");
}

#[test]
fn xlsx_formula_is_rejected_before_evaluation() {
    let fixture = fixture();
    let schema_output = run(&["schema", fixture.to_str().expect("fixture path")]);
    assert!(schema_output.status.success());
    let schema = parse_stdout(&schema_output);
    let hash = schema["schema_hash"].as_str().expect("schema hash");
    let input_path = temp_path("xlsx");
    write_test_workbook(&input_path, hash, true);

    let output = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&input_path).ok();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("formulas are rejected"));
}

#[test]
fn stale_xlsx_template_reports_structural_hash_diagnostic() {
    let fixture = fixture();
    let input_path = temp_path("xlsx");
    write_test_workbook(&input_path, "stale", false);
    let output = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&input_path).ok();
    assert!(!output.status.success());
    let result = parse_stdout(&output);
    assert!(result["diagnostics"][0]["message"]
        .as_str()
        .expect("diagnostic")
        .contains("stale calculation template"));
}

#[test]
fn canonical_values_cover_generics_sums_maps_sets_and_case_isolation() {
    let source_path = temp_path("runa");
    let input_path = temp_path("json");
    std::fs::write(
        &source_path,
        "# Choice = Fixed(amount: Int) | Pair(Int, String) | Empty\n\
# Box(a) = Box(value: a)\n\
# Input(choice: Choice, boxed: Box(Int), totals: Map(String, Int), flags: Set(Int))\n\
# Output(choice: Choice, boxed: Box(Int), totals: Map(String, Int), flags: Set(Int))\n\
@ calculate\n\
> echo(input: Input) -> Output {\n\
    Output(choice = input.choice, boxed = input.boxed, totals = input.totals, flags = input.flags)\n\
}\n",
    )
    .expect("write canonical-value calculation");
    let template = run(&[
        "template",
        source_path.to_str().expect("source path"),
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(
        template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&template.stderr)
    );
    let mut input: Value =
        serde_json::from_slice(&std::fs::read(&input_path).expect("template")).unwrap();
    input["cases"] = serde_json::json!([
        {
            "case_id": "valid",
            "input": {
                "choice": { "$variant": "Pair", "$values": [7, "seven"] },
                "boxed": { "value": 9 },
                "totals": { "a": 1, "b": 2 },
                "flags": [2, 1]
            }
        },
        {
            "case_id": "duplicate-set",
            "input": {
                "choice": { "$variant": "Empty" },
                "boxed": { "value": 0 },
                "totals": {},
                "flags": [1, 1]
            }
        },
        {
            "case_id": "unknown-field",
            "input": {
                "choice": { "$variant": "Fixed", "amount": 3 },
                "boxed": { "value": 0 },
                "totals": {},
                "flags": [],
                "undeclared": true
            }
        }
    ]);
    std::fs::write(&input_path, serde_json::to_vec_pretty(&input).unwrap()).unwrap();

    let output = run(&[
        "call",
        source_path.to_str().expect("source path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&source_path).ok();
    std::fs::remove_file(&input_path).ok();
    assert!(!output.status.success());
    let result = parse_stdout(&output);
    assert_eq!(result["results"].as_array().expect("results").len(), 1);
    assert_eq!(result["results"][0]["case_id"], "valid");
    assert_eq!(
        result["results"][0]["result"]["choice"],
        serde_json::json!({ "$variant": "Pair", "$values": [7, "seven"] })
    );
    assert_eq!(result["results"][0]["result"]["boxed"]["value"], 9);
    let diagnostics = result["diagnostics"].as_array().expect("diagnostics");
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic["case_id"] == "duplicate-set"
            && diagnostic["message"]
                .as_str()
                .is_some_and(|message| message.contains("duplicate set value"))
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic["case_id"] == "unknown-field"
            && diagnostic["message"]
                .as_str()
                .is_some_and(|message| message.contains("unknown field"))
    }));
}

fn write_test_workbook(path: &Path, schema_hash: &str, formula: bool) {
    let mut workbook = rust_xlsxwriter::Workbook::new();
    let metadata = workbook.add_worksheet();
    metadata.set_name("_futuruna").unwrap();
    metadata.write_string(0, 0, "key").unwrap();
    metadata.write_string(0, 1, "value").unwrap();
    for (row, (key, value)) in [
        ("schema", "futuruna.calculate.xlsx.input.v6"),
        ("contract_schema", "futuruna.calculate.v1"),
        ("schema_hash", schema_hash),
        ("entry", "calculate_tax"),
        ("encoding", "futuruna-canonical-json-v1"),
        ("label", "Household tax calculation"),
    ]
    .into_iter()
    .enumerate()
    {
        metadata.write_string(row as u32 + 1, 0, key).unwrap();
        metadata.write_string(row as u32 + 1, 1, value).unwrap();
    }

    let cases = workbook.add_worksheet();
    cases.set_name("cases").unwrap();
    cases
        .write_string(0, 0, "Household tax calculation")
        .unwrap();
    for (column, header) in ["case_id", "monthly_income", "filing_status", "deduction"]
        .into_iter()
        .enumerate()
    {
        cases.write_string(1, column as u16, header).unwrap();
    }
    cases.write_string(2, 0, "case-1").unwrap();
    if formula {
        cases.write_formula(2, 1, "=1+1").unwrap();
    } else {
        cases.write_string(2, 1, "0").unwrap();
    }
    cases.write_string(2, 2, "Single").unwrap();
    workbook.save(path).unwrap();
}

fn workbook_title(
    workbook: &mut calamine::Sheets<std::io::BufReader<std::fs::File>>,
    sheet: &str,
) -> String {
    workbook
        .worksheet_range(sheet)
        .expect("worksheet")
        .rows()
        .next()
        .and_then(|row| row.first())
        .expect("title cell")
        .to_string()
}

fn workbook_headers(
    workbook: &mut calamine::Sheets<std::io::BufReader<std::fs::File>>,
    sheet: &str,
) -> Vec<String> {
    workbook
        .worksheet_range(sheet)
        .expect("worksheet")
        .rows()
        .nth(1)
        .expect("header row")
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn workbook_column_paths(
    workbook: &mut calamine::Sheets<std::io::BufReader<std::fs::File>>,
    sheet: &str,
) -> Vec<String> {
    let metadata = workbook
        .worksheet_range("_columns")
        .expect("column metadata");
    let headers = metadata.rows().next().expect("column metadata headers");
    let sheet_column = headers
        .iter()
        .position(|cell| cell.to_string() == "sheet")
        .expect("sheet metadata column");
    let path_column = headers
        .iter()
        .position(|cell| cell.to_string() == "path")
        .expect("path metadata column");
    metadata
        .rows()
        .skip(1)
        .filter(|row| {
            row.get(sheet_column)
                .is_some_and(|cell| cell.to_string() == sheet)
        })
        .filter_map(|row| row.get(path_column))
        .map(ToString::to_string)
        .collect()
}

fn workbook_column_choices(
    workbook: &mut calamine::Sheets<std::io::BufReader<std::fs::File>>,
    sheet: &str,
    path: &str,
) -> Vec<String> {
    let metadata = workbook
        .worksheet_range("_columns")
        .expect("column metadata");
    let headers = metadata.rows().next().expect("column metadata headers");
    let sheet_column = headers
        .iter()
        .position(|cell| cell.to_string() == "sheet")
        .expect("sheet metadata column");
    let path_column = headers
        .iter()
        .position(|cell| cell.to_string() == "path")
        .expect("path metadata column");
    let choices_column = headers
        .iter()
        .position(|cell| cell.to_string() == "choices")
        .expect("choices metadata column");
    metadata
        .rows()
        .skip(1)
        .find(|row| {
            row.get(sheet_column)
                .is_some_and(|cell| cell.to_string() == sheet)
                && row
                    .get(path_column)
                    .is_some_and(|cell| cell.to_string() == path)
        })
        .and_then(|row| row.get(choices_column))
        .map(ToString::to_string)
        .unwrap_or_default()
        .split(" | ")
        .filter(|choice| !choice.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn workbook_collection_sheet_name(
    workbook: &mut calamine::Sheets<std::io::BufReader<std::fs::File>>,
    path: &str,
) -> String {
    let rows: Vec<Vec<Data>> = workbook
        .worksheet_range("_tables")
        .expect("table metadata")
        .rows()
        .map(|row| row.to_vec())
        .collect();
    collection_sheet_name_from_metadata_rows(&rows, path)
}

fn workbook_collection_sheet_name_from_rows(
    sheets: &[(String, Vec<Vec<Data>>)],
    path: &str,
) -> String {
    let rows = &sheets
        .iter()
        .find(|(sheet, _)| sheet == "_tables")
        .expect("table metadata sheet")
        .1;
    collection_sheet_name_from_metadata_rows(rows, path)
}

fn collection_sheet_name_from_metadata_rows(rows: &[Vec<Data>], path: &str) -> String {
    let headers = rows.first().expect("table metadata headers");
    let path_column = headers
        .iter()
        .position(|cell| cell.to_string() == "path")
        .expect("table path column");
    let sheet_column = headers
        .iter()
        .position(|cell| cell.to_string() == "sheet")
        .expect("table sheet column");
    rows.iter()
        .skip(1)
        .find(|row| {
            row.get(path_column)
                .is_some_and(|cell| cell.to_string() == path)
        })
        .and_then(|row| row.get(sheet_column))
        .map(ToString::to_string)
        .unwrap_or_else(|| panic!("missing collection metadata for {path}"))
}

fn assert_workbook_visibility(
    workbook: &calamine::Sheets<std::io::BufReader<std::fs::File>>,
    hidden_sheets: &[&str],
    first_visible_sheet: &str,
) {
    let metadata = workbook.sheets_metadata();
    for expected in hidden_sheets {
        let sheet = metadata
            .iter()
            .find(|sheet| sheet.name == *expected)
            .unwrap_or_else(|| panic!("missing generated sheet {expected}"));
        assert_eq!(
            sheet.visible,
            SheetVisible::Hidden,
            "machine sheet {expected} should be hidden"
        );
    }
    assert_eq!(
        metadata
            .iter()
            .find(|sheet| sheet.visible == SheetVisible::Visible)
            .map(|sheet| sheet.name.as_str()),
        Some(first_visible_sheet)
    );
}

fn edit_workbook(path: &Path, edit: impl FnOnce(&mut Vec<(String, Vec<Vec<Data>>)>)) {
    let mut workbook = open_workbook_auto(path).expect("open workbook for editing");
    let sheet_names = workbook.sheet_names().to_vec();
    let mut sheets: Vec<(String, Vec<Vec<Data>>)> = sheet_names
        .into_iter()
        .map(|name| {
            let rows = workbook
                .worksheet_range(&name)
                .expect("worksheet")
                .rows()
                .map(|row| row.to_vec())
                .collect();
            (name, rows)
        })
        .collect();
    drop(workbook);
    edit(&mut sheets);

    let mut workbook = rust_xlsxwriter::Workbook::new();
    for (name, rows) in sheets {
        let worksheet = workbook.add_worksheet();
        worksheet.set_name(&name).expect("sheet name");
        for (row, cells) in rows.iter().enumerate() {
            for (column, cell) in cells.iter().enumerate() {
                write_workbook_data(worksheet, row as u32, column as u16, cell);
            }
        }
    }
    workbook.save(path).expect("save edited workbook");
}

fn workbook_sheet_mut<'a>(
    sheets: &'a mut [(String, Vec<Vec<Data>>)],
    name: &str,
) -> &'a mut Vec<Vec<Data>> {
    &mut sheets
        .iter_mut()
        .find(|(sheet, _)| sheet == name)
        .unwrap_or_else(|| panic!("missing sheet {name}"))
        .1
}

fn set_workbook_cell(
    sheets: &mut [(String, Vec<Vec<Data>>)],
    sheet: &str,
    row: usize,
    column: usize,
    value: Data,
) {
    let rows = workbook_sheet_mut(sheets, sheet);
    let row = if sheet.starts_with('_') { row } else { row + 1 };
    while rows.len() <= row {
        rows.push(Vec::new());
    }
    if rows[row].len() <= column {
        rows[row].resize(column + 1, Data::Empty);
    }
    rows[row][column] = value;
}

fn copy_workbook_data_row(
    sheets: &mut [(String, Vec<Vec<Data>>)],
    sheet: &str,
    source_row: usize,
    target_row: usize,
) {
    let rows = workbook_sheet_mut(sheets, sheet);
    let source_physical_row = source_row + 1;
    let target_physical_row = target_row + 1;
    let source = rows
        .get(source_physical_row)
        .unwrap_or_else(|| panic!("missing source row {source_row} on sheet {sheet}"))
        .clone();
    while rows.len() <= target_physical_row {
        rows.push(Vec::new());
    }
    rows[target_physical_row] = source;
}

fn set_workbook_cell_by_header(
    sheets: &mut [(String, Vec<Vec<Data>>)],
    sheet: &str,
    row: usize,
    header: &str,
    value: Data,
) {
    let metadata_column = calculation_workbook_column(sheets, sheet, header);
    let display_header = calculation_workbook_display_header(sheets, sheet, header)
        .unwrap_or_else(|| header.to_string());
    let rows = workbook_sheet_mut(sheets, sheet);
    let physical_row = row + 1;
    let column = metadata_column
        .or_else(|| {
            rows.get(1)
                .expect("header row")
                .iter()
                .position(|cell| cell.to_string() == display_header)
        })
        .unwrap_or_else(|| {
            panic!("missing column {header} (displayed as {display_header}) on sheet {sheet}")
        });
    while rows.len() <= physical_row {
        rows.push(Vec::new());
    }
    if rows[physical_row].len() <= column {
        rows[physical_row].resize(column + 1, Data::Empty);
    }
    rows[physical_row][column] = value;
}

fn calculation_workbook_column(
    sheets: &[(String, Vec<Vec<Data>>)],
    sheet: &str,
    path: &str,
) -> Option<usize> {
    let rows = &sheets.iter().find(|(name, _)| name == "_columns")?.1;
    let headers = rows.first()?;
    let column = |name: &str| headers.iter().position(|cell| cell.to_string() == name);
    let sheet_column = column("sheet")?;
    let path_column = column("path")?;
    let input_path_column = column("input_path")?;
    let sheet_rows = rows
        .iter()
        .skip(1)
        .filter(|row| row.get(sheet_column).map(ToString::to_string).as_deref() == Some(sheet))
        .collect::<Vec<_>>();
    let field_column = sheet_rows
        .iter()
        .position(|row| {
            row.get(input_path_column)
                .map(ToString::to_string)
                .as_deref()
                == Some(path)
        })
        .or_else(|| {
            sheet_rows.iter().position(|row| {
                row.get(path_column).map(ToString::to_string).as_deref() == Some(path)
            })
        })?;
    let visible_headers = sheets.iter().find(|(name, _)| name == sheet)?.1.get(1)?;
    let technical_columns = visible_headers.len().checked_sub(sheet_rows.len())?;
    Some(technical_columns + field_column)
}

fn calculation_workbook_display_header(
    sheets: &[(String, Vec<Vec<Data>>)],
    sheet: &str,
    path: &str,
) -> Option<String> {
    let field_column = calculation_workbook_column(sheets, sheet, path)?;
    let visible_headers = sheets.iter().find(|(name, _)| name == sheet)?.1.get(1)?;
    visible_headers.get(field_column).map(ToString::to_string)
}

fn write_workbook_data(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    column: u16,
    value: &Data,
) {
    match value {
        Data::Empty => {}
        Data::String(value) => {
            worksheet.write_string(row, column, value).unwrap();
        }
        Data::Float(value) => {
            worksheet.write_number(row, column, *value).unwrap();
        }
        Data::Int(value) => {
            worksheet.write_number(row, column, *value as f64).unwrap();
        }
        Data::Bool(value) => {
            worksheet.write_boolean(row, column, *value).unwrap();
        }
        value => {
            worksheet
                .write_string(row, column, value.to_string())
                .unwrap();
        }
    }
}
