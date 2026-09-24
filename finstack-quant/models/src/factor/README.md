# Factor models

Canonical multi-asset factor-modelling primitives: factor definitions and
market mappings, dependency-to-factor matching, factor covariance, the
positions × factors `SensitivityMatrix`, and a deterministic hierarchical
credit calibrator with its decomposition counterpart.

Credit has the deepest implementation today: a sequential peel that calibrates
a `CreditFactorModel` from sparse issuer-spread history, plus decomposition of
observed spreads back into level factor values. Rates, equity, FX, volatility,
commodity, and inflation factors are first-class through the generic
`FactorType`, `FactorDefinition`, `MarketMapping`, `MatchingConfig`, and
`FactorModelConfig` types, but have no calibrator of their own yet.

## Position in the stack

Depends on `finstack-quant-core` and `finstack-quant-analytics` (the latter for
`beta` OLS slopes in the peel and `nearest_correlation_matrix` /
`validate_correlation_matrix` in covariance assembly). Consumed by
`finstack-quant-valuations`, `finstack-quant-attribution` (credit-factor P&L
decomposition), and `finstack-quant-portfolio`. Exposed by the umbrella crate
as `finstack_quant::models::factor`.

Pricing engines that take `&dyn Instrument` — the delta-based and
full-repricing factor sensitivity engines — live in
[`finstack-quant-portfolio`](../../../portfolio/README.md)'s `sensitivity` module,
because they depend on the instrument trait surface. This crate provides only
the storage type they fill in.

## Public surface

The crate root exports configuration, covariance, envelope, factor,
dependency, and sensitivity types. The public submodules are `matching`,
`credit`, and `schema`.

| Group | Items |
|-------|-------|
| Factor identity | `FactorId`, `FactorType` (`Rates`, `Credit`, `Equity`, `Fx`, `Volatility`, `Commodity`, `Inflation`, `Custom(String)`), `FactorDefinition` |
| Market mapping | `MarketMapping` (`CurveParallel`, `CurveBucketed`, `EquitySpot`, `FxRate`, `VolShift`), `MarketDependency`, `DependencyType`, `CurveType` |
| Run configuration | `FactorModelConfig`, `RiskMeasure`, `PricingMode`, `BumpSizeConfig`, `UnmatchedPolicy` |
| Covariance | `FactorCovarianceMatrix` |
| Persistence | `FactorModelConfigEnvelope`, `FactorModelConfigSchema`, `FACTOR_MODEL_CONFIG_CONTRACT` |
| Matching (`matching`) | `MatchingConfig`, `MappingRule`, `DependencyFilter`, `AttributeFilter`, `FactorMatcher`, `CascadeMatcher`, `HierarchicalMatcher`, `MappingTableMatcher`, `CreditHierarchicalMatcher`, `HierarchicalConfig`, `CreditHierarchicalConfig`, `FactorMatchEntry`, `FactorMatchError`, `FactorNode`, `bucket_factor_id`, `dimension_key`, `CREDIT_GENERIC_FACTOR_ID`, `ISSUER_ID_META_KEY` |
| Sensitivity | `SensitivityMatrix` |
| Credit (`credit`) | `credit::hierarchy`, `credit::calibration`, `credit::decomposition` |
| Schemas (`schema`) | `ARTIFACTS` and the four checked-in `*_schema()` accessors |

```rust
use finstack_quant_models::factor::{FactorDefinition, FactorId, FactorType, MarketMapping};
use finstack_quant_core::market_data::bumps::BumpUnits;
use finstack_quant_core::types::CurveId;

let def = FactorDefinition {
    id: FactorId::new("USD_10Y_SWAP"),
    factor_type: FactorType::Rates,
    market_mapping: MarketMapping::CurveParallel {
        curve_ids: vec![CurveId::new("USD-OIS")],
        units: BumpUnits::RateBp,
    },
    description: Some("USD 10Y swap rate".to_string()),
};
assert_eq!(def.factor_type, FactorType::Rates);
```

### Errors

The crate defines no root `Error`; fallible entry points return
`finstack_quant_core::Result`. Two scoped `thiserror` enums exist for domains
core cannot express: `matching::FactorMatchError` and
`credit::decomposition::DecompositionError`.

