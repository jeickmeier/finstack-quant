# Revolving Credit Facility

Corporate revolving credit facilities (revolvers) with deterministic or
stochastic utilization, fixed or floating base rates, tiered fees, and optional
hazard-curve survival weighting. A single cashflow engine drives both modes so
deterministic and stochastic pricing cannot drift apart.

Where [`term_loan`](../term_loan/) has a funded balance and a draw calendar, a
revolver has a commitment that is drawn and repaid over its life, with fees on
both the drawn and the undrawn portion.

## Public surface

Import path:
`finstack_quant_valuations::instruments::fixed_income::revolving_credit`
(`RevolvingCredit` is also re-exported at
`finstack_quant_valuations::instruments`).

| Item | Purpose |
|------|---------|
| `RevolvingCredit` | The instrument. Build with `RevolvingCredit::builder()`; `RevolvingCredit::example()` for a canonical facility. |
| `BaseRateSpec` | `Fixed { rate }` or `Floating(FloatingRateSpec)` (floors, caps, gearing, reset lag). |
| `RevolvingCreditFees` | `upfront_fee`, `commitment_fee_tiers`, `usage_fee_tiers`, `facility_fee_bp`. Helpers: `flat(..)`, `flat_bp(..)`. |
| `DrawRepaySpec`, `DrawRepayEvent` | `Deterministic(Vec<DrawRepayEvent>)` or `Stochastic(Box<StochasticUtilizationSpec>)`. |
| `StochasticUtilizationSpec`, `UtilizationProcess` | Path count, seed, antithetic/Sobol switch, and the utilization process. |
| `McConfig`, `CreditSpreadProcessSpec`, `InterestRateProcessSpec` | Optional multi-factor dynamics: correlation matrix, credit-spread and short-rate processes. |
| `RevolvingCreditPricer` | `price_with_paths(facility, market, as_of)` for full Monte Carlo path capture; `expected_cashflows(..)` for the path-averaged schedule of a stochastic facility. |
| `EnhancedMonteCarloResult`, `PathResult` | MC statistics plus per-path PV, cashflows and factor trajectories. |
| `PathAwareCashflowSchedule`, `ThreeFactorPathData` | Cashflow schedule carrying the simulated factor path. |
| `ZERO_TOLERANCE`, `UTILIZATION_CHANGE_THRESHOLD`, `INTERPOLATION_TOLERANCE`, `MIN_CIR_SPREAD`, `MAX_RECOVERY_RATE`, `MC_CLOCK_DAY_COUNT` | Module numerical constants; the last is the ACT/365F Monte Carlo clock. |

Note that `pricer` and `types` are `pub(crate)` submodules — import the names
above from the module root, not from `revolving_credit::types::…`.

## Module layout

```
revolving_credit/
├── mod.rs                # re-exports, numerical constants, module overview
├── types.rs              # RevolvingCredit, fees, rate/draw specs, MC config, Instrument impl
├── cashflow_engine.rs    # single engine for both deterministic and path-driven schedules
├── utils.rs              # calendar-aware schedules, reset dates, floating projection, balance evolution
├── pricing/
│   ├── unified.rs                    # RevolvingCreditPricer: registry entry, mode dispatch
│   ├── path_pricing.rs               # single-path PV: discounting, survival, default leg (recovery + LEQ)
│   ├── stochastic.rs                 # MC orchestration, statistics, expected cashflows
│   ├── components.rs                 # upfront-fee PV
│   ├── path_generator.rs             # 3-factor path generation (Philox or Sobol, optional antithetic)
│   ├── monte_carlo_process.rs        # utilization / rate / spread process definitions
│   └── monte_carlo_discretization.rs # discretization schemes
└── metrics/              # utilization_rate, available_capacity, weighted_average_cost, CS01
```

## Construction

```rust
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, DrawRepayEvent, DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DayCount, Tenor};
use finstack_quant_core::money::Money;
use time::macros::date;

let facility = RevolvingCredit::builder()
    .id("RC-001".into())
    .commitment_amount(Money::new(10_000_000.0, Currency::USD)?)
    .drawn_amount(Money::new(5_000_000.0, Currency::USD)?)
    .commitment_date(date!(2025 - 01 - 01))
    .maturity(date!(2028 - 01 - 01))
    .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
    .day_count(DayCount::Act360)
    .frequency(Tenor::quarterly())
    .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0)?)   // commitment / usage / facility, bp
    .draw_repay_spec(DrawRepaySpec::Deterministic(vec![
        DrawRepayEvent {
            date: date!(2025 - 03 - 01),
            amount: Money::new(1_000_000.0, Currency::USD)?,
            is_draw: true,
        },
        DrawRepayEvent {
            date: date!(2025 - 06 - 01),
            amount: Money::new(500_000.0, Currency::USD)?,
            is_draw: false,
        },
    ]))
    .discount_curve_id("USD-OIS".into())
    // Optional credit inputs:
    // .credit_curve_id("BORROWER-HZ".into())
    .recovery_rate(0.4)
    .build()?;
```

