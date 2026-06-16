# Quant Finance Review — `finstack-quant/core` + Python/WASM Bindings (follow-up pass)

- **Date:** 2026-06-13
- **Scope:** `finstack-quant/core` (~89k LOC: `dates`, `math` incl. `volatility`/`fractional`/`interp`/`random`, `market_data` term-structures/surfaces/dtsm/bumps, `cashflow`, `credit`, `money`/`fx`, `expr`, `rating_scales`, `config`) plus `finstack-quant-py/src/bindings/core/` and `finstack-quant-wasm/src/api/core_ns/`.
- **Method:** Eight parallel read-only review agents (dates/conventions; core numerics; volatility models; term structures; surfaces/dtsm/bumps; cashflow/money/fx; credit; expr+bindings), each instructed to re-derive the math and find issues the 2026-06-09 review **missed** or that remediation introduced, and to report only defects present in the **current** code. The headline findings were independently re-verified at source by the orchestrator (marked ✅ below), including a check of actual call sites to calibrate real-world reachability.
- **Relationship to prior review:** The 2026-06-09 review (`docs/reviews/2026-06-09-core-quant-review.md`, 4 Blockers + ~29 Majors) is **fully remediated and verified present** — Heston "little-trap" CF, SABR Hagan lognormal/normal expansions, Dupire undiscounted forward form, `roll_forward` realized-forward semantics, weighted PAV isotonic regression, the Moody's WARF table (Ba3=1766, CC/C/D=10000), FX pinned-quote triangulation + `triangulated` flag, `npv` past-flow exclusion, Sobol Joe-Kuo table, normal-vol (`vol_normal`) materializers. This pass looks only for **new** or **residual** defects.
- **Known intentional conventions (not flagged):** DV01/CS01 dPV/dy native sign; `npv` default excludes flows ≤ valuation date (opt-in `include_past_flows`, `npv_raw` keeps trade-NPV); Money is `f64`-via-`from_f64` (clean decimals); rates can legitimately reach ≈ −1% (MonotoneConvex DF>1 accepted under `NegativeRateFriendly`).

---

## TL;DR

Core is in strong shape; the prior remediation holds up under independent re-derivation, and the closed-form pricing layer (BSM/Black-76/Bachelier, Heston/SABR/SVI, day-counts, FX triangulation, discounting, PD/LGD/migration coefficient tables) turned up **nothing new** — a strong signal. **No active Blocker.** The residual risk concentrates in two patterns:

1. **Latent silent-failure modes that current call sites happen to dodge** — `binomial_pmf_all` underflows to an all-zero PMF for large pools but the only caller uses N=125; Student-t collapses to the normal at `df>100` but the only caller (t-copula) uses small `df`.
2. **Numerical-robustness gaps in the newer rough-vol and normal-vol surface paths** (rough-Heston fixed Fourier grid, Mittag-Leffler premature truncation, quote-type-unaware vol bumps) that aren't exercised at the wing/long-dated/normal-quote regimes where they'd bite.

Four findings are small, isolated, and fully verified — the cleanest immediate wins: **#1** (log-space binomial), **#6** (wire up inflation extrapolation), **#8** (atomic `set_quotes`), **#14** (`-0.0` hash).

Tally: **0 Blocker, 4 Major, 10 Moderate, 9 Minor.**

---

## Findings

### Major

#### M1 ✅ — `binomial_pmf_all` silently returns an all-zero PMF for large pools

