# What runs in WebAssembly: the compiled model or the interpreter

Status: decision record for D1 (tax-service roadmap, milestone M2)

## Question

The tax engine runs inside Node (`endavu-service`). Two things can be compiled
to WebAssembly:

- **A. The compiled model.** `runa wasm` turns the model into Rust and the
  Rust into a WebAssembly package. Each exported function is one entry point.
- **B. The interpreter.** The `futuruna` library crate, built for
  `wasm32-unknown-unknown`, loads the model's sources and runs a calculation
  entry exactly as `runa call` does.

Both were built and run on the tax engine from Node 22. This document records
what each route takes, what it measures, and what it cannot do.

## Route A: the compiled model

The tax report is exported through a wrapper in the tax service
(`src/wasm_entry.runa`): a `TaxCase` as JSON in, the `TaxReportOutcome` as
JSON out, through the model's own Futuruna codec (`codec.runa`):

```runa
@ import ./codec

@ export
> tax_case_json(body: String) -> String {
    match decode_tax_case(body) {
        | Ok(tc) -> outcome_json(tax_report(tc))
        | Err(e) -> json_object([["error", text_json(e)]])
    }
}
```

```sh
runa wasm --target nodejs src/wasm_entry.runa
node wasm/compiled.mjs wasm_entry-wasm cases/demo-2026.json
```

`wasm/compiled.mjs` calls the export once per case and wraps the results in
the `runa call` output envelope, with object keys sorted as `runa call` writes
them.

What it took in the compiler:

- **Top-level bindings of generic types.** A binding stored outside `main`
  (every top-level binding of a WASM or `runa lib` module, and any binding a
  function reads) is declared with its Rust type. A constructor of a generic
  type, such as `= ask_ceiling_meta = Metadata(attachments = (Source(...), ...))`,
  was declared without its type arguments (`Option<Metadata>`), which rustc
  rejects. The type arguments are now instantiated from the constructor's
  arguments, also through tuples, lists and nested generic constructors
  (`Metadata<(TaxMeta<SourceInfo>, ...)>`). This also fixed `runa build` of a
  program whose function reads such a binding.
- **`String` parameters.** WASM mode declared every read-only `String`
  parameter of every function as `&str`, while the function bodies are
  generated for `&String` (`*name == "FritDepot"` compares `str` with `&str`).
  Private functions now borrow `&String` as in native code; a WASM export takes
  `&str` at the JavaScript boundary and works on an owned copy, the ABI
  `runa lib` exports already use.
- **Type checking.** `runa wasm` now type-checks before generating Rust, as
  `runa build` does, and hands the checked rule metadata to code generation.
  An error in the model is reported against the source, not as rustc errors.
- **A Node package.** `runa wasm --target nodejs` builds a CommonJS package
  that `require()` loads synchronously; `--target web` (the default) and
  `--target bundler` select the other wasm-pack targets.

## Route B: the interpreter

The library crate builds for `wasm32-unknown-unknown` without feature gates:
its filesystem, process, network and thread calls compile on that target and
fail at run time, and the calculation path calls none of them. Two changes
made the calculation path itself run:

- **Provided source files.** `futuruna::provide_source_files` registers source
  files by path. Imports, type checking and calculation calls read a
  registered path before they try the filesystem (`read_source_file`).
- **No clock unless tracing.** Calculation calls read the clock only when
  `FUTURUNA_CALCULATION_TRACE` asks for timings; WebAssembly in Node has no
  clock that Rust's `Instant` can read.

The engine is a separate crate, `research/wasm-engine`, built with
`wasm-pack build --target nodejs --release`. Its interface is one class:

```ts
class Model {
  constructor(sources: string, main: string); // { "src/x.runa": "<source>", ... }; parse and type-check
  entries(): string[];
  call(entry: string, input: string): string; // runa call input document -> output document
}
```

```sh
node wasm/interpreted.mjs <engine pkg> src/tax_report.calculate.runa tax_report cases/demo-2026.json
node wasm/interpreted.mjs <engine pkg> src/ekapital_ask.calculate.runa aktiesparekonto_report cases/ekapital-ask-2026.json
```

`call` returns the `runa call` document itself; nothing is assembled in
JavaScript. The module reserves a 64 MiB stack for the interpreter's
recursion.

## Measurements

Node 22.15 on an Apple Silicon Mac (Darwin 25.2), Rust 1.94.1, wasm-pack 0.14,
no `wasm-opt`. The host was shared and under load (load average 15 to 29), so
times varied by up to 3x between runs; the table gives the best of three to
five runs in fresh processes (`wasm/bench.mjs` in the tax service).

