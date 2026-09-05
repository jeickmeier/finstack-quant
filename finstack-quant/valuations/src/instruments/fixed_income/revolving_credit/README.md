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
| `RevolvingCreditPricer` | `price_with_paths(facility, market, as_of)` for full Monte Carlo path capture. |
| `EnhancedMonteCarloResult`, `PathResult` | MC statistics plus per-path PV, cashflows and factor trajectories. |
| `PathAwareCashflowSchedule`, `ThreeFactorPathData` | Cashflow schedule carrying the simulated factor path. |
| `ZERO_TOLERANCE`, `UTILIZATION_CHANGE_THRESHOLD`, `INTERPOLATION_TOLERANCE`, `MIN_CIR_SPREAD`, `MAX_RECOVERY_RATE` | Module numerical constants. |

Note that `pricer` and `types` are `pub(crate)` submodules — import the names
above from the module root, not from `revolving_credit::types::…`.

## Module layout

```
revolving_credit/
├── mod.rs                # re-exports, numerical constants, module overview
├── types.rs              # RevolvingCredit, fees, rate/draw specs, MC config, Instrument impl
├── cashflow_engine.rs    # single engine for both deterministic and path-driven schedules
├── utils.rs              # calendar-aware schedules, reset dates, floating projection, balance evolution
├── pricer/
│   ├── unified.rs                    # RevolvingCreditPricer: single-path PV, MC aggregation, path capture
│   ├── components.rs                 # upfront-fee PV, discount factors, survival weights, rate projection
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
    .commitment_amount(Money::new(10_000_000.0, Currency::USD))
    .drawn_amount(Money::new(5_000_000.0, Currency::USD))
    .commitment_date(date!(2025 - 01 - 01))
    .maturity(date!(2028 - 01 - 01))
    .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
    .day_count(DayCount::Act360)
    .frequency(Tenor::quarterly())
    .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0)?)   // commitment / usage / facility, bp
    .draw_repay_spec(DrawRepaySpec::Deterministic(vec![
        DrawRepayEvent {
            date: date!(2025 - 03 - 01),
            amount: Money::new(1_000_000.0, Currency::USD),
            is_draw: true,
        },
        DrawRepayEvent {
            date: date!(2025 - 06 - 01),
            amount: Money::new(500_000.0, Currency::USD),
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

**Stochastic mode** consumes simulated factor paths that observe utilization
only at period boundaries. Accruals use the average of start and end
utilization; the matching principal delta is posted at the **period midpoint**,
which is the unbiased timing for a change occurring uniformly within the period
and keeps the funding leg consistent with the average-utilization accrual. Any
outstanding balance is repaid at maturity.

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

### Survival weighting

```text
PV = Σ_i CF_i * DF(t_i) * SP(t_i) + PV(upfront fee)
```

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
  toward the interior.
- **Short rate** — `InterestRateProcessSpec::HullWhite1F`. With `sigma > 0` the
  pricer fits θ(t) to the facility's discount curve and reads the initial rate
  from it, ignoring the supplied `initial`/`theta`. With `sigma == 0` the
  supplied constants are used verbatim (deterministic parity mode). The σ → 0
  limit of the stochastic branch therefore does **not** converge to the σ = 0
  branch unless the supplied constants are already curve-consistent.
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
```

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

`Theta` is registered universally by `metrics::standard_registry()`.

## Bindings

Reachable from Python and WASM through the JSON envelope
(`InstrumentJson::RevolvingCredit` inside `finstack_quant.instrument/1`):

- **Python**: `finstack_quant.valuations.instruments.price_instrument(...)` and
  `finstack_quant.valuations.instruments.instrument_cashflows_json(...)`.
- **WASM**: `valuations.instruments.priceInstrument`,
  `valuations.instruments.instrumentCashflowsJson`.

There is no typed `RevolvingCredit` class in either binding.

Closest notebook:
[`loans_and_credit_facilities.ipynb`](../../../../../../finstack-quant-py/examples/notebooks/02_pricing/instruments/loans_and_credit_facilities.ipynb).

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
