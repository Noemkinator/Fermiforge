"""Shared Value definition: the only entry point for physical data (PLAN.md § 7.1).

Mirrors `fermiforge_core::value::Value` in Rust. Field order matches the
Rust serde representation so snapshots are byte-compatible across languages.
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field


class ValueSourceError(ValueError):
    """Raised when provenance is missing or a value is not finite."""


@dataclass
class Value:
    value: float
    source: str
    edition: str
    uncertainty: float | None = None
    fetched: str | None = None
    url: str | None = None
    id: str | None = None
    method: str | None = None
    derived_from: list[str] = field(default_factory=list)
    based_on: str | None = None

    @classmethod
    def from_source(
        cls,
        value: float,
        *,
        source: str,
        edition: str,
        uncertainty: float | None = None,
        fetched: str | None = None,
        url: str | None = None,
        id: str | None = None,
    ) -> Value:
        """The only path for raw input data ("no bare numbers", PLAN.md § 7.1)."""
        if not source or not source.strip():
            raise ValueSourceError("missing provenance: source")
        if not edition or not edition.strip():
            raise ValueSourceError("missing provenance: edition")
        if not isinstance(value, (int, float)) or not math.isfinite(value):
            raise ValueSourceError(f"non-finite value: {value!r}")
        if uncertainty is not None and (
            not isinstance(uncertainty, (int, float))
            or not math.isfinite(uncertainty)
            or uncertainty < 0
        ):
            raise ValueSourceError(f"invalid uncertainty: {uncertainty!r}")
        return cls(
            value=float(value),
            source=source,
            edition=edition,
            uncertainty=None if uncertainty is None else float(uncertainty),
            fetched=fetched,
            url=url,
            id=id,
        )

    def to_dict(self) -> dict:
        out: dict = {"value": self.value}
        if self.uncertainty is not None:
            out["uncertainty"] = self.uncertainty
        out["source"] = self.source
        out["edition"] = self.edition
        for key in ("fetched", "url", "id", "method"):
            item = getattr(self, key)
            if item is not None:
                out[key] = item
        if self.derived_from:
            out["derived_from"] = list(self.derived_from)
        if self.based_on is not None:
            out["based_on"] = self.based_on
        return out
