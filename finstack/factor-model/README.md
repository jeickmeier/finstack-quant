# Factor Model

Canonical multi-asset factor-modelling primitives for finstack. This crate owns
factor definitions, covariance matrices, dependency matching, credit hierarchy
artifacts, credit calibration/decomposition, and the positions × factors
`SensitivityMatrix`.

Credit has the richest implementation today: a deterministic hierarchical
calibrator from sparse issuer-spread history plus decomposition of observed
spreads into level factor values. Rates, equity, FX, volatility, commodity, and
inflation factors are first-class through the generic `FactorType`,
`FactorDefinition`, `FactorModelConfig`, matching, covariance, and portfolio
sensitivity/risk engines.

## Layout

```
factor-model/
├── primitives/             # FactorId, FactorType, FactorDefinition, dependencies
├── matching/               # Mapping-table, cascade, hierarchy matchers
├── credit/                 # Credit hierarchy, calibration, decomposition
├── calibration/            # Shared calibrator trait shape
├── config.rs               # FactorModelConfig, RiskMeasure, bump config
├── covariance.rs           # FactorCovarianceMatrix
└── sensitivity_matrix.rs   # SensitivityMatrix: positions × factors
```

Pricing engines that take `&dyn Instrument` (delta and full-repricing) live
in `finstack-portfolio::sensitivity` because they depend on the instrument
trait surface.

## Generic capabilities

The following are asset-class agnostic:

- Factor identity and asset class tagging (`FactorId`, `FactorType`).
- Factor-to-market mapping (`MarketMapping`) for curves, equity spot, FX, and
  volatility surfaces.
- Covariance validation and canonical factor ordering
  (`FactorCovarianceMatrix`).
- Dependency-to-factor matching (`MatchingConfig`, `MappingRule`,
  `DependencyFilter`, `AttributeFilter`).
- Risk interpretation and sensitivity extraction configuration
  (`FactorModelConfig`, `RiskMeasure`, `PricingMode`, `BumpSizeConfig`).
- Portfolio sensitivity/risk engines in `finstack-portfolio` that consume the
  generic configuration.

Credit-specific today:

- `CreditFactorModel` as a self-contained hierarchy artifact.
- `CreditCalibrator` and `decompose_levels` / `decompose_period`.
- Credit hierarchy issuer beta, bucket folding, and idiosyncratic vol state.

Future rates/equity/vol calibrators should implement `FactorCalibrator` and
return their own model artifact while reusing the generic primitives above.

## Credit concepts

The credit factor model expresses each issuer spread `S_i` as a linear
decomposition over a hierarchy of factors:

```text
S_i ≡ β_i^PC · g
      + Σ_k β_i^level_k · L_k(g_i^k)
      + adder_i
```

where `g` is a generic (PC) factor common to all issuers, `L_k(·)` are
per-bucket factors at hierarchy level `k` (e.g. rating → region → sector),
and `adder_i` is the per-issuer idiosyncratic residual at the calibration
anchor. The same identity holds for first differences, which is the
reconciliation invariant enforced by `decompose_period` to absolute tolerance
`1e-10`.

Issuers are classified as either `IssuerBeta` (fits per-level β) or
`BucketOnly` (β fixed at 1.0) according to an [`IssuerBetaPolicy`] and
per-issuer overrides.

## Calibration

[`CreditCalibrator`] runs a deterministic sequential peel:

1. Classify each issuer (`IssuerBeta` vs `BucketOnly`).
2. Optionally difference the spread panel into returns.
3. Inventory hierarchy buckets and fold up under-populated buckets.
4. **PC peel**: regress each `IssuerBeta` issuer's series on the generic
   factor; residual is propagated forward.
5. **Per-level peel**: bucket means become factor returns; `IssuerBeta`
   issuers fit a per-level β against the bucket factor and the residual is
   propagated.
6. Adder series → per-issuer idiosyncratic vol with a
   caller-override → history → bucket-peer-proxy → global-default cascade.
7. **Anchor** every factor's level value at `as_of` using the same peeling
   logic on a single observation in level space.
8. Per-factor annualized variance from the unbiased (Bessel-corrected)
   sample variance.
