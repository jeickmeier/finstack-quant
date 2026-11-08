# ANALYTICS-012: Sliding-window alpha and beta with recentered compensated sums

**Decision status:** Accepted for the complete rolling alpha/beta package below (2026-11-07).

**Implementation status:** Implemented in the analytics decision reconciliation dated 2026-11-08. The accepted sections below define the current contract.

**Validation status:** Scoped implementation checks and their limits are recorded in the [analytics inventory](README.md#implementation-evidence). A passing numerical fixture establishes its stated case; it does not certify a population estimator or every supported input.

**Audit provenance:** The original source audit was performed on 2026-11-05 at `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Its historical discrepancies are superseded by the accepted contracts and this implementation. The [source ledger](coverage.json) retains source ownership and dated inspection fingerprints.

## Accepted decision: rolling alpha and beta

**Accepted:** 2026-11-07.

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

## Implementation evidence

[benchmark.rs](../../../../finstack-quant/analytics/src/benchmark.rs), [performance/benchmark.rs](../../../../finstack-quant/analytics/src/performance/benchmark.rs). The [crate integration tests](../../../../finstack-quant/analytics/tests/main.rs), inline numerical regressions and [schema contracts](../../../../finstack-quant/analytics/tests/schema_contract.rs) exercise this scope. See the [inventory validation record](README.md#implementation-evidence) for executed checks and evidence limits.
