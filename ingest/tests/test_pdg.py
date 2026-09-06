import json
from pathlib import Path

import pytest

from ingest.sources import pdg
from ingest.value import ValueSourceError

FIXTURES = Path(__file__).parent / "fixtures"


def load(name: str) -> dict:
    return json.loads((FIXTURES / name).read_text(encoding="utf-8"))


def test_parse_electron_mass():
    value = pdg.parse(load("pdg_11.json"), pdg_id=11, fetched="2026-09-06")
    assert value.value == pytest.approx(0.51099895069)
    assert value.uncertainty == pytest.approx(1.6e-10)
    assert value.source == "PDG"
    assert value.edition == "2024"
    assert value.id == "particles/electron/mass_MeV"


def test_parse_quark_uses_symmetric_max_uncertainty():
    value = pdg.parse(load("pdg_1.json"), pdg_id=1, fetched="2026-09-06")
    assert value.value == pytest.approx(2.16)
    assert value.uncertainty == pytest.approx(0.49)


def test_missing_mass_fails_loudly():
    with pytest.raises(ValueError):
        pdg.parse({"pdg_id": 11, "name": "e", "mass": []}, pdg_id=11, fetched="2026-09-06")


def test_all_particle_fixtures_parse():
    for pid in pdg.PARTICLES:
        value = pdg.parse(load(f"pdg_{pid}.json"), pdg_id=pid, fetched="2026-09-06")
        assert value.value > 0
        assert value.id.endswith("/mass_MeV")
        assert value.source == "PDG"


def test_bare_number_cannot_enter_value():
    with pytest.raises(ValueSourceError):
        pdg.parse(
            {"pdg_id": 11, "name": "e", "mass": [{"value": 0.5, "uncertainty": None}]},
            pdg_id=11,
            edition="",
            fetched="2026-09-06",
        )
