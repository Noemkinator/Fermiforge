"""Fundamental constants via `scipy.constants` (CODATA, PLAN.md § 7.2).

scipy is imported lazily so the parse logic can be unit-tested against a
fixture module without scipy installed.
"""

from __future__ import annotations

from ingest.value import Value

#: scipy.constants.physical_constants keys -> canonical value ids
PHYSICAL_CONSTANTS: dict[str, str] = {
    "electron mass energy equivalent in MeV": "constants/electron_mass_MeV",
    "muon mass energy equivalent in MeV": "constants/muon_mass_MeV",
    "proton mass energy equivalent in MeV": "constants/proton_mass_MeV",
    "neutron mass energy equivalent in MeV": "constants/neutron_mass_MeV",
}


#: alpha fingerprints used to label which CODATA release scipy ships
#: (scipy exposes no version attribute; ingest-only fingerprints, documented).
ALPHA_EDITIONS = {
    7.2973525693e-3: "CODATA 2018",
    7.2973525643e-3: "CODATA 2022",
}


def _detect_edition(alpha: float) -> str:
    for fingerprint, edition in ALPHA_EDITIONS.items():
        if abs(alpha - fingerprint) < 1e-13:
            return edition
    raise ValueError(f"unknown CODATA release for alpha={alpha!r} — update fingerprints")


def parse(constants_module, *, fetched: str) -> list[Value]:
    """Normalize a `scipy.constants`-like module into Values.

    `constants_module` must expose `alpha` and `physical_constants`
    (value, unit, uncertainty) — the scipy API — and may expose
    `codata_version` (used when present).
    """
    edition = str(
        getattr(constants_module, "codata_version", None) or _detect_edition(constants_module.alpha)
    )
    out = [
        Value.from_source(
            constants_module.alpha,
            source="CODATA",
            edition=edition,
            uncertainty=None,
            fetched=fetched,
            url="https://docs.scipy.org/doc/scipy/reference/constants.html",
            id="constants/alpha",
        )
    ]
    for key, value_id in PHYSICAL_CONSTANTS.items():
        value, _unit, uncertainty = constants_module.physical_constants[key]
        out.append(
            Value.from_source(
                value,
                source="CODATA",
                edition=edition,
                uncertainty=float(uncertainty),
                fetched=fetched,
                url="https://docs.scipy.org/doc/scipy/reference/constants.html",
                id=value_id,
            )
        )
    return out


def fetch_all(*, fetched: str) -> list[Value]:
    from scipy import constants

    return parse(constants, fetched=fetched)
