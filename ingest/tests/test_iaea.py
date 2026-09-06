import json
from pathlib import Path

import pytest

from ingest.sources import iaea

FIXTURES = Path(__file__).parent / "fixtures"


def payloads() -> list[dict]:
    return json.loads((FIXTURES / "iaea_nuclei.json").read_text(encoding="utf-8"))


def test_deuteron_values():
    values = iaea.parse(payloads()[0], fetched="2026-09-06")
    by_id = {v.id: v for v in values}
    assert by_id["nuclei/d/mass_excess_MeV"].value == pytest.approx(13.1357980)
    assert by_id["nuclei/d/binding_energy_MeV"].value == pytest.approx(2.2245660)
    assert by_id["nuclei/d/binding_energy_MeV"].edition == "AME2020"


def test_all_fixture_nuclei_parse():
    total = 0
    for payload in payloads():
        values = iaea.parse(payload, fetched="2026-09-06")
        assert len(values) == 2
        total += len(values)
    assert total == 12
