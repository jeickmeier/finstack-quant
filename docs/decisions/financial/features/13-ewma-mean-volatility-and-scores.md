# FEATURES-013: Pandas-span EWMA mean, biased centered volatility and scores

**Decision status:** Draft — observed implementation; adoption pending for this scoped choice. Existing accepted decisions retain their own scope.

**Implementation status:** Observed in the exact source paths and symbols below; discrepancies and limitations are stated explicitly.

**Validation status:** Source and the named test bodies inspected, not executed. This record does not certify numerical correctness or host parity.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Scope and actual selection

`EwmaMean`, `EwmaVol` and `EwmaZscore` share one state recurrence. Required finite `span>=1` gives `alpha=2/(span+1)`, `b=1-alpha`. Span counts finite observations; it is not RiskMetrics lambda or elapsed time. The documented input is a return series, but the numeric kernel accepts any finite scalar units. No annualization, warm-up count or unbiased-variance option is exposed.

## Ordered state computation

1. The first finite observation initializes mean m=x, standard deviation s=0, mature=false. Mean emits x; volatility emits None; z-score emits 0.
2. For each subsequent finite x, calculate the new mean as `m+alpha*(x-m)` when the difference is finite; otherwise use `b*m+alpha*x` to avoid an overflowing difference.
3. Update standard deviation using `libm::hypot(sqrt(b)*s, sqrt(b)*sqrt(alpha)*x - sqrt(b)*sqrt(alpha)*m)`. Algebraically this corresponds to centered biased variance `v_new=b*v_old+b*alpha*(x-m_old)^2`, while the implemented SD form avoids directly squaring large observations. Both terms use the *old* mean/state. Mark mature=true.
4. Reject nonfinite mean or standard-deviation state immediately. Project the updated state: mean is m_new; volatility is s_new when mature; z-score is `(x-m_new)/s_new` for positive s_new, otherwise 0. A nonfinite projected z-score is caught by final output validation.
5. Missing/nonfinite input emits None without updating or decaying state. Repeated equal finite observations mature volatility to 0. Span=1 gives alpha=1, mean=current and subsequent volatility=0.

The source identifies this as pandas `adjust=False`, `ignore_na=True`, biased centered variance. The claimed correspondence is documentary intent; it was not checked against a live external library. EWMA outputs include the current observation in the state, so z-score is not a forecast error relative to the preceding state.

## Rationale and alternatives

Documented reasons for `libm::hypot` are avoiding square overflow/underflow and reducing platform-dependent native/WASM rounding. A fixed routine alone does not establish broad bit identity; [reproducibility invariants](../../../../INVARIANTS.md#21-reproducibility-tiers) explicitly limit per-target and cross-target claims. Proposed alternatives include unbiased EW variance, zero-mean RiskMetrics variance or calendar-time weighting; they are not active variants.

## Evidence

- Owning [EWMA source](../../../../finstack-quant/features/src/timeseries.rs): `ewma_alpha`, `EwmaState::{first,update,vol,zscore}`, `ewma_scan`.
- Inspected [transform tests](../../../../finstack-quant/features/tests/transforms.rs): [1,3,5], span=3 gives means [1,2,3.5], SD [None,1,sqrt(2.75)], and same-state z-score identity, at 1e-12 tolerance.
- Inspected [audit regressions](../../../../finstack-quant/features/tests/audit_regressions.rs): missing rows, initial versus mature zero volatility, span=1 and scale sweeps. `ewma_vol_is_bit_identical_across_build_targets` pins one five-point vector to bits documented as captured from WASM; a single-target fixture assertion is not a two-target run or independent financial oracle.

No native/WASM comparison was executed.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
