# Set Up Futuruna With Your AI

This guide lets an AI coding assistant install Futuruna on your computer, check
that it works, and help you begin a first project. Use
[Claude Code](https://claude.com/product/claude-code),
[Claude Cowork](https://claude.com/product/cowork),
[Codex](https://openai.com/codex/), or
[ChatGPT Work](https://openai.com/chatgpt-work/) to follow it.

If you are a person, give your AI this instruction:

> Read https://futuruna.com/ai-setup.md and set up Futuruna for me.

If you are the AI, follow the guide below. Adapt commands to the user's operating system and environment, explain material choices, and do the work with the user instead of only describing it.

## Setup at a glance

- **Installation:** Use the verified Futuruna download made for the user's computer when one is available. Otherwise, install it with Cargo or build it on that computer.
- **Time:** A download normally takes 1–3 minutes. Building Futuruna normally takes 5–10 minutes when Rust and Cargo are already installed.
- **Rust:** The downloaded program can check, format, run, and audit Futuruna code without installing Rust. Building Futuruna or using `runa build` still requires Rust.
- **Final check:** Print the Futuruna version and run a known example on the computer where the user will work. Do not run the full project test suite merely to check an installation.

## Your task

1. Establish which operating system and processor will actually run `runa`.
2. Get the Futuruna repository when the user wants the examples and legal models.
3. Prefer the verified download for that computer; otherwise use crates.io or build Futuruna there.
4. Verify Futuruna on the user's computer.
5. Ask the user which first project they want to explore.
6. Help them complete that project without guessing facts or silently changing source material.

## Set up Futuruna

### 1. Check the user's computer

First establish which computer will run Futuruna. An AI sandbox, desktop bridge,
remote container, and the user's computer may have different operating systems
and processors. Never install a program built for the AI's computer and present
it as an installation for the user's computer.

Run these commands **on the user's computer** when possible:

```
uname -s
uname -m
```

If you cannot execute commands on that computer, ask the user for its
operating system and architecture. State clearly which steps you can perform
remotely and which checks must still run on their computer.

Ask where the user wants Futuruna installed. Do not overwrite an existing
directory. If a Futuruna checkout already exists, inspect its remote and
working-tree status before changing it. Never discard local work.

### 2. Get the examples and legal models

If Futuruna is not already present, clone it from the canonical repository:

```
git clone https://github.com/Futuruna/futuruna.git
cd futuruna
```

Confirm that the checkout points to the expected repository and report any local changes before continuing:

```
git remote get-url origin
git status --short --branch
```

The checkout supplies the examples, documentation, and legal models. Futuruna
itself can come from a published download; cloning the repository does not mean
you must compile it.

### 3. Install a verified download

Choose the filename for the user's operating system and processor:

| Computer | Download |
| --- | --- |
| Linux `x86_64` | `runa-linux-x86_64` |
| Linux `aarch64` or `arm64` | `runa-linux-arm64` |
| macOS `arm64` | `runa-macos-arm64` |
| macOS `x86_64` | `runa-macos-x86_64` |

From the repository root, replace `DOWNLOAD_NAME` below with that exact filename:

```
BINARY=DOWNLOAD_NAME
RELEASE_BASE=https://github.com/Futuruna/futuruna/releases/latest/download
mkdir -p target/release
curl --fail --location --retry 3 --output "target/release/$BINARY" "$RELEASE_BASE/$BINARY"
curl --fail --location --retry 3 --output target/release/SHA256SUMS "$RELEASE_BASE/SHA256SUMS"
cd target/release
if command -v sha256sum >/dev/null 2>&1; then
    grep "  $BINARY$" SHA256SUMS | sha256sum --check -
else
    grep "  $BINARY$" SHA256SUMS | shasum -a 256 --check -
fi
chmod +x "$BINARY"
mv -f "$BINARY" runa
cd ../..
```

Stop if the download or checksum is unavailable, the checksum line is missing,
or verification fails. The GitHub release page states whether its macOS binaries
are Apple-notarized. If macOS blocks an unsigned download, do not remove its
quarantine attribute or disable Gatekeeper automatically. Tell the user and use
Cargo or a local source build instead unless the user explicitly decides
otherwise.

If there is no download for the user's computer, use one of the installation
methods below instead of trying to build from an unrelated AI sandbox.

### 4. Install with Cargo

When Cargo is already available and the user approves a user-level Cargo
installation, try the crates.io source distribution:

```
cargo install futuruna --locked
runa --version
```

This installs into Cargo's configured binary directory, normally
`~/.cargo/bin`. Early releases may not yet be published there; if Cargo reports
that the package is unavailable, continue with the source build below. Do not
change `PATH` or shell profiles unless the user asks.

### 5. Build from source

Check for Rust and Cargo with `rustc --version` and `cargo --version`. If Rust is
missing, use the official instructions at https://rustup.rs and ask before
installing software or changing a shell profile. Futuruna 0.1.0 supports Rust
1.94 or newer for source and Cargo installation.

Build on the same operating system and architecture where the resulting binary
will run:

```
cargo build --locked --release --bin runa
```

Do not treat `rustup target add` or `-Z build-std` as a routine workaround from a
restricted sandbox: they require additional Rust toolchain downloads and still
need a suitable linker. Prefer the published download or build directly on the
user's computer.

On Windows, use the corresponding `runa.exe` path. Windows does not yet have a
published download.

### 6. Check the installation on the user's computer

For a release or source build in the checkout, run:

```
RUNA_BIN=./target/release/runa
"$RUNA_BIN" --version
"$RUNA_BIN" examples/weather_demo.runa
```

For a Cargo installation, set `RUNA_BIN="$(command -v runa)"` instead. Keep the
verified absolute path for the remaining commands. If a command fails, diagnose
it before continuing. Do not claim setup is complete until both commands
succeed **on the machine where Futuruna will be used**. Do not run the full
Futuruna test suite as part of setup.

When setup succeeds, tell the user:

- where Futuruna was installed,
- which computer and installation method were used,
- which version was installed,
- whether the download checksum was verified,
- whether the release stated that its macOS binary was notarized,
- which verification commands passed, and
- where the `runa` binary is located.

Do not add the compiler to a global path or edit the user's environment unless they ask you to.

## Choose a first project

Ask the user which of these they want to do first.

### Audit your Annual Tax Report (Årsopgørelse)

Suggest this if the user is from Denmark. Futuruna contains an active research implementation of the Danish personal income-tax model. The intended workflow is that you interview the user, help transcribe source facts into a generated workbook, and let Futuruna validate and calculate the result deterministically.

Before handling tax information:

- Explain that this is research software, not individual tax advice.
- Ask the user to choose a private working directory outside the Git checkout.
- Never commit or upload tax documents, generated workbooks, or personal results.
- Do not guess missing facts and do not use the official calculated result as an input.
- Futuruna does not import the Annual Tax Report PDF automatically. A person or AI must read it and transcribe the source facts.

Start by reading:

- `examples/danish-income-tax/website-overblik.md`
- `examples/danish-income-tax/personskat.calculate.runa`
- `docs/reference/calculations.md`

Then inspect the calculation contract and generate an Excel workbook. Replace `PRIVATE_WORK_DIR` with the private directory chosen by the user:

```
"$RUNA_BIN" schema examples/danish-income-tax/personskat.calculate.runa --entry beregn_personskat --output PRIVATE_WORK_DIR/personskat-schema.json
"$RUNA_BIN" template examples/danish-income-tax/personskat.calculate.runa --entry beregn_personskat --format xlsx --output PRIVATE_WORK_DIR/personskat-cases.xlsx
```

Use the field labels, questions, help, units, choices, and source traces in the generated contract to interview the user. Record only facts the user can support. Keep a list of unknown, ambiguous, and unsupported fields instead of filling them speculatively.

When the workbook is complete, run:

```
"$RUNA_BIN" call examples/danish-income-tax/personskat.calculate.runa --entry beregn_personskat --input PRIVATE_WORK_DIR/personskat-cases.xlsx --output PRIVATE_WORK_DIR/personskat-results.xlsx
```

Help the user compare the result with the Annual Tax Report, trace differences back to inputs and rules, and report uncertainties clearly. `schema`, `template`, and `call` are Preview features, and this tax model remains an active research project.

### Explore a rule model

Once a law or contract is encoded, Futuruna can turn its rules inside out. Help
the user ask for counterexamples, thresholds, income cliffs, minima, maxima, or
the worst case inside a clearly stated finite search space.

Build the exploration with Futuruna's existing language:

1. State the question, fixed facts, varied facts, metric, and units.
2. Build each finite domain with a list or end-exclusive `range`.
3. Use `map` for one dimension or nested `flat_map` for combinations.
4. Evaluate every scenario through the canonical encoded rules.
5. Prove every generated scenario is valid, or report the excluded cases.
6. Use `filter` to retain the scenarios that answer the question.
7. Use `foldl` to select a minimum, maximum, or worst case, guarding the empty
   case before using `head`.
8. Name the expected property with `|` and check it with `?`.

Report the searched domain, witness count, selected scenario, assumptions,
sources, and exact units. Call the result exhaustive over the full declared
domain only when every generated scenario is valid. Otherwise scope the result
to the valid subset and report every exclusion.

Start with the
[law-exploration workbook](https://github.com/Futuruna/futuruna/blob/main/examples/danish-income-tax/exploration-workbook.md)
and run its
[income-cliff audit](https://github.com/Futuruna/futuruna/blob/main/examples/danish-income-tax/personskat-income-cliffs.audit.runa).

### Encode a contract

Ask the user for the contract, its jurisdiction, the question they want to answer, and whether they want a one-case self-audit or a broader exploration.

Work with the user to:

1. Preserve the source contract separately and quote it accurately.
2. Identify parties, definitions, dates, obligations, permissions, exceptions, defaults, remedies, and unresolved terms.
3. Encode those concepts as typed Futuruna definitions and rules, with source references and explicit assumptions.
4. Add concrete scenarios for the user's case and boundary cases for ambiguous or conflicting terms.
5. Expose a typed `@ calculate` entry when a formal rule-model workbook would help collect facts.
6. Run `runa check` and `runa fmt --check` on the model, then execute the relevant scenarios.
7. Generate the workbook, interview the user, and complete a self-audit or exploration using the formal rule model available through Futuruna.

Do not silently resolve ambiguity. Show the user where a conclusion follows from the encoded contract and where interpretation is still required.

### Encode a law

Ask for the jurisdiction, official source, version or effective date, and the question the user wants to explore.

Work with the user to:

1. Preserve the official source text and provenance.
2. Model definitions, scope, conditions, exceptions, transitions, decisions, and effects explicitly.
3. Keep source-backed legal rules separate from assumptions or interpretations.
4. Add scenarios for ordinary cases and audits for gaps, tensions, loopholes, missing definitions, and unusual rule interactions.
5. Expose a typed `@ calculate` entry when the law can be explored through structured case facts.
6. Check and format the model, run its scenarios, and complete a self-audit or exploration using the formal rule model available through Futuruna.

State the model's coverage and limitations. Futuruna can make the encoded reasoning deterministic and auditable; it does not make an incomplete legal model complete.

## Working rules for the AI

- Ask before installing software, changing global configuration, or publishing anything.
- Preserve the user's wording and source material. Make interpretations explicit.
- Never invent legal, contractual, tax, or personal facts.
- Keep private documents and generated case files outside the repository unless the user explicitly asks for a sanitized fixture.
- Use AI to interview, organize facts, explain results, and help write models. Use Futuruna to validate and calculate the formal rule model.
- Treat Preview and Experimental features honestly. `schema`, `template`, and `call` are Preview; `audit` is Experimental.
- Finish by reporting commands run, files created or changed, checks performed, results, and remaining uncertainties.

## Useful references

- Repository: https://github.com/Futuruna/futuruna
- Website: https://futuruna.com
- First-run contract: https://github.com/Futuruna/futuruna/blob/main/docs/first-run-contract.md
- Calculation workbooks: https://github.com/Futuruna/futuruna/blob/main/docs/reference/calculations.md
- Law-exploration workbook: https://github.com/Futuruna/futuruna/blob/main/examples/danish-income-tax/exploration-workbook.md
- Language style: https://github.com/Futuruna/futuruna/blob/main/docs/reference/style.md
- Feature stages: https://github.com/Futuruna/futuruna/blob/main/docs/feature-stages.md