- **Location:** `finstack-quant/core/src/math/distributions.rs:326` (seed) and `:328-331` (recurrence).
- **Issue:** The forward recurrence seeds `let mut prob = (1.0 - p).powi(n as i32);` then multiplies forward. For `p≈0.5` the seed underflows to exactly `0.0` at `n ≳ 1075` (`log10(0.5^1075) ≈ −323.6`, below f64 subnormal range); for `p=0.05` at `n ≳ 14527`. Once `prob == 0.0`, every subsequent `prob *= …` stays `0.0`, so the function returns a vector summing to `0` instead of `1` — with no NaN, error, or warning. The sibling `binomial_distribution` (`:162`, log-space via statrs) does **not** have this bug, so the two "equivalent" entry points disagree for large `n`.
- **Impact:** Silent wrong result — a loss distribution / tranche attachment probability computed from this collapses to zero. The mode mass it should produce is O(1/√n), not zero.
- **Reachability:** **Latent, not active.** The only production caller is the CDS tranche loss distribution at `finstack-quant/valuations/src/instruments/credit_derivatives/cds_tranche/pricer/sensitivities.rs:73`, which uses `num_constituents = 125` (CDX/iTraxx index size), where `0.5^125 ≈ 2.4e-38` is comfortably within f64 range. An existing test (`binomial_pmf_all_finite_for_large_n`, same file) asserts finiteness at N=125 but there is **no guard above it**, so any granular homogeneous pool >~1000 names (large ABS/CLO/retail proxies) would break silently.
- **Fix:** Seed and step in log-space — accumulate `ln P(0) = n·ln(1−p)`, step `ln P(k+1) = ln P(k) + ln(n−k) − ln(k+1) + ln(p/(1−p))`, exponentiate per term — or delegate to `statrs::Binomial::pmf` as `binomial_probability` already does. Trivial change; removes the silent failure for all `n`.

#### M2 — Rough-Heston uses a fixed Fourier grid that does not scale with maturity or moneyness

- **Location:** `finstack-quant/core/src/math/volatility/rough_heston.rs:329` (`DEFAULT_UPPER_LIMIT = 200.0`), `:332` (`GL_PANELS=16`, `GL_ORDER=16` → 256 nodes), `:506-544` (`price_european`).
- **Issue:** The Lewis-transform integral is evaluated on `[1e-8, 200]` with 256 fixed nodes (u-space spacing ≈ 0.78) for **every** maturity and strike, whereas `heston.rs` scales its upper limit (Kahl-Jäckel `C∞ = √(1−ρ²)(v0+κθT)/σ` tail bound) and panel count with the interval. The Lewis integrand carries the oscillatory phase `e^{iux}` with `x = ln(F/K)`; for deep wings (|x| ≈ 0.5–1) it oscillates several times per panel and is under-resolved, and for long maturities the CF tail can extend past where 256 nodes resolve it.
- **Impact:** Silent pricing error on wing strikes / long-dated options. The `prices_non_negative` regression only checks K∈[80,120] at T=1, masking it.
- **Secondary (perf):** `price_european` builds a fresh `FractionalRiccatiSolver` and runs the O(num_steps²)=O(200²) product-integration `solve_d` **inside the integrand closure** (`:506,516-535`), i.e. ~256 × 40,000 ≈ 10⁷ complex ops per option, repeated per strike. The Riccati `D(w)` trajectory depends only on `w = u − i/2` (not strike), so a whole smile re-solves identical trajectories.
- **Fix:** Mirror `heston.rs` — derive the upper limit from CF tail decay and set panel count via a density rule so resolution is preserved across T and |x|. Cache the strike-independent CF trajectory across the smile (analogous to `HestonStripCache`); only the `e^{iux}` factor varies per strike. Add a deep-wing / 2y golden so the error cannot hide.

#### M3 — Mittag-Leffler series can truncate before the dominant terms arrive

- **Location:** `finstack-quant/core/src/math/fractional.rs:283-298` (relative-tolerance break), `:303` (cancellation guard).
- **Issue:** The summation loop breaks as soon as `term.norm() / sum_norm < ML_TOL`. For `E_{α,β}(z)` with `|z| > 1` the term magnitudes `|z|^k/Γ(αk+β)` **rise** to a peak near `k* ≈ |z|^{1/α}/α` before decaying. Early (small-`k`) terms can already satisfy the relative test against the partial sum while later terms are orders of magnitude larger → premature truncation and a wrong value. The `ML_MAX_ABS_ERROR` cancellation guard catches catastrophic cancellation but not this premature-break mode (the peak term may be only moderately larger than the sum, passing the cancellation cap while the break still fired too early).
- **Impact:** Wrong rough-Heston characteristic-function values for arguments with `|z|` in the upper part of the validity band, feeding into rough-vol option prices.
- **Confidence:** Reasoned, not test-confirmed within the model's operating range — recommend a high-`|z|` regression test before/after the fix to confirm magnitude.
- **Fix:** Require the running term index past the estimated peak (only allow the relative-tolerance break once `k > z.norm().powf(1.0/alpha)/alpha`, or once consecutive terms are decreasing), or always sum a fixed minimum number of terms beyond the estimated peak index.

