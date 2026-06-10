# Quant Finance Review — `finstack/valuations` + Python/WASM Bindings

- **Date:** 2026-06-09
- **Scope:** `finstack/valuations` (all instrument families, models, calibration, metrics, market conventions, pricer registry/JSON) plus `finstack-py/src/bindings/valuations/` and `finstack-wasm/src/api/valuations/` with stubs, facades, and `parity_contract.toml`.
- **Method:** Seven parallel read-only review agents (rates, credit, fixed income cash, securitized, FX/equity/commodity/exotics, models/calibration/metrics infra, bindings parity), each tracing implementation → helpers → tests before reporting. The five blockers were independently re-verified at the source by the orchestrator (marked ✅). No code was modified and no tests were executed; numerical claims were verified by derivation/recomputation.
- **Severity guide:** Blocker = incorrect price/P&L/risk or broken market standard; Major = numerical instability, bad edge case, or missing market-standard feature; Moderate = perf/API/latent hazard; Minor = docs/polish.
- **Known intentional convention (not flagged):** DV01/CS01 are dPV/dy native sign — negative for long bonds/receivers is intentional.

---

## TL;DR

Core analytics (Black/Bachelier closed forms, barrier closed forms, ISDA CDS engine, implied-vol solver, curve bootstrap, FD sensitivities, JSON pricer plumbing) are production-grade. Defects cluster in: MtM cross-currency swaps, the callable-bond tree, dollar rolls, the "SDA" default curve, seasoned (mid-life) exotics, tranche premium legs, SABR calibration under normal vol quotes, and one broken Python binding. **5 blockers, ~24 majors.**

Recurring failure patterns:

1. **Sibling drift** — a bug class fixed in one module but not its twin (variance-swap annualization, OAS solver residuals, time-axis lookups, OAS mislabeling).
2. **Seasoned-trade support never designed in** (CMS, autocallable, cliquet, equity TRS).
3. **Tests pinning the implementation rather than market truth** (XCCY CIP, dollar roll, call windows, SABR normal-vol calibration).

---

## Blockers

### **[FIXED 2026-06-09]** B1 ✅ — MtM-resetting XCCY swap inverts the CIP forward-FX ratio

- **Location:** `finstack/valuations/src/instruments/rates/xccy_swap/pricing_mtm.rs:486` (`compute_resetting_notional_and_df_r`), consumed by `pv_mtm_reset` and `mtm_resetting_leg_schedule`.
- **Issue:** Code computes `x_t = spot × (P_C / P_R)`. With spot quoted constant-per-resetting (verified against `FxProvider` semantics, `finstack/core/src/money/fx/provider.rs:80-89`), CIP requires `F = S × P_R / P_C`. Example: EUR@1%/USD@2%, S=1.10 → correct F(1y)=1.111 (low-yield ccy at forward premium); code gives 1.089. The module doc and the unit test (`compute_resetting_notional_matches_formula`) encode the inverted formula.
- **Impact:** Every reset notional, rebalancing cashflow, coupon on the resetting leg, and final exchange is wrong; error grows ~2× the rate differential per year (~35% notional error at the last reset for 5%/2% rates over 5y).
- **Fix:** `x_t = spot × (p_r / p_c)`; update module doc, design-spec formula, and test expectation.

### **[FIXED 2026-06-09]** B2 ✅ — Callable-bond tree silently drops part of any cashflow inside the first time step

- **Location:** `finstack/valuations/src/instruments/fixed_income/bond/pricing/engine/tree/bond_valuator.rs:310-334`.
- **Issue:** Distributed cashflow mapping gates the lower-step share on `step_idx > 0`; a coupon with `t < dt` (raw index in (0,1)) loses its `(1−weight)` share. Backward induction includes step 0, and the term-loan tree engine books `lo == 0` correctly — the guard is anomalous. The PV-preservation test uses annual coupons and can't catch it.
- **Impact:** Any callable/putable bond valued within one tree step of a coupon (default 100 steps on 10y → ~36-day window, ~20% of dates) leaks up to a full coupon of PV; contaminates OAS, effective duration/convexity, quote conversions.
- **Fix:** Change the guard to `if step_idx < num_steps`, booking the share at step 0 with the existing `value_at_step_time` DF correction.

### **[FIXED 2026-06-09]** B3 ✅ — Dollar-roll implied financing rate: drop sign inverted; paydown treated as full-par cost

- **Location:** `finstack/valuations/src/instruments/fixed_income/dollar_roll/carry.rs:106-115` (and `break_even_drop`, lines 143-152).
- **Issue:** `net_benefit = drop + coupon − paydown`. The breakeven derivation (independently re-derived) is `r = [coupon + s·(100 − P_back) − drop] / P_front × 360/d`. A larger drop should *lower* the implied financing rate (roll special/cheap); code raises it, so `roll_specialness` (line 134) moves the wrong direction, contradicting its own doc. Paydown enters at full par with the wrong contribution instead of `s·(100 − P_back)`.
- **Impact:** Implied rate, specialness (bp), and break-even drop wrong in level and direction — directly misleading roll/RV decisions. Tests only assert wide bounds, satisfied by the flipped convention too.
- **Fix:** numerator = `coupon_income + principal_paydown × (100 − back_price)/100 − drop`; mirror in `break_even_drop`.

### **[FIXED 2026-06-09]** B4 ✅ — "SDA" default curve is ~10× the PSA/BMA standard at peak, ~100× at terminal