Notes that bite:

- Builder setters follow the **field names**: `maturity` (not `maturity_date`),
  `frequency` (not `payment_frequency`).
- `BaseRateSpec::Floating` is a **tuple variant** wrapping the canonical
  `finstack_quant_cashflows::builder::FloatingRateSpec` — not a struct variant
  with `index_id` / `margin_bp` fields.
- `RevolvingCreditFees::flat` returns `Result` (non-finite bp are rejected);
  `flat_bp` takes typed `Bps` and does not.
- `recovery_rate` is required and must be a finite decimal in `[0, 1]`.
- `leq` (loan-equivalent exposure, Basel CCF) is the fraction of the undrawn
  commitment assumed drawn at default, in `[0, 1]`; it defaults to `0.0`.
- A facility `credit_curve_id` on a stochastic facility requires a
  `CreditSpreadProcessSpec::MarketAnchored` process on that same curve (the
  synthesized default already is). An explicit `Cir`/`Constant` process
  ignores the curve, so hazard CS01 would silently report zero; `validate()`
  rejects the combination.
- `antithetic` and `use_sobol_qmc` are mutually exclusive; `validate()` rejects
  the combination.

## Cashflow engine and sign conventions

Lender perspective:

- Principal draws are negative (capital deployed).
- Principal repayments are positive.
- Interest and every fee are positive, posted at period end.

**Deterministic mode** slices each period around intra-period draw/repay
events, accrues interest and fees on the exact drawn balance in each sub-period,
and posts principal on the contractual event dates.

**Stochastic mode** consumes simulated factor paths observed on the
facility's observation grid (accrual boundaries plus term-index reset dates).
Accruals use the average of start and end utilization; the matching principal
delta is posted at the **midpoint of the simulated interval** (`[period start,
period end]`, or `[as_of, period end]` for the period containing the valuation
date), which is the unbiased timing for a change occurring uniformly within the
interval and keeps the funding leg consistent with the average-utilization
accrual. Any outstanding balance is repaid at maturity. Term-index coupons are
re-fixed at every reset date inside the period, so a reset frequency shorter
than the payment frequency is honoured in both engines.

For a stochastic facility `CashflowScheduleSource::raw_cashflow_schedule` (and
therefore theta carry and the JSON cashflow exporters) returns the **expected
schedule**: the per-path schedules averaged flow by flow, which is well
defined because every path books its flows on the same dates.

Same-date flow ordering is deterministic: interest/reset → fees →
amortization/PIK → notional.

Both modes emit a `CashFlowSchedule`, so metrics and exporters see one shape.

### Fee math

For a sub-period `[t_i, t_{i+1}]` with accrual factor `dt`, commitment `C` and
drawn balance `B`:

```text
interest (fixed)    = B * r * dt
interest (floating) = B * (max(index, floor) + margin) * dt
commitment fee      = (C - B) * commitment_bp * 1e-4 * dt
usage fee           = B       * usage_bp      * 1e-4 * dt
facility fee        = C       * facility_bp   * 1e-4 * dt
```

Tiered fees select the highest tier whose threshold is at or below the current
utilization. Fee tiers must be sorted by threshold ascending — `validate()`
enforces it.

### Survival weighting and the default leg

```text
PV = Σ_i CF_i * DF(t_i) * SP(t_i)
   + Σ_k PD(t_{k-1}, t_k) * DF(t_k) * [ R * E_drawn(t_k) + LEQ * U(t_k) * (R − 1) ]
   + PV(upfront fee)
```

The second line is the default leg on a monthly-or-finer grid: recovery `R` on
the drawn balance `E_drawn`, plus the loan-equivalent draw `LEQ · U` of the
undrawn commitment `U`, which the lender funds at par and recovers at `R`.

- With `credit_curve_id` and no path data, `SP(t)` comes from the hazard curve
  at each cashflow date.
- On a stochastic credit path, the simulated spread maps to hazard via
  `λ_t ≈ s_t / (1 − R)`, integrated cumulatively with linear interpolation
  between grid points to give `SP(t) = exp(−∫λ)`.

## Stochastic utilization

