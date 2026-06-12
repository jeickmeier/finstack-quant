# Quant Finance Review — `finstack/margin` Crate and Bindings

**Date:** 2026-06-12
**Scope:** `finstack/margin` (~21k lines: SIMM IM, schedule/clearing/haircut/internal IM, VM, CSA/collateral/repo types, SA-CCR, FRTB SBA, XVA, metrics, parameter registry), `finstack-py/src/bindings/margin/`, `finstack-wasm/src/api/margin/`, `parity_contract.toml`.
**Method:** Seven parallel subsystem reviews (SIMM, VM/schedule-IM/clearing/haircuts, SA-CCR, FRTB, XVA, domain types/metrics, bindings parity). SIMM parameters were verified against the official ISDA SIMM v2.6 methodology PDF; FRTB against BCBS MAR21/MAR22/MAR23 (d457); SA-CCR against BCBS CRE52/279. All Blocker-level findings were independently re-verified against source before inclusion.

**Headline:** the margin crate's structural skeletons (aggregation shapes, EAD formula, CVA discretization, CSA formula plumbing) are largely correct, but the **embedded regulatory parameter tables are substantially not the published calibrations** — the SIMM v2.6 data file disagrees with the ISDA PDF on the risk-class correlation matrix (14/15 entries), IR risk weights (10/12 tenors), and the IR tenor-correlation matrix; FRTB has wrong commodity/equity/CSR risk-weight tables and a wrong low-correlation-scenario formula. The parity test suite pins the wrong values with false ISDA citations, so nothing catches it. Separately there is a runaway-margin-call ledger bug in VM, a maturity-blind collateral haircut lookup, and an XVA engine that is compiled out of production builds while its config types are exposed through bindings.

---

## Findings

### Blockers — wrong margin/capital numbers or broken market standard

**B1. SIMM risk-class correlation matrix ψ is wrong (14 of 15 off-diagonals).**
`finstack/margin/data/margin/simm.v1.json:46-62` + `calculators/im/simm.rs:1261-1276`. Embedded `{IR-CQ 0.10, IR-CNQ 0.14, IR-EQ 0.12, IR-CM 0.30, IR-FX 0.10, CQ-CNQ 0.60, CQ-EQ 0.66, CQ-CM 0.25, CQ-FX 0.22, CNQ-EQ 0.52, CNQ-CM 0.27, EQ-CM 0.33, EQ-FX 0.24, CM-FX 0.23}` vs ISDA v2.6 ¶88 `{0.04, 0.04, 0.07, 0.37, 0.14, 0.54, 0.70, 0.27, 0.37, 0.46, 0.24, 0.35, 0.39, 0.35}` (only CNQ-FX 0.15 matches). Every multi-risk-class IM is wrong. **Fix:** replace the `risk_class_correlations` block (v2_6 and v2_5 entries) with the published tables.

**B2. SIMM IR delta risk weights wrong for 10 of 12 tenors; no low/high-vol currency tables.**
`simm.v1.json:9-22` (and the v2_5 entry at 233-246). Embedded `3m 80, 6m 67, 1y 61, 2y 52, 3y 49, 5y 51, 10y 51, 15y 51, 20y 54, 30y 62` vs ISDA v2.6 Table 1 (regular vol) `90, 71, 66, 66, 64, 60, 60, 61, 61, 67`. Only 2w 109 and 1m 105 match. Low-vol (JPY) and high-vol currency tables are absent entirely. **Fix:** transcribe Tables 1–3 and key by currency volatility group.

**B3. SIMM IR inter-tenor correlation matrix is synthetic, not the ISDA calibration.**
`simm.v1.json:103-115`. Values are a smooth 0.99→0.51 decay; ISDA v2.6 §D.2 publishes e.g. (2w,1m)=0.77, (2w,3m)=0.67, (2w,1y)=0.48, (2w,30y)=0.20, (2y,10y)=0.80 vs embedded 0.99/0.97/0.88/0.51/0.88. Curve-spread positions get far too much offset. The parity test cites "Table 4" while asserting the synthetic values (see M7). **Fix:** transcribe the §D.2 matrix.

**B4. SIMM concentration ratio computed on risk-weighted sensitivities against raw-sensitivity thresholds.**
`simm.rs:549-556, 704-705, 784-786`. ISDA ¶7(b)/¶8(b): `CR = max(1, sqrt(|Σ s_k| / T_b))` over **raw** PV01/CS01. Code feeds `ws = s × RW` (×51–109 for IR, ×48–343 CQ, ×8.4 FX) into the same thresholds, so CR triggers ~2 orders of magnitude too early (e.g. $50k/bp CS01 in CQ bucket 3 → CF≈5.0 where ISDA gives 1.0). Legacy paths at `simm.rs:1201-1220` use raw sums against the same thresholds — the two paths are mutually inconsistent. **Fix:** compute CR from unweighted sensitivity sums everywhere.