#### M4 — `VolSurface` additive bumps and SVI wing extrapolation are not quote-type-aware

- **Location:** `finstack-quant/core/src/market_data/surfaces/vol_surface.rs:650-665` (`apply_bump` additive path), `:817-848` (extrapolation / SVI wing branch).
- **Issue:** Additive bumps route through `resolve_standard_values` (`RateBp`/10000, `Percent`/100) with no awareness that a `VolQuoteType::Normal` (Bachelier) surface lives in **rate units** (~0.008) rather than lognormal units (~0.2). The SVI wing-extrapolation branch (`:840-848`) always returns a lognormal Black vol via `params.implied_vol`, regardless of `quote_type`.
- **Impact:** Vega/vol-bump risk and wing extrapolation on normal-quoted surfaces — produced by `VolCube::materialize_*_normal` (added in the prior remediation) — are mis-scaled by ~an order of magnitude; a normal-vol grid silently mixes in Black vol at the wings.
- **Fix:** Branch additive bumps and SVI extrapolation on `quote_type`. For `Normal` surfaces force absolute shifts in normal-vol units (and a relative `Factor` for vol-of-vol moves), and fall back to `value_clamped` for wings rather than the lognormal SVI form.

---

### Moderate

#### m1 ✅ — Student-t CDF / inverse-CDF collapse to the normal at `df > 100`

- **Location:** `finstack-quant/core/src/math/special_functions.rs:422-426` (`student_t_cdf`), `:486-490` (`student_t_inv_cdf`).
- **Issue:** Hard cutoff: `if df > 100.0 { return norm_cdf(x) }`. The error does not vanish at the cutoff — there is a discontinuity at `df=100` and a material tail error just above it: `student_t_cdf(-3, 101)` returns Φ(-3)=0.00135 vs true t-CDF ≈ 0.00170 (~26% tail rel error); `student_t_inv_cdf(0.001, 101)` returns −3.090 vs true −3.173 (99.9% VaR quantile understated ~2.6%). The docstring's "error decays as O(1/df)" understates the tail behavior.
- **Impact:** Tail risk / copula tail-dependence with `100 < df ≲ 300` systematically wrong in the tails — precisely where t is used to capture fat tails.
- **Reachability:** **Latent.** The only caller is the Student-t copula (`finstack-quant/valuations/src/correlation/copula/student_t.rs`), which by construction uses small `df` (3–15 — the reason to use a t-copula at all), routing through the exact statrs path. The `df>100` branch is essentially unreachable in production (and at df>100 tail dependence is near-zero anyway).
- **Fix:** Raise the cutoff to `df ≳ 1e6`, or simply delete the approximation and always delegate to `statrs::StudentsT` (it handles large/∞ df). Economic case for the df>100 branch is weak.

#### m2 ✅ — Inflation `extrapolation` policy is unsettable and serde-dead

- **Location:** `finstack-quant/core/src/market_data/term_structures/inflation.rs:623` (`build()` hardcodes `ExtrapolationPolicy::default()`), `:201-209` (`TryFrom` omits it), builder `:558-599` (no setter); field exists at `:165` and `From` writes it at `:193`.
- **Issue:** `build()` always uses the default extrapolation policy; the builder has no `extrapolation()` setter; `RawInflationCurve` serializes the `extrapolation` field and `From<InflationCurve>` writes the live policy, but `TryFrom<RawInflationCurve>` never threads it back. Net: every inflation curve is forced to the default policy, and the serialized field is a no-op on read (round-trip looks faithful only because the value can never be anything but default).
- **Impact:** No control over long-dated CPI continuation past the last pillar (relevant for linkers / pension liabilities); a wire field that looks meaningful but is silently ignored.
- **Fix:** Add `extrapolation(ExtrapolationPolicy)` to `InflationCurveBuilder`, thread `state.extrapolation` through `TryFrom`, and use `self.extrapolation` in `build()` (consider defaulting to `FlatForward` for CPI-level continuation).

#### m3 ✅ — FX rate precedence: an explicit reciprocal quote shadows a date-pinned fixing

