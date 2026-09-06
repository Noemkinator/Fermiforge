"""Generate immutable `data/` snapshots from upstream sources (PLAN.md § 7.3).

Online (monthly cron / manual):  python -m ingest.generate
Offline (reproducible, fixtures): python -m ingest.generate --offline

The generated snapshot is validated with the "no bare numbers" gate before
anything is written; a validation failure aborts without touching `data/`.
"""

from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import types
from pathlib import Path

from ingest import validate
from ingest.sources import bse, codata, iaea, pdg
from ingest.value import Value

REPO = Path(__file__).resolve().parents[1]
DATA = REPO / "data"
FIXTURES = Path(__file__).resolve().parent / "tests" / "fixtures"
SCHEMA_VERSION = 0


def _load_fixture(name: str):
    return json.loads((FIXTURES / name).read_text(encoding="utf-8"))


def _values_block(values: list[Value]) -> dict:
    return {value.id: value.to_dict() for value in values}


def _particles(offline: bool, fetched: str) -> dict:
    if offline:
        values = [
            pdg.parse(_load_fixture(f"pdg_{pid}.json"), pdg_id=pid, fetched=fetched)
            for pid in pdg.PARTICLES
        ]
    else:
        values = pdg.fetch_all(fetched=fetched)
    return {"schema_version": SCHEMA_VERSION, "generated": fetched, "values": _values_block(values)}


def _constants(offline: bool, fetched: str) -> dict:
    if offline:
        fixture = _load_fixture("codata_constants.json")
        module = types.SimpleNamespace(
            alpha=fixture["alpha"],
            codata_version=fixture["codata_version"],
            physical_constants={
                key: tuple(entry) for key, entry in fixture["physical_constants"].items()
            },
        )
        values = codata.parse(module, fetched=fetched)
    else:
        values = codata.fetch_all(fetched=fetched)
    return {"schema_version": SCHEMA_VERSION, "generated": fetched, "values": _values_block(values)}


def _nuclei(offline: bool, fetched: str) -> dict:
    if offline:
        values: list[Value] = []
        for payload in _load_fixture("iaea_nuclei.json"):
            values.extend(iaea.parse(payload, fetched=fetched))
    else:
        values = iaea.fetch_all(fetched=fetched)
    return {"schema_version": SCHEMA_VERSION, "generated": fetched, "values": _values_block(values)}


def _basis(offline: bool, fetched: str) -> dict:
    if offline:
        payload = _load_fixture("bse_sto3g.json")
    else:
        payload = bse.fetch("sto-3g", sorted(bse.ELEMENT_SYMBOLS))
    edition = str(payload.get("version", "1.0"))
    body = bse.parse(payload, basis="sto-3g", edition=edition, fetched=fetched)
    return {"schema_version": SCHEMA_VERSION, "generated": fetched, **body}


def _write(path: Path, obj: dict) -> str:
    text = json.dumps(obj, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    path.write_bytes(text.encode("utf-8"))
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--offline",
        action="store_true",
        help="parse pinned fixtures instead of fetching live sources",
    )
    args = parser.parse_args(argv[1:])
    fetched = datetime.date.today().isoformat()

    files = {
        "particles.json": (_particles(args.offline, fetched), "PDG", pdg.EDITION),
        "constants.json": (_constants(args.offline, fetched), "CODATA", "via scipy.constants"),
        "nuclei.json": (_nuclei(args.offline, fetched), "IAEA/AME", iaea.EDITION),
        "basis_sto3g.json": (_basis(args.offline, fetched), "BSE", "1.0"),
    }

    DATA.mkdir(exist_ok=True)
    digests: dict[str, dict] = {}
    for name, (body, source, edition) in files.items():
        digest = _write(DATA / name, body)
        digests[name] = {
            "source": source,
            "edition": edition,
            "fetched": fetched,
            "sha256": digest,
        }
    manifest = {"schema_version": SCHEMA_VERSION, "generated": fetched, "files": digests}
    _write(DATA / "manifest.json", manifest)

    errors = [error for name in files for error in validate.validate_file(DATA / name)]
    errors += validate.validate_file(DATA / "manifest.json")
    if errors:
        for error in errors:
            print(f"VIOLATION: {error}")
        raise SystemExit("generated data failed validation — fix ingest before committing")
    print(f"wrote {len(files)} data files + manifest.json to {DATA}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(__import__("sys").argv))
