#!/usr/bin/env python3
"""Check partial evaluation of a calculation by leaving out one input path at a time.

For every case of a complete input file, this generates one partial case per
input leaf path (that path left out) and, for every list, one with the whole
list left out. It runs them all with `runa call --partial=check`, so each must
pass the strict-mode oracle and the completion test, and each must answer
`Complete` or `Needs` naming at least one path.

    python3 scripts/partial-leave-one-out.py MODEL INPUT [--entry NAME] [--runa PATH]

Leaves are scalars, `null` values, empty collections and whole sum-type values
(left out, a sum type is unknown at `path.$variant`). Elements of a list are
not left out one by one, since that changes the list; their fields are. Map
entries are not left out, since that changes the keys. Exits non-zero when any
generated case fails; prints the counts.
"""
import argparse
import copy
import json
import os
import subprocess
import sys
import tempfile


def run(runa, args):
    return subprocess.run([runa] + args, capture_output=True, text=True)


class Contract:
    def __init__(self, document):
        self.document = document
        self.definitions = {d["name"]: d for d in document["definitions"]}

    def resolve(self, ty, substitutions):
        if ty["kind"] == "type_parameter":
            return substitutions.get(ty["name"], ty)
        return ty

    def named(self, ty):
        definition = self.definitions[ty["name"]]
        substitutions = dict(zip(definition["parameters"], ty.get("arguments", [])))
        return definition, substitutions


def leaf_paths(contract, value, ty, substitutions, path, out):
    """Paths whose leaving out is a valid partial case."""
    ty = contract.resolve(ty, substitutions)
    kind = ty["kind"]
    if kind == "optional":
        if value is None:
            out.append(path)
        else:
            leaf_paths(contract, value, ty["item"], substitutions, path, out)
        return
    if kind in ("list", "set"):
        out.append(path)  # the whole collection
        if kind == "list":
            for index, item in enumerate(value):
                inner = []
                leaf_paths(contract, item, ty["item"], substitutions, f"{path}[{index}]", inner)
                out.extend(p for p in inner if p != f"{path}[{index}]")
        return
    if kind == "map":
        out.append(path)
        return
    if kind == "named":
        definition, local = contract.named(ty)
        variants = definition["variants"]
        if (len(variants) == 1 and variants[0]["name"] == definition["name"]
                and not variants[0]["positional"]):
            fields = variants[0]["fields"]
        else:
            out.append(path)  # the whole sum value: unknown at `$variant`
            chosen = next((v for v in variants if v["name"] == value.get("$variant")), None)
            if chosen is None or chosen["positional"]:
                return
            fields = chosen["fields"]
        for field in fields:
            if field["name"] in value:
                child = f"{path}.{field['name']}" if path else field["name"]
                leaf_paths(contract, value[field["name"]], field["type"], local, child, out)
        return
    out.append(path)  # primitive or unit


def leave_out(value, path):
    tokens = []
    for segment in path.split("."):
        name, _, rest = segment.partition("[")
        if name:
            tokens.append(name)
        while rest:
            index, _, rest = rest.partition("]")
            tokens.append(int(index))
            rest = rest.lstrip("[")
    result = copy.deepcopy(value)
    target = result
    for token in tokens[:-1]:
        target = target[token]
    if isinstance(tokens[-1], int):
        raise ValueError(f"cannot leave out list element {path}")
    del target[tokens[-1]]
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("model")
    parser.add_argument("input")
    parser.add_argument("--entry")
    parser.add_argument("--runa", default=os.environ.get("RUNA", "runa"))
    options = parser.parse_args()

    entry = ["--entry", options.entry] if options.entry else []
    schema = run(options.runa, ["schema", options.model] + entry)
    if schema.returncode:
        sys.exit(schema.stderr)
    contract = Contract(json.loads(schema.stdout))
    envelope = json.load(open(options.input))

    generated = []
    for case in envelope["cases"]:
        paths = []
        leaf_paths(contract, case["input"], contract.document["input"], {}, "", paths)
        for path in dict.fromkeys(paths):
            if not path or path.endswith("]"):
                continue
            generated.append({
                "case_id": f"{case['case_id']} without {path}",
                "input_status": "ready",
                "input": leave_out(case["input"], path),
            })
    partial = {"$futuruna": envelope["$futuruna"], "cases": generated}
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as handle:
        json.dump(partial, handle)
    try:
        checked = run(options.runa, ["call", options.model, "--input", handle.name, "--partial=check"] + entry)
    finally:
        os.unlink(handle.name)
    try:
        output = json.loads(checked.stdout)
    except json.JSONDecodeError:
        sys.exit(f"runa call failed:\n{checked.stderr}")

    counts = {"Complete": 0, "Needs": 0}
    failures = [f"{d['case_id']}: {d['path']}: {d['message']}" for d in output["diagnostics"]]
    for result in output["results"]:
        answer = result["result"]
        counts[answer["$variant"]] += 1
        if answer["$variant"] == "Needs" and not answer["needs"]:
            failures.append(f"{result['case_id']}: Needs without a path")
    print(f"{options.input}: {len(generated)} partial cases, "
          f"{counts['Complete']} Complete, {counts['Needs']} Needs, {len(failures)} failed")
    for failure in failures:
        print(f"  FAIL {failure}")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