- **Location:** `finstack-quant/core/src/money/fx/matrix.rs:203-223` (`rate()` precedence), identical ordering in `get_or_fetch` `:855-868`.
- **Issue:** Resolution order is direct → **reciprocal** → pinned-direct → pinned-reciprocal → observed → provider. A pair-global `set_quote(USD, EUR, r)` (reciprocal of a EUR→USD query) is therefore returned **before** a date/policy-specific `set_quote_on(EUR, USD, on, policy, r2)`. The `set_quote_on` doc (`:335-337`) implies pins are only outranked by a *same-direction* pair-global quote.
- **Impact:** A pinned fixing can be silently overridden by a constant reverse peg for that date.
- **Fix:** If intended, document that an opposite-direction `set_quote` also outranks pins; otherwise move the pinned-direct/pinned-reciprocal checks ahead of explicit-reciprocal (only explicit-*direct* should outrank a pin). See Open Question OQ1.

#### m4 — `set_quotes` mutates partially on validation failure

- **Location:** `finstack-quant/core/src/money/fx/matrix.rs:361-368`; reachable via `load_from_state` `:512`.
- **Issue:** The loop locks `quotes`, then validates and inserts each entry in sequence. If entry `k` fails `validate_fx_rate`, entries `0..k` are already inserted and the lock drops on early return, leaving the matrix half-updated. A single bad rate in a restored snapshot half-applies the snapshot.
- **Impact:** Non-atomic state restore; breaks reload determinism / the all-or-nothing contract implied elsewhere.
- **Fix:** Validate all entries first (or build into a temp map), then insert under the lock only if all pass.

#### m5 — Hazard `sp`/`default_prob` inconsistent with `hazard_rate` for non-LogLinear survival

