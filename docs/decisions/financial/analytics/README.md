# Analytics algorithm methodology inventory

The analytics crate implements the **15 accepted methodology records** below.
Their decisions were accepted on 2026-11-06–2026-11-07 and reconciled with the
Rust, Python, WASM, schema and UI contracts on 2026-11-08. Accepted sections
state the current contract; the former implementation discrepancies have
been removed from the follow-up registers.

The original 2026-11-05 audit inspected **21 Rust source files** at
`7e82fd03ad98346727769e88dd5ac5d84b56e9fc`: 18 algorithm files and three support
files. The current tree has **23 files**, including the separate correlation and
exact-statistics modules added in the implementation. The [source ledger](coverage.json) retains source ownership and dated
inspection fingerprints. Source enumeration and hashes establish the
identified source boundary, rather than numerical or statistical certification.

## Governing and overview ADRs

| Record | Accepted scope |
| --- | --- |
| [0008](../conventions/0008-financial-defaults-and-cagr.md) | Dated CAGR defaults to Actual/365 Fixed. |
| [0009](../conventions/0009-missing-and-invalid-data.md) | Invalid inputs, permitted edge gaps and undefined estimates remain distinct. |
| [0033](0033-value-at-risk-method-selection.md#accepted-decision-fixed-historical-tail-selection) | Canonical and fixed-summary tails use historical VaR/ES; Gaussian/CF are explicit alternatives. |

## Accepted scopes in detailed records

All 15 records are accepted for their complete scoped methodology. Each
retains its acceptance dates, formulas, units, availability rules, numerical
failure contracts and evidence limits.

| Record | Implemented scope |
| --- | --- |
| [01: Panels, windows, wire and summary](01-panels-windows-and-wire.md#accepted-decision-panels-windows-wire-and-summary) | Actual valuation grid, strict panel admission, optional frequency/benchmark context without inferred defaults, explicit used inputs, atomic windows, validated wire state and fixed-summary provenance. |
| [02: Wealth and annualization](02-wealth-and-annualization.md#accepted-decision-wealth-means-cagr-relative-wealth-and-cash-series) | Exact terminal wipeouts, compensated wealth/means, ACT/365F dated CAGR with full day-count context, relative positive-benchmark wealth, paired dates and aligned annual-effective cash. |
| [03: Periods, lookbacks and win-loss](03-periods-lookbacks-and-win-loss.md#accepted-decision-calendar-grouping-lookbacks-and-win-loss-statistics) | Explicit calendar/fiscal grouping, occupied partial buckets, observed streaks, actual lookback coverage, conditional means, profit-factor/CPC/loss-normalized Kelly and validated outputs. |
| [04: Return-risk ratios](04-return-risk-ratios.md#accepted-decision-standalone-scalar-domains-and-numerical-failures) | Sample SD, arithmetic-excess Sharpe, all-observation downside/Sortino, explicit MAR/Omega threshold, arithmetic gain-to-pain, native-period modified Sharpe and metric-specific zero-risk outcomes. |
| [05: Drawdowns](05-drawdown-paths-and-episodes.md#accepted-decision-drawdown-paths-and-episodes) | Rebased geometric paths, strict recovery, actual dates/calendar durations, nullable unavailable duration, deterministic episode selection and benchmark path differences. |
| [06: Drawdown rewards and CDaR](06-drawdown-reward-and-cdar.md#accepted-decision-drawdown-reward-and-cdar) | Calmar, cash-adjusted Martin, recovery, worst-episode Sterling, observation-normalized all-episode modified Burke, Pain and empirical percentage CDaR. |
| [07: Historical tails](07-historical-tail-risk.md#accepted-decision-historical-tail-numerical-evaluation-and-failures) | Signed R-7 VaR, exact fractional empirical ES, nonempty-sample availability, magnitude tail ratio and scaled requested-output arithmetic. |
| [08: Moments, Gaussian and CF](08-moments-gaussian-and-cornish-fisher.md#accepted-decision-model-support-numerical-evaluation-and-exact-horizons) | Type 2 shape estimators, classical CF plug-in, global analytic ordering, exact whole-period additive horizons, model support, scaled moments and explicit invalid-CF errors. |
| [09: Rolling return-risk](09-rolling-return-risk.md#accepted-decision-rolling-return-risk-calculations) | Explicit windows, compact right-labelled results, scalar-estimator consistency, terminal histories, stable rebuild/error rules and validated result carriers. |
| [10: Benchmark regression](10-benchmark-regression.md#accepted-decision-single-factor-benchmark-regression) | Exact-alignment intercept OLS, two-pair point estimates, Jensen alpha, field-specific fit/uncertainty and fixed labelled 95% Student-t intervals. |
| [11: Benchmark-relative metrics](11-benchmark-relative-metrics.md#accepted-decision-benchmark-relative-metrics) | Paired TE/IR, strict batting/capture populations, signed geometric capture, Treynor, absolute M-squared and explicit availability/errors. |
| [12: Rolling regression](12-rolling-regression.md#accepted-decision-rolling-alpha-and-beta) | Explicit-window paired OLS/Jensen estimates, dated availability, terminal histories, numerical rebuild/errors and strict lossless result restoration. |
| [13: Multifactor regression](13-multifactor-regression.md#accepted-decision-multifactor-regression) | Canonical raw intercept OLS, Total-only panel adapter, explicit return interpretation, prealigned factor inputs, residual risk and field-wise availability. |
| [14: SVD and constrained regression](14-svd-and-constrained-regression.md#accepted-decision-normalized-svd-and-constrained-regression) | Roundoff-based full-rank normalized SVD, bounded/scaled solving, unweighted holdings identity, weighted-gross closure and final optimality/error checks. |
| [15: Correlation](15-correlation-alignment-and-repair.md#accepted-decision-common-sample-pearson-correlation) | Common-sample Pearson only, whole-matrix availability, singular PSD acceptance, bounded endpoint normalization and validated compact sample provenance without repair. |

## Implementation evidence

The [analytics source](../../../../finstack-quant/analytics/src),
[crate integration tests](../../../../finstack-quant/analytics/tests/main.rs),
[schema contracts](../../../../finstack-quant/analytics/tests/schema_contract.rs),
[Python bindings](../../../../finstack-quant-py/src/bindings/analytics) and
[WASM bindings](../../../../finstack-quant-wasm/src/api/analytics) implement and
exercise these contracts. Inline and integration regressions distinguish
independent expected arithmetic from comparisons that reuse production
kernels. JSON/schema cases check nonfinite identity and invalid restoration;
they do not independently validate statistical estimators.

Documentation checks on 2026-11-08 passed: the methodology source-ownership
checker covered 1,724 files and 625 records across 34 scopes, all 6,307 local
links in the changed Markdown resolved, and the documentation diff passed
whitespace checks. This ownership check did not reauthenticate the original
audited source hashes; those retain their dated evidence role.

Verified implementation checks on 2026-11-08:

- The analytics Rust suite passed **262/262** tests.
- Focused Python decision coverage passed **22 audit** and **6 host-decision**
  tests. Python static lint/type checks passed. Runtime documentation checks
  passed **3,684** tests, with **112** skips.
- All **seven analytics notebooks** passed the strict runner and saved fresh
  outputs; their reporting derivatives were regenerated.
- WASM validation passed **51 analytics** and **11 shared error-boundary**
  tests.

The final `mise run all` **default CI tier passed**, without the additional
`--full` nightly tier:

- Workspace Rust: **16,347 passed**, **196 skipped**.
- Python: **14,017 passed**, **112 skipped**.
- Native WASM: **239 passed**; Node facade: **1,090 passed**.
- UI: **1,527 tests passed across 129 files**, with TypeScript checks passing.

`mise run gen-write` completed, **482 schemas** were generated, and the final
generation-consistency and audit checks passed. The final `mise run rust-doc`
also passed: public input documentation was clean across **27 packages**, and
the single-path check covered **3,458 items** with no duplicate umbrella paths.

The analytics benchmark targets compiled successfully through `mise exec --
cargo build -p finstack-quant-analytics --benches`. Performance benchmarks were
not executed. These results are functional, structural and integration
evidence; they make no benchmark-performance claim.

Primary definitions and independent illustrative arithmetic remain linked in
the owning records. Formula checks, finite-sample fixtures, synthetic models,
state round trips and host parity provide evidence for their specific cases.
They do not certify population accuracy, tail forecasting, i.i.d. horizon
assumptions, regulatory suitability, vendor equivalence or numerical accuracy
over every representable input. Small-sample computability and a PSD sample
correlation are not guarantees of reliable future risk estimates.

The [central inventory](../README.md#coverage) navigates adjacent core,
models, portfolio and factor scopes. Shared core primitives retain their
own mathematical contracts and independent adoption boundaries.
