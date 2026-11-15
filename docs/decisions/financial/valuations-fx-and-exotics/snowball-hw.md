# VALUATIONS-FX-AND-EXOTICS-SNOWBALL-HW: Snowball and inverse-floater coupon recursion and HW1F discounting

- Record: `VALUATIONS-FX-AND-EXOTICS-SNOWBALL-HW`
- Decision status: **Accepted 2026-11-15** — complete snowball and inverse-floater coupon, lifecycle, dated Hull–White, approximation and risk package.
- Implementation status: **Observed** in the owning paths below. Reconciliation to the accepted contract is pending.
- Validation status: Independent seasoned cash, recursive Gaussian and inverse-floater floorlet calculations checked. Production pricing, risk and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for snowball and inverse-floater notes

1. **Keep two explicit noncallable variants.** Require positive Money
   face in note currency, explicit long or short, finite annual decimal
   rates, a nonnegative coupon floor and, when supplied, a finite cap
   strictly above that floor. Snowball's raw rate is
   `previous_applied_rate + fixed_rate - index_fixing`. Apply the floor then
   the cap and carry the applied rate into the next reset. A supplied
   nonnegative initial rate seeds the first calculation without creating
   its own cash. Inverse floater's raw rate is
   `fixed_rate - gearing*index_fixing`, with finite positive gearing and
   no prior-coupon state. Keep variant-specific inputs and retire the
   unsupported callable field.
   [ICE snowball](https://idd.ice.com/IRHelp/Content/FM/Snowball.htm),
   [Treasury advisory committee inverse-floater examples](https://home.treasury.gov/system/files/221/TBACCharge2Q22024.pdf).

2. **Date fixing, accrual, payment and redemption separately.** Use ordered
   contractual in-advance fixings and their actual index start and end
   dates. Supply coupon accrual intervals with complete day-count context,
   actual coupon payment dates and dated par redemption. Interest cash is
   face times applied annual rate times coupon accrual fraction. A reset
   determines its coupon and its payment event settles that dated cash.
   Preserve every pending coupon independently, including when another
   reset comes before its payment. Order co-dated events by qualified
   contractual phase.

3. **Admit seasoned notes through authorized state.** Use the exact eligible
   fixing prefix or a qualified snowball snapshot carrying its covered
   prefix, last applied rate and outstanding fixed cash, bound to the
   same terms, schedule, authority and information cut. Benchmark identity
   and publication authority are separate from the projection curve.
   An unpublished same-day fixing is pending. A qualified time-zero
   projection is a model estimate. Required missing history or unresolved
   settlement errors. Retain fixed unpaid cash through discharge, discount
   eligible same-day cash at one and require a qualified overdue or partial
   residual with its actual eligible payment date.
   [Fixing authority](../core-market/scalar-series-and-fixings.md),
   [Dated cash valuation](../core-conventions/016-discounting.md).

4. **Use the dated multiplicative Hull–White basis.** Resolve distinct
   projection and discount roles for the actual index period. For fixing
   `f`, index start `S` and end `E`, multiply the conditional discount
   bond ratio `P(f,S)/P(f,E)` by inception projection gross divided by
   inception discount gross for those same endpoints. Use the checked
   combined log calculation and return the signed simple rate with
   `expm1(log_gross)/index_fraction` under the positive-gross rule. Replace
   the additive simple-rate offset. Preserve native curve clocks, model
   ACT/365F, index day count and coupon day count. Curve coverage reaches
   every required index end, including those beyond the last payment. The
   selected basis is deterministic.
   [Accepted dated term projection](../models-analytic/rate-clock-and-projection.md).

5. **Use exact event simulation for live stochastic pricing.** Select
   one complete finite scalar `kappa,sigma >= 0` pair or an explicitly
   qualified cap/floor fit supplying the pair. Step the centered rate
   factor and log bank account jointly at exact required events. Give
   each path its own applied-rate state and pending coupons. Discount
   each cashflow at its actual payment with the path bank account.
   Require explicit seed, pairing and pricing group count, and report
   actual work, standard error and the 95% sampling interval. Resolve
   fixed cash by dated discounting and the zero-volatility limit
   deterministically. Exact outcomes report zero simulated work.
   [Accepted Hull–White simulation](../models-analytic/hw1f-monte-carlo-discounting.md).

6. **Retain an explicit inverse-floater approximation.** For the selected
   `Discounting` route only, apply the floor and cap to each pending
   inverse-floater coupon's qualified dated forward, value fixed coupons
   from authorized rates and discount the actual payment dates. Label
   the forward substitution as an approximation. It omits floor and cap
   time value and can omit the payment-measure adjustment even for a
   linear coupon. Its error can have either sign. It supplies neither the
   default stochastic valuation nor a fallback. Independently benchmark
   Hull–White inverse floaters using the payment-measure floorlet
   decomposition `floor + g*((K_floor-L)^+ - (K_cap-L)^+)`, with
   `K_bound=(fixed_rate-bound)/g` and the second put omitted when uncapped.
   [Accepted analytic kernels](../models-analytic/hull-white-analytic-kernels.md).

7. **Use selected-basis rate risk and resolved cash export.** Report
   signed parallel and triangular bucketed DV01 in native currency per
   positive bp from actual represented widths. Shock each distinct active
   curve once, preserving its roles, and rebuild affected projections and
   discounts. Freeze authorized history and the selected scalar parameters.
   Pair matching groups for simulation risk. Retire the inactive surface
   field and omit surface vega. Export known signed coupons as `Floating`
   and par as `Notional` with actual retired par and its date separate from
   cash. Restore and validate the same Rust state in the existing Python
   and WASM JSON routes and generated schemas.
   [Accepted rate risk](../valuations-framework/finite-difference-rates.md),
   [Cashflow semantics](../core-conventions/015-cashflow-semantics.md).

## Independent calculations

- Face 1,000, initial applied rate 5%, fixed rate 2%, floor 1%, cap 6%,
  index fixings 0.5%, 9%, -1% and 3%, and coupon fractions 0.25, 0.50,
  0.25 and 0.50 give snowball coupon rates 6%, 1%, 4% and 3%. Coupons
  are 15, 5, 10 and 15. With the first already discharged and factors
  0.99, 0.97 and 0.95 for the remaining dates, PV including par is 978.90.
- A two-reset snowball check used flat 3% discount and 4.5% projection
  curves, `kappa=.08`, `sigma=.03`, initial rate 1.8%, fixed 4%, floor
  0.5%, cap 3.5% and face 100. The fixing/start/end/payment coordinates
  in model days are 90/92/183/192 and 181/183/274/285, with each index
  and coupon fraction 91/360. Gaussian integration gives PV 98.46928610
  with the second reset before the first payment.
- At 2026-01-02, a single-period inverse floater uses fixing 2028-01-03,
  index period 2028-01-05 to 2028-07-05, payment 2028-09-05, index
  fraction 182/360 and coupon fraction 182/365. Face is 100, discount
  is flat 3.2% continuous, `kappa=.07`, `sigma=.025` and the coupon
  clips `7%-1.5*index` to 1%–4%. At inception index forward 4%, payment
  measure floorlets give PV 92.77913077 versus forward-clipped 92.24880256.
  At 2%, the ordering reverses to 93.10598123 versus 93.62189688.
  Independent Gaussian integration agrees with the floorlets.

## Observed implementation and scope

Interest-rate snowballs recursively accumulate coupon rates; inverse floaters apply gearing to a term floating rate. Both default to HW1F MC, with an explicit deterministic-forward approximation only for noncallable inverse floaters.

The following sections describe the audited implementation and qualification
limits. The accepted contract above governs reconciliation.

## Algorithm and conventions

Period boundaries are start date followed by payment dates. Snowball raw coupon is `previous_coupon+fixed_rate-floating_rate`; inverse-floater raw coupon is `fixed_rate-gearing×floating_rate`. Apply floor then optional cap. The snowball formula does **not** use gearing. Multiply the annual decimal coupon by the contractual day-count accrual and notional; add principal at final payment.

Forward discounting samples the tenor-validated forward curve at each reset, applies the coupon rule recursively, and discounts payment amounts by the discount curve. Applying floor/cap to the forward is an approximation to the expected nonlinear coupon, not an option model.

HW MC derives a curve-fitted short-rate process and term-forward bond coefficients. At future reset `t`, model term rate receives a deterministic additive basis equal to quoted forward at reset minus the discount-curve model forward. At start equal to valuation, use the projected term fixing deterministically. Reset events generate pending coupon; next period start/payment event flushes it through reciprocal simulated bank account. Append maturity event so last coupon and principal discount to actual payment. Store previous coupon separately on every path. If no stochastic reset remains, return an exact deterministic estimate.

The shared note adapter attaches MC diagnostics. Only combined parallel DV01 and triangular bucketed DV01 are registered; they reprice the stochastic model with the selected seed/config. A volatility surface dependency can be declared but does not itself calibrate parameters in the pricing path.

## Selection and controls

`MonteCarloHullWhite1F` is the default for both variants. `Discounting` is registered but rejects the snowball variant and all callable structures. Shared HW parameter/config resolution is the same CapFloor-family strict resolver described in the callable range record.

Reject valuation strictly after start (unless already matured, which is zero), because realized coupon/fixing state is unavailable. Callable snowballs error even though a call provision is representable. Payment dates strictly increase; gearing positive, floor finite/nonnegative, cap above floor, and snowball initial coupon nonnegative. Shared defaults: 10,000 estimator streams, antithetic true, four minimum inter-event steps.

## Limitations, failure and fallback

Snowball gearing omission and deterministic-basis dynamics are observed choices. The explicit discounting route approximates cap/floor convexity. Seasoned and callable contracts are unsupported, not silently approximated. Payment date is treated as already settled on that date (`as_of>=final_payment`), unlike FX end-of-day treatment. Declared volatility dependencies do not imply surface-based sigma risk.

## Alternatives and tradeoffs

Deterministic discounting is supported only as an explicit inverse-floater approximation. A dedicated callable/seasoned augmented-state engine is absent. Stochastic basis or separate projection-rate dynamics could alter option value relative to deterministic additive spread.

## Evidence

- [finstack-quant/valuations/exotics/src/exotics/snowball/metrics.rs](../../../../finstack-quant/valuations/exotics/src/exotics/snowball/metrics.rs)
- [finstack-quant/valuations/exotics/src/exotics/snowball/pricer.rs](../../../../finstack-quant/valuations/exotics/src/exotics/snowball/pricer.rs)
- [finstack-quant/valuations/exotics/src/exotics/snowball/types.rs](../../../../finstack-quant/valuations/exotics/src/exotics/snowball/types.rs)
- [finstack-quant/valuations/exotics/src/exotics/hw1f_note.rs](../../../../finstack-quant/valuations/exotics/src/exotics/hw1f_note.rs)

- [finstack-quant/valuations/exotics/tests/exotic_harness/hw1f_curve_repricing.rs](../../../../finstack-quant/valuations/exotics/tests/exotic_harness/hw1f_curve_repricing.rs): HW discount-curve consistency and pathwise bank-account evidence; does not validate every recursive coupon contract.

Canonical reference index: [Hull White](../../../REFERENCES.md#hull-white-1990-pricing-interest-rate-derivatives) and [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo). Production qualification remains pending.

## Follow-up tracking

Implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
