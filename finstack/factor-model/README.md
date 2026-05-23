# Factor Model

Credit factor-model primitives: calibrate a hierarchical credit factor model
from sparse issuer-spread history, decompose observed spreads into level
factor values, and represent positions × factors sensitivity matrices.

This crate was carved out of `finstack-valuations` to shrink that umbrella
crate's edit-rebuild loop. Callers import directly from
`finstack_factor_model::*`; there is no re-export façade in
`finstack-valuations`.

## Layout

```
factor-model/
├── credit_calibration.rs   # CreditCalibrator: peel-the-onion calibration
├── credit_decomposition.rs # decompose_levels / decompose_period
└── sensitivity_matrix.rs   # SensitivityMatrix: positions × factors
```

Pricing engines that take `&dyn Instrument` (delta and full-repricing) live
in `finstack-portfolio::sensitivity` because they depend on the instrument
trait surface.

## Concepts

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

- `finstack-core::factor_model` — canonical artifact types
  (`CreditFactorModel`, `FactorModelConfig`, `FactorCovarianceMatrix`,
  `CreditHierarchySpec`, `IssuerBetaPolicy`, …).
- `finstack-analytics` — `beta` OLS slope and `nearest_correlation_matrix`
  PSD repair used by the calibrator.
- `finstack-portfolio::sensitivity` — sensitivity engines that consume the
  calibrated model and produce a [`SensitivityMatrix`].
- `finstack-valuations` — instrument pricing that consumes the calibrated
  model as a market input.
