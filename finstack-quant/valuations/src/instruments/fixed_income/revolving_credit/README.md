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
| `CommitmentStep`, `MarginStepUp`, `FeeStep` (from `loan_terms`) | Dated commitment, margin and fee changes; see "Dated terms". |
| `LetterOfCreditSpec`, `LcEvent` (from `loan_terms`) | LC sublimit, outstanding face, issuances/expiries, LC and fronting fees, LC draw at default. |
| `UpfrontFee`, `ScheduledFee`, `OidEirSpec` (from `loan_terms`) | Upfront fee as amount or percentage, dated fixed fees, effective-rate reporting switch. |
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
- Schedule conventions are typed fields: `business_day_convention` (default
  Modified Following, payment dates only), `calendar_id` (`None` = weekends
  only; an unknown id fails validation), `payment_lag_days` and
  `settlement_days` (quote metrics only). Attributes metadata is never read
  by the schedule builders.
- `drawn_amount` is the balance at the **simulation anchor**, the later of the
  commitment date and the valuation date, in both modes. Deterministic
  draw/repay events describe the future only; an event dated on or before the
  anchor is rejected by the cashflow engine. The accrual period containing the
  valuation date accrues on the anchor balance from its accrual start.
- `leq` (loan-equivalent exposure, Basel CCF) is the fraction of the undrawn
  commitment assumed drawn at default, in `[0, 1]`; it defaults to `0.0`. A
  positive `leq` (or LC `leq`) needs a default model: the standalone pricer
  rejects it without a `credit_curve_id` or a moving stochastic spread
  process, because the draw at default would be silently inert. Inside a
  structured-credit pool the deal model supplies default probabilities.
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

### Dated terms: commitment, margin and fee steps

Three schedules, all optional and all shared with `TermLoan` through
`instruments::fixed_income::loan_terms`, let a term sheet be entered without
custom code:

- `commitment_schedule: Vec<CommitmentStep { date, amount, fee_bp }>` — the
  commitment in force from each date (amortizing commitments, availability
  expiries, accordions). Utilization is always drawn over the commitment in
  force, so a stochastic facility books the implied principal at a step. A
  step down pays `fee_bp` on the reduced amount on the step date. The drawn
  balance must never exceed the commitment in force: the analyst dates the
  repayment.
- `margin_steps: Vec<MarginStepUp { date, delta_bp }>` — cumulative shifts of
  the floating spread or the fixed rate: leverage or ratings grids the
  analyst has forecast, scheduled step-ups, default-rate margins.
- `fees.steps: Vec<FeeStep { date, commitment_delta_bp, usage_delta_bp,
  facility_delta_bp }>` — cumulative shifts of every tier of the named fee.

Both engines slice accrual on every step date, so a step inside an accrual
period is exact. `commitment_at(date)`, `margin_delta_bp_at(date)` and
`fees.deltas_at(date)` expose the terms in force.

A leverage grid recipe: forecast the covenant ratio per test date, map each
ratio to the grid margin, and enter one `MarginStepUp` per date the mapped
margin changes (`delta_bp` = new margin − previous margin).

### Letters of credit

`lc: Option<LetterOfCreditSpec { sublimit, outstanding, events, fee_bp,
fronting_fee_bp, leq }>` models an LC sublimit. Outstanding letters of credit
reduce availability and the commitment-fee base, count as usage for fee
tiers, accrue the LC fee (`fee_bp`, or the floating margin including margin
steps when unset; a fixed-rate facility must set it) plus the fronting fee,
and enter the default leg as `leq × LC` funded at par and recovered at the
facility recovery rate. `outstanding` is the LC face at the simulation
anchor; `events` are future issuances and expiries. LC usage is deterministic
in both modes, and the utilization process is capped at `1 − LC(t) / C(t)`.
The fees are emitted as `CFKind::LcFee` and `CFKind::FrontingFee`.

### Upfront fee, scheduled fees and the effective rate

`fees.upfront_fee` is `UpfrontFee::Amount(Money)` or
`UpfrontFee::PctOfCommitment(0.02)`, paid on the commitment date; it enters
the present value only while that date lies after the valuation date.
`scheduled_fees: Vec<ScheduledFee { date, amount }>` are dated fixed fees
(amendment, waiver, extension, consent) emitted as `CFKind::Fee`.
`custom("oid_eir_amortization")` reports the origination effective interest
rate (`oid_eir_rate`, the XIRR of the full flow set from the commitment date
including the upfront fee and, unless `oid_eir.include_fees` is false, every
running fee) with the dated `oid_eir_amortization` and
`oid_eir_carrying_value` series.

### Recipes for customized deals