**B5. SIMM IR inter-currency aggregation uses `γ·K_b·K_c` instead of `γ·g_bc·S_b·S_c`.**
`simm.rs:572-580`. K is always positive, so a long-USD/short-EUR book gets a positive cross term where ISDA ¶7(d) gives a negative one (S_b = max(min(ΣWS, K_b), −K_b)); the `g_bc = min(CR)/max(CR)` factor is absent. Systematic IM overstatement for cross-currency-hedged rate books. **Fix:** track signed S per currency (net_ws already computed) and per-currency CR.

**B6. SIMM IR vega: cross-currency vega silently dropped via HashMap key collision.**
`simm.rs:1073-1079`. `(currency, tenor)` keys are collapsed to tenor-only via `collect::<HashMap>`, which **overwrites** duplicates — with USD 5y and EUR 5y vega one is dropped, and which one depends on map order (also a determinism violation). **Fix:** aggregate per currency like delta.

**B7. SIMM non-IR risk classes structurally off-spec.**
`simm.v1.json:43-45, 63-82` + `simm.rs:757-772, 882-904, 1100-1175`. Equity: flat RW 32.0 with full netting across all underliers (long/short equity books margin to ~zero) vs ISDA §G's 12 buckets (RW 19–50) with intra/inter correlations. CreditNonQ: flat 500 pooled scalar vs 280/1,300/1,300 with ρ 83%/32%, γ 43%. Commodity bucket weights mostly wrong (embedded 25,21,23,… vs ISDA 48,29,33,…) and intra-bucket ρ (83–98%) absent. FX: flat 8.4 vs the §I vol-group matrix 7.4/14.7/21.4 with high-vol correlation overrides. **Fix:** add bucketed equity/CNQ paths mirroring `calculate_credit_delta_bucketed`; correct the weight tables.

**B8. SIMM vega margin off-spec in parameters and formula.**
`simm.v1.json:116, 214-217` + `simm.rs:844-904`. VRWs: embedded IR 0.21/CNQ 0.27/EQ 0.26/FX 0.30/CM 0.36 vs ISDA v2.6 IR 0.23, credit 0.76, EQ 0.45 (0.96 b12), FX 0.48, CM 0.55 (only CQ 0.76 correct). Missing σ_kj scaling (¶10(b)), missing HVR (IR 0.47, EQ 0.60, CM 0.74, FX 0.57), missing vega concentration thresholds, and vega is netted to one scalar per risk class with no bucket/correlation aggregation; `fx_vega.values().sum()` nets across currency pairs. **Fix:** implement ¶10 in full with the published VRW/HVR tables.

**B9. SIMM delta concentration thresholds wrong (flat pool values vs per-bucket).**
`simm.v1.json:219-226`. IR 230mm flat vs 330/130/61/30 mm/bp by currency group; EQ 3.3mm flat vs 3–810 mm by bucket; CM 3.5mm flat vs 310–4,000 mm; FX 8.4mm vs 3,300/880/170 mm. (CQ per-bucket thresholds at json:199-213 are correct.) Compounds B4. **Fix:** transcribe §J.

**B10. VM ledger sign bug — runaway margin calls when the book is out of the money.**
`calculators/vm.rs:265-290` (verified by direct trace through `thresholds.rs:124-219`). For exposure −2M with `current_collateral` 0, `calculate` correctly returns delivery 2M (we post), but `generate_margin_calls` then does `current_collateral += delivery`. Since `calculate_margin_call` computes `required − current_collateral` where collateral means *counterparty-posted*, the ledger now reads +2M; the next identical exposure yields `−2M − 2M = −4M` → delivery 4M, then 8M — each repeat doubles the demand. The `.max(0.0)` clamp at line 287 also silently truncates the ledger on over-returns. **Fix:** signed ledger (margin we post = negative balance) or per-direction tracking.

**B11. Collateral haircut lookup ignores maturity; BCBS grid mis-specified.**
`data/margin/collateral_schedules.v1.json:46-48` + `types/collateral.rs:415-421` + `calculators/im/haircut.rs:133-153`. The 0.5% government-bond bucket is keyed `max_remaining_years: 10.0` (BCBS: ≤1y; then 2% 1–5y, 4% >5y), and `haircut_for` is asset-class-keyed first-match — `MaturityConstraints::is_satisfied` has no callers — so a 30y Treasury gets a 0.5% haircut (8x understated). **Fix:** correct the boundary to 1.0 and add `haircut_for(asset_class, remaining_years)` filtering on maturity constraints; thread maturity through `HaircutImCalculator`.

**B12. FRTB low-correlation scenario missing the 0.75ρ floor.**
`regulatory/frtb/types.rs:98` (verified directly): `Self::Low => f64::max(2.0 * rho - 1.0, -1.0)`. MAR21.6(3): `ρ_low = max(2ρ − 1, 0.75ρ)`. ρ=0.5 → code 0.0 vs Basel 0.375; ρ=0 (xccy basis) → code **−1.0** vs Basel 0, creating fictitious offsets. The low scenario is often the binding one for mixed-sign books, so total capital is wrong. Tests at `engine.rs:599, 606-608` encode the bug. Note `params/registry.rs:142-150`'s linear `a·ρ + b` form cannot represent the max-of-two-terms. **Fix:** `f64::max(2.0 * rho - 1.0, 0.75 * rho)`; update tests and the registry scenario model.

