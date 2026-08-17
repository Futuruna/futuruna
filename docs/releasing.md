# Releasing Futuruna

This is the maintainer runbook for a public Futuruna release. A version is not
released merely because `Cargo.toml` contains that version. The GitHub release,
crates.io package, binary checksums, and signed macOS artifacts must all exist.

## Release Outputs

The tag workflow builds and executes `runa` natively on four GitHub-hosted
runners:

| Target | Release asset | Runner |
| --- | --- | --- |
| `x86_64-unknown-linux-gnu` | `runa-linux-x86_64` | `ubuntu-22.04` |
| `aarch64-unknown-linux-gnu` | `runa-linux-arm64` | `ubuntu-22.04-arm` |
| `aarch64-apple-darwin` | `runa-macos-arm64` | `macos-15` |
| `x86_64-apple-darwin` | `runa-macos-x86_64` | `macos-15-intel` |

Every build uses the declared minimum Rust version 1.94.0, `Cargo.lock`, release
symbol stripping, the weather example, and the stable first-run canary. The
macOS binaries are then signed with a Developer ID Application certificate and
submitted to Apple's notary service. The release job publishes the four
binaries and a generated `SHA256SUMS` file.

The crates.io package is built from the explicit `Cargo.toml` `include` list. It
contains the compiler sources, README, license, and compile-time feature-stage
metadata, rather than the website, wiki, research corpus, examples, or tests.
Plain `cargo install futuruna --locked` installs only `runa`; developer
adversarial binaries require the `internal-tools` feature.

## Required Credentials

Do not push a release tag until these GitHub Actions secrets exist:

| Secret | Scope | Purpose |
| --- | --- | --- |
| `MACOS_CERTIFICATE_P12_BASE64` | `release` environment | Base64-encoded Developer ID Application certificate and private key |
| `MACOS_CERTIFICATE_PASSWORD` | `release` environment | Password protecting the PKCS#12 file |
| `APPLE_ID` | `release` environment | Apple account used by `notarytool` |
| `APPLE_TEAM_ID` | `release` environment | Apple Developer team identifier |
| `APPLE_APP_SPECIFIC_PASSWORD` | `release` environment | App-specific password used by `notarytool` |
| `CARGO_REGISTRY_TOKEN` | `release` environment | crates.io token for the first publication |

The workflow fails rather than publishing an unsigned macOS binary or silently
skipping crates.io. Never print, commit, or put these values in workflow inputs.

crates.io trusted publishing can replace the long-lived registry token after
the first version exists. The first package publication still requires a token;
configure the trusted GitHub publisher immediately after that publication.

## Local Preflight

Run focused packaging and installation checks from a clean release commit:

```bash
cargo fmt --all -- --check
cargo build --locked --release --bin runa
./target/release/runa --version
./target/release/runa examples/weather_demo.runa
RUNA_BIN="$PWD/target/release/runa" ./scripts/first-run-canary.sh
cargo publish --locked --dry-run
```

Inspect the package rather than trusting its compressed size alone:

```bash
cargo package --locked --list
ls -lh target/package/futuruna-*.crate
```

The package must not contain private workspace state, the website, wiki,
research material, examples, tests, annual-assessment files, or generated case
data.

## Workflow Dry Run

Run the `Release` workflow manually from the intended release commit before
creating a tag. Manual dispatch performs package validation and all four native
builds, including macOS signing and notarization, but does not publish to
crates.io or create a GitHub release.

Review all four build logs. Download the workflow artifacts and verify that:

- every binary reports the expected version;
- the Linux binaries have the expected ELF architecture;
- the macOS binaries have the expected Mach-O architecture;
- `codesign --verify --strict` succeeds for both macOS binaries;
- Apple's notarization result is `Accepted`.

## Create the Release

Confirm that `Cargo.toml`, `Cargo.lock`, `CITATION.cff`, CodeMeta, the
compatibility guide, and release notes agree on the version. Use an annotated or
signed tag when the maintainer's signing setup is available:

```bash
git tag -s v0.1.0 -m "Futuruna 0.1.0"
git push origin v0.1.0
```

If signed tags are not configured, stop and configure them instead of silently
substituting a lightweight tag. The workflow rejects a tag whose version does
not exactly match `Cargo.toml`.

The tag workflow publishes the crate only after all binary jobs pass, then
creates the GitHub release. A rerun detects an already published crate version
and does not attempt to overwrite it.

## Post-Release Verification

From clean Linux x86-64, Linux ARM64, Apple Silicon, and Intel macOS machines:

1. Download the matching release asset and `SHA256SUMS`.
2. Verify the checksum before renaming or executing the binary.
3. Run `runa --version` and the weather example.
4. On macOS, confirm Gatekeeper accepts the downloaded binary without an
   `xattr` bypass.
5. Run `cargo install futuruna --version 0.1.0 --locked` in an isolated Cargo
   home and repeat the smoke checks.
6. Confirm `https://futuruna.com/ai-setup.md` completes successfully from a
   clean agent session.

After the first crates.io publication, configure its trusted publisher for this
repository. After the public archive is minted, update citation and CodeMeta
records with the real DOI; never predeclare a release date or identifier.