```rust
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
    DrawRepaySpec, StochasticUtilizationSpec, UtilizationProcess,
};

let stochastic = DrawRepaySpec::Stochastic(Box::new(StochasticUtilizationSpec {
    utilization_process: UtilizationProcess::MeanReverting {
        target_rate: 0.5,
        speed: 1.0,
        volatility: 0.15,
        spread_sensitivity: 0.0, // > 0 links the target to the simulated spread
    },
    num_paths: 10_000,
    seed: Some(42),
    antithetic: false,
    use_sobol_qmc: false,
    mc_config: None,     // Some(McConfig { .. }) enables rate and credit dynamics
}));
```

Factors, when `McConfig` is supplied:

- **Utilization** — clamped Ornstein-Uhlenbeck. Each step uses the exact OU
  transition and is then clamped to `[0, 1]`. Keep the stationary standard
  deviation `volatility / sqrt(2 * speed)` small relative to the distance from
  `target_rate` to the nearest boundary, or the clamp biases the simulated mean
  toward the interior. With `spread_sensitivity` β > 0 the target follows the
  simulated spread, `θ(t) = clamp(target_rate + β · (s(t)/s(0) − 1), 0, 1)`:
  a borrower whose spread doubles draws toward `target_rate + β`. This is the
  "draw on spread level" channel, on top of the shock correlation below; it
  needs a stochastic spread process to have any effect. Zero `volatility`
  freezes utilization (parity mode) in both the standalone facility and the
  structured-credit pool engine.
- **Short rate** — `InterestRateProcessSpec::HullWhite1F`. With `sigma > 0` the
  pricer fits θ(t) to the facility's discount curve and reads the initial rate
  from it, ignoring the supplied `initial`/`theta`. The simulated rate is the
  OIS numeraire (pathwise bank-account discounting); a term-index fixing is
  rebuilt as `F_index(t) + (r_t − f_OIS(t))`, so the OIS/index basis is kept
  and the σ → 0 limit reproduces the deterministic-forward valuation. With
  `sigma == 0` the supplied constants are used verbatim (deterministic parity
  mode), which does **not** coincide with the σ → 0 limit unless the supplied
  constants are curve-consistent.
- **Credit spread** — `CreditSpreadProcessSpec::{Cir, Constant, MarketAnchored}`.
  `MarketAnchored` anchors the initial spread and mean level to a hazard curve
  and scales volatility from a CDS index implied vol.

Correlation across the three factors comes from `McConfig::correlation_matrix`
(3×3, symmetric, positive semi-definite) or from the
`util_credit_corr` shortcut, which builds
`[[1, 0, ρ], [0, 1, 0], [ρ, 0, 1]]`.

**Adverse selection by default**: when the facility carries a hazard curve and
no explicit `McConfig` is supplied, the synthesized config uses a positive
utilization–credit correlation and a genuinely stochastic credit spread, so
spread up ⇒ utilization up ⇒ higher exposure at default. Pass an explicit
`McConfig` with `util_credit_corr: Some(0.0)` to disable it.

**Determinism**: the seed is always fixed (`None` falls back to 42), so
bump-and-reprice sensitivities reuse the same variates for base and bumped runs
(common random numbers) and finite-difference Greeks carry no MC noise.

**Clock**: simulation time, pathwise survival and the pathwise bank account run
on the ACT/365F model clock (`MC_CLOCK_DAY_COUNT`), the clock the rate and
credit processes are calibrated on; interest and fee accrual keep the
facility's `day_count`. Seasoned facilities simulate from the valuation date
with the current drawn amount as the known t₀ state.

## Pricing

`RevolvingCreditPricer` is registered under both `ModelKey::Discounting` and
`ModelKey::MonteCarloGBM` in
[`src/pricer/fixed_income.rs`](../../../pricer/fixed_income.rs):

| Mode | Behavior |
|------|----------|
| Deterministic | Single schedule, discounted and survival-weighted. |
| Monte Carlo | Path generation, per-path deterministic pricing, MC aggregation; PV is the mean estimate. |

`Instrument::value(&market, as_of)` picks the mode from `draw_repay_spec`
(`Deterministic` vs `Stochastic`) rather than from the requested model key, so a
stochastic facility has one canonical value regardless of which public entry
point invokes it. Setting `attributes.meta["pricing_model"]` to a `ModelKey`
string routes through the registry instead.

For per-path detail use the pricer directly:

```rust
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCreditPricer;

// Requires DrawRepaySpec::Stochastic; a deterministic spec is a validation error.
let enhanced = RevolvingCreditPricer::price_with_paths(&facility, &market, as_of)?;
let mean_pv = enhanced.mc_result.estimate.mean;
let per_path = &enhanced.path_results;
let option_cost = enhanced.draw_option_cost.mean; // negative when spreads widened
```