- **Location:** `finstack/cashflows/src/builder/specs/default.rs:66-80` (consumed by structured_credit via `DefaultModelSpec`).
- **Issue:** Implemented: ramp to **6% CDR at month 30**, decline to **3% terminal by month 60**. Actual 100 SDA: 0.02%/month ramp to **0.60% CDR** at month 30, **flat months 30–60**, decline months 61–120 to **0.03%**, flat after. Wrong level and wrong shape (no plateau, wrong decline window).
- **Impact:** Anyone selecting `DefaultCurve::Sda { speed_multiplier: 1.0 }` expecting 100 SDA gets default rates an order of magnitude too high — massively wrong tranche prices/losses.
- **Fix:** Implement actual SDA knots (0.006 peak, plateau 30–60, decline to 0.0003 by month 120) or rename the variant.

### **[FIXED 2026-06-09]** B5 ✅ — Python `CDSOption.price()` hardcodes the decommissioned `"black76"` model — always fails

- **Location:** `finstack-py/src/bindings/valuations/credit_derivatives.rs:89-96`; `price()` at lines 54-57 passes the macro's `$model` with no override.
- **Issue:** The registry registers only `BloombergCdso` for CDSOption (`finstack/valuations/src/pricer/credit.rs:43-51`; registry test asserts `get_pricer(CDSOption, Black76).is_none()`). Every Python `CDSOption.price(market, as_of)` raises. No behavioral test covers it (only topology tests touch the module).
- **Fix:** Pass `"default"` (resolving via `Instrument::default_model()`) for all four CDS-family wrappers (see also Mo-binding-5); add a `CDSOption.example().price(...)` smoke test.

---

## Majors

### Rates

1. **[FIXED 2026-06-09]** **Cash-settled swaption `ParYield` annuity never discounted from expiry** — `swaption/types/swaption.rs:743-777` (dispatch :684, used in `price_model_base` :509-535). `A = (1 − (1+S/m)^(−N))/S` is the cash annuity *at expiry*; market formula requires `DF(as_of→settlement) × A_cash × Black(...)`. Physical/IsdaParPar/ZeroCoupon modes discount; ParYield doesn't. Prices overstated by ~e^{rT} (4% at 1y, 20%+ at 5y expiry). Mitigant: serde default is `IsdaParPar`. Fix: multiply by `relative_df_discounting(disc, as_of, expiry)` in the ParYield arm.
2. **[FIXED 2026-06-09]** **HW1F MC exotics (TARN, snowball, callable range accrual) mix risk-neutral paths with deterministic time-0 DFs** — `tarn/pricer.rs:92,141`, `snowball/pricer.rs:95,131`, `callable_range_accrual/pricer.rs:141,553`; harness `exotics_shared/hw1f_mc.rs` has no pathwise numeraire. Values `P(0,T)·E^Q[coupon(r_T)]` instead of `E^Q[coupon·e^{−∫r}]`; bias ≈ σ²/(2κ²)(1−e^{−κT})² ≈ 10–15bp at T=5y for σ=1%, compounding for inverse floaters and TARN knockout timing. The Bermudan LSMC in the same tree (`swaption/pricing/monte_carlo_lsmc.rs:524-630`) does it correctly via pathwise bank factors. Fix: accumulate pathwise bank account in `RateExoticHw1fMcPricer`.
3. **[FIXED 2026-06-09]** **Seasoned CMS coupons re-projected from live curves instead of recorded fixings** — `cms_swap/pricer.rs:89-166`, `cms_option/pricer.rs:85-177`, `cms_option/replication_pricer.rs:228-233`, `cms_spread_option/pricer.rs:114-129`. No `FIXING:` lookup exists; cap/floor pricer enforces it correctly (`cap_floor/pricing/pricer.rs:46-60`). Phantom P&L on every seasoned CMS trade. Fix: mirror the cap/floor pattern.

### Credit

4. **[FIXED 2026-06-09]** **Tranche premium-leg accrual-on-default adjustment is complement-swapped** — `cds_tranche/pricer/engine.rs:251-262`. Correct: subtract `(1−f)·ΔEL`; code subtracts `f·ΔEL` (agrees only at f=0.5; moves the wrong direction as hazard rises). With AoD disabled, premium = full coupon on start-of-period notional (defaulted names pay through period end), error `c·Δ·ΔEL`/period — tens of bp on HY equity tranches. Fix: `(1−f)·ΔEL` when enabled, full `ΔEL` when disabled. Note: the protection-leg use of `f` (engine.rs:305-309) is correct — don't change it.
5. **[FIXED 2026-06-09]** **Tranche discounting uses absolute DFs on the credit curve's time axis** — `cds_tranche/pricer/engine.rs:214-217, 277, 305-309, 333-348`. Payment times via `years_from_base` (hazard curve base + day count) fed to `discount_curve.df(t)`; no re-basing to as_of (upfront row *is* as_of-based — inconsistent within one PV). Single-name CDS avoids both via `df_asof_to`. Zero when curve base = as_of and day counts match; biased otherwise. Fix: discount-curve-axis relative DFs.
6. **[FIXED 2026-06-09]** **No senior-side recovery amortization for tranches** — `cds_tranche/pricer/engine.rs:222, 499-552`, `expected_loss.rs:48-112`. Detachment never written down by `defaulted × R` from the top; super-senior keeps paying full premium after defaults. Pool survival factor conflates loss with defaulted notional (exact only at R=0). Fix: track defaulted notional, erode detach top-down.

