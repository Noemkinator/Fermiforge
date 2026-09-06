# `data/` — immutable data snapshots

Every file here is **generated** by the ingest pipeline (`python -m ingest.generate`)
and committed as an immutable, versioned snapshot. Never edit these files by
hand; change the ingest and regenerate (PLAN.md § 7).

## Schema versioning rule (PLAN.md § 7.4)

- Every file carries `schema_version` (currently `0`).
- **Adding fields is allowed; changing the meaning of existing fields is not.**
  A meaning change requires a major version bump plus a migration.
- Snapshots are immutable: an update replaces the file wholesale, the old
  version stays in git history and is citable via its commit.

## The "no bare numbers" rule (PLAN.md § 7.1)

Every physical number is a provenance object:

```json
{ "value": 2.16, "uncertainty": 0.49, "source": "PDG", "edition": "2024",
  "fetched": "2026-09-06", "url": "https://pdgapi.lbl.gov/...",
  "id": "particles/up/mass_MeV" }
```

Enforced by `python -m ingest.validate` (CI `data-integrity` job):

- floats may only appear as the `value`/`uncertainty` of a provenance object
  with non-empty `source` and `edition`;
- integers only under structural keys (`schema_version`, `z`, `a`, `n`, `l`);
- provenance objects may only contain the fields defined by the `Value` type.

## Files

| File | Source | Edition | Content |
|---|---|---|---|
| `particles.json` | PDG REST API | 2024 | lepton/quark/hadron masses, MeV |
| `constants.json` | CODATA via `scipy.constants` | CODATA 2022 | α, mass equivalents, MeV |
| `basis_sto3g.json` | Basis Set Exchange | 1.0 | STO-3G exponents/coefficients (H, C) |
| `nuclei.json` | IAEA LiveChart / AME | AME2020 | mass excess + binding energy, MeV |
| `manifest.json` | — | — | per-file source/edition/fetched/sha256 |

## Status of the current snapshot

The committed snapshot was generated with `--offline` from the fixture-pinned
API responses (`ingest/tests/fixtures/`) on 2026-09-06. The first live run of
the monthly `data-update.yml` cron (or a manual `workflow_dispatch`) replaces
it with freshly fetched upstream values; PDG asymmetric uncertainties are
stored as the symmetric max until the data model gains asymmetric support
(documented limitation, PLAN.md § 13.8).