**B13. FRTB vega risk weights missing liquidity-horizon scaling.**
`params/girr.rs:34, csr.rs:58, commodity.rs:38, fx.rs:7, equity.rs:36` used raw in `vega.rs:63,124,162,184,210`. MAR21.92: `RW = min(0.55·√(LH/10), 1.0)` → GIRR (LH 60) = 1.00, CSR (120) = 1.00, CM (120) = 1.00, FX (40) = 1.00, EQ large-cap (20) = 0.78, EQ small-cap (60) = 1.00. Code applies raw 0.55 (≈45% capital understatement) and 0.78 for all equity buckets. `csr.rs:55-57` even documents that callers should scale — none do. **Fix:** bake LH-scaled values into the constants; bucket-dependent equity vega RW.

---

### Majors

#### SIMM

**M1.** Curvature computed once across risk classes with ψ² and added **outside** the ψ aggregation, with a flat non-ISDA `curvature_scale_factor: 1.5` instead of the expiry-dependent `SF(t) = 0.5·min(1, 14d/t)` — `simm.rs:936-989, 1248-1253`. ISDA ¶11 computes curvature per risk class/bucket (ρ², γ², per-class θ/λ, IR scaled by HVR⁻²) inside `IM_r`. (The λ/θ/floor shape itself is correct.)

**M2.** No product-class separation: ISDA ¶5-6 requires SIMM = SIMM_RatesFX + SIMM_Credit + SIMM_Equity + SIMM_Commodity, each ψ-aggregated separately. The single pooled aggregation grants prohibited cross-product diversification (understates IM) — `simm.rs` whole calculator.

**M3.** IR risk-factor dimension incomplete: no sub-curve index (¶14, φ=99.3% in v2.6), no inflation factor (RW 61, ρ 24%), no cross-currency basis factor (RW 21, ρ 4%, CR-exempt). `ir_inter_currency_correlation` defaults to 0.27 (`registry/mod.rs:837`) vs v2.6 γ=0.32 — `simm_types.rs:168` + `simm.rs:496-616`.

**M4.** CQ intra-bucket correlation ignores issuer/tenor identity: flat ρ=0.46 for all pairs vs ISDA ¶42 0.93 same-issuer/different-vertex (understates single-issuer term-structure positions), residual bucket should use 0.50; per-issuer CR with `f_kl` is degenerate (≡1) — `simm.rs:691-719`.

**M5.** No calculation-currency handling for FX delta: ISDA ¶19/¶28/¶69 require the calc currency's FX risk weight be zero; `SimmSensitivities.base_currency` is never consulted — `simm.rs:780-806, 1159-1165`.

**M6.** Unknown tenor/bucket labels silently dropped (`filter_map` + `?`): a "7y" IR key or unrecognized commodity label contributes zero margin with no error — silent under-margining on producer typos — `simm.rs:547-553, 596-604, 818-825, 855-864`.

**M7.** The "golden" parity test pins the wrong values with false ISDA citations: `tests/simm_schedule_parity.rs:53-126` claims "ISDA SIMM v2.6 — Section E.1, Table 1" while asserting 3m=80 (Table 1: 90) and "Table 4" while asserting (2w,1m)=0.99 (actual 0.77). It is a drift guard against the same wrong JSON. No end-to-end test against ISDA's published unit-test portfolios exists.

#### VM / Schedule IM / haircut calculators

**M8.** Delivery and return both rounded to **nearest** (`thresholds.rs:221-229`); ISDA CSA standard election is delivery rounded UP, return rounded DOWN, and direction is not configurable. Receiver under-collateralized by up to half the rounding lot.

**M9.** NGR formula deviates from BCBS/CEM: code uses `|Σ MtM| / Σ |MtM|` (`im/schedule.rs:452-468`); standard is `max(0, Σ MtM) / Σ max(0, MtM_i)`. Mixed books understated ~14% in the worked example; all-negative books get NGR=1. Test at `schedule.rs:718-745` codifies the wrong value.

**M10.** `margin_call_dates` infinite loop for `MarginTenor::OnDemand` when adjusted end == adjusted start — `vm.rs:298-321`. Hang + unbounded Vec growth.

**M11.** Haircut/repo IM uses `|net MtM|` as the collateral value (`im/haircut.rs:163-164`, dispatched from `metrics/instrument.rs:139-144`): a repo's net MtM ≈ 0 at inception so IM ≈ 0 exactly when haircut protection matters. The `im_exposure_base` fail-closed mechanism exists for the other calculators but is not wired into the Haircut path.

#### SA-CCR

**M12.** No as-of/valuation date anywhere in the API: remaining maturity computed as `end_date − start_date` (`add_on.rs:117`, `engine.rs:83`), so a 10y swap traded 6y ago is bucketed/SD'd as a 10y trade (SD overstated ~2.3x, wrong bucket); forward starts unrepresentable (`supervisory_duration` always called with S=0). **Fix:** add `as_of` to the config and compute S/E from it.

