# Fermiforge

*Build matter from quarks to molecules — and rewrite the laws while you're at it.*

Fermiforge is a client-side construction kit for matter: assemble particles,
atoms and molecules and watch their orbitals, spectra and behaviour in real
time. No physical value is sealed in code — particles, constants and
parameters are editable data, so you can explore not only real physics but
"what if" scenarios. Physics runs in Rust/WASM, rendering in WebGL2, and any
scene state is shareable through the URL — no server, no accounts, no data
storage.

**Status:** milestone M0 (skeleton): physics core crate with provenance-tracked
values, scene state + URL codec, data ingest pipeline and CI. The web app
arrives in M1. See [PLAN.md](PLAN.md) for the full strategy — it is the
single source of truth for human and AI contributors alike.

## Development setup

```sh
git clone ... && cd fermiforge
sh scripts/install-hooks.sh        # pre-commit AI guard (required after clone)
cargo test                         # Rust core
python -m venv .venv && .venv/Scripts/pip install -r ingest/requirements-dev.txt
python -m pytest                   # ingest pipeline tests
python -m ingest.generate --offline  # regenerate data/ from pinned fixtures
```

## Layout

- `core/` — Rust: all physics, zero UI code, zero hardcoded constants
- `data/` — immutable, provenance-tracked JSON snapshots (generated, never edited)
- `ingest/` — Python build-time pipeline: fetch → normalize → validate → PR
- `web/` — Svelte/TS frontend (M1)

## Acknowledgments

This work was supported by the Ministry of Education, Youth and Sports of the
Czech Republic through the e-INFRA CZ (ID: 90140).