### Fixed income (cash)

7. **[FIXED 2026-06-09]** **ACT/ACT (ICMA) bonds cannot be priced from yield** — `bond/pricing/quote_conversions.rs:667,683`, `bond/metrics/duration_macaulay.rs:98-105`, `convexity.rs:116-123`, `ytm_solver.rs:269-273,382-386`, `inflation_linked_bond/types.rs:840-885`. `year_fraction` with default context hard-errors `MissingFrequencyForActActIsma`. The accrual engine passes frequency; the YTM/duration/convexity path doesn't. Canonical TIPS constructor defaults to ActActIsma → `real_yield` fails. Fix: thread `DayCountContext { frequency, .. }` through.
8. **[FIXED 2026-06-09]** **Call/put windows exercise only at the two endpoint dates on the tree** — `bond/pricing/engine/tree/bond_valuator.rs:77-96` (`_cashflow_dates` is dead). YTW (`quote_conversions.rs:783-803`) enumerates every coupon date in the window; tree/OAS and YTW use different exercise sets. Issuer option materially undervalued (PV biased up, OAS down). A unit test enshrines the behavior; the `CallPut` docs call it a window. Fix: exercise at every step (or coupon date) within a window.
9. **[FIXED 2026-06-09]** **Term-loan IRR/DM metrics price at `clean% × notional_limit`** — `term_loan/metrics/irr_helpers.rs:85-95` (feeds YTM/YTC/YTW), `discount_margin.rs:104-110`, `cs01.rs:52-55`. No accrued interest; original commitment instead of funded/amortized outstanding (LSTA prices against current funded outstanding). Loan amortized to 70% quoted at 99 → purchase leg overstated ~43%. Fix: `px/100 × outstanding_at_settlement + accrued_at_settlement`.
10. **[FIXED 2026-06-09]** **TIPS index ratio interpolated in the lagged month, not the settlement month** — `inflation_linked_bond/types.rs:649-661` + `core/src/market_data/scalars/inflation_index.rs:349-359`. Official: `RefCPI(d) = CPI(m−3) + (day−1)/D(settlement month) × [CPI(m−2) − CPI(m−3)]`; code weights by days in the lagged month and inherits `add_months(-3)` day-clamping kinks. Won't reconcile with Treasury-published index ratios; wrong settlement invoices.
11. **[FIXED 2026-06-09]** **Ex-coupon window: accrued floored at zero; buyer keeps the ex-period coupon** — `finstack/cashflows/src/accrual.rs:498-513` (returns `None` → accrued 0) + `bond/types/pricing.rs:20-38` (no ex-coupon flow filter). Market standard (gilts) is negative accrued and no coupon to buyer. Clean↔dirty and YTM wrong in the ex window.

### Securitized

12. **[FIXED 2026-06-09]** **Prepayment01/Default01 are exactly zero for PSA/SDA curve specs** — `structured_credit/metrics/risk/prepayment01.rs:30-39`, `default01.rs:30-38`; root cause `cashflows/src/builder/specs/prepayment.rs:75-86`, `default.rs:66-80` (bump mutates `cpr`/`cdr`, curve branches ignore those fields). Also `prepayment01.rs:52` lacks the clamp-aware `achieved_bump` correction `default01.rs:43` has. Fix: bump `speed_multiplier` for curve specs.
13. **[FIXED 2026-06-09]** **CMO `Oas` metric is a static Z-spread mislabeled as OAS** — `cmo/metrics/oas.rs:33-128`. No rate paths/option value; non-bracketable solve returns `oas: 0.0, converged: false` and the metric layer discards the flag (silent 0). The identical mislabel was fixed for MBS passthrough (`mbs_passthrough/metrics/mod.rs:47-79`). Fix: route through MC over collateral paths + waterfall, or rename; propagate non-convergence.
14. **[FIXED 2026-06-09]** **MBS MC-OAS HW model not fitted to the discount curve; payment delay ignored on paths** — `mbs_passthrough/metrics/mc_oas.rs:326-359, 201-283`. Constant θ from a single 5Y zero → at OAS=0 the model doesn't reprice the curve, so solved OAS absorbs calibration error; path discounting ignores the 55/50/14-day delay (~several bp). Fix: fit θ(t) to curve DFs; discount to actual payment dates.
15. **[FIXED 2026-06-09]** **`wam` interpreted two contradictory ways** — `mbs_passthrough/pricer.rs:122-146` (original term at issue) vs `metrics/mc_oas.rs:249` (remaining term at as_of). Same instrument amortizes over different horizons across its own metrics; pool-data convention matches the MC reading. Fix: define as current remaining WAM in both (PSA ramp keeps using WALA).
16. **[FIXED 2026-06-09]** **Broken-PAC principal vanishes** — `cmo/waterfall.rs:247-300, 152-171` + `cmo/pricer.rs:186-197`. After support exhaustion, excess principal above the PAC schedule lands in dropped `residual_principal` instead of accelerating the PAC; Σ tranche principal < collateral principal in fast-prepay scenarios. Fix: allocate remaining principal to PACs beyond schedule (balance-capped).
17. **[FIXED 2026-06-09]** **CMO IO strips: two inconsistent models, no interest conservation** — `cmo/waterfall.rs:374-378, 115-127` + `cmo/pricer.rs:154-172`. Priced standalone: IO interest uncapped at collateral interest (IO+PO PV > collateral PV possible). Inside waterfall: IO notional never amortizes, over-consuming interest at priority 0. No shortfall tracking; `AgencyCmo::example()` is itself interest-deficient (Z tranche silently shorted). Fix: cap at available interest, amortize IO notional by factor, track shortfall, validate at build.