**M13.** FX add-on nets across currency-pair hedging sets: `hedging_set_values.iter().sum() * rho` with ρ=1 yields `SF×|Σ_HS EN|` (`add_on.rs:210-217`); CRE52.58 requires `Σ_HS SF×|EN_HS|`. Long EURUSD vs short GBPUSD nets to zero add-on — EAD understatement.

**M14.** Flat 5% supervisory factor for all credit (`params.rs:9`) vs the rating-graded BCBS table (AAA 0.38% … CCC 6%); IG single names overstated ~10x, CCC understated. `SaCcrTrade` has no rating/index attribute, so unimplementable without a model change.

**M15.** Equity: no index SF (20%) and ρ=0.8 applied to single names (should be 0.5; index 0.8) — `params.rs:10,19`. Indices overstated 60% on SF; single-name cross-name offsets wrong.

#### FRTB

**M16.** Equity delta RW buckets 11–13 wrong/swapped: code `(11, 15.0), (12, 70.0), (13, 70.0)` vs MAR21.77 `11 (other) 70%, 12 (large-cap indices) 15%, 13 (other indices) 25%`; header comment inverts Basel's bucketing — `params/equity.rs:24-26`.

**M17.** Commodity delta RW table is not the Basel table (resembles SIMM values): code `(1,19)(2,20)(3,17)(4,16)…` vs MAR21.82 `30/35/60/80/40/45/20/35/25/35/50`. E.g. freight 16 vs 80 — `params/commodity.rs:17-29`.

**M18.** CSR non-sec RWs buckets 8–15 shifted by one (transcription error): bucket 8 (covered bonds) should be 1.0%, 9–15 are the values keyed one bucket too low — `params/csr.rs:30-38`. Buckets 1–7, 16–18 correct.

**M19.** Equity correlations uniform 0.15 intra and inter vs MAR21.78/21.80's bucket-dependent values (25% b5-8, 80% b12-13, bucket 11 undiversified Σ|WS|; inter 45%/75%/0% structure) — `params/equity.rs:30-33` + `delta.rs:246-250`.

**M20.** Commodity correlations flat 0.55/0.20 vs MAR21.83-84 per-bucket ρ_cty (15–95%) with tenor/basis factors, and γ=0 vs bucket 11 — `params/commodity.rs:32-35`.

**M21.** CSR non-sec inter-bucket γ flat 0.40 vs the MAR21.55 rating×sector matrix; bucket 16 should aggregate with γ=0 — `params/csr.rs:50`.

**M22.** Curvature: intra-bucket correlation not squared (`rho * c_i * c_j` — should be ρ²), and S_b clamped to ±K_b, which is the delta/vega alternative — MAR21.5(3) curvature S_b is the uncapped CVR sum. Test `curvature.rs:308-320` encodes the deviation — `curvature.rs:217-233, 244-257`.

**M23.** DRC: subordinated LGD 0.75 (MAR22.10: non-senior = 100%); netting per `(sector, issuer)` ignores seniority ordering (MAR22.15); obligor rating taken from the first position seen (`or_insert`), silently misweighting mixed-rating issuers — `drc.rs:28-33, 84-104`.

**M24.** `FrtbSbaEngineBuilder::params(...)` is a silent no-op for calculations: charge helpers read the `pub const` tables directly, while the result is audit-stamped with the overlay revision — worse than not having the feature — `engine.rs:157-162` + `params/registry.rs:11-13`.

#### Repo margining

**M25.** Repeated margin calls for the same deficit: `_current_collateral_posted` is updated but never read; deficit recomputed each date against raw external valuations only, so a persistent 2M shortfall emits a 2M `VariationMarginPay` **every day** — `types/repo_cashflows.rs:34-65`.

**M26.** `margin_call_threshold` (GMRA trigger) never applied in cashflow generation: calls fire on any `deficit > 0` — `repo_cashflows.rs:39-53` vs `repo_margin.rs:252-263`.

**M27.** Margin maintenance measures exposure against the static purchase price; GMRA 2011 Para 4 requires the Repurchase Price (purchase price + accrued price differential). A 3-month repo at 5% accrues ~1.25% unmargined — comparable to the entire 2% margin — `repo_margin.rs:244-246` + `repo_cashflows.rs:24-35`.

#### XVA / wire types / bindings

**M28.** The entire XVA engine (`cva`, `exposure`, `netting`, `traits` — ~3.5k lines incl. CVA/DVA/FVA integration and the stochastic exposure engine) is `#[cfg(test)]` — `xva/mod.rs:30-40` (verified). Only `types` is public. Doc comments in `cva.rs:344-346` tell users to call `compute_bilateral_xva`, which does not exist for them; the Python bindings expose `XvaConfig`/`XvaResult`/`CsaTerms`/`XvaNettingSet` that nothing can consume (see M34). The mod-level doc declares this intentional, but shipping 7 binding classes against a compiled-out engine is a product defect. **Fix:** re-publicize a supported engine (deterministic path at minimum) or trim the public/binding surface to serde containers and say so.

