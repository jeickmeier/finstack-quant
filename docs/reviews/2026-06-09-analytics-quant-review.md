# Quant Review — `finstack-analytics` + bindings

**Date:** 2026-06-09
**Scope:** All 23 source files in `finstack/analytics` (~7.5k source lines), the core math it delegates to (`finstack/core/src/math/stats.rs`, `finstack/core/src/dates/periods.rs`), `finstack-py/src/bindings/analytics`, `finstack-wasm/src/api/analytics`, `.pyi` stubs, `parity_contract.toml`, and the test suites.
**Method:** Direct line-level review of the core quant math (returns, scalar metrics, tail risk, drawdowns, rolling kernels, lookback/aggregation) plus two parallel deep-review agents over the benchmark/correlation modules and the binding parity surface. The silent-degradation findings (nearest-correlation early exit, FYTD fiscal-day fallback, hardcoded NYSE calendar) were independently re-verified against source before reporting. All 192 crate tests pass (`cargo nextest run -p finstack-analytics --lib --test '*'`).

**Verdict:** No Blockers — core formulas, sign conventions, and annualization are correct. One Major and nine Moderate findings, all at edges and boundaries rather than in the central math.

## Findings

### Major

**M1 — FYTD lookback silently bakes in the NYSE calendar and a Jan-1 fiscal day in both bindings — not overridable, not documented.**
`finstack-py/src/bindings/analytics/performance.rs:33-51`, `finstack-wasm/src/api/analytics/performance.rs:17-50` (verified in source). The Rust API requires `lookback_returns(ref_date, fiscal_config, calendar)` explicitly, and the calendar materially affects the window: `finstack/analytics/src/lookback.rs:158-160` business-day-adjusts the fiscal start with `Following`. The bindings pin `DEFAULT_FISCAL_CALENDAR_ID = "nyse"` and `DEFAULT_FISCAL_START_DAY = 1` with no parameter to change either, and neither the `.pyi` stub (`finstack/analytics/__init__.pyi:561-572`) nor `index.d.ts:541` mentions it. A user with an LSE/TSE panel gets NYSE-holiday-aligned FYTD; a UK April-6 fiscal year (`FiscalConfig::new(4, 6)`) is inexpressible since only the month is exposed.
**Fix:** add `calendar: str = "nyse"` (resolved via `CalendarRegistry`) and `fiscal_year_start_day: int = 1` keyword parameters in both bindings (`lookback_returns`, `lookback_returns_to_dataframe`, WASM `lookbackReturns`) and document the defaults. Defaults preserve existing numbers.

### Moderate

**M2 — Nearest-correlation early-exit can return a non-unit diagonal, violating its own contract and validator.**
`finstack/analytics/src/correlation/nearest_correlation.rs:119-147` (verified in source). The input gate admits diagonals within `1e-3` of 1.0 (`DIAGONAL_GATE`); `symmetrize` preserves the diagonal; and if Cholesky succeeds the matrix is returned as-is. The docs promise a unit diagonal, and the sibling `validate_correlation_matrix` (`correlation/mod.rs:25,66`) rejects anything off by more than `1e-10` — so a "repaired" matrix can fail validation. Existing tests only use exact-unit diagonals.
**Fix:** rescale to unit diagonal (`x_ij / sqrt(d_i * d_j)`) before the early-exit Cholesky check, or take the early exit only when the diagonal is within the strict `1e-10` tolerance.