9. Assemble correlation + covariance per [`CovarianceStrategy`]:
   - `Diagonal` — identity ρ, `Σ = diag(σ²)`.
   - `Ridge { alpha }` — sample ρ (PSD-repaired if needed), `Σ = D·ρ·D + α·I`.
   - `FullSampleRepaired` — sample ρ repaired via nearest-correlation
     projection, `Σ = D·ρ_repaired·D`.
10. Bundle a [`CreditFactorModel`] and run its `validate()`.

```rust
use finstack_factor_model::{
    CreditCalibrator, CreditCalibrationConfig, CreditCalibrationInputs,
    HistoryPanel, IssuerTagPanel, GenericFactorSeries,
    BucketSizeThresholds, CovarianceStrategy, PanelSpace, VolModelChoice,
    BetaShrinkage,
};

let config = CreditCalibrationConfig {
    policy: issuer_beta_policy,
    hierarchy: hierarchy_spec,
    min_bucket_size_per_level: BucketSizeThresholds::default_for_levels(3),
    vol_model: VolModelChoice::Sample,
    covariance_strategy: CovarianceStrategy::FullSampleRepaired,
    beta_shrinkage: BetaShrinkage::TowardOne { alpha: 0.25 },
    use_returns_or_levels: PanelSpace::Returns,
    annualization_factor: 12.0,
};

let model = CreditCalibrator::new(config).calibrate(CreditCalibrationInputs {
    history_panel,
    issuer_tags,
    generic_factor,
    as_of,
    asof_spreads,
    idiosyncratic_overrides: Default::default(),
})?;
```

### Determinism

All keyed maps are `BTreeMap`, all iteration orders are stable, and
peer-proxy vol lists are sorted. Two calibrations with byte-identical inputs
serialize to byte-identical JSON.

### Supported / deferred features

- `VolModelChoice::Sample` is the only supported variant today. `Garch`,
  `Egarch`, and `Ewma` are accepted at the type level but the calibrator
  returns a clean `Error::Validation` if any of them is requested.
- The peer-proxy fallback chain treats `idiosyncratic_overrides` as the
  highest-precedence source for adder vols (caller → history → bucket-peer
  proxy → global mean → zero).

## Decomposition

[`decompose_levels`] takes a calibrated [`CreditFactorModel`] and observed
issuer spreads at a date and returns a [`LevelsAtDate`] — the generic factor,
per-level bucket values, and per-issuer adders. Issuers not present in the
model can be decomposed under bucket-only semantics by supplying
`runtime_tags`.

[`decompose_period`] differences two snapshots into a
[`PeriodDecomposition`] (`d_generic`, per-level deltas, `d_adder`) and
preserves the linear reconciliation invariant on `ΔS_i` to absolute
tolerance `1e-10` for every issuer present in both snapshots.

```rust
use finstack_factor_model::{decompose_levels, decompose_period};

let levels_t0 = decompose_levels(&model, &spreads_t0, generic_t0, t0, None)?;
let levels_t1 = decompose_levels(&model, &spreads_t1, generic_t1, t1, None)?;
let period = decompose_period(&levels_t0, &levels_t1)?;
```

Failure modes are surfaced through [`DecompositionError`]: `UnknownIssuer`,
`MissingTag`, `ModelInconsistent`, `SnapshotShapeMismatch`, and
`DateMismatchInPeriod`.

## Sensitivity matrix

[`SensitivityMatrix`] is the canonical row-major dense layout
(positions × factors) used by the delta-based and full-repricing factor
sensitivity engines in `finstack-portfolio::sensitivity`. The crate provides
only the storage type plus accessors (`delta`, `set_delta`, `position_deltas`,
`factor_deltas`, `as_slice`); the engines themselves live in
`finstack-portfolio` because they take `&dyn Instrument`.

## Related

- `finstack-core` — base primitives consumed by factor models (`Currency`,
  `Date`, `CurveId`, `IssuerId`, market-data bump units, and errors).
- `finstack-analytics` — `beta` OLS slope and `nearest_correlation_matrix`
  PSD repair used by the calibrator.
- `finstack-portfolio::sensitivity` — sensitivity engines that consume the
  calibrated model and produce a [`SensitivityMatrix`].
- `finstack-valuations` — instrument pricing that consumes the calibrated
  model as a market input.
