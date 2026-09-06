"""Gaussian basis sets from Basis Set Exchange (basissetexchange.org).

API: GET /api/basis/<name>/format/json?elements=<csv Z>. Exponents and
contraction coefficients are floats from the source and therefore each
becomes its own provenance-tracked Value ("no bare numbers", PLAN.md § 7.1).
"""

from __future__ import annotations

import json
import urllib.request

from ingest.value import Value

BASE = "https://www.basissetexchange.org/api/basis"
#: structural element labels for the elements we ingest (not physical data)
ELEMENT_SYMBOLS: dict[int, str] = {1: "H", 6: "C"}


def fetch(basis: str, elements: list[int], *, timeout: int = 60) -> dict:
    query = ",".join(str(z) for z in elements)
    url = f"{BASE}/{basis}/format/json?elements={query}"
    with urllib.request.urlopen(url, timeout=timeout) as response:
        return json.load(response)


def parse(payload: dict, *, basis: str, edition: str, fetched: str) -> dict:
    """Normalize a BSE JSON export into the `data/basis_sto3g.json` shape."""
    elements_out: dict = {}
    for z_str, calc_lists in sorted(payload["elements"].items(), key=lambda kv: int(kv[0])):
        symbol = ELEMENT_SYMBOLS[int(z_str)]
        shells_out = []
        for calculation in calc_lists:
            for shell in calculation["electron_shells"]:
                angular = "+".join(shell["angular_momentum"])
                primitives = []
                for k, exponent in enumerate(shell["exponents"]):
                    coefficients = [float(contraction[k]) for contraction in shell["coefficients"]]
                    base_id = f"basis/{basis}/{symbol}/{angular}/{k}"
                    primitives.append(
                        {
                            "exponent": Value.from_source(
                                float(exponent),
                                source="BSE",
                                edition=edition,
                                fetched=fetched,
                                url=payload.get("reference", BASE),
                                id=f"{base_id}/exponent",
                            ).to_dict(),
                            "coefficients": [
                                Value.from_source(
                                    coefficient,
                                    source="BSE",
                                    edition=edition,
                                    fetched=fetched,
                                    url=payload.get("reference", BASE),
                                    id=f"{base_id}/contract/{j}",
                                ).to_dict()
                                for j, coefficient in enumerate(coefficients)
                            ],
                        }
                    )
                shells_out.append({"angular_momentum": angular, "primitives": primitives})
        elements_out[symbol] = {"shells": shells_out}
    return {"basis": basis, "elements": elements_out}


def fetch_all(*, basis: str = "sto-3g", edition: str, fetched: str) -> dict:
    payload = fetch(basis, sorted(ELEMENT_SYMBOLS))
    return parse(payload, basis=basis, edition=edition, fetched=fetched)
