# finstack-quant-attribution

Multi-period P&L attribution for individual instruments. Decomposes mark-to-market
change between two dates (T₀ → T₁) into contributions from carry, rates curves,
credit curves, inflation, correlations, FX, volatility, model parameters, and
market scalars.

Attribution is layered by cost and fidelity. The lightest entry points reprice
once per date, the heaviest perform per-factor bump-and-reprice loops. Pick the
cheapest method that answers your question — every additional tier adds
repricing cost and operational moving parts.

## Methodologies

| Tier         | Entry point                                                | Behavior                                                                                       |
|--------------|------------------------------------------------------------|------------------------------------------------------------------------------------------------|
| Minimal      | [`pnl_bridge`](src/lib.rs)                          | Scalar `value(T₁) − value(T₀)` in target currency. No decomposition.                          |
| Linear       | [`attribute_pnl_metrics_based`](src/metrics_based/)      | Linear (and optional second-order) approximation from precomputed metrics. No extra repricing. |
| Parallel     | [`attribute_pnl`](src/lib.rs) + `AttributionMethod::Parallel` | Isolate one factor at a time (T₀ for that factor, T₁ elsewhere). Residual carries cross-effects. |
| Waterfall    | [`attribute_pnl`](src/lib.rs) + `AttributionMethod::Waterfall` | Apply factors in order; per-factor P&Ls sum to total P&L up to tolerance. Order matters.       |
| Taylor       | [`attribute_pnl`](src/lib.rs) + `AttributionMethod::Taylor` | First- and optional second-order sensitivity expansion from bump-and-reprice Greeks; FX, inflation, correlations, scalars, and model parameters are isolated by restore-and-reprice. |

`AttributionMethod` selects among the four decomposition methods when
dispatching through a spec: `Parallel` (the `Default`),
`Waterfall(Vec<AttributionFactor>)`, `MetricsBased`, and
`Taylor(TaylorAttributionConfig)`.

Precomputed `Theta` and `CarryTotal` must use the attribution window's
`theta_period_days`; carry metrics are not rescaled across coupon payments.
Supplying net `CarryTotal` also requires `FundingCost` (explicit zero when
unfunded), so attribution can recover gross carry consistently with endpoint P&L.
The spec executor requests the matching horizon automatically. Direct callers
must recompute metrics for their window (an omitted stamp assumes one day).

Default waterfall order (from [`default_waterfall_order`](src/waterfall.rs)):

```text
Carry → RatesCurves → CreditCurves → InflationCurves → Correlations
      → Fx → Volatility → ModelParameters → MarketScalars
```

Separately, [`attribute_return_contribution`](src/return_contribution.rs)
performs single-period *return* contribution (weight × return roll-ups by
group, with optional factor exposures and a benchmark-relative view). It is a
weights-and-returns calculation on plain `f64` rows, not a repricing method —
it never touches instruments or market contexts.

## Factors

`AttributionFactor` in [`types/result.rs`](src/types/result.rs) enumerates the
nine factor families. Each populates a top-level field on `PnlAttribution` (`carry`,
`rates_curves_pnl`, …) and, when requested, an optional `*_detail` struct with
finer breakdowns:

- **Carry** — theta, accrual, pull-to-par, financing.
- **RatesCurves** — per-curve and optional per-tenor IR risk
  (`RatesCurvesAttribution`).
- **CreditCurves** — per-hazard-curve spread P&L, with optional generic /
  per-level / adder decomposition via a calibrated `CreditFactorModel`
  (`CreditFactorAttribution`).
- **InflationCurves** — CPI curve moves and published inflation index changes.
- **Correlations** — base correlation curve changes for structured credit.
- **Fx** — spot FX revaluation in the target reporting currency.
- **Volatility** — implied-vol surface moves (`VolAttribution`); parallel and
  waterfall also classify declared scalar volatility quotes here.
- **ModelParameters** — prepayment, default, recovery, conversion-policy and
  other model inputs snapshotted via
  `finstack_quant_valuations::instruments::model_params::ModelParamsSnapshot`.
- **MarketScalars** — dividends and equity/commodity spots.

## Layout

