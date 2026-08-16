use futuruna::{scan_meta_comments_with_dir, MetaValue, MetaValueArgument};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

struct FindingSource {
    identifier: String,
    url: String,
    fetched_date: String,
}

struct Finding {
    label: String,
    title: String,
    model_layer: String,
    statement_status: String,
    scope: String,
    program_references: Vec<String>,
    sources: Vec<FindingSource>,
    assumptions: Vec<String>,
    result: String,
    limitation: String,
    source_file: String,
    code_id: String,
}

fn main() {
    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be available"),
    );
    let repository_root = manifest_dir
        .parent()
        .expect("website must live below the repository root");
    let corpus_root = repository_root.join("examples/danish-constitution");
    println!("cargo:rerun-if-changed={}", corpus_root.display());

    let files = [
        ("grundlov-bestemmelser.audit.runa", "audit-bestemmelser"),
        ("grundlov-procedurer.audit.runa", "audit-procedurer"),
        ("grundlov-rettigheder.audit.runa", "audit-rettigheder"),
        ("grundlov-tvaergaaende.audit.runa", "audit-tvaergaaende"),
        (
            "fortolkninger/troskrav-og-ligebehandling.fortolkning.runa",
            "fortolkning-troskrav",
        ),
        (
            "fortolkninger/skat-og-ekspropriation.fortolkning.runa",
            "fortolkning-skat-ekspropriation",
        ),
    ];

    let mut findings = Vec::new();
    for (relative_path, code_id) in files {
        let path = corpus_root.join(relative_path);
        println!("cargo:rerun-if-changed={}", path.display());
        collect_findings(&path, relative_path, code_id, &mut findings);
    }

    if findings.len() != files.len() {
        panic!(
            "expected one GrundlovPrøvningsfund in each of {} files, found {}",
            files.len(),
            findings.len()
        );
    }

    let output = render_findings(&findings);
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must be available"));
    fs::write(out_dir.join("danish_constitution_findings.rs"), output)
        .expect("generated Constitution findings must be writable");
}

fn collect_findings(path: &Path, relative_path: &str, code_id: &str, findings: &mut Vec<Finding>) {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let source_dir = path
        .parent()
        .expect("metadata file must have a parent")
        .to_string_lossy()
        .into_owned();
    let index = scan_meta_comments_with_dir(&source, Some(source_dir));
    if !index.diagnostics.is_empty() {
        let diagnostics = index
            .diagnostics
            .iter()
            .map(|diagnostic| format!("line {}: {}", diagnostic.line, diagnostic.message))
            .collect::<Vec<_>>()
            .join("; ");
        panic!("invalid metadata in {}: {diagnostics}", path.display());
    }

    for anchor in &index.anchors {
        for reference in &anchor.references {
            if reference.qualified_type.as_deref() != Some("GrundlovPrøvningsfund") {
                continue;
            }
            let value = reference.static_data.as_ref().unwrap_or_else(|| {
                panic!(
                    "GrundlovPrøvningsfund {} in {} is not statically resolvable",
                    anchor.label,
                    path.display()
                )
            });
            findings.push(parse_finding(&anchor.label, value, relative_path, code_id));
        }
    }
}

fn parse_finding(label: &str, value: &MetaValue, source_file: &str, code_id: &str) -> Finding {
    let (_, arguments) = constructor(value, "GrundlovPrøvningsfund");
    let model_layer_value = field(arguments, "modellag");
    let model_layer = match constructor_name(model_layer_value) {
        "Kildemodel" => "Kildemodel".to_string(),
        "Fortolkningsmodel" => {
            let (_, model_arguments) = constructor(model_layer_value, "Fortolkningsmodel");
            format!(
                "Fortolkningsmodel: {}",
                string(field(model_arguments, "navn"))
            )
        }
        other => panic!("unsupported Constitution model layer {other}"),
    };

    let statement_status = display_constructor(
        constructor_name(field(arguments, "udsagnsstatus")),
        &[
            ("TekstnærKontrol", "Tekstnær kontrol"),
            ("Modelresultat", "Modelresultat"),
            ("Fortolkningsspørgsmål", "Fortolkningsspørgsmål"),
            ("RetskildestøttetKonklusion", "Retskildestøttet konklusion"),
        ],
    );
    let scope = display_constructor(
        constructor_name(field(arguments, "prøvningsomfang")),
        &[
            ("EnkeltScenarie", "Enkelt scenarie"),
            ("Scenariesamling", "Scenariesamling"),
            ("Grænseanalyse", "Grænseanalyse"),
            ("UdtømmendeLukketDomæne", "Udtømmende lukket domæne"),
            ("AfgrænsetSøgning", "Afgrænset søgning"),
        ],
    );

    let program_references = list(field(arguments, "programhenvisninger"))
        .iter()
        .map(program_reference)
        .collect();

    let (_, basis_arguments) = constructor(field(arguments, "grundlag"), "Fortolkningsgrundlag");
    let sources = list(field(basis_arguments, "kilder"))
        .iter()
        .map(finding_source)
        .collect();
    let assumptions = list(field(basis_arguments, "forudsætninger"))
        .iter()
        .map(|value| string(value).to_string())
        .collect();

    Finding {
        label: label.to_string(),
        title: string(field(arguments, "titel")).to_string(),
        model_layer,
        statement_status,
        scope,
        program_references,
        sources,
        assumptions,
        result: string(field(arguments, "resultat")).to_string(),
        limitation: string(field(arguments, "afgrænsning")).to_string(),
        source_file: source_file.to_string(),
        code_id: code_id.to_string(),
    }
}

