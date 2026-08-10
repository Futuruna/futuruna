use calamine::{open_workbook_auto, Data, Reader, SheetVisible};
use serde_json::Value;
use std::fs::File;
use std::io::Read;
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

fn run_with_env(args: &[&str], environment: &[(&str, &str)]) -> Output {
    Command::new(runa())
        .args(args)
        .envs(environment.iter().copied())
        .output()
        .expect("run runa calculation command with environment")
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

fn worksheet_xml_parts(path: &Path) -> Vec<(String, String)> {
    let file = File::open(path).expect("open XLSX package");
    let mut archive = zip::ZipArchive::new(file).expect("read XLSX package");
    let mut worksheets = Vec::new();
    for index in 0..archive.len() {
        let mut part = archive.by_index(index).expect("read XLSX package part");
        let name = part.name().to_string();
        if !name.starts_with("xl/worksheets/sheet") || !name.ends_with(".xml") {
            continue;
        }
        let mut xml = String::new();
        part.read_to_string(&mut xml)
            .expect("read worksheet XML as UTF-8");
        worksheets.push((name, xml));
    }
    worksheets
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
fn calculation_contract_cache_hits_and_tracks_transitive_imports() {
    let root = temp_path("calculation-cache");
    std::fs::create_dir_all(&root).expect("create calculation cache fixture directory");
    let cache = root.join("cache");
    let source = root.join("model.calculate.runa");
    let dependency = root.join("domain.runa");
    std::fs::write(
        &source,
        "@ import ./domain\n\
@ calculate(\"Cached calculation\")\n\
| calculate_cached(input: CachedInput) -> CachedResult(total = input.amount)\n",
    )
    .expect("write cached calculation source");
    std::fs::write(
        &dependency,
        "# CachedInput(amount: Int)\n# CachedResult(total: Int)\n",
    )
    .expect("write cached calculation dependency");

    let source = source.to_str().expect("calculation source path");
    let cache = cache.to_str().expect("calculation cache path");
    let environment = [
        ("FUTURUNA_CALCULATION_CACHE_DIR", cache),
        ("FUTURUNA_CALCULATION_CACHE_TRACE", "1"),
    ];
    let first = run_with_env(&["schema", source], &environment);
    assert!(
        first.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(String::from_utf8_lossy(&first.stderr).contains("cache: miss"));
    let first_schema = parse_stdout(&first);

    let second = run_with_env(&["schema", source], &environment);
    assert!(
        second.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(String::from_utf8_lossy(&second.stderr).contains("cache: hit"));
    assert_eq!(parse_stdout(&second), first_schema);

    std::fs::write(
        &dependency,
        "# CachedInput(amount: Int, supplement: Int)\n# CachedResult(total: Int)\n",
    )
    .expect("change cached calculation dependency");
    let invalidated = run_with_env(&["schema", source], &environment);
    assert!(
        invalidated.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&invalidated.stderr)
    );
    assert!(String::from_utf8_lossy(&invalidated.stderr).contains("cache: miss"));
    let invalidated_schema = parse_stdout(&invalidated);
    assert_ne!(
        invalidated_schema["schema_hash"],
        first_schema["schema_hash"]
    );

    std::fs::remove_dir_all(root).ok();
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
fn schema_projects_enum_discriminator_metadata_to_scalar_enum_columns() {
    let path = temp_path("runa");
    std::fs::write(
        &path,
        "# Mode = Fixed | Percentage\n\
# GiftArt = Binding(mode: Mode) | Other\n\
# Gift(art: GiftArt)\n\
# GiftInput(gifts: List(Gift))\n\
# Input(gift_input: GiftInput, direct_mode: Mode)\n\
# Result(value: Int)\n\
# RelativeField(path: ProgramReference, label: String)\n\
# RelativeMeta(fields: List(RelativeField))\n\
# ExactField(path: String, label: String)\n\
# ExactMeta(fields: List(ExactField))\n\
# impl Meta for RelativeMeta {}\n\
# impl Meta for ExactMeta {}\n\
= gift_meta = RelativeMeta(fields = [\n\
    RelativeField(path = refof(GiftInput::gifts::art::Binding::mode::$variant), label = \"Payment mode\")\n\
])\n\
--@label:GiftInput::meta:gift_meta--\n\
= exact_meta = ExactMeta(fields = [\n\
    ExactField(path = pathof(Input::direct_mode::$variant), label = \"Direct mode\")\n\
])\n\
--@label:calculate::meta:exact_meta--\n\
@ calculate\n\
| calculate(input: Input) -> Result(value = 0)\n",
    )
    .expect("write nested enum metadata source");

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
    assert!(fields
        .iter()
        .any(|field| { field["path"] == "direct_mode" && field["label"] == "Direct mode" }));
    assert!(fields.iter().any(|field| {
        field["path"] == "gift_input.gifts.art.Binding.mode" && field["label"] == "Payment mode"
    }));
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

    let worksheets = worksheet_xml_parts(&input_path);
    let (_, validation_sheet) = worksheets
        .iter()
        .find(|(_, xml)| xml.contains("FuturunaChoices1"))
        .expect("worksheet with generated choice validation");
    assert!(
        validation_sheet.contains(r#"sqref="B3:B1001""#),
        "choice validation must start below the row-2 header"
    );
    assert!(!validation_sheet.contains(r#"sqref="B2:B1000""#));

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
            "Valg af sømandsfradrag",
            "Valg af fiskerfradrag",
            "Dødsboets skattegrundlag efter § 30",
            "Dødsboets behandlingsform",
            "Bobeskatningsindkomst",
            "Dødsdato (ISO 8601) - år",
            "Dødsdato (ISO 8601) - måned",
            "Dødsdato (ISO 8601) - dag",
            "Boopgørelsens type",
            "Boopgørelsens skæringsdag (ISO 8601) - år",
            "Boopgørelsens skæringsdag (ISO 8601) - måned",
            "Boopgørelsens skæringsdag (ISO 8601) - dag",
            "Afdødes indkomstårsforhold",
            "Bagudforskudt dødsårs startdato - år",
            "Bagudforskudt dødsårs startdato - måned",
            "Bagudforskudt dødsårs startdato - dag",
            "Fremadforskudt dødsårs startdato - år",
            "Fremadforskudt dødsårs startdato - måned",
            "Fremadforskudt dødsårs startdato - dag",
            "Tidligere afdød ægtefælle efter § 62",
            "Førstafdøde ægtefælles dødsdato - år",
            "Førstafdøde ægtefælles dødsdato - måned",
            "Førstafdøde ægtefælles dødsdato - dag",
            "Førstafdødes særbo efter § 67, stk. 7",
            "Anvendt ekstra progressionsgrænse i førstafdødes særbo",
            "Dokumentation for førstafdødes særboskat",
            "Aktieindkomst i dødsboet",
            "Opgjort aktieindkomst i bobeskatningsperioden",
            "Dokumentation for boets aktieindkomst",
            "Skattehistorik til carry back efter § 31",
            "Længstlevende ægtefælle i § 31-historikken",
            "Længstlevende ægtefælles identifikation",
            "Fordeling mellem fællesbo og særbo",
            "Fællesboets bobeskatningsindkomst",
            "Fællesboets aktieindkomst",
            "Dokumentation for fællesboets indkomster",
            "Særboets bobeskatningsindkomst",
            "Særboets aktieindkomst",
            "Dokumentation for særboets indkomster",
            "Kulbrinteskattegrundlag efter § 21, stk. 2",
            "Personstatus efter kulbrinteskattelovens § 21, stk. 2",
            "Arbejdsgiverens hjemting efter kulbrinteskatteloven",
            "Indkomstkategori efter kulbrinteskatteloven",
            "Dansk beskatningsret til kulbrinteindkomsten",
            "Beskatningsvalg for kulbrinteindkomsten",
            "Alder ved indkomstårets udløb for kulbrinteindkomsten",
            "Status for øvrige lønmodtagerudgifter",
            "Personens rolle ved rejserne",
            "Fradrag for dobbelt husførelse",
            "Skatteyderens status for faglige kontingenter",
            "Skattepligtsposition for A-kasse og lignende bidrag",
            "Fødselsår",
            "Fødselsmåned",
            "Fødselsdag i måneden",
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
            "Ejendomsejerens folkepensionsalder",
            "Samlevende ægtefælles folkepensionsalder",
            "Skattemæssigt hjemsted for pensionistnedslag",
            "Årsopgørelse",
            "Ordinært aktieår",
            "Fremført tab på markedsaktier",
            "Fremført underskud efter § 13",
            "Årsopgørelsens reference for underskuddet",
            "Skatteforvaltningens afgørelsesreference",
            "Anden myndigheds afgørelsesreference",
            "Hovedpersonens fremførte negative aktieskat",
            "Ægtefællens fremførte negative aktieskat",
            "Indkomstår for udenlandsk skattenedslag",
            "Nedslag for arbejde i udlandet efter § 33 A",
        ] {
            assert!(
                case_headers.iter().any(|header| header == expected),
                "missing human Personskatteloven input label {expected}"
            );
        }
        let foreign_tax_payments_path =
            "ligningslov33.hovedperson.MedLigningslov33.input.kreditgrupper.skattebetalinger";
        let foreign_tax_payments_sheet =
            workbook_collection_sheet_name(&mut workbook, foreign_tax_payments_path);
        let foreign_tax_payment_paths =
            workbook_column_paths(&mut workbook, &foreign_tax_payments_sheet);
        for expected in [
            "opkrævningsmåde",
            "betalt_udenlandsk_skat_øre",
            "skatteart",
            "betalingsdokumentreference",
            "overenskomstgrundlag.$variant",
        ] {
            assert!(
                foreign_tax_payment_paths
                    .iter()
                    .any(|path| path == expected),
                "missing canonical LL § 33 tax-payment path {expected} on {foreign_tax_payments_sheet}"
            );
        }
        let foreign_tax_payment_headers =
            workbook_headers(&mut workbook, &foreign_tax_payments_sheet);
        for expected in [
            "Opkrævningsmåde for udenlandsk skat",
            "Betalt udenlandsk skat",
            "Den udenlandske skats art",
            "Dokumentation for betalt udenlandsk skat",
            "Loft efter dobbeltbeskatningsoverenskomst",
        ] {
            assert!(
                foreign_tax_payment_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human LL § 33 tax-payment label {expected} on {foreign_tax_payments_sheet}"
            );
        }
        let ll33a_dated_income_path = "ligningslov33.ligningslov33a_hovedperson.MedLigningslov33A.input.ansættelsesforhold.indkomstgrundlag.Ll33ADateredeIndkomstfordelinger.perioder";
        let ll33a_dated_income_sheet =
            workbook_collection_sheet_name(&mut workbook, ll33a_dated_income_path);
        let ll33a_dated_income_paths =
            workbook_column_paths(&mut workbook, &ll33a_dated_income_sheet);
        for expected in [
            "identifikation",
            "fra_dato.år",
            "fra_dato.måned",
            "fra_dato.dag",
            "til_dato.år",
            "til_dato.måned",
            "til_dato.dag",
            "fordeling.samlet_lønindkomst_efter_danske_regler.skattepligtig_nettoindkomst_kroner",
            "fordeling.heraf_arbejde_i_riget_efter_danske_regler.skattepligtig_nettoindkomst_kroner",
            "dokumentreference",
        ] {
            assert!(
                ll33a_dated_income_paths.iter().any(|path| path == expected),
                "missing canonical LL § 33 A dated-income path {expected} on {ll33a_dated_income_sheet}"
            );
        }
        let ll33a_dated_income_headers = workbook_headers(&mut workbook, &ll33a_dated_income_sheet);
        for expected in [
            "Lønperiodens identifikation",
            "Lønperiodens startår",
            "Lønperiodens startmåned",
            "Lønperiodens startdag",
            "Lønperiodens slutår",
            "Lønperiodens slutmåned",
            "Lønperiodens slutdag",
            "Skattepligtig løn i indkomstfordelingen",
            "Skattepligtig løn for arbejde i Danmark",
            "Dokumentation for lønperioden",
        ] {
            assert!(
                ll33a_dated_income_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human LL § 33 A dated-income label {expected} on {ll33a_dated_income_sheet}"
            );
        }
        let ll33a_period_election_path = "ligningslov33.ligningslov33a_hovedperson.MedLigningslov33A.input.ansættelsesforhold.periodevalg.valgte_udrejsedatoer";
        let ll33a_period_election_sheet =
            workbook_collection_sheet_name(&mut workbook, ll33a_period_election_path);
        let ll33a_period_election_paths =
            workbook_column_paths(&mut workbook, &ll33a_period_election_sheet);
        for expected in ["år", "måned", "dag"] {
            assert!(
                ll33a_period_election_paths
                    .iter()
                    .any(|path| path == expected),
                "missing canonical LL § 33 A period-election path {expected} on {ll33a_period_election_sheet}"
            );
        }
        let ll33a_period_election_headers =
            workbook_headers(&mut workbook, &ll33a_period_election_sheet);
        for expected in [
            "Valgt udrejsedato (ISO 8601) - år",
            "Valgt udrejsedato (ISO 8601) - måned",
            "Valgt udrejsedato (ISO 8601) - dag",
        ] {
            assert!(
                ll33a_period_election_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human LL § 33 A period-election label {expected} on {ll33a_period_election_sheet}"
            );
        }
        let freight_tax_balances_path =
            "ligningslov33.hovedperson.MedLigningslov33.input.fragtskat_åbningssaldi";
        let freight_tax_balances_sheet =
            workbook_collection_sheet_name(&mut workbook, freight_tax_balances_path);
        let freight_tax_balance_paths =
            workbook_column_paths(&mut workbook, &freight_tax_balances_sheet);
        for expected in [
            "område.$variant",
            "område.Ll33FremmedStat.landekode",
            "indkomstår",
            "saldo_øre",
        ] {
            assert!(
                freight_tax_balance_paths.iter().any(|path| path == expected),
                "missing canonical LL § 33(9) balance path {expected} on {freight_tax_balances_sheet}"
            );
        }
        let freight_tax_balance_headers =
            workbook_headers(&mut workbook, &freight_tax_balances_sheet);
        for expected in [
            "Fremmed stat for fremført fragtskat",
            "Landekode",
            "Fremførselsår for fragtskat",
            "Fremført fragtskat",
        ] {
            assert!(
                freight_tax_balance_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human LL § 33(9) balance label {expected} on {freight_tax_balances_sheet}"
            );
        }
        let freight_tax_documents_path = "ligningslov33.hovedperson.MedLigningslov33.input.fragtskat_åbningssaldi.dokumentreferencer";
        let freight_tax_documents_sheet =
            workbook_collection_sheet_name(&mut workbook, freight_tax_documents_path);
        assert_eq!(
            workbook_headers(&mut workbook, &freight_tax_documents_sheet),
            [
                "case_id",
                "parent_id",
                "item_id",
                "position",
                "Dokumentation for fremført fragtskat",
            ]
        );
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
        let nonstandard_taxpayer_paths = workbook_column_paths(&mut workbook, "cases");
        for expected in [
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.$variant",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.boform",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.bobeskatningsindkomst_kroner",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.dødsdato.år",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.dødsdato.måned",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.dødsdato.dag",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.boopgørelsestype",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.boopgørelsens_skæringsdag.år",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.boopgørelsens_skæringsdag.måned",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.boopgørelsens_skæringsdag.dag",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.indkomstårsforhold.$variant",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.indkomstårsforhold.Dbl30BagudforskudtIndkomstår.dødsårets_startdato.år",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.indkomstårsforhold.Dbl30BagudforskudtIndkomstår.dødsårets_startdato.måned",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.indkomstårsforhold.Dbl30BagudforskudtIndkomstår.dødsårets_startdato.dag",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.indkomstårsforhold.Dbl30FremadforskudtIndkomstår.dødsårets_startdato.år",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.indkomstårsforhold.Dbl30FremadforskudtIndkomstår.dødsårets_startdato.måned",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.indkomstårsforhold.Dbl30FremadforskudtIndkomstår.dødsårets_startdato.dag",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.ægtefælleforhold.$variant",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.ægtefælleforhold.Dbl30TidligereAfdødÆgtefælleEfterPar62.førstafdødes_dødsdato.år",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.ægtefælleforhold.Dbl30TidligereAfdødÆgtefælleEfterPar62.førstafdødes_dødsdato.måned",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.ægtefælleforhold.Dbl30TidligereAfdødÆgtefælleEfterPar62.førstafdødes_dødsdato.dag",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.ægtefælleforhold.Dbl30TidligereAfdødÆgtefælleEfterPar62.par67_stk7_progressionsforhold.$variant",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.ægtefælleforhold.Dbl30TidligereAfdødÆgtefælleEfterPar62.par67_stk7_progressionsforhold.Dbl67Stk7FørstafdødesSærboEndeligtSkatteberegnet.anvendt_ekstra_progressionsgrænse_kroner",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.ægtefælleforhold.Dbl30TidligereAfdødÆgtefælleEfterPar62.par67_stk7_progressionsforhold.Dbl67Stk7FørstafdødesSærboEndeligtSkatteberegnet.dokumentreference",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.aktieindkomstgrundlag.$variant",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.aktieindkomstgrundlag.Dbl32OpgjortAktieindkomstEfterPar21.aktieindkomst_kroner",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.aktieindkomstgrundlag.Dbl32OpgjortAktieindkomstEfterPar21.dokumentreference",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.carrybackgrundlag.$variant",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.carrybackgrundlag.Dbl31DokumenteretCarrybackgrundlag.længstlevende_ægtefælleforhold.$variant",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.carrybackgrundlag.Dbl31DokumenteretCarrybackgrundlag.længstlevende_ægtefælleforhold.Dbl31LængstlevendeÆgtefælle.identifikation",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.carrybackgrundlag.Dbl31DokumenteretCarrybackgrundlag.bofordelingsgrundlag.$variant",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.carrybackgrundlag.Dbl31DokumenteretCarrybackgrundlag.bofordelingsgrundlag.Dbl31FællesboOgSærboSkiftesHverForSig.fællesbo.bobeskatningsindkomst_kroner",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.carrybackgrundlag.Dbl31DokumenteretCarrybackgrundlag.bofordelingsgrundlag.Dbl31FællesboOgSærboSkiftesHverForSig.fællesbo.aktieindkomst_kroner",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.carrybackgrundlag.Dbl31DokumenteretCarrybackgrundlag.bofordelingsgrundlag.Dbl31FællesboOgSærboSkiftesHverForSig.fællesbo.boopgørelsens_skæringsdag.år",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.carrybackgrundlag.Dbl31DokumenteretCarrybackgrundlag.bofordelingsgrundlag.Dbl31FællesboOgSærboSkiftesHverForSig.fællesbo.boopgørelsens_skæringsdag.måned",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.carrybackgrundlag.Dbl31DokumenteretCarrybackgrundlag.bofordelingsgrundlag.Dbl31FællesboOgSærboSkiftesHverForSig.fællesbo.boopgørelsens_skæringsdag.dag",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.carrybackgrundlag.Dbl31DokumenteretCarrybackgrundlag.bofordelingsgrundlag.Dbl31FællesboOgSærboSkiftesHverForSig.fællesbo.dokumentreference",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.carrybackgrundlag.Dbl31DokumenteretCarrybackgrundlag.bofordelingsgrundlag.Dbl31FællesboOgSærboSkiftesHverForSig.særbo.bobeskatningsindkomst_kroner",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.carrybackgrundlag.Dbl31DokumenteretCarrybackgrundlag.bofordelingsgrundlag.Dbl31FællesboOgSærboSkiftesHverForSig.særbo.aktieindkomst_kroner",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.carrybackgrundlag.Dbl31DokumenteretCarrybackgrundlag.bofordelingsgrundlag.Dbl31FællesboOgSærboSkiftesHverForSig.særbo.boopgørelsens_skæringsdag.år",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.carrybackgrundlag.Dbl31DokumenteretCarrybackgrundlag.bofordelingsgrundlag.Dbl31FællesboOgSærboSkiftesHverForSig.særbo.boopgørelsens_skæringsdag.måned",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.carrybackgrundlag.Dbl31DokumenteretCarrybackgrundlag.bofordelingsgrundlag.Dbl31FællesboOgSærboSkiftesHverForSig.særbo.boopgørelsens_skæringsdag.dag",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.carrybackgrundlag.Dbl31DokumenteretCarrybackgrundlag.bofordelingsgrundlag.Dbl31FællesboOgSærboSkiftesHverForSig.særbo.dokumentreference",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.kulbrinteskattegrundlag.$variant",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.kulbrinteskattegrundlag.Søbl5BKulbrinteskattegrundlag.kildefakta.personstatus",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.kulbrinteskattegrundlag.Søbl5BKulbrinteskattegrundlag.kildefakta.arbejdsgiverhjemting",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.kulbrinteskattegrundlag.Søbl5BKulbrinteskattegrundlag.kildefakta.indkomstkategori",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.kulbrinteskattegrundlag.Søbl5BKulbrinteskattegrundlag.kildefakta.dansk_beskatningsret",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.kulbrinteskattegrundlag.Søbl5BKulbrinteskattegrundlag.kildefakta.beskatningsvalg",
            "lønmodtager.personlig_indkomst.sømandsbeskatning.kulbrinteskattegrundlag.Søbl5BKulbrinteskattegrundlag.kildefakta.alder_ved_indkomstårets_udløb",
        ] {
            assert!(
                nonstandard_taxpayer_paths
                    .iter()
                    .any(|path| path == expected),
                "missing canonical non-standard SØBL source-fact path {expected}"
            );
        }
        let death_estate_tax_history_path = "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.Søbl5Dødsboskattegrundlag.input.carrybackgrundlag.Dbl31DokumenteretCarrybackgrundlag.betalte_årsskatter";
        let death_estate_tax_history_sheet =
            workbook_collection_sheet_name(&mut workbook, death_estate_tax_history_path);
        assert_eq!(
            workbook_title(&mut workbook, &death_estate_tax_history_sheet),
            "Dansk personskat - Betalte årsskatter til § 31-loftet"
        );
        let death_estate_tax_history_paths =
            workbook_column_paths(&mut workbook, &death_estate_tax_history_sheet);
        for expected in [
            "person",
            "indkomstår",
            "skat_af_skattepligtig_indkomst_kroner",
            "skat_af_aktieindkomst_kroner",
            "arbejdsmarkedsbidrag_kroner",
            "heraf_skat_efter_kildeskattelov48e_48f_kroner",
            "dokumentreference",
        ] {
            assert!(
                death_estate_tax_history_paths
                    .iter()
                    .any(|path| path == expected),
                "missing canonical § 31 tax-history path {expected} on {death_estate_tax_history_sheet}"
            );
        }
        let death_estate_tax_history_headers =
            workbook_headers(&mut workbook, &death_estate_tax_history_sheet);
        for expected in [
            "Person i skattehistorikken",
            "Årsskattens indkomstår",
            "Betalt skat af skattepligtig indkomst",
            "Betalt skat af aktieindkomst",
            "Arbejdsmarkedsbidrag i § 31-historikken",
            "Heraf skat efter §§ 48 E og 48 F",
            "Dokumentation for den betalte årsskat",
        ] {
            assert!(
                death_estate_tax_history_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human § 31 tax-history label {expected} on {death_estate_tax_history_sheet}"
            );
        }
        let seafarer_employments_path = "lønmodtager.ligningsfradrag.sømandsfradrag.beskæftigelser";
        let seafarer_employments_sheet =
            workbook_collection_sheet_name(&mut workbook, seafarer_employments_path);
        let seafarer_employment_paths =
            workbook_column_paths(&mut workbook, &seafarer_employments_sheet);
        for expected in [
            "identifikation",
            "indkomstår",
            "ansættelsesforhold_startdato.år",
            "ansættelsesforhold_startdato.måned",
            "ansættelsesforhold_startdato.dag",
            "arbejdssted.$variant",
            "arbejdssted.ArbejdePåSkib.bruttotonnage",
            "arbejdssted.ArbejdePåSkib.anvendelse.$variant",
            "fart",
            "hjemsted",
            "flag",
            "forhyringsvilkår",
            "fuldtidsomregnede_sødage_hundrededele",
        ] {
            assert!(
                seafarer_employment_paths.iter().any(|path| path == expected),
                "missing canonical SØBL § 3 source-fact path {expected} on {seafarer_employments_sheet}"
            );
        }
        let seafarer_employment_headers =
            workbook_headers(&mut workbook, &seafarer_employments_sheet);
        for expected in [
            "Beskæftigelsens identifikation",
            "Sømandsbeskæftigelsens indkomstår",
            "Ansættelsesforholdets startdato (ISO 8601) - år",
            "Ansættelsesforholdets startdato (ISO 8601) - måned",
            "Ansættelsesforholdets startdato (ISO 8601) - dag",
            "Arbejdssted til søs",
            "Skibets bruttotonnage",
            "Skibets udelukkende anvendelse",
            "Fart uden for eller inden for begrænset fart",
            "Fartøjets eller installationens registrerede hjemsted",
            "Fartøjets flag",
            "Forhyringsvilkår",
            "Fuldtidsomregnede sødage",
        ] {
            assert!(
                seafarer_employment_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human SØBL § 3 input label {expected} on {seafarer_employments_sheet}"
            );
        }
        let seafarer_source_links_path =
            "lønmodtager.ligningsfradrag.sømandsbeskatningslov4.kildetilknytninger";
        let seafarer_source_links_sheet =
            workbook_collection_sheet_name(&mut workbook, seafarer_source_links_path);
        let seafarer_source_link_paths =
            workbook_column_paths(&mut workbook, &seafarer_source_links_sheet);
        for expected in [
            "kilde.$variant",
            "kilde.Søbl4Ligningslov9Stk1Lønmodtagerudgift.kildeidentifikation",
            "kilde.Søbl4Ligningslov9BErhvervsbefordring.kildeidentifikation",
            "kilde.Søbl4Ligningslov9COg9DBefordringsforhold.kildeidentifikation",
            "kilde.Søbl4Pensionsbeskatningslov49Stk1Bidrag.kildeidentifikation",
            "arbejdstilknytning.$variant",
            "arbejdstilknytning.Søbl4Sømandsbeskæftigelsesperiode.beskæftigelsesidentifikation",
            "arbejdstilknytning.Søbl4AndetArbejdsforhold.arbejdsforhold_identifikation",
        ] {
            assert!(
                seafarer_source_link_paths
                    .iter()
                    .any(|path| path == expected),
                "missing canonical SØBL § 4 source-link path {expected} on {seafarer_source_links_sheet}"
            );
        }
        let seafarer_source_link_headers =
            workbook_headers(&mut workbook, &seafarer_source_links_sheet);
        for expected in [
            "Fradragskilde omfattet af § 4",
            "Identifikation for § 9, stk. 1-lønmodtagerudgiften",
            "Identifikation for § 9 B-kørselssagen",
            "Identifikation for §§ 9 C-9 D-befordringsforholdet",
            "Identifikation for § 49, stk. 1-bidraget",
            "Arbejdsperiodens faktiske tilknytning",
            "Tilknyttet sømandsbeskæftigelse",
            "Tilknyttet andet arbejdsforhold",
        ] {
            assert!(
                seafarer_source_link_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human SØBL § 4 source-link label {expected} on {seafarer_source_links_sheet}"
            );
        }
        let commuting_path = "lønmodtager.ligningsfradrag.befordring.forhold";
        let commuting_sheet = workbook_collection_sheet_name(&mut workbook, commuting_path);
        assert_eq!(
            workbook_title(&mut workbook, &commuting_sheet),
            "Dansk personskat - Befordringsforhold til arbejde eller uddannelse"
        );
        let commuting_paths = workbook_column_paths(&mut workbook, &commuting_sheet);
        for expected in [
            "identifikation",
            "befordringsmål_identifikation",
            "arbejdsdage",
            "arbejdsgiverbetalt_befordring",
            "ligningslov9d.$variant",
            "ligningslov9d.MedLigningslov9D.input.faktisk_udgiftsgrundlag.Ll9dDokumenteredeFaktiskeBefordringsudgifter.beløb_kroner",
        ] {
            assert!(
                commuting_paths.iter().any(|path| path == expected),
                "missing canonical commuting source-fact path {expected} on {commuting_sheet}"
            );
        }
        let commuting_headers = workbook_headers(&mut workbook, &commuting_sheet);
        for expected in [
            "Befordringsforholdets identifikation",
            "Arbejds- eller uddannelsessted for befordringen",
            "Arbejdsgiverbetalt transport",
        ] {
            assert!(
                commuting_headers.iter().any(|header| header == expected),
                "missing human commuting input label {expected} on {commuting_sheet}"
            );
        }
        let employee_expenses_path =
            "lønmodtager.ligningsfradrag.øvrige_lønmodtagerudgifter.udgifter";
        let employee_expenses_sheet =
            workbook_collection_sheet_name(&mut workbook, employee_expenses_path);
        assert_eq!(
            workbook_title(&mut workbook, &employee_expenses_sheet),
            "Dansk personskat - Øvrige lønmodtagerudgifter"
        );
        let employee_expense_paths = workbook_column_paths(&mut workbook, &employee_expenses_sheet);
        for expected in [
            "identifikation",
            "indkomstår",
            "arbejdsforhold_identifikation",
            "udgiftsart.$variant",
            "udgiftsart.Ll9Stk1Kursus.formål",
            "udgiftsart.Ll9Stk1Faglitteratur.art",
            "udgiftsart.Ll9Stk1Faglitteratur.nødvendig_for_at_varetage_arbejdet_i_året",
            "udgiftsart.Ll9Stk1SærligtArbejdstøj.særligt_til_arbejdet",
            "udgiftsart.Ll9Stk1SærligtArbejdstøj.kan_anvendes_som_almindeligt_tøj",
            "udgiftsart.Ll9Stk1Arbejdsværelse.arbejdets_art_eller_omfang_gør_rummet_uegnet_som_almindeligt_opholdsrum",
            "udgiftsart.Ll9Stk1DriftsmiddelAfskrivning.erhvervsmæssig_andel_basispoint",
            "udgiftsart.Ll9Stk1Repræsentation.aflønningsform_giver_mulighed_for_at_påvirke_indtægten",
            "udgiftsart.Ll9Stk1AndenUdgift.beskrivelse",
            "udgiftsart.Ll9Stk1AndenUdgift.forbindelse",
            "afholdt_eller_beregnet_beløb_kroner",
            "arbejdsgiver_refunderet_efter_regning_kroner",
            "dokumentation",
        ] {
            assert!(
                employee_expense_paths.iter().any(|path| path == expected),
                "missing canonical LL § 9, stk. 1 source-fact path {expected} on {employee_expenses_sheet}"
            );
        }
        let employee_expense_headers = workbook_headers(&mut workbook, &employee_expenses_sheet);
        for expected in [
            "Lønmodtagerudgiftens identifikation",
            "Lønmodtagerudgiftens indkomstår",
            "Arbejdsforhold for lønmodtagerudgiften",
            "Lønmodtagerudgiftens art",
            "Kursets faglige formål",
            "Faglitteraturens art",
            "Faglitteraturen var nødvendig for arbejdet",
            "Tøjet er særligt til arbejdet",
            "Tøjet kan anvendes som almindeligt tøj",
            "Arbejdsværelset er uegnet som almindeligt opholdsrum",
            "Driftsmidlets erhvervsmæssige andel",
            "Aflønningsformen kan påvirkes af repræsentationen",
            "Beskrivelse af anden lønmodtagerudgift",
            "Den anden udgifts forbindelse til arbejdet",
            "Afholdt eller beregnet lønmodtagerudgift",
            "Arbejdsgiverens refusion efter regning",
            "Dokumentation for lønmodtagerudgiften",
        ] {
            assert!(
                employee_expense_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human LL § 9, stk. 1 input label {expected} on {employee_expenses_sheet}"
            );
        }
        let dis_income_path = "lønmodtager.personlig_indkomst.sømandsbeskatning.indkomster";
        let dis_income_sheet = workbook_collection_sheet_name(&mut workbook, dis_income_path);
        assert_eq!(
            workbook_title(&mut workbook, &dis_income_sheet),
            "Dansk personskat - Lønindkomster fra arbejde om bord på skibe"
        );
        let dis_income_paths = workbook_column_paths(&mut workbook, &dis_income_sheet);
        for expected in [
            "identifikation",
            "indkomstår",
            "person.skattepligt",
            "person.statsborgerskab",
            "person.relation.$variant",
            "person.relation.SøblEjerPartrederEllerInteressent.sammenligning",
            "person.relation.SøblBestemmendeIndflydelseEllerNærtstående.sammenligning",
            "skib.identifikation",
            "skib.registrering.$variant",
            "skib.registrering.SøblUdenlandskSkibRegistreretIEUEØS.flag",
            "skib.registrering.SøblSkibRegistreretUdenForEUEØS.flag",
            "skib.bruttotonnage",
            "skib.arbejdsgiverstatus",
            "arbejde.anvendelse.$variant",
            "arbejde.anvendelse.SøblUdelukkendeAnvendtTil.aktivitet",
            "arbejde.arbejdsområde",
            "arbejde.passagerrute",
            "arbejde.arbejdsrolle.$variant",
            "arbejde.arbejdsrolle.SøblMidlertidigtVedligeholdEllerReparation.skib_i_almindelig_drift",
            "arbejde.arbejdsrolle.SøblMidlertidigtVedligeholdEllerReparation.del_af_sikkerheds_eller_driftsbesætning",
            "arbejde.arbejdsrolle.SøblMidlertidigtVedligeholdEllerReparation.kan_umiddelbart_udføres_af_landpersonale",
            "arbejde.arbejdsrolle.SøblArbejdePåMidlertidigtUdeAfDriftSkib.periode.startdato.år",
            "arbejde.arbejdsrolle.SøblArbejdePåMidlertidigtUdeAfDriftSkib.periode.startdato.måned",
            "arbejde.arbejdsrolle.SøblArbejdePåMidlertidigtUdeAfDriftSkib.periode.startdato.dag",
            "arbejde.arbejdsrolle.SøblArbejdePåMidlertidigtUdeAfDriftSkib.periode.slutdato.år",
            "arbejde.arbejdsrolle.SøblArbejdePåMidlertidigtUdeAfDriftSkib.periode.slutdato.måned",
            "arbejde.arbejdsrolle.SøblArbejdePåMidlertidigtUdeAfDriftSkib.periode.slutdato.dag",
            "arbejde.arbejdsrolle.SøblArbejdePåMidlertidigtUdeAfDriftSkib.umiddelbart_før_omfattet",
            "arbejde.arbejdsrolle.SøblArbejdePåMidlertidigtUdeAfDriftSkib.kun_ansat_til_arbejde_mens_skibet_er_ude_af_drift",
            "arbejde.arbejdsrolle.SøblNybygningstilsyn.periode.startdato.år",
            "arbejde.arbejdsrolle.SøblNybygningstilsyn.periode.startdato.måned",
            "arbejde.arbejdsrolle.SøblNybygningstilsyn.periode.startdato.dag",
            "arbejde.arbejdsrolle.SøblNybygningstilsyn.periode.slutdato.år",
            "arbejde.arbejdsrolle.SøblNybygningstilsyn.periode.slutdato.måned",
            "arbejde.arbejdsrolle.SøblNybygningstilsyn.periode.slutdato.dag",
            "arbejde.arbejdsrolle.SøblNybygningstilsyn.skibet_opfylder_betingelser_efter_færdiggørelse",
            "arbejde.arbejdsrolle.SøblNybygningstilsyn.påmønstres_umiddelbart_efter_færdiggørelse",
            "arbejde.arbejdsrolle.SøblKursusophold.opgørelse.opgørelsesperiode.startdato.år",
            "arbejde.arbejdsrolle.SøblKursusophold.opgørelse.opgørelsesperiode.startdato.måned",
            "arbejde.arbejdsrolle.SøblKursusophold.opgørelse.opgørelsesperiode.startdato.dag",
            "arbejde.arbejdsrolle.SøblKursusophold.opgørelse.opgørelsesperiode.slutdato.år",
            "arbejde.arbejdsrolle.SøblKursusophold.opgørelse.opgørelsesperiode.slutdato.måned",
            "arbejde.arbejdsrolle.SøblKursusophold.opgørelse.opgørelsesperiode.slutdato.dag",
            "arbejde.arbejdsrolle.SøblKursusophold.umiddelbart_før_omfattet",
            "arbejde.arbejdsrolle.SøblKursusophold.fortsat_ansat_af_rederiet",
            "arbejde.arbejdsrolle.SøblNødvendigeRejsedage.opgørelse.opgørelsesperiode.startdato.år",
            "arbejde.arbejdsrolle.SøblNødvendigeRejsedage.opgørelse.opgørelsesperiode.startdato.måned",
            "arbejde.arbejdsrolle.SøblNødvendigeRejsedage.opgørelse.opgørelsesperiode.startdato.dag",
            "arbejde.arbejdsrolle.SøblNødvendigeRejsedage.opgørelse.opgørelsesperiode.slutdato.år",
            "arbejde.arbejdsrolle.SøblNødvendigeRejsedage.opgørelse.opgørelsesperiode.slutdato.måned",
            "arbejde.arbejdsrolle.SøblNødvendigeRejsedage.opgørelse.opgørelsesperiode.slutdato.dag",
            "arbejde.par8_valg",
            "løn.indkomsttype",
            "løn.løngrundlag",
            "løn.beløb_kroner",
        ] {
            assert!(
                dis_income_paths.iter().any(|path| path == expected),
                "missing canonical SØBL §§ 5-8 source-fact path {expected} on {dis_income_sheet}"
            );
        }
        let dis_income_headers = workbook_headers(&mut workbook, &dis_income_sheet);
        for expected in [
            "Sømandsindkomstens identifikation",
            "Sømandsindkomstens indkomstår",
            "Skattepligtsposition for sømandsindkomsten",
            "Statsborgerskab ved passagersejlads",
            "Relation til skibet eller rederiet",
            "Aflønning som ejer, partreder eller interessent",
            "Aflønning ved bestemmende indflydelse eller nærtstående relation",
            "Skibets identifikation",
            "Skibets registrering",
            "Flag for udenlandsk EU/EØS-skib",
            "Flag for skib registreret uden for EU/EØS",
            "Skibets bruttotonnage",
            "Arbejdsgiverens status efter sømandsbeskatningsloven",
            "Skibets anvendelsesforløb",
            "Skibets udelukkende aktivitet",
            "Arbejdsområde inden for eller uden for EU/EØS",
            "Passagersejladsens rute",
            "Arbejdsrolle om bord",
            "Skibet i almindelig drift under vedligeholdelsen",
            "Del af sikkerheds- eller driftsbesætningen",
            "Arbejdet kunne umiddelbart udføres af landpersonale",
            "Driftsophørets startdato (ISO 8601) - år",
            "Driftsophørets startdato (ISO 8601) - måned",
            "Driftsophørets startdato (ISO 8601) - dag",
            "Driftsophørets slutdato (ISO 8601) - år",
            "Driftsophørets slutdato (ISO 8601) - måned",
            "Driftsophørets slutdato (ISO 8601) - dag",
            "Omfattet umiddelbart før driftsophøret",
            "Kun ansat til arbejde under driftsophøret",
            "Nybygningstilsynets startdato (ISO 8601) - år",
            "Nybygningstilsynets startdato (ISO 8601) - måned",
            "Nybygningstilsynets startdato (ISO 8601) - dag",
            "Nybygningstilsynets slutdato (ISO 8601) - år",
            "Nybygningstilsynets slutdato (ISO 8601) - måned",
            "Nybygningstilsynets slutdato (ISO 8601) - dag",
            "Nybygningen opfylder betingelserne efter færdiggørelse",
            "Påmønstring umiddelbart efter færdiggørelse",
            "Kursusopgørelsens startdato (ISO 8601) - år",
            "Kursusopgørelsens startdato (ISO 8601) - måned",
            "Kursusopgørelsens startdato (ISO 8601) - dag",
            "Kursusopgørelsens slutdato (ISO 8601) - år",
            "Kursusopgørelsens slutdato (ISO 8601) - måned",
            "Kursusopgørelsens slutdato (ISO 8601) - dag",
            "Omfattet umiddelbart før kursusopholdet",
            "Fortsat ansat af rederiet under kurset",
            "Rejsedagsopgørelsens startdato (ISO 8601) - år",
            "Rejsedagsopgørelsens startdato (ISO 8601) - måned",
            "Rejsedagsopgørelsens startdato (ISO 8601) - dag",
            "Rejsedagsopgørelsens slutdato (ISO 8601) - år",
            "Rejsedagsopgørelsens slutdato (ISO 8601) - måned",
            "Rejsedagsopgørelsens slutdato (ISO 8601) - dag",
            "Valg for arbejde uden for EU/EØS efter § 8",
            "Sømandsindkomstens retlige type",
            "Sømandsindkomstens løngrundlag",
            "Løn eller direkte tilknyttet ydelse",
        ] {
            assert!(
                dis_income_headers.iter().any(|header| header == expected),
                "missing human SØBL §§ 5-8 input label {expected} on {dis_income_sheet}"
            );
        }
        let dis_course_period_path = "lønmodtager.personlig_indkomst.sømandsbeskatning.indkomster.arbejde.arbejdsrolle.SøblKursusophold.opgørelse.kursusperioder";
        let dis_course_period_sheet =
            workbook_collection_sheet_name(&mut workbook, dis_course_period_path);
        assert_eq!(
            workbook_title(&mut workbook, &dis_course_period_sheet),
            "Dansk personskat - Kursusperioder i 12-månedersopgørelsen"
        );
        let dis_course_period_paths =
            workbook_column_paths(&mut workbook, &dis_course_period_sheet);
        for expected in [
            "startdato.år",
            "startdato.måned",
            "startdato.dag",
            "slutdato.år",
            "slutdato.måned",
            "slutdato.dag",
        ] {
            assert!(
                dis_course_period_paths
                    .iter()
                    .any(|path| path == expected),
                "missing canonical dated SØBL course-period path {expected} on {dis_course_period_sheet}"
            );
        }
        let dis_course_period_headers = workbook_headers(&mut workbook, &dis_course_period_sheet);
        for expected in [
            "Kursusperiodens startdato (ISO 8601) - år",
            "Kursusperiodens startdato (ISO 8601) - måned",
            "Kursusperiodens startdato (ISO 8601) - dag",
            "Kursusperiodens slutdato (ISO 8601) - år",
            "Kursusperiodens slutdato (ISO 8601) - måned",
            "Kursusperiodens slutdato (ISO 8601) - dag",
        ] {
            assert!(
                dis_course_period_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human dated SØBL course-period label {expected} on {dis_course_period_sheet}"
            );
        }
        let dis_travel_date_path = "lønmodtager.personlig_indkomst.sømandsbeskatning.indkomster.arbejde.arbejdsrolle.SøblNødvendigeRejsedage.opgørelse.rejsedatoer";
        let dis_travel_date_sheet =
            workbook_collection_sheet_name(&mut workbook, dis_travel_date_path);
        assert_eq!(
            workbook_title(&mut workbook, &dis_travel_date_sheet),
            "Dansk personskat - Nødvendige rejsedatoer i 12-månedersopgørelsen"
        );
        let dis_travel_date_paths = workbook_column_paths(&mut workbook, &dis_travel_date_sheet);
        for expected in ["år", "måned", "dag"] {
            assert!(
                dis_travel_date_paths.iter().any(|path| path == expected),
                "missing canonical dated SØBL travel-day path {expected} on {dis_travel_date_sheet}"
            );
        }
        let dis_travel_date_headers = workbook_headers(&mut workbook, &dis_travel_date_sheet);
        for expected in [
            "Nødvendig rejsedato (ISO 8601) - år",
            "Nødvendig rejsedato (ISO 8601) - måned",
            "Nødvendig rejsedato (ISO 8601) - dag",
        ] {
            assert!(
                dis_travel_date_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human dated SØBL travel-day label {expected} on {dis_travel_date_sheet}"
            );
        }
        let dis_annual_ship_path =
            "lønmodtager.personlig_indkomst.sømandsbeskatning.skibsårsdrifter";
        let dis_annual_ship_sheet =
            workbook_collection_sheet_name(&mut workbook, dis_annual_ship_path);
        assert_eq!(
            workbook_title(&mut workbook, &dis_annual_ship_sheet),
            "Dansk personskat - Årsdrift for bugser- og bjærgningsfartøjer"
        );
        let dis_annual_ship_paths = workbook_column_paths(&mut workbook, &dis_annual_ship_sheet);
        for expected in [
            "skibsidentifikation",
            "indkomstår",
            "driftstid.søtransport_minutter",
            "driftstid.mobilisering_til_søs_minutter",
            "driftstid.andre_aktiviteter_minutter",
            "driftstid.ventetid_minutter",
        ] {
            assert!(
                dis_annual_ship_paths.iter().any(|path| path == expected),
                "missing canonical SØBL § 6 annual-vessel path {expected} on {dis_annual_ship_sheet}"
            );
        }
        let dis_annual_ship_headers = workbook_headers(&mut workbook, &dis_annual_ship_sheet);
        for expected in [
            "Skibsidentifikation for årsdriften",
            "Indkomstår for skibets årsdrift",
            "Søtransporttid ved bugsering og bjærgning",
            "Mobiliseringstid til søs ved bugsering og bjærgning",
            "Anden aktivitetstid ved bugsering og bjærgning",
            "Ventetid ved bugsering og bjærgning",
        ] {
            assert!(
                dis_annual_ship_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human SØBL § 6 annual-vessel label {expected} on {dis_annual_ship_sheet}"
            );
        }
        let other_ligningslov7u_path =
            "lønmodtager.personlig_indkomst.sømandsbeskatning.andre_ligningslov7u_indkomster";
        let other_ligningslov7u_sheet =
            workbook_collection_sheet_name(&mut workbook, other_ligningslov7u_path);
        assert_eq!(
            workbook_title(&mut workbook, &other_ligningslov7u_sheet),
            "Dansk personskat - Andre indkomster omfattet af ligningslovens § 7 U"
        );
        let other_ligningslov7u_paths =
            workbook_column_paths(&mut workbook, &other_ligningslov7u_sheet);
        for expected in ["identifikation", "beløb_kroner"] {
            assert!(
                other_ligningslov7u_paths
                    .iter()
                    .any(|path| path == expected),
                "missing canonical other LL § 7 U source-fact path {expected} on {other_ligningslov7u_sheet}"
            );
        }
        let other_ligningslov7u_headers =
            workbook_headers(&mut workbook, &other_ligningslov7u_sheet);
        for expected in [
            "Den anden § 7 U-indkomsts identifikation",
            "Anden indkomst efter ligningslovens § 7 U",
        ] {
            assert!(
                other_ligningslov7u_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human other LL § 7 U input label {expected} on {other_ligningslov7u_sheet}"
            );
        }
        let union_dues_path = "lønmodtager.ligningsfradrag.faglige_kontingenter.kontingenter";
        let union_dues_sheet = workbook_collection_sheet_name(&mut workbook, union_dues_path);
        let union_dues_paths = workbook_column_paths(&mut workbook, &union_dues_sheet);
        for expected in [
            "identifikation",
            "indkomstår",
            "periode.fra_dato.år",
            "periode.fra_dato.måned",
            "periode.fra_dato.dag",
            "periode.til_dato.år",
            "periode.til_dato.måned",
            "periode.til_dato.dag",
            "foreningsart",
            "betalt_kontingent_kroner",
            "foreningens_opgjorte_andel_til_faglige_økonomiske_interesser_kroner",
            "foreningens_hovedformål_er_erhvervsgruppens_økonomiske_interesser",
            "skatteyder_hører_til_erhvervsgruppen",
            "indberetningsstatus",
        ] {
            assert!(
                union_dues_paths.iter().any(|path| path == expected),
                "missing canonical LL § 13 source-fact path {expected} on {union_dues_sheet}"
            );
        }
        let union_dues_headers = workbook_headers(&mut workbook, &union_dues_sheet);
        for expected in [
            "Kontingentets identifikation",
            "Kontingentets indkomstår",
            "Kontingentperiodens startdato (ISO 8601) - år",
            "Kontingentperiodens startdato (ISO 8601) - måned",
            "Kontingentperiodens startdato (ISO 8601) - dag",
            "Kontingentperiodens slutdato (ISO 8601) - år",
            "Kontingentperiodens slutdato (ISO 8601) - måned",
            "Kontingentperiodens slutdato (ISO 8601) - dag",
            "Foreningens art",
            "Betalt fagligt kontingent",
            "Foreningens faglige økonomiske andel",
            "Foreningens hovedformål er erhvervsgruppens økonomiske interesser",
            "Skatteyderen tilhører erhvervsgruppen",
            "Indberetning af fagligt kontingent",
        ] {
            assert!(
                union_dues_headers.iter().any(|header| header == expected),
                "missing human LL § 13 input label {expected} on {union_dues_sheet}"
            );
        }
        let union_period_assessments_path =
            "lønmodtager.ligningsfradrag.fiskerfradrag.kontingentperiodeundtagelser";
        let union_period_assessments_sheet =
            workbook_collection_sheet_name(&mut workbook, union_period_assessments_path);
        let union_period_assessment_paths =
            workbook_column_paths(&mut workbook, &union_period_assessments_sheet);
        for expected in [
            "kontingent_identifikation",
            "bedømmelse.$variant",
            "bedømmelse.Ll9GFørFørsteRegistreringGodkendtEfterKonkretBedømmelse.andet_arbejdsforhold_identifikation",
            "bedømmelse.Ll9GFørFørsteRegistreringGodkendtEfterKonkretBedømmelse.ligningsmæssig_bedømmelsesreference",
            "bedømmelse.Ll9GEfterFuldtOphørGodkendtEfterKonkretBedømmelse.andet_erhverv_identifikation",
            "bedømmelse.Ll9GEfterFuldtOphørGodkendtEfterKonkretBedømmelse.ligningsmæssig_bedømmelsesreference",
        ] {
            assert!(
                union_period_assessment_paths
                    .iter()
                    .any(|path| path == expected),
                "missing canonical LL § 9 G union-period assessment path {expected} on {union_period_assessments_sheet}"
            );
        }
        let union_period_assessment_headers =
            workbook_headers(&mut workbook, &union_period_assessments_sheet);
        for expected in [
            "Kontingentbetaling for den konkrete bedømmelse",
            "Kontingentperiodens overgangstype",
            "Andet arbejde før første fiskerregistrering",
            "Reference for bedømmelsen før første registrering",
            "Andet erhverv efter fuldstændigt ophør med fiskeriet",
            "Reference for bedømmelsen efter fuldstændigt ophør",
        ] {
            assert!(
                union_period_assessment_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human LL § 9 G union-period assessment label {expected} on {union_period_assessments_sheet}"
            );
        }
        let unemployment_contributions_path =
            "lønmodtager.ligningsfradrag.arbejdsløshed_efterløn_og_fleksydelse.bidrag";
        let unemployment_contributions_sheet =
            workbook_collection_sheet_name(&mut workbook, unemployment_contributions_path);
        let unemployment_contribution_paths =
            workbook_column_paths(&mut workbook, &unemployment_contributions_sheet);
        for expected in [
            "identifikation",
            "indkomstår",
            "betalt_beløb_kroner",
            "art.$variant",
            "art.Pbl49Akassekontingent.arbejdsløshedskasse",
            "art.Pbl49Akassekontingent.skatteyderen_er_medlem_og_forsikret",
            "art.Pbl49PrivatArbejdsløshedsforsikring.skatteyderen_er_forsikringsejer",
            "art.Pbl49PrivatArbejdsløshedsforsikring.skatteyderen_er_forsikret",
            "art.Pbl49AEfterlønsbidrag.medlem_af_arbejdsløshedskasse",
            "art.Pbl49BFleksydelsesbidrag.tilmeldt_fleksydelsesordningen_efter_fleksydelseslov4",
        ] {
            assert!(
                unemployment_contribution_paths
                    .iter()
                    .any(|path| path == expected),
                "missing canonical PBL §§ 49-49 B source-fact path {expected} on {unemployment_contributions_sheet}"
            );
        }
        let unemployment_contribution_headers =
            workbook_headers(&mut workbook, &unemployment_contributions_sheet);
        for expected in [
            "Bidragets identifikation",
            "Bidragets indkomstår",
            "Betalt bidrag",
            "Bidragets retlige art",
            "A-kassens hjemsted og anerkendelsesstatus",
            "Skatteyderen er medlem og forsikret i A-kassen",
            "Skatteyderen ejer den private arbejdsløshedsforsikring",
            "Skatteyderen er forsikret af den private arbejdsløshedsforsikring",
            "Medlem af A-kassen ved efterlønsbidrag",
            "Tilmeldt fleksydelsesordningen",
        ] {
            assert!(
                unemployment_contribution_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human PBL §§ 49-49 B input label {expected} on {unemployment_contributions_sheet}"
            );
        }
        let gifts_path = "lønmodtager.ligningsfradrag.gaver.gaver";
        let gifts_sheet = workbook_collection_sheet_name(&mut workbook, gifts_path);
        let gift_paths = workbook_column_paths(&mut workbook, &gifts_sheet);
        for expected in [
            "identifikation",
            "indkomstår",
            "betalt_beløb_kroner",
            "modtager.identifikation",
            "modtager.formål.$variant",
            "modtager.godkendelse.$variant",
            "indberettet_efter_skatteindberetningslov26",
            "art.$variant",
            "art.Ll12BindendeLøbendeYdelse.ydelsesfastsættelse",
            "art.Ll12BindendeLøbendeYdelse.forfalden_årlig_ydelse_efter_aftalen_kroner",
            "art.Ll12BindendeLøbendeYdelse.aftalevarighed.$variant",
            "art.Ll12BindendeLøbendeYdelse.aftalevarighed.Ll12BestemtAftaleperiode.aftaleperiode_år",
            "art.Ll12BindendeLøbendeYdelse.aftalen_kan_ikke_uden_videre_ophæves",
        ] {
            assert!(
                gift_paths.iter().any(|path| path == expected),
                "missing canonical LL §§ 8 A, 8 H and 12 source-fact path {expected} on {gifts_sheet}"
            );
        }
        let gift_headers = workbook_headers(&mut workbook, &gifts_sheet);
        for expected in [
            "Gavens identifikation",
            "Gavens indkomstår",
            "Betalt gave eller løbende ydelse",
            "Gavemodtagerens identifikation",
            "Gavemodtagerens formål",
            "Gavemodtagerens skattemæssige godkendelse",
            "Gaven er indberettet",
            "Gavens retlige art",
            "Den løbende ydelses fastsættelse",
            "Forfalden årlig ydelse efter aftalen",
            "Den bindende aftales varighed",
            "Den tidsbegrænsede aftales løbetid",
            "Aftalen kan ikke uden videre ophæves",
        ] {
            assert!(
                gift_headers.iter().any(|header| header == expected),
                "missing human LL §§ 8 A, 8 H and 12 input label {expected} on {gifts_sheet}"
            );
        }
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
        let overnight_travel_path = "lønmodtager.ligningsfradrag.rejser.rejser";
        let overnight_travel_sheet =
            workbook_collection_sheet_name(&mut workbook, overnight_travel_path);
        let overnight_travel_paths = workbook_column_paths(&mut workbook, &overnight_travel_sheet);
        for expected in [
            "identifikation",
            "indkomstår",
            "startdato.år",
            "startdato.måned",
            "startdato.dag",
            "rejseart",
            "arbejdssted_identifikation",
            "arbejdsstedskarakter.$variant",
            "overnatningsforhold.$variant",
            "hverv",
            "varighed_minutter",
            "kost.dækning.$variant",
            "kost.godtgørelsesudbetaling.$variant",
            "kost.godtgørelsesudbetaling.Ll9AUopdeltGodtgørelse.udbetalt_kroner",
            "kost.godtgørelsesudbetaling.Ll9AEndeligtOpdeltGodtgørelse.godtgørelse_efter_sats_kroner",
            "kost.godtgørelsesudbetaling.Ll9AEndeligtOpdeltGodtgørelse.supplerende_løn_kroner",
            "kost.fradragsprincip",
            "kontrol",
            "lønomlægning",
            "indkomstforhold.$variant",
        ] {
            assert!(
                overnight_travel_paths.iter().any(|path| path == expected),
                "missing canonical LL § 9 A source-fact path {expected} on {overnight_travel_sheet}"
            );
        }
        let overnight_travel_headers = workbook_headers(&mut workbook, &overnight_travel_sheet);
        for expected in [
            "Rejsens identifikation",
            "Rejsens indkomstår",
            "Rejsens startdato - år",
            "Rejsens startdato - måned",
            "Rejsens startdato - dag",
            "Rejsens art",
            "Arbejdsstedets identifikation",
            "Arbejdsstedets karakter",
            "Mulighed for at overnatte hjemme",
            "Hverv under rejsen",
            "Rejsens samlede varighed",
            "Arbejdsgiverens dækningsprincip for kost",
            "Afregning af kostgodtgørelsen",
            "Uopdelt kostgodtgørelse",
            "Kostgodtgørelse opgjort efter sats",
            "Supplerende løn ved kostafregningen",
            "Fradragsprincip for rejsens kost",
            "Arbejdsgiverens kontrol af rejseafregningen",
            "Godtgørelse og lønomlægning",
            "Arbejdsindkomstens danske skatteforhold",
        ] {
            assert!(
                overnight_travel_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human LL § 9 A input label {expected} on {overnight_travel_sheet}"
            );
        }
        let prior_period_travel_path =
            "lønmodtager.ligningsfradrag.rejser.arbejdshistorik.tidligere_rejser";
        let prior_period_travel_sheet =
            workbook_collection_sheet_name(&mut workbook, prior_period_travel_path);
        let prior_period_travel_paths =
            workbook_column_paths(&mut workbook, &prior_period_travel_sheet);
        for expected in [
            "identifikation",
            "startdato.år",
            "startdato.måned",
            "startdato.dag",
            "arbejdssted_identifikation",
            "rejseart",
            "varighed_minutter",
        ] {
            assert!(
                prior_period_travel_paths
                    .iter()
                    .any(|path| path == expected),
                "missing canonical LL § 9 A prior-period path {expected} on {prior_period_travel_sheet}"
            );
        }
        let prior_period_travel_headers =
            workbook_headers(&mut workbook, &prior_period_travel_sheet);
        for expected in [
            "Tidligere rejses identifikation",
            "Tidligere rejses startdato - år",
            "Tidligere rejses startdato - måned",
            "Tidligere rejses startdato - dag",
            "Tidligere rejses arbejdssted",
            "Tidligere rejses art",
            "Tidligere rejses samlede varighed",
        ] {
            assert!(
                prior_period_travel_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human LL § 9 A prior-period label {expected} on {prior_period_travel_sheet}"
            );
        }
        let workday_path = "lønmodtager.ligningsfradrag.rejser.arbejdshistorik.arbejdsdage";
        let workday_sheet = workbook_collection_sheet_name(&mut workbook, workday_path);
        let workday_paths = workbook_column_paths(&mut workbook, &workday_sheet);
        for expected in [
            "dato.år",
            "dato.måned",
            "dato.dag",
            "sted.$variant",
            "sted.Ll9AArbejdePåReeltArbejdssted.arbejdssted_identifikation",
        ] {
            assert!(
                workday_paths.iter().any(|path| path == expected),
                "missing canonical LL § 9 A workday path {expected} on {workday_sheet}"
            );
        }
        let workday_headers = workbook_headers(&mut workbook, &workday_sheet);
        for expected in [
            "Arbejdsdagens dato - år",
            "Arbejdsdagens dato - måned",
            "Arbejdsdagens dato - dag",
            "Arbejdsdagens stedstype",
            "Arbejdsdagens reelle arbejdssted",
        ] {
            assert!(
                workday_headers.iter().any(|header| header == expected),
                "missing human LL § 9 A workday label {expected} on {workday_sheet}"
            );
        }
        let workplace_distance_path =
            "lønmodtager.ligningsfradrag.rejser.arbejdshistorik.arbejdsstedsafstande";
        let workplace_distance_sheet =
            workbook_collection_sheet_name(&mut workbook, workplace_distance_path);
        let workplace_distance_paths =
            workbook_column_paths(&mut workbook, &workplace_distance_sheet);
        for expected in [
            "fra_arbejdssted_identifikation",
            "til_arbejdssted_identifikation",
            "gældende_fra.år",
            "gældende_fra.måned",
            "gældende_fra.dag",
            "afstand_ad_normal_transportvej_kilometer",
        ] {
            assert!(
                workplace_distance_paths
                    .iter()
                    .any(|path| path == expected),
                "missing canonical LL § 9 A workplace-distance path {expected} on {workplace_distance_sheet}"
            );
        }
        let workplace_distance_headers = workbook_headers(&mut workbook, &workplace_distance_sheet);
        for expected in [
            "Afstand fra arbejdssted",
            "Afstand til arbejdssted",
            "Afstand gældende fra - år",
            "Afstand gældende fra - måned",
            "Afstand gældende fra - dag",
            "Afstand ad normal transportvej",
        ] {
            assert!(
                workplace_distance_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human LL § 9 A workplace-distance label {expected} on {workplace_distance_sheet}"
            );
        }
        let lodging_day_path = format!("{overnight_travel_path}.logidøgn");
        let lodging_day_sheet = workbook_collection_sheet_name(&mut workbook, &lodging_day_path);
        let lodging_day_paths = workbook_column_paths(&mut workbook, &lodging_day_sheet);
        for expected in [
            "rejsedøgnsnummer",
            "dækning.$variant",
            "dækning.Ll9ALogiHeltEllerDelvistDækketEfterRegning.arbejdsgiverbetalt_kroner",
            "godtgørelsesudbetaling.$variant",
            "godtgørelsesudbetaling.Ll9AUopdeltGodtgørelse.udbetalt_kroner",
            "godtgørelsesudbetaling.Ll9AEndeligtOpdeltGodtgørelse.godtgørelse_efter_sats_kroner",
            "godtgørelsesudbetaling.Ll9AEndeligtOpdeltGodtgørelse.supplerende_løn_kroner",
            "dokumenteret_logiudgift_betalt_før_refusion_kroner",
            "fradragsprincip",
        ] {
            assert!(
                lodging_day_paths.iter().any(|path| path == expected),
                "missing canonical LL § 9 A lodging-day path {expected} on {lodging_day_sheet}"
            );
        }
        let lodging_day_headers = workbook_headers(&mut workbook, &lodging_day_sheet);
        for expected in [
            "Rejsedøgnets nummer",
            "Arbejdsgiverens dækning af logidøgnet",
            "Logi dækket efter regning dette døgn",
            "Afregning af logigodtgørelsen dette døgn",
            "Uopdelt logigodtgørelse dette døgn",
            "Logigodtgørelse opgjort efter sats dette døgn",
            "Supplerende løn ved logiafregningen dette døgn",
            "Dokumenteret logiudgift før refusion dette døgn",
            "Fradragsprincip for logidøgnet",
        ] {
            assert!(
                lodging_day_headers.iter().any(|header| header == expected),
                "missing human LL § 9 A lodging-day label {expected} on {lodging_day_sheet}"
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
            "par13a_kildefakta.AblPar13AUdbytteForAktieOmfattetAfPar44.aktie_identifikation",
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
            "§ 44-aktie bag udbyttet",
        ] {
            assert!(
                dividend_headers.iter().any(|header| header == expected),
                "missing human dividend input label {expected} on {dividend_sheet}"
            );
        }
        let par44_holdings_path = "aktieavance.udbytter.par13a_kildefakta.AblPar13AUdbytteForAktieOmfattetAfPar44.beholdning.aktier";
        let par44_holdings_sheet =
            workbook_collection_sheet_name(&mut workbook, par44_holdings_path);
        let par44_holding_paths = workbook_column_paths(&mut workbook, &par44_holdings_sheet);
        for expected in [
            "identifikation",
            "selskabsidentifikation",
            "indkomstår",
            "kapitalmængde.$variant",
            "kapitalmængde.AblAktiekapitalUdenPålydendeVærdi.antal_aktier",
            "erhvervelsesgrundlag.$variant",
            "erhvervelsesgrundlag.AblPar44AktieErhvervetFør2006.kildegrundlag.erhvervelsesdato.år",
            "erhvervelsesgrundlag.AblPar44AktieErhvervetFør2006.kildegrundlag.børsstatus_pr_31_december_2005",
            "erhvervelsesgrundlag.AblPar44AktieErhvervetFør2006.kildegrundlag.beholdningsfakta.egen_kursværdi_pr_31_december_2005_kroner",
            "erhvervelsesgrundlag.AblPar44AktieErhvervetFør2006.kildegrundlag.historisk_undtagelsesstatus",
            "erhvervelsesgrundlag.AblPar44FondsaktieTildeltPåGrundlagAfPar44Aktie.grundaktiens_identifikation",
            "erhvervelsesgrundlag.AblPar44FondsaktieTildeltPåGrundlagAfPar44Aktie.tildelingsdato.år",
            "statusforløb.$variant",
            "statusforløb.AblPar44StatusændretTilIkkeReguleretMarked.statusændringsdato.år",
            "statusforløb.AblPar44StatusændretTilIkkeReguleretMarked.kursværdi_på_statusændringstidspunktet_kroner",
        ] {
            assert!(
                par44_holding_paths.iter().any(|path| path == expected),
                "missing canonical ABL § 44 holding path {expected} on {par44_holdings_sheet}"
            );
        }
        let par44_holding_headers = workbook_headers(&mut workbook, &par44_holdings_sheet);
        for expected in [
            "Den historiske akties identifikation",
            "Selskabet bag § 44-aktien",
            "Indkomstår for § 44-vurderingen",
            "Kapitalmængde for § 44-aktien",
            "Erhvervelsesgrundlag efter ABL § 44",
            "Aktiens erhvervelsesår",
            "Børsstatus den 31. december 2005",
            "Egen børsnoteret beholdning den 31. december 2005",
            "Historisk undtagelse efter § 2 c eller § 2 e",
            "Fondsaktiens grundaktie",
            "Fondsaktiens tildelingsår",
            "§ 44-aktiens statusforløb",
            "Statusændringens år",
            "Kursværdi ved § 44-statusændringen",
        ] {
            assert!(
                par44_holding_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human ABL § 44 holding label {expected} on {par44_holdings_sheet}"
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
        let spouse_property_tax_path = "ægtefælle.MedÆgtefælle.fakta.ejendomsskatter.ejendomme";
        let spouse_property_tax_sheet =
            workbook_collection_sheet_name(&mut workbook, spouse_property_tax_path);
        assert_eq!(
            workbook_title(&mut workbook, &spouse_property_tax_sheet),
            "Dansk personskat - Ejendomme med ejendomsskatter"
        );
        assert_eq!(
            workbook_column_paths(&mut workbook, &spouse_property_tax_sheet),
            property_tax_paths,
            "spouse property table must expose the same typed source facts"
        );
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
            "Ejendomsandelens identifikation",
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
        let property_tax_intervals_path = "ejendomsskatter.ejendomme.ordinært_grundlag.ejendomsværdiskatteperiode.EjendomsskatIIntervaller.intervaller";
        let property_tax_intervals_sheet =
            workbook_collection_sheet_name(&mut workbook, property_tax_intervals_path);
        assert_eq!(
            workbook_column_paths(&mut workbook, &property_tax_intervals_sheet),
            [
                "fra_dato.år",
                "fra_dato.måned",
                "fra_dato.dag",
                "til_dato.år",
                "til_dato.måned",
                "til_dato.dag",
            ]
        );
        let property_tax_interval_headers =
            workbook_headers(&mut workbook, &property_tax_intervals_sheet);
        for expected in [
            "Startår for boligperiode",
            "Startmåned for boligperiode",
            "Startdag for boligperiode",
            "Slutår for boligperiode",
            "Slutmåned for boligperiode",
            "Slutdag for boligperiode",
        ] {
            assert!(
                property_tax_interval_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human property-tax interval label {expected} on {property_tax_intervals_sheet}"
            );
        }
        assert_eq!(
            workbook_headers(&mut workbook, &spouse_property_tax_sheet),
            property_tax_headers,
            "spouse property table must reuse the human property-tax labels"
        );
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
            "lønmodtager.ligningsfradrag.befordring.forhold.identifikation",
            "lønmodtager.ligningsfradrag.befordring.forhold.befordringsmål_identifikation",
            "lønmodtager.ligningsfradrag.befordring.forhold.arbejdsdage",
            "lønmodtager.ligningsfradrag.befordring.forhold.arbejdsgiverbetalt_befordring",
            "lønmodtager.ligningsfradrag.befordring.forhold.ligningslov9d.$variant",
            "lønmodtager.ligningsfradrag.befordring.forhold.ligningslov9d.MedLigningslov9D.input.faktisk_udgiftsgrundlag.Ll9dDokumenteredeFaktiskeBefordringsudgifter.beløb_kroner",
            "lønmodtager.erhvervsbefordring.sager.identifikation",
            "lønmodtager.erhvervsbefordring.sager.rækkefølge_i_indkomståret",
            "lønmodtager.erhvervsbefordring.sager.godtgørende_arbejdsgiver_identifikation",
            "lønmodtager.erhvervsbefordring.sager.køretøj",
            "lønmodtager.erhvervsbefordring.sager.befordring.kilometer_i_sagen",
            "lønmodtager.erhvervsbefordring.sager.godtgørelsesforhold.udbetalt_godtgørelse_kroner",
            "lønmodtager.pension.fødselsdato.år",
            "lønmodtager.pension.fødselsdato.måned",
            "lønmodtager.pension.fødselsdato.dag",
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
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.par25_valg.obligationer_på_reguleret_marked.position_primo.$variant",
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.par25_valg.obligationer_på_reguleret_marked.aktuelt_princip",
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.par25_valg.obligationer_på_reguleret_marked.ændringstilladelse.$variant",
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.par25_valg.valutakursændringer.position_primo.$variant",
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.par25_valg.valutakursændringer.aktuelt_princip",
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.par25_valg.valutakursændringer.ændringstilladelse.$variant",
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.fordringer.identifikation",
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.fordringer.kilde.fordringsart",
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.fordringer.position_primo.$variant",
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.fordringer.hændelser.$variant",
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.valutainstrumenter.identifikation",
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.valutainstrumenter.valuta.iso_4217_kode",
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.valutainstrumenter.aktiv.$variant",
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.valutainstrumenter.position_primo.$variant",
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.valutainstrumenter.årsændring.$variant",
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
            "underskudsforhold.FremførtUnderskudFraForrigePersonskatÅr.resultat.indkomstår",
            "underskudsforhold.FremførtUnderskudFraForrigePersonskatÅr.resultat.åbningsgrundlag_gyldigt",
            "underskudsforhold.FremførtUnderskudFraForrigePersonskatÅr.resultat.fremført_underskud_ultimo_kroner",
            "underskudsforhold.EksterntFastsatFremførtUnderskud.fra_indkomstår",
            "underskudsforhold.EksterntFastsatFremførtUnderskud.underskud_kroner",
            "underskudsforhold.EksterntFastsatFremførtUnderskud.proveniens.$variant",
            "underskudsforhold.EksterntFastsatFremførtUnderskud.proveniens.SkatteforvaltningensÅrsopgørelse.dokumentreference",
            "underskudsforhold.EksterntFastsatFremførtUnderskud.proveniens.SkatteforvaltningensAfgørelse.dokumentreference",
            "underskudsforhold.EksterntFastsatFremførtUnderskud.proveniens.AndenMyndighedsafgørelse.myndighed",
            "underskudsforhold.EksterntFastsatFremførtUnderskud.proveniens.AndenMyndighedsafgørelse.dokumentreference",
            "negativ_aktieskat_fremførsel.hovedperson.$variant",
            "negativ_aktieskat_fremførsel.hovedperson.FremførtNegativAktieskatFraForrigePersonskatÅr.resultat.indkomstår",
            "negativ_aktieskat_fremførsel.hovedperson.EksterntFastsatFremførtNegativAktieskat.proveniens.$variant",
            "negativ_aktieskat_fremførsel.hovedperson.EksterntFastsatFremførtNegativAktieskat.proveniens.SkatteforvaltningensÅrsopgørelse.dokumentreference",
            "negativ_aktieskat_fremførsel.hovedperson.EksterntFastsatFremførtNegativAktieskat.trancher.ejer",
            "negativ_aktieskat_fremførsel.hovedperson.EksterntFastsatFremførtNegativAktieskat.trancher.oprindelsesår",
            "negativ_aktieskat_fremførsel.hovedperson.EksterntFastsatFremførtNegativAktieskat.trancher.resterende_negativ_skat_kroner",
            "negativ_aktieskat_fremførsel.ægtefælle.$variant",
            "ligningslov33.hovedperson.$variant",
            "ligningslov33.hovedperson.MedLigningslov33.input.indkomstår",
            "ligningslov33.hovedperson.MedLigningslov33.input.ikke_henførbare_udgifter.beløb_kroner",
            "ligningslov33.hovedperson.MedLigningslov33.input.kreditgrupper.område.$variant",
            "ligningslov33.hovedperson.MedLigningslov33.input.kreditgrupper.indkomstposter.art",
            "ligningslov33.hovedperson.MedLigningslov33.input.kreditgrupper.indkomstposter.beløb_kroner",
            "ligningslov33.hovedperson.MedLigningslov33.input.kreditgrupper.indkomstposter.indkomstkategorier",
            "ligningslov33.hovedperson.MedLigningslov33.input.kreditgrupper.skattebetalinger.skatteart",
            "ligningslov33.hovedperson.MedLigningslov33.input.fragtskat_åbningssaldi.område.$variant",
            "ligningslov33.hovedperson.MedLigningslov33.input.fragtskat_åbningssaldi.område.Ll33FremmedStat.landekode",
            "ligningslov33.hovedperson.MedLigningslov33.input.fragtskat_åbningssaldi.indkomstår",
            "ligningslov33.hovedperson.MedLigningslov33.input.fragtskat_åbningssaldi.saldo_øre",
            "ligningslov33.hovedperson.MedLigningslov33.input.fragtskat_åbningssaldi.dokumentreferencer",
            "ligningslov33.ligningslov33a_hovedperson.$variant",
            "ligningslov33.ligningslov33a_ægtefælle.$variant",
            "ægtefælle.$variant",
            "ægtefælle.MedÆgtefælle.fakta.lønmodtager.bruttoløn_kroner",
            "ægtefælle.MedÆgtefælle.fakta.kapitalindkomst.renter.renteudgifter_kroner",
            "ægtefælle.MedÆgtefælle.fakta.ejendomsskatter.person.ejer_folkepensionsalder.$variant",
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
            .any(|path| path
                .contains("aktuelt_underskud_ikke_rummet_i_tidligere_indkomst_eller_skat")));
        assert!(!canonical_input_paths
            .iter()
            .any(|path| path.contains("MedUnderskudshistorik")));
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
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6C.fakta.par6a_fakta.oprindelig_fortjeneste.erhvervsfortjeneste_før_par6_stk2_kroner",
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6C.fakta.par6a_fakta.investering.erhvervsmæssigt_anskaffelsesgrundlag_kroner",
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6C.fakta.arbejdsart",
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6C.fakta.arbejde_påbegyndt",
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
                "status.$variant",
                "status.EblPar5UdgiftMedGenanbragtFortjenesteEfterPar6C.nedslag_for_afstået_del_kroner",
            ] {
                assert!(
                    expense_paths.iter().any(|path| path == expected),
                    "missing canonical § 5 expense path {expected} on {expense_sheet}"
                );
            }
            let expense_headers = workbook_headers(&mut workbook, &expense_sheet);
            assert!(
                expense_headers
                    .iter()
                    .any(|header| header == "Genanbragt fortjeneste efter § 6 C"),
                "missing human § 6 C expense label on {expense_sheet}"
            );

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
            "fordringsgruppe.$variant",
            "fordringsgruppe.KglÅrsnettoEnkeltfordring.mængdeenhed",
            "fordringsgruppe.KglÅrsnettoFondskode.fondskode",
            "fordringsgruppe.KglÅrsnettoFondskode.mængdeenhed",
            "fordringsgruppe.KglÅrsnettoSammeVilkår.udsteder_identifikation",
            "fordringsgruppe.KglÅrsnettoSammeVilkår.vilkårsidentifikation",
            "position_primo.$variant",
            "position_primo.KglÅrsnettoVidereførtPositionPrimo.fra_indkomstår",
            "position_primo.KglÅrsnettoVidereførtPositionPrimo.skattemæssig_værdi_kroner",
            "position_primo.KglÅrsnettoVidereførtPositionPrimo.opgørelsesprincip",
            "position_primo.KglÅrsnettoVidereførtMængdepositionPrimo.fra_indkomstår",
            "position_primo.KglÅrsnettoVidereførtMængdepositionPrimo.opgørelsesprincip",
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
            "Fordringer, som skal opgøres samlet",
            "Fordringens fondskode",
            "Mængdeenhed for fondskoden",
            "Udsteder for fordringer med samme vilkår",
            "Fælles vilkårsidentifikation",
            "Fordringens position ved årets begyndelse",
            "Indkomstår for de videreførte trancher",
            "Skattemæssig værdi ved årets begyndelse",
            "Opgørelsesprincip for den videreførte åbningsværdi",
            "Opgørelsesprincip for de videreførte trancher",
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
            "KglÅrsnettoMængdeanskaffelse.tidspunkt.dato.år",
            "KglÅrsnettoMængdeanskaffelse.tidspunkt.rækkefølge_på_dagen",
            "KglÅrsnettoMængdeanskaffelse.tranche_identifikation",
            "KglÅrsnettoMængdeanskaffelse.mængde",
            "KglÅrsnettoMængdeanskaffelse.anskaffelsessum_kroner",
            "KglÅrsnettoMængdeafståelse.tidspunkt.dato.år",
            "KglÅrsnettoMængdeafståelse.afståelsesart",
            "KglÅrsnettoMængdeafståelse.mængde",
            "KglÅrsnettoMængdeafståelse.afståelsessum_kroner",
            "KglÅrsnettoMængdeultimoværdi.tidspunkt.dato.år",
            "KglÅrsnettoMængdeultimoværdi.værdi_kroner",
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
            "År for hændelsen eller trancheanskaffelsen",
            "Hændelsens rækkefølge på dagen",
            "Anskaffelsestranchens identifikation",
            "Anskaffet mængde",
            "Anskaffelsestranchens anskaffelsessum",
            "Hvordan fordringsmængden blev realiseret",
            "Afstået eller indfriet mængde",
            "Beløb ved afståelse eller indfrielse",
            "Resterende positions værdi ved årets udgang",
        ] {
            assert!(
                kgl_annual_claim_event_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human annual KGL event label {expected} on {kgl_annual_claim_events_sheet}"
            );
        }
        let kgl_annual_opening_tranches_path = format!(
            "{kgl_annual_claim_path}.position_primo.KglÅrsnettoVidereførtMængdepositionPrimo.trancher"
        );
        let kgl_annual_opening_tranches_sheet =
            workbook_collection_sheet_name(&mut workbook, &kgl_annual_opening_tranches_path);
        let kgl_annual_opening_tranche_paths =
            workbook_column_paths(&mut workbook, &kgl_annual_opening_tranches_sheet);
        for expected in [
            "identifikation",
            "anskaffelsestidspunkt.dato.år",
            "anskaffelsestidspunkt.dato.måned",
            "anskaffelsestidspunkt.dato.dag",
            "anskaffelsestidspunkt.rækkefølge_på_dagen",
            "resterende_mængde",
            "resterende_anskaffelsessum_kroner",
        ] {
            assert!(
                kgl_annual_opening_tranche_paths
                    .iter()
                    .any(|path| path == expected),
                "missing annual KGL opening-tranche path {expected} on {kgl_annual_opening_tranches_sheet}"
            );
        }
        let kgl_annual_opening_tranche_headers =
            workbook_headers(&mut workbook, &kgl_annual_opening_tranches_sheet);
        for expected in [
            "Den videreførte tranches identifikation",
            "År for hændelsen eller trancheanskaffelsen",
            "Måned for hændelsen eller trancheanskaffelsen",
            "Dag for hændelsen eller trancheanskaffelsen",
            "Hændelsens rækkefølge på dagen",
            "Tranchens resterende mængde",
            "Tranchens resterende anskaffelsessum",
        ] {
            assert!(
                kgl_annual_opening_tranche_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human annual KGL opening-tranche label {expected} on {kgl_annual_opening_tranches_sheet}"
            );
        }
        let kgl_currency_path = "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.valutainstrumenter";
        let kgl_currency_sheet = workbook_collection_sheet_name(&mut workbook, kgl_currency_path);
        let kgl_currency_paths = workbook_column_paths(&mut workbook, &kgl_currency_sheet);
        for expected in [
            "identifikation",
            "valuta.iso_4217_kode",
            "aktiv.$variant",
            "aktiv.KglÅrsnettoValutafordring.kilde.fordringsart",
            "aktiv.KglÅrsnettoValutafordring.kilde.debitorrelation",
            "position_primo.$variant",
            "position_primo.KglÅrsnettoVidereførtValutapositionPrimo.position.par25_valgpositioner.obligationer_på_reguleret_marked.indkomstår",
            "position_primo.KglÅrsnettoVidereførtValutapositionPrimo.position.par25_valgpositioner.obligationer_på_reguleret_marked.princip",
            "position_primo.KglÅrsnettoVidereførtValutapositionPrimo.position.par25_valgpositioner.valutakursændringer.indkomstår",
            "position_primo.KglÅrsnettoVidereførtValutapositionPrimo.position.par25_valgpositioner.valutakursændringer.princip",
            "årsændring.$variant",
            "årsændring.KglÅrsnettoValutaAnskaffetOgAfstået.anskaffelsesværdi.beløb_hundreddele",
            "årsændring.KglÅrsnettoValutaAnskaffetOgAfstået.anskaffelsesværdi.kurs.dkk_øre_tæller",
            "årsændring.KglÅrsnettoValutaAnskaffetOgAfstået.anskaffelsesværdi.kurs.valuta_hundreddele_nævner",
            "årsændring.KglÅrsnettoValutaAnskaffetOgAfstået.afståelsesværdi.beløb_hundreddele",
            "årsændring.KglÅrsnettoValutaAnskaffetOgAfstået.afståelsesværdi.kurs.dkk_øre_tæller",
            "årsændring.KglÅrsnettoValutaAnskaffetOgAfstået.afståelsesværdi.kurs.valuta_hundreddele_nævner",
        ] {
            assert!(
                kgl_currency_paths.iter().any(|path| path == expected),
                "missing foreign-currency KGL source path {expected} on {kgl_currency_sheet}"
            );
        }
        for forbidden in ["kredit_eller_pris", "valutakurs", "årets_rå_netto_kroner"] {
            assert!(
                !kgl_currency_paths.iter().any(|path| path == forbidden),
                "derived foreign-currency KGL field {forbidden} leaked into {kgl_currency_sheet}"
            );
        }
        let kgl_currency_headers = workbook_headers(&mut workbook, &kgl_currency_sheet);
        for expected in [
            "Valutainstrumentets identifikation",
            "Valutaens ISO 4217-kode",
            "Valutainstrumentets art",
            "Valutafordringens art",
            "Valutafordringens relation til debitor",
            "Valutainstrumentets position ved årets begyndelse",
            "År for positionens tidligere obligationsvalg",
            "Positionens tidligere obligationsprincip",
            "År for positionens tidligere valutavalg",
            "Positionens tidligere valutaprincip",
            "Valutainstrumentets ændring i året",
            "Afstået positions valutamængde ved anskaffelsen",
            "Afstået positions anskaffelseskurs i DKK-øre",
            "Afstået positions anskaffelseskurs i valutahundreddele",
            "Valutamængden ved afståelsen",
            "Afståelseskursen i DKK-øre",
            "Afståelseskursen i valutahundreddele",
        ] {
            assert!(
                kgl_currency_headers.iter().any(|header| header == expected),
                "missing human foreign-currency KGL label {expected} on {kgl_currency_sheet}"
            );
        }
        let kgl_abl22_path = "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.obligationsbaserede_minimumsbeviser";
        let kgl_abl22_sheet = workbook_collection_sheet_name(&mut workbook, kgl_abl22_path);
        let kgl_abl22_headers = workbook_headers(&mut workbook, &kgl_abl22_sheet);
        for expected in [
            "Minimumsbevisets identifikation",
            "Minimumsbevisets position ved årets begyndelse",
            "Minimumsbevisets anskaffelsessum fra tidligere år",
            "Opgørelsesprincip for minimumsbevisets åbningsværdi",
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
        let kgl_par32_case_paths = workbook_column_paths(&mut workbook, "cases");
        let kgl_par32_distribution_path = "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.par32_kontraktforløb.MedPar32Kontraktforløb.aktuelt_år.valg.aktiemodregningsfordeling.$variant";
        assert!(
            kgl_par32_case_paths
                .iter()
                .any(|path| path == kgl_par32_distribution_path),
            "missing current-year KGL §32 allocation choice"
        );
        assert!(
            case_headers
                .iter()
                .any(|header| header == "Fordeling af kontrakttab på aktiegevinster"),
            "missing human KGL §32 allocation label"
        );
        let kgl_par32_allocation_sources_path = "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.par32_kontraktforløb.MedPar32Kontraktforløb.aktuelt_år.valg.aktiemodregningsfordeling.KglPar32FordelEfterKilder.kilder";
        let kgl_par32_allocation_sources_sheet =
            workbook_collection_sheet_name(&mut workbook, kgl_par32_allocation_sources_path);
        assert!(
            workbook_title(&mut workbook, &kgl_par32_allocation_sources_sheet)
                .contains("Kilder til aktiemodregning i valgt rækkefølge"),
            "missing human KGL §32 allocation-source table title"
        );
        let kgl_par32_allocation_source_paths =
            workbook_column_paths(&mut workbook, &kgl_par32_allocation_sources_sheet);
        for expected in [
            "$variant",
            "KglPar32SupplerendeAblKilde.kildeidentifikation",
        ] {
            assert!(
                kgl_par32_allocation_source_paths
                    .iter()
                    .any(|path| path == expected),
                "missing KGL §32 allocation source path {expected} on {kgl_par32_allocation_sources_sheet}"
            );
        }
        let kgl_par32_allocation_source_headers =
            workbook_headers(&mut workbook, &kgl_par32_allocation_sources_sheet);
        for expected in ["Aktiegevinstkildens art", "ABL-kilde til aktiemodregning"] {
            assert!(
                kgl_par32_allocation_source_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human KGL §32 allocation source label {expected} on {kgl_par32_allocation_sources_sheet}"
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
            "fakta.valg.aktiemodregningsfordeling.$variant",
            "årsgrundlag.ejendomsavance.$variant",
            "årsgrundlag.øvrige_instrumenter.par25_valg.obligationer_på_reguleret_marked.position_primo.$variant",
            "årsgrundlag.øvrige_instrumenter.par25_valg.valutakursændringer.position_primo.$variant",
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
        let kgl_par32_history_debt_path =
            format!("{kgl_par32_history_path}.årsgrundlag.gældsposter");
        let kgl_par32_history_debt_sheet =
            workbook_collection_sheet_name(&mut workbook, &kgl_par32_history_debt_path);
        let kgl_par32_history_debt_paths =
            workbook_column_paths(&mut workbook, &kgl_par32_history_debt_sheet);
        for expected in [
            "identifikation",
            "beløb.gældens_værdi_ved_påtagelse_kroner",
            "beløb.gældens_værdi_ved_frigørelse_eller_indfrielse_kroner",
            "valuta",
            "gældsordning.$variant",
        ] {
            assert!(
                kgl_par32_history_debt_paths
                    .iter()
                    .any(|path| path == expected),
                "missing historical KGL §32 debt path {expected} on {kgl_par32_history_debt_sheet}"
            );
        }
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
            "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.fraflytningsdato.år",
            "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.fraflytningsdato.måned",
            "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.fraflytningsdato.dag",
            "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.ophørsgrund",
            "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.kontekstgrundlag.$variant",
            "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.kildehistorik.opgjort_pr.år",
            "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.kildehistorik.opgjort_pr.måned",
            "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.kildehistorik.opgjort_pr.dag",
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
            "Fraflytningsdatoens år",
            "Fraflytningsdatoens måned",
            "Fraflytningsdatoens dag",
            "Grund til ophør af dansk beskatningsret",
            "Grundlag for fraflytningsårets aktieindkomstkontekst",
            "Kildehistorikkens opgørelsesdato, år",
            "Kildehistorikkens opgørelsesdato, måned",
            "Kildehistorikkens opgørelsesdato, dag",
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
        let par37_extensions_path = "aktieavance.særlige_aktiver.kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.kildehistorik.fristudsættelser";
        let par37_extensions_sheet =
            workbook_collection_sheet_name(&mut workbook, par37_extensions_path);
        assert_eq!(
            workbook_title(&mut workbook, &par37_extensions_sheet),
            "Dansk personskat - Udsatte frister for årlige oplysninger"
        );
        let par37_extension_paths = workbook_column_paths(&mut workbook, &par37_extensions_sheet);
        for expected in [
            "indkomstår",
            "udsat_oplysningsfrist.år",
            "udsat_oplysningsfrist.måned",
            "udsat_oplysningsfrist.dag",
        ] {
            assert!(
                par37_extension_paths.iter().any(|path| path == expected),
                "missing § 39 A deadline-extension path {expected} on {par37_extensions_sheet}"
            );
        }
        let par37_extension_headers = workbook_headers(&mut workbook, &par37_extensions_sheet);
        for expected in [
            "Indkomstår for den udsatte oplysningsfrist",
            "Den udsatte oplysningsfrists år",
            "Den udsatte oplysningsfrists måned",
            "Den udsatte oplysningsfrists dag",
        ] {
            assert!(
                par37_extension_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human § 39 A deadline-extension label {expected} on {par37_extensions_sheet}"
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
                "årsopgørelse.MedEksaktÅrsopgørelse.afregningsfakta.$variant",
                "Afregning af årsopgørelsen",
                "Skal årsopgørelsen afregnes",
            ),
            (
                "årsopgørelse.MedEksaktÅrsopgørelse.afregningsfakta.AfregnOverskydendeSkat.fakta.restancer_personlig_skat_med_morarenter_øre",
                "Restancer til modregning",
                "Hvor store restancer",
            ),
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
                "lønmodtager.ligningsfradrag.befordring.forhold.arbejdsgiverbetalt_befordring",
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
                        "lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.$variant",
                        Data::String("Søbl5IntetDødsboskattegrundlag".to_string()),
                    ),
                    (
                        "lønmodtager.personlig_indkomst.sømandsbeskatning.kulbrinteskattegrundlag.$variant",
                        Data::String("Søbl5BIntetKulbrinteskattegrundlag".to_string()),
                    ),
                    (
                        "lønmodtager.ligningsfradrag.sømandsfradrag.valg",
                        Data::String("FravælgSømandsfradrag".to_string()),
                    ),
                    (
                        "lønmodtager.ligningsfradrag.fiskerfradrag.valg",
                        Data::String("Ll9GFravælgFiskerfradrag".to_string()),
                    ),
                    (
                        "lønmodtager.ligningsfradrag.øvrige_lønmodtagerudgifter.skatteyderstatus",
                        Data::String("Ll9Stk1Lønmodtager".to_string()),
                    ),
                    (
                        "lønmodtager.ligningsfradrag.rejser.personrolle",
                        Data::String("Ll9AAlmindeligLønmodtager".to_string()),
                    ),
                    (
                        "lønmodtager.ligningsfradrag.rejser.ølogi.$variant",
                        Data::String("UdenØlogifradrag".to_string()),
                    ),
                    (
                        "lønmodtager.ligningsfradrag.rejser.dobbelt_husførelse.$variant",
                        Data::String("Ll9AIntetFradragForDobbeltHusførelse".to_string()),
                    ),
                    (
                        "lønmodtager.ligningsfradrag.faglige_kontingenter.skatteyderstatus",
                        Data::String("Ll13Lønmodtager".to_string()),
                    ),
                    (
                        "lønmodtager.ligningsfradrag.arbejdsløshed_efterløn_og_fleksydelse.skattepligtsposition.$variant",
                        Data::String(
                            "Pbl49FuldtSkattepligtigOgHjemmehørendeIDanmark".to_string(),
                        ),
                    ),
                    (
                        "lønmodtager.pension.fødselsdato.år",
                        Data::String("1990".to_string()),
                    ),
                    (
                        "lønmodtager.pension.fødselsdato.måned",
                        Data::String("1".to_string()),
                    ),
                    (
                        "lønmodtager.pension.fødselsdato.dag",
                        Data::String("1".to_string()),
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
                        "negativ_aktieskat_fremførsel.hovedperson.$variant",
                        Data::String("UdenFremførtNegativAktieskat".to_string()),
                    ),
                    (
                        "negativ_aktieskat_fremførsel.ægtefælle.$variant",
                        Data::String("UdenFremførtNegativAktieskat".to_string()),
                    ),
                    (
                        "ligningslov33.hovedperson.$variant",
                        Data::String("UdenLigningslov33".to_string()),
                    ),
                    (
                        "ligningslov33.ægtefælle.$variant",
                        Data::String("UdenLigningslov33".to_string()),
                    ),
                    (
                        "ligningslov33.ligningslov33a_hovedperson.$variant",
                        Data::String("UdenLigningslov33A".to_string()),
                    ),
                    (
                        "ligningslov33.ligningslov33a_ægtefælle.$variant",
                        Data::String("UdenLigningslov33A".to_string()),
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
        let fill_kgl_choices = |sheets: &mut [(String, Vec<Vec<Data>>)], row: usize| {
            for (header, value) in [
                (
                    "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.par25_valg.obligationer_på_reguleret_marked.position_primo.$variant",
                    Data::String("KglÅrsnettoIntetPar25ValgPrimo".to_string()),
                ),
                (
                    "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.par25_valg.obligationer_på_reguleret_marked.aktuelt_princip",
                    Data::String("KglRealisationsprincip".to_string()),
                ),
                (
                    "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.par25_valg.obligationer_på_reguleret_marked.ændringstilladelse.$variant",
                    Data::String("KglÅrsnettoIngenPar25Ændringstilladelse".to_string()),
                ),
                (
                    "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.par25_valg.valutakursændringer.position_primo.$variant",
                    Data::String("KglÅrsnettoIntetPar25ValgPrimo".to_string()),
                ),
                (
                    "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.par25_valg.valutakursændringer.aktuelt_princip",
                    Data::String("KglRealisationsprincip".to_string()),
                ),
                (
                    "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrige_instrumenter.par25_valg.valutakursændringer.ændringstilladelse.$variant",
                    Data::String("KglÅrsnettoIngenPar25Ændringstilladelse".to_string()),
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
        fill_wage_case(sheets, 24, "personskat-underskud-ekstern-2026");
        for (header, value) in [
            (
                "underskudsforhold.$variant",
                Data::String("EksterntFastsatFremførtUnderskud".to_string()),
            ),
            (
                "underskudsforhold.EksterntFastsatFremførtUnderskud.fra_indkomstår",
                Data::Int(2025),
            ),
            (
                "underskudsforhold.EksterntFastsatFremførtUnderskud.underskud_kroner",
                Data::Int(40_000),
            ),
            (
                "underskudsforhold.EksterntFastsatFremførtUnderskud.proveniens.$variant",
                Data::String("SkatteforvaltningensÅrsopgørelse".to_string()),
            ),
            (
                "underskudsforhold.EksterntFastsatFremførtUnderskud.proveniens.SkatteforvaltningensÅrsopgørelse.dokumentreference",
                Data::String("årsopgørelse-2025-version-1".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "cases", 24, header, value);
        }
        fill_wage_case(sheets, 25, "personskat-underskud-årsresultat-2026");
        fill_wage_case(sheets, 26, "personskat-soemandsfradrag-2026");
        fill_wage_case(sheets, 27, "personskat-pbl15a-relationer-2026");
        fill_wage_case(sheets, 28, "personskat-pbl15a-foraeldreloes-2026");
        for row in [5, 14, 15, 20, 21] {
            fill_kgl_choices(sheets, row);
        }
        set_workbook_cell_by_header(
            sheets,
            "cases",
            26,
            "lønmodtager.ligningsfradrag.sømandsfradrag.valg",
            Data::String("AnvendSømandsfradrag".to_string()),
        );
        for (header, value) in [
            (
                "underskudsforhold.$variant",
                Data::String("FremførtUnderskudFraForrigePersonskatÅr".to_string()),
            ),
            (
                "underskudsforhold.FremførtUnderskudFraForrigePersonskatÅr.resultat.indkomstår",
                Data::Int(2025),
            ),
            (
                "underskudsforhold.FremførtUnderskudFraForrigePersonskatÅr.resultat.åbningsgrundlag_gyldigt",
                Data::Bool(true),
            ),
            (
                "underskudsforhold.FremførtUnderskudFraForrigePersonskatÅr.resultat.fremført_underskud_primo_kroner",
                Data::Int(0),
            ),
            (
                "underskudsforhold.FremførtUnderskudFraForrigePersonskatÅr.resultat.årets_skattepligtige_indkomst_før_fremførsel_kroner",
                Data::Int(-30_000),
            ),
            (
                "underskudsforhold.FremførtUnderskudFraForrigePersonskatÅr.resultat.fremført_underskud_anvendt_i_egen_indkomst_kroner",
                Data::Int(0),
            ),
            (
                "underskudsforhold.FremførtUnderskudFraForrigePersonskatÅr.resultat.årets_nye_underskud_kroner",
                Data::Int(30_000),
            ),
            (
                "underskudsforhold.FremførtUnderskudFraForrigePersonskatÅr.resultat.dækket_ved_egen_skattemodregning_kroner",
                Data::Int(0),
            ),
            (
                "underskudsforhold.FremførtUnderskudFraForrigePersonskatÅr.resultat.fradraget_i_ægtefælles_indkomst_kroner",
                Data::Int(0),
            ),
            (
                "underskudsforhold.FremførtUnderskudFraForrigePersonskatÅr.resultat.dækket_ved_ægtefælles_skattemodregning_kroner",
                Data::Int(0),
            ),
            (
                "underskudsforhold.FremførtUnderskudFraForrigePersonskatÅr.resultat.fremført_underskud_ultimo_kroner",
                Data::Int(30_000),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "cases", 25, header, value);
        }
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
                (
                    "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.par32_kontraktforløb.MedPar32Kontraktforløb.aktuelt_år.valg.aktiemodregningsfordeling.$variant",
                    Data::String("KglPar32AfledEntydigFordeling".to_string()),
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
            -1_000,
            3_000,
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
            -5_000,
            -3_000,
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
                Data::String("KglPar32KunEgneAktiegevinster".to_string()),
            ),
            (
                "fakta.valg.aktiemodregningsvalg.beløb.$variant",
                Data::String("KglPar32MaksimalAktiemodregning".to_string()),
            ),
            (
                "fakta.valg.aktiemodregningsfordeling.$variant",
                Data::String("KglPar32AfledEntydigFordeling".to_string()),
            ),
            (
                "årsgrundlag.ejendomsavance.$variant",
                Data::String("UdenEjendomsavance".to_string()),
            ),
            (
                "årsgrundlag.øvrige_instrumenter.par25_valg.obligationer_på_reguleret_marked.position_primo.$variant",
                Data::String("KglÅrsnettoIntetPar25ValgPrimo".to_string()),
            ),
            (
                "årsgrundlag.øvrige_instrumenter.par25_valg.obligationer_på_reguleret_marked.aktuelt_princip",
                Data::String("KglRealisationsprincip".to_string()),
            ),
            (
                "årsgrundlag.øvrige_instrumenter.par25_valg.obligationer_på_reguleret_marked.ændringstilladelse.$variant",
                Data::String("KglÅrsnettoIngenPar25Ændringstilladelse".to_string()),
            ),
            (
                "årsgrundlag.øvrige_instrumenter.par25_valg.valutakursændringer.position_primo.$variant",
                Data::String("KglÅrsnettoIntetPar25ValgPrimo".to_string()),
            ),
            (
                "årsgrundlag.øvrige_instrumenter.par25_valg.valutakursændringer.aktuelt_princip",
                Data::String("KglRealisationsprincip".to_string()),
            ),
            (
                "årsgrundlag.øvrige_instrumenter.par25_valg.valutakursændringer.ændringstilladelse.$variant",
                Data::String("KglÅrsnettoIngenPar25Ændringstilladelse".to_string()),
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
        let par32_history_debt_path = format!("{par32_history_path}.årsgrundlag.gældsposter");
        let par32_history_debt_sheet =
            workbook_collection_sheet_name_from_rows(sheets, &par32_history_debt_path);
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-kgl-par32-historik-2026".to_string()),
            ),
            ("parent_id", Data::String("par32-historik-2025".to_string())),
            (
                "item_id",
                Data::String("par32-historisk-usd-laan".to_string()),
            ),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String("par32-historisk-usd-laan".to_string()),
            ),
            (
                "beløb.gældens_værdi_ved_påtagelse_kroner",
                Data::Int(10_000),
            ),
            (
                "beløb.gældens_værdi_ved_frigørelse_eller_indfrielse_kroner",
                Data::Int(9_000),
            ),
            (
                "beløb.fordringens_værdi_for_kreditor_kroner",
                Data::Int(9_000),
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
            set_workbook_cell_by_header(sheets, &par32_history_debt_sheet, 1, header, value);
        }
        let par32_history_abl22_path = format!(
            "{par32_history_path}.årsgrundlag.øvrige_instrumenter.obligationsbaserede_minimumsbeviser"
        );
        let par32_history_abl22_sheet =
            workbook_collection_sheet_name_from_rows(sheets, &par32_history_abl22_path);
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-kgl-par32-historik-2026".to_string()),
            ),
            (
                "parent_id",
                Data::String("par32-historik-2025".to_string()),
            ),
            (
                "item_id",
                Data::String("par32-historisk-abl22".to_string()),
            ),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String("par32-historisk-abl22".to_string()),
            ),
            ("kilde.klassifikation.indkomstår", Data::Int(2025)),
            (
                "kilde.klassifikation.aktivmasse.indkomstår",
                Data::Int(2025),
            ),
            (
                "kilde.klassifikation.oplysninger.$variant",
                Data::String("AblPar21OplysningerIndsendt".to_string()),
            ),
            (
                "kilde.klassifikation.oplysninger.AblPar21OplysningerIndsendt.frist.år",
                Data::Int(2026),
            ),
            (
                "kilde.klassifikation.oplysninger.AblPar21OplysningerIndsendt.frist.måned",
                Data::Int(7),
            ),
            (
                "kilde.klassifikation.oplysninger.AblPar21OplysningerIndsendt.frist.dag",
                Data::Int(1),
            ),
            (
                "kilde.klassifikation.oplysninger.AblPar21OplysningerIndsendt.indsendelsesdato.år",
                Data::Int(2026),
            ),
            (
                "kilde.klassifikation.oplysninger.AblPar21OplysningerIndsendt.indsendelsesdato.måned",
                Data::Int(7),
            ),
            (
                "kilde.klassifikation.oplysninger.AblPar21OplysningerIndsendt.indsendelsesdato.dag",
                Data::Int(1),
            ),
            (
                "kilde.par17_modprøve.næringsstatus",
                Data::String("AblPar17UdøverIkkeNæringVedKøbOgSalgAfAktier".to_string()),
            ),
            (
                "kilde.par17_modprøve.erhvervelsesstatus",
                Data::String("AblPar17IkkeErhvervetSomLedINæringsvej".to_string()),
            ),
            (
                "position_primo.$variant",
                Data::String("KglÅrsnettoIngenPositionPrimo".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &par32_history_abl22_sheet, 1, header, value);
        }
        let par32_history_abl22_assets_path =
            format!("{par32_history_abl22_path}.kilde.klassifikation.aktivmasse.direkte_aktiver");
        let par32_history_abl22_assets_sheet =
            workbook_collection_sheet_name_from_rows(sheets, &par32_history_abl22_assets_path);
        for (row, item_id, art, value) in [
            (
                1,
                "par32-historisk-abl22-aktiv-1",
                "AblKvalificerendeAktieaktiv",
                20_000,
            ),
            (
                2,
                "par32-historisk-abl22-aktiv-2",
                "AblAndetVærdipapir",
                80_000,
            ),
        ] {
            for (header, cell) in [
                (
                    "case_id",
                    Data::String("personskat-kgl-par32-historik-2026".to_string()),
                ),
                (
                    "parent_id",
                    Data::String("par32-historisk-abl22".to_string()),
                ),
                ("item_id", Data::String(item_id.to_string())),
                ("position", Data::Int(row as i64)),
                (
                    "$variant",
                    Data::String("AblDirekteInvesteringsaktiv".to_string()),
                ),
                (
                    "AblDirekteInvesteringsaktiv.art",
                    Data::String(art.to_string()),
                ),
                (
                    "AblDirekteInvesteringsaktiv.gennemsnitlig_værdi_kroner",
                    Data::Int(value),
                ),
            ] {
                set_workbook_cell_by_header(
                    sheets,
                    &par32_history_abl22_assets_sheet,
                    row,
                    header,
                    cell,
                );
            }
        }
        let par32_history_abl22_events_path = format!("{par32_history_abl22_path}.hændelser");
        let par32_history_abl22_events_sheet =
            workbook_collection_sheet_name_from_rows(sheets, &par32_history_abl22_events_path);
        for (row, item_id, variant, header, value) in [
            (
                1,
                "par32-historisk-abl22-anskaffelse",
                "KglÅrsnettoAnskaffelse",
                "KglÅrsnettoAnskaffelse.anskaffelsessum_kroner",
                10_000,
            ),
            (
                2,
                "par32-historisk-abl22-afståelse",
                "KglÅrsnettoAfståelse",
                "KglÅrsnettoAfståelse.afståelsessum_kroner",
                11_500,
            ),
        ] {
            for (column, cell) in [
                (
                    "case_id",
                    Data::String("personskat-kgl-par32-historik-2026".to_string()),
                ),
                (
                    "parent_id",
                    Data::String("par32-historisk-abl22".to_string()),
                ),
                ("item_id", Data::String(item_id.to_string())),
                ("position", Data::Int(row as i64)),
                ("$variant", Data::String(variant.to_string())),
                (header, Data::Int(value)),
            ] {
                set_workbook_cell_by_header(
                    sheets,
                    &par32_history_abl22_events_sheet,
                    row,
                    column,
                    cell,
                );
            }
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
            5_000,
            -5_000,
            false,
            "KglPar32KildeUdenSærligRelation",
            None,
            "KglPar32KildeEnkeltaktie",
        );
        set_workbook_cell_by_header(
            sheets,
            &par32_history_contracts_sheet,
            1,
            "underliggende.KglPar32KildeEnkeltaktie.markedsstatus",
            Data::String("AblOptagetTilHandelPåReguleretMarked".to_string()),
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
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag.$variant",
                Data::String("Søbl5IntetDødsboskattegrundlag".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.personlig_indkomst.sømandsbeskatning.kulbrinteskattegrundlag.$variant",
                Data::String("Søbl5BIntetKulbrinteskattegrundlag".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.ligningsfradrag.sømandsfradrag.valg",
                Data::String("FravælgSømandsfradrag".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.ligningsfradrag.fiskerfradrag.valg",
                Data::String("Ll9GFravælgFiskerfradrag".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.ligningsfradrag.øvrige_lønmodtagerudgifter.skatteyderstatus",
                Data::String("Ll9Stk1Lønmodtager".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.ligningsfradrag.rejser.personrolle",
                Data::String("Ll9AAlmindeligLønmodtager".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.ligningsfradrag.rejser.ølogi.$variant",
                Data::String("UdenØlogifradrag".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.ligningsfradrag.rejser.dobbelt_husførelse.$variant",
                Data::String("Ll9AIntetFradragForDobbeltHusførelse".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.ligningsfradrag.faglige_kontingenter.skatteyderstatus",
                Data::String("Ll13Lønmodtager".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.ligningsfradrag.arbejdsløshed_efterløn_og_fleksydelse.skattepligtsposition.$variant",
                Data::String(
                    "Pbl49FuldtSkattepligtigOgHjemmehørendeIDanmark".to_string(),
                ),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.pension.fødselsdato.år",
                Data::String("1990".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.pension.fødselsdato.måned",
                Data::String("1".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.lønmodtager.pension.fødselsdato.dag",
                Data::String("1".to_string()),
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
                "ægtefælle.MedÆgtefælle.fakta.ejendomsskatter.person.ejer_folkepensionsalder.$variant",
                Data::String("EjskFolkepensionsalderIkkeOpnået".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.ejendomsskatter.person.samlevende_ægtefælles_folkepensionsalder.$variant",
                Data::String("EjskFolkepensionsalderIkkeOpnået".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.ejendomsskatter.person.skattemæssigt_hjemsted.$variant",
                Data::String("EjskFuldtSkattepligtigEfterKildeskattelovensPar1".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.ejendomsskatter.person.egen_udbytteindkomst_kroner",
                Data::String("0".to_string()),
            ),
            (
                "ægtefælle.MedÆgtefælle.fakta.ejendomsskatter.person.ægtefælles_udbytteindkomst_kroner",
                Data::String("0".to_string()),
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
        let seafarer_employments_sheet = workbook_collection_sheet_name_from_rows(
            sheets,
            "lønmodtager.ligningsfradrag.sømandsfradrag.beskæftigelser",
        );
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-soemandsfradrag-2026".to_string()),
            ),
            ("item_id", Data::String("fragtskib-over-500".to_string())),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String("fragtskib-over-500".to_string()),
            ),
            ("indkomstår", Data::Int(2026)),
            ("ansættelsesforhold_startdato.år", Data::Int(2021)),
            ("ansættelsesforhold_startdato.måned", Data::Int(12)),
            ("ansættelsesforhold_startdato.dag", Data::Int(31)),
            (
                "arbejdssted.$variant",
                Data::String("ArbejdePåSkib".to_string()),
            ),
            ("arbejdssted.ArbejdePåSkib.bruttotonnage", Data::Int(500)),
            (
                "arbejdssted.ArbejdePåSkib.anvendelse.$variant",
                Data::String("ErhvervsmæssigBefordringAfPassagerer".to_string()),
            ),
            (
                "arbejdssted.ArbejdePåSkib.anvendelse.ErhvervsmæssigBefordringAfPassagerer.rute.$variant",
                Data::String("RegelmæssigPassagersejladsMellemEUEØSHavne".to_string()),
            ),
            (
                "arbejdssted.ArbejdePåSkib.anvendelse.ErhvervsmæssigBefordringAfPassagerer.rute.RegelmæssigPassagersejladsMellemEUEØSHavne.skattepligt",
                Data::String("SkattepligtigEfterKildeskattelov1".to_string()),
            ),
            (
                "arbejdssted.ArbejdePåSkib.anvendelse.ErhvervsmæssigBefordringAfPassagerer.rute.RegelmæssigPassagersejladsMellemEUEØSHavne.statsborgerskab",
                Data::String("AndetStatsborgerskab".to_string()),
            ),
            ("fart", Data::String("UdenForBegrænsetFart".to_string())),
            (
                "hjemsted",
                Data::String("RegistreretMedHjemstedIDanmark".to_string()),
            ),
            ("flag", Data::String("FlagFraEUEØSStat".to_string())),
            (
                "forhyringsvilkår",
                Data::String("SædvanligeForhyringsvilkårForSøfolk".to_string()),
            ),
            ("fuldtidsomregnede_sødage_hundrededele", Data::Int(36_500)),
        ] {
            set_workbook_cell_by_header(sheets, &seafarer_employments_sheet, 1, header, value);
        }
        let pbl15a_case_id = "personskat-pbl15a-relationer-2026";
        let pbl15a_disposal_id = "pbl15a-virksomhedsafståelse";
        let pbl15a_disposals_path = "lønmodtager.pension.pbl15a_årsgrundlag.afståelser";
        let pbl15a_disposals_sheet =
            workbook_collection_sheet_name_from_rows(sheets, pbl15a_disposals_path);
        for (header, value) in [
            ("case_id", Data::String(pbl15a_case_id.to_string())),
            ("item_id", Data::String(pbl15a_disposal_id.to_string())),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String(pbl15a_disposal_id.to_string()),
            ),
            ("afståelsesdato.år", Data::Int(2026)),
            ("afståelsesdato.måned", Data::Int(7)),
            ("afståelsesdato.dag", Data::Int(1)),
            (
                "grundlag.$variant",
                Data::String("Pbl15AEgenVirksomhedsfortjeneste".to_string()),
            ),
            (
                "fortjenestegrundlag.ejendomsavance.$variant",
                Data::String("Pbl15AUdenEjendomsavance".to_string()),
            ),
            (
                "fortjenestegrundlag.aktieavance.$variant",
                Data::String("Pbl15AUdenAktieavance".to_string()),
            ),
            ("alder_hele_år_ved_afståelsen", Data::Int(60)),
            (
                "passiv_kapital.aktuel_regnskabsperiodes_startdato.år",
                Data::Int(2026),
            ),
            (
                "passiv_kapital.aktuel_regnskabsperiodes_startdato.måned",
                Data::Int(1),
            ),
            (
                "passiv_kapital.aktuel_regnskabsperiodes_startdato.dag",
                Data::Int(1),
            ),
            (
                "passiv_kapital.holdingforløb.personens_afståelsesrækkefølge_på_dagen",
                Data::Int(1),
            ),
            (
                "passiv_kapital.holdingforløb.afvikling.$variant",
                Data::String("Pbl15AHoldingFortsatBestående".to_string()),
            ),
            (
                "passiv_kapital.næringsvirksomhed_med_værdipapirer_eller_finansiering",
                Data::Bool(false),
            ),
            (
                "udlejning_af_afskrivningsberettigede_driftsmidler_eller_skibe",
                Data::Bool(false),
            ),
            ("antal_ejere", Data::Int(1)),
            (
                "opretter_deltog_i_driften_i_væsentligt_omfang",
                Data::Bool(true),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &pbl15a_disposals_sheet, 1, header, value);
        }

        let pbl15a_depreciation_sources_path =
            format!("{pbl15a_disposals_path}.fortjenestegrundlag.afskrivningslovsposter");
        let pbl15a_depreciation_sources_sheet =
            workbook_collection_sheet_name_from_rows(sheets, &pbl15a_depreciation_sources_path);
        for (header, value) in [
            ("case_id", Data::String(pbl15a_case_id.to_string())),
            ("parent_id", Data::String(pbl15a_disposal_id.to_string())),
            (
                "item_id",
                Data::String("pbl15a-al6-afståelsesfortjeneste".to_string()),
            ),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String("pbl15a-al6-afståelsesfortjeneste".to_string()),
            ),
            (
                "kilde.$variant",
                Data::String("Pbl15AAl6Salg".to_string()),
            ),
            (
                "kilde.Pbl15AAl6Salg.input.straksafskrivningsresultat.indkomstår",
                Data::Int(2026),
            ),
            (
                "kilde.Pbl15AAl6Salg.input.straksafskrivningsresultat.anskaffelsesår",
                Data::Int(2024),
            ),
            (
                "kilde.Pbl15AAl6Salg.input.straksafskrivningsresultat.skatteyder",
                Data::String("AlSelvstændigErhvervsdrivendePerson".to_string()),
            ),
            (
                "kilde.Pbl15AAl6Salg.input.straksafskrivningsresultat.valg",
                Data::String("Al6Småaktiv".to_string()),
            ),
            (
                "kilde.Pbl15AAl6Salg.input.straksafskrivningsresultat.omfattet_af_par6_stk1_modellen",
                Data::Bool(true),
            ),
            (
                "kilde.Pbl15AAl6Salg.input.straksafskrivningsresultat.kort_levetid_betingelse_opfyldt",
                Data::Bool(false),
            ),
            (
                "kilde.Pbl15AAl6Salg.input.straksafskrivningsresultat.småaktiv_betingelse_opfyldt",
                Data::Bool(true),
            ),
            (
                "kilde.Pbl15AAl6Salg.input.straksafskrivningsresultat.forskning_betingelse_opfyldt",
                Data::Bool(false),
            ),
            (
                "kilde.Pbl15AAl6Salg.input.straksafskrivningsresultat.valg_hjemmel_opfyldt",
                Data::Bool(true),
            ),
            (
                "kilde.Pbl15AAl6Salg.input.straksafskrivningsresultat.omfattet_af_stk3",
                Data::Bool(false),
            ),
            (
                "kilde.Pbl15AAl6Salg.input.straksafskrivningsresultat.udskydelse_efter_stk3_gælder",
                Data::Bool(false),
            ),
            (
                "kilde.Pbl15AAl6Salg.input.straksafskrivningsresultat.første_mulige_fradragsår",
                Data::Int(2024),
            ),
            (
                "kilde.Pbl15AAl6Salg.input.straksafskrivningsresultat.fradragsår_opfyldt",
                Data::Bool(true),
            ),
            (
                "kilde.Pbl15AAl6Salg.input.straksafskrivningsresultat.valg_gyldigt",
                Data::Bool(true),
            ),
            (
                "kilde.Pbl15AAl6Salg.input.straksafskrivningsresultat.forskningsudgift_under_loft_kroner",
                Data::Int(0),
            ),
            (
                "kilde.Pbl15AAl6Salg.input.straksafskrivningsresultat.forskningsudgift_over_loft_kroner",
                Data::Int(0),
            ),
            (
                "kilde.Pbl15AAl6Salg.input.straksafskrivningsresultat.fradrag_i_skattepligtig_indkomst_kroner",
                Data::Int(100_000),
            ),
            (
                "kilde.Pbl15AAl6Salg.input.straksafskrivningsresultat.anskaffelsessum_fradraget_senest_i_indkomståret_kroner",
                Data::Int(100_000),
            ),
            (
                "kilde.Pbl15AAl6Salg.input.straksafskrivningsresultat.ufradraget_anskaffelsessum_kroner",
                Data::Int(0),
            ),
            (
                "kilde.Pbl15AAl6Salg.input.salgssum_kroner",
                Data::Int(100_000),
            ),
            (
                "kilde.Pbl15AAl6Salg.input.leveret_i_indkomståret",
                Data::Bool(true),
            ),
            (
                "kilde.Pbl15AAl6Salg.input.virksomhed_sælges_eller_ophører_i_indkomståret",
                Data::Bool(false),
            ),
        ] {
            set_workbook_cell_by_header(
                sheets,
                &pbl15a_depreciation_sources_sheet,
                1,
                header,
                value,
            );
        }

        let pbl15a_periods_path =
            format!("{pbl15a_disposals_path}.passiv_kapital.seneste_tre_regnskabsperioder");
        let pbl15a_periods_sheet =
            workbook_collection_sheet_name_from_rows(sheets, &pbl15a_periods_path);
        for (row, year, position) in [(1, 2023, 1), (2, 2024, 2), (3, 2025, 3)] {
            let period_id = format!("pbl15a-regnskabsperiode-{year}");
            for (header, value) in [
                ("case_id", Data::String(pbl15a_case_id.to_string())),
                ("parent_id", Data::String(pbl15a_disposal_id.to_string())),
                ("item_id", Data::String(period_id)),
                ("position", Data::Int(position)),
                ("rækkefølge_fra_ældste", Data::Int(position)),
                ("startdato.år", Data::Int(year)),
                ("startdato.måned", Data::Int(1)),
                ("startdato.dag", Data::Int(1)),
                ("slutdato.år", Data::Int(year)),
                ("slutdato.måned", Data::Int(12)),
                ("slutdato.dag", Data::Int(31)),
            ] {
                set_workbook_cell_by_header(sheets, &pbl15a_periods_sheet, row, header, value);
            }
        }

        let pbl15a_company_accounts_path = format!("{pbl15a_periods_path}.selskabsregnskaber");
        let pbl15a_company_accounts_sheet =
            workbook_collection_sheet_name_from_rows(sheets, &pbl15a_company_accounts_path);
        for (row, year, position) in [(1, 2023, 1), (2, 2024, 1), (3, 2025, 1)] {
            let period_id = format!("pbl15a-regnskabsperiode-{year}");
            let account_id = format!("pbl15a-selskabsregnskab-{year}");
            for (header, value) in [
                ("case_id", Data::String(pbl15a_case_id.to_string())),
                ("parent_id", Data::String(period_id)),
                ("item_id", Data::String(account_id)),
                ("position", Data::Int(position)),
                (
                    "selskab.identifikation",
                    Data::String("pbl15a-driftsselskab".to_string()),
                ),
                (
                    "selskab.ejerforhold.direkte_ejerandel_basispoint",
                    Data::Int(1_000),
                ),
                (
                    "selskab.udøver_aktiv_udlejning_efter_abl34_stk7",
                    Data::Bool(false),
                ),
                (
                    "aktiernes_handelsværdi_i_virksomheden_kroner",
                    Data::Int(900_000),
                ),
                ("aktieafkast_i_virksomheden_kroner", Data::Int(300_000)),
            ] {
                set_workbook_cell_by_header(
                    sheets,
                    &pbl15a_company_accounts_sheet,
                    row,
                    header,
                    value,
                );
            }
        }

        let pbl15a_owner_paths_path =
            format!("{pbl15a_company_accounts_path}.selskab.ejerforhold.indirekte_ejerveje");
        let pbl15a_owner_paths_sheet =
            workbook_collection_sheet_name_from_rows(sheets, &pbl15a_owner_paths_path);
        for (row, year) in [(1, 2023), (2, 2024), (3, 2025)] {
            let account_id = format!("pbl15a-selskabsregnskab-{year}");
            let owner_path_id = format!("pbl15a-indirekte-ejervej-{year}");
            for (header, value) in [
                ("case_id", Data::String(pbl15a_case_id.to_string())),
                ("parent_id", Data::String(account_id)),
                ("item_id", Data::String(owner_path_id.clone())),
                ("position", Data::Int(1)),
                ("identifikation", Data::String(owner_path_id)),
            ] {
                set_workbook_cell_by_header(sheets, &pbl15a_owner_paths_sheet, row, header, value);
            }
        }
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-pbl15a-foraeldreloes-2026".to_string()),
            ),
            (
                "parent_id",
                Data::String("manglende-pbl15a-selskabsregnskab".to_string()),
            ),
            (
                "item_id",
                Data::String("pbl15a-foraeldreloes-ejervej".to_string()),
            ),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String("pbl15a-foraeldreloes-ejervej".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &pbl15a_owner_paths_sheet, 4, header, value);
        }

        let pbl15a_owner_shares_path =
            format!("{pbl15a_owner_paths_path}.ejerandele_gennem_kæden_basispoint");
        let pbl15a_owner_shares_sheet =
            workbook_collection_sheet_name_from_rows(sheets, &pbl15a_owner_shares_path);
        for (row, year, position, basispoints) in [
            (1, 2023, 1, 5_000),
            (2, 2023, 2, 4_000),
            (3, 2024, 1, 5_000),
            (4, 2024, 2, 4_000),
            (5, 2025, 1, 5_000),
            (6, 2025, 2, 4_000),
        ] {
            let owner_path_id = format!("pbl15a-indirekte-ejervej-{year}");
            for (header, value) in [
                ("case_id", Data::String(pbl15a_case_id.to_string())),
                ("parent_id", Data::String(owner_path_id.clone())),
                (
                    "item_id",
                    Data::String(format!("{owner_path_id}-led-{position}")),
                ),
                ("position", Data::Int(position)),
                ("value", Data::Int(basispoints)),
            ] {
                set_workbook_cell_by_header(sheets, &pbl15a_owner_shares_sheet, row, header, value);
            }
        }

        let pbl15a_company_income_path =
            format!("{pbl15a_company_accounts_path}.selskabets_indtægter_før_ejerandel");
        let pbl15a_company_income_sheet =
            workbook_collection_sheet_name_from_rows(sheets, &pbl15a_company_income_path);
        let pbl15a_company_assets_path =
            format!("{pbl15a_company_accounts_path}.selskabets_aktiver_før_ejerandel");
        let pbl15a_company_assets_sheet =
            workbook_collection_sheet_name_from_rows(sheets, &pbl15a_company_assets_path);
        for (row, year) in [(1, 2023), (2, 2024), (3, 2025)] {
            let account_id = format!("pbl15a-selskabsregnskab-{year}");
            for (header, value) in [
                ("case_id", Data::String(pbl15a_case_id.to_string())),
                ("parent_id", Data::String(account_id.clone())),
                (
                    "item_id",
                    Data::String(format!("pbl15a-driftsindtægt-{year}")),
                ),
                ("position", Data::Int(1)),
                (
                    "identifikation",
                    Data::String(format!("pbl15a-driftsindtægt-{year}")),
                ),
                ("beløb_kroner", Data::Int(1_000_000)),
                ("art", Data::String("Pbl15AØvrigDriftsindtægt".to_string())),
            ] {
                set_workbook_cell_by_header(
                    sheets,
                    &pbl15a_company_income_sheet,
                    row,
                    header,
                    value,
                );
            }
            for (header, value) in [
                ("case_id", Data::String(pbl15a_case_id.to_string())),
                ("parent_id", Data::String(account_id)),
                (
                    "item_id",
                    Data::String(format!("pbl15a-driftsaktiv-{year}")),
                ),
                ("position", Data::Int(1)),
                (
                    "identifikation",
                    Data::String(format!("pbl15a-driftsaktiv-{year}")),
                ),
                ("handelsværdi_kroner", Data::Int(2_000_000)),
                ("art", Data::String("Pbl15AØvrigtDriftsaktiv".to_string())),
            ] {
                set_workbook_cell_by_header(
                    sheets,
                    &pbl15a_company_assets_sheet,
                    row,
                    header,
                    value,
                );
            }
        }

        let pbl15a_transfer_companies_path = format!(
            "{pbl15a_disposals_path}.passiv_kapital.selskabsaktiver_på_overdragelsestidspunktet"
        );
        let pbl15a_transfer_companies_sheet =
            workbook_collection_sheet_name_from_rows(sheets, &pbl15a_transfer_companies_path);
        let pbl15a_transfer_company_id = "pbl15a-selskabsaktiv-ved-afståelse";
        for (header, value) in [
            ("case_id", Data::String(pbl15a_case_id.to_string())),
            ("parent_id", Data::String(pbl15a_disposal_id.to_string())),
            (
                "item_id",
                Data::String(pbl15a_transfer_company_id.to_string()),
            ),
            ("position", Data::Int(1)),
            (
                "selskab.identifikation",
                Data::String("pbl15a-driftsselskab".to_string()),
            ),
            (
                "selskab.ejerforhold.direkte_ejerandel_basispoint",
                Data::Int(1_000),
            ),
            (
                "selskab.udøver_aktiv_udlejning_efter_abl34_stk7",
                Data::Bool(false),
            ),
            (
                "aktiernes_handelsværdi_i_virksomheden_kroner",
                Data::Int(900_000),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &pbl15a_transfer_companies_sheet, 1, header, value);
        }

        let pbl15a_transfer_owner_paths_path =
            format!("{pbl15a_transfer_companies_path}.selskab.ejerforhold.indirekte_ejerveje");
        let pbl15a_transfer_owner_paths_sheet =
            workbook_collection_sheet_name_from_rows(sheets, &pbl15a_transfer_owner_paths_path);
        let pbl15a_transfer_owner_path_id = "pbl15a-indirekte-ejervej-ved-afståelse";
        for (header, value) in [
            ("case_id", Data::String(pbl15a_case_id.to_string())),
            (
                "parent_id",
                Data::String(pbl15a_transfer_company_id.to_string()),
            ),
            (
                "item_id",
                Data::String(pbl15a_transfer_owner_path_id.to_string()),
            ),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String(pbl15a_transfer_owner_path_id.to_string()),
            ),
        ] {
            set_workbook_cell_by_header(
                sheets,
                &pbl15a_transfer_owner_paths_sheet,
                1,
                header,
                value,
            );
        }
        let pbl15a_transfer_owner_shares_path =
            format!("{pbl15a_transfer_owner_paths_path}.ejerandele_gennem_kæden_basispoint");
        let pbl15a_transfer_owner_shares_sheet =
            workbook_collection_sheet_name_from_rows(sheets, &pbl15a_transfer_owner_shares_path);
        for (row, position, basispoints) in [(1, 1, 5_000), (2, 2, 4_000)] {
            for (header, value) in [
                ("case_id", Data::String(pbl15a_case_id.to_string())),
                (
                    "parent_id",
                    Data::String(pbl15a_transfer_owner_path_id.to_string()),
                ),
                (
                    "item_id",
                    Data::String(format!("{pbl15a_transfer_owner_path_id}-led-{position}")),
                ),
                ("position", Data::Int(position)),
                ("value", Data::Int(basispoints)),
            ] {
                set_workbook_cell_by_header(
                    sheets,
                    &pbl15a_transfer_owner_shares_sheet,
                    row,
                    header,
                    value,
                );
            }
        }
        let pbl15a_transfer_assets_path =
            format!("{pbl15a_transfer_companies_path}.selskabets_aktiver_før_ejerandel");
        let pbl15a_transfer_assets_sheet =
            workbook_collection_sheet_name_from_rows(sheets, &pbl15a_transfer_assets_path);
        for (header, value) in [
            ("case_id", Data::String(pbl15a_case_id.to_string())),
            (
                "parent_id",
                Data::String(pbl15a_transfer_company_id.to_string()),
            ),
            (
                "item_id",
                Data::String("pbl15a-driftsaktiv-ved-afståelse".to_string()),
            ),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String("pbl15a-driftsaktiv-ved-afståelse".to_string()),
            ),
            ("handelsværdi_kroner", Data::Int(2_000_000)),
            ("art", Data::String("Pbl15AØvrigtDriftsaktiv".to_string())),
        ] {
            set_workbook_cell_by_header(sheets, &pbl15a_transfer_assets_sheet, 1, header, value);
        }

        let pbl15a_plans_sheet = workbook_collection_sheet_name_from_rows(
            sheets,
            "lønmodtager.pension.pbl15a_årsgrundlag.ordninger",
        );
        for (header, value) in [
            ("case_id", Data::String(pbl15a_case_id.to_string())),
            ("item_id", Data::String("pbl15a-ophørspension".to_string())),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String("pbl15a-ophørspension".to_string()),
            ),
            ("oprettelsesår", Data::Int(2026)),
            ("art", Data::String("Pbl15ARateopsparing".to_string())),
            (
                "oprettelsesafståelse_identifikation",
                Data::String(pbl15a_disposal_id.to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &pbl15a_plans_sheet, 1, header, value);
        }

        let pbl15a_qualification_years_sheet = workbook_collection_sheet_name_from_rows(
            sheets,
            "lønmodtager.pension.pbl15a_årsgrundlag.kvalifikationsår",
        );
        for (row, year) in (2016_i64..=2025).enumerate() {
            let position = row as i64 + 1;
            for (header, value) in [
                ("case_id", Data::String(pbl15a_case_id.to_string())),
                (
                    "item_id",
                    Data::String(format!("pbl15a-kvalifikationsår-{year}")),
                ),
                ("position", Data::Int(position)),
                ("indkomstår", Data::Int(year)),
                (
                    "grundlag",
                    Data::String("Pbl15AEgenSelvstændigVirksomhed".to_string()),
                ),
            ] {
                set_workbook_cell_by_header(
                    sheets,
                    &pbl15a_qualification_years_sheet,
                    row + 1,
                    header,
                    value,
                );
            }
        }

        let pbl18_contributions_sheet = workbook_collection_sheet_name_from_rows(
            sheets,
            "lønmodtager.pension.pbl18_indbetalinger",
        );
        for (header, value) in [
            ("case_id", Data::String(pbl15a_case_id.to_string())),
            (
                "item_id",
                Data::String("pbl15a-indbetaling-2026".to_string()),
            ),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String("pbl15a-indbetaling-2026".to_string()),
            ),
            (
                "ordning",
                Data::String("Pbl18Par15ARateordning".to_string()),
            ),
            (
                "indbetalingskilde",
                Data::String("Pbl18EgenIndbetaling".to_string()),
            ),
            (
                "fradragsretshaver",
                Data::String("Pbl18OrdningensEjer".to_string()),
            ),
            ("betaling.beløb_kroner", Data::Int(75_000)),
            ("betaling.forfaldsår", Data::Int(2026)),
            ("betaling.betalingsår", Data::Int(2026)),
            (
                "betaling.betalt_senest_bankjusteret_1_april_efter_forfald",
                Data::Bool(true),
            ),
            (
                "betaling.hidrører_fra_par22e_tilbagebetaling",
                Data::Bool(false),
            ),
            (
                "betaling.par15a_fradragsplacering.$variant",
                Data::String("Pbl18Par15AIndbetalingsår".to_string()),
            ),
            ("betaling.arbejdsmarkedsbidrag_kroner", Data::Int(0)),
            (
                "fordelingsforløb.$variant",
                Data::String("Pbl18IngenTiårsfordeling".to_string()),
            ),
            (
                "indeksordningsgrundlag.$variant",
                Data::String("Pbl18IkkeIndeksordning".to_string()),
            ),
            (
                "forfaldne_ikke_tidligere_fratrukket_kroner",
                Data::Int(75_000),
            ),
            (
                "særligt_ordningsgrundlag.$variant",
                Data::String("Pbl18Par15AIndbetalingsgrundlag".to_string()),
            ),
            (
                "særligt_ordningsgrundlag.Pbl18Par15AIndbetalingsgrundlag.ordning_identifikation",
                Data::String("pbl15a-ophørspension".to_string()),
            ),
            (
                "særligt_ordningsgrundlag.Pbl18Par15AIndbetalingsgrundlag.afståelse_identifikation",
                Data::String(pbl15a_disposal_id.to_string()),
            ),
            (
                "begrænsninger.pbl54_personkreds_opfyldt",
                Data::Bool(true),
            ),
            (
                "begrænsninger.afgiftspligt_for_hele_ordningen_indtrådt",
                Data::Bool(false),
            ),
            (
                "begrænsninger.udenlandsk_overførsel_med_tidligere_fradrag_uden_skatte_eller_afgiftskonsekvens",
                Data::Bool(false),
            ),
        ] {
            set_workbook_cell_by_header(
                sheets,
                &pbl18_contributions_sheet,
                1,
                header,
                value,
            );
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
        let commuting_path = "lønmodtager.ligningsfradrag.befordring.forhold";
        let commuting_sheet = workbook_collection_sheet_name_from_rows(sheets, commuting_path);
        for (header, value) in [
            (
                "case_id".to_string(),
                Data::String("personskat-renter-befordring-2026".to_string()),
            ),
            (
                "item_id".to_string(),
                Data::String("befordring-arbejde-1".to_string()),
            ),
            ("position".to_string(), Data::Int(1)),
            (
                format!("{commuting_path}.identifikation"),
                Data::String("befordring-arbejde-1".to_string()),
            ),
            (
                format!("{commuting_path}.befordringsmål_identifikation"),
                Data::String("arbejdsforhold-1".to_string()),
            ),
            (format!("{commuting_path}.arbejdsdage"), Data::Int(203)),
            (
                format!("{commuting_path}.daglige_befordringskilometer"),
                Data::Int(60),
            ),
            (
                format!("{commuting_path}.bopæl_i_yderkommune_eller_lille_ø"),
                Data::Bool(false),
            ),
            (
                format!("{commuting_path}.befordringsformål"),
                Data::String("IndtægtsgivendeArbejdsplads".to_string()),
            ),
            (
                format!("{commuting_path}.modtaget_skattefri_befordringsgodtgørelse_for_strækning"),
                Data::Bool(false),
            ),
            (
                format!("{commuting_path}.modtaget_uddannelsesbefordringsrabat_eller_godtgørelse_for_strækning"),
                Data::Bool(false),
            ),
            (
                format!("{commuting_path}.ligningslov9d.$variant"),
                Data::String("UdenLigningslov9D".to_string()),
            ),
            (
                format!("{commuting_path}.fradrag_udelukket_folketingshverv_m_v"),
                Data::Bool(false),
            ),
            (
                format!("{commuting_path}.arbejdsgiverbetalt_befordring"),
                Data::String("UdenArbejdsgiverbetaltBefordring".to_string()),
            ),
            (
                format!("{commuting_path}.broer.storebælt_bil_motorcykel_passager"),
                Data::Int(0),
            ),
            (
                format!("{commuting_path}.broer.storebælt_kollektiv_passager"),
                Data::Int(0),
            ),
            (
                format!("{commuting_path}.broer.øresund_bil_motorcykel_passager"),
                Data::Int(0),
            ),
            (
                format!("{commuting_path}.broer.øresund_kollektiv_passager"),
                Data::Int(0),
            ),
            (
                format!("{commuting_path}.broer.dokumenteret_og_afholdt_af_skattepligtige"),
                Data::Bool(false),
            ),
            (
                format!("{commuting_path}.særlig_transport.faktisk_dokumenteret_udgift_kroner"),
                Data::Int(0),
            ),
            (
                format!("{commuting_path}.særlig_transport.geografiske_forhold_tidsforbrug_økonomisk_rimelighed_kræver_transporten"),
                Data::Bool(false),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &commuting_sheet, 1, &header, value);
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
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.fraflytningsdato.år",
                Data::Int(2026),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.fraflytningsdato.måned",
                Data::Int(7),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.fraflytning.fraflytningsdato.dag",
                Data::Int(1),
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
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.kildehistorik.opgjort_pr.år",
                Data::Int(2026),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.kildehistorik.opgjort_pr.måned",
                Data::Int(12),
            ),
            (
                "kilde.PersonskatFraflytteraktierEfterPar37Til40.fakta.kildehistorik.opgjort_pr.dag",
                Data::Int(31),
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

        let pbl15a_disposals_path = "lønmodtager.pension.pbl15a_årsgrundlag.afståelser";
        let pbl15a_periods_path =
            format!("{pbl15a_disposals_path}.passiv_kapital.seneste_tre_regnskabsperioder");
        let pbl15a_periods_sheet =
            workbook_collection_sheet_name(&mut workbook, &pbl15a_periods_path);
        let pbl15a_periods = workbook
            .worksheet_range(&pbl15a_periods_sheet)
            .expect("PBL § 15 A accounting periods");
        let pbl15a_period_rows = pbl15a_periods.rows().skip(2).collect::<Vec<_>>();
        assert_eq!(pbl15a_period_rows.len(), 3);
        for (row, item_id, position) in [
            (&pbl15a_period_rows[0], "pbl15a-regnskabsperiode-2023", "1"),
            (&pbl15a_period_rows[1], "pbl15a-regnskabsperiode-2024", "2"),
            (&pbl15a_period_rows[2], "pbl15a-regnskabsperiode-2025", "3"),
        ] {
            assert_eq!(row[0].to_string(), "personskat-pbl15a-relationer-2026");
            assert_eq!(row[1].to_string(), "pbl15a-virksomhedsafståelse");
            assert_eq!(row[2].to_string(), item_id);
            assert_eq!(row[3].to_string(), position);
        }
        let pbl15a_company_accounts_path = format!("{pbl15a_periods_path}.selskabsregnskaber");
        let pbl15a_owner_paths_path =
            format!("{pbl15a_company_accounts_path}.selskab.ejerforhold.indirekte_ejerveje");
        let pbl15a_owner_paths_sheet =
            workbook_collection_sheet_name(&mut workbook, &pbl15a_owner_paths_path);
        let pbl15a_owner_paths = workbook
            .worksheet_range(&pbl15a_owner_paths_sheet)
            .expect("PBL § 15 A indirect ownership paths");
        let pbl15a_owner_path_rows = pbl15a_owner_paths.rows().skip(2).collect::<Vec<_>>();
        assert_eq!(pbl15a_owner_path_rows.len(), 4);
        assert_eq!(
            pbl15a_owner_path_rows[0][1].to_string(),
            "pbl15a-selskabsregnskab-2023"
        );
        assert_eq!(
            pbl15a_owner_path_rows[3][0].to_string(),
            "personskat-pbl15a-foraeldreloes-2026"
        );
        assert_eq!(
            pbl15a_owner_path_rows[3][1].to_string(),
            "manglende-pbl15a-selskabsregnskab"
        );
    }

    let output = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&input_path).ok();
    assert!(!output.status.success());
    let result = parse_stdout(&output);
    let xlsx_exact_tax_result = result["results"]
        .as_array()
        .expect("XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-årsopgørelse-2026")
        .expect("XLSX exact annual-assessment result");
    assert_eq!(xlsx_exact_tax_result["result"]["slutskat_øre"], 20_872_564);
    assert_eq!(
        xlsx_exact_tax_result["result"]["slutskat_kroner_kompatibilitetsprojektion"],
        208_725
    );
    assert_eq!(
        xlsx_exact_tax_result["result"]["årsopgørelse"]["input"]["slutskat_øre"],
        20_872_564
    );
    assert_eq!(
        xlsx_exact_tax_result["result"]["årsopgørelse"]["resultat"]["slutskat_med_tillæg_øre"],
        21_022_564
    );
    let diagnostics = result["diagnostics"]
        .as_array()
        .expect("XLSX Personskat diagnostics");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
    assert_eq!(
        diagnostics[0]["case_id"],
        "personskat-pbl15a-foraeldreloes-2026"
    );
    assert!(diagnostics[0]["message"]
        .as_str()
        .is_some_and(|message| message.contains("orphan parent_id")));
    let xlsx_pbl15a_result = result["results"]
        .as_array()
        .expect("XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-pbl15a-relationer-2026")
        .expect("XLSX PBL § 15 A relation result");
    let xlsx_pbl15a_canonical_result = xlsx_pbl15a_result["result"].clone();
    let xlsx_pbl15a_annual_result = &xlsx_pbl15a_result["result"]["pension"]["pbl18_årsresultat"];
    assert_eq!(xlsx_pbl15a_annual_result["input_gyldigt"], true);
    assert_eq!(xlsx_pbl15a_annual_result["par15a_fradrag_kroner"], 75_000);
    assert_eq!(
        xlsx_pbl15a_annual_result["samlet_fradrag_i_skattepligtig_indkomst_kroner"],
        75_000
    );
    let xlsx_pbl15a_disposal =
        &xlsx_pbl15a_annual_result["par15a_grundlagsresultat"]["afståelsesresultater"][0];
    assert_eq!(xlsx_pbl15a_disposal["fakta_gyldige"], true);
    assert_eq!(xlsx_pbl15a_disposal["kvalificerer_til_ophørspension"], true);
    assert_eq!(
        xlsx_pbl15a_disposal["højeste_indbetaling_fra_afståelsen_kroner"],
        100_000
    );
    let xlsx_pbl15a_capital = &xlsx_pbl15a_disposal["passiv_kapital_resultat"];
    assert_eq!(xlsx_pbl15a_capital["fakta_gyldige"], true);
    assert_eq!(xlsx_pbl15a_capital["passive_indtægter_tre_år_kroner"], 0);
    assert_eq!(
        xlsx_pbl15a_capital["samlede_indtægter_tre_år_kroner"],
        900_000
    );
    assert_eq!(
        xlsx_pbl15a_capital["samlede_aktiver_tre_år_kroner"],
        1_800_000
    );
    assert_eq!(
        xlsx_pbl15a_capital["samlede_aktiver_ved_afståelse_kroner"],
        600_000
    );
    assert_eq!(
        xlsx_pbl15a_capital["overvejende_passiv_kapitalanbringelse"],
        false
    );
    let xlsx_seafarer_result = result["results"]
        .as_array()
        .expect("XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-soemandsfradrag-2026")
        .expect("XLSX seafarer-deduction result");
    assert_eq!(
        xlsx_seafarer_result["result"]["ligningsfradrag"]["sømandsfradrag"]
            ["samlet_fradrag_kroner"],
        56_900
    );
    assert_eq!(
        xlsx_seafarer_result["result"]["ligningsfradrag"]["samlet_ligningsfradrag_kroner"],
        56_900
    );
    let seafarer_employment = &xlsx_seafarer_result["result"]["ligningsfradrag"]["sømandsfradrag"]
        ["beskæftigelsesresultater"][0];
    assert_eq!(
        seafarer_employment["fakta"]["ansættelsesforhold_startdato"],
        serde_json::json!({ "år": 2021, "måned": 12, "dag": 31 })
    );
    assert_eq!(
        seafarer_employment["par3a_anvendelse"]["$variant"],
        "Søbl3AAnsættelsesforholdPåbegyndtFør2022"
    );

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
    let kgl_par25_choices = || {
        serde_json::json!({
            "obligationer_på_reguleret_marked": {
                "position_primo": { "$variant": "KglÅrsnettoIntetPar25ValgPrimo" },
                "aktuelt_princip": { "$variant": "KglRealisationsprincip" },
                "ændringstilladelse": {
                    "$variant": "KglÅrsnettoIngenPar25Ændringstilladelse"
                }
            },
            "valutakursændringer": {
                "position_primo": { "$variant": "KglÅrsnettoIntetPar25ValgPrimo" },
                "aktuelt_princip": { "$variant": "KglRealisationsprincip" },
                "ændringstilladelse": {
                    "$variant": "KglÅrsnettoIngenPar25Ændringstilladelse"
                }
            }
        })
    };
    json_input["cases"][0]["case_id"] = Value::String("personskat-abl-personlig-2026".into());
    json_input["cases"][0]["input"] = serde_json::json!({
        "lønmodtager": {
            "skatteår": 2026,
            "kommune": { "$variant": "København" },
            "bruttoløn_kroner": 600_000,
            "personlig_indkomst": {
                "etableringskonto": { "$variant": "UdenEtableringskontoindskud" },
                "underholdsbidrag": { "bidrag": [] },
                "børnebidragslov19_bidrag": { "bidrag": [] },
                "sømandsbeskatning": { "indkomster": [], "skibsårsdrifter": [], "andre_ligningslov7u_indkomster": [], "dødsboskattegrundlag": { "$variant": "Søbl5IntetDødsboskattegrundlag" }, "kulbrinteskattegrundlag": { "$variant": "Søbl5BIntetKulbrinteskattegrundlag" } },
                "ordinære_forhold": {
                    "arbejdsgiverydelser": [],
                    "virksomheder_uden_virksomhedsordning": [],
                    "forenings_og_arbejdsløshedsydelser": []
                }
            },
            "erhvervsbefordring": { "sager": [] },
            "ligningsfradrag": {
                "sømandsfradrag": {
                    "valg": { "$variant": "FravælgSømandsfradrag" },
                    "beskæftigelser": []
                },
                "sømandsbeskatningslov4": { "kildetilknytninger": [] },
                "fiskerfradrag": {
                    "valg": { "$variant": "Ll9GFravælgFiskerfradrag" },
                    "registreringer": [],
                    "arbejdsforhold": [],
                    "fangstture": [],
                    "selvstændige_udgiftsvurderinger": [],
                    "kontingentperiodeundtagelser": [],
                    "kildetilknytninger": []
                },
                "øvrige_lønmodtagerudgifter": {
                    "skatteyderstatus": { "$variant": "Ll9Stk1Lønmodtager" },
                    "udgifter": []
                },
                "befordring": { "forhold": [] },
                "rejser": {
                    "personrolle": { "$variant": "Ll9AAlmindeligLønmodtager" },
                    "rejser": [],
                    "udenlandske_indkomstkilder": [],
                    "arbejdshistorik": {
                        "tidligere_rejser": [],
                        "arbejdsdage": [],
                        "arbejdsstedsafstande": []
                    },
                    "ølogi": { "$variant": "UdenØlogifradrag" },
                    "dobbelt_husførelse": {
                        "$variant": "Ll9AIntetFradragForDobbeltHusførelse"
                    }
                },
                "faglige_kontingenter": {
                    "skatteyderstatus": { "$variant": "Ll13Lønmodtager" },
                    "kontingenter": []
                },
                "arbejdsløshed_efterløn_og_fleksydelse": {
                    "skattepligtsposition": {
                        "$variant": "Pbl49FuldtSkattepligtigOgHjemmehørendeIDanmark"
                    },
                    "bidrag": []
                },
                "gaver": { "gaver": [] }
            },
            "pension": {
                "fødselsdato": { "år": 1990, "måned": 1, "dag": 1 },
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
        "negativ_aktieskat_fremførsel": {
            "hovedperson": { "$variant": "UdenFremførtNegativAktieskat" },
            "ægtefælle": { "$variant": "UdenFremførtNegativAktieskat" }
        },
        "ægtefælle": { "$variant": "UdenÆgtefælle" },
        "årsopgørelse": { "$variant": "UdenÅrsopgørelse" }
    });
    let mut fisher_union_transition_case = json_input["cases"][0].clone();
    fisher_union_transition_case["case_id"] =
        Value::String("personskat-fisker-kontingentovergange-2026".into());
    fisher_union_transition_case["input"]["aktieavance"]["særlige_aktiver"] = serde_json::json!([]);
    fisher_union_transition_case["input"]["lønmodtager"]["ligningsfradrag"]["fiskerfradrag"] = serde_json::json!({
        "valg": { "$variant": "Ll9GVælgFiskerfradrag" },
        "registreringer": [{
            "identifikation": "a-status-forår",
            "status": { "$variant": "Ll9GErhvervsfiskerMedAStatus" },
            "fra_dato": { "år": 2026, "måned": 4, "dag": 1 },
            "til_dato": { "år": 2026, "måned": 6, "dag": 30 }
        }, {
            "identifikation": "a-status-efterår",
            "status": { "$variant": "Ll9GErhvervsfiskerMedAStatus" },
            "fra_dato": { "år": 2026, "måned": 9, "dag": 1 },
            "til_dato": { "år": 2026, "måned": 10, "dag": 31 }
        }],
        "arbejdsforhold": [{
            "identifikation": "ansat-fisker-overgang",
            "erhvervsform": { "$variant": "Ll9GAnsatFisker" },
            "fra_dato": { "år": 2026, "måned": 4, "dag": 1 },
            "til_dato": { "år": 2026, "måned": 10, "dag": 31 }
        }],
        "fangstture": [{
            "identifikation": "fangsttur-overgang",
            "arbejdsforhold_identifikation": "ansat-fisker-overgang",
            "afgang_fra_havn": {
                "dato": { "år": 2026, "måned": 6, "dag": 1 },
                "klokkeslæt": { "time": 6, "minut": 0 }
            },
            "ankomst_til_havn": {
                "dato": { "år": 2026, "måned": 6, "dag": 2 },
                "klokkeslæt": { "time": 6, "minut": 0 }
            }
        }],
        "selvstændige_udgiftsvurderinger": [],
        "kontingentperiodeundtagelser": [{
            "kontingent_identifikation": "kontingent-før-registrering",
            "bedømmelse": {
                "$variant": "Ll9GFørFørsteRegistreringGodkendtEfterKonkretBedømmelse",
                "andet_arbejdsforhold_identifikation": "landarbejde-før-fiskeri",
                "ligningsmæssig_bedømmelsesreference": "TfS-1996-105-DEP/konkret-bedømmelse/før"
            }
        }, {
            "kontingent_identifikation": "kontingent-efter-fuldt-ophør",
            "bedømmelse": {
                "$variant": "Ll9GEfterFuldtOphørGodkendtEfterKonkretBedømmelse",
                "andet_erhverv_identifikation": "nyt-erhverv-efter-fiskeri",
                "ligningsmæssig_bedømmelsesreference": "TfS-1996-105-DEP/konkret-bedømmelse/efter"
            }
        }],
        "kildetilknytninger": [{
            "kilde": {
                "$variant": "Ll9GLigningslov13Forening",
                "forening_identifikation": "fiskerforbund-overgang"
            },
            "arbejdstilknytning": {
                "$variant": "Ll9GTilknyttetErhvervsfiskerarbejde",
                "arbejdsforhold_identifikation": "ansat-fisker-overgang"
            }
        }]
    });
    fisher_union_transition_case["input"]["lønmodtager"]["ligningsfradrag"]
        ["faglige_kontingenter"] = serde_json::json!({
        "skatteyderstatus": { "$variant": "Ll13Lønmodtager" },
        "kontingenter": [{
            "identifikation": "kontingent-før-registrering",
            "forening_identifikation": "fiskerforbund-overgang",
            "indkomstår": 2026,
            "periode": {
                "fra_dato": { "år": 2026, "måned": 1, "dag": 1 },
                "til_dato": { "år": 2026, "måned": 3, "dag": 31 }
            },
            "foreningsart": { "$variant": "Ll13Fagforening" },
            "betalt_kontingent_kroner": 1000,
            "foreningens_opgjorte_andel_til_faglige_økonomiske_interesser_kroner": 1000,
            "foreningens_hovedformål_er_erhvervsgruppens_økonomiske_interesser": true,
            "skatteyder_hører_til_erhvervsgruppen": true,
            "indberetningsstatus": { "$variant": "Ll13IndberettetEfterSkatteindberetningslov31" }
        }, {
            "identifikation": "kontingent-under-registrering",
            "forening_identifikation": "fiskerforbund-overgang",
            "indkomstår": 2026,
            "periode": {
                "fra_dato": { "år": 2026, "måned": 4, "dag": 1 },
                "til_dato": { "år": 2026, "måned": 6, "dag": 30 }
            },
            "foreningsart": { "$variant": "Ll13Fagforening" },
            "betalt_kontingent_kroner": 1000,
            "foreningens_opgjorte_andel_til_faglige_økonomiske_interesser_kroner": 1000,
            "foreningens_hovedformål_er_erhvervsgruppens_økonomiske_interesser": true,
            "skatteyder_hører_til_erhvervsgruppen": true,
            "indberetningsstatus": { "$variant": "Ll13IndberettetEfterSkatteindberetningslov31" }
        }, {
            "identifikation": "kontingent-midlertidigt-uden-registrering",
            "forening_identifikation": "fiskerforbund-overgang",
            "indkomstår": 2026,
            "periode": {
                "fra_dato": { "år": 2026, "måned": 7, "dag": 1 },
                "til_dato": { "år": 2026, "måned": 8, "dag": 31 }
            },
            "foreningsart": { "$variant": "Ll13Fagforening" },
            "betalt_kontingent_kroner": 1000,
            "foreningens_opgjorte_andel_til_faglige_økonomiske_interesser_kroner": 1000,
            "foreningens_hovedformål_er_erhvervsgruppens_økonomiske_interesser": true,
            "skatteyder_hører_til_erhvervsgruppen": true,
            "indberetningsstatus": { "$variant": "Ll13IndberettetEfterSkatteindberetningslov31" }
        }, {
            "identifikation": "kontingent-efter-fuldt-ophør",
            "forening_identifikation": "fiskerforbund-overgang",
            "indkomstår": 2026,
            "periode": {
                "fra_dato": { "år": 2026, "måned": 11, "dag": 1 },
                "til_dato": { "år": 2026, "måned": 12, "dag": 31 }
            },
            "foreningsart": { "$variant": "Ll13Fagforening" },
            "betalt_kontingent_kroner": 1000,
            "foreningens_opgjorte_andel_til_faglige_økonomiske_interesser_kroner": 1000,
            "foreningens_hovedformål_er_erhvervsgruppens_økonomiske_interesser": true,
            "skatteyder_hører_til_erhvervsgruppen": true,
            "indberetningsstatus": { "$variant": "Ll13IndberettetEfterSkatteindberetningslov31" }
        }]
    });
    let mut fisher_cross_year_case = fisher_union_transition_case.clone();
    fisher_cross_year_case["case_id"] =
        Value::String("personskat-fisker-nytårsfordeling-2026".into());
    fisher_cross_year_case["input"]["lønmodtager"]["ligningsfradrag"]["fiskerfradrag"] = serde_json::json!({
        "valg": { "$variant": "Ll9GVælgFiskerfradrag" },
        "registreringer": [{
            "identifikation": "a-status-over-årsskifte",
            "status": { "$variant": "Ll9GErhvervsfiskerMedAStatus" },
            "fra_dato": { "år": 2025, "måned": 12, "dag": 1 },
            "til_dato": { "år": 2026, "måned": 12, "dag": 31 }
        }],
        "arbejdsforhold": [{
            "identifikation": "ansat-fisker-over-årsskifte",
            "erhvervsform": { "$variant": "Ll9GAnsatFisker" },
            "fra_dato": { "år": 2025, "måned": 12, "dag": 1 },
            "til_dato": { "år": 2026, "måned": 12, "dag": 31 }
        }],
        "fangstture": [{
            "identifikation": "fangsttur-over-årsskifte",
            "arbejdsforhold_identifikation": "ansat-fisker-over-årsskifte",
            "afgang_fra_havn": {
                "dato": { "år": 2025, "måned": 12, "dag": 31 },
                "klokkeslæt": { "time": 18, "minut": 0 }
            },
            "ankomst_til_havn": {
                "dato": { "år": 2026, "måned": 1, "dag": 2 },
                "klokkeslæt": { "time": 0, "minut": 0 }
            }
        }],
        "selvstændige_udgiftsvurderinger": [],
        "kontingentperiodeundtagelser": [],
        "kildetilknytninger": []
    });
    fisher_cross_year_case["input"]["lønmodtager"]["ligningsfradrag"]["faglige_kontingenter"] = serde_json::json!({
        "skatteyderstatus": { "$variant": "Ll13Lønmodtager" },
        "kontingenter": []
    });
    let fisher_travel = |identifikation: &str, arbejdssted: &str, startdag: i64| {
        serde_json::json!({
            "identifikation": identifikation,
            "indkomstår": 2026,
            "startdato": { "år": 2026, "måned": 6, "dag": startdag },
            "rejseart": { "$variant": "Ll9ATjenesterejse" },
            "arbejdssted_identifikation": arbejdssted,
            "arbejdsstedskarakter": {
                "$variant": "Ll9AStedbundetArbejdssted",
                "tidsbegrænsning": {
                    "$variant": "Ll9ATidsbegrænsetTilKonkretOpgavesFærdiggørelse"
                }
            },
            "overnatningsforhold": {
                "$variant": "Ll9AIngenMulighedForOvernatningPåSædvanligBopæl",
                "afstand_ad_normal_transportvej_kilometer": 200,
                "korteste_transporttid_hver_vej_minutter": 180
            },
            "hverv": { "$variant": "Ll9AAlmindeligtHverv" },
            "varighed_minutter": 1440,
            "kost": {
                "dækning": { "$variant": "Ll9AKostIkkeDækketEfterRegning" },
                "godtgørelsesudbetaling": {
                    "$variant": "Ll9AUopdeltGodtgørelse",
                    "udbetalt_kroner": 300
                },
                "fri_morgenmad_antal": 0,
                "fri_frokost_antal": 0,
                "fri_aftensmad_antal": 0,
                "dokumenterede_kostudgifter_før_arbejdsgiverdækning_kroner": 0,
                "fradragsprincip": { "$variant": "Ll9AKostfradragMedStandardsats" }
            },
            "logidøgn": [{
                "rejsedøgnsnummer": 1,
                "dækning": { "$variant": "Ll9ALogiIkkeDækketAfArbejdsgiver" },
                "godtgørelsesudbetaling": {
                    "$variant": "Ll9AUopdeltGodtgørelse",
                    "udbetalt_kroner": 100
                },
                "dokumenteret_logiudgift_betalt_før_refusion_kroner": 0,
                "fradragsprincip": { "$variant": "Ll9ALogifradragMedStandardsats" }
            }],
            "kontrol": { "$variant": "Ll9AArbejdsgiverkontrolUdført" },
            "lønomlægning": { "$variant": "Ll9AGodtgørelseUdenLønomlægning" },
            "indkomstforhold": { "$variant": "Ll9ADanskSkattepligtigArbejdsindkomst" }
        })
    };
    let mut fisher_mixed_travel_case = json_input["cases"][0].clone();
    fisher_mixed_travel_case["case_id"] =
        Value::String("personskat-fisker-blandede-rejser-2026".into());
    fisher_mixed_travel_case["input"]["aktieavance"]["særlige_aktiver"] = serde_json::json!([]);
    fisher_mixed_travel_case["input"]["lønmodtager"]["bruttoløn_kroner"] =
        serde_json::json!(600_000);
    fisher_mixed_travel_case["input"]["lønmodtager"]["ligningsfradrag"]["fiskerfradrag"] = serde_json::json!({
        "valg": { "$variant": "Ll9GVælgFiskerfradrag" },
        "registreringer": [{
            "identifikation": "blandede-rejser-a-status",
            "status": { "$variant": "Ll9GErhvervsfiskerMedAStatus" },
            "fra_dato": { "år": 2026, "måned": 1, "dag": 1 },
            "til_dato": { "år": 2026, "måned": 12, "dag": 31 }
        }],
        "arbejdsforhold": [{
            "identifikation": "ansat-fisker",
            "erhvervsform": { "$variant": "Ll9GAnsatFisker" },
            "fra_dato": { "år": 2026, "måned": 1, "dag": 1 },
            "til_dato": { "år": 2026, "måned": 12, "dag": 31 }
        }],
        "fangstture": [{
            "identifikation": "blandede-rejser-fangsttur",
            "arbejdsforhold_identifikation": "ansat-fisker",
            "afgang_fra_havn": {
                "dato": { "år": 2026, "måned": 6, "dag": 1 },
                "klokkeslæt": { "time": 6, "minut": 0 }
            },
            "ankomst_til_havn": {
                "dato": { "år": 2026, "måned": 6, "dag": 2 },
                "klokkeslæt": { "time": 6, "minut": 0 }
            }
        }],
        "selvstændige_udgiftsvurderinger": [],
        "kontingentperiodeundtagelser": [],
        "kildetilknytninger": [{
            "kilde": {
                "$variant": "Ll9GLigningslov9ARejse",
                "kildeidentifikation": "fisker-rejse"
            },
            "arbejdstilknytning": {
                "$variant": "Ll9GTilknyttetErhvervsfiskerarbejde",
                "arbejdsforhold_identifikation": "ansat-fisker"
            }
        }, {
            "kilde": {
                "$variant": "Ll9GLigningslov9ARejse",
                "kildeidentifikation": "andet-job-rejse"
            },
            "arbejdstilknytning": {
                "$variant": "Ll9GTilknyttetAndetArbejde",
                "arbejdsforhold_identifikation": "andet-job"
            }
        }]
    });
    fisher_mixed_travel_case["input"]["lønmodtager"]["ligningsfradrag"]["rejser"] = serde_json::json!({
        "personrolle": { "$variant": "Ll9AAlmindeligLønmodtager" },
        "rejser": [
            fisher_travel("fisker-rejse", "ansat-fisker", 10),
            fisher_travel("andet-job-rejse", "andet-job", 20)
        ],
        "udenlandske_indkomstkilder": [],
        "arbejdshistorik": {
            "tidligere_rejser": [],
            "arbejdsdage": [],
            "arbejdsstedsafstande": [{
                "fra_arbejdssted_identifikation": "ansat-fisker",
                "til_arbejdssted_identifikation": "andet-job",
                "gældende_fra": { "år": 2026, "måned": 1, "dag": 1 },
                "afstand_ad_normal_transportvej_kilometer": 20
            }]
        },
        "ølogi": {
            "$variant": "MedØlogifradrag",
            "bopæl": {
                "kommune": { "$variant": "Samsø" },
                "ø": { "$variant": "Ll9AAndenDanskØ", "navn": "Samsø" },
                "vejforbindelse": { "$variant": "Ll9AIngenFastVejforbindelseFraØen" }
            },
            "arbejdsforhold": [{
                "arbejdssted_identifikation": "andet-job",
                "arbejdsstedskarakter": { "$variant": "Ll9AØlogiFastArbejdssted" },
                "overnatningsforhold": {
                    "$variant": "Ll9AØlogiIngenMulighedForOvernatningPåSædvanligBopæl",
                    "afstand_ad_normal_transportvej_kilometer": 100,
                    "korteste_transporttid_hver_vej_minutter": 180
                },
                "hverv": { "$variant": "Ll9AAlmindeligtHverv" },
                "ophold": [{
                    "identifikation": "andet-job-ølogi",
                    "starttidspunkt": {
                        "dato": { "år": 2026, "måned": 7, "dag": 1 },
                        "klokkeslæt": { "time": 0, "minut": 0 }
                    },
                    "sluttidspunkt_eksklusiv": {
                        "dato": { "år": 2026, "måned": 7, "dag": 2 },
                        "klokkeslæt": { "time": 0, "minut": 0 }
                    },
                    "udgiftsforhold": { "$variant": "Ll9AØlogiEgenUdgiftAfholdt" }
                }]
            }]
        },
        "dobbelt_husførelse": { "$variant": "Ll9AIntetFradragForDobbeltHusførelse" }
    });
    let mut fisher_mixed_travel_no_election_case = fisher_mixed_travel_case.clone();
    fisher_mixed_travel_no_election_case["case_id"] =
        Value::String("personskat-fisker-blandede-rejser-uden-valg-2026".into());
    fisher_mixed_travel_no_election_case["input"]["lønmodtager"]["ligningsfradrag"]
        ["fiskerfradrag"]["valg"] = serde_json::json!({
        "$variant": "Ll9GFravælgFiskerfradrag"
    });
    let mut seafarer_commute_case = json_input["cases"][0].clone();
    seafarer_commute_case["case_id"] =
        Value::String("personskat-soemand-blandet-befordring-2026".into());
    seafarer_commute_case["input"]["aktieavance"]["særlige_aktiver"] = serde_json::json!([]);
    seafarer_commute_case["input"]["lønmodtager"]["bruttoløn_kroner"] = serde_json::json!(200_000);
    seafarer_commute_case["input"]["lønmodtager"]["ligningsfradrag"]["sømandsfradrag"] = serde_json::json!({
        "valg": { "$variant": "AnvendSømandsfradrag" },
        "beskæftigelser": [{
            "identifikation": "blandet-fragtskib-json-xlsx",
            "indkomstår": 2026,
            "ansættelsesforhold_startdato": { "år": 2026, "måned": 1, "dag": 1 },
            "arbejdssted": {
                "$variant": "ArbejdePåSkib",
                "bruttotonnage": 500,
                "anvendelse": { "$variant": "ErhvervsmæssigBefordringAfGods" }
            },
            "fart": { "$variant": "UdenForBegrænsetFart" },
            "hjemsted": { "$variant": "RegistreretMedHjemstedIDanmark" },
            "flag": { "$variant": "FlagFraEUEØSStat" },
            "forhyringsvilkår": { "$variant": "SædvanligeForhyringsvilkårForSøfolk" },
            "fuldtidsomregnede_sødage_hundrededele": 18_250
        }]
    });
    seafarer_commute_case["input"]["lønmodtager"]["ligningsfradrag"]["sømandsbeskatningslov4"] = serde_json::json!({
        "kildetilknytninger": [{
            "kilde": {
                "$variant": "Søbl4Ligningslov9COg9DBefordringsforhold",
                "kildeidentifikation": "sømandsbefordring-json-xlsx"
            },
            "arbejdstilknytning": {
                "$variant": "Søbl4Sømandsbeskæftigelsesperiode",
                "beskæftigelsesidentifikation": "blandet-fragtskib-json-xlsx"
            }
        }, {
            "kilde": {
                "$variant": "Søbl4Ligningslov9COg9DBefordringsforhold",
                "kildeidentifikation": "landbefordring-json-xlsx"
            },
            "arbejdstilknytning": {
                "$variant": "Søbl4AndetArbejdsforhold",
                "arbejdsforhold_identifikation": "landarbejde-json-xlsx"
            }
        }]
    });
    let commute_facts = |identification: &str, destination: &str| {
        serde_json::json!({
            "identifikation": identification,
            "befordringsmål_identifikation": destination,
            "arbejdsdage": 100,
            "daglige_befordringskilometer": 100,
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
        })
    };
    seafarer_commute_case["input"]["lønmodtager"]["ligningsfradrag"]["befordring"] = serde_json::json!({
        "forhold": [
            commute_facts(
                "sømandsbefordring-json-xlsx",
                "blandet-fragtskib-json-xlsx"
            ),
            commute_facts("landbefordring-json-xlsx", "landarbejde-json-xlsx")
        ]
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
    interest_case["input"]["lønmodtager"]["ligningsfradrag"]["befordring"] = serde_json::json!({
        "forhold": [{
            "identifikation": "befordring-arbejde-1",
            "befordringsmål_identifikation": "arbejdsforhold-1",
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
        }]
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
                    "par25_valg": kgl_par25_choices(),
                    "fordringer": [],
                    "valutainstrumenter": [],
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
                    "underholdsbidrag": { "bidrag": [] },
                    "børnebidragslov19_bidrag": { "bidrag": [] },
                    "sømandsbeskatning": { "indkomster": [], "skibsårsdrifter": [], "andre_ligningslov7u_indkomster": [], "dødsboskattegrundlag": { "$variant": "Søbl5IntetDødsboskattegrundlag" }, "kulbrinteskattegrundlag": { "$variant": "Søbl5BIntetKulbrinteskattegrundlag" } },
                    "ordinære_forhold": {
                        "arbejdsgiverydelser": [],
                        "virksomheder_uden_virksomhedsordning": [],
                        "forenings_og_arbejdsløshedsydelser": []
                    }
                },
                "erhvervsbefordring": { "sager": [] },
                "ligningsfradrag": {
                    "sømandsfradrag": {
                        "valg": { "$variant": "FravælgSømandsfradrag" },
                        "beskæftigelser": []
                    },
                    "sømandsbeskatningslov4": { "kildetilknytninger": [] },
                    "fiskerfradrag": {
                        "valg": { "$variant": "Ll9GFravælgFiskerfradrag" },
                        "registreringer": [],
                        "arbejdsforhold": [],
                        "fangstture": [],
                        "selvstændige_udgiftsvurderinger": [],
                        "kontingentperiodeundtagelser": [],
                        "kildetilknytninger": []
                    },
                    "øvrige_lønmodtagerudgifter": {
                        "skatteyderstatus": { "$variant": "Ll9Stk1Lønmodtager" },
                        "udgifter": []
                    },
                    "befordring": { "forhold": [] },
                    "rejser": {
                        "personrolle": { "$variant": "Ll9AAlmindeligLønmodtager" },
                        "rejser": [],
                        "udenlandske_indkomstkilder": [],
                        "arbejdshistorik": {
                            "tidligere_rejser": [],
                            "arbejdsdage": [],
                            "arbejdsstedsafstande": []
                        },
                        "ølogi": { "$variant": "UdenØlogifradrag" },
                        "dobbelt_husførelse": {
                            "$variant": "Ll9AIntetFradragForDobbeltHusførelse"
                        }
                    },
                    "faglige_kontingenter": {
                        "skatteyderstatus": { "$variant": "Ll13Lønmodtager" },
                        "kontingenter": []
                    },
                    "arbejdsløshed_efterløn_og_fleksydelse": {
                        "skattepligtsposition": {
                            "$variant": "Pbl49FuldtSkattepligtigOgHjemmehørendeIDanmark"
                        },
                        "bidrag": []
                    },
                    "gaver": { "gaver": [] }
                },
                "pension": {
                    "fødselsdato": { "år": 1990, "måned": 1, "dag": 1 },
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
        "samlevende_ved_indkomstårets_udløb": true,
        "kildeskat25a_fordelinger": []
    });
    spouse_case["input"]["ægtefælle"]["fakta"]["ejendomsskatter"] =
        spouse_case["input"]["ejendomsskatter"].clone();
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
        .push(property_tax_case.clone());
    let no_pension_context_2024 = serde_json::json!({
        "indkomstår": 2024,
        "kildefakta": {
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
        "egen_indkomst": {
            "personlig_indkomst_kroner": 0,
            "kapitalindkomst_kroner": 0,
            "aktieindkomst_kroner": 0
        },
        "ægtefælles_indkomst": {
            "personlig_indkomst_kroner": 0,
            "kapitalindkomst_kroner": 0,
            "aktieindkomst_kroner": 0
        },
        "gift_og_samlevende_ved_indkomstårets_udgang": false
    });
    let relief_facts = |acquisition_year: i64, acquisition_month: i64, acquisition_day: i64| {
        serde_json::json!({
            "ejerskabshistorik": {
                "oprindelig_erhvervelsesdato": {
                    "år": acquisition_year,
                    "måned": acquisition_month,
                    "dag": acquisition_day
                },
                "ejerskifter": []
            },
            "boliganvendelse": { "$variant": "EjskHelårsbolig" },
            "selvstændige_boligenheder": 1,
            "ejendomsform": { "$variant": "EjskIkkeEjerlejlighed" },
            "fredet_og_omfattet_af_ligningslovens_par15k": false,
            "par24_beregningsgrundlag": {
                "$variant": "EjskPar24SammeVærdiSomPar13"
            },
            "pensionistsuccession": {
                "$variant": "EjskIngenPensionistsuccession"
            },
            "udenlandske_ejendomsskatter": []
        })
    };
    let partial_exemption_ordinary = serde_json::json!({
        "identifikation": "delvist-fritaget-json-xlsx",
        "kommune": { "$variant": "København" },
        "kategori": { "$variant": "EjskEnBoligenhed" },
        "beliggenhed": { "$variant": "EjskDanmark" },
        "erhvervsmæssigt_udlejet": false,
        "særlige_betingelser_for_nr6_til_nr8_opfyldt": true,
        "ejendomsværdi_kroner": 0,
        "grundværdi_kroner": 1_500_000,
        "produktionsjord": false,
        "ejendomsværdiskatteperiode": {
            "$variant": "HeleEjendomsskatteåret"
        },
        "grundskyldsperiode": { "$variant": "HeleEjendomsskatteåret" },
        "ejerandel_basispoint": 10_000
    });
    let partial_exemption_relief = relief_facts(2020, 1, 1);
    let mut partial_exemption_case = property_tax_case.clone();
    partial_exemption_case["case_id"] =
        Value::String("personskat-ejendomsskat-delvis-fritagelse-2025".into());
    partial_exemption_case["input"]["ejendomsskatter"]["ejendomme"] = serde_json::json!([{
        "ordinært_grundlag": partial_exemption_ordinary.clone(),
        "nedslagsfakta": partial_exemption_relief.clone(),
        "overgangsomfang": {
            "vurderingskategori": {
                "$variant": "EjskEjerboligEfterEjendomsvurderingslovensPar3Stk1Nr1"
            },
            "ejerkreds": { "$variant": "EjskKunFysiskeEjere" }
        },
        "overgangsvurderinger": {
            "rabat": {
                "$variant": "EjskRabatvurderingerOplyst",
                "fakta": {
                    "eget_rabatgrundlag_2024": {
                        "kontekst_2024": no_pension_context_2024,
                        "ny_lov_helårsgrundlag": partial_exemption_ordinary,
                        "ny_lov_nedslagsfakta": partial_exemption_relief,
                        "tidligere_ejendomsværdiskat": {
                            "ejendomsværdi_året_før_kroner": 0,
                            "ejendomsværdi_2001_kroner": 0,
                            "ejendomsværdi_2002_kroner": 0,
                            "historisk_begrænsning": {
                                "foregående_indkomstårs_ejendomsværdiskat_øre": null,
                                "par9b_nedsættelse_øre": 0,
                                "vurderet_helt_eller_delvis_benyttet_til_ejerbolig": true,
                                "ejerlejlighed_frigjort_for_lejemål": false,
                                "ombygning_over_100_procent": false
                            },
                            "udenlandske_ejendomsskatter": []
                        },
                        "tidligere_grundskyld": {
                            "grundværdi_efter_fradrag_og_fritagelser_kroner": 200_000,
                            "foregående_års_afgiftspligtige_grundværdi_kroner": 200_000,
                            "grundskyld_promille_2023_tiendedele": 100
                        },
                        "byggeri": { "$variant": "EjskIngenNyEllerOmbygning" },
                        "grundskyld_fritaget_basispoint": 5_000,
                        "grundskyld_kan_fordeles_på_samme_boligenhed": false
                    },
                    "hændelser": []
                }
            },
            "stigningsbegrænsning": {
                "$variant": "EjskIngenStigningsvurderingerOplyst"
            }
        }
    }]);
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(partial_exemption_case);
    let temporary_rental_identifier = "midlertidig-udlejning-json-xlsx";
    let temporary_rental_relief = relief_facts(1998, 7, 1);
    let temporary_rental_ordinary_2024 = serde_json::json!({
        "identifikation": temporary_rental_identifier,
        "kommune": { "$variant": "København" },
        "kategori": { "$variant": "EjskEnBoligenhed" },
        "beliggenhed": { "$variant": "EjskDanmark" },
        "erhvervsmæssigt_udlejet": false,
        "særlige_betingelser_for_nr6_til_nr8_opfyldt": true,
        "ejendomsværdi_kroner": 2_250_000,
        "grundværdi_kroner": 0,
        "produktionsjord": false,
        "ejendomsværdiskatteperiode": {
            "$variant": "HeleEjendomsskatteåret"
        },
        "grundskyldsperiode": { "$variant": "HeleEjendomsskatteåret" },
        "ejerandel_basispoint": 10_000
    });
    let temporary_rental_current = serde_json::json!({
        "identifikation": temporary_rental_identifier,
        "kommune": { "$variant": "København" },
        "kategori": { "$variant": "EjskEnBoligenhed" },
        "beliggenhed": { "$variant": "EjskDanmark" },
        "erhvervsmæssigt_udlejet": false,
        "særlige_betingelser_for_nr6_til_nr8_opfyldt": true,
        "ejendomsværdi_kroner": 2_250_000,
        "grundværdi_kroner": 0,
        "produktionsjord": false,
        "ejendomsværdiskatteperiode": {
            "$variant": "EjendomsskatIIntervaller",
            "intervaller": [{
                "fra_dato": { "år": 2025, "måned": 2, "dag": 1 },
                "til_dato": { "år": 2025, "måned": 2, "dag": 28 }
            }]
        },
        "grundskyldsperiode": { "$variant": "HeleEjendomsskatteåret" },
        "ejerandel_basispoint": 10_000
    });
    let mut temporary_rental_case = property_tax_case.clone();
    temporary_rental_case["case_id"] =
        Value::String("personskat-ejendomsskat-midlertidig-udlejning-2025".into());
    temporary_rental_case["input"]["ejendomsskatter"]["ejendomme"] = serde_json::json!([{
        "ordinært_grundlag": temporary_rental_current,
        "nedslagsfakta": temporary_rental_relief.clone(),
        "overgangsomfang": {
            "vurderingskategori": {
                "$variant": "EjskEjerboligEfterEjendomsvurderingslovensPar3Stk1Nr1"
            },
            "ejerkreds": { "$variant": "EjskKunFysiskeEjere" }
        },
        "overgangsvurderinger": {
            "rabat": {
                "$variant": "EjskRabatvurderingerOplyst",
                "fakta": {
                    "eget_rabatgrundlag_2024": {
                        "kontekst_2024": no_pension_context_2024.clone(),
                        "ny_lov_helårsgrundlag": temporary_rental_ordinary_2024,
                        "ny_lov_nedslagsfakta": temporary_rental_relief,
                        "tidligere_ejendomsværdiskat": {
                            "ejendomsværdi_året_før_kroner": 1_062_500,
                            "ejendomsværdi_2001_kroner": 850_000,
                            "ejendomsværdi_2002_kroner": 850_000,
                            "historisk_begrænsning": {
                                "foregående_indkomstårs_ejendomsværdiskat_øre": null,
                                "par9b_nedsættelse_øre": 0,
                                "vurderet_helt_eller_delvis_benyttet_til_ejerbolig": true,
                                "ejerlejlighed_frigjort_for_lejemål": false,
                                "ombygning_over_100_procent": false
                            },
                            "udenlandske_ejendomsskatter": []
                        },
                        "tidligere_grundskyld": {
                            "grundværdi_efter_fradrag_og_fritagelser_kroner": 0,
                            "foregående_års_afgiftspligtige_grundværdi_kroner": 0,
                            "grundskyld_promille_2023_tiendedele": 0
                        },
                        "byggeri": { "$variant": "EjskIngenNyEllerOmbygning" },
                        "grundskyld_fritaget_basispoint": 0,
                        "grundskyld_kan_fordeles_på_samme_boligenhed": true
                    },
                    "hændelser": [{
                        "dato": { "år": 2025, "måned": 1, "dag": 1 },
                        "art": {
                            "$variant": "EjskBoligKanIkkeTjeneTilBoligForEjeren"
                        }
                    }, {
                        "dato": { "år": 2025, "måned": 2, "dag": 1 },
                        "art": {
                            "$variant": "EjskBoligKanIgenTjeneTilBoligForEjeren"
                        }
                    }, {
                        "dato": { "år": 2025, "måned": 3, "dag": 1 },
                        "art": {
                            "$variant": "EjskBoligKanIkkeTjeneTilBoligForEjeren"
                        }
                    }]
                }
            },
            "stigningsbegrænsning": {
                "$variant": "EjskIngenStigningsvurderingerOplyst"
            }
        }
    }]);
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(temporary_rental_case);
    let spouse_rebate_identifier = "aegtefaellemodtagelse-json-xlsx";
    let spouse_rebate_ordinary_2024 = serde_json::json!({
        "identifikation": spouse_rebate_identifier,
        "kommune": { "$variant": "København" },
        "kategori": { "$variant": "EjskEnBoligenhed" },
        "beliggenhed": { "$variant": "EjskDanmark" },
        "erhvervsmæssigt_udlejet": false,
        "særlige_betingelser_for_nr6_til_nr8_opfyldt": true,
        "ejendomsværdi_kroner": 2_250_000,
        "grundværdi_kroner": 0,
        "produktionsjord": false,
        "ejendomsværdiskatteperiode": {
            "$variant": "HeleEjendomsskatteåret"
        },
        "grundskyldsperiode": { "$variant": "HeleEjendomsskatteåret" },
        "ejerandel_basispoint": 5_000
    });
    let spouse_rebate_source_2024 = serde_json::json!({
        "kontekst_2024": no_pension_context_2024,
        "ny_lov_helårsgrundlag": spouse_rebate_ordinary_2024,
        "ny_lov_nedslagsfakta": relief_facts(1998, 7, 1),
        "tidligere_ejendomsværdiskat": {
            "ejendomsværdi_året_før_kroner": 1_062_500,
            "ejendomsværdi_2001_kroner": 850_000,
            "ejendomsværdi_2002_kroner": 850_000,
            "historisk_begrænsning": {
                "foregående_indkomstårs_ejendomsværdiskat_øre": null,
                "par9b_nedsættelse_øre": 0,
                "vurderet_helt_eller_delvis_benyttet_til_ejerbolig": true,
                "ejerlejlighed_frigjort_for_lejemål": false,
                "ombygning_over_100_procent": false
            },
            "udenlandske_ejendomsskatter": []
        },
        "tidligere_grundskyld": {
            "grundværdi_efter_fradrag_og_fritagelser_kroner": 0,
            "foregående_års_afgiftspligtige_grundværdi_kroner": 0,
            "grundskyld_promille_2023_tiendedele": 0
        },
        "byggeri": { "$variant": "EjskIngenNyEllerOmbygning" },
        "grundskyld_fritaget_basispoint": 0,
        "grundskyld_kan_fordeles_på_samme_boligenhed": true
    });
    let mut spouse_rebate_recipient_relief = relief_facts(1998, 7, 1);
    spouse_rebate_recipient_relief["ejerskabshistorik"]["ejerskifter"] = serde_json::json!([{
        "dato": { "år": 2025, "måned": 7, "dag": 1 },
        "art": { "$variant": "EjskOverdragelseMellemÆgtefæller" }
    }]);
    let mut spouse_rebate_recipient_case = property_tax_case.clone();
    spouse_rebate_recipient_case["case_id"] =
        Value::String("personskat-ejendomsskat-aegtefaellemodtager-2025".into());
    spouse_rebate_recipient_case["input"]["ejendomsskatter"]["ejendomme"] = serde_json::json!([{
        "ordinært_grundlag": {
            "identifikation": spouse_rebate_identifier,
            "kommune": { "$variant": "København" },
            "kategori": { "$variant": "EjskEnBoligenhed" },
            "beliggenhed": { "$variant": "EjskDanmark" },
            "erhvervsmæssigt_udlejet": false,
            "særlige_betingelser_for_nr6_til_nr8_opfyldt": true,
            "ejendomsværdi_kroner": 2_250_000,
            "grundværdi_kroner": 0,
            "produktionsjord": false,
            "ejendomsværdiskatteperiode": {
                "$variant": "EjendomsskatFraOgMed",
                "dato": { "år": 2025, "måned": 7, "dag": 1 }
            },
            "grundskyldsperiode": {
                "$variant": "EjendomsskatFraOgMed",
                "dato": { "år": 2025, "måned": 7, "dag": 1 }
            },
            "ejerandel_basispoint": 5_000
        },
        "nedslagsfakta": spouse_rebate_recipient_relief,
        "overgangsomfang": {
            "vurderingskategori": {
                "$variant": "EjskEjerboligEfterEjendomsvurderingslovensPar3Stk1Nr1"
            },
            "ejerkreds": { "$variant": "EjskKunFysiskeEjere" }
        },
        "overgangsvurderinger": {
            "rabat": {
                "$variant": "EjskRabatvurderingerOplyst",
                "fakta": {
                    "eget_rabatgrundlag_2024": null,
                    "hændelser": [{
                        "dato": { "år": 2025, "måned": 7, "dag": 1 },
                        "art": {
                            "$variant": "EjskÆgtefælleoverdragelse",
                            "grund": {
                                "$variant": "EjskOverdragelseMellemÆgtefæller"
                            },
                            "retning": {
                                "$variant": "EjskModtagerRabatFraÆgtefælle",
                                "ny_ejerandel_basispoint": 5_000,
                                "overtaget_rabatgrundlag": {
                                    "overgangsomfang": {
                                        "vurderingskategori": {
                                            "$variant": "EjskEjerboligEfterEjendomsvurderingslovensPar3Stk1Nr1"
                                        },
                                        "ejerkreds": {
                                            "$variant": "EjskKunFysiskeEjere"
                                        }
                                    },
                                    "rabat_2024": spouse_rebate_source_2024
                                }
                            }
                        }
                    }]
                }
            },
            "stigningsbegrænsning": {
                "$variant": "EjskIngenStigningsvurderingerOplyst"
            }
        }
    }]);
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(spouse_rebate_recipient_case);
    let mut partial_year_cap_case = property_tax_case.clone();
    partial_year_cap_case["case_id"] =
        Value::String("personskat-ejendomsskat-halvårsloft-2025".into());
    partial_year_cap_case["input"]["ejendomsskatter"]["ejendomme"] = serde_json::json!([{
        "ordinært_grundlag": {
            "identifikation": "halvårsloft-json-xlsx",
            "kommune": { "$variant": "København" },
            "kategori": { "$variant": "EjskAndenEjendom" },
            "beliggenhed": { "$variant": "EjskDanmark" },
            "erhvervsmæssigt_udlejet": false,
            "særlige_betingelser_for_nr6_til_nr8_opfyldt": true,
            "ejendomsværdi_kroner": 0,
            "grundværdi_kroner": 2_000_000,
            "produktionsjord": false,
            "ejendomsværdiskatteperiode": {
                "$variant": "HeleEjendomsskatteåret"
            },
            "grundskyldsperiode": {
                "$variant": "EjendomsskatFraOgMed",
                "dato": { "år": 2025, "måned": 7, "dag": 1 }
            },
            "ejerandel_basispoint": 10_000
        },
        "nedslagsfakta": relief_facts(2025, 7, 1),
        "overgangsomfang": {
            "vurderingskategori": { "$variant": "EjskAndenVurderingskategori" },
            "ejerkreds": { "$variant": "EjskKunFysiskeEjere" }
        },
        "overgangsvurderinger": {
            "rabat": { "$variant": "EjskIngenRabatvurderingerOplyst" },
            "stigningsbegrænsning": {
                "$variant": "EjskStigningsvurderingerOplyst",
                "fakta": {
                    "tidligere_grundskyld_2024": {
                        "grundværdi_efter_fradrag_og_fritagelser_kroner": 200_000,
                        "foregående_års_afgiftspligtige_grundværdi_kroner": 200_000,
                        "grundskyld_promille_2023_tiendedele": 260
                    },
                    "tidligere_år": [{
                        "kalenderår": 2024,
                        "kommune": { "$variant": "København" },
                        "grundværdi_kroner": 1_500_000,
                        "produktionsjord": false,
                        "ejerandel_basispoint": 10_000,
                        "almen_bolig": false,
                        "hændelse": { "$variant": "EjskIngenStigningshændelse" }
                    }],
                    "aktuel_almen_bolig": false,
                    "aktuel_hændelse": { "$variant": "EjskIngenStigningshændelse" }
                }
            }
        }
    }]);
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(partial_year_cap_case);
    let mut spouse_property_credit_case = property_tax_case.clone();
    spouse_property_credit_case["case_id"] =
        Value::String("personskat-par8a-aegtefaelle-ejendomsskatter-2025".into());
    spouse_property_credit_case["input"]["ejendomsskatter"] =
        json_input["cases"][0]["input"]["ejendomsskatter"].clone();
    spouse_property_credit_case["input"]["ægtefælle"] = par37_spouse_relationship.clone();
    spouse_property_credit_case["input"]["ægtefælle"]["fakta"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": [],
        "udbytter": []
    });
    spouse_property_credit_case["input"]["ægtefælle"]["fakta"]["ejendomsskatter"] =
        property_tax_case["input"]["ejendomsskatter"].clone();
    spouse_property_credit_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": {
            "$variant": "MedOrdinærtAktieår",
            "input": {
                "indkomstår": 2025,
                "hændelsesforløb": [{
                    "position_primo": {
                        "selskabsidentifikation": "DK-PAR8A-AEGTEFAELLE-EJENDOM",
                        "kapitalmængde": {
                            "$variant": "AblAktiekapitalUdenPålydendeVærdi",
                            "antal_aktier": 100
                        },
                        "anskaffelsessum_kroner": 1_500_000
                    },
                    "hændelser": [{
                        "$variant": "AblOrdinærAfståelse",
                        "kapitalmængde": {
                            "$variant": "AblAktiekapitalUdenPålydendeVærdi",
                            "antal_aktier": 100
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
                                "ejertidsudbytter": [],
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
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(spouse_property_credit_case);
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
                "par25_valg": kgl_par25_choices(),
                "fordringer": [],
                "valutainstrumenter": [],
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
                "par25_valg": kgl_par25_choices(),
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
                        "dba_status": { "$variant": "KglÅrsnettoIngenDbaBegrænsning" }
                    },
                    "fordringsgruppe": {
                        "$variant": "KglÅrsnettoEnkeltfordring",
                        "mængdeenhed": { "$variant": "KglÅrsnettoNominelleHundreddele" }
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
                "valutainstrumenter": [],
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
        .push(annual_claim_case.clone());
    let mut partial_claim_case = annual_claim_case;
    partial_claim_case["case_id"] = Value::String("personskat-kgl-delrealisering-2026".into());
    partial_claim_case["input"]["kapitalindkomst"]["kursgevinst"]["fakta"]["øvrige_instrumenter"]
        ["fordringer"] = serde_json::json!([{
        "identifikation": "obligation-delrealisering-2026",
        "kilde": {
            "fordringsart": { "$variant": "KglÅrsnettoObligation" },
            "markedsfakta": {
                "$variant": "KglÅrsnettoIkkeOptagetPåReguleretMarked"
            },
            "debitorrelation": { "$variant": "KglÅrsnettoUafhængigDebitor" },
            "næringsforhold": { "$variant": "KglÅrsnettoIkkeNæringsdrivende" },
            "erhvervelsesgrundlag": {
                "$variant": "KglÅrsnettoAlmindeligErhvervelse"
            },
            "dba_status": { "$variant": "KglÅrsnettoIngenDbaBegrænsning" }
        },
        "fordringsgruppe": {
            "$variant": "KglÅrsnettoFondskode",
            "fondskode": "DK0000000001",
            "mængdeenhed": { "$variant": "KglÅrsnettoAntalStyk" }
        },
        "position_primo": {
            "$variant": "KglÅrsnettoVidereførtMængdepositionPrimo",
            "fra_indkomstår": 2025,
            "opgørelsesprincip": { "$variant": "KglRealisationsprincip" },
            "fordringsgruppe": {
                "$variant": "KglÅrsnettoFondskode",
                "fondskode": "DK0000000001",
                "mængdeenhed": { "$variant": "KglÅrsnettoAntalStyk" }
            },
            "trancher": [{
                "identifikation": "tranche-2025-a",
                "anskaffelsestidspunkt": {
                    "dato": { "år": 2025, "måned": 1, "dag": 10 },
                    "rækkefølge_på_dagen": 1
                },
                "resterende_mængde": 100,
                "resterende_anskaffelsessum_kroner": 10_000
            }]
        },
        "hændelser": [
            {
                "$variant": "KglÅrsnettoMængdeanskaffelse",
                "tidspunkt": {
                    "dato": { "år": 2026, "måned": 2, "dag": 10 },
                    "rækkefølge_på_dagen": 1
                },
                "tranche_identifikation": "tranche-2026-b",
                "mængde": 100,
                "anskaffelsessum_kroner": 20_000
            },
            {
                "$variant": "KglÅrsnettoMængdeafståelse",
                "tidspunkt": {
                    "dato": { "år": 2026, "måned": 3, "dag": 10 },
                    "rækkefølge_på_dagen": 1
                },
                "afståelsesart": { "$variant": "KglÅrsnettoDelafdrag" },
                "mængde": 50,
                "afståelsessum_kroner": 7_500
            },
            {
                "$variant": "KglÅrsnettoMængdeafståelse",
                "tidspunkt": {
                    "dato": { "år": 2026, "måned": 4, "dag": 10 },
                    "rækkefølge_på_dagen": 1
                },
                "afståelsesart": { "$variant": "KglÅrsnettoSalg" },
                "mængde": 100,
                "afståelsessum_kroner": 15_000
            }
        ]
    }]);
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(partial_claim_case);
    let mut currency_claim_case = json_input["cases"][0].clone();
    currency_claim_case["case_id"] = Value::String("personskat-kgl-valutakomponenter-2026".into());
    currency_claim_case["input"]["kapitalindkomst"]["kursgevinst"] = serde_json::json!({
        "$variant": "MedKursgevinst",
        "fakta": {
            "skatteyder_identifikation": "Borger",
            "ægtefælles_skatteyder_identifikation": null,
            "sælgerpantebreve": [],
            "gældsposter": [],
            "øvrige_instrumenter": {
                "par25_valg": kgl_par25_choices(),
                "fordringer": [],
                "valutainstrumenter": [{
                    "identifikation": "nærtstående-usd-2026",
                    "valuta": { "iso_4217_kode": "USD" },
                    "aktiv": {
                        "$variant": "KglÅrsnettoValutafordring",
                        "kilde": {
                            "fordringsart": { "$variant": "KglÅrsnettoPengefordring" },
                            "markedsfakta": {
                                "$variant": "KglÅrsnettoIkkeOptagetPåReguleretMarked"
                            },
                            "debitorrelation": {
                                "$variant": "KglÅrsnettoNærtståendePerson"
                            },
                            "næringsforhold": {
                                "$variant": "KglÅrsnettoIkkeNæringsdrivende"
                            },
                            "erhvervelsesgrundlag": {
                                "$variant": "KglÅrsnettoAlmindeligErhvervelse"
                            },
                            "dba_status": {
                                "$variant": "KglÅrsnettoIngenDbaBegrænsning"
                            }
                        }
                    },
                    "position_primo": {
                        "$variant": "KglÅrsnettoIngenValutapositionPrimo"
                    },
                    "årsændring": {
                        "$variant": "KglÅrsnettoValutaAnskaffetOgAfstået",
                        "anskaffelsesværdi": {
                            "beløb_hundreddele": 1_000_000,
                            "kurs": {
                                "dkk_øre_tæller": 500,
                                "valuta_hundreddele_nævner": 100
                            }
                        },
                        "afståelsesværdi": {
                            "beløb_hundreddele": 800_000,
                            "kurs": {
                                "dkk_øre_tæller": 800,
                                "valuta_hundreddele_nævner": 100
                            }
                        }
                    }
                }],
                "obligationsbaserede_minimumsbeviser": []
            },
            "par32_kontraktforløb": {
                "$variant": "UdenPar32Kontraktforløb"
            }
        }
    });
    currency_claim_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": [],
        "udbytter": []
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(currency_claim_case.clone());
    let mut carried_currency_claim_case = currency_claim_case;
    carried_currency_claim_case["case_id"] =
        Value::String("personskat-kgl-valutaposition-videreført-2026".into());
    carried_currency_claim_case["input"]["kapitalindkomst"]["kursgevinst"]["fakta"]
        ["øvrige_instrumenter"]["par25_valg"] = serde_json::json!({
        "obligationer_på_reguleret_marked": {
            "position_primo": {
                "$variant": "KglÅrsnettoVidereførtPar25Valg",
                "fra_indkomstår": 2025,
                "princip": { "$variant": "KglRealisationsprincip" }
            },
            "aktuelt_princip": { "$variant": "KglRealisationsprincip" },
            "ændringstilladelse": {
                "$variant": "KglÅrsnettoIngenPar25Ændringstilladelse"
            }
        },
        "valutakursændringer": {
            "position_primo": {
                "$variant": "KglÅrsnettoVidereførtPar25Valg",
                "fra_indkomstår": 2025,
                "princip": { "$variant": "KglLagerprincip" }
            },
            "aktuelt_princip": { "$variant": "KglLagerprincip" },
            "ændringstilladelse": {
                "$variant": "KglÅrsnettoIngenPar25Ændringstilladelse"
            }
        }
    });
    carried_currency_claim_case["input"]["kapitalindkomst"]["kursgevinst"]["fakta"]
        ["øvrige_instrumenter"]["valutainstrumenter"][0]["identifikation"] =
        Value::String("valutavalg-usd-2026".into());
    carried_currency_claim_case["input"]["kapitalindkomst"]["kursgevinst"]["fakta"]
        ["øvrige_instrumenter"]["valutainstrumenter"][0]["position_primo"] = serde_json::json!({
        "$variant": "KglÅrsnettoVidereførtValutapositionPrimo",
        "position": {
            "indkomstår": 2025,
            "erhvervelsesår": 2025,
            "valuta": { "iso_4217_kode": "USD" },
            "par25_valgpositioner": {
                "obligationer_på_reguleret_marked": {
                    "indkomstår": 2025,
                    "princip": { "$variant": "KglRealisationsprincip" }
                },
                "valutakursændringer": {
                    "indkomstår": 2025,
                    "princip": { "$variant": "KglLagerprincip" }
                }
            },
            "anskaffelsesværdi": {
                "beløb_hundreddele": 1_000_000,
                "kurs": {
                    "dkk_øre_tæller": 500,
                    "valuta_hundreddele_nævner": 100
                }
            },
            "seneste_værdi": {
                "beløb_hundreddele": 800_000,
                "kurs": {
                    "dkk_øre_tæller": 800,
                    "valuta_hundreddele_nævner": 100
                }
            },
            "kredit_eller_pris_tidligere_medregnet_øre": 0,
            "valutakurs_tidligere_medregnet_øre": 2_400_000
        }
    });
    carried_currency_claim_case["input"]["kapitalindkomst"]["kursgevinst"]["fakta"]
        ["øvrige_instrumenter"]["valutainstrumenter"][0]["årsændring"] = serde_json::json!({
        "$variant": "KglÅrsnettoValutaUltimoværdi",
        "ultimoværdi": {
            "beløb_hundreddele": 800_000,
            "kurs": {
                "dkk_øre_tæller": 900,
                "valuta_hundreddele_nævner": 100
            }
        }
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(carried_currency_claim_case);
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
                "par25_valg": kgl_par25_choices(),
                "fordringer": [],
                "valutainstrumenter": [],
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
        "underholdsbidrag": { "bidrag": [] },
        "børnebidragslov19_bidrag": { "bidrag": [] },
        "sømandsbeskatning": { "indkomster": [], "skibsårsdrifter": [], "andre_ligningslov7u_indkomster": [], "dødsboskattegrundlag": { "$variant": "Søbl5IntetDødsboskattegrundlag" }, "kulbrinteskattegrundlag": { "$variant": "Søbl5BIntetKulbrinteskattegrundlag" } },
        "ordinære_forhold": {
            "arbejdsgiverydelser": [],
            "virksomheder_uden_virksomhedsordning": [],
            "forenings_og_arbejdsløshedsydelser": []
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
                    "fraflytningsdato": { "år": 2026, "måned": 7, "dag": 1 },
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
                "kildehistorik": {
                    "opgjort_pr": { "år": 2026, "måned": 12, "dag": 31 },
                    "fristudsættelser": []
                },
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
    par37_spouse_case["input"]["ægtefælle"] = par37_spouse_relationship.clone();
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
    let mut par37_simultaneous_spouse_case = par37_case.clone();
    par37_simultaneous_spouse_case["case_id"] =
        Value::String("personskat-par37-40-samtidige-aegtefaeller-2026".into());
    let main_departure =
        &mut par37_simultaneous_spouse_case["input"]["aktieavance"]["særlige_aktiver"][0];
    main_departure["identifikation"] = Value::String("faelles-fraflytning".into());
    main_departure["kilde"]["fakta"]["identifikation"] =
        Value::String("faelles-fraflytning".into());
    let main_departure_share = &mut main_departure["kilde"]["fakta"]["fraflytning"]["aktier"][0];
    main_departure_share["identifikation"] = Value::String("samtidig-hovedperson-tab".into());
    main_departure_share["selskabsidentifikation"] =
        Value::String("DK-SAMTIDIG-HOVEDPERSON".into());
    main_departure_share["handelsværdi_ved_ophør_kroner"] = serde_json::json!(170_000);
    main_departure_share["skattemæssig_anskaffelsessum_kroner"] = serde_json::json!(200_000);
    main_departure_share["aktivgrundlag"]["fakta"]["par5a_kildefakta"] = serde_json::json!({
        "$variant": "AblOrdinærPar5AKildefakta",
        "fakta": {
            "anvendelsesgrundlag": {
                "$variant": "AblPar5AAfståelseDen24November2010EllerSenere"
            },
            "skatteydergrundlag": {
                "$variant": "AblPar5APersonSkattepligtigEfterPar7"
            },
            "ejertidsudbytter": [],
            "præferenceposition": {
                "modtaget_tilsvarende_udbytte_kroner": 0,
                "allerede_anvendt_til_tabsreduktion_kroner": 0
            },
            "koncernbeløb": []
        }
    });
    let mut spouse_departure = par37_case["input"]["aktieavance"]["særlige_aktiver"][0].clone();
    spouse_departure["identifikation"] = Value::String("faelles-fraflytning".into());
    spouse_departure["kilde"]["fakta"]["identifikation"] =
        Value::String("faelles-fraflytning".into());
    spouse_departure["kilde"]["fakta"]["fraflytning"]["aktier"][0]["identifikation"] =
        Value::String("samtidig-aegtefaelle-gevinst".into());
    spouse_departure["kilde"]["fakta"]["fraflytning"]["aktier"][0]["selskabsidentifikation"] =
        Value::String("DK-SAMTIDIG-AEGTEFAELLE".into());
    par37_simultaneous_spouse_case["input"]["ægtefælle"] = par37_spouse_relationship;
    par37_simultaneous_spouse_case["input"]["ægtefælle"]["fakta"]["lønmodtager"]["skatteår"] =
        serde_json::json!(2026);
    par37_simultaneous_spouse_case["input"]["ægtefælle"]["fakta"]["lønmodtager"]["kommune"] =
        serde_json::json!({ "$variant": "København" });
    par37_simultaneous_spouse_case["input"]["ægtefælle"]["fakta"]["kapitalindkomst"]["renter"]
        ["renteudgifter_kroner"] = serde_json::json!(0);
    par37_simultaneous_spouse_case["input"]["ægtefælle"]["fakta"]["aktieavance"]
        ["særlige_aktiver"] = Value::Array(vec![spouse_departure]);
    par37_simultaneous_spouse_case["input"]["ægtefælle"]["fakta"]["aktieavance"]["udbytter"] =
        serde_json::json!([]);
    par37_simultaneous_spouse_case["input"]["ægtefælle"]["samlevende_ved_indkomstårets_udløb"] =
        serde_json::json!(false);
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
        .push(par37_simultaneous_spouse_case);
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
            },
            "aktiemodregningsfordeling": {
                "$variant": "KglPar32AfledEntydigFordeling"
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
                    "par25_valg": kgl_par25_choices(),
                    "fordringer": [],
                    "valutainstrumenter": [],
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
        5_000,
        -5_000,
        false,
        serde_json::json!({ "$variant": "KglPar32KildeUdenSærligRelation" }),
        serde_json::json!({
            "$variant": "KglPar32KildeEnkeltaktie",
            "markedsstatus": {
                "$variant": "AblOptagetTilHandelPåReguleretMarked"
            }
        }),
    );
    let mut historiske_par32_choices = par32_choices();
    historiske_par32_choices["aktiemodregningsvalg"]["omfang"] =
        serde_json::json!({ "$variant": "KglPar32KunEgneAktiegevinster" });
    let par32_historikår = serde_json::json!({
        "fakta": {
            "indkomstår": 2025,
            "kontrakter": [historisk_par32_tab],
            "valg": historiske_par32_choices
        },
        "aktieavance": {
            "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
            "særlige_aktiver": [],
            "udbytter": []
        },
        "årsgrundlag": {
            "ejendomsavance": { "$variant": "UdenEjendomsavance" },
            "sælgerpantebreve": [],
            "gældsposter": [{
                "identifikation": "par32-historisk-usd-laan",
                "beløb": {
                    "gældens_værdi_ved_påtagelse_kroner": 10_000,
                    "gældens_værdi_ved_frigørelse_eller_indfrielse_kroner": 9_000,
                    "fordringens_værdi_for_kreditor_kroner": 9_000
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
                "par25_valg": kgl_par25_choices(),
                "fordringer": [],
                "valutainstrumenter": [],
                "obligationsbaserede_minimumsbeviser": [{
                    "identifikation": "par32-historisk-abl22",
                    "kilde": {
                        "klassifikation": {
                            "indkomstår": 2025,
                            "aktivmasse": {
                                "indkomstår": 2025,
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
                            "oplysninger": {
                                "$variant": "AblPar21OplysningerIndsendt",
                                "frist": { "år": 2026, "måned": 7, "dag": 1 },
                                "indsendelsesdato": { "år": 2026, "måned": 7, "dag": 1 }
                            }
                        },
                        "par17_modprøve": {
                            "næringsstatus": { "$variant": "AblPar17UdøverIkkeNæringVedKøbOgSalgAfAktier" },
                            "erhvervelsesstatus": { "$variant": "AblPar17IkkeErhvervetSomLedINæringsvej" }
                        }
                    },
                    "position_primo": { "$variant": "KglÅrsnettoIngenPositionPrimo" },
                    "hændelser": [
                        {
                            "$variant": "KglÅrsnettoAnskaffelse",
                            "anskaffelsessum_kroner": 10_000
                        },
                        {
                            "$variant": "KglÅrsnettoAfståelse",
                            "afståelsessum_kroner": 11_500
                        }
                    ]
                }]
            }
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
                -1_000,
                3_000,
                false,
                serde_json::json!({ "$variant": "KglPar32KildeUdenSærligRelation" }),
                serde_json::json!({ "$variant": "KglPar32KildeIkkeAktiebaseret" }),
            ),
            par32_contract(
                "par32-gevinst-b-2026",
                2,
                -5_000,
                -3_000,
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

    let par19_classification = |aktiebaseret: bool| {
        let (meddelelse, direkte_aktiver, oplysninger) = if aktiebaseret {
            (
                serde_json::json!({
                    "$variant": "AblPar19BOrdinærMeddelelse",
                    "virkningsår": 2026,
                    "indsendelsesdato": { "år": 2025, "måned": 11, "dag": 1 }
                }),
                serde_json::json!([
                    {
                        "$variant": "AblDirekteInvesteringsaktiv",
                        "art": { "$variant": "AblKvalificerendeAktieaktiv" },
                        "gennemsnitlig_værdi_kroner": 60_000
                    },
                    {
                        "$variant": "AblDirekteInvesteringsaktiv",
                        "art": { "$variant": "AblAndetVærdipapir" },
                        "gennemsnitlig_værdi_kroner": 40_000
                    }
                ]),
                serde_json::json!({
                    "$variant": "AblPar19BOplysningerIndsendt",
                    "indsendelsesdato": { "år": 2027, "måned": 7, "dag": 1 }
                }),
            )
        } else {
            (
                serde_json::json!({ "$variant": "AblIngenPar19BMeddelelse" }),
                serde_json::json!([
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
                ]),
                serde_json::json!({ "$variant": "AblPar19BOplysningerIkkeIndsendt" }),
            )
        };
        serde_json::json!({
            "$variant": "AblPar19BPar19CKlassifikation",
            "input": {
                "indkomstår": 2026,
                "meddelelse": meddelelse,
                "aktivmasse": {
                    "indkomstår": 2026,
                    "direkte_aktiver": direkte_aktiver,
                    "ejerposter": []
                },
                "oplysninger": oplysninger
            }
        })
    };
    let par19_asset = |identifikation: &str,
                       afståelsessum_kroner: i64,
                       anskaffelsessum_kroner: i64,
                       instrument: &str,
                       klassifikation: Value| {
        serde_json::json!({
            "identifikation": identifikation,
            "kilde": {
                "$variant": "PersonskatØvrigtAktieaktiv",
                "input": {
                    "indkomstår": 2026,
                    "aktiv": { "$variant": "AblInvesteringsselskabPar19TilKlassifikation" },
                    "afståelsessum_kroner": afståelsessum_kroner,
                    "anskaffelsessum_kroner": anskaffelsessum_kroner,
                    "koncernintern_konvertibel_eller_tegningsret": false,
                    "andelsforening_stiftet_før_22_maj_1987": false,
                    "afståelse_sker_for_at_undgå_likvidationsbeskatning": false,
                    "investeringsklassifikation": klassifikation
                },
                "par17_modprøvekilde": {
                    "$variant": "MedPar17Modprøvekilde",
                    "fakta": {
                        "indkomstår": 2026,
                        "skattepligtsgrundlag": {
                            "$variant": "AblPar7PersonEfterKildeskatteloven"
                        },
                        "næringsstatus": {
                            "$variant": "AblPar17UdøverIkkeNæringVedKøbOgSalgAfAktier"
                        },
                        "instrument": { "$variant": instrument },
                        "erhvervelsesstatus": {
                            "$variant": "AblPar17IkkeErhvervetSomLedINæringsvej"
                        },
                        "afståelsessum_kroner": afståelsessum_kroner,
                        "anskaffelsessum_kroner": anskaffelsessum_kroner
                    }
                }
            },
            "markedsstatus": {
                "$variant": "AblOptagetTilHandelPåReguleretMarked"
            }
        })
    };
    let mut par32_mixed_case = json_input["cases"][0].clone();
    par32_mixed_case["case_id"] =
        Value::String("personskat-kgl-par32-blandet-fordeling-2026".into());
    par32_mixed_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": [
            par19_asset(
                "par32-json-abl19b",
                50_000,
                30_000,
                "AblPar17UndtagetEfterPar19B",
                par19_classification(true)
            ),
            par19_asset(
                "par32-json-abl19c",
                42_000,
                30_000,
                "AblPar17UndtagetEfterPar19C",
                par19_classification(false)
            )
        ],
        "udbytter": []
    });
    let mut par32_mixed_kursgevinst = par32_kursgevinst(
        "par32-blandet-person",
        vec![],
        vec![par32_contract(
            "par32-blandet-tab-2026",
            1,
            35_000,
            10_000,
            false,
            serde_json::json!({ "$variant": "KglPar32KildeUdenSærligRelation" }),
            serde_json::json!({
                "$variant": "KglPar32KildeEnkeltaktie",
                "markedsstatus": {
                    "$variant": "AblOptagetTilHandelPåReguleretMarked"
                }
            }),
        )],
    );
    par32_mixed_kursgevinst["fakta"]["par32_kontraktforløb"]["aktuelt_år"]["valg"]
        ["aktiemodregningsvalg"]["omfang"] =
        serde_json::json!({ "$variant": "KglPar32KunEgneAktiegevinster" });
    par32_mixed_kursgevinst["fakta"]["par32_kontraktforløb"]["aktuelt_år"]["valg"]
        ["aktiemodregningsfordeling"] = serde_json::json!({
        "$variant": "KglPar32FordelEfterKilder",
        "kilder": [
            {
                "$variant": "KglPar32SupplerendeAblKilde",
                "kildeidentifikation": "par32-json-abl19c"
            },
            {
                "$variant": "KglPar32SupplerendeAblKilde",
                "kildeidentifikation": "par32-json-abl19b"
            }
        ]
    });
    par32_mixed_case["input"]["kapitalindkomst"]["kursgevinst"] = par32_mixed_kursgevinst;
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(par32_mixed_case);

    let mut external_deficit_case = json_input["cases"][0].clone();
    external_deficit_case["case_id"] = Value::String("personskat-underskud-ekstern-2026".into());
    external_deficit_case["input"]["underskudsforhold"] = serde_json::json!({
        "$variant": "EksterntFastsatFremførtUnderskud",
        "fra_indkomstår": 2025,
        "underskud_kroner": 40_000,
        "proveniens": {
            "$variant": "SkatteforvaltningensÅrsopgørelse",
            "dokumentreference": "årsopgørelse-2025-version-1"
        }
    });
    external_deficit_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": [],
        "udbytter": []
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(external_deficit_case);

    let mut prior_deficit_result_case = json_input["cases"][0].clone();
    prior_deficit_result_case["case_id"] =
        Value::String("personskat-underskud-årsresultat-2026".into());
    prior_deficit_result_case["input"]["underskudsforhold"] = serde_json::json!({
        "$variant": "FremførtUnderskudFraForrigePersonskatÅr",
        "resultat": {
            "indkomstår": 2025,
            "åbningsgrundlag_gyldigt": true,
            "fremført_underskud_primo_kroner": 0,
            "årets_skattepligtige_indkomst_før_fremførsel_kroner": -30_000,
            "fremført_underskud_anvendt_i_egen_indkomst_kroner": 0,
            "årets_nye_underskud_kroner": 30_000,
            "dækket_ved_egen_skattemodregning_kroner": 0,
            "fradraget_i_ægtefælles_indkomst_kroner": 0,
            "dækket_ved_ægtefælles_skattemodregning_kroner": 0,
            "fremført_underskud_ultimo_kroner": 30_000
        }
    });
    prior_deficit_result_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": [],
        "udbytter": []
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(prior_deficit_result_case);

    let mut negative_share_tax_carry_case = json_input["cases"][0].clone();
    negative_share_tax_carry_case["case_id"] =
        Value::String("personskat-negativ-aktieskat-fremførsel-2026".into());
    negative_share_tax_carry_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": [],
        "udbytter": []
    });
    negative_share_tax_carry_case["input"]["negativ_aktieskat_fremførsel"] = serde_json::json!({
        "hovedperson": {
            "$variant": "EksterntFastsatFremførtNegativAktieskat",
            "trancher": [{
                "ejer": { "$variant": "HovedpersonensNegativAktieskat" },
                "oprindelsesår": 2024,
                "resterende_negativ_skat_kroner": 1_000
            }],
            "proveniens": {
                "$variant": "SkatteforvaltningensÅrsopgørelse",
                "dokumentreference": "årsopgørelse-2024-test"
            }
        },
        "ægtefælle": { "$variant": "UdenFremførtNegativAktieskat" }
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(negative_share_tax_carry_case);

    let mut dis_case = json_input["cases"][0].clone();
    dis_case["case_id"] = Value::String("personskat-dis-2026".into());
    dis_case["input"]["lønmodtager"]["bruttoløn_kroner"] = serde_json::json!(300_000);
    dis_case["input"]["lønmodtager"]["personlig_indkomst"]["sømandsbeskatning"] = serde_json::json!({
        "indkomster": [{
            "identifikation": "dis-gods-2026",
            "indkomstår": 2026,
            "person": {
                "skattepligt": { "$variant": "SøblFuldtSkattepligtigEfterKsl1" },
                "statsborgerskab": { "$variant": "SøblStatsborgerIEUEØS" },
                "relation": { "$variant": "SøblAlmindeligLønmodtager" }
            },
            "skib": {
                "identifikation": "dis-skib-2026",
                "registrering": { "$variant": "SøblDanskSkibRegistreretIDIS" },
                "bruttotonnage": 12_000,
                "arbejdsgiverstatus": { "$variant": "SøblDanskArbejdsgiver" }
            },
            "arbejde": {
                "anvendelse": {
                    "$variant": "SøblUdelukkendeAnvendtTil",
                    "aktivitet": {
                        "$variant": "SøblBugseringOgBjærgning"
                    }
                },
                "arbejdsområde": { "$variant": "SøblArbejdeIndenForEUEØS" },
                "passagerrute": { "$variant": "SøblIngenPassagersejlads" },
                "arbejdsrolle": { "$variant": "SøblNormalDriftsbesætning" },
                "par8_valg": {
                    "$variant": "SøblAnvendPar10RefusionEllerAlmindeligBeskatning"
                }
            },
            "løn": {
                "indkomsttype": { "$variant": "SøblLønVedArbejdeOmBord" },
                "løngrundlag": {
                    "$variant": "SøblSkattefriNettolønFastsatUnderHensynTilFritagelsen"
                },
                "beløb_kroner": 500_000
            }
        }],
        "skibsårsdrifter": [{
            "skibsidentifikation": "dis-skib-2026",
            "indkomstår": 2026,
            "driftstid": {
                "søtransport_minutter": 4_500,
                "mobilisering_til_søs_minutter": 500,
                "andre_aktiviteter_minutter": 5_000,
                "ventetid_minutter": 2_000
            }
        }],
        "andre_ligningslov7u_indkomster": [{
            "identifikation": "anden-ligningslov7u-2026",
            "beløb_kroner": 60_000
        }],
        "dødsboskattegrundlag": { "$variant": "Søbl5IntetDødsboskattegrundlag" },
        "kulbrinteskattegrundlag": { "$variant": "Søbl5BIntetKulbrinteskattegrundlag" }
    });
    let mut dis_ligningslov7u_income = dis_case["input"]["lønmodtager"]["personlig_indkomst"]
        ["sømandsbeskatning"]["indkomster"][0]
        .clone();
    dis_ligningslov7u_income["identifikation"] = Value::String("dis-ligningslov7u-2026".into());
    dis_ligningslov7u_income["løn"]["indkomsttype"] =
        serde_json::json!({ "$variant": "SøblLigningslov7UYdelseMedDirekteTilknytning" });
    dis_ligningslov7u_income["løn"]["beløb_kroner"] = serde_json::json!(20_000);
    dis_case["input"]["lønmodtager"]["personlig_indkomst"]["sømandsbeskatning"]["indkomster"]
        .as_array_mut()
        .expect("DIS source incomes")
        .push(dis_ligningslov7u_income);
    dis_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": [],
        "udbytter": []
    });
    let mut dis_course_case = dis_case.clone();
    dis_course_case["case_id"] = Value::String("personskat-dis-kursus-2026".into());
    let mut dis_course_income = dis_course_case["input"]["lønmodtager"]["personlig_indkomst"]
        ["sømandsbeskatning"]["indkomster"][0]
        .clone();
    dis_course_income["identifikation"] = Value::String("dis-kursus-2026".into());
    dis_course_income["arbejde"]["anvendelse"]["aktivitet"] =
        serde_json::json!({ "$variant": "SøblTransportAfGodsMellemForskelligeDestinationer" });
    dis_course_income["arbejde"]["arbejdsområde"] =
        serde_json::json!({ "$variant": "SøblArbejdeUdenForEUEØS" });
    dis_course_income["arbejde"]["arbejdsrolle"] = serde_json::json!({
        "$variant": "SøblKursusophold",
        "opgørelse": {
            "opgørelsesperiode": {
                "startdato": { "år": 2026, "måned": 1, "dag": 1 },
                "slutdato": { "år": 2026, "måned": 12, "dag": 31 }
            },
            "kursusperioder": [
                {
                    "startdato": { "år": 2026, "måned": 1, "dag": 1 },
                    "slutdato": { "år": 2026, "måned": 1, "dag": 31 }
                },
                {
                    "startdato": { "år": 2026, "måned": 2, "dag": 1 },
                    "slutdato": { "år": 2026, "måned": 2, "dag": 28 }
                },
                {
                    "startdato": { "år": 2026, "måned": 3, "dag": 1 },
                    "slutdato": { "år": 2026, "måned": 3, "dag": 31 }
                }
            ]
        },
        "umiddelbart_før_omfattet": true,
        "fortsat_ansat_af_rederiet": true
    });
    dis_course_case["input"]["lønmodtager"]["personlig_indkomst"]["sømandsbeskatning"]
        ["indkomster"] = Value::Array(vec![dis_course_income]);
    dis_course_case["input"]["lønmodtager"]["personlig_indkomst"]["sømandsbeskatning"]
        ["skibsårsdrifter"] = Value::Array(vec![]);
    dis_course_case["input"]["lønmodtager"]["personlig_indkomst"]["sømandsbeskatning"]
        ["andre_ligningslov7u_indkomster"] = Value::Array(vec![]);

    let mut death_estate_case = dis_case.clone();
    death_estate_case["case_id"] = Value::String("personskat-dis-doedsbo-2026".into());
    death_estate_case["input"]["lønmodtager"]["bruttoløn_kroner"] = serde_json::json!(0);
    let mut death_estate_income = death_estate_case["input"]["lønmodtager"]["personlig_indkomst"]
        ["sømandsbeskatning"]["indkomster"][0]
        .clone();
    death_estate_income["identifikation"] = Value::String("doedsbo-dis-gods-2026".into());
    death_estate_income["person"]["skattepligt"] =
        serde_json::json!({ "$variant": "SøblDødsboEfterDødsboskattelov1Stk2" });
    death_estate_income["skib"]["identifikation"] = Value::String("doedsbo-dis-skib-2026".into());
    death_estate_income["arbejde"]["anvendelse"] = serde_json::json!({
        "$variant": "SøblUdelukkendeAnvendtTil",
        "aktivitet": {
            "$variant": "SøblTransportAfGodsMellemForskelligeDestinationer"
        }
    });
    death_estate_income["arbejde"]["arbejdsområde"] =
        serde_json::json!({ "$variant": "SøblArbejdeUdenForEUEØS" });
    death_estate_income["løn"]["beløb_kroner"] = serde_json::json!(200_000);
    death_estate_case["input"]["lønmodtager"]["personlig_indkomst"]["sømandsbeskatning"] = serde_json::json!({
        "indkomster": [death_estate_income],
        "skibsårsdrifter": [],
        "andre_ligningslov7u_indkomster": [],
        "dødsboskattegrundlag": {
            "$variant": "Søbl5Dødsboskattegrundlag",
            "input": {
                "boform": { "$variant": "Dbl1Stk2BoBehandlesHeltEllerDelvistIDanmark" },
                "bobeskatningsindkomst_kroner": 600_000,
                "dødsdato": { "år": 2026, "måned": 8, "dag": 15 },
                "boopgørelsestype": { "$variant": "Dbl30OrdinærBoopgørelse" },
                "boopgørelsens_skæringsdag": { "år": 2026, "måned": 8, "dag": 31 },
                "indkomstårsforhold": { "$variant": "Dbl30Kalenderindkomstår" },
                "ægtefælleforhold": {
                    "$variant": "Dbl30TidligereAfdødÆgtefælleEfterPar62",
                    "førstafdødes_dødsdato": { "år": 2026, "måned": 7, "dag": 15 },
                    "par67_stk7_progressionsforhold": {
                        "$variant": "Dbl67Stk7IntetSkifteAfFørstafdødesSærbo"
                    }
                },
                "aktieindkomstgrundlag": { "$variant": "Dbl32IngenAktieindkomst" },
                "carrybackgrundlag": { "$variant": "Dbl31IngenDokumenteretCarrybackgrundlag" }
            }
        },
        "kulbrinteskattegrundlag": { "$variant": "Søbl5BIntetKulbrinteskattegrundlag" }
    });

    let mut death_estate_share_case = death_estate_case.clone();
    death_estate_share_case["case_id"] =
        Value::String("personskat-dis-doedsbo-aktieindkomst-2026".into());
    death_estate_share_case["input"]["lønmodtager"]["personlig_indkomst"]["sømandsbeskatning"]
        ["dødsboskattegrundlag"]["input"]["aktieindkomstgrundlag"] = serde_json::json!({
        "$variant": "Dbl32OpgjortAktieindkomstEfterPar21",
        "aktieindkomst_kroner": 100_000,
        "dokumentreference": "boopgørelse-aktieindkomst-2026"
    });
    death_estate_share_case["input"]["lønmodtager"]["personlig_indkomst"]["sømandsbeskatning"]
        ["dødsboskattegrundlag"]["input"]["ægtefælleforhold"]["par67_stk7_progressionsforhold"] = serde_json::json!({
        "$variant": "Dbl67Stk7FørstafdødesSærboEndeligtSkatteberegnet",
        "anvendt_ekstra_progressionsgrænse_kroner": 30_000,
        "dokumentreference": "skatteberegning-førstafdødes-særbo-2026"
    });

    let mut death_estate_carryback_case = death_estate_case.clone();
    death_estate_carryback_case["case_id"] =
        Value::String("personskat-dis-doedsbo-carryback-2026".into());
    let death_estate_carryback_input = &mut death_estate_carryback_case["input"]["lønmodtager"]
        ["personlig_indkomst"]["sømandsbeskatning"]["dødsboskattegrundlag"]["input"];
    death_estate_carryback_input["bobeskatningsindkomst_kroner"] = serde_json::json!(-200_000);
    death_estate_carryback_input["ægtefælleforhold"] =
        serde_json::json!({ "$variant": "Dbl30IntetÆgtefælleforholdEfterPar62" });
    death_estate_carryback_input["carrybackgrundlag"] = serde_json::json!({
        "$variant": "Dbl31DokumenteretCarrybackgrundlag",
        "længstlevende_ægtefælleforhold": {
            "$variant": "Dbl31IngenLængstlevendeÆgtefælle"
        },
        "betalte_årsskatter": [
            {
                "person": { "$variant": "Dbl31Afdøde" },
                "indkomstår": 2024,
                "skat_af_skattepligtig_indkomst_kroner": 35_000,
                "skat_af_aktieindkomst_kroner": 5_000,
                "arbejdsmarkedsbidrag_kroner": 0,
                "heraf_skat_efter_kildeskattelov48e_48f_kroner": 0,
                "dokumentreference": "årsopgørelse-afdøde-2024"
            },
            {
                "person": { "$variant": "Dbl31Afdøde" },
                "indkomstår": 2025,
                "skat_af_skattepligtig_indkomst_kroner": 30_000,
                "skat_af_aktieindkomst_kroner": 10_000,
                "arbejdsmarkedsbidrag_kroner": 0,
                "heraf_skat_efter_kildeskattelov48e_48f_kroner": 0,
                "dokumentreference": "årsopgørelse-afdøde-2025"
            }
        ],
        "bofordelingsgrundlag": {
            "$variant": "Dbl31FællesboOgSærboSkiftesHverForSig",
            "fællesbo": {
                "bobeskatningsindkomst_kroner": -120_000,
                "aktieindkomst_kroner": 0,
                "boopgørelsens_skæringsdag": { "år": 2026, "måned": 8, "dag": 20 },
                "dokumentreference": "fællesboopgørelse-2026-08-20"
            },
            "særbo": {
                "bobeskatningsindkomst_kroner": -80_000,
                "aktieindkomst_kroner": 0,
                "boopgørelsens_skæringsdag": { "år": 2026, "måned": 8, "dag": 31 },
                "dokumentreference": "særboopgørelse-2026-08-31"
            }
        }
    });

    let mut limited_taxpayer_case = death_estate_case.clone();
    limited_taxpayer_case["case_id"] =
        Value::String("personskat-dis-begraenset-skattepligt-2026".into());
    let mut limited_taxpayer_income = limited_taxpayer_case["input"]["lønmodtager"]
        ["personlig_indkomst"]["sømandsbeskatning"]["indkomster"][0]
        .clone();
    limited_taxpayer_income["identifikation"] = Value::String("begraenset-dis-gods-2026".into());
    limited_taxpayer_income["person"]["skattepligt"] =
        serde_json::json!({ "$variant": "SøblBegrænsetSkattepligtigEfterKsl2Stk2" });
    limited_taxpayer_income["person"]["statsborgerskab"] =
        serde_json::json!({ "$variant": "SøblAndetStatsborgerskab" });
    limited_taxpayer_income["skib"]["identifikation"] =
        Value::String("begraenset-dis-skib-2026".into());
    limited_taxpayer_income["løn"]["beløb_kroner"] = serde_json::json!(300_000);
    limited_taxpayer_case["input"]["lønmodtager"]["personlig_indkomst"]["sømandsbeskatning"] = serde_json::json!({
        "indkomster": [limited_taxpayer_income],
        "skibsårsdrifter": [],
        "andre_ligningslov7u_indkomster": [],
        "dødsboskattegrundlag": { "$variant": "Søbl5IntetDødsboskattegrundlag" },
        "kulbrinteskattegrundlag": { "$variant": "Søbl5BIntetKulbrinteskattegrundlag" }
    });

    let mut hydrocarbon_case = death_estate_case.clone();
    hydrocarbon_case["case_id"] = Value::String("personskat-dis-kulbrinte-2026".into());
    let mut hydrocarbon_income = hydrocarbon_case["input"]["lønmodtager"]["personlig_indkomst"]
        ["sømandsbeskatning"]["indkomster"][0]
        .clone();
    hydrocarbon_income["identifikation"] = Value::String("kulbrinte-dis-2026".into());
    hydrocarbon_income["person"]["skattepligt"] =
        serde_json::json!({ "$variant": "SøblKulbrinteskattepligtigEfterPar21Stk2" });
    hydrocarbon_income["person"]["statsborgerskab"] =
        serde_json::json!({ "$variant": "SøblAndetStatsborgerskab" });
    hydrocarbon_income["skib"] = serde_json::json!({
        "identifikation": "kulbrinte-eu-skib-2026",
        "registrering": {
            "$variant": "SøblUdenlandskSkibRegistreretIEUEØS",
            "flag": { "$variant": "SøblEUEØSFlag" }
        },
        "bruttotonnage": 12_000,
        "arbejdsgiverstatus": { "$variant": "SøblUdenlandskArbejdsgiverGodkendtEfterPar11A" }
    });
    hydrocarbon_income["løn"]["beløb_kroner"] = serde_json::json!(500_000);
    hydrocarbon_case["input"]["lønmodtager"]["personlig_indkomst"]["sømandsbeskatning"] = serde_json::json!({
        "indkomster": [hydrocarbon_income],
        "skibsårsdrifter": [],
        "andre_ligningslov7u_indkomster": [],
        "dødsboskattegrundlag": { "$variant": "Søbl5IntetDødsboskattegrundlag" },
        "kulbrinteskattegrundlag": {
            "$variant": "Søbl5BKulbrinteskattegrundlag",
            "kildefakta": {
                "personstatus": { "$variant": "KulbrintePersonIkkeOmfattetAfKildeskattelov1" },
                "arbejdsgiverhjemting": { "$variant": "KulbrinteArbejdsgiverUdenHjemtingIDanmark" },
                "indkomstkategori": { "$variant": "KulbrinteLønEllerAndetIkkeErhvervsmæssigtVederlag" },
                "dansk_beskatningsret": { "$variant": "KulbrinteDanskBeskatningsretBekræftet" },
                "beskatningsvalg": { "$variant": "KulbrinteEndeligBruttoskatEfterPar21Stk2" },
                "alder_ved_indkomstårets_udløb": 40
            }
        }
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(dis_case);
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(dis_course_case);
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(death_estate_case);
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(death_estate_share_case);
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(death_estate_carryback_case);
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(limited_taxpayer_case);
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(hydrocarbon_case);
    let pbl15a_company = |owner_path_id: &str| {
        serde_json::json!({
            "identifikation": "pbl15a-driftsselskab",
            "ejerforhold": {
                "direkte_ejerandel_basispoint": 1_000,
                "indirekte_ejerveje": [{
                    "identifikation": owner_path_id,
                    "ejerandele_gennem_kæden_basispoint": [5_000, 4_000]
                }]
            },
            "udøver_aktiv_udlejning_efter_abl34_stk7": false
        })
    };
    let pbl15a_period = |year: i64, position: i64| {
        let owner_path_id = format!("pbl15a-indirekte-ejervej-{year}");
        serde_json::json!({
            "rækkefølge_fra_ældste": position,
            "startdato": { "år": year, "måned": 1, "dag": 1 },
            "slutdato": { "år": year, "måned": 12, "dag": 31 },
            "virksomhedens_indtægter": [],
            "virksomhedens_aktiver": [],
            "selskabsregnskaber": [{
                "selskab": pbl15a_company(&owner_path_id),
                "aktiernes_handelsværdi_i_virksomheden_kroner": 900_000,
                "aktieafkast_i_virksomheden_kroner": 300_000,
                "selskabets_indtægter_før_ejerandel": [{
                    "identifikation": format!("pbl15a-driftsindtægt-{year}"),
                    "beløb_kroner": 1_000_000,
                    "art": { "$variant": "Pbl15AØvrigDriftsindtægt" }
                }],
                "selskabets_aktiver_før_ejerandel": [{
                    "identifikation": format!("pbl15a-driftsaktiv-{year}"),
                    "handelsværdi_kroner": 2_000_000,
                    "art": { "$variant": "Pbl15AØvrigtDriftsaktiv" }
                }]
            }]
        })
    };
    let pbl15a_qualification_years = (2016..=2025)
        .map(|year| {
            serde_json::json!({
                "indkomstår": year,
                "grundlag": { "$variant": "Pbl15AEgenSelvstændigVirksomhed" }
            })
        })
        .collect::<Vec<_>>();
    let mut pbl15a_case = json_input["cases"][0].clone();
    pbl15a_case["case_id"] = Value::String("personskat-pbl15a-relationer-2026".into());
    pbl15a_case["input"]["aktieavance"]["særlige_aktiver"] = serde_json::json!([]);
    pbl15a_case["input"]["lønmodtager"]["pension"]["pbl18_indbetalinger"] = serde_json::json!([{
        "identifikation": "pbl15a-indbetaling-2026",
        "ordning": { "$variant": "Pbl18Par15ARateordning" },
        "indbetalingskilde": { "$variant": "Pbl18EgenIndbetaling" },
        "fradragsretshaver": { "$variant": "Pbl18OrdningensEjer" },
        "betaling": {
            "beløb_kroner": 75_000,
            "forfaldsår": 2026,
            "betalingsår": 2026,
            "betalt_senest_bankjusteret_1_april_efter_forfald": true,
            "hidrører_fra_par22e_tilbagebetaling": false,
            "par15a_fradragsplacering": {
                "$variant": "Pbl18Par15AIndbetalingsår"
            },
            "arbejdsmarkedsbidrag_kroner": 0
        },
        "fordelingsforløb": { "$variant": "Pbl18IngenTiårsfordeling" },
        "indeksvalg": { "fradragsvalgte_kontraktbidrag_kroner": [] },
        "indeksordningsgrundlag": { "$variant": "Pbl18IkkeIndeksordning" },
        "forfaldne_ikke_tidligere_fratrukket_kroner": 75_000,
        "særligt_ordningsgrundlag": {
            "$variant": "Pbl18Par15AIndbetalingsgrundlag",
            "ordning_identifikation": "pbl15a-ophørspension",
            "afståelse_identifikation": "pbl15a-virksomhedsafståelse"
        },
        "begrænsninger": {
            "pbl54_personkreds_opfyldt": true,
            "afgiftspligt_for_hele_ordningen_indtrådt": false,
            "udenlandsk_overførsel_med_tidligere_fradrag_uden_skatte_eller_afgiftskonsekvens": false
        }
    }]);
    pbl15a_case["input"]["lønmodtager"]["pension"]["pbl15a_årsgrundlag"] = serde_json::json!({
        "afståelser": [{
            "identifikation": "pbl15a-virksomhedsafståelse",
            "afståelsesdato": { "år": 2026, "måned": 7, "dag": 1 },
            "grundlag": { "$variant": "Pbl15AEgenVirksomhedsfortjeneste" },
            "fortjenestegrundlag": {
                "afskrivningslovsposter": [{
                    "identifikation": "pbl15a-al6-afståelsesfortjeneste",
                    "kilde": {
                        "$variant": "Pbl15AAl6Salg",
                        "input": {
                            "straksafskrivningsresultat": {
                                "indkomstår": 2026,
                                "anskaffelsesår": 2024,
                                "skatteyder": {
                                    "$variant": "AlSelvstændigErhvervsdrivendePerson"
                                },
                                "valg": { "$variant": "Al6Småaktiv" },
                                "omfattet_af_par6_stk1_modellen": true,
                                "kort_levetid_betingelse_opfyldt": false,
                                "småaktiv_betingelse_opfyldt": true,
                                "forskning_betingelse_opfyldt": false,
                                "valg_hjemmel_opfyldt": true,
                                "omfattet_af_stk3": false,
                                "udskydelse_efter_stk3_gælder": false,
                                "første_mulige_fradragsår": 2024,
                                "fradragsår_opfyldt": true,
                                "valg_gyldigt": true,
                                "forskningsudgift_under_loft_kroner": 0,
                                "forskningsudgift_over_loft_kroner": 0,
                                "fradrag_i_skattepligtig_indkomst_kroner": 100_000,
                                "anskaffelsessum_fradraget_senest_i_indkomståret_kroner": 100_000,
                                "ufradraget_anskaffelsessum_kroner": 0
                            },
                            "salgssum_kroner": 100_000,
                            "leveret_i_indkomståret": true,
                            "virksomhed_sælges_eller_ophører_i_indkomståret": false
                        }
                    }
                }],
                "ejendomsavance": { "$variant": "Pbl15AUdenEjendomsavance" },
                "aktieavance": { "$variant": "Pbl15AUdenAktieavance" },
                "kursgevinstposter": []
            },
            "alder_hele_år_ved_afståelsen": 60,
            "passiv_kapital": {
                "seneste_tre_regnskabsperioder": [
                    pbl15a_period(2023, 1),
                    pbl15a_period(2024, 2),
                    pbl15a_period(2025, 3)
                ],
                "aktuel_regnskabsperiodes_startdato": {
                    "år": 2026,
                    "måned": 1,
                    "dag": 1
                },
                "virksomhedens_aktiver_på_overdragelsestidspunktet": [],
                "selskabsaktiver_på_overdragelsestidspunktet": [{
                    "selskab": pbl15a_company("pbl15a-indirekte-ejervej-ved-afståelse"),
                    "aktiernes_handelsværdi_i_virksomheden_kroner": 900_000,
                    "selskabets_aktiver_før_ejerandel": [{
                        "identifikation": "pbl15a-driftsaktiv-ved-afståelse",
                        "handelsværdi_kroner": 2_000_000,
                        "art": { "$variant": "Pbl15AØvrigtDriftsaktiv" }
                    }]
                }],
                "holdingforløb": {
                    "personens_afståelsesrækkefølge_på_dagen": 1,
                    "datterselskabshændelser": [],
                    "afvikling": { "$variant": "Pbl15AHoldingFortsatBestående" }
                },
                "næringsvirksomhed_med_værdipapirer_eller_finansiering": false
            },
            "udlejning_af_afskrivningsberettigede_driftsmidler_eller_skibe": false,
            "antal_ejere": 1,
            "opretter_deltog_i_driften_i_væsentligt_omfang": true
        }],
        "ordninger": [{
            "identifikation": "pbl15a-ophørspension",
            "oprettelsesår": 2026,
            "art": { "$variant": "Pbl15ARateopsparing" },
            "oprettelsesafståelse_identifikation": "pbl15a-virksomhedsafståelse"
        }],
        "kvalifikationsår": pbl15a_qualification_years,
        "tidligere_indbetalinger": []
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(pbl15a_case);
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(fisher_union_transition_case);
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(fisher_cross_year_case);
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(fisher_mixed_travel_no_election_case);
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(fisher_mixed_travel_case);
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(seafarer_commute_case);
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
        let foreign_relief = case["input"]
            .as_object_mut()
            .expect("Personskat input")
            .entry("ligningslov33".to_string())
            .or_insert_with(|| {
                serde_json::json!({
                    "hovedperson": { "$variant": "UdenLigningslov33" },
                    "ægtefælle": { "$variant": "UdenLigningslov33" },
                    "ligningslov33a_hovedperson": { "$variant": "UdenLigningslov33A" },
                    "ligningslov33a_ægtefælle": { "$variant": "UdenLigningslov33A" }
                })
            });
        let foreign_relief = foreign_relief
            .as_object_mut()
            .expect("Personskat foreign-relief input");
        foreign_relief
            .entry("ligningslov33a_hovedperson".to_string())
            .or_insert_with(|| serde_json::json!({ "$variant": "UdenLigningslov33A" }));
        foreign_relief
            .entry("ligningslov33a_ægtefælle".to_string())
            .or_insert_with(|| serde_json::json!({ "$variant": "UdenLigningslov33A" }));
    }
    let mut freight_tax_case = json_input["cases"][0].clone();
    freight_tax_case["case_id"] = Value::String("personskat-ligningslov33-fragtskat-2026".into());
    freight_tax_case["input"]["ligningslov33"] = serde_json::json!({
        "hovedperson": {
            "$variant": "MedLigningslov33",
            "input": {
                "indkomstår": 2026,
                "dansk_bruttoindkomst_kroner": 600_000,
                "ikke_henførbare_udgifter": [],
                "kreditgrupper": [{
                    "identifikation": "norsk-fragtskat-2026",
                    "grupperingsgrundlag": {
                        "$variant": "Ll33SamletIndkomstFraSkatteområdet"
                    },
                    "område": {
                        "$variant": "Ll33FremmedStat",
                        "landekode": "NO"
                    },
                    "indkomstposter": [{
                        "identifikation": "norsk-fragtskat-bruttoindkomst-2026",
                        "art": { "$variant": "Ll33UdenlandskBruttoindtægt" },
                        "beløb_kroner": 100_000,
                        "indkomstkategorier": [{
                            "$variant": "Ll33SkattepligtigIndkomstkategori"
                        }],
                        "dokumentreference": "Norsk indkomstbilag 2026"
                    }],
                    "skattebetalinger": [{
                        "opkrævningsmåde": { "$variant": "Ll33DirektePåligning" },
                        "betalt_udenlandsk_skat_øre": 100_000,
                        "skatteart": { "$variant": "Ll33Indkomstskat" },
                        "betalingsdokumentreference": "Norsk indkomstskattebilag 2026",
                        "overenskomstgrundlag": {
                            "$variant": "Ll33IngenDobbeltbeskatningsoverenskomst"
                        }
                    }, {
                        "opkrævningsmåde": { "$variant": "Ll33DirektePåligning" },
                        "betalt_udenlandsk_skat_øre": 100_000,
                        "skatteart": {
                            "$variant": "Ll33FragtskatPåBruttofortjenesteVedInternationalSkibstrafik"
                        },
                        "betalingsdokumentreference": "Norsk fragtskattebilag 2026",
                        "overenskomstgrundlag": {
                            "$variant": "Ll33IngenDobbeltbeskatningsoverenskomst"
                        }
                    }],
                    "lønlempelsesstatus": { "$variant": "Ll33IngenLønindkomst" }
                }],
                "par6_kreditter": [],
                "fragtskat_åbningssaldi": [{
                    "område": {
                        "$variant": "Ll33FremmedStat",
                        "landekode": "NO"
                    },
                    "indkomstår": 2025,
                    "saldo_øre": 100_000,
                    "dokumentreferencer": ["Futuruna-fragtskattesaldo 2025"]
                }]
            }
        },
        "ægtefælle": { "$variant": "UdenLigningslov33" },
        "ligningslov33a_hovedperson": { "$variant": "UdenLigningslov33A" },
        "ligningslov33a_ægtefælle": { "$variant": "UdenLigningslov33A" }
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(freight_tax_case);
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
    let simultaneous_spouse_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-par37-40-samtidige-aegtefaeller-2026")
        .expect("simultaneous-spouse §§ 37-40 JSON case")
        .clone();
    let ordinary_share_loss_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-renter-befordring-2026")
        .expect("ordinary ABL share-loss JSON case")
        .clone();
    let spouse_property_credit_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-par8a-aegtefaelle-ejendomsskatter-2025")
        .expect("spouse property-tax credit JSON case")
        .clone();
    let partial_exemption_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-ejendomsskat-delvis-fritagelse-2025")
        .expect("partial property-tax exemption JSON case")
        .clone();
    let temporary_rental_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-ejendomsskat-midlertidig-udlejning-2025")
        .expect("temporary-rental property-tax JSON case")
        .clone();
    let spouse_rebate_recipient_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-ejendomsskat-aegtefaellemodtager-2025")
        .expect("spouse rebate recipient JSON case")
        .clone();
    let partial_year_cap_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-ejendomsskat-halvårsloft-2025")
        .expect("partial-year property-tax cap JSON case")
        .clone();
    let annual_claim_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-aarsnetto-fordring-2026")
        .expect("annual KGL claim JSON case")
        .clone();
    let partial_claim_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-delrealisering-2026")
        .expect("partial KGL claim JSON case")
        .clone();
    let currency_claim_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-valutakomponenter-2026")
        .expect("foreign-currency KGL claim JSON case")
        .clone();
    let carried_currency_claim_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-valutaposition-videreført-2026")
        .expect("carried foreign-currency KGL claim JSON case")
        .clone();
    let external_deficit_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-underskud-ekstern-2026")
        .expect("external deficit JSON case")
        .clone();
    let prior_deficit_result_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-underskud-årsresultat-2026")
        .expect("prior deficit result JSON case")
        .clone();
    let negative_share_tax_carry_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-negativ-aktieskat-fremførsel-2026")
        .expect("negative share-tax carry JSON case")
        .clone();
    let dis_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-dis-2026")
        .expect("DIS JSON case")
        .clone();
    let dis_course_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-dis-kursus-2026")
        .expect("DIS course JSON case")
        .clone();
    let death_estate_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-dis-doedsbo-2026")
        .expect("death-estate DIS JSON case")
        .clone();
    let death_estate_share_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-dis-doedsbo-aktieindkomst-2026")
        .expect("death-estate share-income DIS JSON case")
        .clone();
    let death_estate_carryback_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-dis-doedsbo-carryback-2026")
        .expect("death-estate carryback DIS JSON case")
        .clone();
    let limited_taxpayer_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-dis-begraenset-skattepligt-2026")
        .expect("limited-taxpayer DIS JSON case")
        .clone();
    let hydrocarbon_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-dis-kulbrinte-2026")
        .expect("hydrocarbon DIS JSON case")
        .clone();
    let par32_mixed_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-par32-blandet-fordeling-2026")
        .expect("mixed-class KGL §32 JSON case")
        .clone();
    let fisher_union_transition_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-fisker-kontingentovergange-2026")
        .expect("fisher union-transition JSON case")
        .clone();
    let fisher_cross_year_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-fisker-nytårsfordeling-2026")
        .expect("fisher cross-year JSON case")
        .clone();
    let fisher_mixed_travel_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-fisker-blandede-rejser-2026")
        .expect("fisher mixed-travel JSON case")
        .clone();
    let fisher_mixed_travel_no_election_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-fisker-blandede-rejser-uden-valg-2026")
        .expect("non-elected fisher mixed-travel JSON case")
        .clone();
    let seafarer_commute_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-soemand-blandet-befordring-2026")
        .expect("mixed seafarer-commute JSON case")
        .clone();
    let freight_tax_case = json_input["cases"]
        .as_array()
        .expect("Personskat JSON cases")
        .iter()
        .find(|case| case["case_id"] == "personskat-ligningslov33-fragtskat-2026")
        .expect("LL § 33(9) freight-tax JSON case")
        .clone();
    hydrated_json_input["cases"] = Value::Array(vec![
        mixed_case,
        simultaneous_spouse_case.clone(),
        ordinary_share_loss_case,
        spouse_property_credit_case,
        partial_exemption_case,
        temporary_rental_case,
        spouse_rebate_recipient_case,
        partial_year_cap_case,
        annual_claim_case,
        partial_claim_case,
        currency_claim_case,
        carried_currency_claim_case,
        external_deficit_case,
        prior_deficit_result_case,
        negative_share_tax_carry_case,
        dis_case,
        dis_course_case,
        death_estate_case,
        death_estate_share_case,
        death_estate_carryback_case,
        limited_taxpayer_case,
        hydrocarbon_case,
        par32_mixed_case,
        fisher_union_transition_case,
        fisher_cross_year_case,
        fisher_mixed_travel_no_election_case,
        fisher_mixed_travel_case,
        seafarer_commute_case,
        freight_tax_case,
    ]);
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
    let json_freight_tax_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-ligningslov33-fragtskat-2026")
        .expect("JSON LL § 33(9) freight-tax result");
    let hydrated_freight_tax_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-ligningslov33-fragtskat-2026")
        .expect("hydrated XLSX LL § 33(9) freight-tax result");
    assert_eq!(
        hydrated_freight_tax_result["result"],
        json_freight_tax_result["result"]
    );
    assert_eq!(
        json_freight_tax_result["result"]["ligningslov33"]["input_gyldigt"],
        true
    );
    assert_eq!(
        json_freight_tax_result["result"]["ligningslov33"]["hovedpersons_nedslag_øre"],
        300_000
    );
    assert_eq!(
        json_freight_tax_result["result"]["ligningslov33"]["hovedperson"]["kreditgrupper"][0]
            ["skattebetalinger"]
            .as_array()
            .expect("mixed LL § 33 tax-payment results")
            .len(),
        2
    );
    assert_eq!(
        json_freight_tax_result["result"]["ligningslov33"]["hovedperson"]["fragtskat_ultimosaldi"]
            .as_array()
            .expect("LL § 33(9) closing freight-tax balances")
            .len(),
        0
    );
    let json_fisher_union_transition_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-fisker-kontingentovergange-2026")
        .expect("JSON fisher union-transition result");
    let hydrated_fisher_union_transition_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-fisker-kontingentovergange-2026")
        .expect("hydrated XLSX fisher union-transition result");
    assert_eq!(
        hydrated_fisher_union_transition_result["result"],
        json_fisher_union_transition_result["result"]
    );
    let fisher_union_transition_trace =
        &json_fisher_union_transition_result["result"]["ligningsfradrag"];
    assert_eq!(fisher_union_transition_trace["alle_input_gyldige"], true);
    assert_eq!(
        fisher_union_transition_trace["fiskerfradragssamordning"]
            ["kontingentperiodeundtagelser_gyldige"],
        true
    );
    assert_eq!(
        fisher_union_transition_trace["faglige_kontingenter"]["samlet_fradrag_kroner"],
        2_000
    );
    let preserved_union_dues = fisher_union_transition_trace["faglige_kontingenter"]["input"]
        ["kontingenter"]
        .as_array()
        .expect("preserved transition-period union dues");
    assert_eq!(preserved_union_dues.len(), 2);
    assert!(preserved_union_dues
        .iter()
        .any(|dues| dues["identifikation"] == "kontingent-før-registrering"));
    assert!(preserved_union_dues
        .iter()
        .any(|dues| dues["identifikation"] == "kontingent-efter-fuldt-ophør"));
    let json_fisher_cross_year_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-fisker-nytårsfordeling-2026")
        .expect("JSON fisher cross-year result");
    let hydrated_fisher_cross_year_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-fisker-nytårsfordeling-2026")
        .expect("hydrated XLSX fisher cross-year result");
    assert_eq!(
        hydrated_fisher_cross_year_result["result"],
        json_fisher_cross_year_result["result"]
    );
    let fisher_cross_year_annual = &json_fisher_cross_year_result["result"]["ligningsfradrag"]
        ["fiskerfradragssamordning"]["fiskerfradrag"];
    assert_eq!(fisher_cross_year_annual["alle_input_gyldige"], true);
    assert_eq!(
        fisher_cross_year_annual["påbegyndte_havdage_før_årsloft"],
        1
    );
    assert_eq!(fisher_cross_year_annual["samlet_fradrag_kroner"], 190);
    let fisher_cross_year_trip = &fisher_cross_year_annual["fangstturresultater"][0];
    assert_eq!(
        fisher_cross_year_trip["fangsttur_overlapper_indkomståret"],
        true
    );
    assert_eq!(
        fisher_cross_year_trip["samlede_påbegyndte_havdage_på_fangstturen"],
        2
    );
    assert_eq!(fisher_cross_year_trip["påbegyndte_havdage"], 1);
    let json_fisher_mixed_travel_no_election_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-fisker-blandede-rejser-uden-valg-2026")
        .expect("JSON non-elected fisher mixed-travel result");
    let hydrated_fisher_mixed_travel_no_election_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-fisker-blandede-rejser-uden-valg-2026")
        .expect("hydrated XLSX non-elected fisher mixed-travel result");
    assert_eq!(
        hydrated_fisher_mixed_travel_no_election_result["result"],
        json_fisher_mixed_travel_no_election_result["result"]
    );
    let fisher_mixed_travel_no_election_trace =
        &json_fisher_mixed_travel_no_election_result["result"]["ligningsfradrag"];
    assert_eq!(
        fisher_mixed_travel_no_election_trace["alle_input_gyldige"],
        true
    );
    assert_eq!(
        fisher_mixed_travel_no_election_trace["fiskerfradragssamordning"]["fiskerfradrag"]
            ["fradrag_foretaget_efter_par9g"],
        false
    );
    assert_eq!(
        fisher_mixed_travel_no_election_trace["rejser"]["samlet_skattefri_godtgørelse_kroner"],
        800
    );
    assert_eq!(
        fisher_mixed_travel_no_election_trace["rejser"]["samlet_skattepligtig_godtgørelse_kroner"],
        0
    );
    assert_eq!(
        fisher_mixed_travel_no_election_trace["rejser"]["samlet_ll9a_fradrag_efter_årsloft_kroner"],
        1_254
    );
    assert_eq!(
        fisher_mixed_travel_no_election_trace["rejser"]["ølogi"]
            ["fradrag_efter_fælles_årsloft_kroner"],
        268
    );
    assert_eq!(
        json_fisher_mixed_travel_no_election_result["result"]["skat"]["bruttoløn_kroner"],
        600_000
    );
    assert_eq!(
        json_fisher_mixed_travel_no_election_result["result"]["skat"]
            ["arbejdsmarkedsbidrag_kroner"],
        48_000
    );
    let json_fisher_mixed_travel_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-fisker-blandede-rejser-2026")
        .expect("JSON fisher mixed-travel result");
    let hydrated_fisher_mixed_travel_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-fisker-blandede-rejser-2026")
        .expect("hydrated XLSX fisher mixed-travel result");
    assert_eq!(
        hydrated_fisher_mixed_travel_result["result"],
        json_fisher_mixed_travel_result["result"]
    );
    let fisher_mixed_travel_trace = &json_fisher_mixed_travel_result["result"]["ligningsfradrag"];
    assert_eq!(fisher_mixed_travel_trace["alle_input_gyldige"], true);
    assert_eq!(
        fisher_mixed_travel_trace["rejser_før_ligningslov9g"]
            ["samlet_skattefri_godtgørelse_kroner"],
        800
    );
    assert_eq!(
        fisher_mixed_travel_trace["rejser_før_ligningslov9g"]
            ["samlet_ll9a_fradrag_efter_årsloft_kroner"],
        1_254
    );
    assert_eq!(
        fisher_mixed_travel_trace["rejser"]["samlet_skattefri_godtgørelse_kroner"],
        400
    );
    assert_eq!(
        fisher_mixed_travel_trace["rejser"]["samlet_skattepligtig_godtgørelse_kroner"],
        400
    );
    assert_eq!(
        fisher_mixed_travel_trace["rejser"]["samlet_ll9a_fradrag_efter_årsloft_kroner"],
        493
    );
    assert_eq!(
        fisher_mixed_travel_trace["rejser"]["ølogi"]["stk11_udelukket"],
        true
    );
    assert_eq!(
        fisher_mixed_travel_trace["samlet_afskåret_efter_ligningslov9g_kroner"],
        761
    );
    let fisher_mixed_travel_results = fisher_mixed_travel_trace["rejser"]["rejseresultater"]
        .as_array()
        .expect("fisher mixed-travel results");
    let fisher_trip = fisher_mixed_travel_results
        .iter()
        .find(|result| result["fakta"]["identifikation"] == "fisker-rejse")
        .expect("fishing-linked travel result");
    assert_eq!(
        fisher_trip["ekstern_udelukkelse"]["$variant"],
        "Ll9AUdelukketEfterValgtFiskerfradragPar9G"
    );
    assert_eq!(fisher_trip["stk1_til_stk9_udelukket"], true);
    let unrelated_trip = fisher_mixed_travel_results
        .iter()
        .find(|result| result["fakta"]["identifikation"] == "andet-job-rejse")
        .expect("unrelated-job travel result");
    assert_eq!(
        unrelated_trip["ekstern_udelukkelse"]["$variant"],
        "Ll9AIngenEksternUdelukkelse"
    );
    assert_eq!(unrelated_trip["stk1_til_stk9_udelukket"], false);
    assert_eq!(unrelated_trip["skattefri_godtgørelse_kroner"], 400);
    assert_eq!(unrelated_trip["fradrag_før_årsloft_kroner"], 493);
    assert_eq!(
        json_fisher_mixed_travel_result["result"]["skat"]["bruttoløn_kroner"],
        600_400
    );
    assert_eq!(
        json_fisher_mixed_travel_result["result"]["skat"]["arbejdsmarkedsbidrag_kroner"],
        48_032
    );
    let json_seafarer_commute_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-soemand-blandet-befordring-2026")
        .expect("JSON mixed seafarer-commute result");
    let hydrated_seafarer_commute_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-soemand-blandet-befordring-2026")
        .expect("hydrated XLSX mixed seafarer-commute result");
    assert_eq!(
        hydrated_seafarer_commute_result["result"],
        json_seafarer_commute_result["result"]
    );
    let seafarer_commute_trace = &json_seafarer_commute_result["result"]["ligningsfradrag"];
    assert_eq!(seafarer_commute_trace["alle_input_gyldige"], true);
    assert_eq!(
        seafarer_commute_trace["befordring"]["samlet_par9c_grundfradrag_kroner"],
        48_184
    );
    assert_eq!(
        seafarer_commute_trace["befordring"]["lavindkomsttillæg_kroner"],
        30_800
    );
    assert_eq!(
        seafarer_commute_trace["befordring_efter_sømandsbeskatningslov4"]["input"]["forhold"]
            .as_array()
            .expect("retained commute relationships")
            .len(),
        1
    );
    assert_eq!(
        seafarer_commute_trace["befordring_efter_sømandsbeskatningslov4"]["input"]["forhold"][0]
            ["identifikation"],
        "landbefordring-json-xlsx"
    );
    assert_eq!(
        seafarer_commute_trace["befordring_efter_sømandsbeskatningslov4"]
            ["samlet_par9c_grundfradrag_kroner"],
        24_092
    );
    assert_eq!(
        seafarer_commute_trace["befordring_efter_sømandsbeskatningslov4"]
            ["lavindkomsttillæg_kroner"],
        15_418
    );
    assert_eq!(
        seafarer_commute_trace["befordring_fradrag_anvendt_kroner"],
        39_510
    );
    let json_pbl15a_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-pbl15a-relationer-2026")
        .expect("JSON PBL § 15 A relation result");
    assert_eq!(xlsx_pbl15a_canonical_result, json_pbl15a_result["result"]);
    let json_ordinary_share_loss_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-renter-befordring-2026")
        .expect("JSON ordinary ABL share-loss result");
    let hydrated_ordinary_share_loss_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-renter-befordring-2026")
        .expect("hydrated XLSX ordinary ABL share-loss result");
    assert_eq!(
        hydrated_ordinary_share_loss_result["result"],
        json_ordinary_share_loss_result["result"]
    );
    let ordinary_share_loss_result = &json_ordinary_share_loss_result["result"];
    assert_eq!(
        ordinary_share_loss_result["aktieavance"]["aktieindkomst_kroner"],
        -18_000
    );
    assert_eq!(
        ordinary_share_loss_result["aktieindkomst_parår"]["egen_skat_kroner"],
        -4_860
    );
    assert_eq!(
        ordinary_share_loss_result["negativ_aktieindkomstskat"]["egen"]["negativ_skat_kroner"],
        4_860
    );
    assert_eq!(
        ordinary_share_loss_result["negativ_aktieindkomstskat"]["egen"]
            ["modregnet_i_egen_slutskat_kroner"],
        4_860
    );
    assert_eq!(
        ordinary_share_loss_result["negativ_aktieindkomstskat"]["egen"]["fremført_kroner"],
        0
    );
    assert_eq!(
        ordinary_share_loss_result["samlet_skat_efter_negativ_aktieindkomstskat_kroner"],
        ordinary_share_loss_result["samlet_skat_inkl_endelig_aktieindkomstskat_kroner"]
            .as_i64()
            .expect("gross tax before negative share-income tax")
            - 4_860
    );
    let json_partial_exemption_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-ejendomsskat-delvis-fritagelse-2025")
        .expect("JSON partial property-tax exemption result");
    let hydrated_partial_exemption_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-ejendomsskat-delvis-fritagelse-2025")
        .expect("hydrated XLSX partial property-tax exemption result");
    assert_eq!(
        hydrated_partial_exemption_result["result"],
        json_partial_exemption_result["result"]
    );
    let partial_exemption_property =
        &json_partial_exemption_result["result"]["ejendomsskatter"]["ejendomsresultater"][0];
    assert_eq!(
        partial_exemption_property["overgang"]["rabat"]["$variant"],
        "EjskBeregnetRabat"
    );
    let partial_exemption_basis =
        &partial_exemption_property["overgang"]["rabat"]["resultat"]["eget_grundlag_2024"];
    assert_eq!(partial_exemption_basis["ny_grundskyld_øre"], 306_000);
    assert_eq!(partial_exemption_basis["tidligere_grundskyld_øre"], 200_000);
    assert_eq!(
        partial_exemption_basis["forskelsbeløb_grundskyld_øre"],
        106_000
    );
    assert_eq!(partial_exemption_basis["rabat_grundskyld_øre"], 106_000);
    let json_temporary_rental_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-ejendomsskat-midlertidig-udlejning-2025")
        .expect("JSON temporary-rental property-tax result");
    let hydrated_temporary_rental_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-ejendomsskat-midlertidig-udlejning-2025")
        .expect("hydrated XLSX temporary-rental property-tax result");
    assert_eq!(
        hydrated_temporary_rental_result["result"],
        json_temporary_rental_result["result"]
    );
    let temporary_rental_property =
        &json_temporary_rental_result["result"]["ejendomsskatter"]["ejendomsresultater"][0];
    assert_eq!(
        temporary_rental_property["ordinært_resultat"]["ejendomsværdiskat"]["periode"]
            ["skattepligtige_dage"],
        30
    );
    let temporary_rental_rebate = &temporary_rental_property["overgang"]["rabat"]["resultat"];
    assert_eq!(
        temporary_rental_rebate["par15_og_par42_perioder_konsistente"],
        true
    );
    assert_eq!(
        temporary_rental_property["ejendomsværdiskat_før_overgang_øre"],
        51_500
    );
    assert_eq!(
        temporary_rental_rebate["rabat_ejendomsværdiskat_øre"],
        9_083
    );
    assert_eq!(temporary_rental_property["ejendomsværdiskat_øre"], 42_417);
    let json_spouse_rebate_recipient_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-ejendomsskat-aegtefaellemodtager-2025")
        .expect("JSON spouse rebate recipient result");
    let hydrated_spouse_rebate_recipient_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-ejendomsskat-aegtefaellemodtager-2025")
        .expect("hydrated XLSX spouse rebate recipient result");
    assert_eq!(
        hydrated_spouse_rebate_recipient_result["result"],
        json_spouse_rebate_recipient_result["result"]
    );
    let spouse_rebate_recipient_property =
        &json_spouse_rebate_recipient_result["result"]["ejendomsskatter"]["ejendomsresultater"][0];
    assert_eq!(
        spouse_rebate_recipient_property["overgang"]["rabat"]["$variant"],
        "EjskBeregnetRabat"
    );
    let spouse_rebate_recipient =
        &spouse_rebate_recipient_property["overgang"]["rabat"]["resultat"];
    assert_eq!(spouse_rebate_recipient["eget_grundlag_2024"], Value::Null);
    assert_eq!(
        spouse_rebate_recipient["rabat_ejendomsværdiskat_før_par41_stk3_øre"],
        27_250
    );
    assert_eq!(
        spouse_rebate_recipient_property["ejendomsværdiskat_før_overgang_øre"],
        154_500
    );
    assert_eq!(
        spouse_rebate_recipient_property["ejendomsværdiskat_øre"],
        127_250
    );
    let json_partial_year_cap_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-ejendomsskat-halvårsloft-2025")
        .expect("JSON partial-year property-tax cap result");
    let hydrated_partial_year_cap_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-ejendomsskat-halvårsloft-2025")
        .expect("hydrated XLSX partial-year property-tax cap result");
    assert_eq!(
        hydrated_partial_year_cap_result["result"],
        json_partial_year_cap_result["result"]
    );
    let partial_year_cap_property =
        &json_partial_year_cap_result["result"]["ejendomsskatter"]["ejendomsresultater"][0];
    assert_eq!(
        partial_year_cap_property["grundskyld_før_overgang_øre"],
        408_000
    );
    assert_eq!(partial_year_cap_property["grundskyld_øre"], 279_380);
    assert_eq!(
        partial_year_cap_property["overgang"]["stigningsbegrænsning"]["$variant"],
        "EjskBeregnetStigningsbegrænsning"
    );
    let partial_year_cap =
        &partial_year_cap_property["overgang"]["stigningsbegrænsning"]["resultat"];
    assert_eq!(partial_year_cap["ordinær_grundskyld_helår_øre"], 816_000);
    assert_eq!(partial_year_cap["ordinær_grundskyld_øre"], 408_000);
    assert_eq!(
        partial_year_cap["grundskyld_efter_stigningsbegrænsning_helår_øre"],
        558_760
    );
    assert_eq!(
        partial_year_cap["grundskyld_efter_stigningsbegrænsning_øre"],
        279_380
    );
    assert_eq!(partial_year_cap["begrænsning_øre"], 128_620);
    let json_spouse_property_credit_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-par8a-aegtefaelle-ejendomsskatter-2025")
        .expect("JSON spouse property-tax credit result");
    let hydrated_spouse_property_credit_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-par8a-aegtefaelle-ejendomsskatter-2025")
        .expect("hydrated XLSX spouse property-tax credit result");
    assert_eq!(
        hydrated_spouse_property_credit_result["result"],
        json_spouse_property_credit_result["result"]
    );
    let spouse_property_credit_result = &json_spouse_property_credit_result["result"];
    assert_eq!(
        spouse_property_credit_result["ægtefælles_ejendomsskatter"]["$variant"],
        "BeregnedeÆgtefælleEjendomsskatter"
    );
    assert_eq!(
        spouse_property_credit_result["ægtefælles_ejendomsskatter"]["resultat"]
            ["samlet_ejendomsskat_kroner"],
        6_012
    );
    assert_eq!(
        spouse_property_credit_result["negativ_aktieindkomstskat"]["input"]
            ["ægtefælles_slutskat_før_negativ_skat_kroner"],
        6_012
    );
    assert_eq!(
        spouse_property_credit_result["negativ_aktieindkomstskat"]["egen"]
            ["modregnet_i_ægtefælles_slutskat_kroner"],
        6_012
    );
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
    let json_partial_claim_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-delrealisering-2026")
        .expect("JSON partial KGL claim result");
    let hydrated_partial_claim_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-delrealisering-2026")
        .expect("hydrated XLSX partial KGL claim result");
    assert_eq!(
        hydrated_partial_claim_result["result"],
        json_partial_claim_result["result"]
    );
    let partial_claim_trace =
        &hydrated_partial_claim_result["result"]["kapitalindkomst"]["kursgevinst_resultat"];
    assert_eq!(partial_claim_trace["input_gyldigt"], true);
    assert_eq!(
        partial_claim_trace["årets_samlede_netto_efter_par14_kroner"],
        2_500
    );
    assert_eq!(
        partial_claim_trace["øvrige_instrumentresultat"]["fordringsresultater"][0]["forløb"]
            ["allokeringsprincip"]["$variant"],
        "KglÅrsnettoFifoEfterPar26Stk5"
    );
    assert_eq!(
        partial_claim_trace["øvrige_instrumentresultat"]["fordringsresultater"][0]["opgørelser"]
            .as_array()
            .expect("partial KGL event calculations")
            .len(),
        2
    );
    assert_eq!(
        partial_claim_trace["øvrige_instrumentresultat"]["fordringsresultater"][0]["opgørelser"][0]
            ["hændelsesresultat"]["opgørelsesgrundlag_kroner"],
        5_000
    );
    assert_eq!(
        partial_claim_trace["øvrige_instrumentresultat"]["fordringsresultater"][0]["opgørelser"][1]
            ["hændelsesresultat"]["opgørelsesgrundlag_kroner"],
        15_000
    );
    assert_eq!(
        partial_claim_trace["kursgevinstlov_resultater"][0]["skattepligtig_gevinst_kroner"],
        2_500
    );
    assert_eq!(
        partial_claim_trace["kursgevinstlov_resultater"][1]["netto_efter_kursgevinstloven_kroner"],
        0
    );
    assert_eq!(
        partial_claim_trace["øvrige_instrumentresultat"]["fordringsresultater"][0]["forløb"]
            ["position_ultimo"]["trancher"][0]["identifikation"],
        "tranche-2026-b"
    );
    assert_eq!(
        partial_claim_trace["øvrige_instrumentresultat"]["fordringsresultater"][0]["forløb"]
            ["position_ultimo"]["trancher"][0]["resterende_mængde"],
        50
    );
    assert_eq!(
        partial_claim_trace["øvrige_instrumentresultat"]["fordringsresultater"][0]["forløb"]
            ["position_ultimo"]["trancher"][0]["resterende_anskaffelsessum_kroner"],
        10_000
    );
    assert_eq!(
        hydrated_partial_claim_result["result"]["kapitalindkomst"]["kapitalindkomst_resultat"]
            ["nettokapitalindkomst_kroner"],
        2_500
    );
    let json_currency_claim_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-valutakomponenter-2026")
        .expect("JSON foreign-currency KGL claim result");
    let hydrated_currency_claim_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-valutakomponenter-2026")
        .expect("hydrated XLSX foreign-currency KGL claim result");
    assert_eq!(
        hydrated_currency_claim_result["result"],
        json_currency_claim_result["result"]
    );
    let currency_claim_trace =
        &hydrated_currency_claim_result["result"]["kapitalindkomst"]["kursgevinst_resultat"];
    assert_eq!(currency_claim_trace["input_gyldigt"], true);
    assert_eq!(
        currency_claim_trace["årets_samlede_netto_efter_par14_kroner"],
        14_000
    );
    assert_eq!(
        currency_claim_trace["øvrige_instrumentresultat"]["valutainstrumentresultater"][0]
            ["kredit_eller_pris"]["årets_rå_netto_kroner"],
        -10_000
    );
    assert_eq!(
        currency_claim_trace["øvrige_instrumentresultat"]["valutainstrumentresultater"][0]
            ["valutakurs"]["årets_rå_netto_kroner"],
        24_000
    );
    assert_eq!(
        currency_claim_trace["kursgevinstlov_resultater"][0]["tab_afskåret_kroner"],
        10_000
    );
    assert_eq!(
        currency_claim_trace["kursgevinstlov_resultater"][1]["skattepligtig_gevinst_kroner"],
        24_000
    );
    assert_eq!(
        hydrated_currency_claim_result["result"]["kapitalindkomst"]["kapitalindkomst_resultat"]
            ["nettokapitalindkomst_kroner"],
        24_000
    );
    let json_carried_currency_claim_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-valutaposition-videreført-2026")
        .expect("JSON carried foreign-currency KGL claim result");
    let hydrated_carried_currency_claim_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-valutaposition-videreført-2026")
        .expect("hydrated XLSX carried foreign-currency KGL claim result");
    assert_eq!(
        hydrated_carried_currency_claim_result["result"],
        json_carried_currency_claim_result["result"]
    );
    let carried_currency_claim_trace = &hydrated_carried_currency_claim_result["result"]
        ["kapitalindkomst"]["kursgevinst_resultat"];
    assert_eq!(carried_currency_claim_trace["input_gyldigt"], true);
    assert_eq!(
        carried_currency_claim_trace["øvrige_instrumentresultat"]["par25_valgresultat"]
            ["input_gyldigt"],
        true
    );
    assert_eq!(
        carried_currency_claim_trace["øvrige_instrumentresultat"]["valutainstrumentresultater"][0]
            ["valutakurs"]["tidligere_medregnet_øre"],
        2_400_000
    );
    assert_eq!(
        carried_currency_claim_trace["øvrige_instrumentresultat"]["valutainstrumentresultater"][0]
            ["valutakurs"]["årets_rå_netto_kroner"],
        8_000
    );
    assert_eq!(
        carried_currency_claim_trace["øvrige_instrumentresultat"]["valutainstrumentresultater"][0]
            ["position_ultimo"]["par25_valgpositioner"]["valutakursændringer"]["princip"]
            ["$variant"],
        "KglLagerprincip"
    );
    assert_eq!(
        hydrated_carried_currency_claim_result["result"]["kapitalindkomst"]
            ["kapitalindkomst_resultat"]["nettokapitalindkomst_kroner"],
        8_000
    );
    let json_dis_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-dis-2026")
        .expect("JSON DIS result");
    let hydrated_dis_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-dis-2026")
        .expect("hydrated XLSX DIS result");
    assert_eq!(hydrated_dis_result["result"], json_dis_result["result"]);
    let json_death_estate_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-dis-doedsbo-2026")
        .expect("JSON death-estate DIS result");
    let hydrated_death_estate_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-dis-doedsbo-2026")
        .expect("hydrated XLSX death-estate DIS result");
    assert_eq!(
        hydrated_death_estate_result["result"],
        json_death_estate_result["result"]
    );
    let death_estate_annual =
        &hydrated_death_estate_result["result"]["personlig_indkomst"]["sømandsbeskatning"];
    assert_eq!(death_estate_annual["alle_input_gyldige"], true);
    assert_eq!(death_estate_annual["beregningsklar"], true);
    assert_eq!(
        death_estate_annual["skattepligtskategori"]["$variant"],
        "SøblDødsboSkattepligtKategori"
    );
    assert_eq!(
        death_estate_annual["dødsbo_lempelse"]["forholdsmæssig_lempelse_kroner"],
        86_500
    );
    assert_eq!(
        death_estate_annual["dødsbo_lempelse"]["dødsboskat_efter_søbl5_kroner"],
        173_000
    );
    assert_eq!(
        death_estate_annual["dødsbo_lempelse"]["arbejdsmarkedsbidrag_kroner"],
        0
    );
    assert_eq!(
        hydrated_death_estate_result["result"]["sømandsbeskatning"]["input_gyldigt"],
        false
    );

    let json_death_estate_share_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-dis-doedsbo-aktieindkomst-2026")
        .expect("JSON death-estate share-income DIS result");
    let hydrated_death_estate_share_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-dis-doedsbo-aktieindkomst-2026")
        .expect("hydrated XLSX death-estate share-income DIS result");
    assert_eq!(
        hydrated_death_estate_share_result["result"],
        json_death_estate_share_result["result"]
    );
    let death_estate_share_annual =
        &hydrated_death_estate_share_result["result"]["personlig_indkomst"]["sømandsbeskatning"];
    assert_eq!(death_estate_share_annual["alle_input_gyldige"], false);
    assert_eq!(death_estate_share_annual["beregningsklar"], false);
    assert_eq!(
        death_estate_share_annual["dødsbo_lempelse"]["input_gyldigt"],
        false
    );
    assert_eq!(
        death_estate_share_annual["input"]["dødsboskattegrundlag"]["input"]
            ["aktieindkomstgrundlag"],
        serde_json::json!({
            "$variant": "Dbl32OpgjortAktieindkomstEfterPar21",
            "aktieindkomst_kroner": 100_000,
            "dokumentreference": "boopgørelse-aktieindkomst-2026"
        })
    );
    assert_eq!(
        death_estate_share_annual["input"]["dødsboskattegrundlag"]["input"]["ægtefælleforhold"]
            ["par67_stk7_progressionsforhold"],
        serde_json::json!({
            "$variant": "Dbl67Stk7FørstafdødesSærboEndeligtSkatteberegnet",
            "anvendt_ekstra_progressionsgrænse_kroner": 30_000,
            "dokumentreference": "skatteberegning-førstafdødes-særbo-2026"
        })
    );
    let death_estate_share_tax = &death_estate_share_annual["dødsbo_lempelse"]
        ["dødsboskat_før_søbl5"]["aktieskatteberegning"];
    assert_eq!(
        death_estate_share_tax["progressionsgrænse_før_par67_stk7_kroner"],
        158_800
    );
    assert_eq!(
        death_estate_share_tax["anvendt_ekstra_progressionsgrænse_i_førstafdødes_særbo_kroner"],
        30_000
    );
    assert_eq!(
        death_estate_share_tax["effektiv_progressionsgrænse_kroner"],
        128_800
    );

    let json_death_estate_carryback_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-dis-doedsbo-carryback-2026")
        .expect("JSON death-estate carryback DIS result");
    let hydrated_death_estate_carryback_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-dis-doedsbo-carryback-2026")
        .expect("hydrated XLSX death-estate carryback DIS result");
    assert_eq!(
        hydrated_death_estate_carryback_result["result"],
        json_death_estate_carryback_result["result"]
    );
    let death_estate_carryback_annual = &hydrated_death_estate_carryback_result["result"]
        ["personlig_indkomst"]["sømandsbeskatning"];
    assert_eq!(death_estate_carryback_annual["alle_input_gyldige"], false);
    assert_eq!(death_estate_carryback_annual["beregningsklar"], false);
    let death_estate_carryback_trace = &death_estate_carryback_annual["dødsbo_lempelse"]
        ["dødsboskat_før_søbl5"]["carryback_efter_par31"];
    assert_eq!(death_estate_carryback_trace["historik_komplet"], true);
    assert_eq!(
        death_estate_carryback_trace["historikloft_efter_stk2_stk3_kroner"],
        80_000
    );
    assert_eq!(
        death_estate_carryback_trace["udbetaling_efter_historikloft_kroner"],
        60_000
    );
    assert_eq!(death_estate_carryback_trace["beregningsklar"], true);
    let death_estate_carryback_distribution = &death_estate_carryback_trace["fordeling_efter_stk4"];
    assert_eq!(
        death_estate_carryback_distribution["fællesboets_bobeskatningsperiode_gyldig"],
        true
    );
    assert_eq!(
        death_estate_carryback_distribution["særboets_bobeskatningsperiode_gyldig"],
        true
    );
    assert_eq!(
        death_estate_carryback_distribution["skæringsdage_afstemt"],
        true
    );
    assert_eq!(
        death_estate_carryback_distribution["udbetaling_til_fællesbo_kroner"],
        36_000
    );
    assert_eq!(
        death_estate_carryback_distribution["udbetaling_til_særbo_kroner"],
        24_000
    );
    let hydrated_carryback_source = &death_estate_carryback_annual["input"]["dødsboskattegrundlag"]
        ["input"]["carrybackgrundlag"];
    assert_eq!(
        hydrated_carryback_source["betalte_årsskatter"]
            .as_array()
            .expect("hydrated § 31 annual tax history")
            .len(),
        2
    );
    assert_eq!(
        hydrated_carryback_source["betalte_årsskatter"][0]["dokumentreference"],
        "årsopgørelse-afdøde-2024"
    );
    assert_eq!(
        hydrated_carryback_source["betalte_årsskatter"][1]["dokumentreference"],
        "årsopgørelse-afdøde-2025"
    );
    assert_eq!(
        hydrated_carryback_source["bofordelingsgrundlag"]["fællesbo"]["boopgørelsens_skæringsdag"],
        serde_json::json!({ "år": 2026, "måned": 8, "dag": 20 })
    );
    assert_eq!(
        hydrated_carryback_source["bofordelingsgrundlag"]["særbo"]["boopgørelsens_skæringsdag"],
        serde_json::json!({ "år": 2026, "måned": 8, "dag": 31 })
    );

    let json_limited_taxpayer_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-dis-begraenset-skattepligt-2026")
        .expect("JSON limited-taxpayer DIS result");
    let hydrated_limited_taxpayer_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-dis-begraenset-skattepligt-2026")
        .expect("hydrated XLSX limited-taxpayer DIS result");
    assert_eq!(
        hydrated_limited_taxpayer_result["result"],
        json_limited_taxpayer_result["result"]
    );
    let limited_taxpayer_annual =
        &hydrated_limited_taxpayer_result["result"]["personlig_indkomst"]["sømandsbeskatning"];
    assert_eq!(limited_taxpayer_annual["alle_input_gyldige"], true);
    assert_eq!(limited_taxpayer_annual["beregningsklar"], true);
    assert_eq!(
        limited_taxpayer_annual["begrænset_skattefritagelse"]["løn_uden_dansk_skat_kroner"],
        300_000
    );
    assert_eq!(
        limited_taxpayer_annual["begrænset_skattefritagelse"]["dansk_indkomstskat_kroner"],
        0
    );
    assert_eq!(
        limited_taxpayer_annual["begrænset_skattefritagelse"]["arbejdsmarkedsbidrag_kroner"],
        0
    );
    assert_eq!(
        hydrated_limited_taxpayer_result["result"]["sømandsbeskatning"]["input_gyldigt"],
        false
    );

    let json_hydrocarbon_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-dis-kulbrinte-2026")
        .expect("JSON hydrocarbon DIS result");
    let hydrated_hydrocarbon_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-dis-kulbrinte-2026")
        .expect("hydrated XLSX hydrocarbon DIS result");
    assert_eq!(
        hydrated_hydrocarbon_result["result"],
        json_hydrocarbon_result["result"]
    );
    let hydrocarbon_annual =
        &hydrated_hydrocarbon_result["result"]["personlig_indkomst"]["sømandsbeskatning"];
    assert_eq!(hydrocarbon_annual["alle_input_gyldige"], true);
    assert_eq!(hydrocarbon_annual["beregningsklar"], true);
    assert_eq!(
        hydrocarbon_annual["kulbrinte_lempelse"]["indkomstskattenedsættelse_kroner"],
        150_000
    );
    assert_eq!(
        hydrocarbon_annual["kulbrinte_lempelse"]["arbejdsmarkedsbidragsfritagelse_kroner"],
        40_000
    );
    assert_eq!(
        hydrocarbon_annual["kulbrinte_lempelse"]["samlet_skat_efter_søbl5b_kroner"],
        0
    );
    assert_eq!(
        hydrated_hydrocarbon_result["result"]["sømandsbeskatning"]["input_gyldigt"],
        false
    );
    let json_dis_course_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-dis-kursus-2026")
        .expect("JSON DIS course result");
    let hydrated_dis_course_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-dis-kursus-2026")
        .expect("hydrated XLSX DIS course result");
    assert_eq!(
        hydrated_dis_course_result["result"],
        json_dis_course_result["result"]
    );
    let dis_course_income_result = &hydrated_dis_course_result["result"]["personlig_indkomst"]
        ["sømandsbeskatning"]["indkomstresultater"][0];
    assert_eq!(dis_course_income_result["input_gyldigt"], true);
    assert_eq!(dis_course_income_result["arbejdsrolle_omfattet"], true);
    assert_eq!(
        dis_course_income_result["fakta"]["arbejde"]["arbejdsrolle"]["opgørelse"]["kursusperioder"]
            .as_array()
            .expect("hydrated dated DIS course periods")
            .len(),
        3
    );
    assert_eq!(
        dis_course_income_result["retsgrundlag"]["$variant"],
        "SøblPar5Stk1"
    );
    let dis_annual_result =
        &hydrated_dis_result["result"]["personlig_indkomst"]["sømandsbeskatning"];
    assert_eq!(
        dis_annual_result["årsdriftsgrundlag"]["input_gyldigt"],
        true
    );
    assert_eq!(
        dis_annual_result["årsdriftsgrundlag"]["resultater"]
            .as_array()
            .expect("SØBL § 6 annual-vessel results")
            .len(),
        1
    );
    assert_eq!(
        dis_annual_result["årsdriftsgrundlag"]["resultater"][0]["søtransportminutter_før_ventetid"],
        5_000
    );
    assert_eq!(
        dis_annual_result["årsdriftsgrundlag"]["resultater"][0]["ventetid_til_søtransport_tæller"],
        10_000_000
    );
    assert_eq!(
        dis_annual_result["årsdriftsgrundlag"]["resultater"][0]["ventetidsfordeling_nævner"],
        10_000
    );
    assert_eq!(
        dis_annual_result["årsdriftsgrundlag"]["resultater"][0]
            ["søtransport_inkl_fordelt_ventetid_tæller"],
        60_000_000
    );
    assert_eq!(
        dis_annual_result["årsdriftsgrundlag"]["resultater"][0]["samlet_driftstid_tæller"],
        120_000_000
    );
    assert!(dis_annual_result["indkomstresultater"]
        .as_array()
        .expect("SØBL income results")
        .iter()
        .all(|result| result["årsdriftsrelation"]["$variant"] == "Søbl6ÅrsdriftTilknyttet"));
    let ligningslov7u_allocation = &dis_annual_result["ligningslov7u_bundfradrag"];
    assert_eq!(
        ligningslov7u_allocation["direkte_dis_nettoløn_før_bundfradrag_kroner"],
        20_000
    );
    assert_eq!(
        ligningslov7u_allocation["anden_indkomst_før_bundfradrag_kroner"],
        60_000
    );
    assert_eq!(
        ligningslov7u_allocation["bundfradrag_fordelt_til_dis_kroner"],
        2_000
    );
    assert_eq!(
        ligningslov7u_allocation["bundfradrag_fordelt_til_anden_indkomst_kroner"],
        6_000
    );
    assert_eq!(
        dis_annual_result["samlet_dis_personlig_indkomst_før_ligningslov7u_bundfradrag_kroner"],
        520_000
    );
    assert_eq!(
        dis_annual_result["samlet_dis_personlig_indkomst_kroner"],
        518_000
    );
    assert_eq!(
        dis_annual_result["samlet_anden_ligningslov7u_indkomst_efter_bundfradrag_kroner"],
        54_000
    );
    assert_eq!(
        hydrated_dis_result["result"]["skat"]["bruttoløn_kroner"],
        354_000
    );
    assert_eq!(
        hydrated_dis_result["result"]["skat"]["arbejdsmarkedsbidrag_kroner"],
        28_320
    );
    assert_eq!(
        hydrated_dis_result["result"]["skat"]["personlig_indkomst_efter_am_kroner"],
        843_680
    );
    assert_eq!(
        hydrated_dis_result["result"]["sømandsbeskatning"]["dis_personlig_indkomst_kroner"],
        518_000
    );
    assert_eq!(
        hydrated_dis_result["result"]["sømandsbeskatning"]["input_gyldigt"],
        true
    );
    assert!(
        hydrated_dis_result["result"]["sømandsbeskatning"]["samlet_lempelse_kroner"]
            .as_i64()
            .expect("DIS relief")
            > 0
    );
    let exact_dis_result = &hydrated_dis_result["result"]["sømandsbeskatning_eksakt"];
    assert_eq!(exact_dis_result["input_gyldigt"], true);
    let exact_dis_relief_øre = exact_dis_result["samlet_lempelse_øre"]
        .as_i64()
        .expect("exact DIS relief in øre");
    assert!(exact_dis_relief_øre > 0);
    assert_ne!(exact_dis_relief_øre % 100, 0);
    assert_eq!(
        exact_dis_result["samlet_lempelse_kroner_kompatibilitetsprojektion"],
        exact_dis_relief_øre / 100
    );
    assert!(exact_dis_result["komponenter"]
        .as_array()
        .expect("exact DIS relief components")
        .iter()
        .any(|component| {
            component["forholdsmæssig_beregning"]["nævner"]
                .as_i64()
                .is_some_and(|denominator| denominator > 1)
                && component["forholdsmæssig_lempelse_øre"]
                    .as_i64()
                    .is_some_and(|relief| relief % 100 != 0)
        }));
    assert_eq!(
        hydrated_dis_result["result"]["eksakt_ordinær_skat_efter_dis_øre"],
        exact_dis_result["samlet_skat_inkl_am_efter_dis_lempelse_øre"]
    );
    assert_eq!(
        exact_dis_result["samlet_skat_inkl_am_efter_dis_lempelse_øre"]
            .as_i64()
            .expect("exact tax after DIS relief"),
        exact_dis_result["arbejdsmarkedsbidrag_øre"]
            .as_i64()
            .expect("exact labour-market contribution")
            + exact_dis_result["indkomstskat_efter_dis_lempelse_øre"]
                .as_i64()
                .expect("exact income tax after DIS relief")
    );
    assert_eq!(
        hydrated_dis_result["result"]["sømandsbeskatning"]["par13_stk5_lempelsesgrundlag"]
            ["$variant"],
        "SømandsbeskatningslovPar6"
    );
    assert_eq!(
        hydrated_dis_result["result"]["samlet_skat_inkl_endelig_aktieindkomstskat_kroner"]
            .as_i64()
            .expect("final income tax"),
        hydrated_dis_result["result"]["skat"]["samlet_inkl_am_efter_personfradrag_kroner"]
            .as_i64()
            .expect("ordinary final tax")
            - hydrated_dis_result["result"]["sømandsbeskatning"]["samlet_lempelse_kroner"]
                .as_i64()
                .expect("DIS relief")
    );
    for (case_id, expected_opening_deficit) in [
        ("personskat-underskud-ekstern-2026", 40_000),
        ("personskat-underskud-årsresultat-2026", 30_000),
    ] {
        let xlsx_deficit_result = result["results"]
            .as_array()
            .expect("XLSX Personskat results")
            .iter()
            .find(|case| case["case_id"] == case_id)
            .unwrap_or_else(|| panic!("missing XLSX deficit result {case_id}"));
        let json_deficit_result = json_result["results"]
            .as_array()
            .expect("JSON Personskat results")
            .iter()
            .find(|case| case["case_id"] == case_id)
            .unwrap_or_else(|| panic!("missing JSON deficit result {case_id}"));
        let hydrated_deficit_result = hydrated_xlsx_result["results"]
            .as_array()
            .expect("hydrated XLSX Personskat results")
            .iter()
            .find(|case| case["case_id"] == case_id)
            .unwrap_or_else(|| panic!("missing hydrated XLSX deficit result {case_id}"));
        assert_eq!(xlsx_deficit_result["result"], json_deficit_result["result"]);
        assert_eq!(
            hydrated_deficit_result["result"],
            json_deficit_result["result"]
        );
        let annual_result = &xlsx_deficit_result["result"]["underskudsår"]["hovedperson"];
        assert_eq!(annual_result["åbningsgrundlag_gyldigt"], true);
        assert_eq!(
            annual_result["fremført_underskud_primo_kroner"],
            expected_opening_deficit
        );
        assert_eq!(
            annual_result["fremført_underskud_anvendt_i_egen_indkomst_kroner"],
            expected_opening_deficit
        );
        assert_eq!(annual_result["fremført_underskud_ultimo_kroner"], 0);
    }
    let json_negative_share_tax_carry = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-negativ-aktieskat-fremførsel-2026")
        .expect("JSON negative share-tax carry result");
    let hydrated_negative_share_tax_carry = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-negativ-aktieskat-fremførsel-2026")
        .expect("hydrated XLSX negative share-tax carry result");
    assert_eq!(
        hydrated_negative_share_tax_carry["result"],
        json_negative_share_tax_carry["result"]
    );
    let carry_result = &json_negative_share_tax_carry["result"];
    assert_eq!(
        carry_result["negativ_aktieskat_fremførselsår"]["alle_åbningsgrundlag_gyldige"],
        true
    );
    assert_eq!(
        carry_result["negativ_aktieskat_fremførselsår"]["hovedperson"]
            ["fremført_negativ_skat_primo"][0]["oprindelsesår"],
        2024
    );
    assert_eq!(
        carry_result["negativ_aktieskat_fremførselsår"]["tidligere_fremført_negativ_skat"]
            ["egen_slutskat_modregnet_kroner"],
        1_000
    );
    assert_eq!(
        carry_result["negativ_aktieskat_fremførselsår"]["hovedperson"]
            ["fremført_negativ_skat_ultimo"],
        serde_json::json!([])
    );
    assert_eq!(
        carry_result["samlet_skat_inkl_ejendomsskatter_efter_negativ_aktieindkomstskat_kroner"]
            .as_i64()
            .expect("tax before prior negative share-tax carry")
            - carry_result["samlet_skat_inkl_ejendomsskatter_efter_fremført_negativ_aktieindkomstskat_kroner"]
                .as_i64()
                .expect("tax after prior negative share-tax carry"),
        1_000
    );
    let json_par32_mixed_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-par32-blandet-fordeling-2026")
        .expect("JSON mixed-class KGL §32 result");
    let hydrated_par32_mixed_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-kgl-par32-blandet-fordeling-2026")
        .expect("hydrated XLSX mixed-class KGL §32 result");
    assert_eq!(
        hydrated_par32_mixed_result["result"],
        json_par32_mixed_result["result"]
    );
    let par32_mixed_result = &json_par32_mixed_result["result"];
    let par32_mixed_trace = &par32_mixed_result["kursgevinst_par32"];
    assert_eq!(par32_mixed_trace["input_gyldigt"], true);
    let par32_mixed_distribution =
        &par32_mixed_trace["aktuelt_årsresultat"]["venstre_aktiemodregningsfordeling"];
    assert_eq!(par32_mixed_distribution["input_gyldigt"], true);
    assert_eq!(par32_mixed_distribution["modregning_i_alt_kroner"], 25_000);
    assert_eq!(par32_mixed_distribution["fordelt_i_alt_kroner"], 25_000);
    let par32_mixed_applications = par32_mixed_distribution["anvendelser"]
        .as_array()
        .expect("mixed-class KGL §32 applications");
    assert_eq!(par32_mixed_applications.len(), 2);
    assert_eq!(
        par32_mixed_applications[0]["kilde"]["mål"]["kildeidentifikation"],
        "par32-json-abl19c"
    );
    assert_eq!(
        par32_mixed_applications[0]["kilde"]["personskattelov_kategori"]["$variant"],
        "AblKapitalindkomstEfterPslPar4Nr5"
    );
    assert_eq!(par32_mixed_applications[0]["modregnet_kroner"], 12_000);
    assert_eq!(
        par32_mixed_applications[1]["kilde"]["mål"]["kildeidentifikation"],
        "par32-json-abl19b"
    );
    assert_eq!(
        par32_mixed_applications[1]["kilde"]["personskattelov_kategori"]["$variant"],
        "AblAktieindkomstEfterPslPar4a"
    );
    assert_eq!(par32_mixed_applications[1]["modregnet_kroner"], 13_000);
    assert_eq!(
        par32_mixed_result["aktieavance"]["aktieindkomst_kroner"],
        7_000
    );
    assert_eq!(
        par32_mixed_result["kapitalindkomst"]["kapitalindkomst_resultat"]
            ["nettokapitalindkomst_kroner"],
        0
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
    let par32_historical_year = &par32_history_trace["historiske_årsresultater"][0];
    assert_eq!(
        par32_historical_year["venstre_årsgrundlag"]["input_gyldigt"],
        true
    );
    assert_eq!(
        par32_historical_year["venstre_årsgrundlag"]["kursgevinst_resultat"]
            ["årets_samlede_netto_efter_par14_kroner"],
        2_500
    );
    assert_eq!(
        par32_historical_year["venstre_årsgrundlag"]["kursgevinst_resultat"]["gældsresultater"]
            .as_array()
            .expect("historical KGL §32 debt results")
            .len(),
        1
    );
    let par32_historical_abl22 = &par32_historical_year["venstre_årsgrundlag"]
        ["kursgevinst_resultat"]["øvrige_instrumentresultat"]["obligationsbevisresultater"][0];
    assert_eq!(par32_historical_abl22["input_gyldigt"], true);
    assert_eq!(
        par32_historical_abl22["aktieavancebeskatningslov_resultat"]
            ["netto_efter_aktieavancebeskatningsloven_kroner"],
        1_500
    );
    assert_eq!(
        par32_historical_year["kursgevinst"]["venstre"]
            ["tab_modregnet_i_egne_aktiegevinster_kroner"],
        1_500
    );
    assert_eq!(
        par32_historical_year["kursgevinst"]["venstre"]
            ["aktiebaserede_tab_fremført_til_følgende_indkomstår_kroner"],
        8_500
    );
    let par32_history_current =
        &par32_history_trace["aktuelt_årsresultat"]["kursgevinst"]["venstre"];
    assert_eq!(
        par32_history_current["gyldigt_fremførte_aktiebaserede_tab_kroner"],
        8_500
    );
    assert_eq!(
        par32_history_current["tab_modregnet_i_egne_indkomstårsgevinster_kroner"],
        6_000
    );
    assert_eq!(
        par32_history_current["aktiebaserede_tab_fremført_til_følgende_indkomstår_kroner"],
        2_500
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
    assert_eq!(
        xlsx_par37_result["result"]["aktieindkomst_parår"]["egen_skat_kroner"],
        30_090
    );
    assert_eq!(
        xlsx_par37_result["result"]["negativ_aktieindkomstskat"]["samlet_negativ_skat_kroner"],
        0
    );
    assert_eq!(
        xlsx_par37_result["result"]["samlet_skat_efter_negativ_aktieindkomstskat_kroner"],
        xlsx_par37_result["result"]["samlet_skat_inkl_endelig_aktieindkomstskat_kroner"]
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
    let json_par37_simultaneous_spouse_result = json_result["results"]
        .as_array()
        .expect("JSON Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-par37-40-samtidige-aegtefaeller-2026")
        .expect("JSON simultaneous-spouse §§ 37-40 result");
    let hydrated_par37_simultaneous_spouse_result = hydrated_xlsx_result["results"]
        .as_array()
        .expect("hydrated XLSX Personskat results")
        .iter()
        .find(|case| case["case_id"] == "personskat-par37-40-samtidige-aegtefaeller-2026")
        .expect("hydrated XLSX simultaneous-spouse §§ 37-40 result");
    assert_eq!(
        hydrated_par37_simultaneous_spouse_result["result"],
        json_par37_simultaneous_spouse_result["result"]
    );
    let simultaneous_result = &json_par37_simultaneous_spouse_result["result"];
    assert_eq!(
        simultaneous_spouse_case["input"]["ægtefælle"]["samlevende_ved_indkomstårets_udløb"],
        false
    );
    let simultaneous_main_special = &simultaneous_result["aktieavance"]["særlige_resultater"][0];
    let simultaneous_spouse_special =
        &simultaneous_result["ægtefælle"]["grundlag"]["aktieavance"]["særlige_resultater"][0];
    assert_eq!(
        simultaneous_result["aktieavance"]["aktieindkomst_kroner"],
        0
    );
    assert_eq!(
        simultaneous_result["ægtefælle"]["grundlag"]["aktieavance"]["aktieindkomst_kroner"],
        70_000
    );
    assert_eq!(
        simultaneous_result["aktieindkomst_parår"]["input"]["egen_aktieindkomst_kroner"],
        0
    );
    assert_eq!(
        simultaneous_result["aktieindkomst_parår"]["input"]["ægtefælles_aktieindkomst_kroner"],
        70_000
    );
    assert_eq!(
        simultaneous_result["aktieindkomst_parår"]
            ["egen_aktieindkomst_efter_ægtefællemodregning_kroner"],
        0
    );
    assert_eq!(
        simultaneous_result["aktieindkomst_parår"]
            ["ægtefælles_aktieindkomst_efter_ægtefællemodregning_kroner"],
        70_000
    );
    assert_eq!(
        simultaneous_result["negativ_aktieindkomstskat"]["samlet_negativ_skat_kroner"],
        0
    );
    assert_eq!(
        simultaneous_result["negativ_aktieindkomstskat"]["samlet_negativ_skat_fremført_kroner"],
        0
    );
    assert_eq!(
        simultaneous_result["samlet_skat_efter_negativ_aktieindkomstskat_kroner"],
        simultaneous_result["samlet_skat_inkl_endelig_aktieindkomstskat_kroner"]
    );
    assert_eq!(
        simultaneous_main_special["kilderesultater"][0]["resultat"]
            ["netto_efter_aktieavancebeskatningsloven_kroner"],
        -30_000
    );
    assert_eq!(
        simultaneous_main_special["kilderesultater"][0]["resultat"]["personskattelov_kategori"]
            ["$variant"],
        "AblIkkeMedregnetISlice"
    );
    assert_eq!(
        simultaneous_spouse_special["kilderesultater"][0]["resultat"]["bruttogevinst_kroner"],
        100_000
    );
    assert_eq!(
        simultaneous_spouse_special["kilderesultater"][0]["resultat"]
            ["skattepligtig_gevinst_kroner"],
        70_000
    );
    assert_eq!(
        simultaneous_main_special["par37_til40_forløbsresultat"]
            ["årets_beregnede_fraflytterskat_kroner"],
        0
    );
    assert_eq!(
        simultaneous_spouse_special["par37_til40_forløbsresultat"]
            ["årets_beregnede_fraflytterskat_kroner"],
        18_900
    );
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
    let xlsx_results = result["results"].as_array().expect("XLSX results");
    let json_results = json_result["results"].as_array().expect("JSON results");
    for xlsx_case in &xlsx_results[1..=9] {
        let case_id = xlsx_case["case_id"].as_str().expect("XLSX case id");
        let json_case = json_results
            .iter()
            .find(|case| case["case_id"] == case_id)
            .unwrap_or_else(|| panic!("missing JSON result for {case_id}"));
        assert_eq!(
            xlsx_case["result"], json_case["result"],
            "JSON/XLSX mismatch for {case_id}"
        );
    }

    assert_eq!(
        result["results"][0]["result"]["skat"]["samlet_inkl_am_efter_personfradrag_kroner"],
        208_726
    );
    assert_eq!(
        result["results"][0]["result"]["årsopgørelse"]["$variant"],
        "BeregnetÅrsopgørelse"
    );
    assert_eq!(
        result["results"][0]["result"]["årsopgørelse"]["resultat"]["slutskat_med_tillæg_øre"],
        21_022_564
    );
    assert_eq!(
        result["results"][0]["result"]["årsopgørelse"]["resultat"]["restskat_øre"],
        21_022_564
    );
    assert_eq!(
        result["results"][0]["result"]["årsopgørelse"]["afregning"]["$variant"],
        "IngenSlutopgørelsesafregning"
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
        result["results"][2]["result"]["aktieindkomst_parår"]["egen_skat_kroner"],
        -4_860
    );
    assert_eq!(
        result["results"][2]["result"]["negativ_aktieindkomstskat"]["egen"]["negativ_skat_kroner"],
        4_860
    );
    assert_eq!(
        result["results"][2]["result"]["negativ_aktieindkomstskat"]["egen"]
            ["modregnet_i_egen_slutskat_kroner"],
        4_860
    );
    assert_eq!(
        result["results"][2]["result"]["negativ_aktieindkomstskat"]["egen"]["fremført_kroner"],
        0
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
        result["results"][2]["result"]["ligningsfradrag"]["befordring"]["forholdsresultater"][0]
            ["$variant"],
        "BeregnetBefordringsfradragsforhold"
    );
    assert_eq!(
        result["results"][2]["result"]["ligningsfradrag"]["befordring"]["forholdsresultater"][0]
            ["ligningslov9c_input"]["aftrapningsindkomst_kroner"],
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

fn round_trip_source_inputs_through_generated_personskat_workbook(
    source_scenario: &Path,
    source_entry: &str,
    source_cases: &[(&str, &str)],
) -> (Value, Value) {
    round_trip_source_inputs_through_generated_personskat_workbook_with_inspection(
        source_scenario,
        source_entry,
        source_cases,
        |_| {},
    )
}

fn round_trip_source_inputs_through_generated_personskat_workbook_with_inspection(
    source_scenario: &Path,
    source_entry: &str,
    source_cases: &[(&str, &str)],
    inspect_workbook: impl FnOnce(&Path),
) -> (Value, Value) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture = root.join("examples/danish-income-tax/personskat.calculate.runa");
    let source_input_path = temp_path("json");
    let public_template_path = temp_path("json");
    let public_input_path = temp_path("json");
    let workbook_path = temp_path("xlsx");

    let source_template = run(&[
        "template",
        source_scenario.to_str().expect("source scenario path"),
        "--entry",
        source_entry,
        "--format",
        "json",
        "--output",
        source_input_path.to_str().expect("source input path"),
    ]);
    assert!(
        source_template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&source_template.stderr)
    );
    let mut source_input: Value = serde_json::from_slice(
        &std::fs::read(&source_input_path).expect("read source JSON template"),
    )
    .expect("source JSON template");
    let source_case = source_input["cases"][0].clone();
    source_input["cases"] = Value::Array(
        source_cases
            .iter()
            .map(|(case_id, variant)| {
                let mut case = source_case.clone();
                case["case_id"] = Value::String((*case_id).to_string());
                case["input"]["$variant"] = Value::String((*variant).to_string());
                case
            })
            .collect(),
    );
    std::fs::write(
        &source_input_path,
        serde_json::to_vec_pretty(&source_input).expect("encode source calculation input"),
    )
    .expect("write source calculation input");

    let source_call = run(&[
        "call",
        source_scenario.to_str().expect("source scenario path"),
        "--entry",
        source_entry,
        "--input",
        source_input_path.to_str().expect("source input path"),
    ]);
    assert!(
        source_call.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&source_call.stderr),
        String::from_utf8_lossy(&source_call.stdout)
    );
    let source_output = parse_stdout(&source_call);
    assert_eq!(source_output["diagnostics"], serde_json::json!([]));

    let public_template = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--format",
        "json",
        "--output",
        public_template_path.to_str().expect("public template path"),
    ]);
    assert!(
        public_template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&public_template.stderr)
    );
    let mut public_input: Value = serde_json::from_slice(
        &std::fs::read(&public_template_path).expect("read Personskat JSON template"),
    )
    .expect("Personskat JSON template");
    public_input["cases"] = Value::Array(
        source_output["results"]
            .as_array()
            .expect("source calculation results")
            .iter()
            .map(|result| {
                serde_json::json!({
                    "case_id": result["case_id"].as_str().expect("source case id"),
                    "input": result["result"].clone()
                })
            })
            .collect(),
    );
    std::fs::write(
        &public_input_path,
        serde_json::to_vec_pretty(&public_input).expect("encode canonical Personskat input"),
    )
    .expect("write canonical Personskat input");

    let direct_call = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        public_input_path.to_str().expect("public input path"),
    ]);
    assert!(
        direct_call.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&direct_call.stderr),
        String::from_utf8_lossy(&direct_call.stdout)
    );
    let hydrate = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--input",
        public_input_path.to_str().expect("public input path"),
        "--format",
        "xlsx",
        "--output",
        workbook_path.to_str().expect("workbook path"),
    ]);
    assert!(
        hydrate.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&hydrate.stderr),
        String::from_utf8_lossy(&hydrate.stdout)
    );
    inspect_workbook(&workbook_path);
    let workbook_call = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        workbook_path.to_str().expect("workbook path"),
    ]);

    std::fs::remove_file(&source_input_path).ok();
    std::fs::remove_file(&public_template_path).ok();
    std::fs::remove_file(&public_input_path).ok();
    std::fs::remove_file(&workbook_path).ok();

    assert!(
        workbook_call.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&workbook_call.stderr),
        String::from_utf8_lossy(&workbook_call.stdout)
    );
    let direct_output = parse_stdout(&direct_call);
    let workbook_output = parse_stdout(&workbook_call);
    assert_eq!(direct_output["diagnostics"], serde_json::json!([]));
    assert_eq!(workbook_output["diagnostics"], serde_json::json!([]));
    assert_eq!(workbook_output["results"], direct_output["results"]);

    (source_output, workbook_output)
}

#[test]
fn anonymized_2025_source_facts_round_trip_through_generated_personskat_workbook() {
    let source_scenario = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/danish-income-tax/personskat-2025-aarsopgoerelse.scenario.runa");
    let (source_output, workbook_output) =
        round_trip_source_inputs_through_generated_personskat_workbook(
            &source_scenario,
            "årsopgørelse2025_kanoniske_kildefakta_til_arbejdsbog",
            &[(
                "personskat-anonymiseret-aarsopgoerelse-2025",
                "AnonymiseretÅrsopgørelse2025",
            )],
        );

    let canonical_input = &source_output["results"][0]["result"];
    assert_eq!(canonical_input["lønmodtager"]["skatteår"], 2025);
    assert_eq!(canonical_input["lønmodtager"]["bruttoløn_kroner"], 817_097);
    assert_eq!(
        canonical_input["årsopgørelse"]["$variant"],
        "MedEksaktÅrsopgørelse"
    );

    let result = &workbook_output["results"][0]["result"];
    assert_eq!(
        result["skat"]["personlig_indkomst_efter_am_kroner"],
        749_936
    );
    assert_eq!(result["skat"]["nettokapitalindkomst_kroner"], -36_404);
    assert_eq!(result["aktieavance"]["aktieindkomst_kroner"], 14_967);
    assert_eq!(result["slutskat_øre"], 29_059_034);
    assert_eq!(
        result["årsopgørelse"]["resultat"]["overskydende_skat_øre"],
        8_280
    );
    assert_eq!(
        result["årsopgørelse"]["afregning"]["$variant"],
        "OverskydendeSkatAfregnet"
    );
    assert_eq!(
        result["årsopgørelse"]["afregning"]["resultat"]["udbetales_kroner"],
        82
    );
    assert_eq!(
        result["årsopgørelse"]["afregning"]["resultat"]["ikke_udbetalt_øre"],
        80
    );
}

#[test]
fn anonymized_2023_and_2024_source_facts_round_trip_through_generated_personskat_workbook() {
    let source_scenario = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/danish-income-tax/personskat-2023-2024-aarsopgoerelser.scenario.runa");
    let (_, workbook_output) = round_trip_source_inputs_through_generated_personskat_workbook(
        &source_scenario,
        "historiske_kanoniske_kildefakta_til_arbejdsbog",
        &[
            (
                "personskat-anonymiseret-aarsopgoerelse-2023",
                "AnonymiseretÅrsopgørelse2023",
            ),
            (
                "personskat-anonymiseret-aarsopgoerelse-2024",
                "AnonymiseretÅrsopgørelse2024",
            ),
        ],
    );
    let results = workbook_output["results"]
        .as_array()
        .expect("historical Personskat workbook results");
    let result_2023 = &results
        .iter()
        .find(|case| case["case_id"] == "personskat-anonymiseret-aarsopgoerelse-2023")
        .expect("2023 Personskat workbook result")["result"];
    let result_2024 = &results
        .iter()
        .find(|case| case["case_id"] == "personskat-anonymiseret-aarsopgoerelse-2024")
        .expect("2024 Personskat workbook result")["result"];

    for (result, expected) in [
        (
            result_2023,
            (2023, 767_928, -26_159, 858, 60_108, 22_735, 658_926, 66_732),
        ),
        (
            result_2024,
            (
                2024, 914_168, -23_253, 1_385, 63_772, 20_301, 806_842, 79_740,
            ),
        ),
    ] {
        let (year, personal, capital, shares, deductions, spouse, taxable, am) = expected;
        assert_eq!(result["skat"]["skatteår"], year);
        assert_eq!(
            result["skat"]["personlig_indkomst_efter_am_kroner"],
            personal
        );
        assert_eq!(result["skat"]["nettokapitalindkomst_kroner"], capital);
        assert_eq!(result["aktieavance"]["aktieindkomst_kroner"], shares);
        assert_eq!(
            result["skat"]["samlede_ligningsmæssige_fradrag_kroner"],
            deductions
        );
        assert_eq!(
            result["indgående_ægtefælle"]["par13_indkomstfradrag_kroner"],
            spouse
        );
        assert_eq!(
            result["skat"]["almindelig_skattepligtig_indkomst_kroner"],
            taxable
        );
        assert_eq!(result["skat"]["arbejdsmarkedsbidrag_kroner"], am);
    }

    assert_eq!(result_2023["slutskat_øre"], 30_605_958);
    assert_eq!(
        result_2023["årsopgørelse"]["resultat"]["overskydende_skat_øre"],
        3_350_634
    );
    assert_eq!(
        result_2023["årsopgørelse"]["afregning"]["resultat"]["godtgørelse_øre"],
        26_805
    );
    assert_eq!(
        result_2023["årsopgørelse"]["afregning"]["resultat"]["udbetales_kroner"],
        33_774
    );
    assert_eq!(
        result_2023["årsopgørelse"]["afregning"]["resultat"]["ikke_udbetalt_øre"],
        39
    );

    assert_eq!(result_2024["slutskat_øre"], 39_709_195);
    assert_eq!(
        result_2024["årsopgørelse"]["resultat"]["overskydende_skat_øre"],
        292_459
    );
    assert_eq!(
        result_2024["årsopgørelse"]["afregning"]["resultat"]["godtgørelse_øre"],
        1_754
    );
    assert_eq!(
        result_2024["årsopgørelse"]["afregning"]["resultat"]["udbetales_kroner"],
        2_942
    );
    assert_eq!(
        result_2024["årsopgørelse"]["afregning"]["resultat"]["ikke_udbetalt_øre"],
        13
    );
}

#[test]
fn personskat_support_payment_round_trips_through_generated_workbook_with_danish_labels() {
    let source_scenario = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/danish-income-tax/personskat-underholdsbidrag.scenario.runa");
    let (source_output, workbook_output) =
        round_trip_source_inputs_through_generated_personskat_workbook_with_inspection(
            &source_scenario,
            "personskat_underhold_kildefakta_til_arbejdsbog",
            &[
                (
                    "personskat-underhold-ægtefællebidrag-2025",
                    "ÆgtefællebidragBetalt2025",
                ),
                (
                    "personskat-underhold-privat-begrænset-børnebidrag-2025",
                    "PrivatBegrænsetBørnebidragBetalt2025",
                ),
                (
                    "personskat-underhold-barn-fylder-18-modtager-2025",
                    "BarnFylder18ModtagerHeleMånedsbeløbet2025",
                ),
            ],
            |workbook_path| {
                let mut workbook =
                    open_workbook_auto(workbook_path).expect("Personskat support workbook");
                let sheet = workbook_collection_sheet_name(
                    &mut workbook,
                    "lønmodtager.personlig_indkomst.underholdsbidrag.bidrag",
                );
                let paths = workbook_column_paths(&mut workbook, &sheet);
                let headers = workbook_headers(&mut workbook, &sheet);
                for (path, label) in [
                    ("identifikation", "Bidragets identifikation"),
                    ("rolle.$variant", "Personens rolle i bidraget"),
                    (
                        "modtager_identifikation",
                        "Bidragsmodtagerens identifikation",
                    ),
                    (
                        "fastsættelse.DokumenteretPrivatAftale.fradragsvurdering.$variant",
                        "Privataftalens fradragsvurdering",
                    ),
                    (
                        "bidragsart.LøbendeUnderholdsbidrag.beløbsgrundlag",
                        "Det løbende bidrags beløbsgrundlag",
                    ),
                    ("beløb_kroner", "Betalt eller modtaget bidrag"),
                ] {
                    let column = paths
                        .iter()
                        .position(|candidate| candidate == path)
                        .unwrap_or_else(|| panic!("missing support-payment path {path}"));
                    assert_eq!(headers[column + 3], label);
                }
            },
        );

    let canonical_input = &source_output["results"][0]["result"];
    let contribution =
        &canonical_input["lønmodtager"]["personlig_indkomst"]["underholdsbidrag"]["bidrag"][0];
    assert_eq!(contribution["identifikation"], "ægtefællebidrag-2025");
    assert_eq!(contribution["beløb_kroner"], 24_000);
    assert_eq!(
        contribution["fastsættelse"]["fradragsvurdering"]["$variant"],
        "IkkeRelevantForFradraget"
    );
    assert_eq!(
        contribution["bidragsart"]["beløbsgrundlag"]["$variant"],
        "HeleForfaldsmånedensBidrag"
    );

    let result = &workbook_output["results"][0]["result"];
    assert_eq!(
        result["personlig_indkomst"]["underholdsbidrag"]["samlet_ligningsmæssigt_fradrag_kroner"],
        24_000
    );
    assert!(
        result["personlig_indkomst"]["underholdsbidrag"]["kan_sammensættes"]
            .as_bool()
            .expect("support-payment composability")
    );

    let limited_source = &source_output["results"][1]["result"]["lønmodtager"]
        ["personlig_indkomst"]["underholdsbidrag"]["bidrag"][0];
    assert_eq!(limited_source["beløb_kroner"], 5_000);
    assert_eq!(
        limited_source["fastsættelse"]["fradragsvurdering"]["anerkendt_betalt_beløb_kroner"],
        2_000
    );

    let limited_result = &workbook_output["results"][1]["result"];
    assert_eq!(
        limited_result["personlig_indkomst"]["underholdsbidrag"]["resultater"][0]
            ["fradragsberettiget_bruttobidrag_kroner"],
        2_000
    );
    assert_eq!(
        limited_result["personlig_indkomst"]["underholdsbidrag"]
            ["samlet_ligningsmæssigt_fradrag_kroner"],
        1_816
    );

    let birthday_source = &source_output["results"][2]["result"]["lønmodtager"]
        ["personlig_indkomst"]["underholdsbidrag"]["bidrag"][0];
    assert_eq!(birthday_source["rolle"]["$variant"], "Bidragsmodtager");
    assert_eq!(birthday_source["beløb_kroner"], 1_603);
    assert_eq!(
        birthday_source["bidragsart"]["beløbsgrundlag"]["$variant"],
        "HeleForfaldsmånedensBidrag"
    );

    let birthday_result = &workbook_output["results"][2]["result"];
    assert_eq!(
        birthday_result["personlig_indkomst"]["underholdsbidrag"]["resultater"][0]
            ["dage_før_og_med_18_år_i_måneden"],
        5
    );
    assert_eq!(
        birthday_result["personlig_indkomst"]["underholdsbidrag"]
            ["samlet_personlig_indkomst_kroner"],
        1_335
    );
}

#[test]
fn personskat_par19_contributions_round_trip_through_generated_workbook() {
    let source_scenario = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/danish-income-tax/personskat-par19-bidrag.scenario.runa");
    let (source_output, workbook_output) =
        round_trip_source_inputs_through_generated_personskat_workbook_with_inspection(
            &source_scenario,
            "personskat_par19_kildefakta_til_arbejdsbog",
            &[
                (
                    "personskat-par19-foedselsbidrag-2025",
                    "FødselsbidragBetalt2025",
                ),
                (
                    "personskat-par19-navngivningsbidrag-2025",
                    "NavngivningsbidragBetalt2025",
                ),
                (
                    "personskat-par19-foedselsbidrag-for-sent-2025",
                    "FødselsbidragForSentAnsøgt2025",
                ),
            ],
            |workbook_path| {
                let mut workbook =
                    open_workbook_auto(workbook_path).expect("Personskat § 19 workbook");
                let sheet = workbook_collection_sheet_name(
                    &mut workbook,
                    "lønmodtager.personlig_indkomst.børnebidragslov19_bidrag.bidrag",
                );
                let paths = workbook_column_paths(&mut workbook, &sheet);
                let headers = workbook_headers(&mut workbook, &sheet);
                for (path, label) in [
                    ("identifikation", "§ 19-bidragets identifikation"),
                    ("rolle.$variant", "Personens skattemæssige rolle i § 19-bidraget"),
                    ("bidragsart.$variant", "§ 19-bidragets art"),
                    (
                        "retlig_modtager.$variant",
                        "§ 19-bidragets retlige modtager",
                    ),
                    (
                        "ansøgningsgrundlag.$variant",
                        "§ 19-bidragets ansøgningsvej",
                    ),
                    (
                        "ansøgningsgrundlag.Myndighedsansøgning.fristgrundlag.$variant",
                        "§ 19-ansøgningens fristgrundlag",
                    ),
                    ("beløb_kroner", "Betalt eller modtaget § 19-bidrag"),
                    (
                        "navngivningsskattekontekst.MedNavngivningsskattekontekst.retsgrundlag.$variant",
                        "Navngivningsbidragets familiemæssige skattegrundlag",
                    ),
                ] {
                    let column = paths
                        .iter()
                        .position(|candidate| candidate == path)
                        .unwrap_or_else(|| panic!("missing § 19 contribution path {path}"));
                    assert_eq!(headers[column + 3], label);
                }
            },
        );

    let birth_source = &source_output["results"][0]["result"]["lønmodtager"]["personlig_indkomst"]
        ["børnebidragslov19_bidrag"]["bidrag"][0];
    assert_eq!(
        birth_source["identifikation"],
        "personskat-fødselsbidrag-2025"
    );
    assert_eq!(birth_source["bidragsart"]["$variant"], "Fødselsbidrag");
    assert_eq!(birth_source["beløb_kroner"], 976);

    let naming_source = &source_output["results"][1]["result"]["lønmodtager"]["personlig_indkomst"]
        ["børnebidragslov19_bidrag"]["bidrag"][0];
    assert_eq!(
        naming_source["bidragsart"]["$variant"],
        "NavngivningsbidragHerunderDåb"
    );
    assert_eq!(naming_source["beløb_kroner"], 1_419);
    assert_eq!(
        naming_source["navngivningsskattekontekst"]["$variant"],
        "MedNavngivningsskattekontekst"
    );

    let birth_result =
        &workbook_output["results"][0]["result"]["personlig_indkomst"]["børnebidragslov19_bidrag"];
    assert_eq!(birth_result["resultater"][0]["officiel_sats_kroner"], 976);
    assert_eq!(birth_result["samlet_ligningsmæssigt_fradrag_kroner"], 0);
    assert!(birth_result["kan_sammensættes"].as_bool().unwrap());

    let naming_result =
        &workbook_output["results"][1]["result"]["personlig_indkomst"]["børnebidragslov19_bidrag"];
    assert_eq!(
        naming_result["resultater"][0]["officiel_sats_kroner"],
        1_419
    );
    assert_eq!(
        naming_result["resultater"][0]["ligningsmæssigt_fradrag_kroner"],
        1_419
    );
    assert_eq!(
        naming_result["samlet_ligningsmæssigt_fradrag_kroner"],
        1_419
    );
    assert!(naming_result["kan_sammensættes"].as_bool().unwrap());

    let late_source = &source_output["results"][2]["result"]["lønmodtager"]["personlig_indkomst"]
        ["børnebidragslov19_bidrag"]["bidrag"][0];
    assert_eq!(
        late_source["ansøgningsgrundlag"]["$variant"],
        "Myndighedsansøgning"
    );
    assert_eq!(
        late_source["ansøgningsgrundlag"]["fristgrundlag"]["$variant"],
        "FødslenSomFristgrundlag"
    );
    assert_eq!(
        late_source["ansøgningsgrundlag"]["ansøgningsdato"]["måned"],
        3
    );
    assert_eq!(
        late_source["ansøgningsgrundlag"]["ansøgningsdato"]["dag"],
        11
    );

    let late_result =
        &workbook_output["results"][2]["result"]["personlig_indkomst"]["børnebidragslov19_bidrag"];
    assert!(!late_result["resultater"][0]["ansøgningsgrundlag_gyldigt"]
        .as_bool()
        .unwrap());
    assert!(!late_result["alle_input_gyldige"].as_bool().unwrap());
    assert!(!late_result["kan_sammensættes"].as_bool().unwrap());
}

#[test]
fn personskat_ksl25a_shared_annual_cap_round_trips_through_generated_workbook() {
    let source_scenario = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/danish-income-tax/personskat-kildeskat25a.scenario.runa");
    let (source_output, workbook_output) =
        round_trip_source_inputs_through_generated_personskat_workbook_with_inspection(
            &source_scenario,
            "personskat_ksl25a_kildefakta_til_arbejdsbog",
            &[(
                "personskat-ksl25a-stk3-over-faelles-aarsloft-2025",
                "Stk3OverFællesÅrsloft2025",
            )],
            |workbook_path| {
                let mut workbook =
                    open_workbook_auto(workbook_path).expect("Personskat KSL § 25 A workbook");
                let sheet = workbook_collection_sheet_name(
                    &mut workbook,
                    "ægtefælle.MedÆgtefælle.kildeskat25a_fordelinger",
                );
                let paths = workbook_column_paths(&mut workbook, &sheet);
                let headers = workbook_headers(&mut workbook, &sheet);
                for (path, label) in [
                    ("virksomhedsidentifikation", "Virksomhedens identifikation"),
                    (
                        "fordeling.$variant",
                        "Regel for ægtefællernes virksomhedsindkomst",
                    ),
                    (
                        "fordeling.Ksl25AStk3TilMedarbejdendeÆgtefælle.anmodet_overførsel_kroner",
                        "Anmodet overførsel til medarbejdende ægtefælle",
                    ),
                    (
                        "fordeling.Ksl25AStk3TilMedarbejdendeÆgtefælle.arbejdsindsats_forsvarligt_maksimum_kroner",
                        "Forsvarligt maksimum efter arbejdsindsatsen",
                    ),
                ] {
                    let column = paths
                        .iter()
                        .position(|candidate| candidate == path)
                        .unwrap_or_else(|| panic!("missing KSL § 25 A allocation path {path}"));
                    assert_eq!(headers[column + 3], label);
                }
            },
        );

    let allocations = source_output["results"][0]["result"]["ægtefælle"]
        ["kildeskat25a_fordelinger"]
        .as_array()
        .expect("canonical KSL § 25 A allocations");
    assert_eq!(allocations.len(), 2);
    for allocation in allocations {
        assert_eq!(
            allocation["fordeling"]["anmodet_overførsel_kroner"],
            160_000
        );
    }

    let result = &workbook_output["results"][0]["result"];
    let kildeskat25a = &result["kildeskat25a"];
    assert_eq!(kildeskat25a["stk3_samlet_overført_kroner"], 320_000);
    assert_eq!(kildeskat25a["stk3_reguleret_årsloft_kroner"], 282_400);
    assert!(kildeskat25a["fælles_virksomheder_bruger_samme_årsregel"]
        .as_bool()
        .unwrap());
    assert!(kildeskat25a["stk3_samme_virksomhedsdriver"]
        .as_bool()
        .unwrap());
    assert!(!kildeskat25a["stk3_årsloft_overholdt"].as_bool().unwrap());
    assert!(!kildeskat25a["alle_input_gyldige"].as_bool().unwrap());
    assert_eq!(
        kildeskat25a["hovedperson"]["arbejdsmarkedsbidragsgrundlag_regulering_kroner"],
        0
    );
    assert_eq!(
        kildeskat25a["ægtefælle"]["arbejdsmarkedsbidragsgrundlag_regulering_kroner"],
        0
    );
    assert!(!result["personlig_indkomst"]["alle_input_gyldige"]
        .as_bool()
        .unwrap());
}

#[test]
fn ligningslov9a_xlsx_round_trips_split_food_and_nested_lodging_days() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/danish-income-tax/ligningsloven-par9a-rejser.calculate.runa");
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
        let mut workbook = open_workbook_auto(&input_path).expect("travel workbook");
        let foreign_income_sheet =
            workbook_collection_sheet_name(&mut workbook, "årsinput.udenlandske_indkomstkilder");
        assert_eq!(
            workbook_column_paths(&mut workbook, &foreign_income_sheet),
            [
                "identifikation",
                "lønindkomst_medregnet_i_danmark_kroner",
                "øvrige_fradrag_vedrørende_indkomsten_kroner",
            ]
        );
        assert_eq!(
            workbook_headers(&mut workbook, &foreign_income_sheet),
            [
                "case_id",
                "item_id",
                "position",
                "Den udenlandske indkomstkildes identifikation",
                "Udenlandsk løn medregnet i Danmark",
                "Øvrige fradrag vedrørende den udenlandske løn",
            ]
        );
    }

    edit_workbook(&input_path, |sheets| {
        let travel_sheet = workbook_collection_sheet_name_from_rows(sheets, "årsinput.rejser");
        let lodging_sheet =
            workbook_collection_sheet_name_from_rows(sheets, "årsinput.rejser.logidøgn");

        set_workbook_cell(sheets, "cases", 1, 0, Data::String("ll9a-xlsx".to_string()));
        for (path, value) in [
            ("indkomstår", Data::Int(2026)),
            (
                "årsinput.personrolle",
                Data::String("Ll9AAlmindeligLønmodtager".to_string()),
            ),
            (
                "årsinput.dobbelt_husførelse.$variant",
                Data::String("Ll9AIntetFradragForDobbeltHusførelse".to_string()),
            ),
            (
                "ekstern_udelukkelse",
                Data::String("Ll9AIngenEksternUdelukkelse".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "cases", 1, path, value);
        }

        set_workbook_cell(
            sheets,
            &travel_sheet,
            1,
            0,
            Data::String("ll9a-xlsx".to_string()),
        );
        set_workbook_cell(
            sheets,
            &travel_sheet,
            1,
            1,
            Data::String("rejse-1".to_string()),
        );
        set_workbook_cell(sheets, &travel_sheet, 1, 2, Data::Int(1));
        for (path, value) in [
            ("identifikation", Data::String("xlsx-rejse".to_string())),
            ("indkomstår", Data::Int(2026)),
            ("startdato.år", Data::Int(2026)),
            ("startdato.måned", Data::Int(3)),
            ("startdato.dag", Data::Int(1)),
            ("rejseart", Data::String("Ll9ATjenesterejse".to_string())),
            (
                "arbejdssted_identifikation",
                Data::String("xlsx-projektsted".to_string()),
            ),
            (
                "arbejdsstedskarakter.$variant",
                Data::String("Ll9AStedbundetArbejdssted".to_string()),
            ),
            (
                "arbejdsstedskarakter.Ll9AStedbundetArbejdssted.tidsbegrænsning",
                Data::String("Ll9ATidsbegrænsetTilBestemtPeriode".to_string()),
            ),
            (
                "overnatningsforhold.$variant",
                Data::String(
                    "Ll9AIngenMulighedForOvernatningPåSædvanligBopæl".to_string(),
                ),
            ),
            (
                "overnatningsforhold.Ll9AIngenMulighedForOvernatningPåSædvanligBopæl.afstand_ad_normal_transportvej_kilometer",
                Data::Int(350),
            ),
            (
                "overnatningsforhold.Ll9AIngenMulighedForOvernatningPåSædvanligBopæl.korteste_transporttid_hver_vej_minutter",
                Data::Int(240),
            ),
            ("hverv", Data::String("Ll9AAlmindeligtHverv".to_string())),
            ("varighed_minutter", Data::Int(2880)),
            (
                "kost.dækning.$variant",
                Data::String("Ll9AKostIkkeDækketEfterRegning".to_string()),
            ),
            (
                "kost.godtgørelsesudbetaling.$variant",
                Data::String("Ll9AEndeligtOpdeltGodtgørelse".to_string()),
            ),
            (
                "kost.godtgørelsesudbetaling.Ll9AEndeligtOpdeltGodtgørelse.godtgørelse_efter_sats_kroner",
                Data::Int(1250),
            ),
            (
                "kost.godtgørelsesudbetaling.Ll9AEndeligtOpdeltGodtgørelse.supplerende_løn_kroner",
                Data::Int(200),
            ),
            ("kost.fri_morgenmad_antal", Data::Int(0)),
            ("kost.fri_frokost_antal", Data::Int(0)),
            ("kost.fri_aftensmad_antal", Data::Int(0)),
            (
                "kost.dokumenterede_kostudgifter_før_arbejdsgiverdækning_kroner",
                Data::Int(1600),
            ),
            (
                "kost.fradragsprincip",
                Data::String("Ll9AKostfradragMedStandardsats".to_string()),
            ),
            (
                "kontrol",
                Data::String("Ll9AArbejdsgiverkontrolUdført".to_string()),
            ),
            (
                "lønomlægning",
                Data::String("Ll9AGodtgørelseUdenLønomlægning".to_string()),
            ),
            (
                "indkomstforhold.$variant",
                Data::String("Ll9ADanskSkattepligtigArbejdsindkomst".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &travel_sheet, 1, path, value);
        }

        for (row, item_id, position) in [(1, "logi-1", 1), (2, "logi-2", 2)] {
            set_workbook_cell(
                sheets,
                &lodging_sheet,
                row,
                0,
                Data::String("ll9a-xlsx".to_string()),
            );
            set_workbook_cell(
                sheets,
                &lodging_sheet,
                row,
                1,
                Data::String("rejse-1".to_string()),
            );
            set_workbook_cell(
                sheets,
                &lodging_sheet,
                row,
                2,
                Data::String(item_id.to_string()),
            );
            set_workbook_cell(sheets, &lodging_sheet, row, 3, Data::Int(position));
            set_workbook_cell_by_header(
                sheets,
                &lodging_sheet,
                row,
                "rejsedøgnsnummer",
                Data::Int(position),
            );
        }
        for (path, value) in [
            (
                "dækning.$variant",
                Data::String("Ll9ALogiIkkeDækketAfArbejdsgiver".to_string()),
            ),
            (
                "godtgørelsesudbetaling.$variant",
                Data::String("Ll9AUopdeltGodtgørelse".to_string()),
            ),
            (
                "godtgørelsesudbetaling.Ll9AUopdeltGodtgørelse.udbetalt_kroner",
                Data::Int(150),
            ),
            (
                "dokumenteret_logiudgift_betalt_før_refusion_kroner",
                Data::Int(450),
            ),
            (
                "fradragsprincip",
                Data::String("Ll9ALogifradragMedStandardsats".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &lodging_sheet, 1, path, value);
        }
        for (path, value) in [
            (
                "dækning.$variant",
                Data::String("Ll9ALogiHeltEllerDelvistDækketEfterRegning".to_string()),
            ),
            (
                "dækning.Ll9ALogiHeltEllerDelvistDækketEfterRegning.arbejdsgiverbetalt_kroner",
                Data::Int(400),
            ),
            (
                "godtgørelsesudbetaling.$variant",
                Data::String("Ll9AUopdeltGodtgørelse".to_string()),
            ),
            (
                "godtgørelsesudbetaling.Ll9AUopdeltGodtgørelse.udbetalt_kroner",
                Data::Int(0),
            ),
            (
                "dokumenteret_logiudgift_betalt_før_refusion_kroner",
                Data::Int(700),
            ),
            (
                "fradragsprincip",
                Data::String("Ll9ALogifradragMedDokumenteredeUdgifter".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &lodging_sheet, 2, path, value);
        }
    });

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
    let annual = &result["results"][0]["result"];
    assert_eq!(annual["samlet_udbetalt_godtgørelse_kroner"], 1600);
    assert_eq!(annual["samlet_supplerende_løn_kroner"], 200);
    assert_eq!(annual["samlet_skattefri_godtgørelse_kroner"], 1400);
    assert_eq!(annual["samlet_skattepligtig_godtgørelse_kroner"], 200);
    assert_eq!(annual["samlet_rejsefradrag_efter_årsloft_kroner"], 418);
}

#[test]
fn ligningslov9a_xlsx_round_trips_grouped_foreign_income_ceiling() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/danish-income-tax/ligningsloven-par9a-rejser.calculate.runa");
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

    let foreign_trip = |identifikation: &str, dag: i64, indkomstkilde: &str| {
        serde_json::json!({
            "identifikation": identifikation,
            "indkomstår": 2026,
            "startdato": { "år": 2026, "måned": 3, "dag": dag },
            "rejseart": { "$variant": "Ll9ATjenesterejse" },
            "arbejdssted_identifikation": identifikation,
            "arbejdsstedskarakter": { "$variant": "Ll9AMobiltArbejdssted" },
            "overnatningsforhold": {
                "$variant": "Ll9AIngenMulighedForOvernatningPåSædvanligBopæl",
                "afstand_ad_normal_transportvej_kilometer": 350,
                "korteste_transporttid_hver_vej_minutter": 240
            },
            "hverv": { "$variant": "Ll9AAlmindeligtHverv" },
            "varighed_minutter": 1440,
            "kost": {
                "dækning": { "$variant": "Ll9AKostIkkeDækketEfterRegning" },
                "godtgørelsesudbetaling": {
                    "$variant": "Ll9AUopdeltGodtgørelse",
                    "udbetalt_kroner": 0
                },
                "fri_morgenmad_antal": 0,
                "fri_frokost_antal": 0,
                "fri_aftensmad_antal": 0,
                "dokumenterede_kostudgifter_før_arbejdsgiverdækning_kroner": 0,
                "fradragsprincip": { "$variant": "Ll9AKostfradragMedStandardsats" }
            },
            "logidøgn": [{
                "rejsedøgnsnummer": 1,
                "dækning": { "$variant": "Ll9ALogiIkkeDækketAfArbejdsgiver" },
                "godtgørelsesudbetaling": {
                    "$variant": "Ll9AUopdeltGodtgørelse",
                    "udbetalt_kroner": 0
                },
                "dokumenteret_logiudgift_betalt_før_refusion_kroner": 0,
                "fradragsprincip": { "$variant": "Ll9ALogifradragMedStandardsats" }
            }],
            "kontrol": { "$variant": "Ll9AArbejdsgiverkontrolUdført" },
            "lønomlægning": { "$variant": "Ll9AGodtgørelseUdenLønomlægning" },
            "indkomstforhold": {
                "$variant": "Ll9AUdenlandskSkattepligtigArbejdsindkomst",
                "indkomstkilde_identifikation": indkomstkilde
            }
        })
    };
    let mut input: Value =
        serde_json::from_slice(&std::fs::read(&json_path).expect("JSON template"))
            .expect("travel JSON template");
    input["cases"][0]["case_id"] = Value::String("ll9a-grouped-foreign-income".into());
    input["cases"][0]["input"] = serde_json::json!({
        "indkomstår": 2026,
        "årsinput": {
            "personrolle": { "$variant": "Ll9AAlmindeligLønmodtager" },
            "rejser": [
                foreign_trip("foreign-a-1", 1, "foreign-income-a"),
                foreign_trip("foreign-a-2", 5, "foreign-income-a"),
                foreign_trip("foreign-b-1", 10, "foreign-income-b")
            ],
            "udenlandske_indkomstkilder": [
                {
                    "identifikation": "foreign-income-a",
                    "lønindkomst_medregnet_i_danmark_kroner": 1000,
                    "øvrige_fradrag_vedrørende_indkomsten_kroner": 100
                },
                {
                    "identifikation": "foreign-income-b",
                    "lønindkomst_medregnet_i_danmark_kroner": 600,
                    "øvrige_fradrag_vedrørende_indkomsten_kroner": 100
                }
            ],
            "arbejdshistorik": {
                "tidligere_rejser": [],
                "arbejdsdage": [],
                "arbejdsstedsafstande": []
            },
            "ølogi": { "$variant": "UdenØlogifradrag" },
            "dobbelt_husførelse": { "$variant": "Ll9AIntetFradragForDobbeltHusførelse" }
        },
        "ekstern_udelukkelse": { "$variant": "Ll9AIngenEksternUdelukkelse" }
    });
    std::fs::write(
        &json_path,
        serde_json::to_vec_pretty(&input).expect("encode populated travel input"),
    )
    .expect("write populated travel input");

    let direct_call = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        json_path.to_str().expect("JSON path"),
    ]);
    assert!(
        direct_call.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&direct_call.stderr),
        String::from_utf8_lossy(&direct_call.stdout)
    );
    let direct_result = parse_stdout(&direct_call);

    let hydrate = run(&[
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
        hydrate.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&hydrate.stderr),
        String::from_utf8_lossy(&hydrate.stdout)
    );

    {
        let mut workbook = open_workbook_auto(&xlsx_path).expect("hydrated travel workbook");
        let foreign_income_sheet =
            workbook_collection_sheet_name(&mut workbook, "årsinput.udenlandske_indkomstkilder");
        let travel_sheet = workbook_collection_sheet_name(&mut workbook, "årsinput.rejser");
        assert!(workbook
            .worksheet_range(&foreign_income_sheet)
            .expect("foreign income sheet")
            .rows()
            .flatten()
            .any(|cell| cell.to_string() == "foreign-income-a"));
        assert!(workbook
            .worksheet_range(&travel_sheet)
            .expect("travel sheet")
            .rows()
            .flatten()
            .any(|cell| cell.to_string() == "foreign-income-a"));
    }

    let xlsx_call = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        xlsx_path.to_str().expect("XLSX path"),
    ]);
    std::fs::remove_file(&json_path).ok();
    std::fs::remove_file(&xlsx_path).ok();
    assert!(
        xlsx_call.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&xlsx_call.stderr),
        String::from_utf8_lossy(&xlsx_call.stdout)
    );
    let xlsx_result = parse_stdout(&xlsx_call);
    assert_eq!(xlsx_result["diagnostics"], serde_json::json!([]));
    assert_eq!(
        xlsx_result["results"][0]["result"],
        direct_result["results"][0]["result"]
    );
    let annual = &xlsx_result["results"][0]["result"];
    assert_eq!(annual["udlandsindkomstkilder_gyldige"], true);
    assert_eq!(
        annual["rejseresultater"][0]["fradrag_før_årsloft_kroner"],
        893
    );
    assert_eq!(
        annual["rejseresultater"][1]["fradrag_før_årsloft_kroner"],
        7
    );
    assert_eq!(
        annual["rejseresultater"][2]["fradrag_før_årsloft_kroner"],
        500
    );
    assert_eq!(annual["samlet_rejsefradrag_efter_årsloft_kroner"], 1400);
}

#[test]
fn ligningslov9a_xlsx_round_trips_island_lodging_input() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/danish-income-tax/ligningsloven-par9a-rejser.calculate.runa");
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
            .expect("travel JSON template");
    input["cases"][0]["case_id"] = Value::String("ll9a-island-lodging".into());
    input["cases"][0]["input"] = serde_json::json!({
        "indkomstår": 2026,
        "årsinput": {
            "personrolle": { "$variant": "Ll9AAlmindeligLønmodtager" },
            "rejser": [],
            "udenlandske_indkomstkilder": [],
            "arbejdshistorik": {
                "tidligere_rejser": [],
                "arbejdsdage": [],
                "arbejdsstedsafstande": []
            },
            "ølogi": {
                "$variant": "MedØlogifradrag",
                "bopæl": {
                    "kommune": { "$variant": "Samsø" },
                    "ø": {
                        "$variant": "Ll9AAndenDanskØ",
                        "navn": "Samsø"
                    },
                    "vejforbindelse": {
                        "$variant": "Ll9AIngenFastVejforbindelseFraØen"
                    }
                },
                "arbejdsforhold": [{
                    "arbejdssted_identifikation": "fast-arbejdssted-fra-samsø",
                    "arbejdsstedskarakter": {
                        "$variant": "Ll9AØlogiFastArbejdssted"
                    },
                    "overnatningsforhold": {
                        "$variant": "Ll9AØlogiIngenMulighedForOvernatningPåSædvanligBopæl",
                        "afstand_ad_normal_transportvej_kilometer": 95,
                        "korteste_transporttid_hver_vej_minutter": 180
                    },
                    "hverv": { "$variant": "Ll9AAlmindeligtHverv" },
                    "ophold": [{
                        "identifikation": "samsø-fire-døgn",
                        "starttidspunkt": {
                            "dato": { "år": 2026, "måned": 6, "dag": 1 },
                            "klokkeslæt": { "time": 10, "minut": 20 }
                        },
                        "sluttidspunkt_eksklusiv": {
                            "dato": { "år": 2026, "måned": 6, "dag": 5 },
                            "klokkeslæt": { "time": 10, "minut": 20 }
                        },
                        "udgiftsforhold": {
                            "$variant": "Ll9AØlogiEgenUdgiftAfholdt"
                        }
                    }]
                }]
            },
            "dobbelt_husførelse": {
                "$variant": "Ll9AIntetFradragForDobbeltHusførelse"
            }
        },
        "ekstern_udelukkelse": { "$variant": "Ll9AIngenEksternUdelukkelse" }
    });
    std::fs::write(
        &json_path,
        serde_json::to_vec_pretty(&input).expect("encode island lodging input"),
    )
    .expect("write island lodging input");

    let direct_call = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        json_path.to_str().expect("JSON path"),
    ]);
    assert!(
        direct_call.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&direct_call.stderr),
        String::from_utf8_lossy(&direct_call.stdout)
    );
    let direct_result = parse_stdout(&direct_call);

    let hydrate = run(&[
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
        hydrate.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&hydrate.stderr),
        String::from_utf8_lossy(&hydrate.stdout)
    );

    {
        let mut workbook = open_workbook_auto(&xlsx_path).expect("hydrated island workbook");
        let work_sheet = workbook_collection_sheet_name(
            &mut workbook,
            "årsinput.ølogi.MedØlogifradrag.arbejdsforhold",
        );
        let stay_sheet = workbook_collection_sheet_name(
            &mut workbook,
            "årsinput.ølogi.MedØlogifradrag.arbejdsforhold.ophold",
        );
        let work_headers = workbook_headers(&mut workbook, &work_sheet);
        assert!(work_headers
            .iter()
            .any(|header| header == "Arbejdsstedets identifikation for ølogi"));
        let stay_headers = workbook_headers(&mut workbook, &stay_sheet);
        assert!(stay_headers
            .iter()
            .any(|header| header == "Ølogiopholdets starttidspunkt - time"));
        assert!(stay_headers
            .iter()
            .any(|header| header == "Ølogiopholdets sluttidspunkt - minut"));
        assert!(workbook
            .worksheet_range(&stay_sheet)
            .expect("island lodging stays")
            .rows()
            .flatten()
            .any(|cell| cell.to_string() == "samsø-fire-døgn"));

        let column_metadata = workbook
            .worksheet_range("_columns")
            .expect("island column metadata");
        let metadata_headers = column_metadata.rows().next().expect("metadata headers");
        let input_path_column = metadata_headers
            .iter()
            .position(|cell| cell.to_string() == "input_path")
            .expect("input path metadata column");
        let sources_column = metadata_headers
            .iter()
            .position(|cell| cell.to_string() == "sources")
            .expect("sources metadata column");
        let end_minute_metadata = column_metadata
            .rows()
            .skip(1)
            .find(|row| {
                row.get(input_path_column)
                    .map(ToString::to_string)
                    .as_deref()
                    == Some(
                        "årsinput.ølogi.MedØlogifradrag.arbejdsforhold.ophold.sluttidspunkt_eksklusiv.klokkeslæt.minut",
                    )
            })
            .expect("island lodging end-minute metadata");
        assert!(end_minute_metadata
            .get(sources_column)
            .map(ToString::to_string)
            .expect("island lodging sources")
            .contains("https://info.skat.dk/data.aspx?oid=2289990"));
    }

    let xlsx_call = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        xlsx_path.to_str().expect("XLSX path"),
    ]);
    std::fs::remove_file(&json_path).ok();
    std::fs::remove_file(&xlsx_path).ok();
    assert!(
        xlsx_call.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&xlsx_call.stderr),
        String::from_utf8_lossy(&xlsx_call.stdout)
    );
    let xlsx_result = parse_stdout(&xlsx_call);
    assert_eq!(xlsx_result["diagnostics"], serde_json::json!([]));
    assert_eq!(
        xlsx_result["results"][0]["result"],
        direct_result["results"][0]["result"]
    );
    let annual = &xlsx_result["results"][0]["result"];
    assert_eq!(annual["alle_input_gyldige"], true);
    assert_eq!(annual["ølogi"]["bopæl_omfattet_af_par9c_stk3"], true);
    assert_eq!(annual["ølogi"]["fradrag_før_fælles_årsloft_kroner"], 1072);
    assert_eq!(annual["samlet_ll9a_fradrag_efter_årsloft_kroner"], 1072);
    let stay = &annual["ølogi"]["arbejdsforholdsresultater"][0]["opholdsresultater"][0];
    assert_eq!(stay["samlet_varighed_minutter"], 5760);
    assert_eq!(stay["samlet_antal_fulde_logidøgn"], 4);
    assert_eq!(stay["antal_fulde_logidøgn_i_indkomståret"], 4);
    assert_eq!(stay["fakta"]["starttidspunkt"]["klokkeslæt"]["time"], 10);
    assert_eq!(
        stay["fakta"]["sluttidspunkt_eksklusiv"]["klokkeslæt"]["minut"],
        20
    );
}

#[test]
fn ligningslov9a_xlsx_round_trips_typed_double_household_input() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/danish-income-tax/ligningsloven-par9a-rejser.calculate.runa");
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
            .expect("travel JSON template");
    input["cases"][0]["case_id"] = Value::String("ll9a-double-household".into());
    input["cases"][0]["input"] = serde_json::json!({
        "indkomstår": 2026,
        "årsinput": {
            "personrolle": { "$variant": "Ll9AAlmindeligLønmodtager" },
            "rejser": [],
            "udenlandske_indkomstkilder": [],
            "arbejdshistorik": {
                "tidligere_rejser": [],
                "arbejdsdage": [],
                "arbejdsstedsafstande": []
            },
            "ølogi": { "$variant": "UdenØlogifradrag" },
            "dobbelt_husførelse": {
                "$variant": "Ll9ADobbeltHusførelseEfterStatsskattelov6",
                "input": {
                    "personkreds": { "$variant": "Sl6DhGift" },
                    "ophold": [{
                        "identifikation": "xlsx-dobbelt-husførelse",
                        "arbejdssted_identifikation": "xlsx-projektsted",
                        "oprindelig_startdato": { "år": 2026, "måned": 1, "dag": 1 },
                        "fradragsperiode_fra_dato": { "år": 2026, "måned": 1, "dag": 1 },
                        "fradragsperiode_til_dato": { "år": 2026, "måned": 4, "dag": 30 },
                        "erhvervsårsag": { "$variant": "Sl6DhSkatteydersEgetArbejdsforhold" },
                        "midlertidighed": {
                            "$variant": "Sl6DhMidlertidigtArbejde",
                            "art": { "$variant": "Sl6DhTidsbegrænsetAnsættelse" },
                            "aftalt_fra_dato": { "år": 2026, "måned": 1, "dag": 1 },
                            "aftalt_til_dato": { "år": 2026, "måned": 12, "dag": 31 }
                        },
                        "daglig_transport": {
                            "vurdering": {
                                "$variant": "Sl6DhDagligTransportIkkeRimeligEfterKonkretVurdering"
                            },
                            "afstand_mellem_boliger_kilometer": 250,
                            "samlet_transporttid_pr_dag_minutter": 360,
                            "arbejdstid_pr_dag_minutter": 480
                        },
                        "hjemmebolig": {
                            "$variant": "Sl6DhUdenlandskFamilieboligOpretholdt",
                            "familieplacering": {
                                "$variant": "Sl6DhSkatteyderIDanmarkFamilieIHjemlandet"
                            },
                            "dokumentation": {
                                "familieforbindelse_kildeidentifikation": "xlsx-vielsesdokument",
                                "bopælsregistrering_kildeidentifikation": "xlsx-bopælsregister",
                                "boligudgifter_kildeidentifikation": "xlsx-huslejekvitteringer"
                            }
                        },
                        "hjemlandsophold": [{
                            "identifikation": "xlsx-langt-hjemlandsophold",
                            "fra_dato": { "år": 2026, "måned": 1, "dag": 15 },
                            "til_dato": { "år": 2026, "måned": 3, "dag": 16 }
                        }],
                        "arbejdsboligform": { "$variant": "Sl6DhPrivatIndkvartering" },
                        "merudgiftsforhold": {
                            "$variant": "Sl6DhMerudgifterTilKostEllerBoligAfholdt"
                        },
                        "arbejdsgiverdækning": { "$variant": "Sl6DhIngenArbejdsgiverdækning" },
                        "opgørelsesmetode": {
                            "$variant": "Sl6DhStandardbeløb",
                            "antal_hele_uger": 17
                        },
                        "toårsgrænse": { "$variant": "Sl6DhAlmindeligToårsgrænse" }
                    }]
                }
            }
        },
        "ekstern_udelukkelse": { "$variant": "Ll9AIngenEksternUdelukkelse" }
    });
    std::fs::write(
        &json_path,
        serde_json::to_vec_pretty(&input).expect("encode double household input"),
    )
    .expect("write double household input");

    let direct_call = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        json_path.to_str().expect("JSON path"),
    ]);
    assert!(
        direct_call.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&direct_call.stderr),
        String::from_utf8_lossy(&direct_call.stdout)
    );
    let direct_result = parse_stdout(&direct_call);

    let hydrate = run(&[
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
        hydrate.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&hydrate.stderr),
        String::from_utf8_lossy(&hydrate.stdout)
    );

    {
        let mut workbook = open_workbook_auto(&xlsx_path).expect("double household workbook");
        let stay_path =
            "årsinput.dobbelt_husførelse.Ll9ADobbeltHusførelseEfterStatsskattelov6.input.ophold";
        let stay_sheet = workbook_collection_sheet_name(&mut workbook, stay_path);
        let expense_path = "årsinput.dobbelt_husførelse.Ll9ADobbeltHusførelseEfterStatsskattelov6.input.ophold.opgørelsesmetode.Sl6DhDokumenteredeMerudgifter.udgifter";
        let expense_sheet = workbook_collection_sheet_name(&mut workbook, expense_path);
        let home_stay_path = "årsinput.dobbelt_husførelse.Ll9ADobbeltHusførelseEfterStatsskattelov6.input.ophold.hjemlandsophold";
        let home_stay_sheet = workbook_collection_sheet_name(&mut workbook, home_stay_path);
        let stay_headers = workbook_headers(&mut workbook, &stay_sheet);
        assert!(stay_headers
            .iter()
            .any(|header| header == "Periodens identifikation"));
        assert!(stay_headers
            .iter()
            .any(|header| header == "Hele uger med standardbeløb"));
        let expense_headers = workbook_headers(&mut workbook, &expense_sheet);
        assert!(expense_headers
            .iter()
            .any(|header| header == "Dokumenteret merudgift"));
        assert!(expense_headers
            .iter()
            .any(|header| header == "Merudgiftens periode fra - år"));
        let home_stay_headers = workbook_headers(&mut workbook, &home_stay_sheet);
        assert!(home_stay_headers
            .iter()
            .any(|header| header == "Hjemlandsopholdets identifikation"));
        assert!(workbook
            .worksheet_range(&stay_sheet)
            .expect("double household stays")
            .rows()
            .flatten()
            .any(|cell| cell.to_string() == "xlsx-dobbelt-husførelse"));
        assert!(workbook
            .worksheet_range(&home_stay_sheet)
            .expect("double household home-country stays")
            .rows()
            .flatten()
            .any(|cell| cell.to_string() == "xlsx-langt-hjemlandsophold"));

        let column_metadata = workbook
            .worksheet_range("_columns")
            .expect("double household column metadata");
        let metadata_headers = column_metadata.rows().next().expect("metadata headers");
        let input_path_column = metadata_headers
            .iter()
            .position(|cell| cell.to_string() == "input_path")
            .expect("input path metadata column");
        let sources_column = metadata_headers
            .iter()
            .position(|cell| cell.to_string() == "sources")
            .expect("sources metadata column");
        let period_metadata = column_metadata
            .rows()
            .skip(1)
            .find(|row| {
                row.get(input_path_column)
                    .map(ToString::to_string)
                    .as_deref()
                    == Some(&format!("{stay_path}.oprindelig_startdato.år"))
            })
            .expect("double household period metadata");
        assert!(period_metadata
            .get(sources_column)
            .map(ToString::to_string)
            .expect("double household sources")
            .contains("https://www.retsinformation.dk/eli/lta/1922/149"));
    }

    let xlsx_call = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        xlsx_path.to_str().expect("XLSX path"),
    ]);
    std::fs::remove_file(&json_path).ok();
    std::fs::remove_file(&xlsx_path).ok();
    assert!(
        xlsx_call.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&xlsx_call.stderr),
        String::from_utf8_lossy(&xlsx_call.stdout)
    );
    let xlsx_result = parse_stdout(&xlsx_call);
    assert_eq!(xlsx_result["diagnostics"], serde_json::json!([]));
    assert_eq!(
        xlsx_result["results"][0]["result"],
        direct_result["results"][0]["result"]
    );
    let annual = &xlsx_result["results"][0]["result"];
    assert_eq!(annual["alle_input_gyldige"], true);
    assert_eq!(annual["dobbelt_husførelse_før_fælles_loft_kroner"], 3200);
    assert_eq!(annual["dobbelt_husførelse_efter_fælles_loft_kroner"], 3200);
    assert_eq!(annual["samlet_ll9a_fradrag_efter_årsloft_kroner"], 0);
    assert_eq!(annual["samlet_fradrag_under_fælles_årsloft_kroner"], 3200);
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
