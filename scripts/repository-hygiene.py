#!/usr/bin/env python3
"""Check repository artifacts and local references without executing programs."""

from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path
from urllib.parse import unquote, urlsplit


NATIVE_MAGIC = {
    b"\x7fELF", b"\xfe\xed\xfa\xce", b"\xce\xfa\xed\xfe",
    b"\xfe\xed\xfa\xcf", b"\xcf\xfa\xed\xfe",
    b"\xca\xfe\xba\xbe", b"\xbe\xba\xfe\xca",
    b"\xca\xfe\xba\xbf", b"\xbf\xba\xfe\xca",
}
WIKI_SUPPORT = ("wiki/skills/", "wiki/_templates/", "wiki/.raw/", "wiki/_attachments/")


def prose(text: str) -> str:
    """Omit fenced examples and inline code, preserving line numbers."""
    lines = []
    fence = None
    for line in text.splitlines():
        match = re.match(r"\s*(`{3,}|~{3,})", line)
        if match:
            marker = match.group(1)
            if fence is None:
                fence = marker
            elif marker[0] == fence[0] and len(marker) >= len(fence):
                fence = None
            lines.append("")
        elif fence:
            lines.append("")
        else:
            lines.append(re.sub(r"(`+).*?\1", "", line))
    return "\n".join(lines)


def wiki_candidates(target: str, files: set[str]) -> list[str]:
    if not target:
        return []
    name = target if Path(target).suffix else target + ".md"
    if name in files:
        return [name]
    return sorted(path for path in files if path.endswith("/" + name))


def validate(root: Path, files: set[str]) -> list[str]:
    errors = []
    for name in sorted(files):
        path = root / name
        if not path.is_file():
            errors.append(f"{name}: tracked file is missing")
            continue
        with path.open("rb") as stream:
            header = stream.read(16)
        if header[:4] in NATIVE_MAGIC or header.startswith(b"MZ"):
            errors.append(f"{name}: compiled native artifact belongs outside Git")
        if name.endswith((".db", ".db-shm", ".db-wal")) or header.startswith(b"SQLite format 3"):
            errors.append(f"{name}: runtime database/sidecar belongs outside Git")

        if path.suffix == ".rs":
            text = path.read_text()
            for match in re.finditer(r'include_(?:str|bytes)!\s*\(\s*"([^"\n]+)"', text):
                if not (path.parent / match.group(1)).is_file():
                    errors.append(f"{name}: missing Rust include {match.group(1)}")

        if path.suffix != ".md" or name.startswith(WIKI_SUPPORT) or name == "wiki/workflows.md":
            continue
        text = prose(path.read_text())
        if name.startswith("wiki/") and text.startswith("---\n"):
            frontmatter = text.split("---", 2)[1]
            source_list = False
            for line in frontmatter.splitlines():
                if line.startswith("source_paths:"):
                    source_list = True
                    continue
                if line.startswith("source_path:"):
                    source = line.split(":", 1)[1].strip().strip("\"'")
                elif source_list and line.startswith("  - "):
                    source = line[4:].strip().strip("\"'")
                else:
                    source_list = False
                    continue
                if not (root / source).exists():
                    errors.append(f"{name}: missing source provenance {source}")
        for number, line in enumerate(text.splitlines(), 1):
            for match in re.finditer(r"\[[^\]\n]*\]\(([^)\n]+)\)", line):
                target = match.group(1).strip().split(' "', 1)[0].strip("<>")
                if target.startswith(("/", "#")) or urlsplit(target).scheme:
                    continue
                local = unquote(urlsplit(target).path)
                if local and not (path.parent / local).exists():
                    errors.append(f"{name}:{number}: missing Markdown target {local}")
            if name.startswith("wiki/"):
                for match in re.finditer(r"\[\[([^\]\n]+)\]\]", line):
                    target = match.group(1).split("|", 1)[0].split("#", 1)[0]
                    if not target:
                        continue
                    candidates = wiki_candidates(target, files)
                    if len(candidates) != 1:
                        reason = "ambiguous" if candidates else "missing"
                        errors.append(f"{name}:{number}: {reason} wiki target {target}")

    manifest = root / "docs/repository-migrations.json"
    if manifest.is_file():
        data = json.loads(manifest.read_text())
        for old, new in data["moves"].items():
            if any(path == old or path.startswith(old + "/") for path in files):
                errors.append(f"{old}: retired path returned; use {new}")
            if not any(path == new or path.startswith(new + "/") for path in files):
                errors.append(f"{new}: migration destination is missing")
        for artifact in data["removed_artifacts"]:
            if artifact["path"] in files:
                errors.append(f"{artifact['path']}: removed build artifact returned")
    return errors


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    # Include non-ignored additions during local development; CI sees the
    # committed tree. Never inspect ignored local case records or build caches.
    output = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=root
    )
    files = set(output.decode().rstrip("\0").split("\0")) - {""}
    errors = validate(root, files)
    if errors:
        print("Repository hygiene failed:\n" + "\n".join(errors))
        return 1
    print(f"Repository hygiene passed: {len(files)} files; artifacts and local references checked.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
