"""Semi-empirical model parameters (PLAN.md § 6.3: approximations labeled).

These are literature model parameters, not measurements: each carries a
`method` label and a literature reference in `url` (PLAN.md § 13.8). The
ingest source exists so the values live in `data/` with provenance instead
of being hardcoded in core (PLAN.md § 13.2).
"""

from __future__ import annotations

from ingest.value import Value

EDITION = "Pauling-1960"

#: (canonical id, value, method, reference URL)
PARAMETERS: list[tuple[str, float, str, str]] = [
    (
        "models/huckel/bond_cutoff_angstrom",
        1.6,
        "approximation: single global cutoff for covalent bond perception",
        "https://en.wikipedia.org/wiki/Covalent_radius (C-C 1.54 A, general 1.6 A cutoff)",
    ),
]


def parse(*, fetched: str) -> list[Value]:
    out = []
    for value_id, value, method, url in PARAMETERS:
        v = Value.from_source(
            value,
            source="literature",
            edition=EDITION,
            fetched=fetched,
            url=url,
            id=value_id,
        )
        v.method = method
        out.append(v)
    return out
