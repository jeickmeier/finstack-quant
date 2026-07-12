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

## Metric-specific unresolved comparisons

A few externally sourced metrics retain vendor values that the executable
runner cannot yet reproduce. Both layers read one strict shared file,
[`known_non_executable.json`](known_non_executable.json), with paths relative
to `data/`:

```jsonc
{
  "description": "why this allowlist exists",
  "fixtures": [
    {
      "path": "pricing/example/vendor_fixture.json",
      "description": "fixture-level benchmark summary",
      "metrics": [
        {
          "metric": "dv01",
          "reason": "why the vendor target must remain authoritative",
          "evidence": "exact expected, actual, difference, and tolerance"
        }
      ]
    }
  ]
}
```

The schema is closed at the root, fixture, and metric levels. Unknown fields,
invalid types, blank required strings, duplicate fixture paths, and duplicate
metric keys are fatal. Every path must resolve to a valid fixture and every
listed metric must exist in that fixture's `expected` map.

Allowlisting is comparison-specific, never fixture-wide:

- fixture loading, schema validation, and execution errors remain fatal;
- metrics not listed remain strictly asserted;
- a listed metric mismatch is emitted as an expected-unresolved diagnostic;
- a listed metric that now passes is a stale-entry failure;
- missing fixtures or metrics are stale-entry failures.

The Rust pricing walk implements this in `known_non_executable`,
`unresolved_metrics_for_path`, and `classify_fixture_run` in
[`pricing.rs`](pricing.rs). Python mirrors it in
`_parse_unresolved_allowlist`, `_known_unresolved_metrics`, and `run_golden` in
`finstack-quant-py/tests/golden/conftest.py`. Python does not use whole-test
`xfail` markers.

Strict mode is enabled only when `GOLDEN_IGNORE_NON_EXECUTABLE` is one of
`1`, `true`, `yes`, or `on` (case-insensitive, surrounding whitespace ignored).
Absent, empty, `0`, and `false` do not enable strict mode. Use
`GOLDEN_IGNORE_NON_EXECUTABLE=1` or `mise run goldens-test-strict` to surface
every unresolved metric as a normal failure.

## Migrating fixtures

The one-off v1→v2 migration script lives at
[`scripts/golden/migrate_v1_to_v2.py`](../../../../scripts/golden/migrate_v1_to_v2.py)
(`uv run scripts/golden/migrate_v1_to_v2.py [--check]`).
