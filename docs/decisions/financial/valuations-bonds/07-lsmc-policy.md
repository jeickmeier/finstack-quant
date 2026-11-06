# VALUATIONS-BONDS-07: Joint bond continuation regression, make-whole projection and sampling

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed in the linked bond production scope; discrepancies are identified explicitly.
**Validation status:** Evidence inspected or located, not executed; no accuracy certification.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and controls

Exercise-contingent joint rates-credit value uses independent training and pricing samples. Default `mc_paths=20000` means independent estimators for each required stage, with antithetic pairs by default; at least two estimators are required. Instrument ID/scenario label seeds mix with distinct training, make-whole-training and pricing domains. Native parallel and WASM serial evaluation collect in stable path order. No Sobol or Brownian-bridge sampler is selected by this bond route.

## Algorithm

1. Generate decision snapshots at origin, exercise dates, terminal and redemption dates from record 06. Eight state features are short rate, hazard, outstanding, locked term-coupon amount, accrued cash, accrued PIK, cumulative distributions and cumulative target-discounted distributions.
2. For stochastic-rate make-whole calls, first train a separate conditional-value policy for remaining promised cash. Roll future reference cash plus principal with path rate discounts multiplied by deterministic reference-curve/base-curve interval ratios and reference spread. Exclude hazard and OAS; set hazard feature to zero. Deterministic-rate make-wholes instead use deterministic reference PV. Missing training policies use contractual price only while constructing training data, not as final pricing fallback.
3. Work backward through exercise decisions. Regress affine realized continuation from later trained policy on the eight current features using standardized quadratic polynomials including interactions. Fit over all training paths, not an in-the-money-only subset. The shared fitter drops constant dimensions; when there are too few samples, a solver fails or fitted coefficients are nonfinite, it tries quadratic without interactions, linear and intercept-only. Rank deficiency alone does not force a smaller basis: truncated SVD can retain the full basis with unresolved singular directions zeroed; see the [shared regression-policy record](../models-monte-carlo/24-regression-policies.md). Nonfinite data errors; there is no look-ahead use of realized continuation as a missing-policy replacement.
4. Apply holder put first, then issuer call if the post-put estimated continuation exceeds call plus friction. Training/pricing retains the realized future payoff when policy chooses hold. Current cash is added outside the exercise decision. Incompatible put/call barriers error. Terminal redemption is an exact known hold claim, requiring no continuation regression.
5. Freeze policies and evaluate the independent pricing sample. Average each primary/antithetic pair into one estimator, accumulate `OnlineStats` and report mean, standard error, CI and physical/stage counts. A requested 95% CI half-width is a final fixed-budget acceptance gate; failure errors rather than increasing paths automatically.

Training checkpoint blocks change storage only: use one block when decisions×physical paths fit `2^20` snapshots, otherwise blocks of about square-root decision count, retaining factor and product state. Replaying blocks must reproduce full-path events; path budgets are not truncated.

## Limits, alternatives and evidence

Pricing confidence intervals condition on the fitted policy and do not measure regression, calibration, exercise-grid or model error. Make-whole regression also adds approximation error. Cubic refinement is supported by the shared fitter/tests but production selects quadratic; alternative nested conditional simulation or richer bases remain unadopted. OAS trial solves reuse seed domains and disable the final CI gate until the candidate spread is found.

Owning code: [stage orchestration](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/mod.rs), [policies](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/policy.rs), [snapshots/replay](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/replay.rs), [polynomial fitter](../../../../finstack-quant/models/src/monte_carlo/pricer/polynomial.rs). [LSMC tests](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/tests.rs) contain quadratic-surface recovery, cubic refinement, intercept fallback, independent-estimator accounting, stage reproducibility and block/full replay equality. These are manufactured-surface/structural checks, not independent option benchmarks. [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) is general MC provenance; a specific Longstaff–Schwartz citation is not present in the canonical list.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
