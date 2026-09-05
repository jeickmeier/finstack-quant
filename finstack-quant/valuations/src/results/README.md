# Results

`ValuationResult` is the standard pricing output: present value, requested
measures, policy metadata, optional model detail, optional covenant reports, and
an optional explanation trace.

## Types

`ValuationResult` ([`valuation_result.rs`](valuation_result.rs)) is
`Serialize + Deserialize + JsonSchema` with `deny_unknown_fields`.

| Field | Type | Content |
|-------|------|---------|
| `schema_version` | `SchemaVersion` | Wire-format version; only numeric `1` is accepted |
| `instrument_id` | `String` | Identifier of the priced instrument |
| `as_of` | `Date` | Valuation date (T+0), wire-encoded as an ISO date |
| `value` | `Money` | PV in the instrument's native currency. Never present in `measures` |
| `measures` | `IndexMap<MetricId, f64>` | Requested metrics, in request order. Units follow the `MetricId` contract |
| `details` | `Option<ValuationDetails>` | Model-specific detail for FX, credit derivatives, composite instruments, structured-credit stochastic runs, and Monte Carlo valuations |
| `meta` | `ResultsMeta` | Re-exported from `finstack_quant_core::config`: numeric mode, rounding context, FX policy, timing |
| `covenants` | `Option<IndexMap<String, CovenantReport>>` | Present for loans and structured credit |
| `explanation` | `Option<ExplanationTrace>` | Step trace, enabled via `ExplainOpts` |

`ValuationRow` ([`dataframe.rs`](dataframe.rs)) is the flat export row:
`instrument_id`, `as_of_date` (ISO string), `pv`, `currency`, and a
`#[serde(flatten)]` `IndexMap<String, f64>` of every measure keyed by its metric
string. No metric is special-cased or renamed, and measure insertion order is
preserved.

## Construction

Pricers normally build results; construct them directly only for tests or custom
pricers.

```rust
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::results::ValuationResult;
use indexmap::IndexMap;
use time::macros::date;

let as_of = date!(2025 - 01 - 15);
let pv = Money::new(1_000_000.0, Currency::USD)?;

let mut measures = IndexMap::new();
measures.insert(MetricId::Dv01, -1_250.0);
measures.insert(MetricId::Ytm, 0.0425);

let result = ValuationResult::stamped("BOND-001", as_of, pv).with_measures(measures);

assert_eq!(result.metric(MetricId::Ytm), Some(0.0425));
```

Constructors and builders:

| Method | Use |
|--------|-----|
| `stamped(instrument_id, as_of, value)` | Default config metadata |
| `stamped_with_config(...)` | Stamp from an explicit `FinstackConfig` |
| `stamped_with_meta(...)` | Reuse a prebuilt `ResultsMeta` — build it once per batch with `finstack_quant_core::config::results_meta(&config)` to avoid re-deriving config per instrument |
| `with_measures`, `with_details`, `with_explanation` | Attach payloads |
| `with_covenants`, `with_covenant(key, report)` | Attach covenant reports |

Accessors: `metric(MetricId)`, `metric_str(&str)`, `metric_series(&MetricId)`
(bucketed series read back as `(labels, value)` pairs), `all_covenants_passed()`,
and `failed_covenants()`.

`MonteCarloValuationDetails` reports the model key, sampling standard error,
configured independent exercise-policy paths, configured independent
make-whole-reference paths, estimator paths, each stage's total simulated path
count including antithetic partners, seed, time grid, and variance-reduction
flags. For LSMC valuations, the standard error covers pricing-path sampling
uncertainty under the frozen fitted exercise policy; it excludes regression
approximation, time-grid discretization, and model error. A stage that did not
run reports zero paths; a scalar engine leaves
`details` absent.

## FX policy metadata

`ResultsMeta::fx_policy_applied` records which FX conversion or cross-currency
assumption fed a valuation. When `PricerRegistry` stamps metadata the precedence
is:

1. A policy already set on `result.meta.fx_policy_applied` by the instrument
   pricer — for example an FX option stamping its conversion direction.
2. Otherwise, the `fx_policy` stamp on any discount, forward, or hazard curve
   the instrument declares in `market_dependencies()`. Stamps are de-duplicated
   in dependency order (discount, then forward, then credit) and joined with
   `" | "`.
3. Otherwise `None`.

Curve stamps are opaque strings set at calibration time or via the curve builder
(`DiscountCurve::builder(..).fx_policy(..)`). This is how a single-currency
instrument discounted off a cross-currency curve inherits the policy without the
pricer restating it.

## Covenants

```rust
let result = ValuationResult::stamped("LOAN-001", as_of, pv)
    .with_covenant("dscr_test", dscr_report);

assert!(result.all_covenants_passed());
assert!(result.failed_covenants().is_empty());
```

`CovenantReport` comes from [`finstack-quant-covenants`](../../../covenants/README.md).

## Export

- `ValuationResult::to_row()` — one `ValuationRow`

Rows serialize to a flat JSON object suitable for pandas/Polars construction
downstream; measure keys are the `MetricId` strings, so they stay stable across
releases. This crate has no DataFrame dependency of its own.

## Flow

1. `Instrument::price_with_metrics` (or a pricer via `PricerRegistry`) produces a
   `ValuationResult`.
2. Portfolio code scales by quantity and converts to base currency, carrying the
   stamped FX policy forward.
3. Reporting flattens via `ValuationRow` or a custom exporter.

## Verification

```bash
mise run rust-test-filter -- finstack-quant-valuations results
mise run gen-check   # ValuationResult schema lives in ../../schemas/results/
```

## Related

- [`../metrics/README.md`](../metrics/README.md) — the `MetricId` contract
- [`../instruments/README.md`](../instruments/README.md) — pricing entry points
- [`../../schemas/README.md`](../../schemas/README.md) — generated JSON Schema artifacts
