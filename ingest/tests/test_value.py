import pytest

from ingest.value import Value, ValueSourceError


def test_from_source_requires_source_and_edition():
    with pytest.raises(ValueSourceError):
        Value.from_source(1.0, source="", edition="2024")
    with pytest.raises(ValueSourceError):
        Value.from_source(1.0, source="PDG", edition="  ")


def test_from_source_rejects_non_finite():
    for bad in (float("nan"), float("inf"), float("-inf")):
        with pytest.raises(ValueSourceError):
            Value.from_source(bad, source="PDG", edition="2024")
    with pytest.raises(ValueSourceError):
        Value.from_source(1.0, source="PDG", edition="2024", uncertainty=-1.0)


def test_to_dict_omits_empty_fields():
    v = Value.from_source(1.5, source="PDG", edition="2024", id="x/y")
    assert v.to_dict() == {"value": 1.5, "source": "PDG", "edition": "2024", "id": "x/y"}


def test_to_dict_keeps_full_provenance():
    v = Value.from_source(
        2.16,
        source="PDG",
        edition="2024",
        uncertainty=0.49,
        fetched="2026-02-14",
        url="https://example.test",
        id="particles/up/mass_MeV",
    )
    d = v.to_dict()
    assert d["uncertainty"] == 0.49
    assert d["fetched"] == "2026-02-14"
    assert d["url"] == "https://example.test"
