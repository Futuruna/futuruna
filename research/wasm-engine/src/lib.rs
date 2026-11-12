//! The Futuruna interpreter as a WebAssembly module.
//!
//! A [`Model`] is `runa call` without a filesystem: the model's source files
//! come in as one JSON object, `{ "<path>": "<source>", ... }`, are parsed and
//! type-checked once, and each [`Model::call`] takes the JSON document
//! `runa call --input` reads and returns the document `runa call` writes, byte
//! for byte.

use futuruna::calculate::{self, CalculationContract, CalculationInputEnvelope};
use futuruna::{
    parse_prelude, prepend_prelude, provide_source_files, Lexer, Parser, Stmt, TypeChecker,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
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

/// A parsed and type-checked model with its calculation entries.
#[wasm_bindgen]
pub struct Model {
    files: Vec<(PathBuf, String)>,
    statements: Vec<Stmt>,
    source_dir: Option<String>,
    contracts: Vec<CalculationContract>,
}

#[wasm_bindgen]
impl Model {
    /// Load the model whose root file is `main`.
    ///
    /// `sources` maps paths relative to the model root
    /// (`src/tax_report.calculate.runa`) to their text, and must hold `main`
    /// and every file it imports. A model that does not parse or type-check is
    /// an error carrying the diagnostics.
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
        Ok(Model {
            files,
            statements,
            source_dir,
            contracts: artifacts.calculation_contracts,
        })
    }

    /// The model's calculation entries.
    pub fn entries(&self) -> Vec<String> {
        self.contracts
            .iter()
            .map(|contract| contract.entry.clone())
            .collect()
    }

    /// Run the calculation `entry` on `input`, a `runa call` input document,
    /// and return the `runa call` output document.
    pub fn call(&self, entry: &str, input: &str) -> Result<String, String> {
        let contract = self
            .contracts
            .iter()
            .find(|contract| contract.entry == entry)
            .ok_or_else(|| {
                format!(
                    "unknown calculation entry `{entry}`; available entries: {}",
                    self.entries().join(", ")
                )
            })?;
        let envelope: CalculationInputEnvelope = calculate::parse_calculation_json(input)
            .and_then(serde_json::from_value)
            .map_err(|error| format!("calculation input: {error}"))?;
        // Imports are read again at run time; serve them from this model.
        provide_source_files(self.files.iter().cloned());
        let result = calculate::invoke_calculation_cases(
            contract,
            &self.statements,
            self.source_dir.clone(),
            &envelope,
        );
        let document = serde_json::to_string_pretty(&result).map_err(|error| error.to_string())?;
        Ok(document + "\n")
    }
}
