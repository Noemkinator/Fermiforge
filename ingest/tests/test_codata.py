import json
import types
from pathlib import Path

import pytest

from ingest.sources import codata

FIXTURES = Path(__file__).parent / "fixtures"


@pytest.fixture()
def fake_scipy() -> types.SimpleNamespace:
    fixture = json.loads((FIXTURES / "codata_constants.json").read_text(encoding="utf-8"))
    return types.SimpleNamespace(
        alpha=fixture["alpha"],
        codata_version=fixture["codata_version"],
        physical_constants={
            key: tuple(entry) for key, entry in fixture["physical_constants"].items()
        },
    )


def test_alpha_and_masses_parse(fake_scipy):
    values = codata.parse(fake_scipy, fetched="2026-09-06")
    by_id = {v.id: v for v in values}
    assert by_id["constants/alpha"].value == pytest.approx(0.0072973525643)
    assert by_id["constants/alpha"].edition == "CODATA 2022"
    assert by_id["constants/proton_mass_MeV"].value == pytest.approx(938.27208816)
    assert by_id["constants/proton_mass_MeV"].uncertainty == pytest.approx(2.9e-8)
    assert len(values) == 5


def test_real_scipy_matches_fixture():
    pytest.importorskip("scipy")
    from scipy import constants

    values = {v.id: v for v in codata.parse(constants, fetched="2026-09-06")}
    fixture = json.loads((FIXTURES / "codata_constants.json").read_text(encoding="utf-8"))
    assert values["constants/alpha"].value == pytest.approx(fixture["alpha"], rel=1e-12)
