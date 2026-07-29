#!/usr/bin/env python3
"""Check Danish income-tax Retsinformation source metadata against Runa records.

The script reads named `Retskilde(...)` records from
`examples/danish-income-tax/source-status.runa`, fetches each official
`/dan/xml` document from Retsinformation, and reports semantic drift between the
encoded source model and the official XML metadata.

It intentionally reports differences instead of editing `.runa` files. Legal
source changes should be reviewed before the executable law model is updated.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import re
import sys
import tempfile
import urllib.error
import urllib.request
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, TextIO


DEFAULT_SOURCE = Path("examples/danish-income-tax/source-status.runa")
STATUS_TO_RUNA = {
    "Valid": "Gældende",
    "Historic": "Historisk",
}


@dataclass(frozen=True)
class SourceRecord:
    name: str
    eli: str
    year: int
    number: int
    status: str
    fetched_yyyymmdd: int
    start_yyyymmdd: int
    end_yyyymmdd: int


@dataclass(frozen=True)
class OfficialMetadata:
    title: str
    popular_title: str
    year: int
    number: int
    status_xml: str
    status_runa: str
    start_yyyymmdd: int
    end_yyyymmdd: int
    historic_mark_yyyymmdd: int | None


@dataclass(frozen=True)
class Finding:
    field: str
    encoded: object
    official: object


@dataclass(frozen=True)
class CheckResult:
    record: SourceRecord
    official: OfficialMetadata | None
    findings: tuple[Finding, ...]
    error: str | None = None


RECORD_RE = re.compile(
    r"^=\s+(?P<name>[\wæøåÆØÅ]+)\s+=\s+Retskilde\((?P<body>.*?)^\)",
    re.MULTILINE | re.DOTALL,
)
FIELD_RE = re.compile(r"^\s*(?P<field>[\wæøåÆØÅ]+)\s*=\s*(?P<value>.*?)(?:,)?\s*$")


def parse_scalar(raw: str) -> object:
    raw = raw.strip()
    if raw.startswith('"') and raw.endswith('"'):
        return raw[1:-1]
    if re.fullmatch(r"-?\d+", raw):
        return int(raw)
    return raw


def parse_source_records(text: str) -> list[SourceRecord]:
    records: list[SourceRecord] = []
    for match in RECORD_RE.finditer(text):
        fields: dict[str, object] = {}
        for line in match.group("body").splitlines():
            field_match = FIELD_RE.match(line)
            if not field_match:
                continue
            fields[field_match.group("field")] = parse_scalar(field_match.group("value"))

        required = {
            "eli",
            "år",
            "nummer",
            "status",
            "hentet_yyyymmdd",
            "metadata_start_yyyymmdd",
            "metadata_end_yyyymmdd",
        }
        missing = sorted(required.difference(fields))
        if missing:
            raise ValueError(f"{match.group('name')} missing fields: {', '.join(missing)}")

        records.append(
            SourceRecord(
                name=match.group("name"),
                eli=str(fields["eli"]),
                year=int(fields["år"]),
                number=int(fields["nummer"]),
                status=str(fields["status"]),
                fetched_yyyymmdd=int(fields["hentet_yyyymmdd"]),
                start_yyyymmdd=int(fields["metadata_start_yyyymmdd"]),
                end_yyyymmdd=int(fields["metadata_end_yyyymmdd"]),
            )
        )
    if not records:
        raise ValueError("no Retskilde records found")
    return records


def local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1]


def child_text(element: ET.Element, name: str) -> str:
    for child in element:
        if local_name(child.tag) == name:
            return (child.text or "").strip()
    return ""


def find_child(element: ET.Element, name: str) -> ET.Element | None:
    for child in element:
        if local_name(child.tag) == name:
            return child
    return None


def parse_date_yyyymmdd(value: str) -> int:
    if not value:
        raise ValueError("missing date")
    return int(value.replace("-", ""))


def parse_optional_date_yyyymmdd(value: str) -> int | None:
    if not value:
        return None
    return parse_date_yyyymmdd(value)


def parse_official_metadata(xml_bytes: bytes) -> OfficialMetadata:
    root = ET.fromstring(xml_bytes)
    meta = find_child(root, "Meta")
    if meta is None:
        raise ValueError("Retsinformation XML has no Meta element")

    status_xml = child_text(meta, "Status")
    return OfficialMetadata(
        title=child_text(meta, "DocumentTitle"),
        popular_title=child_text(meta, "PopularTitle"),
        year=int(child_text(meta, "Year")),
        number=int(child_text(meta, "Number")),
        status_xml=status_xml,
        status_runa=STATUS_TO_RUNA.get(status_xml, status_xml),
        start_yyyymmdd=parse_date_yyyymmdd(child_text(meta, "StartDate")),
        end_yyyymmdd=parse_date_yyyymmdd(child_text(meta, "EndDate")),
        historic_mark_yyyymmdd=parse_optional_date_yyyymmdd(child_text(meta, "DateOfHistoricMark")),
    )


def fetch_official_xml(record: SourceRecord, timeout_seconds: float) -> bytes:
    url = record.eli.rstrip("/") + "/dan/xml"
    request = urllib.request.Request(
        url,
        headers={"User-Agent": "Futuruna Danish tax source metadata checker"},
    )
    with urllib.request.urlopen(request, timeout=timeout_seconds) as response:
        return response.read()


def load_offline_xml(record: SourceRecord, offline_xml_dir: Path) -> bytes:
    path = offline_xml_dir / f"{record.year}_{record.number}.xml"
    return path.read_bytes()


def compare_record(
    record: SourceRecord,
    official: OfficialMetadata,
    today_yyyymmdd: int,
    ignore_fetch_date: bool,
) -> tuple[Finding, ...]:
    checks: list[tuple[str, object, object]] = [
        ("år", record.year, official.year),
        ("nummer", record.number, official.number),
        ("status", record.status, official.status_runa),
        ("metadata_start_yyyymmdd", record.start_yyyymmdd, official.start_yyyymmdd),
        ("metadata_end_yyyymmdd", record.end_yyyymmdd, official.end_yyyymmdd),
    ]
    if not ignore_fetch_date:
        checks.append(("hentet_yyyymmdd", record.fetched_yyyymmdd, today_yyyymmdd))

    return tuple(Finding(field, encoded, official) for field, encoded, official in checks if encoded != official)


def check_records(
    records: Iterable[SourceRecord],
    today_yyyymmdd: int,
    timeout_seconds: float,
    offline_xml_dir: Path | None,
    ignore_fetch_date: bool,
) -> list[CheckResult]:
    results: list[CheckResult] = []
    for record in records:
        try:
            xml_bytes = (
                load_offline_xml(record, offline_xml_dir)
                if offline_xml_dir is not None
                else fetch_official_xml(record, timeout_seconds)
            )
            official = parse_official_metadata(xml_bytes)
            findings = compare_record(record, official, today_yyyymmdd, ignore_fetch_date)
            results.append(CheckResult(record=record, official=official, findings=findings))
        except (OSError, ET.ParseError, ValueError, urllib.error.URLError) as error:
            results.append(CheckResult(record=record, official=None, findings=(), error=str(error)))
    return results


def render_report(results: Iterable[CheckResult], today_yyyymmdd: int) -> str:
    rows = list(results)
    error_count = sum(1 for result in rows if result.error is not None)
    drift_count = sum(1 for result in rows if result.findings)
    ok_count = len(rows) - error_count - drift_count

    lines = [
        f"Retsinformation source metadata check date: {today_yyyymmdd}",
        f"records={len(rows)} ok={ok_count} drift={drift_count} errors={error_count}",
        "",
        "| record | official status | official end | findings |",
        "| --- | --- | ---: | --- |",
    ]
    for result in rows:
        if result.error is not None:
            lines.append(f"| {result.record.name} | ERROR | - | {result.error} |")
            continue
        assert result.official is not None
        findings = "; ".join(
            f"{finding.field}: encoded={finding.encoded} official={finding.official}"
            for finding in result.findings
        )
        lines.append(
            "| {record} | {status} | {end} | {findings} |".format(
                record=result.record.name,
                status=result.official.status_xml,
                end=result.official.end_yyyymmdd,
                findings=findings or "OK",
            )
        )
    return "\n".join(lines)


def default_today_yyyymmdd() -> int:
    return int(_dt.date.today().strftime("%Y%m%d"))


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--offline-xml-dir", type=Path)
    parser.add_argument("--today", type=int, default=default_today_yyyymmdd())
    parser.add_argument("--timeout", type=float, default=30.0)
    parser.add_argument("--fail-on-drift", action="store_true")
    parser.add_argument("--ignore-fetch-date", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    return parser


def run(args: argparse.Namespace, stdout: TextIO, stderr: TextIO) -> int:
    try:
        records = parse_source_records(args.source.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        print(f"source parse error: {error}", file=stderr)
        return 2

    results = check_records(
        records=records,
        today_yyyymmdd=args.today,
        timeout_seconds=args.timeout,
        offline_xml_dir=args.offline_xml_dir,
        ignore_fetch_date=args.ignore_fetch_date,
    )
    print(render_report(results, args.today), file=stdout)

    if any(result.error for result in results):
        return 2
    if args.fail_on_drift and any(result.findings for result in results):
        return 1
    return 0


SAMPLE_RUNA = """
# Kildestatus = Gældende | Historisk
# Beregningsformål = AktuelSkatteberegning | DagsaktuelAutomatiskBeregning | HistoriskAudit | KildeDiff
# Retskilde(eli: Tekst, år: Heltal, nummer: Heltal, status: Kildestatus, hentet_yyyymmdd: Heltal, metadata_start_yyyymmdd: Heltal, metadata_end_yyyymmdd: Heltal)

