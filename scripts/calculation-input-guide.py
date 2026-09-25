#!/usr/bin/env python3
"""Read one branch of a fresh compact calculation schema; never construct facts.

This is a repository navigation helper, not a contract validator or calculator.
The source export and its reported fingerprint must come from a trusted model.
"""

import argparse
import json
import math
import os
from pathlib import Path
import re
import stat
import sys

MAX_INPUT_BYTES = 32 * 1024 * 1024
MAX_OUTPUT_BYTES = 1024 * 1024


def require(condition, message):
    if not condition:
        raise ValueError(message)


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, "Duplicate JSON member: " + repr(key))
        result[key] = value
    return result


def finite_float(token):
    value = float(token)
    require(math.isfinite(value), "Non-finite JSON number")
    return value


def invalid_constant(token):
    raise ValueError("Non-JSON number: " + token)


def read_schema(path):
    # Nonblocking open prevents a FIFO from hanging an offline inspection.
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NONBLOCK", 0))
    with os.fdopen(descriptor, "rb") as handle:
        info = os.fstat(handle.fileno())
        require(stat.S_ISREG(info.st_mode) and info.st_size <= MAX_INPUT_BYTES,
                "Choose a regular compact-schema file of at most 32 MiB")
        data = handle.read(MAX_INPUT_BYTES + 1)
    require(len(data) <= MAX_INPUT_BYTES, "Compact schema exceeds 32 MiB")
    return json.loads(data.decode("utf-8"), object_pairs_hook=unique_object,
                      parse_float=finite_float, parse_constant=invalid_constant)


def index_by(rows, key):
    require(isinstance(rows, list), "Expected a schema list")
    result = {}
    for row in rows:
        require(isinstance(row, dict) and isinstance(row.get(key), str),
                "Malformed schema entry: " + key)
        name = row[key]
        require(name not in result, "Duplicate schema entry: " + repr(name))
        result[name] = row
    return result


def child_path(parent, member):
    return parent + "." + member if parent else member


def substitute(reference, bindings):
    require(isinstance(reference, dict), "Malformed type reference")
    kind = reference["kind"]
    if kind == "type_parameter":
        name = reference["name"]
        require(name in bindings, "Unbound type parameter: " + name)
        return bindings[name]
    if kind in ("list", "set", "optional"):
        return {**reference, "item": substitute(reference["item"], bindings)}
    if kind == "map":
        return {**reference, "key": substitute(reference["key"], bindings),
                "value": substitute(reference["value"], bindings)}
    if kind == "named" and "arguments" in reference:
        return {**reference, "arguments": [substitute(item, bindings)
                                           for item in reference["arguments"]]}
    require(kind in ("named", "primitive", "unit"), "Unsupported type kind: " + str(kind))
    return reference