**M3 — FYTD fiscal-start construction silently degenerates for valid configs like "Feb 30".**
`finstack/analytics/src/lookback.rs:163-173` (verified: `FiscalConfig::new` accepts `start_day` 1..=31 with no month-length check, `finstack/core/src/dates/periods.rs:388`, and core's `DateExt::fiscal_year` explicitly supports overflow days by clamping, `date_extensions.rs:182-189`). Here `create_date(y, Feb, 30)` fails and `unwrap_or(ref_date)` sets the fiscal start to the reference date itself — so FYTD compounds a single observation and returns a plausible-looking wrong number with no error.
**Fix:** clamp the day to the month length exactly as `DateExt::fiscal_year` does (`config.start_day.min(month.length(year))`); the `unwrap_or(ref_date)` fallback then becomes unreachable and should be removed or turned into an error.

**M4 — Rolling-greeks kernel: a NaN poisons up to 63 windows *after* it has left the window.**
`finstack/analytics/src/benchmark.rs:587-604`. Once a non-finite value enters the running sums, `new − old` arithmetic can't remove it (`x − NaN = NaN`); the sums stay NaN until the next full recompute, which happens only every `ROLLING_GREEKS_RECOMPUTE_INTERVAL = 64` slides. Windows containing only finite data emit NaN, contradicting the docstring (lines 511-513: "Non-finite input values are propagated through the output as sentinel NaN values"). Impact is contained because `Performance` rejects non-finite returns at construction (`performance/mod.rs:150-161`), but the kernel is the documented NaN path for crate-internal callers.
**Fix:** trigger an immediate recompute when a non-finite value exits the window (or when the running sums are NaN).

**M5 — Nearest-correlation docs and lock-in test assert mathematically false suboptimality claims — the implementation is actually correct.**
`finstack/analytics/src/correlation/nearest_correlation.rs:13-21, 66-72, 324-376`. The module doc claims the one-sided Dykstra "is not guaranteed to be Frobenius-nearest," citing Borsdorf & Higham (2010); but the unit-diagonal set is affine, and Dykstra needs correction increments only on non-affine sets — carrying the correction on the PSD projection alone **is** Higham (2002) Algorithm 3.3, which does converge to the nearest correlation matrix. The test's counter-example is also wrong: for 3×3 equicorrelation at ρ = −0.55, PSD requires ρ ≥ −1/2, and the true nearest matrix has off-diagonals **−0.5** (Frobenius distance 0.1225) — exactly what the code returns — not −1/3 (distance 0.5307; verified numerically). Borsdorf & Higham (2010) is a Newton-method paper and proves no such "two-sided increments required" result. The docs understate a guarantee the code actually provides, which invites users to bolt on unnecessary post-processing.
**Fix:** correct the module/function docs and the `one_sided_dykstra_yields_valid_nearby_matrix` test commentary; remove the −1/3 claim and the Borsdorf–Higham misattribution.

**M6 — Bindings rename/re-scope `dates()` — canonical-API drift with semantic consequences.**
Rust has `dates()` (full constructed grid, `performance/mod.rs:597-599`) and `active_dates()` (windowed, `performance/mod.rs:548-552`); both bindings expose a single `dates()` that returns `active_dates()` (`finstack-py/src/bindings/analytics/performance.rs:337-344`, `finstack-wasm/src/api/analytics/performance.rs:225-233`). After `reset_date_range`, code ported from Rust silently changes meaning, and the full grid is unreachable from Python/JS. Violates the project's "names match Rust exactly" canonical-API rule.
**Fix:** expose `active_dates()` under its Rust name; add `dates()` with Rust semantics or record an explicit exception in `parity_contract.toml`.

**M7 — `parity_contract.toml` has no `[crates.analytics.symbols]` block — the analytics surface is unguarded.**
`finstack-py/parity_contract.toml:95-115` records only module-level `status = "flattened"` notes. `test_contract_symbols_match_live_surface` iterates `CRATES_WITH_SYMBOLS` only, so the 10-name analytics surface can drift without CI noticing — unlike `valuations`/`attribution`, which have `symbols.public` lists.
**Fix:** add `[crates.analytics.symbols] public = ["AnalyticsError", "Performance", "LookbackReturns", "PeriodStats", "BetaResult", "GreeksResult", "RollingGreeks", "MultiFactorResult", "DrawdownEpisode", "DatedSeries"]`.

**M8 — `.pyi` stub declares `list[float]` where the binding returns `numpy.ndarray`.**
`RollingGreeks.alphas/betas`, `MultiFactorResult.betas`, `LookbackReturns.mtd/qtd/ytd/fytd`, `DatedSeries.values` all return `PyArray1<f64>` (`finstack-py/src/bindings/analytics/types.rs:200-417`) but are typed `list[float]` in the stub (`.pyi:147-153, 168-170, 219-233, 251-253`). Type-checked user code (`.append`, `isinstance(x, list)`, JSON-serialize) passes mypy and breaks at runtime. The scalar-metric methods genuinely return `list[float]`, so the stub is inconsistent within the same module.
**Fix:** declare `numpy.typing.NDArray[numpy.float64]` for those seven attributes.

**M9 — `summary_to_dataframe` hardcodes MAR/threshold = 0 while accepting `risk_free_rate`.**
`finstack-py/src/bindings/analytics/performance.rs:750-795`. Sortino, downside deviation, and Omega are pinned at 0.0 regardless of the `risk_free_rate` argument, undocumented; and the 22-metric selection lives only in the Python binding (no Rust or WASM counterpart), so it can drift per language.
**Fix:** document the fixed MAR at minimum (docstring + `.pyi:588-594`); preferably move the summary assembly into Rust (e.g. `Performance::summary()`) and render it from both bindings.

**M10 — No external golden/parity tests — the numeric fixture is self-pinned.**
`finstack/analytics/src/fixture_test.rs` pins outputs against `api_invariants_data.json`, which guards regressions but not correctness against market-standard sources. The project's testing standards call for golden tests vs Bloomberg/QuantLib/etc. where feasible; for these metrics, `quantstats` (Python) and R `PerformanceAnalytics` are natural cross-check sources for Sharpe/Sortino/VaR/CDaR/Burke/Sterling values on a published return series.

### Minor

- **t-anchors anti-conservative at df 38–39** (`benchmark.rs:278`): the `38..=59` bucket uses the df-40 value 2.0211, but exact t₀.₉₇₅ is 2.0244 (df 38) / 2.0227 (df 39) — CIs ~0.1–0.16% too narrow, contradicting the "conservative" doc claim (line 310). Anchor the bucket at df 38 (2.024394…). Also df ≥ 240 uses 1.95996 vs exact 1.96990 at df 240 (~0.5% narrow, documented as asymptotic regime).
- **Degenerate-benchmark sentinel inconsistency**: batch `beta()`/`beta_only()`/`greeks()` report β = 0.0 for a zero-variance benchmark (`benchmark.rs:354, 393-403, 464`; root cause `core/src/math/stats.rs:928-934`), and `beta_only → treynor` turns that into ±∞, while `rolling_greeks` deliberately emits NaN with a comment explaining why 0.0 is dangerous (`benchmark.rs:573-577`). Standardize on NaN for unidentifiable β.
- **`r_squared` doesn't truncate to the shorter series** (`benchmark.rs:217-220`) unlike every sibling (`tracking_error`, `beta`, `greeks` all use `min` length) — returns NaN on length mismatch where docs say 0.0. Unreachable through `Performance` (aligned panel) but inconsistent crate-internal contract.
- **Nearest-correlation non-convergence misreported** as `NotPositiveSemiDefinite { row: 0 }` with a fabricated row index (`nearest_correlation.rs:180`), and the converged iterate is only PSD up to `opts.tol` with no final Cholesky verification — with user-loosened tolerance (e.g. `1e-6`) the output can fail `validate_correlation_matrix` despite the unconditional PSD doc claim. Add a `DidNotConverge`-style variant and a final `project_psd` + diagonal rescale or `cholesky_correlation` check.
- **Rolling-greeks kernel uses the uncentered `n·Σxy − Σx·Σy` form** (`benchmark.rs:566-580`) — the cancellation-prone variance formula for level-like inputs; mitigated by compensated slides + 64-step recompute (the stress test accepts 4.5e-4 absolute beta error, line 1258), but a centered sliding-Welford form removes the failure mode at the same O(n) cost.
- **Lookback convention asymmetry**: FYTD business-day-adjusts the period start (`lookback.rs:158-160`); MTD/QTD/YTD use raw calendar starts. For grids with observations on non-business days (7-day crypto data with an NYSE calendar, cross-market panels), `fytd` with `FiscalConfig::calendar_year()` disagrees with `ytd` on the same data. Drop the adjustment or apply the same rule to all four selectors.
- **API asymmetry trap**: `Performance::sharpe(risk_free_rate)` takes an *annualized* rate while `sortino(mar)` / `rolling_sortino(..., mar)` / `downside_deviation(mar)` take a *per-period* MAR (`performance/scalar.rs:105 vs 123`). Each is documented, but a user passing the same 0.02 to both gets inconsistent semantics. Add a prominent doc note on `sortino`.
- **`PeriodStats.cpc_ratio` doc mislabels the CPC index as "Common Sense Ratio"** (`aggregation.rs:40`). In standard usage (e.g. quantstats) CPC index = profit factor × win rate × payoff ratio (what's implemented), while Common Sense Ratio = profit factor × tail ratio — a different metric.
- **Burke ratio is the "modified Burke"**: `drawdown.rs:817-825` divides by `sqrt(Σdd²/n)` while Burke (1994) — and R PerformanceAnalytics' default — use `sqrt(Σdd²)` (no 1/n). The formula is documented honestly, but cross-tool comparisons will differ by `sqrt(n)`; the doc should flag the variant.
- **Doc/error mismatch in `Performance::new`**: docs say errors are `InputError::Invalid` but the code returns `InputError::InvalidReturnSeries` (`performance/mod.rs:192`).
- **Bindings nits** (file/line refs verified by the parity sweep):
  - `skew_kurt()` and `value_at_risk_and_es()` (`performance/scalar.rs:260, 277`) not exposed in either binding — Python users computing both VaR and ES pay two tail passes.
  - TS declares `DatedSeries.values?: Float64Array` (`index.d.ts:404-411`) which the WASM payload never emits — `dated_series_to_js` (`finstack-wasm/src/api/analytics/performance.rs:119-128`) only emits the metric-named key; also diverges from Python's `DatedSeries` shape.
  - Python `from_returns(returns_df, …)` reuses the Rust name with a different signature (`performance.rs:266-284`; Rust shape lives at `from_returns_arrays`) without a contract-recorded exception. Mitigated by a good `TypeError` message.
  - Date accessors are methods (`DrawdownEpisode.start()/valley()/end()`, `RollingGreeks.dates()`, `DatedSeries.dates()`) while numeric siblings are properties on the same result classes (`types.rs:283-316, 191-206`). Rust exposes all as plain fields; make them `#[getter]`s.
  - `py_to_date` duck-typing (`finstack-py/src/bindings/core/dates/utils.rs:8-15`) raises a bare `AttributeError` for ISO strings instead of `TypeError`, and silently accepts tz-aware timestamps' wall-clock date with no documented normalization. WASM's `parse_iso_date` is stricter and clearer.
  - `drawdown_details_to_dataframe` doc (`performance.rs:837-840`) omits the `truncated_at_start` column it emits (`performance.rs:881-887`); the `.pyi` is correct.

## Open Questions or Assumptions

- **Sharpe/mean annualization convention**: arithmetic mean × N with annual rf subtracted — algebraically identical to the standard per-period excess form with rf/N, and consistent with Bloomberg/quantstats simple-scaling. Assumed intentional; the geometric path is available via `cagr`.
- **Historical VaR/ES are deliberately not horizon-scaled** (documented — √T is invalid for empirical quantiles), while parametric/CF accept `ann_factor`. Assumed intentional.
- **Empty-window sentinels**: with an empty active window, `value_at_risk`/`max_drawdown`/etc. return 0.0 rather than NaN/error (only `cagr` errors). 0.0 risk for "no data" can read as "no risk" in a report; assumed a deliberate convention, but worth confirming for portfolio-report consumers.

## Brief Summary

This is a genuinely strong analytics crate — well above typical in-house quality. The numerics are taken seriously throughout: Neumaier/Kahan compounding in log-space, Welford variance (sample n−1, matching Bloomberg/QuantLib), type-7 quantiles with explicit cross-tool documentation, bias-corrected G₁/G₂ matching Excel/Bloomberg, a Maillard (2012) monotonicity guard on Cornish-Fisher VaR, SVD-based multi-factor OLS with rank checks (not normal equations), and rolling kernels with drift-bounding periodic recomputation. Construction fails loudly on dirty data (interior NaN, returns ≤ −1, non-monotonic dates), and conventions are documented with literature references. The bindings are thin and precision-clean with centralized, panic-free error mapping.

The residual risk is concentrated in three places: **configuration boundaries** (the hardcoded NYSE/FYTD defaults in the bindings — the one finding that silently changes numbers for real users), **silent-fallback paths** (correlation early-exit, fiscal-day overflow), and **unguarded parity surface** (no analytics symbols in the contract, stub type drift). Fixing M1–M4 and adding the contract block would bring this to production-grade with high confidence.

### Verified-correct items (checked, no findings)

- β = Cov/Var with consistent sample (n−1) convention across `tracking_error`, IR, correlation, and `Performance::correlation_matrix`; β standard error exactly `sqrt(SSres/((n−2)·SSx))`.
- Multi-factor OLS: column-normalized SVD, explicit rank check, errors on collinearity/non-finite/mismatched inputs, residual dof n−k−1, centered SS_tot for R².
- Rolling window indexing: `n−window+1` outputs, right-labeled by window-end date, slide/recompute arithmetic verified line-by-line; no off-by-one.
- Aggregation/lookback compounding uses `Π(1+r)−1` in log-space with Neumaier accumulation, never sums; ISO-week year-boundary grouping correct and tested.
- Higham projection internals: eigenvector layout matches `symmetric_eigen`'s column convention; downstream pivoted Cholesky truncates (rather than rejects) zero pivots so PSD-but-singular repaired matrices validate.
- IR annualization, capture-ratio geometric subset means, Treynor/M² unit consistency, and `excess_returns` geometric rf de-compounding `(1+rf)^(1/n)−1` all correct.
- Binding defaults agree everywhere they exist (window=63, confidence=0.95, rf/mar/threshold=0.0, n=5, freq="daily", agg_freq="monthly", annualize=true) across Python keywords, `.pyi`, WASM `unwrap_or` constants, and TS optionals; no f32 casts or integer truncation; no business logic re-implemented in bindings.

## Quant Notes

- The nearest-correlation finding (M5) cuts the unusual way: the docs claim *less* than the code delivers. Higham (2002), "Computing the nearest correlation matrix — a problem from finance," IMA J. Numer. Anal. 22(3), Alg. 3.3 carries Dykstra corrections only on the PSD-cone projection precisely because the unit-diagonal set is affine (Boyle & Dykstra 1986). The implementation is faithful to it.
- ES tail convention (closed set `{r ≤ VaR}` at the interpolated type-7 threshold, `tail_risk.rs:147-150`) is documented and conservative with ties, but differs slightly from the Acerbi–Tasche α-weighted estimator most academic comparisons use — worth noting in any golden-test tolerance.
- For M10, the cheapest credible external cross-check: run quantstats and R PerformanceAnalytics on one shared monthly return series and pin Sharpe, Sortino, max DD, CDaR, Ulcer/Martin, Sterling, and Burke (the last two are exactly where convention variants will surface — see the Burke `sqrt(n)` note above).