**Draw option cost.** Each simulated draw `ΔD` at observation `t_d` is a
forward loan to maturity at the contractual margin `s_K` (the spread over the
index, or the fixed rate less the par forward to maturity) against the path's
fair spread `s(t_d)`, worth `ΔD · (s_K − s(t_d)) · A(t_d, T)` to the lender
with `A` the risky annuity of the remaining accrual periods on the path
(`Σ DF · SP · dt`). `PathResult::draw_option_cost` sums the path's draws,
`EnhancedMonteCarloResult::draw_option_cost` is the antithetic-aware Monte
Carlo estimate, and `MetricId::custom("draw_option_cost")` reports its mean.
The cost is negative when the path's spread sits above the margin, the
opportunity cost the lender bears on fixed-margin commitments; a constant
spread process equal to the margin gives exactly zero. Loan-equivalent draws
at default are priced in the default leg, not here.

### Rate conventions

`BaseRateSpec::Fixed` uses the contractual rate. Floating facilities project
term forwards for term indices (`USD-SOFR-3M`, EURIBOR) and compound daily
overnight fixings when the index is a registered overnight RFR (`USD-SOFR-OIS`)
or `FloatingRateSpec.overnight_compounding` is set. Reset lag is applied on the
reset grid. Gearing, spread, and floors/caps apply after the index rate in both
cases.

## Metrics

Registered for `InstrumentType::RevolvingCredit` in `metrics/mod.rs`:

| `MetricId` | Meaning |
|-----------|---------|
| `Dv01`, `BucketedDv01` | Parallel and key-rate curve risk |
| `Cs01`, `BucketedCs01` | Par-spread rebootstrap CS01 with a replayable credit curve; z-spread CS01 otherwise |
| `custom("utilization_rate")` | Drawn / commitment at the valuation date |
| `custom("available_capacity")` | Commitment − drawn |
| `custom("weighted_average_cost")` | Approximate all-in cost of the facility |
| `custom("draw_option_cost")` | Monte Carlo mean draw option cost of a stochastic facility (`0` for deterministic schedules) |

`Theta` is registered universally by `metrics::standard_registry()`.

## Bindings

- **Python**: the typed `finstack_quant.valuations.instruments.RevolvingCredit`
  class (`builder()`, `example()`, `from_json` / `to_json` / `to_dict`,
  one getter per Rust field, `price` / `metric` on the `price_instrument`
  pipeline, `expected_cashflows(market, as_of)` returning a
  `CashFlowSchedule`, and `price_with_paths(market, as_of)` returning an
  `EnhancedMonteCarloResult` with the per-path PVs, draw option costs and
  factor trajectories plus `to_dataframe()`). Instances are accepted by
  `price_instrument`, `instrument_cashflows_json` and
  `AssetPool.with_instruments`.
- **WASM**: the JSON-first `valuations.instruments.RevolvingCredit` class
  (`fromJson`, `example`, `toJson`, `id`); price with
  `valuations.instruments.priceInstrument` and the other generic entry points.

Notebooks:
[`loans_and_credit_facilities.ipynb`](../../../../../../finstack-quant-py/examples/notebooks/02_pricing/instruments/loans_and_credit_facilities.ipynb)
and, for pools of facilities tranched into notes,
[`loan_pool_tranching.ipynb`](../../../../../../finstack-quant-py/examples/notebooks/02_pricing/instruments/loan_pool_tranching.ipynb).

## Limitations

- CSA/funding adjustments are external; discounting is curve-driven with no
  embedded FVA/CVA/DVA.
- Covenant modeling is out of scope here — see `finstack-quant-covenants`.
- Single currency per facility throughout the lifecycle.
- No PIK or amortization on the revolver itself; those belong to
  [`../term_loan/`](../term_loan/).

## Verification

```bash
# Revolving-credit unit + integration tests (incl. deterministic/MC parity)
cargo nextest run -p finstack-quant-valuations --test instruments revolving_credit::

# Whole workspace (never `cargo test` — it runs doctests)
mise run rust-test

# Lints
mise run rust-lint
```

`tests/instruments/revolving_credit/revolving_credit_parity.rs` asserts that a
zero-volatility stochastic configuration reproduces the deterministic PV;
`revolving_credit_properties.rs` covers utilization bounds, undrawn arithmetic,
event/balance consistency, cashflow ordering and fee non-negativity.

## See also

- [`../../README.md`](../../README.md) — instrument module map and how to add one
- [`../term_loan/README.md`](../term_loan/README.md) — the funded-balance sibling
- [`INVARIANTS.md`](../../../../../../INVARIANTS.md) — Decimal/f64, determinism and serde invariants
