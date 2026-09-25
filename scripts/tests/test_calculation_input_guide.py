"""Read-only contract navigation, plus opt-in fresh Personskat schema evidence."""

import contextlib
from copy import deepcopy
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("input_guide", ROOT / "scripts/calculation-input-guide.py")
GUIDE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GUIDE)


def primitive(name="Int"):
    return {"kind": "primitive", "name": name}


def named(name, **extra):
    return {"kind": "named", "name": name, **extra}


def field(name, reference):
    return {"name": name, "type": reference}


def variant(name, fields=(), positional=False):
    return {"name": name, "fields": list(fields), "positional": positional}


def fixture():
    source = {"role": "warning", "binding": "source", "text": "Unknown is not zero",
              "data": {"exact_integer": 9223372036854775807}}
    metadata = lambda path: {"path": path, "label": "Question", "question": "Which amount?",
                             "help": "Document it", "unit": "øre", "source_group": "g"}
    return {
        "schema": "futuruna.calculate.compact.v1", "schema_version": 1,
        "contract_schema": "futuruna.calculate.v1", "contract_schema_version": 1,
        "schema_hash": "0" * 64, "entry": "calculate", "input": named("Input"),
        "definitions": [
            {"name": "Input", "parameters": [], "kind": "adt", "variants": [variant("Input", [
                field("amount", primitive()), field("without_help", primitive("Bool")),
                field("choice", named("Choice")),
                field("mapping", {"kind": "map", "key": primitive("String"), "value": primitive()}),
                field("items", {"kind": "set", "item": primitive()}),
            ])]},
            {"name": "Choice", "parameters": [], "kind": "adt", "variants": [
                variant("Absent"), variant("Present", [field("details", {"kind": "optional", "item":
                    named("Box", arguments=[{"kind": "list", "item": primitive()}])})])]},
            {"name": "Box", "parameters": ["T"], "kind": "adt", "variants": [
                variant("Box", [field("value", {"kind": "type_parameter", "name": "T"})])]},
        ],
        "field_metadata": [metadata("amount"), metadata("choice.$variant"), metadata("choice.Present.details")],
        "source_objects": {"s": source}, "source_groups": {"g": ["s", "s"]},
        "metadata": [source, source, {"role": "guidance", "text": "Root guidance"}],
    }