```text
attribution/src/
├── lib.rs                  # Module docs, AttributionRequest, attribute_pnl, pnl_bridge
├── types.rs                # types module declaration
├── types/result.rs         # AttributionFactor, AttributionMethod, ExecutionPolicy,
│                           #   PnlAttribution, AttributionMeta
├── types/detail.rs         # Per-factor *Detail / *Attribution structs
├── factors.rs              # MarketSnapshot, restore flags, per-factor market mutation
├── helpers.rs              # reprice_instrument, compute_pnl, compute_pnl_with_fx
├── parallel.rs             # parallel method
├── waterfall.rs            # waterfall method, default_waterfall_order
├── metrics_based/          # attribute_pnl_metrics_based (linear from metrics)
├── taylor.rs               # Taylor method, TaylorAttributionConfig
├── model_params.rs         # extract/replace model params, measure_*_shift
├── credit_factor.rs        # Credit-factor detail options and model identifiers
├── credit_cascade.rs       # Waterfall credit-factor cascade
├── credit_decomposition.rs # Generic / per-level / adder decomposition
├── execution.rs            # AttributionSpec::execute dispatcher
├── return_contribution.rs  # Single-period weight × return contribution
├── long_rows.rs            # PnlAttribution → long-format and wide-row projections
├── target_currency.rs      # translate_to_target_currency (native → reporting currency)
├── schema.rs               # Published JSON Schema artifacts
├── spec.rs                 # JSON envelope, AttributionSpec, AttributionResult
└── bin/gen_schemas.rs      # gen_attribution_schemas generator binary
```

## Position in the stack

Depends on `finstack-quant-core`, `finstack-quant-cashflows`,
`finstack-quant-models`, and `finstack-quant-valuations`. Consumed by
`finstack-quant-portfolio`, which rolls per-instrument `PnlAttribution` values
into book-level views, and by `finstack-quant-scenarios`. Re-exported by the
umbrella crate as `finstack_quant::attribution`.

Import path uses underscores:

```rust
use finstack_quant_attribution::{
    attribute_pnl, default_waterfall_order, AttributionFactor, AttributionMethod,
    AttributionRequest, ExecutionPolicy, PnlAttribution,
};
```

## Quick start

A runnable parallel-attribution example, plus sign / carry / residual
conventions, lives in the crate rustdoc
(`cargo doc -p finstack-quant-attribution --open`).

Metrics-based attribution needs `ValuationResult`s priced at both dates with
the metrics in [`default_attribution_metrics`](src/spec.rs) (or a caller-chosen
subset). When parallel or waterfall runs request curve detail, optional
`rates_detail` exposes per-`(curve_id, tenor)` P&L.

## JSON specification

[`AttributionEnvelope`](src/spec.rs) / [`AttributionSpec`](src/spec.rs) define a
schema-versioned JSON contract used by bindings and batch pipelines. A spec
carries an `InstrumentJson` payload, two `MarketContextState` snapshots, both
`as_of` dates, the `AttributionMethod`, and optional `model_params_t0`, config,
and credit-factor-model overrides. Both types are
`#[serde(deny_unknown_fields)]`.

The version marker is the `AttributionSchema` enum, whose single variant
serializes as the string constant `ATTRIBUTION_SCHEMA`
(`"finstack_quant.attribution/1"`); an unrecognized marker is rejected during
deserialization.

```rust,ignore
use finstack_quant_attribution::{AttributionEnvelope, AttributionSchema};

let envelope: AttributionEnvelope = serde_json::from_str(&json)?;
assert_eq!(envelope.schema, AttributionSchema::CURRENT);

let result_envelope = envelope.execute()?;
let result = &result_envelope.result; // AttributionResult { attribution, results_meta }
```

`AttributionSpec::from_json_inputs` is the binding-friendly constructor used by
the Python and WASM layers, and `validate_attribution_json` checks a payload
without executing it.

Two schema artifacts are checked in under
[`schemas/attribution/1/`](schemas/attribution/1) — `attribution.schema.json`
(input) and `attribution_result.schema.json` (output) — indexed by
[`schemas/index.json`](schemas/index.json). Regenerate with
`mise run rust-gen-schemas`; verify with `mise run rust-check-schemas`.

## Public API

