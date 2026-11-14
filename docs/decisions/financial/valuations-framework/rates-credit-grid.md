# VALUATIONS-FRAMEWORK-05 — Rates-credit calibration targets, recovery weights, and floating node coupons

- Decision status: **Accepted — complete checked dated rates-credit valuation package, 2026-11-13**.
- Implementation status: **Observed; reconciliation pending** across dated preparation, product pricing, risk and hosts.
- Validation status: Source and primary references inspected; independent bounded date, arithmetic and finite-state examples checked. Runtime, product-price, risk and host qualification remain pending.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked dated rates-credit valuation

Apply the accepted [marginal fit](../models-numerical/rates-credit-marginal-calibration.md#accepted-decision-for-checked-rates-credit-marginal-calibration),
[coupling and sampling](../models-numerical/rates-credit-coupling-and-sampling.md#accepted-decision-for-checked-rates-credit-coupling-and-path-sampling)
and [coupon rollback](../models-numerical/rates-credit-coupon-folding.md#accepted-decision-for-checked-joint-credit-rollback-and-coupon-folding)
to the dated valuation adapter. This package fixes the inputs, preparation and
eligibility of the shared operations. Individual product payoffs and model
dispatch remain with their owning records.

1. **Resolve the model origin and required coverage from product events.**
   The joint bond starts at the effective valuation date; the joint term
   loan starts at settlement. Keep the valuation information phase, model
   origin and loan's separate as-of settlement carry distinct. Resolve the
   last admitted adjusted payment and all other required dated events.
   Extend numerical coverage through any later index maturity needed for
   an eligible coupon. That extension supplies model factors and adds no
   recovery exposure, exercise or cash entitlement. Do not truncate a
   required adjusted payment at legal maturity. A same-day claim with no
   outgoing exposure follows the owner's qualified root payment and
   settlement rules. It needs no invented future credit interval.

2. **Prepare one dated ACT/365F grid with exact event indices.**
   Require a positive explicit number `m` of subdivisions per civil day
   for a live dated grid with positive span `d`. Use `N=d×m` checked
   intervals, terminal time `T=d/365`, and mathematical width `T/N`.
   Map a date `k` days from origin to index `k×m` exactly. Use checked
   day and remainder coordinates; whole days have the canonical `k/365`
   labels, and represented labels must strictly increase. Use exact
   indices for payments, principal events, admitted exercises and index
   dates. Remove the bond minimum-step conversion, independent horizon
   input, term-loan use of the discount clock as the tree clock, and
   date snapping, cash splitting and timing offsets used to fit the grid.
   Keep explicit actual work limits and reject size, coverage and date
   errors. Product terms still determine which dated events are live.

3. **Map each source on its own clock.** Evaluate whole-date nodes on
   the actual date using the source's base, day count, calendar and
   context. At fractional nodes use the accepted
   [fractional-day clock mapping](../models-analytic/rate-clock-and-projection.md#accepted-decision-for-checked-model-clocks-curve-fit-and-term-projection):
   interpolate the adjacent dates' native coordinates and evaluate
   the complete source there. Do not log-interpolate daily factor or
   survival outputs. A native clock plateau is valid and has no local
   source-time advance. Enforce native ordering, tails, signed-query
   admission and range. Do not clamp before a base date or substitute
   default day-count context. Fractional coordinates do not create
   calendar events or fixing dates.

4. **Prepare relative discount and local hazard targets.** Bind the
   exact discount and credit sources to the origin. Admit the discount
   anchor before returning exact `D[0]=1`; each subsequent `D[k]` is
   the checked ratio of the source factor to its anchor. A valid target
   may exceed one. Evaluate local integrated hazard `H[k]` directly
   over adjacent mapped hazard coordinates, with `H[k]≥0` finite.
   Derive cumulative log survival from `−ΣH`. Never divide rounded
   unconditional survivals, infer flat hazard from equal rounded
   survivals, or clamp an increasing survival input. A positive
   local hazard remains meaningful when a survival view rounds to one
   or zero. Discount and survival are separate marginal targets;
   recovery, OAS and correlation do not enter them, and a marginal fit
   alone does not determine joint discounted survival.
   [Discount factors](../core-market/discount-evaluation.md#accepted-decision-explicit-discount-sources-and-checked-rate-extraction),
   [local integrated hazard](../core-market/hazard-survival.md#accepted-decision-explicit-intensity-sources-and-local-probability-evaluation).

5. **Require the complete joint configuration.** Use the accepted two
   additive constant-parameter axes: explicit finite nonnegative mean
   reversions per model year and absolute volatilities per square root
   model year. Zero volatility is a deliberate one-state axis. Require
   a present nullable correlation field: numeric in `[-1,1]` only for
   two active axes, with zero meaning independence and strict
   whole-grid feasibility; null when either axis is deterministic.
   Remove zero-parameter defaults, the standalone `.15` reversion
   ceiling, probability clamps and volatility floors. Validate actual
   support, minimum variance retention, hazard floor share, marginal
   replay, coupling and work against the accepted controls. Reject
   incompatible model blocks, schedules or implied-volatility
   substitutes. Do not switch models or adjust the grid after failure.
   [Accepted valuation model inputs](model-resolution.md#accepted-decision-for-checked-valuation-model-inputs-and-controls).

6. **Use canonical interval exponents for eligible recovery at default.**
   Let `A` be the signed applied discount exponent and `H≥0` the
   local integrated intensity. Under the selected constant equivalent
   rate and intensity within the interval, the conditional discounted
   default weight is `W=H × [−expm1(−(A+H))]/(A+H)`.
   Its limits are `0` at `H=0` and `H` at `A+H=0`. Use checked
   scaled evaluation, retaining representable complete products.
   Do not derive `H` from rounded survival or log a rounded product
   of discount and survival. Negative rates can give `W>1`; this is
   a discounted weight. Validate the selected recovery fraction
   `R∈[0,1]` and eligible outstanding par after current principal
   events before zero shortcuts. Bind authored recovery or selected
   curve metadata explicitly under product terms.
   Multiply by `R` and par once, on the same applied discount basis
   as the hold. Keep survival out of common current cash and apply
   outgoing survival to continuation once. For a curved deterministic
   source this is a local equivalent-rate/intensity approximation;
   qualify interval subdivision independently. Recovery timing, the
   balance basis, accrued interest and delayed or alternative recovery
   remain product decisions. [Duffie on recovery of par](https://www.darrellduffie.com/uploads/1/4/8/0/148007615/duffiedefaultabletermmodels1998.pdf).

7. **Admit the actual parent and fixing for a floating fold.** Require
   one uniquely identified settlement parent and one simple in-advance
   term observation whose full cash dependencies are determined at
   fixing. Keep fixing `f`, index effective `s`, maturity `e`, payment
   `p` and coupon accrual dates distinct. Require exact nodes
   `0≤f≤s<e` and `f<p`. Payment before index maturity is allowed only
   for qualified terms with the amount already determined at fixing.
   Use exact publication eligibility at the valuation information phase.
   A known fixing with resolved amount dependencies remains ordinary
   cash. An unresolved observation before settlement cannot be moved to
   the first loan node or assumed known merely because it precedes model
   origin. Reject missing or conflicting active parents or terms,
   including unsupported multiple or overnight observations, future
   state-dependent balances or accrual, and stochastic capitalization.
   A legitimate zero parent fraction or principal does not bypass
   validation. [Resolved term observations](../cashflows/06-floating-rate-projection.md#accepted-decision-resolved-term-observations-exact-rate-controls-and-strict-failures),
   [parent settlement](../cashflows/08-floating-replay.md#accepted-decision-parent-bound-replay-exact-full-settlement-and-explicit-timing-ownership).

8. **Project the gross index growth on the fitted rate marginal.**
   Use `G_f=G_0×[B(f,s)/B(f,e)]/[B(0,s)/B(0,e)]`, with explicit
   market index growth `G_0` and the achieved rate-only unit bonds
   `B` of the same prepared tree. Use checked log ratios and divide
   the growth increment by the positive contractual index fraction.
   Preserve the market rate at the origin and on a deterministic
   rate axis after validation. Keep hazard, OAS, recovery and exercise
   out of the unit bonds. This preserves the uncomposed index gross's
   first moment in its maturity-forward measure under deterministic
   basis. It does not establish a nonlinear, displaced-payment or
   risky coupon price. Remove the additive reset-to-payment proxy,
   timing-scale adjustment and small-factor floor.
   [Henrard's multicurve model note](https://quant.opengamma.io/Hull-White-One-Factor-Model-OpenGamma.pdf).

9. **Compose and fold one full coupon without removing legal rights.**
   Use the parent's admitted principal and accrual fraction, apply its
   bounds, gearing and spread in the accepted order, and settle signed
   cash through the canonical Money boundary. Apply the accepted joint
   unit claim to the exact payment node with the same discount and
   survival as continuation. Replace exactly one ordinary projected
   price contribution; preserve reporting identity and unrelated cash.
   Decline the fold for a live intervening exercise or termination right,
   an intervening change to the fixed amount or entitlement, or a payoff
   or recovery that depends on the pending coupon. At fixing and payment
   apply the owner's phase rules and the accepted rollback eligibility.
   Do not delete call dates, strip bounds or quietly skip an incomplete
   descriptor. An alternative route requires explicit qualification.

10. **Share the prepared basis across product, price, OAS and risk.**
    Reconcile the joint term-loan adapter, deterministic hazard bond,
    deterministic and sampled joint bond routes, and convertible
    recovery callers where they claim these interval economics. Keep
    bond path-dependent replay and other product payoffs in their
    owners. Carry the exact active sources, phases, grid and fit
    diagnostics. Stochastic joint pricing uses Continuous OAS;
    deterministic alternatives require the owner's explicit admitted
    basis. Reprice applied discount, recovery and coupon folds for
    each OAS trial or relevant shock. Refit affected sources and model
    inputs as required; preserve checked errors and the product's
    accepted price-residual gate. Terminal nodes have no outgoing
    rate, hazard, discount or OAS. An explicitly selected observed
    price branch does not need an unused model fit. Keep the grid,
    target arrays and folds Rust-only. Reconcile existing native and
    registry routes, Python/WASM inputs, schemas, error mapping and
    parity. Qualify actual
    work by route, and compare independent marginal and joint replay,
    cash timing, recovery and risk on feasible grids. Refinement can
    fail the binary support or work gates. Grid stability, marginal
    fit and sampled standard errors establish different things.

## Observed behavior

### Choice and input domain

The valuation adapter maps dated discount/survival curves onto the two-factor lattice's uniform time coordinate and maps floating resets/payments onto that grid. It also computes continuous fractional-recovery-of-par interval weights. These are separate from the lattice transition/backward-induction kernel in the models crate. The implementation's date-based targets preserve different curve bases/day counts rather than evaluating both curves at one assumed year fraction.

### Algorithm

For `steps=n`, date position at slice `k` is `k × calendar_days(origin,horizon_date)/n`. Query the lower and upper integer dates and log-interpolate each positive discount factor and conditional survival: `x=exp((1−w)ln x_lower+w ln x_upper)`. Survival is `S(date)/S(origin)`; nonpositive/nonfinite origin survival fails. Force slice zero to `(DF,S)=(1,1)`. Time values themselves are `k × horizon/n`; the caller supplies the horizon clock.

The daily bond adapter chooses `substeps_per_day=max(ceil(minimum_steps/days),1)` and total steps `days × substeps_per_day`, with horizon `days/365`. Thus the requested step count is a minimum, not the actual number of steps; every calendar date lands on a grid slice. Zero steps, nonpositive horizon/date span and overflow fail.

For an interval discount ratio `d` and survival ratio `s`, let `H=−ln(s)` and `x=−ln(ds)`. Continuous FRP weight is `H(1−exp(−x))/x`. At `|x|<1e−8`, use `H(1−x/2+x²/6)`; zero hazard returns zero. Discount must be positive finite, survival in `(0,1+1e−12]`, with the tiny excess above one clamped to one. This is the constant force-of-interest/hazard integral over an interval; callers multiply by recovery/notional and their start-state weight.

Floating node descriptors scan `CFKind::Floating` flows for future reset, accrual metadata, a projected index rate and positive finite accrual. They snap accrual start and payment time upward to the next slice. A reset/payment collapsing to one slice errors and asks for a finer grid. Set `base_discount_forward=(DF(reset_slice)/DF(payment_slice)−1)/τ`, `timing_scale=DF(actual_payment_time)/DF(payment_slice)`, and take notional at the reset slice. Preserve the separately projected base index rate to retain multicurve basis. An explicit flag strips index floor/cap constraints; PIK detection independently identifies future PIK flows for caller gating.

### Selection, controls, and limitations

`resolve_rates_credit_config` reads HW rate volatility/reversion, hazard volatility/reversion and correlation from model overrides; each omitted scalar defaults to zero. It rejects a volatility schedule, Black–Derman–Toy/Black–Karasinski inputs, or an implied-vol quote used in place of HW volatility. Mean reversions obey the lattice's `KAPPA_MAX`; nonzero correlation requires both volatilities positive. This deterministic default is an actual fallback, not evidence that zero volatility was calibrated or approved.

Calibration target validation checks positivity, not independently monotonic survival; the downstream lattice owns additional feasibility checks. Skipping descriptors with absent projected rates or invalid accrual does not itself validate a complete floating schedule. The descriptor timing uses accrual start after using reset date only to decide whether the reset is future.

## Alternatives and evidence

A date-aligned irregular lattice, analytic discount-survival integration, or finer grids are alternatives, not implementations selected by this adapter.

- [rates_credit.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/rates_credit.rs): date-grid targets and FRP weight; inline tests provide implementation/invariant evidence.
- [floating_reset_descriptors.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/floating_reset_descriptors.rs): node coupon construction and grid failure.
- [pricing_overrides.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing_overrides.rs): rates-credit configuration and incompatible-input checks.
- [Duffie–Singleton](../../../REFERENCES.md#duffie-singleton-1999) and [Hull–White numerical procedures](../../../REFERENCES.md#hull-white-1994-numerical-procedures) provide general model context; they do not validate this adapter's interpolation or zero-parameter defaults.
- [Bond tree preparation](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/tree_pricer.rs), [deterministic hazard bond](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/hazard.rs), [term-loan tree](../../../../finstack-quant/valuations/fixed-income/src/term_loan/pricing/tree_engine.rs) and [convertible valuation](../../../../finstack-quant/valuations/fixed-income/src/convertible/pricing/engine.rs) are live preparation, rollback and recovery routes. The [bond replay record](../valuations-bonds/06-joint-coupon-replay.md) and [term-loan pricing record](../valuations-other-fixed-income/08-term-loan-pricing.md) retain their product questions.

### Independent review evidence

- From 2024-02-28 to 2024-03-02, two subdivisions per day give
  six intervals, whole-date indices `0,2,4,6` and horizon `3/365`.
  For a native Linear discount factor from `1.02` to `1.00`,
  the midpoint relative to the lower-date anchor is `1.01/1.02`
  or about `.99019607843137`. Log interpolation of the endpoint
  factors instead gives `.99014754297667`.
- On an ACT/360 source day with intensity `.01` for the first half
  and `.03` for the second, the first-quarter local hazard is
  `.01/1440`, about `.00000694444444`. Geometric interpolation of
  whole-day survival would assign double that hazard. If prior
  cumulative hazard is `800`, both absolute survivals round to
  zero even though a new local hazard `.02` gives conditional
  survival `.9801986733067553`. Local `H=1e−20` remains positive
  even when its survival rounds to one.
- At `κ=.20` and `T=1`, the worst raw variance retention is
  about `.9602406687` on a 365-step grid and `.9601204411`
  on a 730-step grid. An explicit `.9602` gate admits the first
  and declines the second. Equally weighted discounts `.9,1.1`
  and survivals `.8,1` give the same marginals for any pairing,
  but joint discounted survival is `.91` or `.89` for the two
  extreme pairings and `.90` for independence.
- Recovery weight at `(A,H)=(.03,.02)` is `.0195082301997144`;
  at `(−.10,.10)` it is `.10`; at `(−2,1)` it is about `1.71828`.
  With `A=.03,H=1e−18` the weight is about `9.85149e−19`
  even though survival rounds to one. For `A=.05,H=.1`, putting
  all hazard in the first or second half gives respective weights
  `.0940024779323237` and `.0916815484826199`, versus
  `.0928613490499615` for the single constant-equivalent interval.
  Same totals alone do not fix within-interval recovery value.
  With `A=.03,H=.02`, continuation `100` before discount/survival,
  current due cash `5`, recovery fraction `.4` and post-event par
  `80`, the value is `100.74720581646226` when outgoing survival
  is applied to continuation once and current cash remains separate.
- At a two-state fixing, take rate state prices `.49,.49`,
  `B(f,s)=(.99,.97)`, `B(f,e)=(.97,.91)` and market gross `1.01`.
  The achieved origin ratio is `49/47`. Node gross values
  are about `.9887502630` and `1.0326508186`. The index-maturity
  forward weights `97/188,91/188` recover `1.01` exactly.
  With index fraction `.25`, a zero rate floor, a `.002` spread
  and separate coupon fraction `.20`, the one-million principal
  coupons are `400` and about `26,520.65485535` before Money
  finalization. The first-moment identity does not price the floor.

Qualification still requires independent finite-state price and payoff
replay, dated fixing and lagged-settlement fixtures, product recovery
and event timing checks, feasible-grid price and risk comparisons, and
runtime and host evidence.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
