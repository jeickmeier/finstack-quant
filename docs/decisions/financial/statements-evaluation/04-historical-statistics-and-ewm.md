# STATEMENTS-EVALUATION-004 — Rolling, cumulative, ranked and exponentially weighted statistics

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed for the stated source scope**, with discrepancies identified below. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: [slice audit](README.md).

## Choice, domain and collection

Historical statistics operate on chronological values at or before the current context. Simple column collection includes present slots only; arbitrary expressions are evaluated over available historical/current periods. Current values overlay historical same-period entries. Thus an absent column slot and a present NaN are different observations. This protects lagged calculations from future data, but it does not build a uniform calendar or impute missing entries.

## Algorithms and selection

- Rolling functions take the last `window` available observations, then retain finite values. Window must be positive; default `min_periods=window`, explicit zero is promoted to one, and a minimum above the window errors. Below the finite-count threshold return NaN. Sum/mean, sample variance/std (`n-1` denominator), median, min/max and count delegate to shared core statistics; singleton sample variance is undefined. No calendar-duration weighting is applied.
- Full-history std/var/median and cumulative sum/product/min/max use finite observations. Cumulative product returns NaN and warns on overflow. Their handlers require at least one argument but use **only argument zero**; surplus expressions are neither evaluated nor rejected. This differs from fixed-arity handlers.
- Rank accepts a column or capital reference, requires finite current value and ranks against all finite history including current. Ascending default is `1 + count(x < current-1e-10)`; descending uses `x > current+1e-10`. A supplied direction must be finite: zero selects descending and nonzero ascending. Ties use competition rank. Quantile has the same restricted source argument, validates `q in [0,1]`, and uses R-7 interpolation: `h=(n-1)q`, linear interpolation between adjacent order statistics.
- EWM accepts `alpha in (0,1]`. Trim trailing nonfinite values; initialize at first finite observation with mean `m=x`, biased variance `v=0`, squared-weight sum `s=1`, old weight `w=1`. For each subsequent slot first set `w=(1-alpha)w`; a nonfinite observation skips the update but retains that decay. For a finite value set `a=alpha/(w+alpha)`, `b=w/(w+alpha)`, `d=x-m`; update `m=m+a*d`, `v=b*v+b*a*d^2`, `s=b^2*s+a^2`, then reset `w=1`.
- EWM mean returns `m`. Variance/std default to unbiased output `v/(1-s)`; insufficient finite observations or a near-zero correction denominator return NaN. Explicit numeric zero requests biased `v`; any other numeric value, including NaN at this local test, selects correction. Std takes the square root. The flag does not switch between two weighting recurrences.

## Conventions, limitations and alternatives

Alpha, window and minimum count are per observed model entry, not elapsed days. A present NaN decays EWM weight, while a missing column slot omitted by collection does not. EWM is recursive normalized weighting (`adjust=False` in the cited test comments), with a separate variance bias correction. Rolling and cumulative functions drop nonfinite data without the n-ary sum's skip warning. Do not generalize one missing-data rule across all statistics.

Calendar-complete windows, elapsed-time EWM, population variance and strict missing-value propagation are distinct possible choices; only described parameters are supported.

## Evidence

Owning sources: [aggregates](../../../../finstack-quant/statements/src/evaluator/formula_aggregates.rs), [EWM](../../../../finstack-quant/statements/src/evaluator/formula_ewm.rs), [dispatch](../../../../finstack-quant/statements/src/evaluator/formula_dispatch.rs), [collection](../../../../finstack-quant/statements/src/evaluator/formula_helpers.rs), [core estimators](../../../../finstack-quant/core/src/math/stats.rs). Inspected inline [formula tests](../../../../finstack-quant/statements/src/evaluator/formula.rs) include independent hand-weight checks: `[1,2,3]` at alpha 0.5 has unbiased variance 1.1; `[1,NaN,3]` has mean 7/3; missing-gap variance is 10/11. They reference pandas in comments but this audit did not execute pandas or independently verify a pinned vendor version. [Market-statistics tests](../../../../finstack-quant/statements/tests/market_standards_tests.rs) locate further numerical fixtures. No tests were executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-evaluation-forecasts-and-reporting).