**M29.** `InstrumentMarginResult` JSON serialization fails at runtime when SIMM sensitivities are present: `sensitivities: Option<SimmSensitivities>` embeds tuple-keyed maps (`HashMap<(Currency, String), f64>` etc.), which serde_json rejects ("key must be a string") — `types/netting.rs:102-123` + `types/simm_types.rs:168-225`. The checked-in schema omits `SimmSensitivities`, masking it. **Fix:** string-encoded keys (`"USD|5y"`) or sorted `Vec<(K, f64)>`.

**M30.** No `#[serde(deny_unknown_fields)]` on any public inbound wire type (`CsaSpec`, `VmParameters`, `ImParameters`, `EligibleCollateralSchedule`, `MarginCall`, `OtcMarginSpec`, `RepoMarginSpec`, `NettingSetId`, `InstrumentMarginResult`; only `registry/wire.rs` complies) — violates the workspace strict-serde invariant; typo'd fields silently dropped, e.g. `fx_haircut_addon` defaulting to 0 — `types/csa.rs:98` et al. SA-CCR inbound types same issue (`sa_ccr/types.rs:69,104`).

**M31.** `saccr_ead` is a Python-only invented function with CSA policy hardcoded in the binding: `NettingSetId::bilateral("CPTY", "CSA")`, threshold/MTA/NICA fixed at 0, MPOR 10 — `finstack-py/src/bindings/margin/regulatory.rs:475-486`. Users with real CSA terms get a wrong EAD with no way to express them. `frtb_sba_charge` (regulatory.rs:406) similarly replaces the Rust engine API. Violates the canonical-API rule. **Fix:** bind `SaCcrEngine`/`SaCcrNettingSetConfig`/`FrtbSbaEngine` as classes matching Rust names.

**M32.** Margin section of `parity_contract.toml:203-211` is stale: all modules `status = "missing"` while 27 symbols are bound flat on `finstack.margin`; no `[crates.margin.symbols]`; phantom `config` module; the bound `regulatory` module absent — the contract test cannot catch margin drift.

**M33.** `ImResult` is bound but unconstructable: none of the five Rust IM calculators are bound, there is no `#[new]`/`from_json`, and the Rust `as_of` field is dropped — `finstack-py/src/bindings/margin/calculators.rs:122-174`. The "VM/IM calculators" module docstring is half false.

**M34.** Python XVA surface (7 classes) has no compute path (consequence of M28); `PyExposureProfile` hardcodes `diagnostics: None` with no getter, so the registered `ExposureDiagnostics` class has zero producers — `finstack-py/src/bindings/margin/xva.rs:219, 537-545`.

---

### Moderates

**CSA/VM semantics**
- MTA applied **after** rounding with a comment asserting that as the CSA convention; ISDA Para 3 applies MTA to the unrounded Delivery/Return Amount (test at `thresholds.rs:461` codifies it) — `thresholds.rs:211-216`.
- Single symmetric threshold/MTA/IA cannot represent per-party CSA elections; IA enters as a signed net (`signed_excess + ia`), so a positive IA *reduces* what we post when exposure is negative — `thresholds.rs:124-163`.
- `VmResult` doc sign convention inverted vs behavior ("positive = we need to post" but delivery>0 means counterparty posts for positive exposure) — `vm.rs:26-45`.
- `MarginCall.amount` documented "negative = return" but all producers emit positive amounts with direction in `call_type`; a doc-following producer double-negates — `types/call.rs:101` vs `vm.rs:276-287`, `repo_cashflows.rs:145`.
- `days_to_settle` returns calendar days, documented as business days — `call.rs:201-205`.

**IM calculators**
- Clearing IM is a flat `|exposure| × rate` proxy for every CCP (LCH 2%, CME 3%, ICE Credit 10%…) with no provenance notes; `GenericVaR { confidence, lookback_days }` are dead parameters used only in `Display` — `im/clearing.rs` + `data/margin/ccp_methodologies.v1.json`.
- `ImCalculator::calculate` trait path stamps `default_asset_class` (IR) + 5y maturity on every instrument — an equity derivative gets 4% instead of 15% — `im/schedule.rs:493-524`.
- Group-level IM threshold (€50M BCBS-IOSCO) subtracted **per instrument** (double-counted N times); MTA applied to the requirement level with `<=` (ISDA: transfer "equal to or exceeding" MTA must be made, and MTA applies to the transfer) — `metrics/instrument.rs:211-221`.
- No-CSA fallback reports full MTM as the VM "requirement", inflating `TotalMarginMetric` (`marginable_api.rs:101-103` enshrines it) — `metrics/instrument.rs:296-313`.
- Zero-IM fallback stamps `ImMethodology::Schedule` (never ran) and silently defaults the currency to USD on error — `metrics/instrument.rs:146-160`.

