# Persisted Contracts

Rust serde types are the source of truth for every JSON contract in this
workspace. Checked-in JSON Schemas are deterministic publication artifacts:

```text
runtime Serialize + Deserialize type
    -> JsonSchema derive
    -> crate registry
    -> deterministic emitter
    -> checked-in *.schema.json
```

Schema files, examples, bindings, and documentation never define a second
contract. The pre-release policy is documented in
[`SERDE_STABILITY.md`](SERDE_STABILITY.md).

## Contract rules

- Namespaced roots require an exact `schema: "finstack_quant.<contract>/1"`
  marker represented by a typed Rust enum.
- Numeric roots require `schema_version: 1`, represented by
  `finstack_quant_core::wire::SchemaVersion`.
- Missing, string-encoded, zero, and unsupported numeric markers fail.
- Persisted enums and tags use snake_case. External standards such as ISO
  currencies retain their prescribed spelling.
- Configuration and input objects deny unknown fields unless a field is an
  explicitly documented extension map.
- Dates are `YYYY-MM-DD` strings with JSON Schema `format: date`.
- Exact decimals are JSON strings matching
  `^-?\d+(\.\d+)?([eE][+-]?\d+)?$`; JSON numbers are rejected where a
  decimal is required.
- Rust and Python use identical snake_case contract names. WASM uses the
  corresponding camelCase API name and serializes the canonical Rust JSON.
- There are no aliases, alternate spellings, missing-marker fallbacks, or
  migration branches in the v1 contract.

## Root contract matrix

| Contract | Required marker | Runtime type / ingress | Published schema |
|---|---|---|---|
| Instrument | `finstack_quant.instrument/1` | `InstrumentEnvelope::from_slice_strict` | [instrument union and per-instrument schemas](../finstack-quant/valuations/schemas/instruments/1/) |
| Calibration request | `finstack_quant.calibration/1` | `CalibrationEnvelope::from_slice_strict` | [calibration](../finstack-quant/calibration/schemas/calibration/1/calibration.schema.json) |
| Calibration result | `finstack_quant.calibration/1` | `CalibrationResultEnvelope::from_slice_strict` | embedded in the calibration registry graph |
| Market context state | numeric `schema_version: 1` | `MarketContext::from_state_slice` | derived wherever the state is embedded |
| Financial model | numeric `schema_version: 1` | `FinancialModelSpec::from_slice_strict` | [financial model](../finstack-quant/statements/schemas/statements/1/financial_model_spec.schema.json) |
| Scenario | `finstack_quant.scenario/1` | `ScenarioEnvelope::from_slice_strict` | [scenario](../finstack-quant/scenarios/schemas/scenarios/1/scenario.schema.json) |
| Factor-model configuration | `finstack_quant.factor_model_config/1` | `FactorModelConfigEnvelope::from_slice_strict` | [factor-model configuration](../finstack-quant/models/schemas/factor_model/1/factor_model_config.schema.json) |
| Credit factor model | `finstack_quant.credit_factor_model/1` | `CreditFactorModel::from_slice_strict` | [credit factor model](../finstack-quant/models/schemas/factor_model/1/credit_factor_model.schema.json) |
| Attribution request | `finstack_quant.attribution/1` | `AttributionEnvelope` serde | [attribution request](../finstack-quant/attribution/schemas/attribution/1/attribution.schema.json) |
| Attribution result | `finstack_quant.attribution/1` | `AttributionResultEnvelope` serde | [attribution result](../finstack-quant/attribution/schemas/attribution/1/attribution_result.schema.json) |
| Margin | `finstack_quant.margin/1` | `MarginEnvelope::from_slice` | [margin](../finstack-quant/margin/schemas/margin/1/margin.schema.json) |
| Portfolio materialization | `finstack_quant.portfolio_materialization/1` | `Portfolio::from_materialization` | [portfolio materialization](../finstack-quant/portfolio/schemas/portfolio/1/portfolio_materialization.schema.json) |
| Valuation result | numeric `schema_version: 1` | `ValuationResult` serde | [valuation result](../finstack-quant/valuations/schemas/results/1/valuation_result.schema.json) |
| Statement result | numeric `schema_version: 1` | `StatementResult` serde | [statement result](../finstack-quant/statements/schemas/statements/1/statement_result.schema.json) |
| Portfolio result | numeric `schema_version: 1` | `PortfolioResult` serde | derived where embedded |
| Portfolio optimization result | numeric `schema_version: 1` | `PortfolioOptimizationResultWire` | [portfolio optimization result](../finstack-quant/portfolio/schemas/portfolio/1/portfolio_optimization_result.schema.json) |

`MarginEnvelope` preserves three closed root shapes:
`otc_margin_spec`, `csa_spec`, and `margin_call`. Portfolio materialization
contains a typed `InstrumentEnvelope`; implementations may retain raw bytes
only after typed validation succeeds.

## Generated component schemas

The registries also publish reusable, runtime-backed components:

- cashflow schedule, coupon, fee, amortization, prepayment, recovery, and
  default-model contracts under
  [`cashflow/1`](../finstack-quant/cashflows/schemas/cashflow/1/);
