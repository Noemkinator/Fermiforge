"""PDG particle data via the PDG REST API (pdgapi.lbl.gov).

The response shape is taken from the public PDG REST API documentation and
pinned by `tests/fixtures/pdg_*.json`. A live API format change makes the
fixture-pinned parse tests fail loudly — by design the monthly cron then
opens an issue instead of silently writing bad data (PLAN.md § 7.3).

Known limitation (documented, PLAN.md § 13.8): PDG asymmetric uncertainties
(e.g. quark masses) are stored as the symmetric max of the two errors in M0.
"""

from __future__ import annotations

import json
import urllib.request

from ingest.value import Value

BASE = "https://pdgapi.lbl.gov/pdgrest/api/v1/public/particle"
EDITION = "2024"

#: pdg_id -> canonical value-id prefix
PARTICLES: dict[int, str] = {
    11: "particles/electron",
    13: "particles/muon",
    1: "particles/up",
    2: "particles/down",
    2212: "particles/proton",
    2112: "particles/neutron",
}


def fetch(pdg_id: int, *, timeout: int = 30) -> dict:
    url = f"{BASE}/{pdg_id}"
    with urllib.request.urlopen(url, timeout=timeout) as response:
        return json.load(response)


def parse(payload: dict, *, pdg_id: int, edition: str = EDITION, fetched: str) -> Value:
    """Normalize one PDG particle payload to a mass Value in MeV."""
    prefix = PARTICLES[pdg_id]
    masses = payload["mass"]
    if not masses:
        raise ValueError(f"PDG payload for {pdg_id} has no mass entries")
    entry = masses[0]
    value = float(entry["display"]) if "display" in entry else float(entry["value"])
    uncertainty = entry.get("uncertainty")
    return Value.from_source(
        value,
        source="PDG",
        edition=edition,
        uncertainty=None if uncertainty is None else float(uncertainty),
        fetched=fetched,
        url=f"{BASE}/{pdg_id}",
        id=f"{prefix}/mass_MeV",
    )


def fetch_all(*, fetched: str) -> list[Value]:
    return [parse(fetch(pid), pdg_id=pid, fetched=fetched) for pid in PARTICLES]