**SIMM secondary**
- Missing risk-class-pair correlation falls back to 1.0 silently (an empty overlay passes the PSD check and degrades aggregation to a plain sum) — `simm.rs:95-104, 976-984`.
- CQ residual bucket aggregated inside the sqrt with γ=0 instead of added arithmetically outside (¶8(d)) — understates — `simm.rs:733-745`.
- Parameter lookup by `.find(|p| p.version == version)` over a HashMap: duplicate-version registry entries → nondeterministic parameter selection — `simm.rs:192-197`.
- Currency safety is convention-only: bare `f64` sensitivities; `merge()` doesn't check `base_currency`; `calculate` stamps `mtm.currency()` without validating it equals `sensitivities.base_currency` — `simm_types.rs:362-386` + `simm.rs:1318-1321`.

**SA-CCR secondary**
- Commodity: no electricity 40% SF; ρ-offset spans *all* commodity hedging sets (BCBS: no offset across energy/metals/agri/other) — `params.rs:11` + `add_on.rs:183-218`.
- `SUPERVISORY_OPTION_VOLS` is dead code; no supervisory-delta (Black) helper, no λ shift for negative rates; `validate()` only checks sign/range so a numerically wrong δ passes — `params.rs:24-30`.
- MPOR floor documented but not enforced: `mpor_days: 0` → MF = 0 → margined add-on silently zero — `maturity_factor.rs:45-48` + `types.rs:139-156`.
- Basis (0.5×SF) and volatility (5×SF) transactions not modeled — `types.rs`.
- Netting-set config monetary fields (collateral/threshold/MTA/NICA) never validated: NaN collateral → multiplier silently 1.0, RC 0.0 — `pfe.rs:51`, `replacement_cost.rs:37`.