- **Location:** `finstack-quant/core/src/market_data/term_structures/hazard_curve.rs:262-314` (`sp`), `:687-688` (`cds_quote_bp` fallback uses stored λ).
- **Issue:** `sp(t)` interpolates survival pillars with `survival_interp_style` (e.g. `Linear`), while `hazard_rate(t)` reports the stored piecewise-constant λ. Under any non-LogLinear style, `-ln S(t)/Δt ≠ hazard_rate(t)`, so default densities implied by `default_prob` differ from the instantaneous hazard, and accrual-on-default / protection-leg integrals that consume `hazard_rate` disagree with the survival actually used for discounting. Only the default LogLinear path is self-consistent.
- **Fix:** Restrict survival interpolation to LogLinear (piecewise-constant hazard) for pricing paths, or derive `hazard_rate(t) = -d/dt ln S(t)` from the interpolated survival so both views agree. (O'Kane 2008 §4: piecewise-constant hazard is the self-consistent CDS convention.)

#### m6 — `DiscountCurve::to_forward_curve` produces a biased, non-self-consistent forward strip

- **Location:** `finstack-quant/core/src/market_data/term_structures/discount_curve.rs:1220-1245`.
- **Issue:** Interior forward knots are set to the simple average `0.5*(f_left + f_right)` of adjacent log-DF segment forwards; endpoints use one-sided differences. This is not the instantaneous forward `f(t) = -d/dt ln DF(t)`, and the resulting `ForwardCurve` (which stores simple tenor rates and chains them as `1/(1+f·dt)`) will not reprice the parent DFs. For non-uniform knot spacing the averaging introduces an O(Δt) bias per pillar.
- **Impact:** A forward curve derived from a discount curve disagrees with the discount curve's own implied forwards; round-trip DF reconstruction drifts. Acceptable for display only.
- **Fix:** Compute pillar forwards directly from the parent's `forward(t_i, t_{i+1})`/instantaneous forward, or document the child as a simple-rate display approximation.

#### m7 — Diebold-Li VAR(1) unconditional mean falls back to the raw intercept on a unit root

- **Location:** `finstack-quant/core/src/market_data/dtsm/diebold_li.rs:330`.
- **Issue:** `mu = (I − Phi)^{-1} c` with `.unwrap_or(c)` when `I − Phi` is singular. Near a unit root (common for the persistent level factor) `c` is the regression intercept, **not** the unconditional mean, so the mean-reversion target in `forecast` (`mu + Phi^h(beta − mu)`) is wrong and the forecast drifts toward a meaningless level — with no diagnostic.
- **Impact:** Biased long-horizon factor forecasts for persistent factors.
- **Fix:** Detect near-singular `I − Phi` (spectral radius ≥ 1−ε / condition-number check) and either error or anchor the forecast on the last observed factor rather than `c`.

#### m8 — `.imm()` schedule drops the effective → first-IMM front accrual

- **Location:** `finstack-quant/core/src/dates/schedule_iter.rs:741-746` (and `dates/schedule_gen.rs:54-80`); contrast `.cds_imm()` at `schedule_iter.rs:996-1000`.
- **Issue:** `.imm()` produces only the IMM third-Wednesdays within `[start, end]` (via `generate_imm_dates`), discarding the user-supplied `start` as an accrual anchor (and ignoring its `StubKind::ShortBack`). For a swap/futures strip whose effective date ≠ first IMM date, the initial accrual period from `start` to the first IMM date is dropped. `.cds_imm()` correctly anchors front accrual at `prev_cds_date(start)`.
- **Impact:** Missing initial accrual period for IMM-roll instruments where effective date ≠ first IMM date.
- **Fix:** Prepend `start` when `start < first_imm` (mirroring the CDS front-accrual treatment), or document that `.imm()` returns IMM rolls only and callers must supply the effective-date anchor. See Open Question OQ1.

#### m9 — `EadCalculator::leq_from_observed_ead` clamps realized LEQ to [0,1]

- **Location:** `finstack-quant/core/src/credit/lgd/ead.rs:115-120`.
- **Issue:** Realized ex-post loan-equivalency (CCF) routinely exceeds 1.0 (obligors draw beyond stated commitment pre-default) and can be negative (paydown). Clamping to [0,1] corrupts any downstream CCF calibration that averages these observations — exactly the use case the method's doc describes.
- **Fix:** Return the raw `(observed_ead − drawn) / undrawn` (still guarding `undrawn==0 → None`), and let calibration code decide on winsorization.

#### m10 ✅ — `PyRate` / `PyPercentage` violate Python's hash/eq contract at `-0.0`

- **Location:** `finstack-quant-py/src/bindings/core/types.rs:36-40` (`PyRate::Hash`), `:278-282` (`PyPercentage::Hash`).
- **Issue:** Both declare `#[pyclass(... eq, hash ...)]` where equality derives from the f64-backed `Rate`/`Percentage` `PartialEq`, but `__hash__` hashes `as_decimal().to_bits()` / `as_percent().to_bits()`. Since `0.0 == -0.0` is `true` under f64 but their bit patterns differ, two equal objects can hash differently. `-0.0` is reachable: `Rate::try_from_decimal(-0.0)` and `Rate::from_percent(-0.0)` (`-0.0/100.0 = -0.0`) both store `-0.0`.
- **Impact:** Narrow but real — a `-0.0` rate can land in a different dict/set bucket than `0.0`, breaking Python's data-model invariant. (`PyMoney.__hash__` is Decimal-backed — no signed zero — and is unaffected.)
- **Fix:** Normalize sign of zero before hashing in both `Hash` impls, e.g. hash `(x + 0.0).to_bits()` (turns `-0.0 → +0.0`).

---

### Minor

- **`GaussHermiteQuadrature::integrate_adaptive` order-15 arm ignores `tolerance`** — `math/integration.rs:388-396`. The arm returns `gh20.integrate(f)` (the most accurate available), so this is **not** under-resolution; the real nit is that the function returns `f64` and never signals convergence, making "adaptive" misleading for starting orders 15/20. Document, or return a convergence flag.
- **ICMA `act_act_isma` reference-period length under-count** — `dates/date_extensions.rs:299-310` consumed at `daycount.rs:900`. `months_until` decrements when `self.day() > other.day()` unless both endpoints are EOM; an asymmetric reference like Aug 31 → Feb 27 yields 5 months not 6, scaling the year fraction ~17% low. Only bites the explicit-reference-period helper with a month-end start + non-EOM end (the frequency-based path and EOM→EOM are correct). Fix: derive `period_months` from the schedule frequency, not the two dates.
- **`npv_prediscounted_money` does no date filtering despite the `npv_*` name** — `cashflow/discounting.rs:394-412`. Sums all flows with no valuation-date cutoff, diverging from `npv`'s past-flow exclusion; the `Date` in the `(Date, Money)` signature is dead weight. Document loudly, or accept `&[Money]`.
- **`PyMoney.__rsub__` accepts any scalar while `__radd__` rejects nonzero** — `finstack-quant-py/src/bindings/core/money.rs:289-314`. `5 - money` succeeds while `5 + money` raises; inconsistent currency-safety surface. Reject nonzero scalars symmetrically or document the asymmetry.
- **WASM `DiscountCurve` class doc claims Act/365F default but uses curve-ID inference** — `finstack-quant-wasm/src/api/core_ns/market_data.rs:50-51`. Doc-only; the `@param dayCount` line is already correct, as is the Python wording.
- **`DividendKind::Yield` accepts negative continuous yields while Cash/Stock are floored ≥ 0** — `market_data/dividends.rs:228-230`. Decide policy and make non-negativity symmetric (or document why a negative yield/borrow is allowed).
- **No Basel PD floor applied anywhere** — `credit/pd/calibration.rs:58-133`, `credit/pd/master_scale.rs:117-140`. `ttc_to_pit`/`central_tendency`/`map_pd` can return PDs far below the 3 bp IRB minimum (the AAA master-scale `central_pd` is itself 0.4 bp). Defensible as caller responsibility, but the module advertises Basel II IRB — add an optional `pd.max(BASEL_PD_FLOOR)` helper or document explicitly. See Open Question OQ2.
- **Registry grade validation allows `upper_pd == 0` while `MasterScale::new` rejects it** — `credit/registry.rs:223` (`(0.0..=1.0).contains`) vs `master_scale.rs:81` (`> 0.0`). A config-supplied leading `upper_pd: 0.0` passes registry validation then fails late at `MasterScale::new`. Tighten to `upper_pd > 0.0 && upper_pd <= 1.0`.
- **FX smile interpolated linear-in-strike, not linear-in-variance / log-moneyness** — `surfaces/fx_delta_vol_surface.rs:327`, `surfaces/delta_vol_surface.rs:369`. Documented limitation; flattens wings between 25Δ/10Δ pillars and compounds with linear-in-vol expiry blending. Consider interpolating in total variance vs log-moneyness, or reuse the per-expiry SVI fit.
- **`marginal_pd` computes the forward (conditional) PD, not the unconditional increment** — `credit/pd/term_structure.rs:111-128`. Math is correct (`(S(t1)−S(t2))/S(t1)`) and matches the doc body, but "marginal PD" conventionally means `S(t1)−S(t2)`. Naming only; consider `forward_pd`/`conditional_pd`.

---

## Open Questions / Assumptions

- **OQ1 — Design vs defect:** FX precedence (m3) and the `.imm()` front-stub (m8) read as deliberate design choices as much as bugs, and both touch behavior other crates may rely on. Confirm intended semantics before changing.
- **OQ2 — PD floor ownership:** Is the 3 bp Basel IRB floor `core`'s responsibility or the downstream IRB engine's? That decision sets whether the credit/pd item is a fix or a doc note.
- **OQ3 — Robustness-finding magnitude:** M3 (Mittag-Leffler) and the wing-accuracy half of M2 (rough-Heston grid) were flagged by reasoning, not by a failing test inside the model's operating range. Add a deep-wing / long-dated rough-Heston golden and a high-`|z|` ML test to confirm magnitude before investing in the fixes.

---

## Areas verified clean (no action)

- **Day counts** (`daycount.rs`): Act/360, Act/365F, ActAct ISDA year-splitting, NL/365 Feb-29 exclusion, Act/365L ICMA window, the full 30/360 family (US-SIA, 30E/360 European, 30E/360 ISDA §4.16(h) incl. termination-date exception), ICMA ActAct ISMA reference-period recursion. Business-day conventions (ISDA §4.12). Calendars: NYSE (Good Friday, MLK from 1998, Juneteenth from 2022), USNY/Fed (`mon_if_sun`), cross-year observance, Easter. FX spot/settlement CLS USD asymmetry (`fx.rs`). IMM/CDS dates (`imm.rs`).
- **Numerics:** `norm_cdf`/`pdf`/`erf`/`standard_normal_inv_cdf` (statrs-backed, saturating out-of-domain); Brent IQI acceptance (Numerical Recipes); pivoted Cholesky `cholesky_correlation`; Welford `OnlineStats`/`OnlineCovariance` (correct Bessel n−1); Kahan/Neumaier summation; `Compounding` conversions (`ln_1p`/`exp_m1`); Garman-Klass/Parkinson/Yang-Zhang coefficients; Gauss-Laguerre Golub-Welsch (golden-tested).
- **Pricing models:** Black-76/BSM/Bachelier closed forms (d1/d2, carry, parity, digital limits); Heston CF (Albrecher little-trap form, T-scaled Kahl-Jäckel upper limit); SABR Hagan lognormal eq. A.69a + normal eq. 2.17b with the `−β(2−β)/24` term, χ(z) branch, β=0/β>0 handling; rough-Heston Riccati `a(−i)=0` martingale + H→0.5 classical limit; SVI raw + Roger-Lee wings; arbitrage `g(k)` (butterfly/calendar/SVI/local-vol-density all match Gatheral); implied-vol inversion (intrinsic floor, arb bounds, Halley refinement); VG/Merton CFs.
- **Curves:** DF↔zero↔forward conversions; `roll_forward` realized-forward; MonotoneConvex Hagan-West negative-forward auto-skip; hazard `survival_pillars` build/rebuild consistency + down-bump-past-zero rejection; BaseCorrelation monotonicity/PAVA/FlatZero; Nelson-Siegel/NSS; triangular bucket weights partition to unity.
- **Cashflow/money/FX:** Money cross-currency add/sub rejection, `from_f64` shortest-round-trip, banker's rounding; IRR/XIRR sign-change requirement, deterministic root selection, non-convergence error; FX `validate_fx_rate`/`reciprocal_rate_or_err`, `triangulated` provenance, relative-bump term-structure preservation; ISO-4217 validation.
- **Credit:** Altman Z/Z'/Z'', Ohlson O, Zmijewski coefficients; WARF table (Ba3=1766, B2=2720, CC/C/D=10000); weighted PAV; Vasicek PiT/TtC; matrix-log/Padé generator round-trip, absorbing-state handling; workout-LGD cost discounting. (165 credit unit tests pass.)
- **Expr engine** (`expr/eval_functions.rs`): unified NaN policy, two-pass stable population variance, deliberate `pct_change` vs `/` zero conventions, EWM first-non-NaN seeding, `powi` adjust weight for cross-platform determinism; removed cross-eval cache confirmed a no-op.
- **Bindings:** no business-logic leaks; Decimal path preserved via `decimal_from_py`/`from_decimal`; error mapping via `core_to_py`/`value_error`/`PyKeyError`; curve day-count inference and `ScheduleBuilder` defaults delegated (no default drift); WASM errors → `JsValue`, interp/extrapolation defaults match Rust builders.

---

## Quant Notes

- **M2/M3:** Gatheral (2006) for the rough-vol pricing context; the Lewis (2000) contour integrand's node-resolution requirement scales with `|ln(F/K)|` and CF tail decay — Kahl-Jäckel (2005) gives the Heston tail bound `heston.rs` already uses and rough-Heston should adopt.
- **m1:** For a t-copula, `df > 30` is already visually Gaussian; the cleanest fix is to delete the `df>100` approximation and always use `statrs`.
- **m5:** O'Kane (2008), *Modelling Single-name and Multi-name Credit Derivatives*, §4 — piecewise-constant hazard (LogLinear survival) is the self-consistent CDS convention; mixing linear-in-survival with constant-hazard reporting is internally inconsistent.
- **M1:** Johnson, Kotz & Kemp (1993), *Univariate Discrete Distributions* §3 — the stable evaluation is the log-space recurrence, which also matches the existing `binomial_distribution` path.

---

## Suggested remediation order

1. **One small verified commit:** M1 (log-space binomial), m2 (wire up inflation extrapolation), m4 (atomic `set_quotes`), m10 (`-0.0` hash) — all isolated, fully verified at source, low blast radius. Gate with `mise run rust-lint` + targeted tests.
2. **Robustness pass (with new goldens):** M2 (rough-Heston grid + CF caching), M3 (Mittag-Leffler peak guard), M4 (quote-type-aware vol bumps) — add the deep-wing/long-dated/high-`|z|`/normal-surface tests first so the fixes are measured.
3. **Convention decisions, then act:** resolve OQ1 (FX precedence m3, IMM stub m8) and OQ2 (PD floor) before touching those paths.
4. **Cleanup:** remaining Moderates/Minors as a follow-up.