### FX / equity / exotics

18. **Equity variance swap under-annualizes weekly/bi-weekly observations** — `equity/variance_swap/pricer.rs:236-245, 279-287`: 7-day step → factor 36 (should be 52, as `fx_variance_swap/pricer.rs:147-155` correctly does), 14-day → 18 (should be 26). Realized variance understated ~31% weekly. Daily default unaffected. Fix: match FX mapping or annualize from actual schedule density.
19. **Seasoned autocallables evaluate past observation dates against simulated spot and future-value the payoff** — `equity/autocallable/pricer.rs:97-101, 139-149` + `monte_carlo.rs:199-218`. Negative observation times survive into the payoff with `df_ratio > 1`; no observed-fixing mechanism (barrier option has one). Any mid-life autocallable mispriced. Fix: require fixings for past dates (or error); date-based df_ratios.
20. **Seasoned cliquets silently discard locked-in period returns** — `equity/cliquet_option/pricer.rs:132-142`: the `t > 0` filter drops past resets; no fixing state, so mid-life cliquet reprices as a shorter new contract. Fix: observed reset fixings (Asian `accumulated_state` pattern) or reject past resets.
21. **Equity TRS total-return leg has no spot sensitivity** — `equity_trs/pricer.rs:69-128` + `common_impl/pricing/trs.rs:228-293`. `period_return = F(t₂)/F(t₁) − 1` with the level cancelling → pure deterministic carry; current-period realized move never enters PV; equity delta ≈ 0, contradicting the module README. Confidence medium-high (possible upstream settlement-layer intent, but nothing supports it). Fix: current period from period-start fixing → spot, plus remaining carry.
22. **FX digital/touch options never validate payout currency** — `fx_digital_option/types.rs:133-163`, `fx_touch_option/types.rs:227-249`; expired digital returns `payout_amount` verbatim (`fx_digital_option/pricer.rs:54-69`). Base-currency cash payout (foreign-cash digital, `e^{−r_f T}N(d₁)`) silently priced with the domestic formula and labeled quote-currency. Fix: validate `payout_amount.currency() == quote_currency` or implement foreign-payout formulas.

### Models / calibration / market

23. **[FIXED 2026-06-09]** **SABR calibration weights normal-vol quotes with lognormal Black vega — normal-convention cubes are never smile-calibrated** — `models/volatility/sabr/calibration.rs:18-21` (used at :266, :357, :603 and `calibration/targets/swaption.rs:233-289`). A ~1% normal vol fed to `black_vega` → wing weights collapse to the 1e-10 floor (10⁷–10⁸:1 vs ATM); LM declares `ConvergedGradient` at iteration 0 (ν=0.3, ρ=0.0 initial guesses survive); the residual gate scales by `w/w_max` and passes vacuously. The in-tree test passes without fitting anything. Fix: Bachelier vega `√T·φ((F−K)/(σ_N√T))` for normal quotes (shifted-Black vega for shifted); add an unweighted wing-repricing test.
24. **[FIXED 2026-06-09]** **`SABRSmile::strike_from_delta` returns the strike on the wrong side of the forward** — `sabr/smile.rs:120-138`. Code: `K = F·exp(N⁻¹(Δ)·σ√T)`; correct: `K = F·exp(−σ√T·N⁻¹(Δ) + σ²T/2)`. Requested 25Δ call returns a strike with actual delta 0.78. Only tested at Δ=0.5 where both errors vanish. Corrupts any delta-quoted smile construction.
25. **[FIXED 2026-06-09]** **Parallel "rate bp" bumps shift IR-futures quotes with the wrong sign at 1/100 magnitude** — `market/quotes/rates.rs:273-287, 305-307` (`price += rate_bump` instead of `price −= 100·rate_bump`), consumed by `calibration/bumps/rates.rs:313, 318`. Plan-driven parallel/key-rate shocks on futures-bearing curves (standard USD short end) silently mis-shock those pillars. The CDS-option IR DV01 sidecar excludes futures and is safe.
26. **[FIXED 2026-06-09]** **`calibrate_with_derivatives` feeds LM the gradient of the unweighted SSE while minimizing the vega-weighted SSE** — `models/volatility/sabr_derivatives.rs:206-217` vs `sabr/calibration.rs:342-367`. Converges to the wrong problem's stationary point; doc claim of consistency is false since weighting was added.

### Bindings

27. **[FIXED 2026-06-09]** **WASM map-returning functions emit ES2015 `Map`s; `index.d.ts` declares plain objects** — `api/valuations/exotic_rates.rs:51` (`tarnCouponProfile`), `fx.rs:247-251` (`FxOption.greeks()`), `sabr.rs:179-184` (`arbitrageDiagnostics`), `pricing.rs:194-197` (`listStandardMetricsGrouped`). serde-wasm-bindgen 0.6 default; nothing enables `json_compatible()`. TS users get silent `undefined` property reads; Python returns dicts (shape parity broken). No `.test.mjs` facade tests exist. Fix: `Serializer::json_compatible()` or `js_sys::Reflect` (as `analytic.rs` `bsGreeks` already does); add facade tests.
28. **[FIXED 2026-06-09]** **`SabrCalibrator.calibrate` defaults `beta=1.0` only in Python** — `finstack-py/src/bindings/valuations/sabr.rs:35,390`; Rust and WASM require it. Omitting beta silently fixes the equity convention for a rates user. Fix: make required (match Rust/WASM).
29. **[FIXED 2026-06-09]** **WASM SABR surface drift** — missing `params` getter on `SabrModel` and `withTolerance` on `SabrCalibrator` (Python has both); `calibrate_auto_shift` / `calibrate_auto_shift_with_derivatives` unreachable from both hosts (relevant for negative-rate smiles).

