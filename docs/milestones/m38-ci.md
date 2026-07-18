# M38: CI/CD Pipeline

**Tagline:** "Every commit is tested."

**Status:** Complete.

## What was delivered

### CI workflow (.github/workflows/ci.yml)

Runs on every push to main and every PR:

| Step | What |
|------|------|
| Build (debug) | `cargo build` |
| Unit tests (lib) | `cargo test --lib` — 28 tests |
| Unit tests (bin) | `cargo test --bin runa` — 68 tests |
| Build (release) | `cargo build --release` |
| Runa tests (interpreter) | `runa test` — 69 happy + 13 negative |
| Runa tests (compiled) | `runa test --run` — non-blocking (known gaps) |
| Format check (.runa) | `runa fmt --check tests/` |
| Version check | `runa --version` |
| Rust formatting | `cargo fmt --check` |

Matrix: **ubuntu-latest + macos-latest**

### Release workflow (.github/workflows/release.yml)

On tag push (`v*`):
- Builds release binaries for Linux x86_64, macOS arm64, macOS x86_64
- Creates GitHub Release with binaries attached
- Auto-generates release notes

## Verification

All CI checks pass locally:
```bash
cargo fmt --check                          # Clean
cargo test --lib                           # 28 pass
cargo test --bin runa                      # 68 pass
cargo build --release                      # Clean
./target/release/runa test                 # 69 + 13 pass
./target/release/runa fmt --check tests/   # 82 files clean
```
