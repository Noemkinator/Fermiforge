import json
from pathlib import Path

import pytest

from ingest.sources import bse

FIXTURES = Path(__file__).parent / "fixtures"


@pytest.fixture()
def parsed() -> dict:
    payload = json.loads((FIXTURES / "bse_sto3g.json").read_text(encoding="utf-8"))
    return bse.parse(payload, basis="sto-3g", edition="1.0", fetched="2026-09-06")


def test_hydrogen_has_one_s_shell_with_three_primitives(parsed):
    shells = parsed["elements"]["H"]["shells"]
    assert len(shells) == 1
    assert shells[0]["angular_momentum"] == "S"
    assert len(shells[0]["primitives"]) == 3


def test_exponent_and_coefficient_are_provenance_values(parsed):
    primitive = parsed["elements"]["H"]["shells"][0]["primitives"][0]
    exponent = primitive["exponent"]
    assert exponent["value"] == pytest.approx(3.425250914)
    assert exponent["source"] == "BSE"
    assert exponent["id"] == "basis/sto-3g/H/S/0/exponent"
    coefficient = primitive["coefficients"][0]
    assert coefficient["value"] == pytest.approx(0.1543289671)
    assert coefficient["id"] == "basis/sto-3g/H/S/0/contract/0"


def test_carbon_has_s_s_p_shells(parsed):
    shells = parsed["elements"]["C"]["shells"]
    assert [shell["angular_momentum"] for shell in shells] == ["S", "S", "P"]
    p_first = shells[2]["primitives"][0]["coefficients"][0]
    assert p_first["value"] == pytest.approx(0.0708793)