## Credit model

Each issuer spread `S_i` decomposes linearly over a hierarchy of factors:

```text
S_i ≡ β_i^PC · g
      + Σ_k β_i^level_k · L_k(g_i^k)
      + adder_i
```

`g` is a generic (PC) factor common to all issuers, `L_k(·)` are per-bucket
factors at hierarchy level `k` (e.g. rating → region → sector), and `adder_i`
is the per-issuer idiosyncratic residual at the calibration anchor. The same
identity holds for first differences. `decompose_period` is algebraic
differencing of two snapshots; it does not numerically enforce a tolerance.
Callers checking the identity typically use absolute tolerance `1e-10`.

Callers pass **decimal** spreads (`0.01` = 100 bp). Calibration and
`decompose_levels` convert `× 10_000` at entry. Artifact internals, factor
histories, anchor/decompose outputs, and `Σ` stay in **bp** so they match
CS01 (P&L per bp). Values that look like bp (e.g. `100.0`) are rejected.

Issuers are classified as either `IssuerBeta` (fits a per-level β) or
`BucketOnly` (β fixed at `1.0`) according to an `IssuerBetaPolicy` plus
per-issuer `IssuerBetaOverride` entries. The default policy is
`GloballyOff`. When `IssuerBeta` is on, OLS uses the **leave-one-out**
bucket mean so β is not biased toward 1; the stored/peeled factor is the
**full-bucket** weighted mean so `S_i = β g + Σ β_k L_k + adder` holds.

Bucket means default to **DTS** weights (`SD_years × spread_bp`, Ben Dor /
Barclays). This is the factor-construction weight for risk forecast and
contribution. Position exposure remains `β × CS01`; DTS does not replace
CS01. `BucketWeighting::Equal` is the opt-out for tests and simple books.

### Calibration

`CreditCalibrator::new(config).calibrate(inputs)` runs a deterministic
sequential peel:

1. Classify each issuer (`IssuerBeta` vs `BucketOnly`).
2. Optionally difference the spread panel into returns (`PanelSpace`).
3. Inventory hierarchy buckets and fold up under-populated buckets.
4. **PC peel** — regress each `IssuerBeta` issuer's series on the generic
   factor; the residual propagates forward.
5. **Per-level peel** — bucket means become factor returns; `IssuerBeta`
   issuers fit a per-level β against the bucket factor and the residual
   propagates.
6. Adder series → per-issuer idiosyncratic vol through a
   caller-override → history → bucket-peer-proxy → global-mean → zero cascade.
7. **Anchor** every factor's level value at `as_of`, applying the same peeling
   logic to a single observation in level space.
8. Per-factor variance forecast via the configured `VolModelChoice`:
   `Sample` (unbiased sample variance) or `Ewma { lambda }` (RiskMetrics
   exponentially weighted, λ ∈ (0, 1)).
9. Correlation and covariance per `CovarianceStrategy`:
   - `Diagonal` — identity ρ, `Σ = diag(σ²)`.
   - `Ridge { alpha }` — sample ρ (PSD-repaired if needed), `Σ = D·ρ·D + α·I`.
   - `FullSampleRepaired` (default) — sample ρ repaired via nearest-correlation
     projection, `Σ = D·ρ_repaired·D`.
   - `LedoitWolf` — identity-target shrinkage; Σ and ρ both come from the
     shrunk estimator over complete-case dates.
10. Assemble `FactorModelConfig` with `MatchingConfig::CreditHierarchical`,
    build `CalibrationDiagnostics`, and run `CreditFactorModel::validate()`
    before returning.

```rust,ignore
use finstack_quant_models::factor::credit::calibration::{
    BetaShrinkage, BucketSizeThresholds, BucketWeighting, CovarianceStrategy,
    CreditCalibrationConfig, CreditCalibrationInputs, CreditCalibrator, PanelFrequency,
    PanelSpace, VolModelChoice,
};

let config = CreditCalibrationConfig {
    policy: issuer_beta_policy,
    hierarchy: hierarchy_spec,
    min_bucket_size_per_level: BucketSizeThresholds::default_for_levels(3),
    vol_model: VolModelChoice::Sample,
    covariance_strategy: CovarianceStrategy::FullSampleRepaired,
    beta_shrinkage: BetaShrinkage::TowardOne { alpha: 0.25 },
    use_returns_or_levels: PanelSpace::Returns,
    panel_frequency: PanelFrequency::Monthly,
    bucket_weighting: BucketWeighting::Dts,
};

let model = CreditCalibrator::new(config).calibrate(CreditCalibrationInputs {
    history_panel,
    issuer_tags,
    generic_factor,
    as_of,
    as_of_spreads,
    idiosyncratic_overrides: Default::default(),
    spread_durations, // years, required when bucket_weighting is Dts
})?;
```