| Item                                                                              | Module           | Notes                                       |
|-----------------------------------------------------------------------------------|------------------|---------------------------------------------|
| `pnl_bridge`                                                               | `lib`            | Total P&L, no decomposition                 |
| `attribute_pnl` + `AttributionMethod::Parallel`                                   | `parallel`       | Factor isolation, residual reports cross-effects |
| `attribute_pnl` + `AttributionMethod::Waterfall`, `default_waterfall_order`       | `waterfall`      | Sum-preserving ordered decomposition        |
| `attribute_pnl_metrics_based`                                                     | `metrics_based`  | Linear approximation from precomputed metrics |
| `attribute_pnl` + `AttributionMethod::Taylor`, `TaylorAttributionConfig`          | `taylor`         | Sensitivity-based expansion mapped to `PnlAttribution` |
| `PnlAttribution`, `AttributionFactor`, `AttributionMethod`, `AttributionMeta`     | `types`          | Result envelope and factor enums            |
| `CarryDetail`, `RatesCurvesAttribution`, `CreditCurvesAttribution`, `CreditFactorAttribution`, `InflationCurvesAttribution`, `CorrelationsAttribution`, `FxAttribution`, `VolAttribution`, `ModelParamsAttribution`, `ScalarsAttribution`, `CrossFactorDetail`, `CreditCarryDecomposition`, `CreditCarryByLevel`, `LevelCarry`, `LevelPnl`, `SourceLine` | `types` | Per-factor detail structs                   |
| `translate_to_target_currency`                                                         | `target_currency`     | Post-hoc translation of a native-currency `PnlAttribution` into a reporting currency, adding `fx_translation_pnl` |
| `AttributionJsonInputs` | `spec` | Binding-friendly JSON fragments for `AttributionSpec::from_json_inputs` |
| `pnl_attribution_wide_row`, `PnlAttributionWideRow` | `long_rows` | Single-row aggregate projection used by Python `to_dataframe` |
| `CreditFactorDetailOptions` | `credit_factor` | Controls optional per-issuer and per-bucket detail emitted by the canonical attribution methods |
| `AttributionEnvelope`, `AttributionSpec`, `AttributionJsonInputs`, `AttributionSchema`, `AttributionConfig`, `AttributionResult`, `AttributionResultEnvelope`, `ATTRIBUTION_SCHEMA`, `default_attribution_metrics`, `validate_attribution_json` | `spec` | JSON contract |
| `attribute_return_contribution`, `attribute_return_contribution_json`, `validate_return_contribution_json`, `ReturnContributionSpec`, `ReturnContributionResult`, `ReturnContributionPosition`, `ReturnContributionFactor`, `ReturnContributionWeighting`, `InstrumentContribution`, `GroupContribution`, `FactorContribution`, `BenchmarkRelativeContribution` | `return_contribution` | Single-period weight × return contribution |
| `pnl_attribution_long_rows`, `pnl_attribution_carry_rows`, `pnl_attribution_credit_factor_rows`, `LongDetailRow` | `long_rows` | Long-format projection of a `PnlAttribution`, consumed by the Python DataFrame exports |
| `ARTIFACTS`, `ATTRIBUTION_SCHEMA_BASE` | `schema` | Published JSON Schema artifacts and their base URI |

`long_rows` and `schema` are the crate's only public submodules. Every other
module in the layout above is `pub(crate)`; the `Module` column names where an
item is defined, not an importable path — those items reach callers through the
crate-root re-exports in [`lib.rs`](src/lib.rs).

Sign, carry, currency, and residual conventions are documented in the crate
rustdoc.

## Numerical behavior

- All four methodologies guard against missing curves/surfaces and report
  zero contribution rather than panicking when a factor is absent from both
  market snapshots.
- Per-factor bump-and-reprice paths in `parallel`, `waterfall`, and `taylor`
  reuse a single `MarketSnapshot` and apply targeted restore flags
  (`MarketRestoreFlags`) to avoid full-context cloning.
- `ExecutionPolicy::Serial` is the default for parallel and Taylor attribution.
  Opt into `ExecutionPolicy::Parallel` only when the caller is not already
  parallelizing an outer portfolio or batch loop, so the outer position loop
  owns Rayon and avoids nested thread-pool contention.
- Taylor attribution uses central differences by default; bump sizes are
  configurable via `TaylorAttributionConfig`.
- Strict mode is the spec/execution default (`AttributionConfig::strict_validation`
  omitted or `true`): per-factor pricing errors propagate so official reports
  fail closed. Set `strict_validation = false` only for diagnostic runs; those
  log the failure via `tracing` and zero the factor into residual.
- Output rounding follows `FinstackConfig::rounding`; `AttributionConfig::rounding_scale`
  overrides the per-currency scale for a single run.

## Extending

Adding a new factor requires coordinated updates to:

1. `AttributionFactor` and `PnlAttribution` in
   [`types/result.rs`](src/types/result.rs), plus any detail struct in
   [`types/detail.rs`](src/types/detail.rs).
2. The factor-isolation / restore logic in [`factors.rs`](src/factors.rs).
3. All four methodology modules (`parallel`, `waterfall`, `metrics_based`,
   `taylor`).
