# Golden Tests

Golden fixtures pin Finstack Quant's valuation outputs against externally sourced
reference values (Bloomberg screens, QuantLib, closed-form formulas). Fixtures
live under `data/` and use the strict `finstack_quant.golden/2` schema. Both the Rust
runner (`tests/golden/`) and the Python bindings layer
(`finstack-quant-py/tests/golden/`) load the same JSON files and must agree.

## Schema (`finstack_quant.golden/2`)

Each fixture is one JSON object with exactly these top-level sections:

```jsonc
{
  "schema_version": "finstack_quant.golden/2",
  "metadata": { /* identity, provenance, valuation date, screenshots */ },
  "kind": "pricing",            // or "sabr_smile"
  /* ...kind-specific body fields (see below)... */
  "expected":   { "npv": 1039530.56, "dv01": -333.0 },
  "tolerances": { "npv": { "abs": 0.01, "tolerance_reason": "..." }, "dv01": { "abs": 1.0 } }
}
```

Unknown top-level keys and unknown `metadata` keys are rejected. The canonical
struct definitions are in [`schema.rs`](schema.rs); the Python mirror is in
[`finstack-quant-py/tests/golden/schema.py`](../../../../finstack-quant-py/tests/golden/schema.py).

### `metadata`

`name`, `domain`, `description`, `valuation_date` (YYYY-MM-DD), `source`
(`quantlib` | `bloomberg-api` | `bloomberg-screen` | `intex` | `formula` |
`textbook`), `source_detail`, `captured_by`, `captured_on`, `last_reviewed_by`,
`last_reviewed_on`, `review_interval_months`, `regen_command`, and optional
`screenshots`. `bloomberg-screen` and `intex` sources require at least one
screenshot under a `screenshots/` directory next to the fixture.

### `kind: "pricing"`

Instrument-pricing body:

- `model` — pricing model selector (`discounting`, `tree`, `hull_white_1f`, …).
- `market` — a tagged union, exactly one of:
  - `{ "kind": "snapshot", "data": { /* MarketContext */ } }`
  - `{ "kind": "envelope", "envelope": { /* CalibrationEnvelope */ } }`
- `instrument` — a `finstack_quant.instrument/1` envelope.

The requested metric list is **derived** from the `expected` keys (each key's
base name before `::`, excluding `npv`); there is no separate `metrics` field.

### `kind: "sabr_smile"`

Closed-form SABR smile body: `alpha`, `beta`, `nu`, `rho`, optional `shift`,
`forward`, `time_to_expiry`, and `strikes` (`[{ "key": "...", "strike": ... }]`).
The strike keys must match the `expected` keys exactly. SABR fixtures live under
`data/market_data/sabr/` and run through their own entry point in `sabr.rs`.

### `expected` and `tolerances`

`expected` holds the externally sourced reference values and is the single
source of truth — tests never rewrite it to match model output. `tolerances`
must cover every `expected` metric, each with an `abs` and/or `rel` bound; a
`tolerance_reason` is required for any expected risk metric that is exactly zero.

## Non-executable fixtures

A few Bloomberg fixtures retain screen values that the executable runner cannot
yet reproduce. Rather than encode that status in each fixture, both layers read
one shared allowlist, [`known_non_executable.json`](known_non_executable.json)
(`{ "path": ..., "reason": ... }` entries, paths relative to `data/`). The Rust
runner ([`pricing.rs`](pricing.rs)) reports their failures non-fatally; the
Python layer turns each into `pytest.mark.xfail(strict=False)` via
`discover_fixtures_with_marks` in `conftest.py`. Add a fixture to that one file
to park it on both sides.

To run every fixture strictly and surface the parked failures, set
`GOLDEN_IGNORE_NON_EXECUTABLE=1` (both layers honor it) or run
`mise run goldens-test-strict`.

## Migrating fixtures

The one-off v1→v2 migration script lives at
[`scripts/golden/migrate_v1_to_v2.py`](../../../../scripts/golden/migrate_v1_to_v2.py)
(`uv run scripts/golden/migrate_v1_to_v2.py [--check]`).
