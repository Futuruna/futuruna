"""Exercise the CI guard with representative repository mistakes."""

import importlib.util
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location(
    "repository_hygiene", Path(__file__).resolve().parents[1] / "repository-hygiene.py"
)
HYGIENE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(HYGIENE)


class RepositoryHygieneTests(unittest.TestCase):
    def check_files(self, contents):
        with tempfile.TemporaryDirectory(prefix="futuruna-hygiene-test-") as directory:
            root = Path(directory)
            for name, data in contents.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data.encode() if isinstance(data, str) else data)
            return HYGIENE.validate(root, set(contents))

    def test_extensionless_compiled_programs_are_rejected(self):
        errors = self.check_files({"accidental-program": b"\xcf\xfa\xed\xfe\0\0"})
        self.assertTrue(any("compiled native artifact" in error for error in errors))

    def test_empty_runtime_sidecars_are_rejected(self):
        errors = self.check_files({".store.db-wal": b""})
        self.assertTrue(any("runtime database" in error for error in errors))

    def test_sources_lockfiles_goldens_and_publication_pdf_are_allowed(self):
        self.assertEqual([], self.check_files({
            "scripts/demo.sh": "#!/bin/sh\necho ok\n",
            "Cargo.lock": "version = 4\n",
            "tests/expect/example.stderr": "expected diagnostic\n",
            "paper/paper.pdf": b"%PDF-1.7\n",
        }))

    def test_real_markdown_links_are_checked_but_examples_are_not(self):
        errors = self.check_files({
            "README.md": "[Missing](docs/missing.md)\n`[Example](fake.md)`\n```md\n[Example](fake.md)\n```\n"
        })
        self.assertEqual(1, len(errors))
        self.assertIn("docs/missing.md", errors[0])

    def test_ambiguous_wiki_reference_requires_a_path(self):
        files = {"docs/policy.md": "source", "wiki/sources/policy.md": "summary"}
        self.assertTrue(self.check_files({**files, "wiki/index.md": "[[policy]]"}))
        self.assertEqual([], self.check_files({
            **files, "wiki/index.md": "[[wiki/sources/policy|Policy summary]]"
        }))

    def test_literal_rust_include_must_resolve(self):
        self.assertTrue(self.check_files({"src/lib.rs": 'include_str!("../docs/missing.md");'}))
        self.assertEqual([], self.check_files({
            "src/lib.rs": 'include_str!("../docs/source.md");', "docs/source.md": "source"
        }))

    def test_wiki_source_provenance_must_survive_moves(self):
        files = {"wiki/sources/record.md": '---\nsource_paths:\n  - "docs/original.md"\n---\nSummary'}
        self.assertTrue(self.check_files(files))
        self.assertEqual([], self.check_files({**files, "docs/original.md": "Source"}))

    def test_recorded_migration_destination_must_exist(self):
        files = {"docs/repository-migrations.json":
                 '{"moves":{"old":"docs/new.md"},"removed_artifacts":[]}'}
        self.assertTrue(self.check_files(files))
        self.assertEqual([], self.check_files({**files, "docs/new.md": "Moved record"}))


if __name__ == "__main__":
    unittest.main()
