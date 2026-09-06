"""Nuclear data from the IAEA LiveChart API (AME mass evaluations).

Response shape pinned by `tests/fixtures/iaea_nuclei.json`. Values carry the
AME edition they come from. A live format change fails the fixture tests and
the cron opens an issue (PLAN.md § 7.3).
"""

from __future__ import annotations

import json
import urllib.request

from ingest.value import Value

BASE = "https://www-nds.iaea.org/livechart/api/nucleide"
EDITION = "AME2020"

#: (z, a) -> canonical name for the light-nuclei validation set (PLAN.md § 8.3)
NUCLEI: dict[tuple[int, int], str] = {
    (1, 2): "d",
    (1, 3): "t",
    (2, 3): "He3",
    (2, 4): "He4",
    (6, 12): "C12",
    (8, 16): "O16",
}


def fetch(z: int, a: int, *, timeout: int = 30) -> dict:
    url = f"{BASE}?z={z}&a={a}"
    with urllib.request.urlopen(url, timeout=timeout) as response:
        return json.load(response)


def parse(payload: dict, *, edition: str = EDITION, fetched: str) -> list[Value]:
    """Normalize one nuclide payload: mass excess and total binding energy, MeV."""
    z = int(payload["z"])
    a = int(payload["a"])
    name = NUCLEI[(z, a)]
    url = f"{BASE}?z={z}&a={a}"
    return [
        Value.from_source(
            float(payload["mass_excess"]),
            source="IAEA/AME",
            edition=edition,
            uncertainty=float(payload["mass_excess_uncertainty"]),
            fetched=fetched,
            url=url,
            id=f"nuclei/{name}/mass_excess_MeV",
        ),
        Value.from_source(
            float(payload["binding_energy"]),
            source="IAEA/AME",
            edition=edition,
            uncertainty=float(payload["binding_energy_uncertainty"]),
            fetched=fetched,
            url=url,
            id=f"nuclei/{name}/binding_energy_MeV",
        ),
    ]


def fetch_all(*, fetched: str) -> list[Value]:
    out: list[Value] = []
    for z, a in NUCLEI:
        out.extend(parse(fetch(z, a), fetched=fetched))
    return out
