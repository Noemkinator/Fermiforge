from ingest.sources import models


def test_parameters_carry_method_and_reference():
    values = models.parse(fetched="2026-09-06")
    assert len(values) == len(models.PARAMETERS)
    for v in values:
        assert v.method is not None and "approximation" in v.method
        assert v.url is not None and v.url.startswith("https://")
        assert v.source == "literature"


def test_bond_cutoff_value():
    values = {v.id: v for v in models.parse(fetched="2026-09-06")}
    cutoff = values["models/huckel/bond_cutoff_angstrom"]
    assert cutoff.value == 1.6
