# CALIBRATION-RATE-MODELS-007: Fixed-κ cap and floor premium fit on a selected sigma branch

**Decision status:** Accepted 2026-11-15 — complete fixed-κ scalar premium fit, branch selection and native acceptance package.

**Implementation status:** Observed. Reconciliation to the accepted package is pending.

**Validation status:** Independent delayed-payment premium and two-branch fit calculations verified. Production fitting, market references, restoration and host qualification remain pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory](README.md) and [source ledger](coverage.json).

## Accepted decision for bounded fixed-kappa cap and floor premium fitting

1. **Fit constant sigma at one supplied κ.** Admit finite nonnegative
   κ, including the Ho–Lee limit. Freeze one or more selected live
   term-index cap or floor strips, exact quote identities, authored
   order and positive native volatility tolerances. Use the accepted
   strip preparation, payment-measure pricing and quote law. Retire
   the unused seed, inherited vega admission and hardcoded parameter
   limits.
   [Accepted cap and floor strips](02-cap-schedules-and-quote-inversion.md#accepted-decision-for-checked-cap-and-floor-strips-and-implied-normal-inversion).

2. **Fit equal-weight raw premiums on a per-unit-notional basis.** Use
   the sum of squared signed premium errors
   `Σ_i (P_HW,i − P_market,i)²`. Compare candidates with its
   checked scaled Euclidean norm. This preserves the objective order
   without requiring an unscaled square. Keep signed premium errors
   and native implied-volatility errors distinct.

3. **Select one qualified sigma branch explicitly.** Require a finite
   closed interval `0 ≤ σ_lo < σ_hi`, qualified for continuous
   pricing and a unimodal objective on that branch. Admit either
   endpoint as a candidate. A delayed-payment term caplet can have
   multiple sigma fits over a broader domain. The selected branch
   defines the scope of the solve.
   [Accepted term payment-measure law](../models-analytic/hull-white-analytic-kernels.md#accepted-decision-for-checked-hullwhite-analytic-kernels-and-payment-measures) ·
   [Henrard's term caplet formula](https://quant.opengamma.io/Hull-White-One-Factor-Model-OpenGamma.pdf).

4. **Use bounded fallible golden-section minimization.** Evaluate both
   original endpoints and reuse evaluations by exact represented
   coordinate. Contract the interval under an explicit sigma
   tolerance, nonzero update budget and aggregate evaluation limit.
   Retain the best evaluated point in the final interval and the two
   original endpoints as finalists. Propagate typed pricing failures.
   Distinguish width convergence, work exhaustion and representational
   stagnation.
   [Brent's local minimization treatment](https://maths-people.anu.edu.au/~brent/pub/pub011.html).

5. **Require resolved local price response.** At an interior finalist
   require at least one quote to give consistently directed premium
   changes on distinct represented probes on both sides, with both
   moves clearing the combined operational pricing allowances.
   Use a qualified one-sided probe at an endpoint, including zero.
   This tests local identification on the selected branch.

6. **Apply the native quote and output gates before ranking.** Reprice
   each finalist on the complete prepared strips and reconstruct
   every consumed flat Normal quote through the accepted inverse.
   Require each signed implied-minus-observed error to pass its
   native tolerance. Apply the local sensitivity and complete-output
   gates. Rank passing finalists by the recomputed premium norm
   and select smaller sigma on an exact tie.
   [Accepted strip inversion](02-cap-schedules-and-quote-inversion.md#accepted-decision-for-checked-cap-and-floor-strips-and-implied-normal-inversion) ·
   [Accepted fit reporting](../calibration-framework/14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics).

7. **Publish the admitted scalar with actual fit evidence.** Preserve
   supplied κ and fitted σ, the selected interval, endpoint status,
   effective controls, actual contractions and objective evaluations,
   and signed residuals on both bases. Retire the hardcoded iteration
   count and unused relative-moneyness summaries. Failed runs may
   retain bounded diagnostics.
   [Accepted fitted-state publication](../calibration-framework/18-hull-white-market-adapter.md#accepted-decision-for-dated-hull-white-quote-preparation-and-publication).

## Independent calculation checks

- Take κ `.05`, fixing and benchmark start at model year one,
  benchmark end at year two and payment at year three. Use
  corresponding shared-curve factors `.98`, `.95` and `.92`,
  unit coupon and benchmark accruals, and strike `.98/.95−1`.
  The checked term law gives per-unit premium `.03193582083`
  at σ `.1`, `.08509159912` at σ `.5` and `.00318407696`
  at σ `2`. The `.1` premium also occurs at σ about
  `1.26489942252`, with locally resolved price sensitivity of
  opposite sign. Both reproduce the flat Normal quote about
  `.08701220811`.

## Observed choice and rationale at the audit revision

Supplying `fixed_kappa` selects a one-dimensional σ fit to cap/floor premiums. The objective is `Σ(P_HW,i(σ)−P_market,i)²`, not a signed sum and not the two-parameter vega-scaled objective. Source rationale is to avoid cancellation between opposite quote errors and keep scalar optimization simple.

## Observed domain and algorithm

The caller still validates all quotes, schedules and aggregate market vegas through [cap preparation](02-cap-schedules-and-quote-inversion.md). Fixed κ must be positive/finite and inside inclusive `[0.001,1]`. One or many quotes are supported; `initial_guess` is ignored on this branch.

1. Price each market strip under its quoted flat Normal volatility. For a trial σ, construct a constant HW model and sum squared raw price residuals. Model construction/pricing errors or nonfinite prices return positive infinity to the scalar objective.
2. Search three fixed intervals independently: `[1e-8,0.005]`, `[0.005,0.05]`, `[0.05,2]`. Before searching, require at least one finite objective among interval endpoints/midpoints. This is a finite probe screen, not a proof of finiteness everywhere.
3. Within each interval use golden-section ratio `0.6180339887498948`, contracting until width ≤`1e-12` or 200 iterations. Return the midpoint and its objective if finite/positive σ; skip a bracket with no usable result. Select the candidate with lowest finite SSE across the three.
4. Reject missing/nonpositive/nonfinite solutions and σ within relative `1e-6` of the upper global bound 2. The lower bound is not similarly rejected; κ endpoints are permitted because κ is supplied rather than estimated.
5. Reprice the basket and invert each result to flat Normal volatility. Final report success uses maximum absolute reconstructed quote error and the caller's fit tolerance, with `ScalarMinimization` method and an iteration field hard-coded to 1. That field is not the number of golden-section iterations or pricing evaluations.

## Observed units and diagnostics

Prices are per unit notional, so raw SSE gives high-price/high-vega strips more influence than equal volatility-error weighting. σ is absolute annual rate volatility. Unlike two-parameter LM's minimum `1e-5`, this branch searches down to `1e-8`; it shares the upper bound 2. A supplied initial seed does not narrow that search.

Cap moneyness metadata summarizes `|(K−F)/F|` across every live period of every quote, counting repeated periods repeatedly. It excludes nonfinite F or `|F|<1e-12`, but includes negative F; if no usable periods exist the summary is zero. This is a descriptive diagnostic, not a fit rejection rule. Source comments saying nonpositive forwards are skipped are inaccurate.

## Observed limitations, failure and alternatives

Three golden-section intervals reduce sensitivity to a single broad search but do not establish a global minimum when an interval has multiple local minima or discontinuous feasible regions. The helper returns its midpoint after the fixed iteration cap without a separate convergence status. Its Rustdoc asserts that monotone individual option prices imply a unique unimodal aggregate SSE; its implementation comments correctly avoid relying on that assertion for the whole basket. Neither claim has been numerically certified in this audit.

A common vega-scaled objective would align this branch more closely with the two-parameter method; direct implied-vol fitting or interval-global scalar optimization are other choices. They are not selected automatically. Low-vega rejection occurs before the scalar branch despite its unweighted objective; agreeing that admission policy is separate from removing signed-residual cancellation.

## Implementation and evidence

[cap_floor.rs](../../../../finstack-quant/calibration/src/hull_white/cap_floor.rs) owns objective, intervals, golden search and diagnostics. [tests.rs](../../../../finstack-quant/calibration/src/hull_white/tests.rs) contrasts the SSE fit with a signed-sum root, checks neighboring σ values and validates fixed-κ range/quote count. That is a local regression using the same HW kernel, not a global minimization proof. Fixed-κ parameter recovery and [contractual cap tests](../../../../finstack-quant/calibration/src/hull_white/cap_schedule/tests.rs) are primarily same-kernel repricing. Evidence was inspected, not executed.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[rate-model calibration follow-ups](../FOLLOW_UPS.md#calibration-hullwhite-and-lmm).
