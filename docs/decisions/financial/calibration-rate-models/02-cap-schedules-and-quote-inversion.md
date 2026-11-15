# CALIBRATION-RATE-MODELS-002: Scheduled cap/floor market prices, HW repricing and implied-normal inversion

**Decision status:** Accepted 2026-11-15 — complete cap and floor strip preparation, pricing and implied Normal inversion package.

**Implementation status:** Observed. Reconciliation to the accepted package is pending.

**Validation status:** Independent grid, benchmark accrual, payment-measure and paired premium calculations verified. Production fitting, market references, restoration and host qualification remain pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory](README.md) and [source ledger](coverage.json).

## Accepted decision for checked cap and floor strips and implied Normal inversion

1. **Prepare one exact live term-index strip per quote.** Preserve
   contractual maturity, nominal and actual coupon dates, benchmark
   value dates, fixing publication state and actual payments. Require
   strictly ordered live fixings and nonoverlapping accruals. Admit
   gaps only on an explicitly authored and qualified strip. Include
   the first coupon when the quote convention includes it and the
   fixing has a qualified positive live horizon. A required fixed,
   unresolved or unsupported included observation makes the quote
   ineligible. Reject empty strips. A dated strip may contain one
   live optionlet.
   [Accepted cap and floor terms](../valuations-rates/05-cap-floor-pricing.md#accepted-decision-for-checked-cap-and-floor-optionlet-pricing).

2. **Use an exact regular grid for synthetic quotes.** Require the
   elected frequency `m` and checked whole period count `n`, with
   maturity exactly represented by `n/m`. Fix at each numeric period
   start and pay at its end. The live-only spot strip omits the
   time-zero fixing. Apply the nonempty requirement to the result.
   Retire rounding the period count and redistributing payments.
   [Accepted calibration profiles](01-curve-entry-and-quote-domains.md#accepted-decision-for-explicit-hull-white-calibration-entry-and-quote-domains).

3. **Price the flat Normal quote on its own clock.** Compute
   each forward from the checked projection factor ratio
   at its actual benchmark value dates and its distinct
   benchmark accrual `αx`. Apply one flat annual Normal
   volatility at the actual fixing times on the quote clock.
   Weight each unit caplet or floorlet by its coupon accrual
   `A` and actual payment factor `D(V,P)`. Preserve the
   signed strike, side and any explicitly qualified spread
   mapping.
   [Accepted dated calibration](../calibration-framework/18-hull-white-market-adapter.md#accepted-decision-for-dated-hull-white-quote-preparation-and-publication).

4. **Reprice the same strip under Hull–White.** On ACT/365F
   model time use the checked term gross-option kernel with
   deterministic multiplicative projection basis and the actual
   payment measure. Preserve eligible early and delayed payments.
   Scale the already-discounted gross-option value by `A/αx`
   per unit notional. Use the payment-measure adjustment on
   the Hull–White side and the ordinary projected forward
   on the quote side.
   [Accepted term kernel](../models-analytic/hull-white-analytic-kernels.md#accepted-decision-for-checked-hullwhite-analytic-kernels-and-payment-measures) ·
   [Henrard, §5](https://quant.opengamma.io/Hull-White-One-Factor-Model-OpenGamma.pdf).

5. **Invert one flat Normal volatility for the entire strip.**
   Compare the finite nonnegative Hull–White premium with
   intrinsic on the prepared Normal quote basis. Below-intrinsic
   premiums fail inversion. Exact resolved equality selects
   zero after full admission. For positive quote-basis time
   value `p`, solve on the explicit interval `[0,max_vol]`
   with fallible Brent and objective
   `tanh((log Q(σ) − log p)/2)`, where `Q` is the weighted
   sum of the same-law out-of-the-money Normal premiums. Use
   the analytic lower sign and require an explicit positive
   volatility bound and coordinate tolerance, nonzero update
   budget and nonnegative absolute and relative premium
   tolerances with at least one positive. Retire automatic
   bracket expansion and propagate domain, bound and precision
   failures.
   [Accepted bounded inversion](../models-analytic/implied-volatility-solvers.md#accepted-decision-explicit-bounded-model-inversion-and-paired-premium-acceptance).

6. **Gate gross premium and time value on the actual result.**
   Reprice the returned annual volatility on the same strip,
   including endpoint results. Independently require gross and
   time-value agreement in PV per unit notional under the
   selected absolute and relative tolerances. Compute the
   candidate time value directly from the checked quote-law
   out-of-the-money premiums. Apply the native annual
   implied-minus-observed volatility tolerance separately.

7. **Use vega only when the elected objective needs it.** Evaluate
   the strip vega at the actual strike and quoted volatility
   where it is used. Remove the blanket `1e-8` floor. Use
   the same prepared strip, inversion and native fit gate in
   scalar and piecewise fitting. Preserve quote identities,
   actual work, and typed pricing, range and precision failures.
   [Accepted fit reporting](../calibration-framework/14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics).

## Independent calculation checks

- Rounding a 1.10-year quarterly request to four periods
  produces spacing `.275` instead of the regular `.25`.
- A benchmark factor ratio `401/400` with index accrual
  `90/360` gives a 1% forward. Using coupon accrual
  `60/360` as the benchmark denominator gives 1.5%.
- In the zero-κ limit with Hull–White sigma `.10`, fixing and
  benchmark start at one model year, benchmark end at year two
  and payment at year three, an index accrual of one and gross
  projection `1.05` give payment-measure mean
  `1.05 exp(−.01)`. With coupon accrual one and strike
  `−1`, the gross strike is zero, so the model caplet price
  falls below the quote intrinsic `1.05` on a common positive
  discount scale.
- For two caplets with strike `.02`, weights `.25`, fixing
  horizons 1 and 4, and forwards `.04` and `.02`, a flat
  Normal quote of `.001` gives intrinsic `.005` and time
  value about `.0001994711` per unit notional. A trial
  `.00099` passes the gross relative tolerance `.001`
  and fails the same relative tolerance on time value.

## Observed choice and rationale at the audit revision

A cap/floor quote is a flat Normal volatility applied to a strip of live term-index coupons. Market and HW model values use the same fixing, accrual and payment schedule. The source explicitly excludes the first spot-start coupon and reuses the production scheduled HW caplet kernel so fitting and valuation share contractual timing. Shared-kernel consistency is distinct from independent correctness.

## Observed input domain and schedule

All scheduled times/accruals must be finite. Each period has `0 < fixing ≤ start < end`, `payment ≥ start` and positive contractual accrual. Fixings increase strictly; accrual periods may have gaps but cannot overlap beyond `1e-10`. The final accrual end must match quote maturity within `1e-10`. Payment can precede the unadjusted accrual end after calendar adjustment. Empty live schedules are rejected. This representation excludes compounded overnight options.

Without a contractual schedule, set `n=max(round(maturity×payments_per_year),1)` and `δ=maturity/n`; retain periods `i=1,…,n−1`, with fixing/start `iδ`, end/payment `(i+1)δ` and accrual δ. The first period is excluded, and `n=1` therefore fails the no-live-caplet gate. This divides the whole maturity evenly; it does not construct a contractual final stub. Supplied schedule count must equal quote count.

## Observed ordered algorithm

1. Derive each projected simple forward from the projection factors: `F_i=(P_f(start_i)/P_f(end_i)−1)/α_i`. Nonpositive/nonfinite projection factors yield NaN rather than a floor.
2. Price the market strip as `Σ α_i D(payment_i) Bachelier(F_i,K,σ_N,fixing_i)`, selecting call for cap and put for floor. The strip's quote vega is the corresponding sum of analytical Normal vegas at the actual strike and quoted volatility.
3. Reprice each identical period with `hw1f_term_caplet_price_from_dfs_with_model`, using discount/payment factors, both projection factors, fixing/start/end/payment times, accrual, strike and cap/floor side. Projection/discount basis is deterministic; payment-measure adjustment is retained by the [HW kernel](../models-analytic/hull-white-analytic-kernels.md).
4. For reporting, invert the resulting model strip value back to one flat Normal volatility. Reject negative/nonfinite target price. Start upper vol at `0.01` and double while its market price is below target and the current bound is below `100`; the final doubling can exceed 100. Solve `[0,upper]` with Brent tolerance `1e-14` and the shared default iteration budget. This reconstructs quote error instead of reporting a first-order vega approximation.

## Observed selection and controls

Scalar fixed-κ and two-parameter fits use this preparation and inversion. Both validate aggregate market vega against `1e-8` before branching; this restriction applies even though the fixed-κ objective is unweighted price SSE. The piecewise bootstrap uses the same prices/inversion but does not apply that vega gate. Required final `fit_tolerance` is in decimal Normal-volatility units and is independent of this inversion tolerance.

## Observed limitations, failure and fallback

This is a term-index cap method, not an overnight-cap calibration. The market schedule validator permits gaps, payment before unadjusted end and arbitrary positive accruals, so contractual correctness depends on the caller. The model kernel rejects invalid prices/domains; quote inversion propagates unavailable brackets or solver failures. The finite upper-bracket search is a numerical control, not a universal range guarantee. Tiny/near-intrinsic quote inversion remains conditioned by strip vega.

## Alternatives and tradeoffs

A simple forward-rate Normal-vol proxy would be cheaper but is not the selected HW price; exact scheduled bond-ratio dynamics retain accrual and delay effects. Joint smile fitting or shifted/lognormal cap quotation would require an explicit new objective/domain. No automatic quotation conversion occurs.

## Implementation and evidence

[cap_schedule.rs](../../../../finstack-quant/calibration/src/hull_white/cap_schedule.rs) and [pricing.rs](../../../../finstack-quant/calibration/src/hull_white/pricing.rs) own schedule validation, market pricing and inversion; [cap_floor.rs](../../../../finstack-quant/calibration/src/hull_white/cap_floor.rs) calls them. [cap_schedule_tests.rs](../../../../finstack-quant/calibration/src/hull_white/cap_schedule/tests.rs) exercises ACT/360 accrual, early fixing, delayed/early payment, stubs, invalid schedules and aligned schedule sorting. Its ACT/360 case includes fixed numerical references, but the inspected file does not establish their generator provenance. Repricing also reuses the production caplet kernel.

[tests.rs](../../../../finstack-quant/calibration/src/hull_white/tests.rs) checks spot-caplet exclusion, one-period rejection and cap/floor parity. Parity is an invariant; synthetic recovery is a same-kernel round trip. All evidence was inspected, not run.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[rate-model calibration follow-ups](../FOLLOW_UPS.md#calibration-hullwhite-and-lmm).