---

## Moderates

### Time/axis hygiene
- **[FIXED 2026-06-09]** Option expiry measured with instrument *accrual* day count instead of ACT/365F: `swaption/types/swaption.rs:519,546,645,957`, `cap_floor/pricing/pricer.rs:98-100` (Act360 ⇒ T inflated ×365/360), CMS pricers. `ir_future_option/types.rs:99-104` and `inflation_cap_floor/types.rs:401-410` do it correctly — drift, ~+0.7% vol-equivalent bias. Goldens: `usd_swaption_5y_into_5y_receiver_25_otm` NPV tolerance widened (documented ~0.83% ACT/365F residual vs the Bloomberg SWPM screen); `usd_swaption_normal_vol_self_test` expected values regenerated.
- **[FIXED 2026-06-09]** Axis-based `df(t)`/`zero(t)` instead of `df_between_dates`: equity variance swap Carr-Madan (`equity/variance_swap/pricer.rs:542`), autocallable df_ratios numerator, `equity_index_future/pricer.rs:120-151`, `equity_trs/pricer.rs:80-82`, convertible bond floor (`convertible/metrics/bond_floor.rs:51-69`, plus silent `unwrap_or(0.0)` on year fractions — also at `bond/metrics/wal.rs:59`, `bond_valuator.rs:228`), inflation swap (`inflation_swap/types.rs:309-318, 352-362`). All biased whenever curve base ≠ as_of.

### Convention drift
- **[FIXED 2026-06-09]** OIS presets apply ARRC 2bd / BoE 5bd FRN lookbacks to cleared OIS swaps (`data/conventions/rate_index_conventions.json` + `irs/compounding.rs:211-235`); cleared OIS compounds plain in-arrears with payment delay only. Sub-bp to ~1bp basis; also disables the exact `1/DF` fast path. Goldens: envelope-bootstrapped self-test fixtures regenerated (`aapl_equity_vol/svi_self_test`, `usd_swaption_normal_vol_self_test`, `usd_5y_cds_self_test` dv01, `inflation_linked_bond_5y`); `cdx_ig_46_payer_atm_jun26` NPV band widened to $6 (documented -$5.32 bootstrapped-curve residual vs the Bloomberg screen).
- **[FIXED 2026-06-09]** `CompoundedInArrears.observation_shift` has contradictory sign/DCF semantics between the two IRS pricing paths (`irs/cashflow.rs:52-62` vs `:163-184`); `{lookback:2, shift:2}` silently cancels on one path, errors on the other.
- **[FIXED 2026-06-09]** Tranche IMM schedule path skips business-day adjustment (`cds_tranche/pricer/sensitivities.rs:237-251`).
- **[FIXED 2026-06-09]** FX variance swap seasoned MTM mixes annualization bases (`fx_variance_swap/pricer.rs:70-79`) — the W-33 bug fixed on the equity side (`variance_swap/pricer.rs:166-179`).
- **[FIXED 2026-06-09]** KO rebates pay-at-expiry only (`models/closed_form/barrier.rs:573-614`); market standard is pay-at-hit; the at-hit machinery already exists in `fx_touch_option/pricer.rs:165-214`. Add a `rebate_timing` field.
- **[FIXED 2026-06-09]** Quanto `fx_rate_id`/`fx_vol_id` quote direction unenforced; shipped example inconsistent (`quanto_option/types.rs:189-215` vs `pricer.rs:67-119`). The drift-adjustment formula itself is correct and parity-tested.

### Sensitivity unit traps
- **[FIXED 2026-06-09]** Tranche `calculate_cs01` bumps hazard λ while documented as a 1bp *spread* bump (≈1.67× mislabel at R=40%) — `cds_tranche/pricer/sensitivities.rs:449-481`; registered metric calculators are correct. Ensure its spread bump only. Make error if par spreads not available.
- **[FIXED 2026-06-09]** Index CS01 silently falls back from par-spread re-bootstrap to hazard bump on error — `cds_index/pricer.rs:433-447`. Make it an error.
- **[FIXED 2026-06-09]** `Correlation01` is per-unit-ρ while `Recovery01` is per-1% — 100× internal inconsistency (`cds_tranche/pricer/sensitivities.rs:485-526` vs `metrics/correlation01.rs`).
- **[FIXED 2026-06-09]** IR-future model convexity adjustment `0.5σ²T₁T₂` has no vol-unit guard (`ir_future/types.rs:454-486`); lognormal vol inflates CA by hundreds×. Document normal-vol contract + sanity bound.
- **[FIXED 2026-06-09]** `SABRModel::implied_volatility` returns normal vol for β≈0 and Black vol otherwise from one untagged API (`sabr/model.rs:164-180`); generic vol target could store Bachelier vols in a Black surface.