`CreditCalibrationConfig::default()` uses `IssuerBetaPolicy::GloballyOff`,
`VolModelChoice::Sample`, `CovarianceStrategy::FullSampleRepaired`,
`BetaShrinkage::None`, `PanelSpace::Returns`, `PanelFrequency::Monthly`
(annualizes with 12), and `BucketWeighting::Dts`. The history panel must be
a complete regular grid of that frequency with no `None` issuer observations.
`FullSampleRepaired` rather than `Diagonal` is deliberate: an
identity-correlation default silently drops cross-factor correlation and
understates the vol of a correlated long book.

Embedded `factor_histories` are dense bp series (no `None → 0.0`) kept as
calibration provenance. Parametric `sᵀΣs` is the portfolio risk engine.

### Determinism

Every keyed map is a `BTreeMap`, every iteration order is stable, and
peer-proxy vol lists are sorted. Two calibrations with byte-identical inputs
serialize to byte-identical JSON.

### Decomposition

`decompose_levels(model, spreads, generic, date, runtime_tags)` takes a
calibrated `CreditFactorModel` plus observed issuer spreads at a date and
returns a `LevelsAtDate` — the generic factor, per-level bucket values, and
per-issuer adders. Issuers absent from the model can still be decomposed under
bucket-only semantics by supplying `runtime_tags`.

`decompose_period(levels_t0, levels_t1)` differences two snapshots into a
`PeriodDecomposition` (`d_generic`, per-level deltas, `d_adder`). The
linear identity on `ΔS_i` is algebraic when both snapshots share the same
model vintage and tags; the function does not enforce a numerical tolerance.

```rust,ignore
use finstack_quant_models::factor::credit::decomposition::{decompose_levels, decompose_period};

let levels_t0 = decompose_levels(&model, &spreads_t0, generic_t0, t0, None)?;
let levels_t1 = decompose_levels(&model, &spreads_t1, generic_t1, t1, None)?;
let period = decompose_period(&levels_t0, &levels_t1)?;
```

Failure modes surface through `DecompositionError`: `UnknownIssuer`,
`MissingTag`, `ModelInconsistent`, `SnapshotShapeMismatch`,
`DateMismatchInPeriod`, `InvalidDecimalSpread`, and `InvalidDtsWeight`.

## Sensitivity matrix

`SensitivityMatrix` is the canonical row-major dense layout
(positions × factors). It is storage plus accessors only — `zeros`,
`position_ids`, `factor_ids`, `n_positions`, `n_factors`, `delta`, `set_delta`,
`position_deltas`, `factor_deltas`, `as_slice`. The engines that fill it live
in `finstack-quant-portfolio`'s `sensitivity` module
([`delta_engine.rs`](../../../portfolio/src/sensitivity/delta_engine.rs),
[`repricing_engine.rs`](../../../portfolio/src/sensitivity/repricing_engine.rs)).

## Conventions

- `FactorId` is string-backed and case-sensitive.
- Covariance entries are annualized (co)variances in each factor's canonical
  bump unit: bp for rates and credit, % for equity/commodity/FX, vol points for
  volatility. `FactorCovarianceMatrix` documents the units contract.
- Credit callers pass decimal spreads; internals and `Σ` are bp. DTS weights
  bucket factors; position exposure is `β × CS01`.
- `decompose_period` is algebraic differencing; it does not enforce `1e-10`.
- Spec types are `#[serde(deny_unknown_fields)]`; see
  [`docs/SERDE_STABILITY.md`](../../../../docs/SERDE_STABILITY.md).
- This crate is `f64` throughout; it holds no `Money` and performs no FX. See
  [`INVARIANTS.md`](../../../../INVARIANTS.md).

