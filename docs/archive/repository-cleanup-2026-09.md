# September 2026 repository cleanup

The cleanup removes compiled/runtime residue from the tracked root and groups
supporting material into the existing examples, documentation, and wiki trees.
The [migration manifest](../repository-migrations.json) is the exact old/new
path map and recovery record for the removed artifacts.

## Preserved boundaries

- Compiler/runtime source, standard library, legal corpora, test fixtures and
  reviewed expectations retain their paths and contents.
- Research datasets, publication source/PDF, Cargo manifests and lockfiles,
  feature-stage metadata, and website source/assets are unchanged.
- The legacy Danish Constitution stays at its existing path because the mint
  gate uses it.
- All four application demonstrations emit byte-identical Rust compared with
  their original copies. Changes are formatting and location comments.
- Private local records and ongoing work in the original checkout were outside
  this cleanup. No Git history was rewritten.

## Validation

| Command or inspection | Result |
| --- | --- |
| `python3 scripts/repository-hygiene.py` | Pass: native/runtime artifacts, local Markdown/wiki links, source-provenance fields, literal Rust includes, migration destinations |
| `python3 -m unittest discover -s scripts/tests -p test_repository_hygiene.py` | Eight tests pass, including rejected artifacts and broken/ambiguous references |
| `git diff --check` | Pass |
| JSON parsing of shared Obsidian configuration and migration manifest | Pass |
| `ruby -e 'require "yaml"; ARGV.each { \|p\| YAML.load_file(p) }' .github/workflows/ci.yml wiki/meta/dashboard.base` | Pass |
| `git cat-file blob BLOB_ID`, byte count and SHA-256 comparison for every removed artifact | All eight recorded artifacts recover exactly |
| `runa emit FILE` before and after the application moves/formatting | All four Rust outputs identical |
| `runa fmt --check FILE` for each application demonstration | All four pass |
| `runa check examples/apps/link-shortener/shortener.runa` | Pass |
| `runa check examples/apps/task-tracker/tracker.runa` | Pass |
| `runa check examples/apps/inventory/inventory.runa` | Existing `E0308` failures, reproduced from the original copy |
| `runa check examples/apps/log-analyzer/analyzer.runa` | Existing `alert_level` dispatch/`E0425` failure, reproduced from the original copy |
| From `outputs/local-builds`: `runa build ../../examples/tutorial_tax.scenario.runa`, then `./tutorial_tax.scenario` | Pass: generated output stays in the ignored directory; scenario reports `100000` |

The application checks used the existing local release binary reporting
`runa 0.2.0`. The original four examples also failed formatting checks before
cleanup; they now pass. Their two existing native compilation failures are
documented in the [application index](../../examples/apps/README.md) and tracked
as `td-b9d595` for a separate semantic fix.

The full mint, differential, and website-build lanes were not rerun locally:
this cleanup changes no compiler/runtime behavior, legal models, website code,
or existing tests. Their source files and literal include targets were checked
directly, and the moved examples' emitted Rust was compared rather than assuming
that formatting preserves behavior. CI retains the existing test lanes.

The new reference check validates local file targets; it does not fetch external
URLs or check Markdown heading anchors. Interactive Obsidian behavior was not
tested; its tracked configuration and note targets were validated statically.
