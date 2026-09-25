# Set Up Futuruna With Your AI

This guide lets an AI coding assistant install Futuruna on your computer, check
that it works, and help you begin a first project. Use the
[Claude app](https://claude.com/download/) or
[ChatGPT app](https://chatgpt.com/download/), or work from a terminal with
[Claude Code](https://code.claude.com/docs/en/quickstart) or the
[Codex CLI](https://learn.chatgpt.com/docs/codex/cli).

If you are a person, give your AI this instruction:

> Read https://futuruna.com/ai-setup.md and set up Futuruna for me.

If you are the AI, follow the guide below. Adapt commands to the user's operating system and environment, explain material choices, and do the work with the user instead of only describing it.

## Setup at a glance

- **Installation:** Use the verified Futuruna download made for the user's computer when one is available. Otherwise, install it with Cargo or build it on that computer.
- **Local result:** Leave the user with a working local `runa` executable and, when they want the examples or legal models, a local Futuruna checkout they can use after this AI session ends.
- **Time:** A download normally takes 1–3 minutes. Building Futuruna normally takes 5–10 minutes when Rust and Cargo are already installed.
- **Rust:** Formatting, interpreted execution (`runa FILE.runa`), `check --frontend`, and typed calculations do not require Rust. The default `runa check`, native `runa run`, `runa build`, and building Futuruna itself require Rust. A frontend-only check does not validate the Rust backend.
- **Final check:** Print the Futuruna version and run a known example on the computer where the user will work. Before tax calculations, also run the small runtime compatibility check below. Do not run the full project test suite merely to check an installation.

**Tax-audit compatibility:** The GitHub
[`v0.2.0` binaries](https://github.com/Futuruna/futuruna/releases/tag/v0.2.0) published on
19 September 2026 predate calculation-safety fixes now present in this checkout.
They must not be used for this checkout's tax-audit workflow. A development
binary may also report `0.2.0`, so the version string alone is insufficient.
Use the [runtime compatibility check](#tax-audit-runtime-check) on the exact
binary selected. If it fails, build from this checkout or obtain a newer verified
release that passes it; do not proceed using an older binary.

## Your task

1. Establish which operating system and processor will actually run `runa`.
2. Fetch the Futuruna repository onto that computer when the user wants the examples and legal models.
3. Install a local Futuruna executable there. Prefer the verified download for that computer; otherwise use crates.io or build Futuruna locally.
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

From the repository root, replace `DOWNLOAD_NAME` below with that exact filename
and `RELEASE_TAG` with the selected tag from the
[GitHub releases](https://github.com/Futuruna/futuruna/releases).
Use one explicit tag for both files, not two independently changing `latest`
downloads. This block leaves the verified binary in a new directory and prints
its path; it does not overwrite an existing compiler. The tax-audit warning above
still applies to `v0.2.0`.

```sh
(
set -eu
BINARY=DOWNLOAD_NAME
RELEASE_TAG=RELEASE_TAG
case "$BINARY" in
    runa-linux-x86_64|runa-linux-arm64|runa-macos-arm64|runa-macos-x86_64) ;;
    *) echo "Choose the exact download filename first." >&2; exit 1 ;;
esac
case "$RELEASE_TAG" in
    v[0-9]*) ;;
    *) echo "Choose the release tag first." >&2; exit 1 ;;
esac
RELEASE_BASE="https://github.com/Futuruna/futuruna/releases/download/$RELEASE_TAG"
mkdir -p target
DOWNLOAD_DIR="$(mktemp -d "$PWD/target/runa-download.XXXXXX")"
curl --fail --location --retry 3 --output "$DOWNLOAD_DIR/$BINARY" "$RELEASE_BASE/$BINARY"
curl --fail --location --retry 3 --output "$DOWNLOAD_DIR/SHA256SUMS" "$RELEASE_BASE/SHA256SUMS"
cd "$DOWNLOAD_DIR"
awk -v binary="$BINARY" '$2 == binary { print; matches++ } END { if (matches != 1) exit 1 }' SHA256SUMS > selected.sha256
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum --check selected.sha256
else
    shasum -a 256 --check selected.sha256
fi
chmod +x "$BINARY"
printf 'Verified binary: %s/%s\n' "$DOWNLOAD_DIR" "$BINARY"
)
```

Stop if the download or checksum is unavailable, the checksum line is missing,
or verification fails. If the operating system blocks the downloaded program,
show the user the exact message and ask before changing any security setting.
Use Cargo or a local source build when that is the safer available route.

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
`~/.cargo/bin`. If Cargo cannot reach crates.io or the installation fails,
diagnose the error before continuing with the source build below. Do not change
`PATH` or shell profiles unless the user asks.

### 5. Build from source

Check for Rust and Cargo with `rustc --version` and `cargo --version`. If Rust is
missing, use the official instructions at https://rustup.rs and ask before
installing software or changing a shell profile. Futuruna 0.2.0 supports Rust
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

Set `RUNA_BIN` to the absolute verified download path printed above, or to
`$PWD/target/release/runa` for a source build. For a Cargo installation, use
`RUNA_BIN="$(command -v runa)"`. Then, from the checkout:

```sh
RUNA_BIN=/absolute/path/to/the/verified/binary
"$RUNA_BIN" --version
"$RUNA_BIN" examples/weather_demo.runa
```

Keep the verified absolute path for the remaining commands. If a command fails,
diagnose it before continuing. Do not claim setup is complete until both commands
succeed **on the machine where Futuruna will be used**. Do not run the full
Futuruna test suite as part of setup.

When setup succeeds, tell the user:

- where Futuruna was installed,
- which computer and installation method were used,
- which version was installed,
- whether the download checksum was verified,
- which verification commands passed, and
- where the `runa` binary is located.

Do not add the compiler to a global path or edit the user's environment unless they ask you to.

### Tax-audit runtime check

Before pension scenarios, deduction comparisons or report reconciliation, run
this from the **same checkout as the models**, using the selected absolute binary
path:

```sh
RUNA_BIN="$RUNA_BIN" bash scripts/tax-audit-preflight.sh
```

It runs nine tiny synthetic checks: valid arithmetic and inline-module results,
rejection of division by zero and undefined scalar/list rules, rejection of
duplicate JSON members in both orders, and successful/failing assertions with
explanatory messages. It needs Bash and standard shell tools,
not Rust, Node, private documents or the full test suite. It retains a small
temporary evidence directory and neither installs nor rebuilds anything.

If it fails, stop before generating or evaluating personal cases. Inspect the
named synthetic evidence; do not call the result a tax discrepancy. The source
build above is the fallback when a compatible download is unavailable. Preserve
an existing working compiler and ask before replacing it or installing Rust.
Re-run this check after changing the selected compiler or model checkout.

A pass establishes only these runtime behaviors, not complete model compatibility,
correct tax law, true source documents or complete facts. Continue checking the
model's validity/coverage status and input fingerprint. Generate a fresh template
from the selected model; never repair a mismatch by editing its fingerprint.

## Choose a first project

Ask the user which of these they want to do first.

### Understand pension changes or possible deductions

For Danish users, an ordinary question can be the first project: “What changes
if I pay more or less into my pension?” or “Which deductions should I check?”
Read `examples/danish-income-tax/pension-og-fradrag.md` and start with the
question, tax year and relevant facts. A PDF or full workbook is not a prerequisite
for a useful first conversation.

Distinguish pension contributions from payouts, private from employer-managed
schemes, and annual from monthly amounts. Use the canonical Futuruna model for
the calculation; do not invent a marginal tax formula. Compare a preserved
baseline with clearly labelled alternatives, keep validity and coverage warnings,
and show the contribution, allowed deduction, modeled tax change and change in
available cash separately. Unknown spouse facts must not become fixed assumed
transfers in a new scenario.

Do not interpret an empty pension payout list as confirmed absence. Complete
`lønmodtager.pension.udbetalingsoplysninger` only from reviewed facts. Follow
`pension.oplysningsstatus` to ask about prior-year payouts only when further
history can still affect the extra deduction; retain unknown history as unknown.
If the validity assessment withholds the comparison amount, do not use the raw
tax total instead. The same applies to a calculated spouse.

Review `lønmodtager.pension.atp` even when the person has no company pension.
It defaults to unknown. Employer-reported ATP includes both employee and employer
shares; do not substitute the employee's payslip deduction or invent a gross/net
amount. Use the documented source branch for public-benefit ATP, SUPP or mandatory
pension savings, and do not also enter the same payment as an ordinary pension.

For deductions, identify relevant candidates, check whether they are already
included, and name the evidence or next action needed. Do not present a candidate
as an established entitlement. The guide includes an optional fictional pension
demo; it is not a replacement for a person's facts. Ordinary JSON calculation
batches are enough for these questions; no Explore stream is needed.

### Audit your Annual Tax Report (Årsopgørelse)

Suggest this if the user is from Denmark. Futuruna contains an active research implementation of the Danish personal income-tax model. Ask the user to download their Annual Tax Report as a PDF from [SKAT](https://skat.dk/borger/aarsopgoerelse/aarsopgoerelsen), keep it private, and choose a private working directory. The intended workflow is that you read the PDF with the user, transcribe supported source facts into a generated workbook, and let Futuruna validate and calculate the result deterministically.

Before handling tax information:

- Explain that this is research software, not individual tax advice.
- Ask the user to choose a private working directory outside the Git checkout.
- Never commit or upload tax documents, generated workbooks, or personal results.
- Do not guess missing facts or use the official calculated result as an input to an independent tax calculation. A separate conditional reconciliation may use explicitly labelled report observations, but its inferred amounts must never become verified source facts.
- Futuruna does not import the Annual Tax Report PDF automatically. A person or AI must read it and transcribe the source facts.

Start by reading:

- `examples/danish-income-tax/website-overblik.md`
- `examples/danish-income-tax/personskat.calculate.runa`
- `docs/reference/calculations.md`

Choose the review route before generating a workbook. If the calculation uses
Futuruna's spouse branch, an independent calculation needs the relevant spouse
facts. A spouse's Annual Tax Report may help, but its absence must not prevent a
useful review. Do not repeatedly request unavailable documents or invent the
missing values.

When those facts are unavailable, use the separate conditional workflow in
`examples/danish-income-tax/aarsopgoerelse-afstemning.md`. Its
`afstem_årsopgørelse` entry accepts selected observations from the user's own
report and exposes the spouse transfers necessary for the income and tax totals
to agree, checked legal bounds, discrepancies, and unresolved conditions. It
also accounts for reported earlier refunds on amended assessments. This is
mechanical reconciliation, not an independently verified spouse calculation;
never describe `BetingetAfstemt` as proof that the tax report is legally correct.

Ask whether the report shows a refund or tax owed (`restskat`). The conditional
guide has a separate restskat route for the principal before interest and
percentage additions. Do not enter debt as a negative refund or describe that
principal check as verification of the amount to pay, instalments or due dates.

For an independent calculation with supported source facts, inspect the contract
and generate an Excel workbook. Replace `PRIVATE_WORK_DIR` with the private
directory chosen by the user:

```
"$RUNA_BIN" schema examples/danish-income-tax/personskat.calculate.runa --entry beregn_personskat --output PRIVATE_WORK_DIR/personskat-schema.json
"$RUNA_BIN" template examples/danish-income-tax/personskat.calculate.runa --entry beregn_personskat --format xlsx --output PRIVATE_WORK_DIR/personskat-cases.xlsx
```

Use the field labels, questions, help, units, choices, and source traces in the generated contract to interview the user. Record only facts the user can support. Keep a list of unknown, ambiguous, and unsupported fields instead of filling them speculatively.

The single-parent employment deduction requires facts about extra børnetilskud,
not a deduction copied from the tax report or an inference from marital status.
Its default is unknown; do not silently change it to no benefit received.
Read `examples/danish-income-tax/ligningsloven-par9j-enlig.md` for quarter facts
and migration of existing templates. This applies to an active spouse too;
unavailable facts still permit the separate conditional reconciliation above.

Service and handyman deductions likewise require invoice and payment facts,
including labour/material separation and any household allocation. Read
`examples/danish-income-tax/boligjob.md`; do not copy an official deduction into
another expense field or convert unknown expenses to none. A small standalone
invoice calculation is available before filling the full tax contract.

When the workbook is complete, run:

```
"$RUNA_BIN" call examples/danish-income-tax/personskat.calculate.runa --entry beregn_personskat --input PRIVATE_WORK_DIR/personskat-cases.xlsx --output PRIVATE_WORK_DIR/personskat-results.xlsx
```

Before comparing totals, read the canonical result's `vurdering` as described in
`examples/danish-income-tax/personskat-validity.md`. If its status is
`UgyldigtBeregningsgrundlag`, the comparison amount is `null`: explain the failed
input checks and treat the remaining figures as diagnostic only. Do not turn
successful CLI execution into a claim that the tax calculation is valid.
`BeregnetMedForbehold` permits comparison of the modeled portion, not a conclusion
that all relevant facts and deductions are covered. Preserve the stated coverage
qualifications and never substitute zero for a missing comparison amount.

For a local Danish summary, save canonical output as JSON (`--output
PRIVATE_WORK_DIR/personskat-results.json`) and, if Node.js 18 or newer is
already installed, run `node examples/danish-income-tax/personskat-resultat.mjs
PRIVATE_WORK_DIR/personskat-results.json`. It retains every assessment control,
caveat and case diagnostic and withholds invalid amounts. This read-only view
does not authenticate output, compare against the taxpayer's report, or replace
the detailed spouse and payment-settlement results. See the validity guide for
scope and exit codes; do not treat a zero exit code as tax approval.

Help the user trace differences back to inputs and rules, and report
uncertainties clearly. `schema`, `template`, and `call` are Preview features,
and this tax model remains an active research project.

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
