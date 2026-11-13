//! The Futuruna interpreter as a WebAssembly module.
//!
//! A [`Model`] is `runa call` without a filesystem: the model's source files
//! come in as one JSON object, `{ "<path>": "<source>", ... }`, are parsed and
//! type-checked once, and each [`Model::call`] takes the JSON document
//! `runa call --input` reads and returns the document `runa call` writes, byte
//! for byte.
//!
//! Each calculation entry is prepared on its first call: the interpreter
//! initializes the model (prelude, declarations, the top-level and metadata
//! bindings the entry reaches) once, and every later call of that entry, full
//! or partial, runs its cases from that state. Each case still starts from
//! the initialized state alone, as in `runa call`
//! (`futuruna::calculate::PreparedCalculation`).

use futuruna::calculate::{self, CalculationInputEnvelope, PreparedCalculation};
use futuruna::{
    parse_prelude, prepend_prelude, provide_source_files, Lexer, Parser, Stmt, TypeChecker,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wasm_bindgen::prelude::*;

/// Every model file is placed under this directory, so imports resolve
/// between them exactly as they do on disk.
const MODEL_ROOT: &str = "/model";

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn error(message: &str);
}

/// A panic aborts the WebAssembly call; its message goes to `console.error`
/// so the host sees why.
fn report_panics() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(|| {
        std::panic::set_hook(Box::new(|info| error(&info.to_string())));
    });
}

/// A parsed and type-checked model with its calculation entries, each
/// prepared on its first call and reused by every later one.
#[wasm_bindgen]
pub struct Model {
    files: Vec<(PathBuf, String)>,
    entries: Vec<PreparedCalculation>,
    /// The root file, as the interpreter sees it (`/model/<main>`).
    main: PathBuf,
}

#[wasm_bindgen]
impl Model {
    /// Load the model whose root file is `main`.
    ///
    /// `sources` maps paths relative to the model root
    /// (`src/tax_report.calculate.runa`) to their text, and must hold `main`
    /// and every file it imports. A model that does not parse or type-check is
    /// an error carrying the diagnostics. No entry is prepared yet: the first
    /// call of an entry also initializes the model for it.
    #[wasm_bindgen(constructor)]
    pub fn new(sources: &str, main: &str) -> Result<Model, String> {
        report_panics();
        let sources: BTreeMap<String, String> =
            serde_json::from_str(sources).map_err(|error| format!("model sources: {error}"))?;
        let main_source = sources
            .get(main)
            .ok_or_else(|| format!("model sources hold no `{main}`"))?
            .clone();
        let root = Path::new(MODEL_ROOT);
        let files = sources
            .into_iter()
            .map(|(path, source)| (root.join(path), source))
            .collect::<Vec<_>>();
        provide_source_files(files.iter().cloned());

        let user_statements = Parser::new(Lexer::new(&main_source).tokenize(), &main_source)
            .parse_program()
            .map_err(|error| format!("{main}: {error:?}"))?;
        let statements = prepend_prelude(parse_prelude(), &user_statements);
        let source_dir = root
            .join(main)
            .parent()
            .map(|dir| dir.to_string_lossy().into_owned());
        let artifacts =
            TypeChecker::check_with_artifacts(&statements, source_dir.clone(), &main_source);
        if !artifacts.diagnostics.is_empty() {
            let messages = artifacts
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>();
            return Err(format!("{main}: {}", messages.join("\n")));
        }
        let statements: Arc<[Stmt]> = statements.into();
        let entries = artifacts
            .calculation_contracts
            .into_iter()
            .map(|contract| {
                PreparedCalculation::new(contract, statements.clone(), source_dir.clone())
                    .with_root_file(root.join(main))
            })
            .collect();
        Ok(Model {
            files,
            entries,
            main: root.join(main),
        })
    }

    /// The model's calculation entries.
    pub fn entries(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|entry| entry.contract().entry.clone())
            .collect()
    }

    /// Run the calculation `entry` on `input`, a `runa call` input document,
    /// and return the `runa call` output document.
    pub fn call(&mut self, entry: &str, input: &str) -> Result<String, String> {
        self.invoke(entry, input, None, None)
    }

    /// Run the calculation `entry` on `input`, whose cases may leave facts
    /// out, and return the `runa call --partial` output document: each case
    /// answers `Complete` or `Needs` (`docs/rfcs/calculation-needs.md`).
    #[wasm_bindgen(js_name = callPartial)]
    pub fn call_partial(&mut self, entry: &str, input: &str) -> Result<String, String> {
        self.invoke(
            entry,
            input,
            Some(futuruna::partial::PartialMode::Propagate),
            None,
        )
    }

    /// `call`, with an explanation of every leaf of each result: the
    /// `runa call --explain` document, or `--explain=full` when `full`.
    #[wasm_bindgen(js_name = callExplained)]
    pub fn call_explained(
        &mut self,
        entry: &str,
        input: &str,
        full: Option<bool>,
    ) -> Result<String, String> {
        self.invoke(entry, input, None, Some(full.unwrap_or(false)))
    }

    /// `callPartial`, with an explanation of every `Complete` result: the
    /// `runa call --partial --explain` document.
    #[wasm_bindgen(js_name = callPartialExplained)]
    pub fn call_partial_explained(
        &mut self,
        entry: &str,
        input: &str,
        full: Option<bool>,
    ) -> Result<String, String> {
        self.invoke(
            entry,
            input,
            Some(futuruna::partial::PartialMode::Propagate),
            Some(full.unwrap_or(false)),
        )
    }
}

impl Model {
    fn invoke(
        &mut self,
        entry: &str,
        input: &str,
        partial: Option<futuruna::partial::PartialMode>,
        explain: Option<bool>,
    ) -> Result<String, String> {
        let Some(index) = self
            .entries
            .iter()
            .position(|prepared| prepared.contract().entry == entry)
        else {
            return Err(format!(
                "unknown calculation entry `{entry}`; available entries: {}",
                self.entries().join(", ")
            ));
        };
        let envelope: CalculationInputEnvelope = calculate::parse_calculation_json(input)
            .and_then(serde_json::from_value)
            .map_err(|error| format!("calculation input: {error}"))?;
        // An entry's first call reads the imports again while it initializes
        // the model; serve them from this model.
        provide_source_files(self.files.iter().cloned());
        let prepared = &mut self.entries[index];
        let options = explain.map(|full| calculate::ExplainOptions {
            full,
            root_file: Some(self.main.clone()),
            display_root: Some(PathBuf::from(MODEL_ROOT)),
        });
        let result = match (partial, &options) {
            (None, None) => prepared.invoke(&envelope),
            (Some(mode), None) => prepared.invoke_partial(&envelope, mode),
            (None, Some(options)) => prepared.invoke_explained(&envelope, options),
            (Some(mode), Some(options)) => {
                prepared.invoke_partial_explained(&envelope, mode, options)
            }
        };
        let document = serde_json::to_string_pretty(&result).map_err(|error| error.to_string())?;
        Ok(document + "\n")
    }
}
