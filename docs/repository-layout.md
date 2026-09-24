# Repository layout

Futuruna keeps the compiler, its verification fixtures, legal research models,
website, and supporting research in one repository.

| Location | Purpose |
| --- | --- |
| `src/` | Compiler, runtime, proof system, and command-line implementation |
| `std/` | Futuruna standard-library source |
| `tests/` | Regression tests, authored canaries, fixtures, and reviewed expectations |
| `examples/` | Language examples, application demonstrations, and legal corpora; see the [index](../examples/README.md) |
| `docs/` | Maintained contracts, reference documentation, design records, and historical plans; see the [index](README.md) |
| `research/` | Reproducible syntax experiments, datasets, and exploratory research |
| `paper/` | Paper source and intentionally retained publication PDF |
| `website/` | Website source, static assets, and build/deployment scripts |
| `editors/` | Editor integrations |
| `scripts/` | Development and verification commands |
| `wiki/` | Knowledge notes and their supporting workflows, templates, and attachments |
| `.agents/skills/futuruna/` | Canonical Futuruna authoring/setup skill; `.claude/` delegates to it |
| `.github/` | CI, release workflows, and repository presentation assets |
| `.obsidian/` | Shared configuration for opening the repository root as a vault |

## Source and generated output

Keep source code, legal provenance, research datasets, lockfiles, reviewed
expectation files, and selected publication artifacts in Git. A file being
machine-readable or generated originally does not make it disposable.

Build output belongs in ignored directories such as `target/`, `.runa-build/`,
or `outputs/`. Runtime databases and their sidecars are ignored. Private case
inputs and results belong outside the checkout.

`runa build` currently places its final executable in the working directory.
When experimenting locally, build from an ignored working directory:

```sh
mkdir -p outputs/local-builds
cd outputs/local-builds
runa build ../../examples/tutorial_tax.scenario.runa
```

Run `python3 scripts/repository-hygiene.py` from anywhere in the checkout to
check tracked native binaries/runtime files, repository navigation, wiki note
links, and literal Rust include paths. This is a local reference check, not a
network crawler or a replacement for compiler and website test lanes.

## Moving material without losing references

The [migration manifest](repository-migrations.json) records the September 2026
folder moves and removed build artifacts. It includes the original commit,
Git blob IDs, and SHA-256 hashes, so the old artifacts remain recoverable without
keeping executable copies at the repository root. No Git history was rewritten.

Use `git log --follow -- path/to/file` to inspect a moved file's history. To
inspect an old file, use `git show COMMIT:OLD_PATH` with the commit and path in
the manifest. To recover a binary, redirect that output to a file outside the
checkout and compare its hash; do not execute it just to identify it.

Update Markdown links, wiki links, program imports, scripts, and website
includes with each move. Wiki links to names that also occur in `docs/` use
explicit paths to distinguish summaries from source contracts.

Legal corpus paths and public website URLs remain stable. In particular,
`examples/danish-constitution-legacy/` remains an input to the mint gate.
