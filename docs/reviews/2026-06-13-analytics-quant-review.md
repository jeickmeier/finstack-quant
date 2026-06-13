# Analytics Crate — Quant Finance Review (2026-06-13)

**Scope:** `finstack/analytics/` (returns, risk metrics, tail risk, drawdown,
benchmark/greeks, rolling kernels, nearest-correlation, aggregation, lookback,
the `Performance` orchestrator) and the related Python (`finstack-py/src/bindings/analytics/`)
and WASM (`finstack-wasm/src/api/analytics/`) bindings.

**Grounding:** All 178 `finstack-analytics` library unit tests pass
(`cargo test -p finstack-analytics --lib`). The capture-ratio and alpha
convention gaps below were reproduced numerically. The `tests/` integration
suite was not run and the Python/WASM wheels were not rebuilt.

**Overall:** High-quality, production-grade analytics. Formulas are correct and
well-referenced; numerical hygiene is excellent (Welford/Neumaier/Kahan,
periodic recompute intervals on sliding kernels, disciplined NaN-sentinel
propagation); input validation is strict; Python↔WASM metric parity is
complete. Residual risk is concentrated in **convention alignment** and
**panel-shape strictness**, not in the math. No Blockers.

---

## Findings

### Moderate

#### 1. Capture ratios use a per-period geometric-mean ratio, not the cumulative/annualized return ratio used by Morningstar/Bacon/empyrical

- **Location:** `finstack/analytics/src/benchmark.rs:886` (`geometric_capture`),
  exposed via `up_capture` (`:686`), `down_capture` (`:719`), `capture_ratio` (`:748`).
- **Issue:** `geometric_capture` computes `(Π(1+r_p))^(1/count) − 1` over the
  up/down subset and divides by the benchmark's per-period geometric mean. The
  dominant desk references define capture on the **compound (cumulative)** subset
  return (Carl Bacon; Morningstar) or the **annualized** geometric return
  (empyrical). These do not agree.
- **Evidence:** 36 monthly up-periods at +2% (port) / +1.5% (bench):
  - finstack per-period geometric: **1.3333**
  - Bacon / Morningstar cumulative: **1.4664** (~10% higher)
  - empyrical annualized geometric: **1.3713**

  The gap from the cumulative convention grows with history length and return
  dispersion.
- **Impact:** Capture ratios will not reconcile against Bloomberg/Morningstar
  tear-sheets; the divergence is material for multi-year histories.
- **Fix:** Document which vendor convention is targeted; consider offering a
  cumulative/annualized variant. (The current form is reasonably close to
  empyrical's annualized form to first order, so empyrical parity is the cheapest
  to claim.) The docstrings document "geometric mean" but name no reference tool
  and do not address this choice.

#### 2. `Performance` construction rejects the entire panel on one delisted/defaulted column or any return ≤ −1.0

- **Location:** `finstack/analytics/src/performance/mod.rs:117`
  (`clean_return_column`, lines 136–161).
- **Issue:** After `clean_returns` strips *trailing* NaNs, any column whose length
  no longer equals the grid length is a hard error (delisted / IPO-mid-sample
  names → ragged panel), and any single return that is non-finite or `≤ −1.0`
  (a price hitting 0, a 100% loss period) rejects the **whole multi-ticker
  panel**, not just the offending ticker.
- **Impact:** The engine only accepts perfectly rectangular, survivor-only
  panels. Real-world universes (bankruptcies, ragged start/end dates) are
  rejected wholesale, forcing callers to pre-forward-fill — which itself biases
  drawdown/vol/correlation. Defensible for the clean-equity use case but
  restrictive for credit/distressed/broad-universe analytics.
- **Fix:** Consider per-ticker error isolation (skip/flag the offending column)
  or a NaN-tolerant mode instead of failing the entire constructor.

#### 3. Python `GreeksResult.alpha` is documented as "Jensen's alpha", but the value is the raw-return OLS intercept

- **Location:** `finstack-py/src/bindings/analytics/types.rs:149` labels it
  `Jensen's alpha (annualized)`. The Rust source
  (`finstack/analytics/src/benchmark.rs:492`) computes
  `(mean_x − β·mean_y)·ann_factor` — the intercept of `r_p = α + β·r_m` on **raw**
  returns, with no risk-free term.