**FRTB secondary**
- DRC: no maturity scaling of JTD (MAR22.12-13; no maturity field) — sub-1y positions overcharged; DRC buckets too granular (six sectors vs Basel's corporates/sovereigns/munis three) — denies allowed hedging — `drc.rs:70-127` + `types.rs:122-135`.
- Vega correlations omit the option-maturity factor `e^(−0.01·|T_k−T_l|/min)` (MAR21.95); GIRR vega reuses the delta tenor ρ (θ=0.03, floor 0.40) where MAR21.94 prescribes α=1% no floor — `vega.rs:91, 121-216`.
- √2 RW divisors for liquid currencies/pairs (MAR21.43/21.87) not implemented even as an option — vendor parity will fail for majors.
- FX curvature collapses all pairs into one bucket with unsquared ρ=0.6 (Basel: per-pair buckets, γ²=0.36, per-pair ψ) — `curvature.rs:147-161`.
- CSR sec correlations: CTP should reuse non-sec ρ structure (35%/65%/99%), non-CTP intra 40% same-bucket-different-tranche and inter γ=0 — `params/csr.rs:126-135` + `delta.rs:205-212`.
- NaN charges silently dropped from the total (`if d > 0.0` is false for NaN) — `engine.rs:83-91`.
- Unknown buckets get silent mid-table RW fallbacks (`unwrap_or(5.0)` etc.) — `params/csr.rs:153-170`, `equity.rs:51`, `commodity.rs:50`, `drc.rs:164-169`.

**XVA secondary**
- First CVA bucket assumes EPE(0)=0 and profile validation rejects t=0, so the spot-MTM anchor cannot be supplied — systematic CVA understatement of ~½·EPE(0)·PD(0,t₁)·LGD — `cva.rs:100-142` + `types.rs:131-135, 362-364`.
- DVA + FBA double-count by default: `bilateral_cva = cva − dva + fva` with funding benefit defaulting to the full funding spread — `cva.rs:582-599` + `types.rs:29-48`.
- CVA `recovery_rate` parameter never checked against the hazard curve's embedded recovery (protection/LGD inconsistency) — `cva.rs:72-87`.
- Instrument valuation failures silently become zero exposure (only market-roll failures are ratio-gated): a broken pricer and a matured trade are indistinguishable; 100% failure → CVA = 0 — `exposure.rs:243-301`.
- Collateralized exposure caps at threshold (ignores the MTA residual; standard conservative form is `min(E, threshold + MTA)`), with a discontinuity at threshold+MTA; `mpor_days` stored but unused so zero-threshold CSAs show exactly zero exposure — `xva/netting.rs:105-113` + `types.rs:586-594`.
- No currency/FX-policy stamp in `ExposureProfile`/`XvaResult` envelopes (violates the policy-visibility invariant) — `types.rs:314-331`.

**Types/metrics/errors**
- `MarginUtilization`/`ExcessCollateral` subtract/divide raw amounts across two `Money` values with no currency-equality check — `metrics/mod.rs:35-98`.
- Collateral eligibility/concentration/maturity/rating constraints exist but are never enforced; `calculate_for_collateral` gives *ineligible* asset classes the BCBS default haircut instead of rejecting — `collateral.rs:435-461` + `im/haircut.rs:133-136`.
- `CollateralAssetClass` FromStr/Deserialize never rejects — typos silently become `Custom("goverment_bonds")` — `collateral.rs:119-141`.
- Repo cashflows: silent ACT/360 fallback when `year_fraction` errors (configured convention swapped, error swallowed; inverted dates → negative interest) — `repo_cashflows.rs:104-112`.
- No crate-specific `thiserror` Error enum (workspace standard); everything is stringly `core::Error::Validation` — `lib.rs`.
- Schema parity covers 2 enums out of 16 schema definitions; no regenerate-and-diff; repo/netting/SIMM types absent from the schema; no schema_version field — `tests/schema_parity.rs:34-70`.

**Bindings secondary**
- `FrtbSensitivities` Python binding renames `add_csr_nonsec_delta` → `add_csr_delta` and invents `add_rrao_position`; no way to add DRC positions / GIRR inflation / xccy basis / CSR vega except `from_json`, so `drc` is always 0.0 for add-built portfolios — `regulatory.rs:137-139, 235-241`.
- No behavioral/golden margin tests in either binding: Python has only a namespace import check (6 of 27 symbols); WASM `wasm_margin.rs` asserts JSON shape only. No BCBS d457 / BCBS 279 worked-example tests anywhere.
- Money currency and dates dropped at the boundary: `VmResult` getters return bare floats with no `currency`/`date`/`settlement_date`; metrics classes likewise (inconsistent with `ImResult`, which has `currency`) — `calculators.rs:21-49`, WASM `api/margin/mod.rs:75-82`.
- Nondeterministic dict/list ordering from HashMap iteration in `breakdown_keys()` and `frtb_sba_charge` breakdowns — `calculators.rs:157-159`, `regulatory.rs:421-437`.
- No `[wasm_margin_subset]` section in the parity contract despite a 27-symbol Python vs 4-function WASM gap; large unbound Rust surface (`Marginable`, `MarginCall`, `RepoMarginSpec`, SIMM types, metrics fns, regulatory types) neither bound nor recorded as exclusions; `CONSTANTS` dict is a Python-only invention — `parity_contract.toml`, `types.rs:596-601`.

---

### Minors

- SIMM: hard-coded registry fallback defaults duplicate (wrong) JSON values — `registry/mod.rs:766-779`; `commodity_bucket_weight` magic 64.0 fallback is dead/contradictory — `simm.rs:106-115`; v2_5 JSON entry shares the same suspect provenance — `simm.v1.json:229-340`.
- Schedule IM: NaN maturity silently lands in the Long bucket, negative in Short — `im/schedule.rs:180-193`; all-zero-MtM netting sets fall back to gross (factor 1.0) where NGR treatment is arguable — `schedule.rs:464-466`.
- Haircut IM: repo MPOR `2` hardcoded inline duplicating `defaults.v1.json` — `im/haircut.rs:185`; MPOR units inconsistently documented (calendar vs business days) — `im/clearing.rs:169` vs `thresholds.rs:284-287`.
- SA-CCR: monetary values raw f64 with `reporting_currency` dead; silent SF/ρ fallback `unwrap_or(0.05)` for future asset classes; misleading constant names (`IR_ADJACENT_BUCKET_CORR = 1.4` is 2ρ); result `maturity_factor` is an unweighted average mislabeled "applied"; FxHashMap iteration order differs between 64-bit native and 32-bit wasm (ULP-level sum differences, non-canonical JSON key order) — `engine.rs`, `params.rs:39-49`, `add_on.rs:41-42, 135, 204`.
- FRTB: `tenor_to_years(...).unwrap_or(5.0)` silent fallback — `delta.rs:113`, `vega.rs:66-67`; GIRR delta key can't represent multiple curves per currency (0.999 cross-curve factor unrepresentable) — `types.rs:248`; `reporting_currency` dead, no consistency check vs `base_currency` — `engine.rs:32-33`; scenario doc comment wrong even relative to code — `types.rs:69-71`.
- XVA: `bilateral_cva` field doc says "CVA − DVA", code computes `cva − dva + fva` — `types.rs:221-228`; `compute_fva` accepts unvalidated spreads, no profile-ordering validation in any `compute_*` entry point — `cva.rs:259-337, 508-522`; doctest examples use `crate::` paths that can never compile — `netting.rs:45-55`; `resolve_reporting_currency` fully prices an instrument to read its currency and propagates t=0 errors inconsistently — `exposure.rs:80-101`; quantile sort uses `partial_cmp...unwrap_or(Equal)` (use `total_cmp`) — `exposure.rs:133`; stale test comment on effective EPE — `cva.rs:831-835`.
- Types/metrics: `generate_margin_cashflows` takes both `cash_amount: Money` and a separate `currency` never cross-checked — `repo_cashflows.rs:24-29`; cleared CSA curve id `{ccy}-OIS` vs bilateral `EUR-ESTR` naming drift — `otc.rs:151` vs `csa.rs:158`; `mark_to_market()` sets `pays_margin_interest: true` with no rate so interest silently never accrues — `repo_margin.rs:180-192`; `Money * f64` can panic on non-finite caller rates — `metrics/mod.rs:146-167`; `Haircut01` ignores `current_haircut` and sign undocumented — `metrics/mod.rs:183-206`; negative `required` mislabels adequacy — `metrics/mod.rs:36-42`; total margin adds IM+VM without currency check, sensitivities recomputed with errors swallowed — `metrics/instrument.rs:350-371`; dual enum string vocabularies (serde PascalCase vs Display snake_case) and heterogeneous `ClearingStatus` JSON shape — `types/enums.rs`, `call.rs:53-82`; no margin-call status lifecycle (timing terms are dead data) — `call.rs:84-116`.
- Bindings: zero `text_signature` annotations across all six margin binding files; `__all__` unsorted with three inconsistent orderings (mod.rs / .pyi / `__init__.py`); bare `display_to_py` instead of `serde_json_to_py(err, context)` at JSON boundaries; `CollateralAssetClass` factory methods lack doc comments (runtime `__doc__` is None while the stub claims docstrings); `saccr_ead` docstring references nonexistent `SaCcrEngine.calculate_from_json`; `XvaConfig` accepts `funding` with no getter; stale `#[allow(dead_code)]` on `inner` fields.

---

## Open Questions / Assumptions

1. **Provenance of the embedded SIMM tables.** The v2_6-labeled values match neither v2.6 nor (for the spot-checked subset) v2.5. Were these placeholder calibrations? If ISDA-licensed parameter files cannot be embedded, the honest posture is to ship the engine with a documented "parameters required" registry and no default values, rather than wrong defaults labeled v2_6.
2. **Is the XVA `#[cfg(test)]` gating intentional product staging?** The mod doc says yes, but `cva.rs` public docs and the Python binding surface contradict it. A decision is needed: publish the deterministic engine, or trim the binding surface.
3. **VM rounding/MTA ordering**: if the current nearest-rounding + MTA-after-rounding behavior is a deliberate house convention, it should be documented as a deviation from the ISDA standard election and made configurable; tests currently enshrine it as "CSA convention", which is incorrect.
4. **FRTB CSR sec non-CTP 25-bucket RW table** (`params/csr.rs:97-123`) was not line-verified against MAR21.62 — needs a dedicated check during remediation.
5. **SIMM v2.5 entry** not verified against the ISDA v2.5 PDF (same suspected provenance as v2_6).

---

## Brief Summary

The crate's architecture is good: clean module separation, registry-driven parameters with PSD/symmetry/range validation at load, deterministic ordering discipline in most aggregation paths (canonical sorts before quadratic forms), Neumaier-compensated sums, currency-checked VM entry points, and a correct skeleton for the hard formulas (SIMM CQ inter-bucket S_b clamping, curvature λ/θ, SA-CCR EAD/RC/PFE-multiplier/maturity factors, MAR21.4(5)(b) alternative aggregation, DRC HBR shape, trapezoidal CVA with proper survival-difference PDs and netting-before-flooring).

What fails is the **calibration layer and the convention details**: the embedded SIMM and FRTB parameter tables are substantially not the published ones, several aggregation paths take structural shortcuts (flat correlations, pooled scalars, missing bucket dimensions), and the golden tests pin the wrong values with citations to documents they contradict — which is the single most dangerous pattern found, because it makes the suite assert compliance it doesn't have. Until B1–B9/B12–B13 and the parameter-table Majors are fixed, **no SIMM IM or FRTB capital number from this crate should be used against a real book**, and SA-CCR is reliable only for newly-traded, non-credit/equity, non-FX-hedged portfolios (M12–M15). VM is one ledger fix (B10) and two convention fixes (M8, MTA ordering) away from production-credible; the CSA/threshold formula core is sound.

Remediation priority: (1) re-transcribe all ISDA/BCBS parameter tables directly from the primary PDFs with a provenance README, (2) replace the self-referential parity tests with ISDA SIMM unit-test portfolios and BCBS d457/279 worked examples, (3) fix B10/B11 and the SIMM structural blockers (B4–B8), (4) decide the XVA publication question, (5) bring the bindings/parity contract back into truth.

---

## Quant Notes

- ISDA SIMM v2.6 Methodology (official PDF, isda.org) — the authoritative source for B1–B9; ISDA also publishes unit-test portfolios with expected IM that should become the golden suite.
- BCBS CRE52 (consolidated framework) / BCBS 279 incl. Annex 4a worked examples — golden targets for SA-CCR.
- BCBS MAR21/MAR22/MAR23 (d457, Jan 2019) — FRTB SBA; note MAR21.6(3) low-scenario formula and MAR21.92 vega LH scaling, both currently wrong.
- ISDA 1994/2016 Credit Support Annex Para 3 (transfer timing/MTA) and Para 13 elections (rounding: delivery up, return down).
- GMRA 2011 Para 4 (margin maintenance against Repurchase Price) for the repo findings.
- Gregory, *The xVA Challenge* (4th ed.) Ch. 7 (collateralized exposure: threshold+MTA cap, MPOR gap risk) and Ch. 18-19 (DVA/FVA overlap) for the XVA Moderates.
- BCBS-IOSCO *Margin requirements for non-centrally cleared derivatives* — schedule IM grid (verified correct) and the €50M group-level IM threshold (currently applied per instrument).
