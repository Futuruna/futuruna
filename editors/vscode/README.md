# Futuruna editor support

The extension starts `runa lsp` for diagnostics, completion, hover and
go-to-definition. Configure `futuruna.serverPath` when the compiler is not on
your path. The compiler provides the language behavior; the TextMate grammar
provides syntax colors, including quoted source blocks and template expressions.

## Install from this checkout

Use an installed VS Code or Cursor, Node.js 22 or newer with npm, and a working
Futuruna executable. First complete the
[compiler setup](https://futuruna.com/ai-setup.md) and keep the absolute path to
the exact binary you verified. The editor does not install Futuruna for you.

From the repository root, install the extension's locked dependencies:

```sh
npm ci --ignore-scripts --prefix editors/vscode
```

This includes `vscode-languageclient`, which is required to start the language
server. Copy the resulting `editors/vscode` directory, including `node_modules`,
into the editor's local extensions directory as `futuruna.futuruna-lang-local`.
The default locations are:

| Editor | macOS / Linux | Windows |
| --- | --- | --- |
| VS Code | `~/.vscode/extensions` | `%USERPROFILE%\.vscode\extensions` |
| Cursor | `~/.cursor/extensions` | `%USERPROFILE%\.cursor\extensions` |

For example, this macOS/Linux command installs into VS Code. For Cursor, change
`.vscode` to `.cursor` before running it. It stops if that destination already
exists; inspect an existing installation before replacing it.

```sh
(
set -eu
FUTURUNA_EXTENSION_DIR="$HOME/.vscode/extensions/futuruna.futuruna-lang-local"
if [ -e "$FUTURUNA_EXTENSION_DIR" ] || [ -L "$FUTURUNA_EXTENSION_DIR" ]; then
    echo "Extension destination already exists: $FUTURUNA_EXTENSION_DIR" >&2
    exit 1
fi
mkdir -p "$(dirname "$FUTURUNA_EXTENSION_DIR")"
cp -R editors/vscode "$FUTURUNA_EXTENSION_DIR"
)
```

On Windows, copy the directory with File Explorer to the location above. Custom
profiles, portable editors and remote extension hosts may use another extension
directory. VS Code documents this
[local extension installation method](https://code.visualstudio.com/api/working-with-extensions/publishing-extension#your-extension-folder).
This is a local copy: after updating the checkout, refresh its dependencies and
replace the installed copy deliberately to receive changes.

## Select the compiler and check the editor

Restart the editor, then open **Preferences: Open Workspace Settings (JSON)**
from the Command Palette. Merge this property into the existing object, using
the absolute path you verified during setup:

```json
{
  "futuruna.serverPath": "/absolute/path/to/the/verified/runa-binary"
}
```

Use the executable itself, such as the downloaded `runa-macos-arm64` or a source
build's `target/release/runa`; do not append `lsp` or add shell quotes inside the
setting. Windows paths need JSON escaping, for example
`"C:\\tools\\futuruna\\runa.exe"`. No `PATH` change is needed. The default value
`runa` works only when the editor can already find that command.

Run **Developer: Reload Window** after changing the compiler path. Open a saved
`.runa` file and check that the language mode is **Futuruna**. The **Futuruna
Language Server** channel in the Output panel reports startup errors. In a
scratch file, try `= answer = 40 + 2`, hover `answer`, then add
`= broken = missing_name` and confirm an error appears in Problems. Remove the
deliberate error when finished. The optional **Futuruna Axes** color theme is
available through **Preferences: Color Theme**.

If startup fails, run the configured executable with `--version` in a terminal,
check that `node_modules/vscode-languageclient` was copied, and reload the
window. For an SSH/container workspace, the executable must be available on the
machine running the extension host.

## Grammar development

Run the grammar regressions from this directory:

```sh
npm ci --ignore-scripts
npm test
```

The tests run the actual VS Code TextMate tokenizer and Oniguruma engine, with
versions pinned in `package-lock.json`. They check scopes across line boundaries,
not just whether a regular expression matches a sample. The `Editor grammar` CI
job runs them independently of the Rust compiler gate. The compiler's LSP process
tests live in `tests/lsp_protocol.rs` and run through the ordinary Cargo lane.

The grammar recognizes both canonical `True`/`False` and accepted lowercase
aliases. Editor completion suggests the canonical spellings. Syntax colors
do not establish that a program parses, type-checks or evaluates successfully.