- rate, credit, volatility, and scalar market quotes in
  [`market_quote.schema.json`](../finstack-quant/calibration/schemas/market/1/market_quote.schema.json);
- `Date`, `Decimal`, `Currency`, `Money`, IDs, tenors, day counts, business-day
  conventions, diagnostics, and closed pricing-override maps under
  [`common/1`](../finstack-quant/valuations/schemas/common/1/);
- credit calibration inputs and configuration under
  [`factor_model/1`](../finstack-quant/models/schemas/factor_model/1/);
- the financial-statement normalization sidecar/configuration contract
  (add-back and deduction adjustments, caps, and self-referential cap base
  mode) — not a root envelope — in
  [`normalization_config.schema.json`](../finstack-quant/statements/schemas/statements/1/normalization_config.schema.json);
- the tagged instrument payload `InstrumentJson` (the envelope's `instrument`
  field, embedded by portfolio positions and candidates) in
  [`instrument_json.schema.json`](../finstack-quant/valuations/schemas/common/1/instrument_json.schema.json);
- every other serde type the bindings exchange with a host (analytics results,
  covenant engines and reports, margin and XVA results, model parameters and
  results, portfolio analytics results and specs, statements-analytics
  configurations and reports, and valuation side results), under
  `analytics/1`, `covenants/1`, `margin/1`, `models/1`, `portfolio/1`,
  `statements_analytics/1`, `scenarios/1` and `results/1`. Each crate's
  `schemas/index.json` lists them; the WASM package generates its TypeScript
  declarations from them.

Every instrument artifact is generated from the same registry that owns its
serde tag, example provider, binding exposure, and single-variant envelope.

## Canonical naming

The wire vocabulary uses full concept names:

- `frequency`, `day_count`, and `business_day_convention`;
- `*_currency` for currency-valued identifiers and `*_bp` for basis-point
  values;
- `as_of_spreads`;
- `vol_surface_id` for volatility-surface dependencies;
- `credit_curve_id` for every hazard-curve reference (instrument
  dependencies, function inputs and outputs); it always names a
  `HazardCurve`.

Day-count values are exactly `act_360`, `act_365f`, `act_365l`, `nl_365`,
`30_360`, `30e_360`, `30e_360_isda`, `act_act`, `act_act_isma`, and
`bus_252`.

Pricing configuration exposes three independent closed maps:
`instrument_pricing_overrides`, `metric_pricing_overrides`, and
`scenario_pricing_overrides`. Each defaults to empty when omitted, is omitted
when empty, and rejects `null`.

## Strict loading and canonical identity

Database or network ingress should use a bounded strict loader when one is
available. Strict loaders apply `LoadLimits`, verify the required marker,
deserialize the typed root, and run semantic validation. A JSON Schema check
does not replace semantic validation.

Canonical bytes are produced by `finstack_quant_core::canonical`:

- object keys are recursively sorted by UTF-8 bytes;
- array order is preserved;
- finite numbers use serde_json/Ryu's shortest representation;
- decimal strings, dates, enum labels, and extension-map values are preserved;
- output is compact UTF-8 JSON with no insignificant whitespace.

`content_hash(value)` hashes the domain-separated preimage
`b"finstack-canon/c1\0" || canonical_json_bytes` and returns
`sha256:<lowercase hex>`.

## Canonical fixtures

Each persisted root with a canonical fixture has matching canonical bytes,
content hash, and an owning test:

- [instrument](../finstack-quant/valuations/tests/data/canonical/instrument.json)
- [calibration](../finstack-quant/valuations/tests/data/canonical/calibration.json)
- [market context state](../finstack-quant/core/tests/data/canonical/market_context_state.json)
- [financial model](../finstack-quant/statements/tests/data/canonical/financial_model.json)
- [scenario](../finstack-quant/scenarios/tests/data/canonical/scenario.json)
- [factor-model configuration](../finstack-quant/models/tests/data/canonical/factor_model_config.json)
- [credit factor model](../finstack-quant/models/tests/data/canonical/credit_factor_model.json)
- [portfolio materialization](../finstack-quant/portfolio/tests/data/canonical/portfolio_materialization.json)

The adjacent `.sha256` files contain the domain-separated identity, not a raw
file digest.

## Generation and verification

`mise run rust-gen-schemas` is the sole schema write path. Every generator
supports `--write`, `--check`, `--list`, and `--output-root`.
`mise run gen-write` is the local umbrella that runs that write path plus
`wasm-gen-bindings`. `mise run all-ci` always regenerates through `gen-write`
before tests and the rest of the verification suite.

`mise run rust-check-schemas` is non-mutating. It verifies registry inventory,
checked-in bytes, unique `$id` values, resolvable references, and v1-only
paths. `scripts/check_schema_generation.py` additionally generates into two
independent temporary trees and requires identical bytes. The emitters sort
registry paths and JSON object keys and never include timestamps, absolute
paths, environment values, or randomness.

`scripts/check_schema_residue.py` rejects handwritten `JsonSchema`
implementations, schema macros and patchers, serde aliases, schema-only value
projections, obsolete generators, abbreviated persisted names, and malformed
replacement spellings.
