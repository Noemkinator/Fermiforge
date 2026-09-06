import json
from pathlib import Path

from ingest import validate


def write(tmp_path, name: str, obj) -> Path:
    path = tmp_path / name
    path.write_text(json.dumps(obj), encoding="utf-8")
    return path


def test_provenance_object_passes(tmp_path):
    path = write(
        tmp_path,
        "a.json",
        {
            "schema_version": 0,
            "values": {"x": {"value": 1.0, "uncertainty": 0.1, "source": "PDG", "edition": "2024"}},
        },
    )
    assert validate.validate_file(path) == []


def test_bare_float_fails(tmp_path):
    path = write(tmp_path, "a.json", {"schema_version": 0, "mass": 0.511})
    errors = validate.validate_file(path)
    assert any("bare float" in error for error in errors)


def test_missing_source_fails(tmp_path):
    path = write(
        tmp_path,
        "a.json",
        {"schema_version": 0, "values": {"x": {"value": 1.0, "edition": "2024"}}},
    )
    errors = validate.validate_file(path)
    assert any("source" in error for error in errors)


def test_bare_int_outside_structural_keys_fails(tmp_path):
    path = write(tmp_path, "a.json", {"schema_version": 0, "count": 5})
    errors = validate.validate_file(path)
    assert any("bare int" in error for error in errors)


def test_structural_ints_pass(tmp_path):
    path = write(tmp_path, "a.json", {"schema_version": 0, "z": 82, "a": 207})
    assert validate.validate_file(path) == []


def test_wrong_schema_version_fails(tmp_path):
    path = write(tmp_path, "a.json", {"schema_version": 1})
    errors = validate.validate_file(path)
    assert any("schema_version" in error for error in errors)