class InputGuide:
    def __init__(self, schema):
        require(isinstance(schema, dict), "Expected a compact calculation schema")
        require(schema.get("schema") == "futuruna.calculate.compact.v1"
                and type(schema.get("schema_version")) is int and schema["schema_version"] == 1
                and schema.get("contract_schema") == "futuruna.calculate.v1"
                and type(schema.get("contract_schema_version")) is int
                and schema["contract_schema_version"] == 1,
                "Use a fresh runa schema --format compact-json export")
        require(isinstance(schema.get("schema_hash"), str)
                and re.fullmatch(r"[0-9a-f]{64}", schema["schema_hash"]),
                "Missing or malformed declared schema fingerprint")
        self.schema = schema
        self.definitions = index_by(schema["definitions"], "name")
        self.fields = index_by(schema.get("field_metadata", []), "path")
        self.groups = schema["source_groups"]
        self.sources = schema["source_objects"]
        require(isinstance(self.groups, dict) and isinstance(self.sources, dict),
                "Malformed compact source dictionaries")
        for ids in self.groups.values():
            require(isinstance(ids, list) and ids and all(isinstance(key, str)
                    and key in self.sources for key in ids), "Dangling or empty source group")
        for field in self.fields.values():
            require("source_group" not in field or (isinstance(field["source_group"], str)
                    and field["source_group"] in self.groups), "Dangling field source group")
        require(isinstance(schema.get("metadata", []), list), "Malformed root metadata")

    def node(self, reference, path, contexts):
        original = reference
        for _ in range(64):
            kind = reference["kind"]
            if kind not in ("optional", "list", "set", "map"):
                break
            context = {"path": path, "kind": kind}
            if kind == "map":
                require(reference["key"] == {"kind": "primitive", "name": "String"},
                        "Only string-keyed map navigation is supported")
                reference = reference["value"]
            else:
                reference = reference["item"]
            contexts.append(context)
        else:
            raise ValueError("Type nesting exceeds navigation limit")
        node = {"path": path, "type": original, "kind": "leaf"}
        if kind == "named":
            name = reference["name"]
            require(name in self.definitions, "Missing type definition: " + name)
            definition = self.definitions[name]
            require(definition["kind"] == "adt", "Unsupported definition kind")
            parameters = definition["parameters"]
            arguments = reference.get("arguments", [])
            require(len(parameters) == len(arguments), "Wrong type argument count: " + name)
            bindings = dict(zip(parameters, arguments))
            variants = definition["variants"]
            index_by(variants, "name")
            product = (len(variants) == 1 and variants[0]["name"] == name
                       and not variants[0]["positional"])
            node.update(kind="record" if product else "choice", variants=variants, bindings=bindings)
        else:
            require(kind in ("primitive", "unit"), "Unresolved or unsupported type reference")
        return node

    def children(self, node):
        if node["kind"] == "choice":
            return [{"name": variant["name"], "path": child_path(node["path"], variant["name"]),
                     "kind": "variant", "positional": variant["positional"]}
                    for variant in node["variants"]]
        if node["kind"] not in ("record", "variant"):
            return []
        variant = node["variants"][0]
        require(not variant["positional"],
                "Positional payload navigation is unsupported; inspect the original definition")
        index_by(variant["fields"], "name")
        return [{"name": field["name"], "path": child_path(node["path"], field["name"]),
                 "kind": "field", "type": substitute(field["type"], node["bindings"])}
                for field in variant["fields"]]

    def locate(self, path):
        require(isinstance(path, str) and (not path or all(path.split("."))), "Empty path segment")
        parts = path.split(".") if path else []
        require(len(parts) <= 64, "Path exceeds navigation limit")
        contexts, guards, ancestors = [], [], []
        node = self.node(self.schema["input"], "", contexts)
        for index, part in enumerate(parts):
            ancestors.append(node["path"])
            if node["kind"] == "choice":
                discriminator = child_path(node["path"], "$variant")
                if part == "$variant":
                    require(index == len(parts) - 1, "$variant is terminal")
                    node = {**node, "path": discriminator, "kind": "discriminator"}
                    break
                variants = {item["name"]: item for item in node["variants"]}
                require(part in variants, "Unknown alternative at " + repr(node["path"]) + ": " + repr(part))
                guards.append({"path": discriminator, "equals": part})
                ancestors.append(discriminator)
                node = {**node, "path": child_path(node["path"], part), "kind": "variant",
                        "variants": [variants[part]]}
            else:
                children = {item["name"]: item for item in self.children(node)}
                require(part in children, "Unknown field at " + repr(node["path"]) + ": " + repr(part))
                child = children[part]
                node = self.node(child["type"], child["path"], contexts)
        return node, contexts, guards, ancestors

    def inspect(self, path="", offset=0, limit=20, root_metadata=False):
        require(type(offset) is int and offset >= 0 and type(limit) is int and 1 <= limit <= 100,
                "Use offset >= 0 and limit between 1 and 100")
        result = {
            "schema": "futuruna.input-guide.v1", "entry": self.schema["entry"],
            "schema_hash": self.schema["schema_hash"], "label": self.schema.get("label"),
            "notice": "Read-only schema view, not an input envelope, validation or proof of legal coverage. "
                      "The fingerprint is reported, not authenticated. Use a fresh trusted export. "
                      "Paths name schema members, not case JSON pointers. No alternative or fact is selected.",
        }
        if root_metadata:
            require(not path, "Root metadata cannot be combined with a field path")
            rows = self.schema.get("metadata", [])
            result["root_metadata"] = rows[offset:offset + limit]
        else:
            node, contexts, guards, ancestors = self.locate(path)
            if node["kind"] == "discriminator":
                rows = [{"name": variant["name"]} for variant in node["variants"]]
            else:
                rows = self.children(node)
            selected = rows[offset:offset + limit]
            result.update(path=path, node={key: node[key] for key in ("kind", "type")},
                          containers=contexts, variant_guards=guards, children=selected)
            result["node"]["has_explicit_metadata"] = path in self.fields
            if node["kind"] == "choice":
                result["discriminator_path"] = child_path(path, "$variant")
            for row in selected:
                if "path" in row:
                    row["has_explicit_metadata"] = row["path"] in self.fields
            paths = set(ancestors + [path] + [row["path"] for row in selected if "path" in row])
            if node["kind"] == "choice":
                paths.add(child_path(path, "$variant"))
            fields = [field for key, field in self.fields.items() if key in paths]
            groups = {field["source_group"] for field in fields if "source_group" in field}
            sources = {key for group in groups for key in self.groups[group]}
            result.update(field_metadata=fields,
                          source_groups={key: self.groups[key] for key in sorted(groups)},
                          source_objects={key: self.sources[key] for key in sorted(sources)},
                          root_metadata_total=len(self.schema.get("metadata", [])),
                          root_metadata_access="Use --root-metadata with --offset/--limit; not included here.")
        require(offset <= len(rows), "Offset is beyond the available rows")
        result["page"] = {"offset": offset, "limit": limit, "total": len(rows),
                          "next_offset": offset + limit if offset + limit < len(rows) else None}
        return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("schema", type=Path, help="fresh compact-json schema, not tax facts")
    parser.add_argument("path", nargs="?", default="", help="exact canonical input path; omit for root")
    parser.add_argument("--offset", type=int, default=0)
    parser.add_argument("--limit", type=int, default=20)
    parser.add_argument("--root-metadata", action="store_true", help="page whole-calculation metadata")
    args = parser.parse_args(argv)
    try:
        result = InputGuide(read_schema(args.schema)).inspect(args.path, args.offset, args.limit, args.root_metadata)
        output = json.dumps(result, ensure_ascii=True, allow_nan=False, indent=2)
        require(len(output.encode("utf-8")) <= MAX_OUTPUT_BYTES,
                "View exceeds 1 MiB; choose a deeper path or smaller --limit. No partial output emitted.")
        print(output)
        return 0
    except (OSError, ValueError, KeyError, TypeError, RecursionError) as error:
        print("Input guide: " + ascii(str(error)), file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
