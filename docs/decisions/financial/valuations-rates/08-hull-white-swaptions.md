# Hull–White swaption exercise, tree reuse and regression policy

- ID: VALUATIONS-RATES-08-HULL-WHITE-SWAPTIONS
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed in the owning paths below.
- Validation status: Evidence located/inspected, not executed; no universal accuracy qualification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and rationale

The default Bermudan model is a calibrated one-factor Hull–White trinomial tree. European HullWhite1F uses the same cashflow exercise kernel at one date; Bermudan MonteCarloHullWhite1F is an explicit LSMC alternative. Both are **single-curve** implementations: projection and discount IDs must agree. The detailed calibrated tree geometry/bond formulas are owned by [models-numerical](../models-numerical/hull-white-event-grid-and-affine-bonds.md); rate stepping and affine model kernels are owned by [models-analytic](../models-analytic/hull-white-analytic-kernels.md). This record states product cashflows, exercise and regression policy, not approval of their approximation error.

## Exercise cashflows and conventions

1. Resolve fixed/float leg sentinels using IRS helpers. Accept simple float coupons or compounded coupons with exactly zero lookback, observation shift or cutoff; reject other observation modes. Keep periods whose accrual end and payment both follow exercise. Clip each remaining accrual start and float reset to exercise, recomputing accrual on that truncated interval.
2. At model time t calculate `A=sum(alpha_i*P(t,pay_i))`. Return zero swap rate, annuity and payoff if A is nonpositive. Float PV per notional is `sum((P(t,start)/P(t,end)*delayed_coupon_adjustment - 1 + spread*alpha)*P(t,pay))`. The shared Hull–White adjustment distinguishes simple versus compounded delayed coupons. `S=FloatPV/A`.
3. Physical/CollateralizedCashPrice/IsdaParPar exercise is `N*A*max(sign*(S-K),0)`. ParYield instead uses regular-frequency par annuity from the remaining period count, times a bond to max(original start, exercise); ZeroCoupon uses summed remaining fixed accrual times a bond to the cashflow horizon. European supports these choices; Bermudan always uses CollateralizedCashPrice for cash settlement.
4. Model times are ACT/365F from valuation. Bermudan exercise dates must be strictly increasing in `[underlying_start,underlying_end)`, and lockout removes dates **on or before** lockout. The convenience schedule uses no stubs, weekends-only ModifiedFollowing payments, with original first exercise retained. Future exercise lists exclude today; the public pricer subsequently takes max(continuation, deterministic exercise today) when today is effective. No surviving exercise or matured underlying returns zero before that today adjustment.

## Tree algorithm and prepared-model controls

Tree steps default 100; an instrument tree-step override wins. Resolve a complete scalar HW parameter pair through the [shared swaption-family resolver](../valuations-framework/model-resolution.md), which orders instrument overrides, a supplied prepared-model pair and prefitted market scalars; sigma schedules are rejected by that resolver. Build the tree to the last contractual payment with exercise times mandatory. Start terminal option values at zero except a terminal exercise node, then take `max(discounted continuation, exercise)` at each exercise step. European uses one mandatory expiry and its terminal intrinsic shortcut; past expiry has zero value. The tree valuator rejects NonCoTerminal.

A prepared tree stores valuation date and a serialized discount-curve snapshot. Reuse rejects changed date/curve, insufficient horizon, different resolved parameters, or explicit step-count mismatch; missing mandatory exercise nodes fail time lookup. Metric setup freezes resolved parameters into the instrument. Bumped repricing discards the cached tree, recalibrates to the bumped curve, and retains the cached grid as mandatory times, shifted for date rolls. Thus cached-tree reuse is not silent reuse of an unbumped calibration.

Exercise probabilities first reconstruct the tree continuation policy, then propagate undiscounted surviving transition mass forward. A node exercises on `exercise >= continuation` with positive exercise, removes that mass and records probability. Mass below `1e-15` is skipped. ExpectedExerciseTime is `sum(t*p)/sum(p)`, conditional on exercise; it returns zero below total probability `1e-12`. The metric is available only under tree policy. The separate exercise-today PV adjustment is not incorporated into this future-date probability calculation.

## Hull–White LSMC algorithm and controls

