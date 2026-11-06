# CALIBRATION-RATE-MODELS-007: Fixed-κ cap/floor price least squares by segmented golden-section search

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope linked below; source/documentation discrepancies are stated explicitly.

**Validation status:** Evidence inspected/located, not executed. Internal round trips, analytic special cases and external provenance have distinct evidence roles.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory](README.md) and [source ledger](coverage.json).

## Choice and rationale

Supplying `fixed_kappa` selects a one-dimensional σ fit to cap/floor premiums. The objective is `Σ(P_HW,i(σ)−P_market,i)²`, not a signed sum and not the two-parameter vega-scaled objective. Source rationale is to avoid cancellation between opposite quote errors and keep scalar optimization simple.

## Domain and algorithm

The caller still validates all quotes, schedules and aggregate market vegas through [cap preparation](02-cap-schedules-and-quote-inversion.md). Fixed κ must be positive/finite and inside inclusive `[0.001,1]`. One or many quotes are supported; `initial_guess` is ignored on this branch.

1. Price each market strip under its quoted flat Normal volatility. For a trial σ, construct a constant HW model and sum squared raw price residuals. Model construction/pricing errors or nonfinite prices return positive infinity to the scalar objective.
2. Search three fixed intervals independently: `[1e-8,0.005]`, `[0.005,0.05]`, `[0.05,2]`. Before searching, require at least one finite objective among interval endpoints/midpoints. This is a finite probe screen, not a proof of finiteness everywhere.
3. Within each interval use golden-section ratio `0.6180339887498948`, contracting until width ≤`1e-12` or 200 iterations. Return the midpoint and its objective if finite/positive σ; skip a bracket with no usable result. Select the candidate with lowest finite SSE across the three.
4. Reject missing/nonpositive/nonfinite solutions and σ within relative `1e-6` of the upper global bound 2. The lower bound is not similarly rejected; κ endpoints are permitted because κ is supplied rather than estimated.
5. Reprice the basket and invert each result to flat Normal volatility. Final report success uses maximum absolute reconstructed quote error and the caller's fit tolerance, with `ScalarMinimization` method and an iteration field hard-coded to 1. That field is not the number of golden-section iterations or pricing evaluations.

## Units and diagnostics

Prices are per unit notional, so raw SSE gives high-price/high-vega strips more influence than equal volatility-error weighting. σ is absolute annual rate volatility. Unlike two-parameter LM's minimum `1e-5`, this branch searches down to `1e-8`; it shares the upper bound 2. A supplied initial seed does not narrow that search.

Cap moneyness metadata summarizes `|(K−F)/F|` across every live period of every quote, counting repeated periods repeatedly. It excludes nonfinite F or `|F|<1e-12`, but includes negative F; if no usable periods exist the summary is zero. This is a descriptive diagnostic, not a fit rejection rule. Source comments saying nonpositive forwards are skipped are inaccurate.

## Limitations, failure and alternatives

Three golden-section intervals reduce sensitivity to a single broad search but do not establish a global minimum when an interval has multiple local minima or discontinuous feasible regions. The helper returns its midpoint after the fixed iteration cap without a separate convergence status. Its Rustdoc asserts that monotone individual option prices imply a unique unimodal aggregate SSE; its implementation comments correctly avoid relying on that assertion for the whole basket. Neither claim has been numerically certified in this audit.

A common vega-scaled objective would align this branch more closely with the two-parameter method; direct implied-vol fitting or interval-global scalar optimization are other choices. They are not selected automatically. Low-vega rejection occurs before the scalar branch despite its unweighted objective; agreeing that admission policy is separate from removing signed-residual cancellation.

## Implementation and evidence

[cap_floor.rs](../../../../finstack-quant/calibration/src/hull_white/cap_floor.rs) owns objective, intervals, golden search and diagnostics. [tests.rs](../../../../finstack-quant/calibration/src/hull_white/tests.rs) contrasts the SSE fit with a signed-sum root, checks neighboring σ values and validates fixed-κ range/quote count. That is a local regression using the same HW kernel, not a global minimization proof. Fixed-κ parameter recovery and [contractual cap tests](../../../../finstack-quant/calibration/src/hull_white/cap_schedule/tests.rs) are primarily same-kernel repricing. Evidence was inspected, not executed.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[rate-model calibration follow-ups](../FOLLOW_UPS.md#calibration-hullwhite-and-lmm).