No new fields are needed for these; each is a combination of the schedules
above and is pinned by an executed test in
`tests/instruments/revolving_credit/analyst_coverage.rs`:

- **Term-out**: a `CommitmentStep` to the drawn balance on the term-out date
  (availability ends, commitment fee stops) plus dated repayments for the
  amortization.
- **Clean-down**: a repayment event into the clean-down window and a redraw
  event out of it; the balance, interest and usage fee are zero inside the
  window while the commitment fee runs on the full commitment.
- **Extension**: move `maturity` and add the extension fee as a
  `ScheduledFee` on the extension date.
- **Default margin**: a `MarginStepUp` on the default date (and one back on
  the cure date).
- **Amendment or waiver fee**: a `ScheduledFee`.

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
with `drawn_amount` as the known t₀ state, the same anchor balance the
deterministic engine starts from.

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
forward loan to maturity at the contractual margin against the path's fair
spread. The fair spread is anchored to the margin at the valuation date,
`fair(t_d) = margin + (s(t_d) − s(t₀))`, so a private-credit margin that
carries illiquidity and funding premia over a CDS-style spread does not bias
the cost; only spread changes since the valuation date count. The draw is
worth `−ΔD · (s(t_d) − s(t₀)) · A(t_d, T)` to the lender, with `A` the risky
annuity of the remaining accrual periods on the path (`Σ DF · SP · dt`).
`PathResult::draw_option_cost` sums the path's draws,
`EnhancedMonteCarloResult::draw_option_cost` is the antithetic-aware Monte
Carlo estimate, and `MetricId::custom("draw_option_cost")` reports its mean.
The cost is negative when the spread has widened since the valuation date,
the opportunity cost the lender bears on fixed-margin commitments; any
constant spread process gives exactly zero. Loan-equivalent draws at default
are priced in the default leg, not here.

### Rate conventions

`BaseRateSpec::Fixed` uses the contractual rate. Floating facilities project
term forwards for term indices (`USD-SOFR-3M`, EURIBOR) and compound daily
overnight fixings when the index is a registered overnight RFR (`USD-SOFR-OIS`)
or `FloatingRateSpec.overnight_compounding` is set. Reset lag is applied on the
reset grid. Gearing, spread, and all-in floors/caps apply after the index rate
in both cases. On an overnight index the index floor and cap apply to each daily
fixing before compounding, unless `overnight_index_constraints` is `period`,
which bounds the compounded rate once. `index_tenor` and a non-default
`fallback` are rejected: resets always project from the forward curve.

## Metrics

Registered for `InstrumentType::RevolvingCredit` in `metrics/mod.rs`:

| `MetricId` | Meaning |
|-----------|---------|
| `Dv01`, `BucketedDv01` | Parallel and key-rate curve risk |
| `Cs01`, `BucketedCs01` | Par-spread rebootstrap CS01 with a replayable credit curve; direct hazard-rate bump with an analyst-built (knot) curve; z-spread CS01 without a credit curve, which sees the undrawn commitment only through its fee annuity |
| `custom("exposure_at_default")` | Drawn + `leq` × undrawn + LC `leq` × LC face on the valuation date |
| `custom("expected_loss")` | Value removed by the default model: PV without the credit curve and contingent draws less the base PV (`0` without a credit curve) |
| `custom("utilization_rate")` | Drawn / commitment at the valuation date |
| `custom("available_capacity")` | Commitment − drawn |
| `custom("weighted_average_cost")` | Approximate all-in cost of the facility |
| `custom("draw_option_cost")` | Monte Carlo mean draw option cost of a stochastic facility (`0` for deterministic schedules) |
| `DiscountMargin` | Floating facilities: constant spread over the discount curve repricing the settlement schedule to the quoted clean price on the drawn balance plus accrued (LSTA), else to the model value; decimal |
| `custom("price_from_dm")` | Clean price per 100 of the drawn balance at settlement implied by `market_quotes.quoted_discount_margin` (decimal) |
| `Ytm` | IRR of the holder-view flows after settlement against the quoted or model price, on the facility day count |
| `custom("all_in_rate")` | Running cash cost: interest plus every fee after the valuation date over the time-weighted drawn balance |
| `custom("accrued_interest")` | Cash interest accrued to the settlement date, in the facility currency |
| `custom("oid_eir_amortization")` | Total EIR amortization; stores `oid_eir_rate` and the dated amortization and carrying-value series |

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
- No ABR or prime sub-tranche: a facility accrues on one base rate. Model a
  mixed borrowing as two facilities.
- No borrowing base: availability is the commitment schedule, not a
  collateral formula. Encode a forecast borrowing base as commitment steps.
- Extension options are not priced as options: model the exercised or the
  unexercised contract, not the choice.

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