1. Defaults are 50,000 independent base streams, seed42, antithetic=true, minimum2 steps/event gap, polynomial degree3, in-sample regression. An instrument mc_paths override changes base streams; seed/basis/OOS remain pricer config. Shared event grid also enforces its monthly time-density floor.
2. Rebase the discount curve to valuation, calibrate theta on the event grid and estimate the initial instantaneous forward. Draw Philox substreams per base stream; antithetic partners negate every normal. ExactHullWhite1F steps short rates, while money-market discount factors use **trapezoidal** rate integrals. Conditional bonds use `exp(lnA-B*r)` and require finite positive output.
3. At final exercise retain positive exercise payoff. Work backward. On ITM training paths, regress the future chosen payoff multiplied by `B(now)/B(future)` against the supplied basis evaluated on swap rate and auxiliary annuity. The product adapter supplies PolynomialBasis `{1,S,...,S^degree}`, which **ignores the auxiliary annuity**. Fit only when ITM training count is greater than basis count plus10; otherwise skip exercise at that date. Shared least-squares failures propagate.
4. Apply the fitted rule to every path: exercise only if immediate value is positive and exceeds fitted continuation. Divide final chosen payoff by its pathwise bank account. Nonpositive bank factors cause a zero contribution in the local fallback. Pair antithetics before computing errors; optional split mode trains on even base streams and prices odd ones. Shared estimate requires at least two pricing estimators. Report standard error, 95% interval, pricing count and seed.

## Risk and alternatives

Bermudan Delta bumps the discount curve by configured ±bp and divides by twice the decimal bump. Gamma uses ±10bp and the second difference divided by squared decimal bump. HwSigmaVega bumps resolved sigma by the shared HW increment, central when sigma exceeds the bump and forward otherwise, scaled to absolute0.01 sigma. It is model-parameter risk, not swaption-surface vega. Same seeds give common random numbers for MC repricing. ExpectedExerciseTime errors for LSMC.

Tree refinement versus LSMC grid/regression/sample refinement are distinct accuracy controls. Out-of-sample LSMC is supported but off by default; exact integrated-rate sampling is not used. Standard errors exclude model, regression and discretization bias. Low-ITM skipped exercise is a material policy. Although the tree explicitly rejects NonCoTerminal, the HW LSMC path does not inspect this enum and values one common terminal swap maturity. The expected-time metric excludes the separate exercise-today value. No independent accuracy qualification for clipped mid-coupon exercise is established.

## Evidence

- [Bermudan orchestration and cache](../../../../finstack-quant/valuations/rates/src/swaption/bermudan.rs), [European HW adapter](../../../../finstack-quant/valuations/rates/src/swaption/hw_pricer.rs), [contract validation](../../../../finstack-quant/valuations/rates/src/swaption/types/bermudan.rs), [exercise schedule](../../../../finstack-quant/valuations/rates/src/swaption/types/definitions.rs).
- [Cashflow exercise](../../../../finstack-quant/valuations/rates/src/swaption/pricing/hw_cashflows.rs), [tree policy/probabilities](../../../../finstack-quant/valuations/rates/src/swaption/pricing/tree_valuator.rs), [LSMC kernel](../../../../finstack-quant/valuations/rates/src/swaption/pricing/monte_carlo_lsmc.rs), [bond helper](../../../../finstack-quant/valuations/rates/src/swaption/pricing/swap_rate_utils.rs), [risk](../../../../finstack-quant/valuations/rates/src/swaption/metrics/bermudan_greeks.rs).
- [Bermudan tests](../../../../finstack-quant/valuations/rates/tests/swaption/bermudan_pricing.rs) locate tree/MC comparison, monotonicity, same-day exercise and parameter rejection evidence. The grid/seed acceptance case is slow/ignored in ordinary runs. [Numeraire tests](../../../../finstack-quant/valuations/rates/src/swaption/pricing/tests/numeraire.rs) implement pathwise reference estimators; they share model primitives and do not constitute independent market validation. No tests executed.
- [Hull–White](../../../REFERENCES.md#hull-white-1990-pricing-interest-rate-derivatives), [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) and [shared regression policies](../models-monte-carlo/24-regression-policies.md) provide provenance/context. A canonical Longstaff–Schwartz bibliography entry was not located.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