## JSON schemas

Four v1 artifacts are generated and checked in under
[`schemas/factor_model/1/`](../../schemas/factor_model/1), indexed by
[`schemas/index.json`](../../schemas/index.json):

| File | Rust type |
|------|-----------|
| `factor_model_config.schema.json` | `FactorModelConfigEnvelope` |
| `credit_factor_model.schema.json` | `credit::hierarchy::CreditFactorModel` |
| `credit_calibration_config.schema.json` | `credit::calibration::CreditCalibrationConfig` |
| `credit_calibration_inputs.schema.json` | `credit::calibration::CreditCalibrationInputs` |

The published base URI is
`https://finstack_quant.dev/schemas/factor_model/1/`, kept for compatibility
with the original credit-only schemas. Regenerate and verify with:

```bash
cargo run -p finstack-quant-models --bin gen_factor_model_schemas -- --write
cargo run -p finstack-quant-models --bin gen_factor_model_schemas -- --check
```

`mise run rust-gen-schemas` and `mise run rust-check-schemas` do this for every
crate at once. The Rust serde types and strict loaders remain authoritative for
semantic validation.

## Bindings

Only the credit surface is bound; the generic primitives, matching, and
`SensitivityMatrix` are Rust-only.

- **Python** — `finstack_quant.models.factor.credit` exposes
  `CreditFactorModel`, `CreditCalibrator`, `LevelsAtDate`,
  `PeriodDecomposition`, `FactorCovarianceForecast`, `decompose_levels`, and
  `decompose_period`. `finstack_quant.models.factor.schema` exposes the JSON
  Schema accessors.
- **WASM** — the same seven names under `models.factor.credit`
  ([`exports/models/factor.js`](../../../../finstack-quant-wasm/exports/models/factor.js));
  classes keep their Rust names, the two free functions become
  `decomposeLevels` / `decomposePeriod`. No schema twin.

The authoritative contract is
[`parity_contract.toml`](../../../../finstack-quant-py/parity_contract.toml).

## Tests and benchmarks

| Path | Contents |
|------|----------|
| [`tests/factor_canonical_contract.rs`](../../tests/factor_canonical_contract.rs) | Serialization matches the checked-in canonical artifacts byte for byte |
| [`tests/factor_credit_peel_parity.rs`](../../tests/factor_credit_peel_parity.rs) | Calibration peel vs. decomposition agreement |
| [`tests/factor_multi_asset_config.rs`](../../tests/factor_multi_asset_config.rs) | Generic `FactorModelConfig` across asset classes |
| [`tests/factor_strictness.rs`](../../tests/factor_strictness.rs) | `deny_unknown_fields` behavior |
| [`tests/factor_schema_contract.rs`](../../tests/factor_schema_contract.rs) | Generated schemas match the Rust types |
| [`tests/data/canonical/`](../../tests/data/canonical) | `credit_factor_model.json` and `factor_model_config.json` with SHA-256 sidecars |
| [`benches/factor/factor_model.rs`](../../benches/factor/factor_model.rs) | Calibration and decomposition throughput |

## References

Entries live in [`docs/REFERENCES.md`](../../../../docs/REFERENCES.md):

- Factor models and exposure-based risk —
  [`#meucci-risk-and-asset-allocation`](../../../../docs/REFERENCES.md#meucci-risk-and-asset-allocation)
- Euler capital allocation —
  [`#tasche-2008-capital-allocation`](../../../../docs/REFERENCES.md#tasche-2008-capital-allocation)
- Ledoit-Wolf shrinkage —
  [`#ledoitwolf2004`](../../../../docs/REFERENCES.md#ledoitwolf2004)

## Verification

```bash
mise run rust-lint-crate -- finstack-quant-models
mise run rust-test-crate -- finstack-quant-models
mise run rust-bench-crate -- finstack-quant-models factor_model
```

Workspace gates (`mise run rust-lint`, `mise run rust-test`, `mise run rust-doc`
— the last one runs doctests) are what CI enforces. Use `cargo nextest`, not
`cargo test`, for crate-scoped runs; see
[`CONTRIBUTING.md`](../../../../CONTRIBUTING.md).
