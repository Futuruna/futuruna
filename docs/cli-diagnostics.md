# CLI diagnostics

`runa check model.runa` validates the frontend and the generated Rust. It
performs no host effects: `@ comptime` evaluation is pure, and Rust crates
declared with `@ depend` are downloaded and built (running their build
scripts) only with `runa check --build-deps model.runa`; without the flag such
a program fails the check with an error naming the flag.
`runa check --frontend model.runa` skips Rust generation and validation.
Both return exit code 0 on success and 1 on a reported check failure.

## Source arguments

Source commands such as `check`, `run`, `build`, `emit`, `verify`, and `audit`
require an explicit filename, including inside an initialized project:
`runa check src/main.runa`. They do not infer the entry from `runa.toml`.
A missing filename or a directory in place of a file is a usage error with
exit code 1. `check --json` reports these errors with phase `usage`.

Plain `runa` with no command still opens the REPL. Commands that work on
directories, including `fmt`, `meta`, `test`, `expect`, and `lint-library`,
retain that behavior; see `runa --help` for each command's arguments.

## Missing Rust tools

Full `check`, native `run`, and `build` require a Rust toolchain. If a required
`rustc` or `cargo` process cannot start, the command exits 1 and identifies the
tool and requested operation. The diagnostic points to Rust installation or
executable permissions as appropriate.

Discovery checks `PATH`, then `~/.cargo/bin`, and finally the legacy
`~/.rustup/toolchains/stable-aarch64-apple-darwin/bin` fallback. An installed
toolchain can therefore remain available even when it is absent from `PATH`.
This fallback does not install Rust or change your shell configuration.

For source without Rust interop, `runa check --frontend model.runa` and
`runa model.runa` provide frontend validation and interpreted execution without
Rust. A frontend success does not establish that generated Rust compiles.
Missing-tool failures in `check --json` retain phase `backend` and
`backend_validated: false`.

## JSON reports

Use `runa check --json model.runa` for tools and AI assistants. Add `--frontend`
when only frontend validation is wanted. The command writes one JSON object
to stdout; ordinary diagnostic text and success banners are suppressed.
Explicitly enabled compiler tracing can still write to stderr.

The report has these fields:

| Field | Meaning |
| --- | --- |
| `schema_version` | `1` for this report format. |
| `ok` | Whether the requested check succeeded. |
| `file` | The requested source filename. |
| `phase` | `usage`, `read`, `parse`, `frontend`, or `backend`. |
| `backend_validated` | True only after a successful full backend check, including a valid cache hit. Always false with `--frontend` or a failed check. |
| `diagnostics` | An array of diagnostic objects; empty on success. |
| `summary` | Counts of statements, functions, types, Cargo dependencies and generated Rust lines on success; null on failure. Counts can include the automatic prelude. |

Each diagnostic contains `severity`, `message`, `location`, `import_site`,
`notes`, and `context`. A location has a `file` and an optional `range` with
`start` and `end` positions. Lines and columns are one-based; columns count
Unicode scalar values, and the end position is exclusive. An unavailable
range is null. For imported source errors, `location` identifies the imported
file and `import_site` identifies the importing expression in the requested
file. Otherwise `import_site` is null.

For example, `> f(value: Intt) -> Int { 1 }` produces a frontend diagnostic
whose message contains `unknown type`, with a range beginning at line 1,
column 12. A Rust backend failure instead has phase `backend`, preserves the
backend's message, and leaves the Futuruna range null. Generated Rust
locations within that message are not a source map to the `.runa` file.

Consumers should inspect `ok`, the exit code and `backend_validated` rather
than infer validation from a particular success message. Accept additional
object fields within schema version 1; an incompatible shape requires a new
schema version.

Parser failures produce one diagnostic per recovered error, retaining their
individual ranges and hints. The editor language server uses the same source
spans and converts them to zero-based UTF-16 positions for LSP clients. CLI
JSON positions keep the one-based Unicode scalar convention above. An error
at end of file has an empty range at the actual end of the source.

## Runtime errors and terminal output

`runa file.runa` interprets a script and echoes a final non-unit result to stdout.
For example, a file ending in `= b = 1 + 1` prints `=> 2`. Native execution with
`runa run file.runa` does not echo bindings. Use explicit `@ print(...)` effects
for portable script output, or the JSON `runa call` interface for typed
calculation results consumed by tools.

Ordinary interpreted user faults such as `head([])`, failed assertions and
integer overflow stop execution with exit code 1 and a source diagnostic.
Completed effects remain completed; expressions after the fault do not run.
The location identifies the active expression. If a called function or rule
fails, it identifies the invoking expression rather than treating the
helper's source offsets as offsets in its caller. Import initialization errors
retain the imported file's source. This is not a complete runtime stack trace.

Internal Rust panics remain distinct from these expected user faults. Native
executables and arbitrary embedded Rust retain their own runtime reporting;
the interpreted source-diagnostic boundary does not map native panic sites.
Calculation and exploration entrypoints retain their existing typed failure
contracts.

CLI status and diagnostic colors are enabled only for the corresponding
terminal stream. `NO_COLOR` or `TERM=dumb` disables them. Program-authored
output and emitted source are preserved, including any intentional escape
sequences in those values.