4. `default_waterfall_order` in [`waterfall.rs`](src/waterfall.rs).
5. The long-row projection in [`long_rows.rs`](src/long_rows.rs).
6. The regenerated schemas under [`schemas/attribution/1/`](schemas/attribution/1)
   and the contract tests under [`tests/attribution/`](tests/attribution).

Follow an existing factor (e.g. `Fx` or `Volatility`) end-to-end as a template.

## Bindings

Hosts reach attribution through string-dispatched and JSON entry points; the
large typed decomposition API is deliberately Rust-only.

- **Python** — `finstack_quant.attribution` binds `attribute_pnl`,
  `attribute_pnl_envelope_json`, `validate_attribution_json`,
  `attribute_return_contribution`, `validate_return_contribution_json`, the
  `default_waterfall_order` / `default_attribution_metrics` helpers, and the
  `PnlAttribution` / `ReturnContributionResult` wrappers. Detail structs are
  reachable as serde payloads on `PnlAttribution` detail getters.
  `finstack_quant.attribution.schema` exposes `index` / `get` / `validate`.
- **WASM** — [`exports/attribution.js`](../../finstack-quant-wasm/exports/attribution.js)
  exposes `attributePnl`, `attributePnlJson`, `attributePnlJson`,
  `validateAttributionJson`, `defaultWaterfallOrder`,
  `defaultAttributionMetrics`, and the `AttributionParams` helper class. There
  is no WASM twin for the schema module or for return contribution.

The authoritative contract, including the full Rust-only inventory, is
[`parity_contract.toml`](../../finstack-quant-py/parity_contract.toml)
(`[crates.attribution]`, `[wasm_attribution_subset]`).

## Related

- [`finstack-quant-valuations`](../valuations/README.md) — instrument repricing used at T₀ and T₁.
- [`finstack-quant-cashflows`](../cashflows/README.md) — accrual and carry inputs.
- [`finstack-quant-models`](../models/README.md) — calibrated credit-factor models consumed via `models::factor::CreditFactorModel`.
- [`finstack-quant-portfolio`](../portfolio/README.md) — aggregates per-instrument `PnlAttribution`s into book-level views.

## Tests and benchmarks

| Path | Contents |
|------|----------|
| [`tests/attribution.rs`](tests/attribution.rs) | Aggregator for the [`tests/attribution/`](tests/attribution) tree: invariants, per-factor suites, QuantLib parity, schema and serialization contracts, rounding policy |
| [`tests/market_restore.rs`](tests/market_restore.rs) | `MarketSnapshot` / `MarketRestoreFlags` round-trip behavior |
| [`tests/cross_factor_attribution_tests.rs`](tests/cross_factor_attribution_tests.rs) | Cross-factor / interaction residual behavior |
| [`tests/credit_carry_split.rs`](tests/credit_carry_split.rs) | Credit carry decomposition split |
| [`benches/attribution.rs`](benches/attribution.rs) | Per-method cost against the `pnl_bridge` baseline |
| [`benches/attribution_scale.rs`](benches/attribution_scale.rs) | Scaling with factor and tenor count |

## References

Entries live in [`docs/REFERENCES.md`](../../docs/REFERENCES.md):

- Fixed-income sensitivity intuition —
  [`#tuckman-serrat-fixed-income`](../../docs/REFERENCES.md#tuckman-serrat-fixed-income)
- Risk decomposition and factor attribution —
  [`#meucci-risk-and-asset-allocation`](../../docs/REFERENCES.md#meucci-risk-and-asset-allocation)

## Verification

```bash
cargo clippy -p finstack-quant-attribution --lib --bins --tests --examples --all-features -- -D warnings
cargo nextest run -p finstack-quant-attribution --lib --test '*'
cargo bench -p finstack-quant-attribution --bench attribution
```

Workspace gates (`mise run rust-lint`, `mise run rust-test`, `mise run rust-doc`
— the last one runs doctests) are what CI enforces. Use `cargo nextest`, not
`cargo test`, for crate-scoped runs; see
[`CONTRIBUTING.md`](../../CONTRIBUTING.md).

Carry and endpoint P&L are gross of financing on every methodology. Metrics-based
attribution adds the reported `funding_cost` back to the net `carry_total` metric;
funding remains a separate detail. Supplied net carry metrics must include funding cost (zero when unfunded). Rates and credit source lines partition gross
carry. Scalar volatility dependencies belong to volatility P&L and are excluded
from generic market-scalar P&L.
