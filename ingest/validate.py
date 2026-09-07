"""Data integrity validation (PLAN.md § 7.1, § 8.4): the "no bare numbers" gate.

Rules enforced on every `data/*.json`:
  * every object containing "value" must carry non-empty `source` and
    `edition`, a finite numeric `value` and, if present, a finite
    non-negative `uncertainty`;
  * floats may only appear inside such provenance objects;
  * integers may only appear under structural keys (schema_version, z, a...).

Run:  python -m ingest.validate [data_dir]
Exit: 0 = clean, 1 = violations printed to stdout.
"""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

#: integer-valued structural keys that are not physical measurements
STRUCTURAL_INT_KEYS = frozenset(
    {"schema_version", "z", "a", "n", "l", "mass_number", "atomic_number"}
)


def _is_value_object(node: dict) -> bool:
    return "value" in node


def _is_reference_table(node: dict) -> bool:
    """Bulk reference table with section-level provenance (e.g. bond length
    tables): floats inside `values` inherit source/edition from the wrapper."""
    return (
        "values" in node
        and isinstance(node.get("source"), str)
        and bool(node["source"].strip())
        and isinstance(node.get("edition"), str)
        and bool(node["edition"].strip())
    )


#: exact field set of a provenance Value (mirrors fermiforge_core::value::Value)
KNOWN_VALUE_KEYS = frozenset(
    {
        "value",
        "uncertainty",
        "source",
        "edition",
        "fetched",
        "url",
        "id",
        "method",
        "derived_from",
        "based_on",
    }
)


def _is_finite_number(x) -> bool:
    return isinstance(x, (int, float)) and not isinstance(x, bool) and math.isfinite(x)


def _check_value_object(node: dict, path: str, errors: list[str]) -> None:
    if not _is_finite_number(node["value"]):
        errors.append(f"{path}: 'value' is not a finite number: {node['value']!r}")
    for key in ("source", "edition"):
        field = node.get(key)
        if not isinstance(field, str) or not field.strip():
            errors.append(f"{path}: provenance object missing non-empty '{key}'")
    uncertainty = node.get("uncertainty")
    if uncertainty is not None and (not _is_finite_number(uncertainty) or uncertainty < 0):
        errors.append(f"{path}: invalid uncertainty {uncertainty!r}")
    for key in sorted(set(node) - KNOWN_VALUE_KEYS):
        errors.append(f"{path}: unknown field {key!r} in provenance object")


def _walk(node, path: str, key: str | None, errors: list[str]) -> None:
    if isinstance(node, dict):
        if _is_reference_table(node):
            return
        if _is_value_object(node):
            _check_value_object(node, path, errors)
            # scalar fields of a provenance object are validated above;
            # only nested containers are worth descending into
            for child_key, child in node.items():
                if isinstance(child, (dict, list)):
                    _walk(child, f"{path}.{child_key}", child_key, errors)
            return
        for child_key, child in node.items():
            _walk(child, f"{path}.{child_key}", child_key, errors)
    elif isinstance(node, list):
        for index, item in enumerate(node):
            _walk(item, f"{path}[{index}]", key, errors)
    elif isinstance(node, bool) or node is None or isinstance(node, str):
        return
    elif isinstance(node, float):
        errors.append(f"{path}: bare float outside a provenance object")
    elif isinstance(node, int):
        if key not in STRUCTURAL_INT_KEYS:
            errors.append(f"{path}: bare int under non-structural key {key!r}")


def validate_file(path: Path) -> list[str]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"{path.name}: unreadable: {error}"]
    errors: list[str] = []
    _walk(data, path.name, None, errors)
    if isinstance(data, dict) and data.get("schema_version") != 0:
        errors.append(f"{path.name}: schema_version must be 0, got {data.get('schema_version')!r}")
    return errors


def main(argv: list[str]) -> int:
    data_dir = Path(argv[1]) if len(argv) > 1 else Path(__file__).resolve().parents[1] / "data"
    if not data_dir.is_dir():
        print(f"error: {data_dir} is not a directory")
        return 1
    errors: list[str] = []
    files = sorted(data_dir.glob("*.json"))
    if not files:
        errors.append(f"{data_dir}: no JSON files found")
    for file in files:
        errors.extend(validate_file(file))
    for error in errors:
        print(f"VIOLATION: {error}")
    if errors:
        print(f"{len(errors)} violation(s) in {len(files)} file(s)")
        return 1
    print(f"data integrity OK ({len(files)} files)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