fn finding_source(value: &MetaValue) -> FindingSource {
    let (_, arguments) = constructor(value, "GrundlovKildeInfo");
    FindingSource {
        identifier: string(field(arguments, "identifier")).to_string(),
        url: string(field(arguments, "url")).to_string(),
        fetched_date: date(field(arguments, "hentet_dato")),
    }
}

fn program_reference(value: &MetaValue) -> String {
    match constructor_name(value) {
        "ProgramSymbolReference" => {
            let (_, arguments) = constructor(value, "ProgramSymbolReference");
            string(field(arguments, "name")).to_string()
        }
        "ProgramMemberReference" => {
            let (_, arguments) = constructor(value, "ProgramMemberReference");
            format!(
                "{}::{}",
                string(field(arguments, "root_type")),
                string(field(arguments, "path"))
            )
        }
        other => panic!("unsupported program reference {other}"),
    }
}

fn date(value: &MetaValue) -> String {
    let (_, arguments) = constructor(value, "Dato");
    let year = integer(field(arguments, "år"));
    let month = integer(field(arguments, "måned"));
    let day = integer(field(arguments, "dag"));
    format!("{year:04}-{month:02}-{day:02}")
}

fn render_findings(findings: &[Finding]) -> String {
    let mut output = String::from("const DK_CONSTITUTION_FINDINGS: &[ConstitutionFinding] = &[\n");
    for finding in findings {
        output.push_str("    ConstitutionFinding {\n");
        render_field(&mut output, "label", &finding.label);
        render_field(&mut output, "title", &finding.title);
        render_field(&mut output, "model_layer", &finding.model_layer);
        render_field(&mut output, "statement_status", &finding.statement_status);
        render_field(&mut output, "scope", &finding.scope);
        render_string_slice(
            &mut output,
            "program_references",
            &finding.program_references,
        );
        output.push_str("        sources: &[\n");
        for source in &finding.sources {
            output.push_str("            ConstitutionFindingSource {\n");
            render_indented_field(&mut output, "identifier", &source.identifier, 16);
            render_indented_field(&mut output, "url", &source.url, 16);
            render_indented_field(&mut output, "fetched_date", &source.fetched_date, 16);
            output.push_str("            },\n");
        }
        output.push_str("        ],\n");
        render_string_slice(&mut output, "assumptions", &finding.assumptions);
        render_field(&mut output, "result", &finding.result);
        render_field(&mut output, "limitation", &finding.limitation);
        render_field(&mut output, "source_file", &finding.source_file);
        render_field(&mut output, "code_id", &finding.code_id);
        output.push_str("    },\n");
    }
    output.push_str("];\n");
    output
}

fn render_field(output: &mut String, name: &str, value: &str) {
    render_indented_field(output, name, value, 8);
}

fn render_indented_field(output: &mut String, name: &str, value: &str, spaces: usize) {
    output.push_str(&" ".repeat(spaces));
    output.push_str(name);
    output.push_str(": ");
    output.push_str(&format!("{:?}", value));
    output.push_str(",\n");
}

fn render_string_slice(output: &mut String, name: &str, values: &[String]) {
    output.push_str("        ");
    output.push_str(name);
    output.push_str(": &[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            output.push_str(", ");
        }
        output.push_str(&format!("{:?}", value));
    }
    output.push_str("],\n");
}

fn display_constructor(name: &str, labels: &[(&str, &str)]) -> String {
    labels
        .iter()
        .find_map(|(constructor, label)| (*constructor == name).then_some((*label).to_string()))
        .unwrap_or_else(|| panic!("unsupported metadata constructor {name}"))
}

fn constructor<'a>(
    value: &'a MetaValue,
    expected_name: &str,
) -> (&'a str, &'a [MetaValueArgument]) {
    match value {
        MetaValue::Constructor {
            name, arguments, ..
        } if name == expected_name => (name, arguments),
        _ => panic!("expected {expected_name}, found {}", value.to_source()),
    }
}

fn constructor_name(value: &MetaValue) -> &str {
    match value {
        MetaValue::Constructor { name, .. } => name,
        _ => panic!("expected constructor, found {}", value.to_source()),
    }
}

fn field<'a>(arguments: &'a [MetaValueArgument], name: &str) -> &'a MetaValue {
    arguments
        .iter()
        .find(|argument| argument.field.as_deref() == Some(name))
        .map(|argument| &argument.value)
        .unwrap_or_else(|| panic!("missing metadata field {name}"))
}

fn string(value: &MetaValue) -> &str {
    match value {
        MetaValue::String(value) => value,
        _ => panic!("expected string, found {}", value.to_source()),
    }
}

fn integer(value: &MetaValue) -> i64 {
    match value {
        MetaValue::Integer(value) => *value,
        _ => panic!("expected integer, found {}", value.to_source()),
    }
}

fn list(value: &MetaValue) -> &[MetaValue] {
    match value {
        MetaValue::List(values) => values,
        _ => panic!("expected list, found {}", value.to_source()),
    }
}
