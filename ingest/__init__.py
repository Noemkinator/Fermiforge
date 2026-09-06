"""Build-time data ingest pipeline (PLAN.md § 7).

Fetch -> normalize -> validate -> write immutable `data/` snapshots.
Runtime never calls this package; the app ships the snapshots offline.
"""