- **Issue:** True Jensen's alpha regresses excess returns; it differs from the
  raw intercept by `r_f·(1−β)` per period. At `r_f = 4%`, `β = 1.6` that is
  **−240 bps/yr** annualized. The Rust `GreeksResult` field doc correctly says
  "intercept"; only the Python getter mislabels it.
- **Fix:** Change the Python getter doc to "regression intercept alpha (not
  risk-free-adjusted Jensen's alpha)" to match Rust, or — if Jensen's alpha is
  intended — regress excess returns in `greeks` / `multi_factor_greeks`.

### Minor

#### 4. `greeks` / `multi_factor_greeks` alpha ignores the risk-free rate by design
Same root as #3. Matches quantstats' "greeks" convention and is documented in
the Rust source, so it is a convention note rather than a bug; ensure callers
know `alpha` is not CAPM alpha.

#### 5. Sortino `mar` (per-period) vs Sharpe `risk_free_rate` (annualized) unit asymmetry
Thoroughly documented at `finstack/analytics/src/performance/scalar.rs:119`, but
`summary_to_dataframe` and the flat facade make it easy to pass an annualized
target to `sortino` / `downside_deviation` by mistake. Consider a brief note in
the Python/WASM method docs (currently silent on the per-period requirement).

#### 6. `beta` CI uses the asymptotic 1.96 for `n−2 ≥ 240`
`finstack/analytics/src/benchmark.rs:287` — ~0.5% narrower than exact t(240).
Documented and immaterial at that sample size.

---

## Open Questions / Assumptions

- **Capture-ratio target:** which reference (Morningstar cumulative, empyrical
  annualized, or per-period geometric) is intended? Determines whether #1 is a
  fix or a doc clarification.
- **Panel shape contract:** is the rectangular/survivor-only requirement (#2)
  intentional for the primary equity use case, or should ragged/delisted columns
  be supported?
- **Alpha semantics:** should `greeks.alpha` remain the raw intercept
  (quantstats-style) or become Jensen's alpha (CAPM)? Either is fine; the Python
  doc must match the choice (#3).

---

## Strengths (verified)

- **Distribution moments:** Fisher G₁/G₂ skew/kurtosis match Excel/Bloomberg
  (`tail_risk::moments4`); one-pass Terriberry accumulation, bias-corrected.
- **Cornish-Fisher VaR:** carries the Maillard (2012) monotonicity guard
  (`dz_cf/dz > 0`) with a clean parametric (Gaussian) fallback and structured
  warning.
- **Expected Shortfall:** provably ≤ VaR via the closed-tail `{r ≤ VaR}`
  convention (Rockafellar & Uryasev); invariant tested.
- **OLS beta / SE / t-CI:** correct slope, `(n−2)`-dof standard error, tabulated
  Student-t critical values with conservative step-down bucketing.
- **Nearest correlation:** faithful Higham (2002) Algorithm 3.3 with Dykstra
  correction on the PSD cone only (affine unit-diagonal set), divide-and-conquer
  eigensolver for scale, final projection+rescale for unconditional PSD output.
- **Numerical hygiene:** Welford/Neumaier/Kahan accumulation; sliding-window
  kernels recompute every 64 (greeks) / 1024 (moments) steps to bound drift;
  large-offset stability tested.
- **NaN discipline:** zero-variance benchmark → NaN beta (not a plausible 0);
  invalid annualization factor → NaN; degenerate windows surface NaN sentinels.
- **Bindings:** complete Python↔WASM metric parity; host-idiomatic DataFrame
  helpers (Python) and Float64Array typed outputs (WASM); thin wrappers with no
  business logic.

## Quant Notes

- **Capture conventions:** Bacon, *Practical Portfolio Performance Measurement*
  (cumulative subset return); Morningstar Upside/Downside Capture (geometric);
  empyrical `up_capture`/`down_capture` (annualized geometric via `annual_return`).
  Per-period and annualized geometric agree to first order in small returns;
  cumulative diverges with horizon.
- **VaR horizon scaling:** `parametric_var`/`cornish_fisher_var` with `ann_factor`
  apply delta-normal scaling (`μ·f + z·σ·√f`). The `Performance` facade returns
  per-period VaR (`ann_factor = None`); only `modified_sharpe` annualizes, and it
  does so consistently in both numerator and denominator — correct.
- **Burke ratio:** the *modified* (RMS, `1/n`) form, explicitly documented as
  differing from R `PerformanceAnalytics` by `√n` — good disclosure.
