# VALUATIONS-OTHER-FIXED-INCOME-11: Revolver three-factor paths, survival and default integration

- **Decision status:** Draft — observed implementation; adoption and rationale require agreement.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below.
- **Validation status:** Source and named evidence inspected or located; tests and numerical experiments were not executed. Evidence limits are stated below.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Choice and rationale

[Unified dispatch](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/pricing/unified.rs) automatically chooses deterministic event valuation or Monte Carlo from `draw_repay_spec`, including through the registered Discounting key. Explicit `MonteCarloThreeFactor` requires stochastic inputs. Positive loan-equivalent/LC exposure requires a default model. The model combines utilization, rates and credit-spread factors, then values each schedule with conditional survival and a separate recovery/default-funding leg. Source rationale motivates adverse selection; the default parameter values remain adoption choices.

## Path algorithm and controls

[Path generation](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/pricing/path_generator.rs) starts at `max(issue,as_of)` with current utilization. The process clock is Actual/365 Fixed, distinct from coupon day count. Weekly-or-finer internal steps evolve the shared credit-line process; only contractual observation states are retained. Philox uses a separate deterministic stream per estimator. Optional antithetic sampling negates all normals and keeps pairs adjacent. Embedded [run defaults](../../../../finstack-quant/valuations/core/data/defaults/product_mc_defaults.v1.json) are 10,000 independent estimators, antithetic false; model overrides select count/seed/antithetic.

Utilization is bounded mean-reverting diffusion with spread sensitivity. At `abs(sigma)<1e-8`, the adapter restores the old utilization after every simulation step, freezing drift and spread response as well as volatility. At observations, cap utilization to `1−LC/commitment`. Fixed coupons have fixed rate state. Floating rates default to a deterministic forward curve. Positive-sigma Hull–White fits drift and initial short rate to the discount curve; zero-sigma mode uses caller initial/theta values. Positive-sigma overnight RFR is rejected because daily fixing paths are absent.

Credit choices are explicit CIR, Constant or MarketAnchored. CIR initial/theta/kappa are floored at `1e-8`; Feller violations warn and continue. Constant spread is actually mapped to CIR `(kappa=.01, theta=spread, sigma=.001, initial=spread)`, so positive constants still diffuse. MarketAnchored uses current hazard and conditional average hazard over remaining/capped tenor; with `LGD=max(1−R,1e-6)`, map `s0=LGD×lambda0`, `sbar=LGD×average_lambda`, and choose CIR theta to match the mean-average spread. CIR sigma is fractional spread volatility times square-root reference spread. Without explicit config, hazard-bearing facilities use kappa `.1`, fractional vol `.3` and utilization-credit correlation `.3`; without hazard, Constant(0) and no correlation. Full correlation matrix overrides the utilization-credit shorthand.

## Pricing algorithm

[Single-path pricing](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/pricing/path_pricing.rs) discounts contractual cashflows including signed future funding by `DF(as_of,t) × S(t)/S(as_of)`. Deterministic paths use the hazard curve or unit survival. Stochastic paths integrate `lambda=s/LGD` trapezoidally between retained observations, linearly interpolate cumulative hazard to cashflow dates, and set `S=exp(−H)`. Cumulative hazard is held flat beyond the final observation.

Positive-sigma rates select bank-account discounting: integrate piecewise-linear retained short rates exactly by trapezoids and exponentiate minus the integral; extend short rate flat beyond the grid. Otherwise use the static discount curve. Weekly simulation refinement therefore does not retain weekly rate/hazard integration accuracy.

Default/recovery integration merges monthly dates, cashflow observations, draw/repay and commitment/LC events, ending at **legal maturity**. For each conditional default-probability increment, use average endpoint DFs and average pre-event drawn exposure. Add
`PD × DF_average × [R × drawn_average + (LEQ × undrawn_average + LC_default_draw) × (R−1)]`.
The second term funds contingent draws at par and recovers only fraction R. Upfront fee is separately discounted without survival only if issue is strictly after `as_of`.

[Aggregation](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/pricing/stochastic.rs) averages path PVs. Complete antithetic pairs are one independent observation; sample variance is Bessel-corrected and the 95% mean interval uses Student-t. At least two independent estimators are required. Expected cashflow export averages amounts by date/reset/kind, with absent flows contributing zero; displayed rates average only present rate observations.

## Limitations and alternatives

This is conditional-intensity expected cashflow valuation, not sampled stopping at realized default times. MarketAnchored matches an average spread target, not the entire survival term structure. Recovery stops at legal maturity even if adjusted terminal cash payment is later, while path hazard freezes after final observation. Correlation-driven draw cost is modeled but not a borrower optimal-control solution. Deterministic hazard valuation is supported; exact integrated-rate paths, full survival calibration and endogenous optimal draw policy are unimplemented alternatives.

## Evidence

Colocated path tests cover anchor mapping and curve-fit selection; `path_estimate_uses_sample_stderr_and_student_t_interval` checks a hand-computed two-observation interval. [MC tests](../../../../finstack-quant/valuations/fixed-income/tests/revolving_credit/mc.rs), [parity tests](../../../../finstack-quant/valuations/fixed-income/tests/revolving_credit/revolving_credit_parity.rs) and [default-risk tests](../../../../finstack-quant/valuations/fixed-income/tests/default_risk.rs) are located evidence, not rerun or universal model validation. [Philox](../../../REFERENCES.md#salmon-2011-philox), [Monte Carlo](../../../REFERENCES.md#glasserman-2004-monte-carlo) and [Hull–White](../../../REFERENCES.md#hull-white-1990-pricing-interest-rate-derivatives) supply context.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
