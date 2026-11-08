# ANALYTICS-012: Sliding-window alpha and beta with recentered compensated sums

**Decision status:** Accepted for the complete rolling alpha/beta package below (2026-11-07). Implementation reconciliation and independent qualification remain open; other records retain their own scopes.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice and domain

Rolling alpha/beta are observation-window OLS point estimates over benchmark/ticker overlap, using the same Jensen cash convention as batch regression. Window zero errors publicly; oversized windows yield empty vectors; one-point/constant-benchmark windows yield NaN. Dates label window ends. There are no rolling confidence intervals, R² or adaptive bandwidths.

## Accepted decision: rolling alpha and beta

**Accepted:** 2026-11-07. **Implementation:** Partial; selected source
inspected at `827ebb75b9`. Explicit host inputs, exact alignment, scaled
reconstruction and both-mean protection, emitted-coefficient validation,
calculation errors and result restoration require reconciliation. No runtime
estimator or wire checks were executed.

This package covers the complete window, point-estimator,
availability, terminal-history, numerical-control, error and result-carrier
choices in this record. The [accepted single-factor regression contract](10-benchmark-regression.md#accepted-decision-single-factor-benchmark-regression)
and explicit cash/frequency conventions remain constraints. Record 09 owns
its separate four rolling return/risk calculations.

### Explicit windows, exact pairing and dates

- Require explicit positive integer `window = W` across Rust/Python/WASM;
  reject zero, omission and invalid host values. Remove the implicit
  63-observation window from these methods.
- Use only exact common ticker/benchmark observations inside their active
  spans and the selected active date range, with equal observation weights.
  Require equal portfolio/benchmark/date lengths and strictly increasing
  aligned return-ending dates. Do not silently use the shorter prefix, fill
  gaps, interpolate or independently extend the benchmark sample.
- Form complete trailing windows advancing one paired observation at a time.
  Do not borrow pre-active-range history, emit partial windows or pad dates.
  With `N` eligible pairs, emit `max(N - W + 1, 0)` points labelled by each
  window's final observation date. Empty overlap and oversized windows give
  three empty arrays. Do not prepend warm-up NaNs or drop unavailable points
  from completed windows.
- Validate ticker, window, required cash/frequency and supplied history before
  empty or unavailable outcomes. Irregular dates do not infer frequency or
  turn a count window into a calendar-duration window. General panel/active-
  range ownership remains with record 01.

For `N = 5` and `W = 3`, the three results use observations `1..3`, `2..4`
and `3..5`, labelled by dates 3, 4 and 5.

### Point estimates, cash and availability

For each completed window, apply the accepted equal-weight intercept OLS
and annual arithmetic Jensen alpha to that exact window:

```text
Sxx = sum((b_i - mean(b))^2)
Sxy = sum((b_i - mean(b)) * (p_i - mean(p)))
beta = Sxy / Sxx
q = expm1(log1p(R) / A)
alpha = A * ((mean(p) - q) - beta * (mean(b) - q))
```

`p` and `b` are matched decimal simple returns, `A` is explicit positive
finite periods per year, and `R` is explicit finite annual-effective decimal
cash greater than `-1`. Explicit zero cash is valid; there is no default cash
rate. Cash is converted once using the accepted convention. Beta is
dimensionless and requires no cash adjustment; the combined alpha/beta method
requires cash and frequency because alpha uses both. Alpha is an annual
arithmetic excess-return fraction, not compounded return or dated CAGR.
Preserve negative and zero beta, without a magnitude cutoff.

| Completed window | Accepted result |
| --- | --- |
| `W = 1` | Both alpha and beta undefined at every completed-window date; this remains a valid request. |
| `W >= 2`, benchmark genuinely varies | Both point estimates available, including two-pair windows; no three-pair inference gate. |
| Benchmark genuinely constant | Both undefined because beta is unidentified. |
| Portfolio constant, benchmark varies | Beta zero and alpha available. |
| Covariance genuinely zero, benchmark varies | Beta zero and alpha available, even if the portfolio also varies. |

Each rolling value has the mathematical meaning of its accepted batch point
estimator on the same window. Bitwise identity is not required; independent
expectations and justified tolerances must qualify numerical equivalence.
For benchmark `[-1%, 0%, +1%]`, portfolio `[-1%, +1%, +3%]`, `W = 2`,
`A = 12` and explicit zero cash, both completed windows have beta `2` and
alpha `12%`, labelled by the second and third dates.

Retain only alpha and beta here. There are no rolling confidence intervals,
SEs, R-squared fields or confidence input. No through-origin, weighted,
expanding, calendar-duration, shrunk, robust or serial-dependence-adjusted
regression is substituted. Adjacent windows share observations; rolling
estimates do not establish independent evidence of changing exposure or
outperformance. This package supplies point estimates without inferential
coverage claims.

### Investment history and terminal observations

Validate the complete supplied investment history under record 02 before
overlap/window selection. A valid terminal `-1` is a finite regression
observation included normally when it lies in a completed window of the
eligible paired overlap; do not force a result outside that overlap. Regression does
not compound wealth or take return logarithms. Do not insert future zeros,
permit percentage continuation after wipeout, accept interior invalid values
or wait for them to slide out before declaring later estimates valid. Generic
internal statistical arithmetic retains record 10's finite numeric domain;
panel investment eligibility is a separate boundary.

### Recentered computation and numerical failures

- Retain recentered Neumaier-compensated return, benchmark, cross-product and
  benchmark-square sums for incremental OLS. Reconstruct the current raw
  window after 64 slide updates, or earlier for nonfinite state or relative
  cancellation in benchmark dispersion, covariance or reconstructed means.
  Retain `sqrt(epsilon)` relative-scale triggers as internal controls, without
  public tuning. Mean recovery must protect both portfolio and benchmark
  means; adding a shift back can itself cancel.
- These controls trigger same-estimator reconstruction; they are not
  variance, covariance or beta floors, smoothing, or proven accuracy bounds.
  Ordinary updates take constant work, but frequent reconstruction can cost
  window-length work per observation. Independent accuracy/performance
  qualification can revise the controls while preserving the financial
  estimator and window semantics.
- Validate initial, rebuilt and every emitted state. On suspicious incremental
  arithmetic, use adequately scaled centered reconstruction of that same raw
  window before classifying its result. Only a genuinely constant benchmark
  makes a sufficiently observed window unidentified. Do not convert negative,
  nonfinite or underflowed computed dispersion into ordinary unavailable
  estimates when distinct benchmark values remain.
- Preserve genuine zero covariance as beta zero, including an orthogonal
  nonconstant response. A cancellation trigger is not permission to suppress
  a small nonzero slope. Avoidable overflow/underflow in unscaled intermediate
  products must not discard representable requested alpha/beta. For example,
  benchmark `[0, 1e-200, 2e-200]` and portfolio twice that benchmark identify
  beta `2` despite squared-product underflow in naive arithmetic.
- If adequate same-estimator reconstruction cannot produce valid representable
  requested coefficients, fail the affected rolling metric call with an
  explicit calculation error. Nonfinite final alpha/beta is not a legitimate
  infinite-risk ratio or an unavailable-history sentinel. Invalid requests
  are validation errors; persistent numerical failures do not become NaN
  windows, zero coefficients, partial series, clipping or estimator fallback.

Expected one-pair/constant-benchmark unavailability remains aligned in the
output. A calculation failure in any otherwise identifiable window fails this
rolling call, rather than returning some coefficients as though the operation
succeeded. This leaves broader summary failure policy separately owned.

### Result carrier, hosts and restoration

Retain `RollingGreeks` with parallel `dates`, `alphas` and `betas`. Every
completed window contributes one point to all three arrays. A successful point
contains two finite coefficients or two undefined values, never an infinity
or only one unavailable coefficient.

Preserve undefined values as NaN in Rust and native Python NumPy/WASM typed
arrays. Reuse the existing core nonfinite-sequence wire mechanism, encoding
undefined values as `"nan"` in JSON, and align the schema with the actual
serializer. Do not replace them with zero/null or omit dates. Restoration must
validate equal array lengths, strictly increasing dates, matching alpha/beta
unavailable positions and finite-or-undefined coefficient domains. Reject
malformed shapes, unknown fields/sentinels, nulls and semantic infinities;
do not truncate, reorder, repair or infer discarded window history. The
carrier does not store the source sample/window and cannot independently
verify coefficient values or source-derived output counts on restoration.
No new status carrier or compatibility path is needed.

The [NIST OLS definition](https://www.itl.nist.gov/div898/handbook/pmd/section4/pmd431.htm)
supports the underlying straight-line estimator. The [statsmodels rolling-regression documentation](https://www.statsmodels.org/stable/examples/notebooks/generated/rolling_ls.html)
illustrates observation-count windows and end-date alignment. Its default
missing-value handling, output padding and inference surface are not adopted
by this package. These references do not qualify the internal 64-step controls,
error budget or wire restoration.

## Algorithm and controls

For each window of w points, recenter portfolio and benchmark observations around shift_r/shift_b, initially the window's first values. Maintain Neumaier sums Sr, Sb, Srb and Sb² of shifted returns and products. Compute denominator `D=w*Sb²−Sb*Sb` and slope `(w*Srb−Sb*Sr)/D`; reconstruct unshifted means and annualized Jensen alpha. A window with w<2 or nonpositive/nonfinite D yields NaN for both alpha and beta.

Sliding updates subtract outgoing shifted values/products and add incoming ones. The implementation tracks maximum variance, covariance and return-sum scales since the last rebuild. It fully recenters/recomputes when denominator, covariance numerator, shifted return sum or recovered mean suffers the configured sqrt(machine epsilon) relative cancellation test; when any sum is nonfinite; or after 64 updates. This trigger rebuilds an estimate; it does not classify every tiny positive benchmark variance as zero. Nonfinite input in private callers can affect its windows and recover once it exits; public Performance normally prevents such inputs.

The 64-update interval is justified in source as a drift/performance tradeoff for four cross-term sums and compared with the 1,024-step scalar rolling kernels. The decision is internal and not user-tunable. Typical updates are constant work, but recurring cancellation triggers can force full-window work for many points.

The accepted package requires removing private shortest-prefix truncation, distinguishing
true constant-benchmark unavailability from numerical denominator collapse,
protecting both reconstructed means and validating final coefficients. Current
nonfinite-input recovery is not an accepted missing-data policy. Source
comments claiming shift reconstruction cannot cancel need correction.

## Conventions, wire limitations and alternatives

Alpha is annual arithmetic excess return, beta dimensionless, cash an annual effective rate converted to per-observation q. Frequency and cash do not determine the rolling dates or ensure equal elapsed intervals. Full-window independent recomputation is a numerical alternative; exponentially weighted regression or robust covariance estimates are different unimplemented methods.

`RollingGreeks` derives serde on plain `Vec<f64>` alphas/betas, unlike `DatedSeries`' explicit nonfinite sequence adapter and batch regression's explicit nonfinite fields. The kernel intentionally produces NaN at undefined windows. Therefore lossless roundtrip/host representation of those values is not established by this output type; the adapter discrepancy is statically confirmed, while a complete wire failure was not executed. This wire boundary remains unqualified under [ADR 0009](../conventions/0009-missing-and-invalid-data.md); that does not permit silent substitution of zero.

Native Python NumPy getters and WASM typed-array output already preserve NaN;
the identified serialization/restoration discrepancy does not establish a
native-array failure. Python/WASM still default the window to 63 and cash to
zero, contrary to the explicit-input contract accepted here and the already
accepted cash convention.

## Implementation and validation evidence

- [Rolling implementation and output type](../../../../finstack-quant/analytics/src/benchmark.rs) and [public overlap/window guard](../../../../finstack-quant/analytics/src/performance/benchmark.rs).
- Host defaults and native output conversion are in the [Python Performance bindings](../../../../finstack-quant-py/src/bindings/analytics/performance.rs) and [WASM Performance bindings](../../../../finstack-quant-wasm/src/api/analytics/performance.rs); the [existing core wire adapters](../../../../finstack-quant/core/src/wire/non_finite.rs) own sentinel spelling.
- Located inline `rolling_greeks_stays_close_to_exact_recomputation_on_long_series`, constant-window and NaN-exit tests compare to fresh OLS windows. [Audit regressions](../../../../finstack-quant/analytics/tests/audit_regressions.rs) cover outlier/constant spans and tiny nonzero benchmark variance. They are numerical consistency cases, not independent regression-oracle validation or wire roundtrips for RollingGreeks.
- [Accuracy/stability reference](../../../REFERENCES.md#higham-accuracy-and-stability) is context, not provenance for the exact 64-step/relative-trigger settings.

No tests, serialization experiments or benchmarks were run.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[rolling alpha/beta follow-up](../FOLLOW_UPS.md#analytics-rolling-regression).