= personskatteloven_2021_1284 = Retskilde(
    eli = "https://www.retsinformation.dk/eli/lta/2021/1284",
    år = 2021,
    nummer = 1284,
    status = Gældende,
    hentet_yyyymmdd = 20260718,
    metadata_start_yyyymmdd = 20210616,
    metadata_end_yyyymmdd = 20260623
)
"""


SAMPLE_XML = b"""<?xml version="1.0" encoding="utf-8"?>
<Dokument>
  <Meta>
    <DocumentTitle>Bekendtgorelse af lov om indkomstskat for personer m.v. (personskatteloven)</DocumentTitle>
    <Year>2021</Year>
    <StartDate>2021-06-16</StartDate>
    <EndDate>2026-06-23</EndDate>
    <Status>Valid</Status>
    <Number>1284</Number>
    <PopularTitle>Personskatteloven</PopularTitle>
  </Meta>
</Dokument>
"""


def run_self_test(stdout: TextIO) -> int:
    records = parse_source_records(SAMPLE_RUNA)
    assert len(records) == 1
    official = parse_official_metadata(SAMPLE_XML)
    assert not compare_record(records[0], official, 20260718, ignore_fetch_date=False)

    changed = SAMPLE_XML.replace(b"<EndDate>2026-06-23</EndDate>", b"<EndDate>2026-06-24</EndDate>")
    changed_official = parse_official_metadata(changed)
    findings = compare_record(records[0], changed_official, 20260718, ignore_fetch_date=False)
    assert [finding.field for finding in findings] == ["metadata_end_yyyymmdd"]

    with tempfile.TemporaryDirectory() as temp_dir:
        temp = Path(temp_dir)
        source_path = temp / "source-status.runa"
        xml_dir = temp / "xml"
        xml_dir.mkdir()
        source_path.write_text(SAMPLE_RUNA, encoding="utf-8")
        (xml_dir / "2021_1284.xml").write_bytes(SAMPLE_XML)
        args = argparse.Namespace(
            source=source_path,
            offline_xml_dir=xml_dir,
            today=20260718,
            timeout=1.0,
            fail_on_drift=True,
            ignore_fetch_date=False,
        )
        assert run(args, stdout, sys.stderr) == 0

    print("self-test passed", file=stdout)
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.self_test:
        return run_self_test(sys.stdout)
    return run(args, sys.stdout, sys.stderr)


if __name__ == "__main__":
    raise SystemExit(main())
