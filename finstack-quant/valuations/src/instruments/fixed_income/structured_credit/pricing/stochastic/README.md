# Structured Credit — Stochastic Pricing

Valuations owns structured-credit calibration presets and the scenario-tree /
Monte Carlo orchestration that runs each simulated collateral path through the
deal waterfall. Product-independent default, prepayment, correlation, and
finite-pool copula engines live in
`finstack_quant_models::credit::pool`.

## Module layout

```text
stochastic/
├── calibrations.rs  # registry-backed RMBS/CLO/CMBS/ABS presets
├── tree/config.rs   # valuation-owned scenario-tree configuration
└── pricer/
    ├── config.rs    # PricingMode and internal pricer configuration
    ├── engine.rs    # path generation plus waterfall orchestration
    └── result.rs    # StochasticPricingResult and TranchePricingResult
```

## Configure a deal

```rust
use finstack_quant_cashflows::builder::PrepaymentModelSpec;
use finstack_quant_models::credit::pool::{
    CorrelationStructure, StochasticDefaultSpec, StochasticPrepaySpec,
};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCredit;

let mut clo = StructuredCredit::example();

// Applies valuation-owned, registry-backed presets for the deal type.
clo.enable_stochastic_defaults().expect("valid built-in stochastic defaults");

// Or supply explicit models-owned specifications.
clo.with_stochastic_prepay(StochasticPrepaySpec::factor_correlated(
    PrepaymentModelSpec::constant_cpr(0.15),
    0.40,
    0.25,
))
.with_stochastic_default(StochasticDefaultSpec::gaussian_copula(0.03, 0.20))
.with_correlation(CorrelationStructure::sectored(0.30, 0.10, -0.20).expect("valid correlation"));
```

`StochasticPrepaySpec`, `StochasticDefaultSpec`, `CorrelationStructure`, and
`PoolGranularity` are not re-exported by valuations. Import them from models.

## Pricing

```rust
use finstack_quant_valuations::instruments::fixed_income::structured_credit::PricingMode;

let result = clo.price_stochastic_with_mode(
    &market,
    as_of,
    PricingMode::MonteCarlo {
        num_paths: 50_000,
        antithetic: true,
    },
)?;
```

`PricingMode::default()` uses 10,000 antithetic Monte Carlo paths. Tree mode is
bounded by its non-recombining node count and is intended only for short
horizons. `StochasticPricingResult` and `TranchePricingResult` remain
valuation-owned outputs.

The path seed is fixed by the internal scenario configuration, so repeated
runs and bump-and-reprice sensitivities reuse common random numbers.

Pools of real instruments (`AssetPool::instruments`) price through the same
modes. Each path resolves every name's default from its own hazard curve or
the deal model — through the per-name copula when the default model is a
copula — and advances each stochastic revolver's spread (CIR, market-anchored
on its hazard curve) and utilization (exact OU toward the spread-linked
target) with shocks loaded on the period systematic factor, so draws grow on
the stress paths. The result adds `expected_collateral_draws` (mean funded
draws per path) and `unfunded_draw_path_fraction` (paths on which the reserve
and principal collections could not fund a draw). With zero utilization
volatility, zero spread sensitivity and no default probability every path
reproduces the deterministic instrument pool.

For pools with stochastic revolvers the run also reports the **draw option
cost**: every path is simulated twice on the same random numbers, once as
is and once with each funded revolver draw accruing at the path's fair
spread instead of its contractual margin; a tranche's share is its present
value on the actual run less the counterfactual one, and
`StochasticPricingResult::draw_option_cost` is the sum of the shares (the
per-path values are in `draw_option_cost_paths`). It is negative when
spreads widen after draws, and a pass-through single-revolver pool reproduces
the standalone facility's `draw_option_cost` when the facility carries no
credit risk; with credit risk the two engines apply their own default models
to the annuity and agree only in expectation.

## Calibration ownership

`calibrations.rs` reads the v1 structured-credit assumption registry and
constructs explicit models-owned specs. RMBS, CLO, CMBS, and ABS preset policy
therefore remains in valuations; models has no dependency on the registry or
on instruments.

## Verification

```bash
mise run rust-test-filter -- finstack-quant-valuations structured_credit
mise run rust-test-crate -- finstack-quant-models
```

## Deterministic-only inputs

`validate_stochastic_tranches` rejects, with a named `Validation` error,
deal inputs the path engines do not model: cumulative-loss and timing default
curves (`DefaultCurve::{CumulativeLoss, Timing}`), month-of-default severity
vectors (`RecoveryModelSpec.severity_vector`) and NPL resolution timelines
(`PoolAsset.liquidation`). Price those deals deterministically; the
deterministic engine and the stochastic one otherwise share `pool_flows`,
the waterfall and every coverage test.
