# Instruments

Instrument types, shared traits, pricing models, and JSON loading for `finstack-valuations`.

The canonical overview and quick-start example live in [`mod.rs`](mod.rs) rustdoc. This file summarizes layout and extension points only.

## Layout

Instruments are grouped by asset class:

```
instruments/
├── common/           # Instrument trait, parameters, models, MC helpers
├── fixed_income/     # Bonds, loans, MBS/CMO, structured credit, …
├── rates/            # IRS, basis swap, cap/floor, swaption, deposit, repo, …
├── credit_derivatives/
├── equity/
├── fx/
├── commodity/
└── exotics/
```

Each instrument module typically contains `types.rs`, pricing logic (`pricer.rs` or `pricing/`), optional `cashflows.rs`, and `metrics/`.

## Core API

All instruments implement [`Instrument`](common/traits/mod.rs):

- `value(market, as_of)` — NPV only
- `price_with_metrics(market, as_of, metrics)` — NPV plus requested `MetricId` values
- `cashflow_schedule` / `dated_cashflows` — schedule output (empty schedules are explicit when there is no residual exposure)

Curve requirements are expressed through [`CurveDependencies`](common/traits/mod.rs) where applicable.

## JSON

Instruments load through `InstrumentEnvelope` (`json_loader.rs`) with schema `finstack.instrument/1`. Unknown fields are rejected at deserialize time.

## Adding an instrument

1. Add the type under the appropriate asset-class directory.
2. Implement `Instrument` and register the pricer in `src/pricer/`.
3. Add a variant to the `InstrumentJson` enum in `json_loader.rs`, then add a
   single line to the `with_instrument_json_registry!` macro (the registry is
   the single source of truth — `into_boxed`, the deserialize tag map, and the
   schema-parity check are all generated from it; no per-site hand edits).
4. Add `InstrumentType` in `src/pricer/mod.rs`.
5. Register metrics in the instrument’s `metrics/mod.rs`.
6. Add tests under `tests/instruments/<name>/` and regenerate schemas when the public JSON shape changes (`mise run rust-gen-schemas`).

Follow patterns in an existing instrument in the same asset class (for example `fixed_income/bond/` or `rates/irs/`).

## Related modules

- [`../metrics/README.md`](../metrics/README.md) — metric IDs and calculators
- [`../pricer/`](../pricer/) — pricer registry (no separate README)
- [`../results/README.md`](../results/README.md) — `ValuationResult`
- [`../calibration/README.md`](../calibration/README.md) — curve calibration
