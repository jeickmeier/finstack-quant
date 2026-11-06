# ANALYTICS-011: Tracking error, capture, batting average, Treynor and M-squared

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted directions remain reconciliation requirements.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice and domain

Benchmark-relative statistics operate on the intersection of portfolio/benchmark active date spans. Equal observation weighting and the declared frequency A are used; no currency/benchmark hedging, missing-date interpolation or transaction cost adjustment is performed. Signed decimal simple returns are supplied by the caller. Capture ratios are ratios, not multiplied by 100.

## Algorithms and selection

Tracking error is the sample standard deviation of active returns `p−b`, annualized by sqrt(A). Information ratio is `mean(p−b)*sqrt(A)/sample_sd(p−b)` using the common signed-infinity/0/0 helper. The private kernels return zero for empty input, but the public Information Ratio wrapper requires at least two pairs and returns NaN otherwise; public Tracking Error can remain zero for empty/singleton samples.

Capture selects observation pairs by benchmark return, compounds each selected subset using compensated log growth, annualizes each to `exp(A*sum(log(1+r))/selected_count)−1`, and divides portfolio by benchmark annualized subset return. **Up capture currently selects `benchmark >= 0`; the prior agreed direction requires strictly positive benchmark returns.** Down capture selects benchmark<0. Empty input, no selected observations, or absolute compounded benchmark return below 1e−18 yields zero. Capture ratio divides up capture by down capture, but returns zero when down capture is exactly zero; NaNs otherwise propagate. Neutral benchmark observations can therefore alter the current up-capture numerator/count even though the intended selection excludes them.

Batting average is the fraction of aligned observations for which portfolio return is strictly above benchmark return; ties are not wins, empty input gives zero.

Treynor uses annual arithmetic excess return after geometric cash conversion, divided by OLS beta. Absolute beta below 1e−10 is treated as zero, then the common helper produces signed infinity or zero; invalid excess/beta gives NaN. Negative beta remains signed. The public wrapper requires two observations and uses the slope-only estimator, not the three-observation `BetaResult` uncertainty contract.

M-squared is the **absolute** risk-adjusted annual return `cash_A+(ann_mean−cash_A)*(benchmark_ann_vol/portfolio_ann_vol)`, where cash_A=A*q. It does not subtract the benchmark return. Either annual volatility below 1e−10 or any nonfinite input yields NaN. Both volatilities are computed over the same aligned span; no independent benchmark date window is substituted.

## Rationale, alternatives and limitations

Geometric subset returns distinguish capture from the ratio of arithmetic means, but annualizing a small selected sample can amplify scale dramatically or overflow; there is no clipping or minimum subset count. The 1e−18 capture and 1e−10 beta/volatility cutoffs are fixed internal choices. Arithmetic capture, full-period conditional attribution and relative rather than absolute M² are unimplemented alternatives. Strictly positive up-capture remains the agreed contract; the observed inclusive filter does not reopen that choice.

## Implementation and validation evidence

- [Owning formulas](../../../../finstack-quant/analytics/src/benchmark.rs) and [public alignment/guards](../../../../finstack-quant/analytics/src/performance/benchmark.rs).
- Located inline capture tests compare geometric subset formulas and no-up/no-down conditions; they are implementation/formula checks, not validation of neutral-observation inclusion. Treynor/M² hand cases and [audit regression](../../../../finstack-quant/analytics/tests/audit_regressions.rs) `m_squared_self_comparison_has_no_cash_basis_residual` cover identity/basis behavior. No tests ran.
- Canonical context: [Treynor](../../../REFERENCES.md#treynor1965), [M²](../../../REFERENCES.md#modigliani1997), and [tracking/active management](../../../REFERENCES.md#grinoldKahn1999ActivePortfolio). The precise capture convention and numerical cutoffs lack independent adoption evidence here.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[analytics follow-ups](../FOLLOW_UPS.md#analytics).
