//! Danish source files: keywords and Danish builtin names are resolved per
//! file, after user declarations, and identically in both execution modes.

use futuruna::{parse_prelude, Defn, Stmt, TypeChecker, TypeDecl, DANISH_BUILTIN_NAMES};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIR_ID: AtomicU64 = AtomicU64::new(0);

fn runa() -> std::ffi::OsString {
    std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into())
}

struct Project(PathBuf);

impl Project {
    fn new(files: &[(&str, &str)]) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "futuruna-danish-{}-{}",
            std::process::id(),
            NEXT_DIR_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        for (name, source) in files {
            std::fs::write(dir.join(name), source).unwrap();
        }
        Project(dir)
    }

    fn runa(&self, args: &[&str]) -> Output {
        Command::new(runa())
            .args(args)
            .current_dir(&self.0)
            .env("FUTURUNA_DISABLE_COMPILER_CACHE", "1")
            .output()
            .unwrap()
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn stderr(output: &Output) -> String {
    assert!(!output.status.success(), "{output:?}");
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// Run `main` interpreted and compiled; both must print `expected`.
fn assert_both_modes(project: &Project, main: &str, expected: &str) {
    assert_eq!(
        stdout(&project.runa(&[main])),
        expected,
        "interpreted {main}"
    );
    assert_eq!(
        stdout(&project.runa(&["run", main])),
        expected,
        "compiled {main}"
    );
}

const DANISH_LIBRARY: &str = "@ sprog da
# Sag(beløb: Heltal, første: Heltal)
# Status = Godkendt | Afvist
> tæl(x: Heltal) -> Heltal { x + 1000 }
> fradrag(beløb: Heltal, par: Heltal) -> Heltal { hvis beløb > 100 { beløb - par } ellers { 0 } }
| afgørelse(s: Sag) -> Afvist
| afgørelse(s: Sag) -> Godkendt under s.beløb > 10
= grænse = 50
";

const ENGLISH_LIBRARY: &str = "-- \"tag\" counts taggable items; \"hale\" is a premium tail amount
> tag(xs: List(Int), n: Int) -> Int { n * 1000 }
> hale(xs: List(Int)) -> Int { 555 }
";

#[test]
fn imports_call_user_functions_by_declared_names_across_languages() {
    let project = Project::new(&[
        ("dalib.runa", DANISH_LIBRARY),
        (
            "en_main.runa",
            "@ import ./dalib
= s = Sag(500, 1)
@ print(show(fradrag(500, 20)))
@ print(show(afgørelse(s)))
@ print(show(tæl(1)))
@ print(show(count([4, 5])))
@ print(show(s.første))
@ print(show(grænse))
",
        ),
        ("enlib.runa", ENGLISH_LIBRARY),
        (
            "da_main.runa",
            "@ sprog da
@ importer ./enlib
@ print(vis(tag([1, 2, 3], 2)))
@ print(vis(hale([1, 2, 3])))
@ print(vis(sidste([1, 2, 3])))
",
        ),
    ]);
    assert_both_modes(&project, "en_main.runa", "480\nGodkendt\n1001\n2\n1\n50");
    assert_both_modes(&project, "da_main.runa", "2000\n555\n3");
}

#[test]
fn user_functions_named_like_danish_builtins_are_ordinary_functions() {
    let project = Project::new(&[
        (
            "da.runa",
            "@ sprog da
# Sag(beløb: Heltal)
> vis(s: Sag) -> Tekst { \"Sagen vedrører \" + tekst(s.beløb) + \" kr.\" }
> tekst(n: Heltal) -> Tekst { \"\"\"{{n}}\"\"\" }
@ print(vis(Sag(500)))
@ print(\"\"\"Skat: {{1200}}\"\"\")
@ print(show(7))
",
        ),
        (
            "en.runa",
            "> vis(a: Int) -> String { \"mine\" }
> par(a: Int, b: Int) -> Int { a * b }
> tæl(x: Int) -> Int { x + 100 }
= tag = |x| { x + 1 }
> f(hale: Int) -> Int { hale * 2 }
@ print(vis(5))
@ print(show(par(6, 7)))
@ print(show(tæl(5)))
@ print(show(tag(5)))
@ print(show(f(4)))
",
        ),
    ]);
    assert_both_modes(&project, "da.runa", "Sagen vedrører 500 kr.\nSkat: 1200\n7");
    assert_both_modes(&project, "en.runa", "mine\n42\n105\n6\n8");
}

#[test]
fn english_files_do_not_know_danish_builtin_names() {
    let project = Project::new(&[("en.runa", "@ print(show(hale([7, 8])))\n")]);
    let error = stderr(&project.runa(&["check", "--frontend", "en.runa"]));
    assert!(error.contains("undefined function `hale`"), "{error}");
}

#[test]
fn declared_names_keep_their_spelling_in_named_arguments_fields_and_interpolation() {
    let project = Project::new(&[(
        "navne.runa",
        "@ sprog da
| frist(start: Heltal, område: Heltal) -> start + område
# Sag(første: Heltal, beløb: Heltal) {
    | dobbelt() -> beløb * 2
    | start_beløb() -> første + beløb
}
# Post(første: Heltal, start: Heltal, beløb: Heltal)
= s = Sag(første = 7, beløb = 100)
= p = Post(1, 2, 3)
@ print(vis(frist(start = 1, område = 2)))
@ print(vis(s.dobbelt()))
@ print(vis(s.start_beløb()))
@ print(\"A: \" + vis(p.første) + \" \" + vis(p.start))
@ print(\"\"\"B: {{p.beløb}} C: {{p.første}}\"\"\")
",
    )]);
    assert_both_modes(&project, "navne.runa", "3\n200\n107\nA: 1 2\nB: 3 C: 1");
}

#[test]
fn calculation_contracts_publish_declared_danish_variants() {
    let project = Project::new(&[(
        "kontrol.runa",
        "@ sprog da
# Kontrol = Godkendt | Fejl
# Ind(beløb: Heltal)
# Ud(kontrol: Kontrol)
@ calculate
> kontroller(ind: Ind) -> Ud {
    hvis ind.beløb > 0 { Ud(Godkendt) } ellers { Ud(Fejl) }
}
",
    )]);
    let schema: Value =
        serde_json::from_str(&stdout(&project.runa(&["schema", "kontrol.runa"]))).unwrap();
    let kontrol = schema["definitions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|definition| definition["name"] == "Kontrol")
        .expect("Kontrol definition");
    let variants: Vec<&str> = kontrol["variants"]
        .as_array()
        .unwrap()
        .iter()
        .map(|variant| variant["name"].as_str().unwrap())
        .collect();
    assert_eq!(variants, ["Godkendt", "Fejl"]);

    let input = project.path("input.json");
    let mut template: Value =
        serde_json::from_str(&stdout(&project.runa(&["template", "kontrol.runa"]))).unwrap();
    template["cases"][0]["input_status"] = Value::from("ready");
    template["cases"][0]["input"]["beløb"] = Value::from(0);
    std::fs::write(&input, template.to_string()).unwrap();
    let result: Value = serde_json::from_str(&stdout(&project.runa(&[
        "call",
        "kontrol.runa",
        "--input",
        input.to_str().unwrap(),
    ])))
    .unwrap();
    assert_eq!(
        result["results"][0]["result"]["kontrol"]["$variant"],
        "Fejl"
    );
}

#[test]
fn danish_type_and_constructor_names_yield_to_declarations() {
    let project = Project::new(&[(
        "typer.runa",
        "@ sprog da
# Felt = Tekst | Tegn
# Svar = Noget(Heltal) | Intet
> tal(s: Svar) -> Heltal { skel s { | Noget(n) -> n | Intet -> 0 } }
> indbygget(x: Heltal) -> Option(Heltal) { hvis x > 0 { Some(x) } ellers { None } }
@ print(vis(Tekst))
@ print(vis(Noget(3)))
@ print(vis(tal(Noget(4)) + tal(Intet)))
@ print(vis(indbygget(2)))
",
    )]);
    assert_both_modes(&project, "typer.runa", "Tekst\nNoget(3)\n4\nSome(2)");

    let builtin = Project::new(&[(
        "resultat.runa",
        "@ sprog da
> del(a: Heltal, b: Heltal) -> Result(Heltal, Tekst) {
    hvis b == 0 { Fejl(\"division med nul\") } ellers { Ok(a / b) }
}
> positiv(x: Heltal) -> Option(Heltal) { hvis x > 0 { Noget(x) } ellers { Intet } }
@ print(vis(del(6, 3)))
@ print(vis(del(1, 0)))
@ print(vis(positiv(9)))
@ print(vis(positiv(0)))
",
    )]);
    assert_both_modes(
        &builtin,
        "resultat.runa",
        "Ok(2)\nErr(division med nul)\nSome(9)\nNone",
    );
}

#[test]
fn language_declaration_is_read_by_the_tokenizer() {
    let body = "@ print(vis(hvis Sandt { 7 } ellers { 8 }))\n";
    for (index, header) in [
        "\u{feff}@ sprog da\n",
        "@ sprog DA\n",
        "@sprog Dansk -- skrevet på dansk, ikke english\n",
        "-- indledning\n\n@ language da\n",
    ]
    .iter()
    .enumerate()
    {
        let name = format!("header{index}.runa");
        let source = format!("{header}{body}");
        let project = Project::new(&[(&name, &source)]);
        assert_eq!(stdout(&project.runa(&[&name])), "7", "{header:?}");
    }

    let english = Project::new(&[(
        "en.runa",
        "@ sprog en -- ikke dansk\n@ print(show(if True { 7 } else { 8 }))\n",
    )]);
    assert_eq!(stdout(&english.runa(&["en.runa"])), "7");

    let unknown = Project::new(&[("xx.runa", "@ sprog xx\n@ print(show(1))\n")]);
    let error = stderr(&unknown.runa(&["check", "--frontend", "xx.runa"]));
    assert!(error.contains("unknown language `xx`"), "{error}");
}

#[test]
fn byte_order_mark_does_not_shift_diagnostic_columns() {
    let project = Project::new(&[("bom.runa", "\u{feff}= x = ukendt\n")]);
    let error = stderr(&project.runa(&["check", "--frontend", "bom.runa"]));
    assert!(error.contains("bom.runa:1:7"), "{error}");
}

#[test]
fn every_danish_builtin_name_names_an_existing_builtin() {
    let checker = TypeChecker::new();
    let mut prelude_names = std::collections::BTreeSet::new();
    for stmt in parse_prelude() {
        match stmt {
            Stmt::Defn(Defn::Fn { name, .. }) => {
                prelude_names.insert(name);
            }
            Stmt::TypeDecl(TypeDecl::EffectDecl { ops, .. }) => {
                prelude_names.extend(ops.into_iter().map(|(name, _, _)| name));
            }
            _ => {}
        }
    }
    let missing: Vec<_> = DANISH_BUILTIN_NAMES
        .iter()
        .filter(|(_, english)| {
            !checker.builtins.contains_key(*english) && !prelude_names.contains(*english)
        })
        .collect();
    assert!(
        missing.is_empty(),
        "Danish names without a builtin: {missing:?}"
    );
}

#[test]
fn diagnostics_in_danish_files_use_the_danish_names() {
    let project = Project::new(&[
        ("arity.runa", "@ sprog da\n@ print(vis(fold([1, 2], 0)))\n"),
        (
            "returner.runa",
            "@ sprog da\n> f(x: Heltal) -> Heltal {\n    returner x\n}\n",
        ),
    ]);
    let error = stderr(&project.runa(&["check", "--frontend", "arity.runa"]));
    assert!(
        error.contains("builtin `fold` expects 3 arguments"),
        "{error}"
    );
    let error = stderr(&project.runa(&["check", "--frontend", "returner.runa"]));
    assert!(error.contains("omit `returner`"), "{error}");
}

#[test]
fn danish_number_parsers_accept_danish_number_text() {
    let project = Project::new(&[(
        "tal.runa",
        "@ sprog da
@ print(vis(fortolk_kommatal(\"1,5\")))
@ print(vis(fortolk_heltal(\"1.000\")))
@ print(vis(fortolk_heltal(\" -1.250.000 \")))
@ print(vis(fortolk_kommatal(\"1.234,75\")))
@ print(vis(fortolk_heltal(\"1,5\")))
@ print(vis(fortolk_kommatal(\"1.5\")))
@ print(vis(fortolk_heltal(\"12.34\")))
@ print(vis(fortolk_heltal(\"99.999.999.999.999.999.999\")))
@ print(vis(sorter([\"Bo\", \"Åse\", \"Ærø\", \"anna\"])))
@ print(vis(Sandt))
",
    )]);
    assert_both_modes(
        &project,
        "tal.runa",
        "Ok(1.5)\nOk(1000)\nOk(-1250000)\nOk(1234.75)\n\
Err(`1,5` is not a Danish integer)\n\
Err(`1.5` is not a Danish decimal number)\n\
Err(`12.34` is not a Danish integer)\n\
Err(`99.999.999.999.999.999.999` is outside the Int range)\n\
[Bo, anna, Åse, Ærø]\ntrue",
    );
}

#[test]
fn danish_names_resolve_relative_to_the_importing_file() {
    let project = Project::new(&[]);
    std::fs::create_dir_all(project.path("lib")).unwrap();
    std::fs::write(project.path("lib/enlib.runa"), ENGLISH_LIBRARY).unwrap();
    std::fs::create_dir_all(project.path("app")).unwrap();
    std::fs::write(
        project.path("app/main.runa"),
        "@ sprog da\n@ importer ../lib/enlib\n@ print(vis(tag([1], 3)))\n",
    )
    .unwrap();
    let main = Path::new("app").join("main.runa");
    assert_eq!(stdout(&project.runa(&[main.to_str().unwrap()])), "3000");
}