### Silent failure modes
- **[FIXED 2026-06-09]** OAS tree solver failure residual is `±1e6` keyed to `sign(oas)` (`bond/pricing/engine/tree/tree_pricer.rs:563-571`) — can hand Brent a fabricated bracket; the YTM/DM solvers fixed exactly this pattern.
- **[FIXED 2026-06-09]** MBS/CMO spread solvers return `0.0, converged:false`; CMO metric layer consumes it as a real spread (`mbs_passthrough/metrics/oas.rs:105-117`, `cmo/metrics/oas.rs:114-127`).
- **[FIXED 2026-06-09]** `solve_alpha_for_atm` returns unconverged alpha silently (`sabr/calibration.rs:695-696`) and the pinning objective skips ATM, so nothing catches it.
- **[FIXED 2026-06-09]** Implicit hazard-curve discovery by naming convention (`<discount_id>` / `<discount_id>-CREDIT`) silently switches a bond to credit-risky pricing with no instrument opt-in (`bond/.../hazard.rs:85-105`, `tree_pricer.rs:202-213`).
- **[FIXED 2026-06-09]** BDT default vol = 1% interpreted lognormal (`engine/tree/config.rs:266-301`) — near-zero optionality, OAS ≈ Z-spread, silently.
- **[FIXED 2026-06-09]** `AgencyCmo.collateral` is `#[serde(skip)]` (`cmo/types.rs:256-258`) — deserialized deals silently price on substituted synthetic collateral; violates the stable-wire-format invariant.
- **[FIXED 2026-06-09]** Tranche stochastic-recovery overrides not EL-consistent with the bootstrapped index curve (`cds_tranche/pricer/expected_loss.rs:339-411`) — 0–100% tranche sum ≠ index; renormalize or document loudly.

### Stochastic engines
- **[FIXED 2026-06-09]** Structured-credit Tree mode (the default) exhausts base-`branch_count` digits, leaving a deterministic z ≈ −0.97 shock on trailing months (`structured_credit/pricing/stochastic/pricer/engine.rs:459-489, 714-729`).
- **[FIXED 2026-06-09]** Recombining `ScenarioTree` factors don't diffuse with lattice position (`stochastic/tree/tree.rs:265-300`); period-N factor distribution equals period-1.
- **[FIXED 2026-06-09]** CLO in-period diversion uses stale balances → possible principal over-payment / negative tranche balance accruing negative interest (`structured_credit/pricing/waterfall.rs:222-247` + `simulation_engine.rs:2185-2192`).
- **[FIXED 2026-06-09]** Asian MC control variate biased on non-flat curves (drift schedule vs constant-drift analytic control) — `exotics/asian_option/pricer.rs:398-406` vs `:213-273`. Analytical Asian pricers also assume equal fixing spacing (`:1063-1067, 1267-1275`).
- **[FIXED 2026-06-09]** PAC schedule mis-anchored for seasoned collateral (ramp starts at age 0; `cmo/tranches/pac_support.rs:105-145`).
- **[FIXED 2026-06-09]** Term-loan tree lacks the DF timing correction for distributed cashflows (`term_loan/pricing/tree_engine.rs:152-200`) that the bond valuator has. Golden: `term_loan_b_5y_floating` cs01/discount_margin/ytm regenerated.

### Other moderates
- **[FIXED 2026-06-09]** CMS has no negative-rate model — hard-errors on F≤0 (fail-loud; swaption/cap-floor have Bachelier/shifted fallbacks).
- **[FIXED 2026-06-09]** SABR ρ≈1 fallback `χ ≈ z/(1+z/2)` is wrong (true limit `−ln(1−z)`; 10–70% vol error in branch, reachable only at |1−ρ|<1e-10 via direct `SABRParameters::new(…, 1.0)`) — `sabr/model.rs:309-312`.
- **[FIXED 2026-06-09]** SABR Obloj attribution inverted; genuinely-Obloj z computed then discarded (dead code) — `sabr/model.rs:91-132`.
- **[FIXED 2026-06-09]** WAL fallback path counts writedowns as principal, primary path doesn't (`structured_credit/metrics/pricing/wal.rs:22-47` vs `:88-99`).
- **[FIXED 2026-06-09]** `calculate_tranche_duration` computes Macaulay but is named modified (`structured_credit/metrics/risk/duration.rs:188-215`).
- **[FIXED 2026-06-09]** CTD selection labels gross basis "net basis"; no implied-repo-ranked CTD (`bond_future/types.rs:747-869`). Conversion factor itself verified against all five CME IR232 examples.
- **[FIXED 2026-06-09]** Same-day cashflow inclusion inconsistent across bond engines (discount/hazard include `as_of` flows; tree and YTM exclude). Unified on strict post-as_of exclusion (settlement convention). Golden: `bhccn_10_2032_callable_bloomberg` OAS tolerance widened to 3bp (documented convention residual vs the Bloomberg screen, combined with the OAS-solver and hazard-opt-in fixes).
- **[FIXED 2026-06-09]** FxSpot `spot_rate` bypasses validation via builder/serde; settlement cashflow undiscounted (`fx/fx_spot/types.rs:369-382, 464-474`).
- **[FIXED 2026-06-09]** Python direct instrument wrappers never release the GIL (`finstack-py/src/bindings/valuations/direct_wrapper.rs:54-122`; `pricing.rs`/`calibration.rs`/`fourier.rs` all detach correctly).
- **[FIXED 2026-06-09]** Python `fx`/`exotics` `price_with_metrics` hardcodes `market_history = None` (hvar/ES unreachable; WASM exposes it) — `direct_wrapper.rs:74-94`.
- **[FIXED 2026-06-09]** `barrier_call` returns silent NaN on invalid inputs in both bindings (`finstack-py/.../analytic.rs:253-265`, `finstack-wasm/.../analytic.rs:196-207`); siblings use `checked_closed_form_value`.
- **[FIXED 2026-06-09]** `merton_jump_cos_price` runtime keyword is `lambda` (Python reserved word); stub declares `lambda_` (`finstack-py/.../fourier.rs:180,191` vs `.pyi:1145`).
- **[FIXED 2026-06-09]** CDS-family Python wrappers hardcode model strings with no override (`"hazard_rate"` currently matches defaults; the pattern is what broke B5). Use `"default"`.