| | A: compiled model | B: interpreter |
|---|---|---|
| Output vs `runa call` 0.2.5, demo case (`tax_report`) | byte-identical | byte-identical |
| Output vs `runa call` 0.2.5, eKapital case (`aktiesparekonto_report`) | not built (no codec export) | byte-identical |
| Output vs `runa call` 0.2.5, synthetic 2,000-event case | byte-identical | byte-identical |
| `.wasm` | 971 KB (295 KB gzip) | 6,981 KB (2,090 KB gzip) |
| JavaScript glue | 4.8 KB | 7.7 KB |
| Load (require and instantiate) | 2–13 ms | 8–19 ms |
| Initialization (`__fut_init` / parse and check the model) | 2–6 ms | 334–450 ms |
| Demo case, 2 cases: first call / later calls | 29 ms / 8 ms | 131 ms / 108 ms |
| Synthetic 2,000-event case: first call / later calls | 176 ms / 140 ms | 1,881 ms / 1,750 ms |
| Package build | 14 s | 1.5–2.5 min after a library change |

For reference, native `runa call` 0.2.5 takes 0.09 s for the demo file and
1.35 s for the synthetic case, as a whole process. Its trace
(`FUTURUNA_CALCULATION_TRACE=1`) splits the demo call into 0.18 s of
initialization and 0.06 s of case evaluation: route B's per-call time is mostly
the initialization that `invoke_calculation_cases` repeats on every call (the
runtime rule-dispatch metadata and a fresh interpreter for the model), which a
`Model` could keep between calls.

The synthetic case is the first demo case with 1,982 more trades on the free
depot (round trips that keep cash and holdings reconciled); it returns
`Computed`.

## What does not work, or was stubbed

Route A:

- Every entry needs a hand-written export: a Futuruna JSON decoder for its
  input and an encoder for its result that mirror the calculation contract.
  The tax report has one (`codec.runa`, 637 lines); the eKapital report does
  not, so route A cannot run it today. Byte-identity with `runa call` holds
  only as long as the codec agrees with the contract's JSON encoding.
- WASM exports are limited to non-generic functions without effects whose
  signatures use primitives, strings, numeric lists and options; records cross
  the boundary as JSON strings.
- Needs (partial cases, `Unknown` values, propagation) is an interpreter
  feature and does not exist in generated Rust.

Route B:

- Host builtins have no host: `time` and `sleep` stop the call (no clock);
  `read_file`, `read_lines`, `file_exists`, `env_var`, shell commands, HTTP and
  `http_serve` fail or see nothing. The tax engine uses none of them.
- `runa verify` and Explore (Z3 processes), XLSX and TOML adapters, the
  contract cache and the compiler-validation checks of the `runa` binary are
  not part of the engine. The engine type-checks with the library's checker,
  which is what `runa call` uses for its contracts.
- Calculation cases run on one worker; the worker threads of native
  `runa call` do not exist on this target.
- A Rust panic cannot be caught on `wasm32-unknown-unknown` (panics abort).
  Native `runa call` turns a panic inside one case into that case's
  diagnostic and continues; in WebAssembly the call traps, the panic message
  goes to `console.error`, and the instance must be discarded. Both routes
  share this; isolating a crash per call means instantiating the module again
  after a trap.
- Paths are not canonicalized (there is no filesystem), so a file imported
  through two different relative spellings (`./x` and `../src/x`) would be
  loaded twice. The tax engine imports every file as `./name` from `src/`.
- Provided source files are process-wide; `Model::call` registers its own
  files again before each call, so several models can share one module
  instance.

## Recommendation for D1

**Run the interpreter (route B) in WebAssembly.**

- It is the route that serves M5: Needs is built in the interpreter
  (`feature/needs`), and route B carries it to Node without a second
  implementation in code generation.
- It runs every calculation entry of the model from its sources, and returns
  `runa call`'s own output document, with the contract's encoding. Route A
  needs a Futuruna codec per entry that must be kept equal to the contract by
  hand, and has none for the eKapital report that M4 files.
- One engine package serves every model version: the npm package ships the
  engine once and the model sources with their contract hash, which is the
  package M2 describes.
- Its costs fit the workloads: 0.3–0.5 s to load a model once per process,
  about 0.1 s per demo call and under 2 s for a 2,000-event year, for a
  year-end run per account and an interview that waits on a person.
  Keeping the runtime initialization in `Model` between calls is the first
  optimization, before any comparison with route A's 8 ms.

Route A stays usable: the code generation defects it exposed are fixed, so a
compiled export remains an option where throughput matters more than Needs,
and a second, independent implementation to compare against the interpreter
on the same cases.
