# Set Up Futuruna With Your AI

Futuruna comes with AI integration as a first-class integration.

I recommend using [Claude Code](https://claude.com/product/claude-code), [Claude Cowork](https://claude.com/product/cowork), [Codex](https://openai.com/codex/), or [ChatGPT Work](https://openai.com/chatgpt-work/) for the AI setup.

If you are a person, give your AI this instruction:

> Read https://futuruna.com/ai-setup.md and set up Futuruna for me.

If you are the AI, follow the guide below. Adapt commands to the user's operating system and environment, explain material choices, and do the work with the user instead of only describing it.

## Setup at a glance

- **Setup waiting time:** Allow 5–10 minutes on a modern computer with Rust already installed.
- **Approval steps:** Up to 3 with a tight AI policy: clone the source, download dependencies and build, then run the newly built compiler. Add one approval if Rust must be installed.
- **Futuruna space requirements:** About 300 MB after a clean build, excluding Rust. If Rust is not installed, allow about 1.7 GB total.
- **Verification:** Print the Futuruna version and interpret one known local example. This is a smoke check, not hash verification, a full test suite, or a project-wide audit.

These estimates come from a cold setup dry run on an Apple A18 Pro with 8 GB memory and an empty Cargo cache: cloning took 6 seconds, the release build took 3 minutes 46 seconds, both smoke checks took less than 1 second, and the checkout, dependencies, and build output occupied about 289 MiB. Network speed, computer performance, operating system, and toolchain installation can change the result.

## Your task

1. Get Futuruna from the canonical repository.
2. Build the `runa` compiler.
3. Verify that the compiler and a known example work.
4. Ask the user which first project they want to explore.
5. Help them complete that project without guessing facts or silently changing source material.

## Set up Futuruna

### 1. Inspect the environment

- Ask where the user wants Futuruna installed. Do not overwrite an existing directory.
- If a Futuruna checkout already exists, inspect its remote and working-tree status before changing it. Never discard local work.
- Check for Git, Rust, and Cargo with `git --version`, `rustc --version`, and `cargo --version`.
- If Rust is missing, explain that Futuruna requires the Rust toolchain and use the official instructions at https://rustup.rs. Ask before installing system software, using elevated privileges, or changing a shell profile.

### 2. Get the source

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

### 3. Build and verify the compiler

From the Futuruna repository root, run:

```
cargo build --locked --release --bin runa
./target/release/runa --version
./target/release/runa examples/weather_demo.runa
```

On Windows, use the corresponding `runa.exe` path. If a command fails, diagnose that failure before continuing. Do not claim the setup is complete until the version command and weather example both succeed. Do not run the full Futuruna test suite as part of this setup.

When setup succeeds, tell the user:

- where Futuruna was installed,
- which version was built,
- which verification commands passed, and
- that the compiler is available at `target/release/runa` inside the checkout.

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
./target/release/runa schema examples/danish-income-tax/personskat.calculate.runa --entry beregn_personskat --output PRIVATE_WORK_DIR/personskat-schema.json
./target/release/runa template examples/danish-income-tax/personskat.calculate.runa --entry beregn_personskat --format xlsx --output PRIVATE_WORK_DIR/personskat-cases.xlsx
```

Use the field labels, questions, help, units, choices, and source traces in the generated contract to interview the user. Record only facts the user can support. Keep a list of unknown, ambiguous, and unsupported fields instead of filling them speculatively.

When the workbook is complete, run:

```
./target/release/runa call examples/danish-income-tax/personskat.calculate.runa --entry beregn_personskat --input PRIVATE_WORK_DIR/personskat-cases.xlsx --output PRIVATE_WORK_DIR/personskat-results.xlsx
```

Help the user compare the result with the Annual Tax Report, trace differences back to inputs and rules, and report uncertainties clearly. `schema`, `template`, and `call` are Preview features, and this tax model remains an active research project.

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
- Language style: https://github.com/Futuruna/futuruna/blob/main/docs/reference/style.md
- Feature stages: https://github.com/Futuruna/futuruna/blob/main/docs/feature-stages.md