class InputGuideTests(unittest.TestCase):
    def test_root_pagination_keeps_unannotated_fields(self):
        data = fixture()
        before = deepcopy(data)
        guide = GUIDE.InputGuide(data)
        first = guide.inspect(limit=2)
        self.assertEqual(["amount", "without_help"], [row["path"] for row in first["children"]])
        self.assertEqual([True, False], [row["has_explicit_metadata"] for row in first["children"]])
        self.assertEqual({"offset": 0, "limit": 2, "total": 5, "next_offset": 2}, first["page"])
        self.assertEqual("choice", guide.inspect(offset=2, limit=1)["children"][0]["path"])
        self.assertFalse(guide.inspect("without_help")["node"]["has_explicit_metadata"])
        self.assertEqual(before, data, "inspection never mutates its schema")

    def test_choices_do_not_choose_or_turn_payloads_into_unconditional_fields(self):
        guide = GUIDE.InputGuide(fixture())
        choices = guide.inspect("choice")
        self.assertEqual(["Absent", "Present"], [row["name"] for row in choices["children"]])
        self.assertEqual("choice.$variant", choices["discriminator_path"])
        self.assertEqual([], choices["variant_guards"])
        discriminator = guide.inspect("choice.$variant", limit=1)
        self.assertEqual([{"name": "Absent"}], discriminator["children"])
        self.assertEqual(1, discriminator["page"]["next_offset"])
        self.assertEqual([], guide.inspect("choice.Absent")["children"])

    def test_optional_collection_generic_and_ancestor_guidance_are_retained(self):
        data = fixture()
        view = GUIDE.InputGuide(data).inspect("choice.Present.details.value")
        self.assertEqual({"kind": "list", "item": primitive()}, view["node"]["type"])
        self.assertEqual([{"path": "choice.$variant", "equals": "Present"}], view["variant_guards"])
        self.assertEqual([{"path": "choice.Present.details", "kind": "optional"},
                          {"path": "choice.Present.details.value", "kind": "list"}], view["containers"])
        self.assertEqual(data["field_metadata"][1:], view["field_metadata"])
        self.assertEqual(data["source_groups"], view["source_groups"], "source order/repeats are evidence")
        self.assertEqual(data["source_objects"], view["source_objects"])
        self.assertEqual([{"path": "mapping", "kind": "map"}], GUIDE.InputGuide(data).inspect("mapping")["containers"])
        self.assertEqual([{"path": "items", "kind": "set"}], GUIDE.InputGuide(data).inspect("items")["containers"])

    def test_typo_missing_variant_and_bad_pagination_are_errors_not_empty_success(self):
        guide = GUIDE.InputGuide(fixture())
        for path in ["ammount", "choice.details", "choice.Missing", "choice.$variant.extra",
                     "choice.Present.details.value.0", ".amount", "amount.", "amount.extra"]:
            with self.subTest(path=path), self.assertRaises(ValueError):
                guide.inspect(path)
        for offset, limit in [(-1, 20), (6, 20), (0, 0), (0, 101), (True, 20)]:
            with self.assertRaises(ValueError):
                guide.inspect(offset=offset, limit=limit)

    def test_root_metadata_pagination_preserves_order_and_duplicates(self):
        data = fixture()
        guide = GUIDE.InputGuide(data)
        self.assertEqual(data["metadata"][:2], guide.inspect(root_metadata=True, limit=2)["root_metadata"])
        self.assertEqual(data["metadata"][2:], guide.inspect(root_metadata=True, offset=2)["root_metadata"])
        self.assertEqual(3, guide.inspect("amount")["root_metadata_total"])
        with self.assertRaises(ValueError):
            guide.inspect("amount", root_metadata=True)

    def test_bad_references_and_duplicate_schema_entries_fail(self):
        edits = [
            lambda s: s.update(schema="futuruna.calculate.input.v1"),
            lambda s: s.update(schema_version=True),
            lambda s: s["source_objects"].clear(),
            lambda s: s["source_groups"].clear(),
            lambda s: s["source_groups"].update(g=[]),
            lambda s: s["field_metadata"].append(s["field_metadata"][0]),
            lambda s: s["definitions"].append(s["definitions"][0]),
            lambda s: s.update(schema_hash="bad"),
        ]
        for edit in edits:
            data = fixture()
            edit(data)
            with self.subTest(edit=edit), self.assertRaises(ValueError):
                GUIDE.InputGuide(data)
        data = fixture()
        data["definitions"][1]["variants"][1]["positional"] = True
        with self.assertRaisesRegex(ValueError, "Positional"):
            GUIDE.InputGuide(data).inspect("choice.Present")

    def test_json_reader_preserves_i64_rejects_duplicates_and_nonfinite_values(self):
        with tempfile.TemporaryDirectory(prefix="futuruna-input-guide-test-") as directory:
            path = Path(directory) / "schema.json"
            path.write_text(json.dumps(fixture()), encoding="utf-8")
            self.assertEqual(fixture(), GUIDE.read_schema(path))
            invalid = [b'{"a":1,"a":2}', b'{"a":1,"\\u0061":2}', b'{"a":NaN}',
                       b'{"a":Infinity}', b'{"a":1e9999}', b'{"a":"\xff"}', b'{} trailing']
            for text in invalid:
                path.write_bytes(text)
                with self.subTest(text=text), self.assertRaises(ValueError):
                    GUIDE.read_schema(path)
            path.write_text("{}", encoding="utf-8")
            with patch.object(GUIDE, "MAX_INPUT_BYTES", 1), self.assertRaises(ValueError):
                GUIDE.read_schema(path)

    def test_cli_emits_no_partial_success_on_error_or_output_limit(self):
        with tempfile.TemporaryDirectory(prefix="futuruna-input-guide-cli-") as directory:
            path = Path(directory) / "schema.json"
            path.write_text(json.dumps(fixture()), encoding="utf-8")
            for args, budget in [([str(path), "missing"], GUIDE.MAX_OUTPUT_BYTES), ([str(path)], 10)]:
                out, err = io.StringIO(), io.StringIO()
                with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err), \
                        patch.object(GUIDE, "MAX_OUTPUT_BYTES", budget):
                    self.assertEqual(2, GUIDE.main(args))
                self.assertEqual("", out.getvalue())
                self.assertIn("Input guide:", err.getvalue())
            result = subprocess.run([sys.executable, str(ROOT / "scripts/calculation-input-guide.py"),
                                     str(path), "amount"], capture_output=True, text=True, timeout=10)
            self.assertEqual(0, result.returncode, result.stderr)
            self.assertEqual(9223372036854775807,
                             json.loads(result.stdout)["source_objects"]["s"]["data"]["exact_integer"])

    @unittest.skipUnless(os.environ.get("FUTURUNA_MODEL_TEST_RUNA"), "Set FUTURUNA_MODEL_TEST_RUNA; no build/network")
    def test_actual_personskat_schema_and_sources_are_not_reauthored(self):
        with tempfile.TemporaryDirectory(prefix="futuruna-input-guide-model-") as directory:
            path = Path(directory) / "schema.json"
            result = subprocess.run([os.environ["FUTURUNA_MODEL_TEST_RUNA"], "schema",
                "examples/danish-income-tax/personskat.calculate.runa", "--entry", "beregn_personskat",
                "--format", "compact-json", "--output", str(path)], cwd=ROOT, capture_output=True,
                text=True, timeout=180, env={**os.environ, "FUTURUNA_CALCULATION_JOBS": "1"})
            self.assertEqual(0, result.returncode, result.stderr)
            schema = GUIDE.read_schema(path)
            guide = GUIDE.InputGuide(schema)
            for target in guide.fields:
                with self.subTest(metadata_path=target):
                    self.assertEqual(target, guide.locate(target)[0]["path"])
            sizes = {}
            for prefix in ["", "ægtefælle.MedÆgtefælle.fakta."]:
                target = prefix + "lønmodtager.bruttoløn_kroner"
                view = guide.inspect(target)
                self.assertEqual(primitive(), view["node"]["type"])
                actual = next(row for row in view["field_metadata"] if row["path"] == target)
                self.assertEqual(guide.fields[target], actual)
                self.assertIn("ATP", actual["question"])
                for group, ids in view["source_groups"].items():
                    self.assertEqual(schema["source_groups"][group], ids)
                for key, source in view["source_objects"].items():
                    self.assertEqual(schema["source_objects"][key], source)
                self.assertEqual(bool(prefix), bool(view["variant_guards"]))
                sizes[target] = len(json.dumps(view, ensure_ascii=True, indent=2).encode())
                self.assertLess(sizes[target], path.stat().st_size // 20)
            pension = guide.inspect("lønmodtager.pension.pbl18_indbetalinger", limit=2)
            self.assertEqual("list", pension["containers"][-1]["kind"])
            self.assertIsNotNone(pension["page"]["next_offset"])
            honorar = guide.inspect("lønmodtager.personlig_indkomst.ordinære_forhold.personlige_arbejdsvederlag.udgifter")
            self.assertEqual(["PsHonorarudgifterUoplyst", "PsHonorarudgifterOplyst"],
                             [row["name"] for row in honorar["children"]])
            self.assertTrue(any(source["role"] == "warning" for source in honorar["source_objects"].values()))
            print(json.dumps({"fresh_schema_bytes": path.stat().st_size, "view_bytes": sizes}, ensure_ascii=False))


if __name__ == "__main__":
    unittest.main()
