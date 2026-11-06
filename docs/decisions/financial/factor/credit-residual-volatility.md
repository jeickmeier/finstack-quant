# FACTOR-015: Issuer residual volatility source cascade

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the stated source scope; limitations and discrepancies below remain open.

**Validation status:** Evidence inspected or located, not executed. No numerical qualification is implied.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Assign an issuer adder volatility by an explicit override/history/peer/global/default cascade. BucketOnly names can receive their own residual-history volatility; mode alone does not force peer substitution.

## Input domain and conventions

Overrides arrive in annualized decimal-spread volatility and are converted to annualized bp volatility before selection. Historical estimates use the chosen Sample/EWMA estimator on residual changes. Peer proxies average volatilities, not variances, and do not use DTS weights.

## Algorithm stages

1. A historical residual series qualifies only with at least two valid observations and unless every absolute observation is below 1e-12 bp. Compute its annualized variance and square root.
2. Index qualifying historical volatilities under all issuer bucket path prefixes; sort each peer vector.
3. For each issuer choose caller override first, then its own historical volatility.
4. Otherwise search deepest bucket to root for any qualifying peer vector and use its arithmetic mean, tagging BucketPeerProxy with that bucket.
5. If no peer bucket qualifies, use the mean of all qualifying issuer volatilities and tag Default. If none exist anywhere, return 0 with Default.
6. Save the selected volatility/source in issuer rows and its square in idiosyncratic state.

## Selection and controls

An override of zero is authoritative. Peer lists are built from history estimates, so another issuer override does not change a peer proxy. No minimum peer count beyond one is imposed. Degenerate self-peeling residuals fall through to the cascade; a constant nonzero residual qualifies for history even if demeaned sample variance becomes zero.

## Limitations, failures, and fallbacks

Global fallback and no-data zero share the same Default source label. The 1e-12 test is on residual amplitudes, not variance. Averaging volatility is not equivalent to the square root of average variance. The cascade does not attach uncertainty or distinguish effective sample quality within historical estimates.

## Alternatives and tradeoffs

A variance-pooled proxy, robust median, weighted peers, explicit no-data error or caller-required prior would change risk estimates and disclosure. Residual covariance between issuers is not established by assigning scalar volatilities.

## Implementation and evidence

- [src/credit/calibration/statistics.rs](../../../../finstack-quant/factor/src/credit/calibration/statistics.rs) — inspected implementation.
- [src/credit/calibration/calibrator.rs](../../../../finstack-quant/factor/src/credit/calibration/calibrator.rs) — inspected implementation.

[tests/credit_calibration.rs](../../../../finstack-quant/factor/tests/credit_calibration.rs) locates override precedence, deepest/parent/global fallback, BucketOnly history and all-too-short zero cases; [src/credit/calibration/tests.rs](../../../../finstack-quant/factor/src/credit/calibration/tests.rs) locates singleton residual fallback and decimal-vol override conversion. These are branching evidence, not proof of proxy accuracy.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models). This record does not
authorize a runtime change or certify the observed method.