---

## Minors

- **[FIXED 2026-06-09]** (was already fixed alongside B4) SDA doc comment mislabels the implemented curve as the standard (entangled with B4).
- **[FIXED 2026-06-09]** SABR χ(z) Taylor coefficients c3/c4 wrong (`sabr/model.rs:290-291`; true c3 = (3ρ²−1)/6); negligible today (series governs |z|<1e-5) but fix before widening the region.
- **[FIXED 2026-06-09]** β=0 initial alpha guess scaled by F (`calibration.rs:280-284`); use `alpha₀ = atm_vol` for β≈0.
- **[FIXED 2026-06-09]** SABR auto-shift magnitude ad hoc (`calibration.rs:129`); market practice uses standardized per-currency shifts.
- **[FIXED 2026-06-09]** CDS protection integrates from `as_of`, not step-in T+1 (documented in `cds/README.md:133`; ~$5/$10M/day for 100bp name). Step-in is now convention-driven: T+1 for the ISDA Standard Model convention, spot for QuantLib parity (QuantLib 1.42.1's `IsdaCdsEngine` integrates from the valuation date — pinned by the `cds_quantlib_flat_hazard_decomposition` golden) and Bloomberg CDSW/CDSO conventions.
- **[FIXED 2026-06-09]** Index vs single-name Recovery01 methodology differs 2–5× (frozen-curve vs re-bootstrap), documented but worth unifying.
- **[FIXED 2026-06-09]** Tranche `attach_pct`/`detach_pct` percent-vs-fraction only warns (`cds_tranche/types.rs:225-230`); consider hard error. Now a hard validation error.
- **[FIXED 2026-06-09]** Convexity docstring omits the /100 scaling (`bond/metrics/convexity.rs:13-16`; value verified against the Bloomberg golden).
- **[FIXED 2026-06-09]** `fi_trs/pricer.rs:161-164`: `tracing::warn!` on every valuation call — log spam. Now warns once per process via `std::sync::Once`.
- **[FIXED 2026-06-09]** `tba/allocation.rs:197` and `mbs_passthrough/prepayment.rs:187-198`: panic paths on embedded data in valuation code. Now propagate `Result`.
- **[FIXED 2026-06-09]** `FxOption::default_model` returns `Black76` while pricing is Garman-Kohlhagen spot-form (`fx_option/types.rs:441-443`) — metadata mislabel. Key kept for wire stability; docs now state pricing is GK spot-form.
- **[FIXED 2026-06-09]** Expired-option conventions inconsistent across the FX family (quanto returns 0 at t≤0; FxOption/digitals/barriers return intrinsic at expiry). Quanto now returns quanto-adjusted intrinsic at t≤0.
- **[FIXED 2026-06-09]** Richard-Roll seasonality keyed to `seasoning % 12` not calendar month (latent; amplitude defaults to 0); `expected_smm` omits refi multiplier and Jensen term. Seasonality now keys to calendar month via optional origination month; `expected_smm` includes refi multiplier and Jensen term.
- **[FIXED 2026-06-09]** `CloWalCalculator` returns pool WAM labeled WAL (exported helper, not the registered metric). Now delegates to the principal-weighted WAL calculator.
- **[FIXED 2026-06-09]** Native `__all__` in `finstack-py/.../valuations/mod.rs:161-198` omits five registered names; masked by the Python shim.
- **[FIXED 2026-06-09]** COS `n_terms` stub drift (`.pyi` says `int = 128`; runtime `Option[int] = None`).
- **[FIXED 2026-06-09]** `CreditState` parameter order diverges Python vs WASM (positional-porting hazard). WASM aligned to Rust/Python canonical order (breaking).
- **[FIXED 2026-06-09]** Stale d.ts header (`index.d.ts:561` labels factor-model exports `valuations.creditFactorHierarchy`).
- **[FIXED 2026-06-09]** `theta_days=0` returns silent `inf` theta in release wheels (Rust guard is `debug_assert!` only). Now a hard `assert!` plus input validation in both bindings.
- **[FIXED 2026-06-09]** (was already fixed) Stale doc: `pricer/credit.rs:8` still says "Black76 for CDSOption".
- **[FIXED 2026-06-09]** Mixed key casing in WASM `arbitrageDiagnostics` (camelCase top-level, snake_case violations), pinned by d.ts. Violation keys now camelCase (breaking).
- **[FIXED 2026-06-09]** `cmo/types.rs:166-168`: `factor()` divides by `original_face` without zero guard; `per_name.rs:116-124`: copula build failure silently falls back to Gaussian. Factor returns 0.0 for zero face; copula build errors propagate.

---

## Verified-clean (recorded so it isn't re-reviewed)

- **Closed forms:** Black-Scholes/Black-76/Bachelier d1/d2, put-call parity, Greek scaling, σ→0/T→0 limits; `implied_vol.rs` (arbitrage bounds, bracketed Newton + bisection fallback, explicit non-convergence Err) — exemplary.
- **Barrier closed form:** Reiner-Rubinstein/Haug A–D decomposition verified term-by-term (all 8 in/out × call/put × K-vs-H regimes); first-passage touch probabilities match Shreve Thm 7.2.1; in–out parity tested; BGK shift direction + W-09 clamp correct.
- **Garman-Kohlhagen:** rate roles, premium currency, spot/forward/premium-adjusted deltas (verified against Wystup), DNS strikes exact.
- **CDS single-name:** ISDA piecewise-constant analytic integration matches Bloomberg DOCS 2057273 §3; AoD integral re-derived; Bloomberg CDSW parity goldens; CDSO Bloomberg-quadrature design reconciles a CDX IG 46 screen golden to <$1.
- **IRS/OIS engine:** lockout window, seasoned-fixing enforcement, spread non-compounded, telescoping 1/DF fast-path gating; FRA discounted-at-fixing settlement.
- **CMS Hagan + replication:** convexity formula dimensionally correct; Carr-Madan IBP terms check out; ATM-vol usage consistent.
- **Bermudan LSMC:** pathwise bank-account discounting, as_of-rebased curves, calibration guards.
- **Curve bootstrap:** pillar dates from conventions (not today+tenor), hard failure on no-bracket, f-space tolerance enforcement, order-invariance/determinism tests; residuals are true instrument repricings per notional.
- **FD sensitivities:** central differences with matching effective bumps, CRN seeding for MC Greeks, vol-clamp detection with one-sided fallback, Neumaier summation, per-bp units consistent.
- **Pricer/registry/JSON:** explicit ISO `as_of` (no wall clock in valuation logic), strict metric parsing, duplicate-registration guard, structured errors, order-preserving rayon batch; conventions loaders use `deny_unknown_fields` + schema gating.
- **Variance replication (Carr-Madan):** OTM split at K₀, exact Demeterfi anchor, wing extensions, Neumaier summation; units all standard.
- **Bindings:** all closed-form argument orders Python/WASM→Rust verified correct; JSON pricing delegates to shared `pricer::json` serde path in both hosts; no `unwrap()` on user input in binding code; WASM exotics-class absence documented in `parity_contract.toml`.

---

## Open Questions / Assumptions

1. **Seasoned-trade support** is the biggest thematic gap (CMS, autocallable, cliquet, equity TRS). If "new trades only" with fixings handled upstream is the intended workflow, several majors downgrade — but nothing in the instruments supports injecting fixings, and cap/floor + FRA set the precedent that fixings belong in the pricer.
2. **Curve base = as_of**: several findings (tranche discounting, equity axis-based lookups, inflation swap) are zero-impact when curves are rebuilt at every as_of. Nothing enforces that invariant.
3. Dollar-roll and call-window behaviors are pinned by tests and may be deliberate simplifications — but each contradicts its own module documentation.
4. Reviews were read-only; no test executions. Numerical claims verified by derivation/recomputation.

## Residual Risk / Not Reviewed

- Rates: `repo/`, `swaption` LMM/Cheyette-rough pricers, swaption/IRS/inflation metrics submodules, `exotics_shared` HW1F calibration/LSMC internals.
- Credit: `correlation/copula/multi_factor.rs`, `random_factor_loading.rs`, `models/credit/merton.rs`, tranche `recovery01`/`tail_dependence` internals, `calibration/solver/bootstrap.rs` internals.
- Fixed income: `bond/pricing/engine/merton_mc.rs`, revolving-credit MC internals, core schedule-generation internals.
- Securitized: `stochastic/default/{copula_based,intensity_process,factor_correlated}.rs`, `stochastic/metrics` internals, `pricing/diversion.rs`, pool metrics (`warf`/`was`).
- FX/EQ/commodity: commodity swap/swaption/asian, vol_index futures/options, Heston/rough-Bergomi/rough-Heston MC pricers, dcf_equity, pe_fund, real_estate, vanna-volga internals, per-instrument metrics submodules.
- Models: `calibration/hull_white.rs` (102KB), short-rate tree internals, Heston COS beyond outline.
- Bindings: `valuations/correlation` bindings (both hosts), envelope TypedDict internals.

## Recommended Regression Additions (highest leverage)

1. Futures-bearing-curve parallel-bump test asserting the bumped curve's forward moves +1bp (catches Major 25).
2. Seasoned-trade test per exotic: past fixing required → error (catches Majors 3, 19, 20, 21).
3. `strike_from_delta` round-trip at Δ≠0.5 (Major 24).
4. SABR normal-convention *unweighted* wing-repricing assertion (Major 23); `calibrate` vs `calibrate_with_derivatives` parameter agreement on a skewed smile (Major 26).
5. CMO principal-conservation invariant (Σ tranche principal = collateral principal) across prepay scenarios (Majors 16, 17).
6. Callable bond valued one day before a coupon vs analytic bullet bound (Blocker B2).
7. XCCY MtM resetting notional vs hand-computed CIP forward (Blocker B1).
8. Python `CDSOption.example().price(...)` smoke test (Blocker B5); Node facade test asserting plain-object property access on `FxOption.greeks()` (Major 27).
9. Dollar-roll: assert specialness increases with drop (Blocker B3).
10. SDA curve golden: CDR(month 30) = 0.60% × multiplier (Blocker B4).
