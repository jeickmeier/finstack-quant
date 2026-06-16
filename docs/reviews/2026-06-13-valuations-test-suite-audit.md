# Valuations Crate — Test-Suite Audit (Duplicates / Dead Tests / Coverage Holes)

**Date:** 2026-06-13
**Scope:** `finstack-quant/valuations` test suite — ~7,000 test functions (4,175 in `tests/`, 2,854 unit tests in `src/`) across 580 files / ~171K lines, plus `src/` unit tests, wired into 14 test binaries (43 instrument families + 9 non-instrument test groups).
**Goal:** find (1) duplicate tests, (2) dead / unnecessary tests safe to remove, and (3) major coverage holes worth filling.

## Methodology

Multi-agent fan-out: one reviewer per instrument family / test group (53 slices) read the actual test bodies and flagged candidates, then an **adversarial verifier** re-read every "remove this" recommendation against the source to reject any that would destroy real coverage. Project conventions were encoded so they are **not** mis-flagged: `#[ignore = "slow: covered by mise rust-test-slow"]` and `#[ignore = "diagnostic"]` tests are intentional (slow lane / probes); DV01/CS01 dPV/dy sign conventions are correct-by-design; determinism / currency-safety / serde-stability tests are protected.

**Verifier rejected 95 of the proposed removals** — those are deliberately excluded below.

### Headline counts (post-verification, pre-consolidation)

| Category | Count |
|---|---|
| Duplicate groups to consolidate | 141 |
| Dead / unnecessary tests to remove | 136 |
| Coverage holes | 404 (77 major) |
| Removal candidates rejected by verifier | 95 |

Counts are upper bounds on *distinct findings*; some overlap across slices and should be consolidated when acted on.

---

## Verified quick wins (confirmed by direct inspection)

These four structural findings were independently re-verified by hand and are safe, high-value cleanups:

1. **Orphaned, never-compiled file** — `tests/instruments/revolving_credit/revolving_credit.rs` (10 tests) is **not declared** in `revolving_credit/mod.rs`, so it never compiles or runs. `basic.rs` covers all 10 (plus the unique CS01 z-spread fallback test). **Delete the file.** (Also: the stale `# TODO` block at the top of `revolving_credit/mod.rs` claims `construction.rs`/`pricing.rs`/`cashflows.rs`/`metrics/`/`validation/` "need API updates" — but they are declared and compiling. Remove the stale TODO.)
2. **Orphaned test module — ~27 tests silently not running** — `tests/instruments/common/pricer/` (incl. `registry.rs`, 1,037 lines) is **not declared** in `tests/instruments/common/mod.rs`. Adding `pub mod pricer;` activates the registry tests, including `test_price_batch_matches_serial_results` (a serial≡parallel guard). **Wire it up, then fix any tests that have bit-rotted.**
3. **Byte-identical duplicate** — `tests/instruments/term_loan/metrics/callability.rs` test bodies are identical to `integration.rs:52` / `:94`. **Delete the file and the `mod callability;` line**; coverage is preserved by `integration.rs` (which also has a unique floating-rate DM test).
4. **Dead `cfg(any())` block** — `tests/instruments/cms_option/vanna.rs:1` opens with `#![cfg(any())]`, so the whole module never compiles (predates the Decimal migration). The live `test_cms_option_vanna` covers it. **Delete lines 1–108.**

> Orphaned *files* produce no compiler error (they're outside the module tree), so `cargo` will not surface them. A deliberate mod-graph sweep is recommended as part of cleanup to find any others beyond the two confirmed here.

---

## Cross-cutting themes (recurring across many slices)

These patterns recur in nearly every bucket and are the best targets for a systematic pass:

- **`is_ok()` / `is_finite()`-only "smoke" tests** that duplicate a sibling which actually asserts the value. The weaker twin should be removed (or strengthened) — present in cds_tranche, cds_index, revolving_credit, ir_future, IRS bucketed-DV01, fx_swap, deposit, FRA, and more.
- **Tautological tests** — `assert_eq!` of a literal against itself, `matches!` enum-identity, getter round-trips on ids where `Id::new` does no validation, and `value()`-called-twice "trait consistency" checks. These assert nothing about library logic.
- **Registered-but-untested metrics** — a large class of `MetricId` calculators are wired into registries but never requested by any test: `Recovery01`, `Cs01Hazard`, `Dividend01` (credit); `AllInRate`, `EmbeddedOptionValue`, WAL, FRN z-spread, `CollateralHaircut01`/`CollateralPrice01` (fixed-income); `IrConvexity`/`IrCrossGamma`, `ConvexityAdjustment` (rates); FX delta/correlation, higher-order/cross/bucketed Greeks (equity-fx); `WeightRisk`, `NAV01`/`Carry01`/`Hurdle01` (exotics). **This is the single largest coverage gap.**
- **Missing serde `deny_unknown_fields` + round-trip tests** — the serde-stability invariant is asserted in the rules but has essentially no negative-test coverage across instruments (Swaption, ConvertibleBond, Deposit, FRA, CDS, CDSOption, all FX instruments, TermLoan, Repo, StructuredCredit, variance/autocallable, …).
- **Missing serial≡parallel determinism guards** — no cross-instrument test asserts bit-identical serial-vs-parallel Decimal `value()`, and `PricerRegistry::price_batch` == serial is only in the (orphaned) pricer module.
- **Alt-model pricers with zero integration tests** — Heston / rough-Heston / PDE MC (equity & barrier options), commodity Asian/spread/swaption (inline-only), HW1F LSMC error paths, YoYInflationSwap.
- **Wall-clock assertions** (`elapsed.as_millis() < 100`, `as_secs() < 60`) violate the testing standards (no time-based assertions) — present in `inflation_linked_bond/test_cashflows.rs:336` and `autocallable/test_day_count_basis.rs:99`. Strip them.

---

## Findings by bucket

> Each bucket section is the verified, consolidated output for its slices. File references are `file:line` relative to the slice's directory unless otherwise noted. "remove" = safe per the adversarial verifier; "strengthen rather than delete" calls are noted explicitly.

## credit

The credit instrument test suite carries substantial low-value bloat (positivity/Ok-only duplicates and tautological constant-vs-itself tests) and notable metric coverage gaps, most critically several registered risk metrics (Recovery01, Cs01Hazard, Dividend01) with zero test invocation.

### Duplicates to consolidate

- `metrics_basic.rs:243 test_protection_pv_metric_positive`, `:266 test_premium_pv_metric_positive`, `:220 test_par_spread_metric_positive` (cds_index) — remove; `assert_positive`-only variants of `test_metric_protection_leg_pv:71`, `test_metric_premium_leg_pv:95`, `test_metric_par_spread:40`.
- `metrics_risk.rs:312 test_risky_pv01_present` and `:554 test_risky_pv01_computable` (cds_index) — byte-identical `abs()>0` checks; remove both, `test_risky_pv01_positive:18` subsumes (adds range + positivity).
- `metrics_risk.rs:113 test_risky_pv01_scales_with_notional`, `:154 test_cs01_scales_with_notional`, `:236 test_risky_pv01_increases_with_maturity` (cds_index) — remove; identical to `pricing_single_curve.rs:229/253/336`. Keep `test_cs01_increases_with_maturity:274` (no counterpart).
- `pricing_parity.rs:296 test_mode_independence_of_par_spread`, `edge_cases.rs` vs `pricing_single_curve.rs:379 test_single_curve_zero_notional` (cds_index) — remove the latter of each pair; identical to `pricing_parity.rs:79` / `edge_cases.rs:28 test_zero_notional`.
- `risk_metrics_tests.rs:22/231/313/431/519/703` + `expected_loss_tests.rs:20` (cds_tranche) — remove the seven `*_calculation_succeeds` (`is_ok()`-only); each paired stronger value-asserting test (e.g. `:37`, `:249`, `:328`) unwraps and checks the value.
- `config_tests.rs:148/242/256/268/280/295` (cds_tranche) — remove six `test_custom_config_*` setter/readback tests; field effects covered by `test_heterogeneous_*` pricing and `test_pricer_config_builder_methods_wire...:163`. Also drop tautological `config_tests.rs:116 test_hetero_method_spa` and `:125 test_hetero_method_exact_convolution`.
- `test_trs_pricing_engine.rs:35/153/169` (trs) — remove engine-level `pv_financing_leg`/`financing_annuity` positivity+scaling+spread-monotonicity tests; instrument/metric delegators (`test_equity_trs.rs:352`, `test_trs_metrics.rs:164/280`) cover the same `TrsEngine` paths.
- `test_trs_metrics.rs:595 test_theta_is_finite` (trs) — body-identical to `:551 test_equity_trs_theta_calculation`; remove. Also remove value-trait duplicates `test_equity_trs.rs:148`, `test_fi_index_trs.rs:170` (subsumed by `:83` / `:105`).
- `pricing_tests.rs:80 test_mezzanine_tranche_pricing` (cds_tranche) — remove; `test_tranche_pricing_returns_valid_pv:28` adds a currency assertion and subsumes it.
- structured_credit unit roundtrips `specs_tests.rs:83/97/111` — remove; `serialization_tests.rs:31/50/69 *_all_variants_serialize` cover roundtrip, with format-stability retained separately. Also drop subsumed `stochastic_pricing_tests.rs:175`, `cashflow_sweep_tests.rs:571`, `feature_tests.rs:176 writedown_respects_subordination_order`.
- `test_implied_vol.rs:131 test_implied_vol_positive` (cds_option) — remove; `test_implied_vol_round_trip:9` exercises vol=0.35 at tighter tolerance.

### Dead / unnecessary tests to remove

- `market_standards_tests.rs:193/308/226/59` (cds_tranche) — remove four constant-vs-itself / setup-ordering tautologies (`1.0==1.0`, `0.5==0.5`, `low<high`, `0<3<7<10`); real behavior covered by config-default and correlation/subordination value tests.
- `deal_specific_tests.rs:240/262/332`, `coverage_tests.rs:55/335`, `waterfall_tests.rs:267` (structured_credit) — remove `matches!` enum-identity tautologies, tautological getter constructions, and the PSA-grid-content test; behavior covered by the corresponding calculator/scenario tests in the same files.
- `feature_tests.rs:354 reserve_account_recipient_type_exists`, `:321 cleanup_call_disabled_by_default`, `calendar_tests.rs:315 test_different_calendars_may_produce_different_dates` (structured_credit) — remove; variant-roundtrip/default-value/non-empty-only checks with no behavioral assertion.
- `test_metrics_registry.rs:444 test_bucketed_dv01_registered`, `test_greeks.rs:28 test_delta_put_negative`, `test_pricing.rs:126 test_near_expiry_option` (cds_option) — remove; `is_ok()||is_err()` always-true, `assert_finite` instead of stated sign, and `t=0` no-assertion edge subsumed by `test_very_short_dated_option:143` / `test_put_delta_sign_negative`.
- `test_equity_trs.rs:66 test_equity_trs_different_contract_sizes` (trs) — remove; both instruments use the hardcoded default `contract_size=1.0`, so it asserts the constant against itself.

### Coverage holes to add

- **Recovery01 / Cs01Hazard untested (major, cds_index):** `MetricId::Recovery01` and `MetricId::Cs01Hazard` are registered but have zero test invocations. Add `metrics_risk.rs` cases asserting finite/expected-sign, notional linearity, SingleCurve-vs-Constituents consistency, recovery=0 boundary (no NaN), and Cs01Hazard≈Cs01 under flat hazards.
- **Defaulted-constituent end-to-end pricing (major, cds_index):** no test sets `defaulted: true`. Add a `pricing_constituents.rs` test with 1-of-5 defaulted, `index_factor=0.8`; assert premium leg reduced by defaulted weight, live-weight sum <1.0, and `value()`/leg PVs succeed.
- **Equity TRS Dividend01 metric (major, trs):** `MetricId::Dividend01` registered but unexercised. Add tests for finite/expected-sign with `div_yield_id` set, the `div_yield_id=None` case, and pay-TR side sign flip.
- **Bloomberg parity vega/CS01/theta gated off (major, cds_option):** `BBG_VEGA`/`BBG_CS01`/`BBG_THETA` assertions live only inside `#[ignore]` diagnostics in `test_cdx_ig_46_cdso_diagnostics.rs`; only NPV runs in CI. Promote a test asserting vega and CS01 within ~5% of BBG once the CS01 rebootstrap-anchor issue is resolved; retag as `slow`, do not delete the probes.
- **cds_tranche Recovery01 + validate()/serde (moderate):** add `metrics_calculator_tests.rs` Recovery01 case (finite + documented sign); direct `CDSTranche::validate()` tests (attach≥detach, fractional detach>1.0, non-positive notional → Err); and `TrancheSide` FromStr aliases + JSON roundtrip + `deny_unknown_fields` rejection.
- **Serde unknown-field rejection (moderate, multiple slices):** `StructuredCredit`, `CDSIndex`/`CDSIndexConstituent`, and `CDSOption` all carry serde stability invariants but have no `deny_unknown_fields` negative test (and CDSOption no serialize→deserialize→reprice equality). Add one rejection + one roundtrip test per type.
- **TRS pricing-path gaps (moderate):** add integration tests for `dividend_tax_rate>0` (PV reduction vs 0.0), seasoned `past_fixings`-driven `value()`, `FinancingRateCompounding::OvernightCompounded` (leg PV differs from TermRate), and the par-spread closure (rebuild with computed par spread → `|NPV|<tol`).
- **cds_option mode/path gaps (moderate):** add tests for `MetricId::SpreadDv01` end-to-end (finite positive payer), `ProtectionStartConvention::Spot` pricing (finite + Spot-vs-Forward NPV diff).
- **cds_index integration gaps (moderate):** add tests for `CashflowProvider::cashflow_schedule` (non-empty quarterly Projected schedule) and OC/IC trigger diversion through full `run_simulation` (equity receives less vs no-trigger baseline) and reserve-account cash diversion (structured_credit).
- **Z-spread off-par (moderate, structured_credit):** `risk_tests.rs:93` only covers the trivial zero case; add discount/premium tranche cases asserting positive/negative bps from `calculate_tranche_z_spread`.

## fixed-income-cash

Strong existing coverage, but a recurring pattern of `is_ok()`/`is_finite()`-only assertions, hand-computed tautological DV01 tests, and large registered-but-untested metric/process surfaces (WAL, FRN z-spread, MC variants, repo collateral sensitivities, bond-future CTD selection) needs attention.

### Duplicates to consolidate

- `tests/instruments/revolving_credit/revolving_credit.rs` (entire 10-test file) is undeclared in `mod.rs` and never compiles — delete it; `basic.rs` covers all 10 plus the unique `basic.rs:285 test_revolving_credit_cs01_z_spread_fallback_without_credit_curve`.
- `tests/instruments/term_loan/metrics/callability.rs` is a byte-for-byte subset of `integration.rs` — delete the file and drop `mod callability;`; both tests survive verbatim in `integration.rs` (which also has the unique floating-rate DM test).
- ILB `test_edge_cases.rs` duplicates the `test_pricing.rs`/`test_cashflows.rs` canonical homes — remove `test_cashflow_provider_trait:496`, `test_valuation_at_maturity:34`, `test_valuation_after_maturity:18`, `test_same_issue_and_maturity_date:265`; also remove `test_pricing.rs:341 test_npv_positive_for_positive_coupons` (subsumed by `test_npv_basic:16`) and `test_pricing.rs:34 test_value_via_instrument_trait` (calls `value()` twice, no real dyn dispatch).
- inflation_swap `test_ir01.rs` near-clones `test_dv01.rs` (both use `MetricId::Dv01`) — remove `test_ir01_scales_with_maturity:82` and `test_ir01_zero_for_matured_swap:198` (weaker `<1.0` vs exact-zero); keep `test_ir01_finite_difference_validation`.
- bond_future error/serde duplication — remove the five `integration.rs:528–650` error-handling tests (stricter twin assertions in `types.rs:1455–1562`); collapse `serde.rs:164–207` per-spec roundtrips into one all-field parameterised roundtrip; remove `serde.rs:573 test_bond_future_compact_json`, `serde.rs:285/312` position-only roundtrips, `types.rs:2094 test_instrument_trait_id`, and fold `integration.rs:1231 test_bucketed_dv01_registration`'s Theta check into `metrics/mod.rs:120 test_metrics_registration` then delete it.
- revolving_credit edge_cases vs pricing — remove `validation/edge_cases.rs:45 test_full_utilization` and `:16 test_zero_utilization` (pure `is_ok()` copies of the stronger `pricing.rs:78`/`:48`); keep the unique short/long commitment-period boundary tests.
- repo edge_cases — remove `edge_cases.rs:84 test_very_short_term` (subset of `construction.rs:189`) and `edge_cases.rs:126 test_zero_rate_repo` (fold its `total_repayment` assertion into `pricing.rs:121 test_zero_rate_interest`).

### Dead / unnecessary tests to remove

- ILB `test_duration.rs:202–369` — seven DV01 tests (`test_dv01_positive_before_maturity`, `_zero_at_maturity`, `_zero_after_maturity`, `_scales_with_notional`, `_scales_with_time_to_maturity`, `_reasonable_magnitude`, `test_duration_and_dv01_relationship`) hand-compute `notional*yf*0.0001` and never invoke any ILB metric; remove or rewrite against `price_with_metrics(MetricId::Dv01)`. Real DV01 is already covered by `test_metrics.rs:118`.
- revolving_credit no-assertion / wrong-direction tests — `construction.rs:138/159` assert invalid configs are *accepted* (rewrite to `.build().unwrap().validate()` and assert `Err`, or delete; `validate_method.rs` already covers rejection); strengthen `cashflows.rs:16/45/75` (only `!flows.is_empty()`), `metrics/commitment_fee.rs:16`, `metrics/utilization_fee.rs:16`, and `metrics/dv01.rs:17` (add `dv01 < 0.0` sign check) with the quantitative assertions their comments already promise.
- repo no-assertion / tautological — fix `margin.rs:103 test_margin_frequency_options` (wildcard `matches!(_, _)` → `assert_eq!`); remove trivial getter round-trips `edge_cases.rs:286 test_triparty_flag_variations`, `edge_cases.rs:359 test_business_day_conventions`, `margin.rs:227`, `metrics.rs:371 test_metric_dependencies_resolved`; strengthen `edge_cases.rs:171/193` and `metrics.rs:332 test_bucketed_dv01_metric` (only `contains_key`) with PV/finiteness/sum-to-flat-DV01 bounds.
- bond_future no-assertion / tautological — remove `pricer.rs:545 test_cashflow_debug` (println only), `mod.rs:56 test_module_compiles` (empty), and `types.rs:1277 test_deliverable_bond_construction` (literals echoed back; `InstrumentId::new` does no validation).
- bond `helpers_tests.rs:500/507` (Copy/Clone derive tautologies) and `metrics/oas.rs:12 test_oas_behavior_without_quoted_price` (assertion gated behind `if let Ok…Some`, cannot fail) — remove; positive OAS path covered by `test_oas_with_quoted_price:50`.
- term_loan `cashflows.rs:66 test_amortizing_principal_cashflows` (only `!is_empty()`; covered by four stronger amort tests) — remove; `cashflows.rs:105 test_pik_interest_capitalization` and `metrics/theta.rs:19 test_theta_reflects_time_decay` — strengthen rather than delete (PIK is the sole cashflow-gen exercise of `CouponType::PIK`; theta has no other directional coverage).
- inflation_swap dedupe/strengthen — dedupe the doubled `MetricId::Dv01` entries in `integration/test_full_pricing.rs:34/160` and `:319/321`; add a real sign assertion to `test_ir01.rs:126 test_ir01_sign_pay_fixed` (currently `is_finite` only); delete the stale TODO comment at `test_bucketed_dv01.rs:95-99` (contradicts the now-correct `>0.0` assertion).

### Coverage holes to add

- bond_future CTD selection (major): `determine_ctd` (`types.rs:839`) and `determine_ctd_with_accrued` (`types.rs:942`) are untested (only implied-repo path is) — add tests selecting the cheapest gross/dirty-basis bond from a multi-bond basket plus the no-valid-prices `Err` path.
- bond_future metric calculators (major): `metrics/pricing.rs` (`FuturesPriceCalculator`, `ConversionFactorCalculator`) has zero tests — add registry-dispatch tests asserting outputs match `BondFuturePricer::calculate_model_price` / `calculate_conversion_factor`.
- bond WAL metric (major): `BondWalCalculator` (registered `metrics/mod.rs:194`) has no tests — add (1) bullet WAL == time-to-maturity, (2) amortizing WAL = Σ(Pᵢtᵢ)/ΣPᵢ < maturity, (3) WAL decreases as `as_of` advances.
- revolving_credit untested processes/validation (major): add `McConfig::validate()` branch coverage (recovery≥1, non-PSD corr, CIR Feller, `util_credit_corr>1`); exercise `CreditSpreadProcessSpec::{Cir,Constant}` and `validate_method.rs` draw/repay event dated == commitment_date — all currently only hit via `MarketAnchored`/never.
- term_loan registered-but-untested metrics (major): `AllInRate` (`metrics/mod.rs:144`) and `EmbeddedOptionValue` (`:171`) have zero dispatch tests — add `all_in_rate≈coupon` (plain) vs `>coupon` (with fees), and `EmbeddedOptionValue>0` callable / `==0` non-callable.
- inflation_cap_floor parity + metrics (major): no Cap−Floor put-call-parity test (peer `cap_floor` slice has one) and `Gamma`/`Dv01`/`BucketedDv01` (registered `metrics/mod.rs:33-51`) are untested — add an ATM parity identity vs the YoY/ZC swap PV, plus FD-validated gamma (positive long-cap), bump-diff Dv01, and bucketed-sums-to-parallel checks.
- inflation_swap YoYInflationSwap (major): all 86 slice tests cover only `InflationSwap`; `YoYInflationSwap` has no integration coverage — add par-rate roundtrip, pay/receive sign, multi-period pricing, Inflation01 FD validation, theta, maturity scaling, and lag policies.
- ILB constructors / curve-dependency completeness (major): `new_uk_linker_modern` (`types.rs:478`) and `new_jgbi` (`:550`) have no construction/pricing tests; ILB is absent from `tests/instruments/curve_dependency_completeness.rs` (inflation curve is a separate, undeclared dependency) — add construction+pricing tests for both and an ILB completeness test verifying pricing with only declared deps.
- inflation_swap untested pricing paths (moderate): explicit swap `base_cpi` branch (`types.rs:215`), `validate()` error variants (`InvalidDateRange`/`NonPositiveValue`), BDC/`calendar_id` payment adjustment, HW1F rate process, and missing-discount/inflation-curve `Err` paths are all unexercised — add targeted tests for each.
- repo collateral sensitivities (major): `CollateralHaircut01`/`CollateralPrice01` (registered `metrics/mod.rs:44-53`) are never computed — add tests asserting ~0.0 (documenting the current model) and exercising the haircut01 down-bump clamp branch (`haircut01.rs:58`).
- Serde deny-unknown-fields round-trips (moderate, slice-wide): no inbound-rejection/round-trip tests for `BondQuoteInput`/`PricingOverrides` (bond), `TermLoanSpec`, `Repo` (`deny_unknown_fields` at `types.rs:197`), or RevolvingCredit `DrawRepayEvent` currency mismatch — add serialize→deserialize equality plus unknown-field `Err` tests per the serde-stability invariant.
- term_loan untested enum variants (moderate): `OidPolicy::{WithheldAmount,SeparatePct,SeparateAmount}`, `LoanCallType::{Soft,MakeWhole}` (MakeWhole bypasses the tree pricer at `types.rs:762/782`), and OAS/CS01/BucketedCS01/BucketedDV01 dispatch — add cashflow/metric tests covering each.
- bond parallel/currency/FRN gaps (moderate): no `parallel:false` vs `parallel:true` bitwise-equal Decimal test, no USD-bond-vs-EUR-curve currency-mismatch `Err` test, and all eight `metrics/spreads.rs` z-spread tests use fixed-rate bonds — add an FRN (SOFR+150bp) z-spread round-trip and the two safety/determinism tests.

## rates-derivatives

The rates-derivatives suite carries substantial redundancy (existence-only smoke tests, byte-identical spec/Greek duplicates, and a dead `cfg(any())` block) alongside material gaps in second-order/diagnostic metrics, serde stability, and shifted-lognormal/negative-rate pricing branches.

### Duplicates to consolidate

- IRS rate-sensitivity sweep is tested twice with identical inputs — remove `pricing.rs:320 test_irs_rate_sensitivity_inverse`; `validation/market_standards.rs:454 test_irs_rate_sensitivity` is strictly richer (adds at-par NPV).
- Cap/floor Bachelier-Greeks FD tests are near-identical across two files — merge `validation/normal_vol.rs` and `validation/normal_greeks.rs` into one file, keeping a single FD-delta and FD-vega plus the unique `normal_greeks_with_negative_forward` (line 378) and `normal_delta_increases_with_moneyness` (line 446).
- IR-future spec/convention helpers are byte-for-byte identical, producing five subset duplicates — remove `test_market_standard.rs:375 test_standard_contract_specifications`, `:116 test_tick_value_exchange_standard`, `:152 test_sofr_convention`, and `test_construction.rs:64 test_eurodollar_specs`; either drop or make `create_eurodollar_specs`/`create_sofr_specs` genuinely distinct.
- IRS payer/receiver symmetry is over-covered — drop `proptests.rs:182 payer_receiver_symmetry` (subsumed by `test_swap_symmetry.rs` proptests).
- Basis-swap same-input subsumptions — remove `test_basis_swap_metrics.rs:157 theta_defined_and_finite` (use `test_basis_swap_theta.rs:145`) and `test_basis_swap_sensitivities.rs:183 dv01_sign_convention` (subsumed by `:320 dv01_leg_components_reasonable`).
- Cap/floor `metrics/implied_vol.rs:54 test_implied_vol_requires_market_price` is the weaker twin of `:183 test_implied_vol_fails_without_market_price_override` (which uses `is_err()`) — delete line 54.
- IR-future `test_position.rs:50 test_position_copy` is identical to `:43 test_position_clone` (both implicit Copy move) — remove the copy test or make clone call `.clone()` explicitly.
- xccy missing-FX-matrix path is duplicated — keep the stronger inline `types.rs:1075 base_value_fails_loud_when_fx_matrix_is_missing`; either remove or strengthen `pricing.rs:9 requires_fx_matrix_when_reporting_currency_differs` to assert the instrument id.
- CMS-option vol-surface-missing is tested twice — remove `test_pricing.rs:422 test_cms_option_requires_vol_surface`; keep the realistic wrong-key `:464`.

### Dead / unnecessary tests to remove

- CMS-option `vanna.rs:1-98` is gated by `#![cfg(any())]` (never compiles) and predates the Decimal migration — delete lines 1-98 and the dead-only imports at 99-108; live `test_cms_option_vanna` (line 110) covers it.
- IRS bucketed-DV01 existence-only smoke tests assert only `contains_key`/`is_some` — remove all five: `metrics/bucketed_dv01.rs:109/133/155/177/198`; real coverage is in `test_bucketed_dv01_per_curve` and `test_bucketed_vs_parallel_dv01_sanity`.
- Tautological getter/literal-echo tests — remove IRS `construction.rs:194 test_irs_large_notional` & `:210 test_irs_small_notional`, `integration/margin.rs:195 test_bilateral_vs_cleared_im_difference`, and IR-future `test_pricing.rs:229 test_value_trait_consistency` (both sides call `value()`).
- Permanently-green `is_ok() || is_err()` tests — remove IR-future `test_edge_cases.rs:9 test_expired_future` and `:176 test_future_date_before_base_date`.
- Weak `is_finite()`-only tests superseded elsewhere — remove IRS `integration/complex_scenarios.rs:145 test_multi_curve_environment`, `:260 test_swap_portfolio_aggregation`, `pricing.rs:417 test_irs_long_maturity` (superseded by `numerical_stability.rs:175`), and `metrics/par_rate.rs:151 test_par_rate_positive` (subsumed by `test_par_rate_flat_curve`).
- CMS-option tautological/finiteness-only tests — remove `test_pricing.rs:219 test_long_tenor_cms_convexity_larger_than_short_tenor` (asserts `abs()>=0.0`, never compares the two tenors) and `:356 test_vanna_changes_with_moneyness` (only `is_finite`, redundant with `test_vanna_computable`), or add the intended directional assertion.
- xccy `pricing.rs:214 rejects_non_finite_notional` tests the wrong layer (panic comes from `Money::new`, not XccySwap) — remove; core's `new_rejects_non_finite_amounts` already covers it.
- Basis-swap `test_basis_swap_par_spread.rs:401 par_spread_different_frequencies` is mislabeled (both legs quarterly) and only `is_finite` — fix to use genuinely different frequencies with a sign assertion, or remove.

### Coverage holes to add

- **IRS second-order/diagnostic metrics (major):** `IrConvexity`/`IrCrossGamma` (`metrics/ir_convexity.rs`) and the `schedule_diagnostics` calculators (payment counts, first/last payment dates, first accrual factor) have zero tests — add value-asserting tests for both.
- **IRS serde stability (major):** no round-trip or `deny_unknown_fields` test for `InterestRateSwap` — add a serialize/deserialize equality test plus an unknown-field-rejection test.
- **Cap/floor ShiftedLognormal branch (major):** every test sets `vol_shift: 0.0`; the distinct pricing/Greek branch is untested — add `validation/shifted_lognormal.rs` asserting positive PV under shift with negative forward, convergence to Black-76 as shift→0, and consistent F+shift/K+shift application.
- **Cap/floor BucketedDv01 + outer-struct serde (major):** `MetricId::BucketedDv01` is never requested on a CapFloor, and no test round-trips the `deny_unknown_fields` `CapFloor` struct — add bucket-sum-equals-total test and a serde round-trip + unknown-field-rejection test.
- **CMS-option pricing branches (major):** the default Black76 (Hagan) pricer is never exercised for `OptionType::Put`, the `from_schedule` constructor is never called, and the negative-forward Bachelier fallback (`pricer.rs:131-168`) is untested — add put/floor pricing (bounded by discounted intrinsic), a `from_schedule` build+price test (incl. empty-schedule error), and a negative-rate pricing test for cap and floor.
- **IR-future convexity vol-sanity error + diagnostic metrics (major):** the `[0,0.05]` vol-bound `Err(Validation)` branch (`types.rs:499-509`) and the registered `FuturesPrice`/`ImpliedForward`/`ConvexityAdjustment` metrics are unverified — add a lognormal-vol-feed validation-error test and `price_with_metrics` value assertions for the three metric keys.
- **xccy_swap metrics + introspection + seasoned MtM (moderate):** `Dv01`/`BucketedDv01`/`CrossGammaFxRates`, `market_dependencies()` (FX pair + curve ids), and the past-period skip guard on a seasoned MtM-resetting swap are all untested — add a DV01 smoke test (sign per dPV/dy), a `market_dependencies` assertion, and a seasoned-MtM PV/no-past-cashflow test.
- **Basis-swap error/frequency/serde gaps (moderate→minor):** add missing-curve error-path tests (discount + forward), a genuine different-frequency par-spread+sign test, a `deny_unknown_fields` rejection test, and a `curve_dependencies()` assertion.
- **IR-future golden + defaulting paths (moderate):** add one analytic reference-PV test (`$1MM` SOFR, flat 5%, price 97.50) tied to `(implied-forward)×Face×τ`, and an integration test driving the builder's optional-date defaulting path end-to-end.
- **Cap/floor + CMS smaller branches (moderate):** add negative-strike pricing (Normal positive PV; Lognormal error/fallback), strengthen theta sign assertions (`theta.rs`) and make `test_short_maturity_higher_theta` actually compare magnitudes, exercise `CapFloor::example()`/`market_dependencies()`, and add CMS Delta/Rho/Volga smoke tests plus a put-call-parity check.

## equity-fx

Heavy inline/integration test duplication and a large band of registered-but-unexercised metrics and alt-model pricers (Heston/rough-Heston/PDE MC, OHLC variance methods, FX delta/correlation metrics), plus pervasively missing serde `deny_unknown_fields` round-trip coverage across all FX instruments.

### Duplicates to consolidate

- NDF inline unit tests fully subsumed by integration copies (strict supersets): remove `src/instruments/fx/ndf/types.rs` `test_ndf_value_at_market:1505`, `test_ndf_value_with_fixing_rate:1535`, `test_ndf_value_unfavorable_fixing:1564`, `test_ndf_value_expired:1592`, `test_ndf_with_fixing_rate:1031`, `test_ndf_serde_roundtrip:1095`, `test_ndf_curve_dependencies:1062`, `test_ndf_with_foreign_curve:1072`, `test_ndf_creation:998`, `test_ndf_example:1022`, `test_ndf_instrument_trait:1051`.
- FxForward inline unit tests subsumed by integration supersets: remove `src/instruments/fx/fx_forward/types.rs` `test_fx_forward_creation:795`, `test_fx_forward_example:815`, `test_fx_forward_instrument_trait:844` (+ `test_fx_forward_pricing.rs:335 test_fx_forward_instrument_key`), `test_fx_forward_curve_dependencies:855`, `test_fx_forward_serde_roundtrip:865`, `test_fx_forward_with_forward_points:824`.
- EquityIndexFuture inline unit tests subsumed by `test_types.rs`: remove `src/instruments/equity/equity_index_future/types.rs` `test_equity_future_specs_sp500_emini:504`, `test_equity_future_specs_nasdaq100_emini:512`, `test_sp500_emini_constructor:553`, `test_nasdaq100_emini_constructor:575`, `test_position_sign:597`, `test_serde_round_trip:625`.
- Equity duplicate pricer-vs-types tests (pricer copies hit the same direct field/delegate, add no surface): remove `tests/instruments/equity/pricer_tests.rs` `test_equity_effective_shares_default:240`, `test_equity_effective_shares_explicit:246`, `test_equity_dividend_yield_default:104`, `test_equity_dividend_yield_from_market:115`; remove `types_tests.rs:234 test_equity_instrument_trait_id`.
- fx_spot redundant tests subsumed by stronger copies: remove `tests/instruments/fx_spot/test_bucketed_dv01.rs` (whole file + `mod.rs:31`), `integration/market_standards.rs` `test_standard_eurusd_t_plus_2_settlement:19`/`test_zero_value_at_par:223`/`test_instrument_trait_compliance` already covers `edge_cases.rs:325 test_instrument_key_consistency`, `edge_cases.rs:285 test_extreme_settlement_lag`, `construction.rs:202 test_with_notional_valid_currency`, `pricing.rs:63 test_npv_without_rate_or_matrix_fails`.
- equity_option determinism/parity dupes: remove the four per-Greek determinism tests `test_option_pricing.rs:85/127/161/199` (keep `test_option_all_greeks_determinism:237`), `test_option_put_call_parity_determinism:344`, and `test_near_expiry.rs:257 test_expired_option_price_is_intrinsic` (looser tol than `test_edge_cases.rs:11/32`).
- fx_option redundant inherent-vs-trait/override copies: remove `test_instrument.rs:235 test_value_method_returns_positive_pv`, `test_instrument.rs:301 test_price_with_metrics_matches_value`, `test_instrument.rs:330 test_pricing_overrides_applied`, `test_edge_cases.rs:444 test_currency_mismatch_detected`.
- fx_swap dupes: remove `tests/instruments/fx_swap/edge_cases.rs:74 test_far_before_near` (subsumed by `types.rs:477`), `integration.rs:231 test_par_swap_construction`, `metrics.rs:416 test_bucketed_dv01`.

### Dead / unnecessary tests to remove

- Tautological field-setter / enum-assignment tests exercising no library logic: `equity_option/test_constructors.rs:99 test_settlement_type_variations` and `:113 test_exercise_style_variations`; `fx_option/test_instrument.rs:437 test_exercise_styles` — **replace** this one with a `value()` call on an American-style FxOption asserting `Err` (otherwise the American-rejection path goes uncovered).
- Helper/self-validation tests verifying test scaffolding, not production code: `equity_option/helpers.rs:221 test_assert_approx_eq_helper`, `:226 test_smile_surface_builder`, `:233 test_smile_market_builder`.
- fx_spot tautological/no-assertion tests: `metrics/theta.rs:48 test_theta_with_future_settlement` (no assertion), `construction.rs:60 test_construction_with_bdc` (sets the existing default), `edge_cases.rs:168 test_multiple_clones` (moves, not clones), and the three id getter round-trips `edge_cases.rs:142/149/157` (`Id::new` does no validation).
- `equity/spot/pricer.rs:153 test_equity_pricing_error_message_quality` — `create_test_equity()` always has a price, so the Err branch is unreachable and the Ok branch is tautological; remove (error path covered by `types_tests.rs:343 test_equity_price_missing`).
- `fx_swap/pricing.rs:216 test_pv_currency_consistency` (zero marginal coverage vs `pricing.rs:15`) and `edge_cases.rs:331 test_attributes_access` (generic HashMap behavior).
- `fx_option/test_calculator.rs:115 test_surface_vol_used_in_pricing` (identity check on helpers, no pricing call) and `test_implied_vol.rs:305 test_implied_vol_uses_override_as_initial_guess` (inert param, near-no-op bound; covered by `pricer.rs:410 w45_...`).
- `fx_forward/test_fx_forward_types.rs:152 test_fx_forward_clone` — derived Clone, no behavior verified.

### Coverage holes to add

- **(major) Alt-model MC/PDE pricers for equity_option have zero integration tests.** Add `test_heston_mc.rs` and `test_rough_heston_mc.rs` mirroring `test_rough_bergomi.rs` (ATM sanity, fixed-seed bit determinism, discrete-dividend W-31 rejection, missing-scalar error); add `PdeCrankNicolson1D` (matches BS European, American ≥ European) and a `PdeAdi2D` smoke test.
- **(major) FX variance swap has no Pay-side, no matured-path, and no OHLC coverage.** Add Pay-vs-Receive sign test (`pv_pay == -pv_receive`); a matured case (`as_of >= maturity`) asserting undiscounted PV plus an empty-prices zero-PV case; and OHLC-method tests (e.g. Parkinson) covering populated series and the missing-`open_series_id` error. Add a `metrics.rs` driving all six untested secondary calculators (Vega/VarianceVega/RealizedVariance/VarianceNotional/StrikeVol/TimeToMaturity/DV01) through `MetricContext`.
- **(major) Registered-but-unexercised metric values for fx_spot/fx_forward/quanto.** fx_spot: assert `FxDelta == notional` and `Fx01 == 12000` (1% of 1.2M USD) and DV01 sign. fx_forward: invoke `Dv01`/`Fx01` via the registry and assert against `fx_forward_1y_eurusd.json` (taking dPV/dy native signs as correct), optionally `BucketedDv01` sums to parallel `Dv01`. quanto: extend the finite-metrics test to request `Rho/ForeignRho/Dv01/BucketedDv01/Vanna/Volga`.
- **(major) Untested public market-convention constructors.** `fx_forward::standard_spot_days`/`from_trade_date_auto` (T+1 arms for USD/CAD, USD/TRY; T+2 otherwise) and `fx_option::european_from_trade_date` (CLS spot roll, default `spot_lag_days=2`, weekend adjustment, invalid-calendar error) have zero call sites.
- **(major) quanto reversed-FX-id guard and FxDelta/FxVega no-id paths.** Add a build that embeds the reversed pair (e.g. `USDJPY` when base=JPY/quote=USD) asserting `Err`; cover `validate_fx_id_direction` symmetrically for `fx_vol_id`.
- **(moderate) Serde `deny_unknown_fields` round-trip + unknown-field rejection is missing for essentially every FX instrument.** Add round-trip + negative (unknown-key → `Err`) tests for `EquityOption`, `FxSpot`, `FxOption`, `FxSwap`, `Ndf`, and `EquityIndexFuture`; extend the Equity round-trip to include `discrete_dividends` and an unknown-field case.
- **(moderate) FX `market_dependencies` completeness gaps.** Extend `fx_dependency_completeness.rs` with cases for `FxSpot`, `FxOption`, `FxSwap`, `Ndf`, `FxVarianceSwap`, and `EquityIndexFuture`, asserting declared FX pair / curves / vol-surface ids and the missing-FX-matrix (CIRP) error path where applicable.
- **(moderate) equity_option higher-order/cross/bucketed Greeks unexercised.** Add sign/finiteness smoke tests for `Charm/Color/Speed`, `Vanna/Volga/CrossGammaSpotVol/Dividend01` (Volga>0, Dividend01<0 for calls), and `BucketedVega/BucketedDv01/Dv01` (finite, sum ≈ total, zero outside tenor).
- **(moderate) Discounting / details visibility paths invisible externally.** fx_spot: external test configuring a discount curve asserting `pv_discounted < pv_undiscounted`. fx_forward: assert `FxValuationDetails.fx_triangulated` is `Some(false)` for a direct quote and `None` under spot override. quanto: `Correlation01` boundary error at `rho=±1.0`, and FxDelta/FxVega `Err` when their ids are `None`.
- **(moderate) American/Bermudan early-exercise economics at instrument level.** equity_option: American put PV > European put PV (r>0, deep-ITM, non-dividend), premium grows with rate; Bermudan with a valid 2–3-date schedule bounded `European ≤ Bermudan ≤ American`.
- **(moderate) Equity pricer error/branch coverage.** Cross-currency `convert_price_to_currency` FX-matrix branch (USD equity priced in EUR with/without FxMatrix, latter asserting `'fx_matrix'` error); `equity_index_future` discrete-dividend forward branch, `resolve_dividend_yield` Price-scalar/absent-scalar errors, `expiry == as_of` (T=0, F≈S) boundary; real_estate `unlevered_irr` metric value plus numeric assertions for `going_in/exit_cap_rate ≈ 0.10`.
- **(moderate) fx_swap secondary metrics are `is_finite`-only.** Strengthen `Fx01/Theta/carry_pv/foreign IR01/steep/inverted PV` (`metrics.rs:183/370/393/115`, `pricing.rs:177/199`) with sign+magnitude bounds; add `FxDelta == Fx01` equivalence and `FxSwap::from_trade_date` (T+2 spot lag) coverage.

## exotics-commodity-misc

Commodity-instrument duplicate tests and tautological getters dominate the cleanup; the highest-value gaps are entire untested calculators/pricers (weight-risk, NAV01/Carry01/Hurdle01, Heston barrier MC, commodity Asian/spread/swaption integration) plus broadly missing serde round-trips and golden references.

### Duplicates to consolidate

- Commodity integration tests duplicate stronger inline unit tests — remove `test_commodity_forward.rs:125 test_commodity_forward_instrument_trait`, `:210 test_commodity_forward_long_short_symmetry`, `test_commodity_swap.rs:163 test_commodity_swap_instrument_trait`, `:452 test_commodity_swap_serialization`, `:174 test_commodity_swap_receive_fixed`; each is subsumed (often more tightly) by the inline unit test of the same/sibling name in `commodity_forward/types.rs` / `commodity_swap/types.rs`.
- `variance_swap/valuation.rs:260 test_npv_at_maturity_no_discounting_applied` — remove; the PV==undiscounted-payoff identity is already verified with a different path by `:238 test_npv_at_maturity_recovers_realized_payoff`.
- `private_market_fund/test_private_markets_fund.rs:427 test_irr_calculation_accuracy` — remove; exact copy of `pe_fund/metrics.rs:358 test_irr_calculation`, which is strictly stronger.
- `autocallable/test_day_count_basis.rs:194-202` (pv_365 vs pv_365_again sub-check in `test_autocallable_same_day_count_basis`) — drop only that sub-assertion; determinism is covered by `test_autocallable_deterministic_seeding`. Keep the unique cross-day-count comparison.
- `variance_swap/edge_cases.rs:588-618` (`test_time_elapsed_fraction_boundary_at_start`/`_at_maturity`) — removable (medium confidence); the four `observation.rs:223/236/248/260` boundary tests plus `:308 is_monotonic` cover the same assertions.

### Dead / unnecessary tests to remove

- No-assertion / tautological constructors and getters — remove `basket/test_basket_comprehensive.rs:1589 test_basket_calculator_from_basket`, `private_market_fund/test_private_markets_fund.rs:310 test_private_markets_fund_creation` and `:320 test_private_markets_fund_with_discount_curve` (fold the latter into a real discount-curve PV test).
- `variance_swap/variance_calculation.rs:293 test_expected_variance_before_start_equals_forward` — remove; the forward-variance comparison its name promises is bound to `_forward` and never asserted, and the lone live assertion duplicates two other tests.
- `variance_swap/valuation.rs:357 test_value_method_delegates_to_npv` — remove; tautological 2-call determinism check already covered by `edge_cases.rs:395`.
- `autocallable/test_day_count_basis.rs:99-106` wall-clock `assert!(elapsed.as_secs() < 60)` (and Instant plumbing at :31/:71/:73) inside `test_autocallable_mismatched_day_count_bases` — delete; time-based assertions are barred by testing standards.
- `exotic_harness/bermudan_swaption_parity.rs lsmc_proxy_price_is_nonnegative_and_stable` — remove or rename to `lsmc_harness_smoke`; assertions are satisfied by construction and the engine is covered numerically by `hw1f_lsmc.rs` inline tests.
- Strengthen rather than delete (conditionally-vacuous, only integration coverage): `private_market_fund/test_private_markets_fund.rs:79 test_preferred_return_calculation` and `:109 test_promote_split` — drop the `if !rows.is_empty()` guards, assert unconditionally, tighten the promote split to ~1e-4.

### Coverage holes to add

- Entirely untested calculators — major: add MetricContext tests for `WeightRiskCalculator::calculate` (`basket/metrics/weight_risk.rs`, no test module at all: redistribution, clamp-to-1.0, single-constituent) and for `NAV01/Carry01/Hurdle01` (`pe_fund/metrics/{nav01,carry01,hurdle01}.rs`, zero tests: sign direction Carry01<0/Hurdle01>0, ~10% FD magnitude, no-promote-tier safe path).
- Untested pricers — major: `BarrierOptionHestonMcPricer` (`heston_mc_pricer.rs`, no test module) needs a non-degenerate-Heston smoke price, a Feller-violation `Err`, and an `mc_stderr` measure check; lookback `use_gobet_miri=true` MC dispatch (`types.rs:244`) is never exercised — add an analytical-vs-MC convergence test.
- Missing commodity integration files — major: add `tests/instruments/commodity/test_commodity_{asian_option,spread_option,swaption}.rs` exercising the full MarketContext pricing path (ordering/parity/zero-vol intrinsic/validation `Err`/serde), all currently inline-only.
- Commodity-option core metrics — major: in `commodity_option/metrics.rs` add Delta/Vega tests (call δ∈(0,1), put δ∈(-1,0), vega>0) and DV01/BucketedDV01 (finite, long-call DV01<0 native dPV/dy, bucketed sum ≈ parallel); all registered but never requested.
- Autocallable structural gaps — major: add a serde round-trip + `deny_unknown_fields` rejection test using `Autocallable::example()`, and an entry in `equity_dependency_completeness.rs` exercising `market_dependencies()` (div_yield_id Some/None, vol-surface).
- Lookback golden references — major: add JSON scenarios under `tests/golden/data/pricing/lookback_option/` (FixedStrike+Call, FloatingStrike+Put) with Haug/QuantLib expected NPV; whitelisted in `golden/runner.rs:39` but the data dir is absent.
- HW1F validation error paths — major/moderate: cover all four `RateExoticHw1fLsmcPricer::validate_inputs` `Err` branches (`hw1f_lsmc.rs:382-408`) and the empty-event-times guard in `RateExoticHw1fMcPricer::price` (`hw1f_mc.rs:71-75`).
- Variance-swap structural gaps — moderate: add serde round-trip (incl. OHLC `#[serde(default)]` and `deny_unknown_fields`), notional-conversion round-trip + zero-vol guard (`types.rs:165/188`), and a `MetricId::BucketedDv01` test (sum ≈ parallel Dv01).
- Forward-price/cost-of-carry fallback branches — moderate (recurring): exercise the spot×exp(r·t) cost-of-carry branch with no PriceCurve for `CommodityForward` (`types.rs:305`), `CommodityOption` (`types.rs:304`), `pe_fund compute_pv` discount-curve path (`pricer.rs:56/88`), and the `quoted_forward` override (`commodity_option/types.rs:290`).
- Basket validation/exposure branches — moderate: notional/currency-mismatch branch of `Basket::validate()` (`types.rs:337`), `AssetExposureCalculator` Instrument-backed branch (non-zero weight), and lock notional-scaling by strengthening `test_instrument_trait_value` to assert PV ratio == notional ratio.
- Commodity/autocallable error & seasoning paths — moderate: integration `Err` on missing forward/discount curve for commodity forward+swap; `CommoditySwap` non-zero `index_lag_days` and `realized_fixings`; autocallable `CapitalProtection` final payoff and expired-instrument (t≤0) early return; missing-market-data `Err` paths.
- Heston/PDE/MC barrier secondary branches — moderate: PDE up-barrier and put variants (`pde_pricer.rs`), up-barrier monitoring_frequency shift (`pricer.rs:803` covers down only), and MC at-hit rebate vs at-expiry ordering.
- TARN / range-accrual end-to-end economics — moderate: realistic-σ TARN early redemption (mc_pv < par, monotone in σ) and a CallableRangeAccrual case with the short rate fully outside the accrual range (coupon PV ≈ 0).
- HW1F rebasing — moderate: a curve whose `base_date` precedes `as_of` to exercise `rebased_discount_fn`'s forward-rebasing branch (`hw1f_curve.rs:76-80`), never hit by current fixtures.

## non-instrument-and-cross-cutting

Heavy duplication of put-call/parity and "smoke" invariants across the sanity_invariants and quantlib_parity layers, several mislabeled no-assertion tests, and systemic gaps in serde round-trip / schema-stability coverage plus ~120 orphaned test-infrastructure tests that never compile.

### Duplicates to consolidate

- `tests/sanity_invariants/test_equity_option_parity.rs:129` (`test_put_call_parity`), `test_fx_option_parity.rs:161` (`test_fx_put_call_parity`), `test_bond_pricing_parity.rs:187` (`test_bond_price_yield_inverse_relationship`): remove all three — the per-instrument suites (`instruments/equity_option/test_put_call_parity.rs`, `instruments/fx_option/test_put_call_parity.rs`, `instruments/bond/pricing.rs:80`) cover the same invariants over far broader scenarios at tighter tolerances.
- `tests/instruments/swaption/integration/bermudan_integration.rs:68-329`: delete the eight functions that are character-for-character identical to `bermudan_pricing.rs:106-367` (the superset); drop the file and its `pub mod` line if it empties.
- `tests/instruments/swaption/integration/quantlib_parity.rs` implied-vol (546-611), `test_quantlib_parity_vega` (347), `test_quantlib_parity_vol_impact` (279), `payer_receiver.rs:19` (`test_payer_receiver_symmetry`), and `cross_validation.rs:8` (`test_all_greeks_run_together`): remove — each is a weaker/equal subset of `metrics/implied_vol.rs`, `metrics/vega.rs:31`, `invariants.rs:176`, `invariants.rs:39`, and the quantlib full-greeks suite respectively.
- `tests/instruments/cds/test_cds_integration_methods.rs` (whole file, 3 tests), `test_cds_pricing.rs:600` (`test_zero_spread_gives_positive_npv_for_buyer`), `test_cds_market_validation.rs:282` (`test_buyer_seller_zero_sum`): delete — positivity/zero-sum already covered (and tightened) in `test_cds_pricing.rs` / `test_cds_edge_cases.rs`.
- `tests/instruments/fra/validation/market_standards.rs` (lines 73, 86, 99, 112, 130, 148, 174, 199, 228): remove the redundant convention/sign/par-rate/DV01 tests — all subsumed by `construction.rs`, `pricing.rs`, `metrics/dv01.rs`, `metrics/par_rate.rs`, and `quantlib_parity.rs`. Also drop the duplicate `quantlib_parity.rs:411` (`quantlib_parity_fra_standard_tenor_3x6`) and `bucketed_dv01.rs:53` (`test_bucketed_dv01_finite`).
- `tests/instruments/deposit/`: remove the three zero-period copies (`metrics/year_fraction.rs:104`, `par_rate.rs:128`, `dv01.rs:76`) keeping `edge_cases.rs:15`; remove `pricing.rs:53`, `pricing.rs:269`, `pricing.rs:353` (par-rate/negative-rate/DV01-magnitude all dominated by the `metrics/*` and `edge_cases.rs` versions).
- `tests/instruments/convertible/test_pricing_basic.rs:17/31/52` (parity at/in/out): delete — identical formula+assertion to `quantlib_parity.rs:181/211/241`.
- `tests/metrics/determinism.rs:153/186` (`test_asian_option_delta_deterministic`, `..._vega_deterministic`): remove — strict subset of `test_asian_option_all_greeks_deterministic` (219).
- `tests/golden/data/pricing/market_envelope_smoke/usd_deposit_3m_envelope.json`: delete the fixture and empty dir — numerically identical to `deposit/usd_deposit_3m.json` (itself now an envelope) and referenced only by the directory-walk runner.
- `tests/calibration/builder.rs:62` (`plan_and_envelope_serde_roundtrip`), `src/schema.rs:287` (`test_all_schemas_parse_successfully`), `tests/integration/metrics/strict_mode.rs:201` (`test_strict_is_default`): remove — fully covered by `serialization.rs` round-trips, the `.expect()` loads in `test_schema_stubs`, and `test_unknown_metric_fails_strict_mode` respectively.

### Dead / unnecessary tests to remove

- `tests/instruments/swaption/pricing/sabr_model.rs:34` (`test_sabr_vs_black_atm`): the only comparative assert is `rel_diff < 10.0` (1000% tolerance) — remove or replace with a real beta=1/nu→0 → Black76 convergence (≤5%) check.
- `tests/instruments/swaption/metrics/gamma.rs:111` (`test_gamma_decreases_with_time_to_expiry`), `tests/instruments/fra/metrics/theta.rs:114` (`test_theta_sign_convention`), `tests/instruments/deposit/metrics/theta.rs:7/24/69` and `bucketed_dv01.rs:7`: names promise directional/sign checks but bodies only assert finiteness/non-negativity — add the missing directional assertion or remove.
- `tests/instruments/fra/metrics/bucketed_dv01.rs:71/91/146` and `construction.rs:172/185`, plus `market_standards.rs:174/199`: no-effective-assertion (only `contains_key` / getter round-trip / calendar arithmetic on literals) — remove or add a value assertion.
- `tests/instruments/cds/test_cds_edge_cases.rs:550` (`test_integration_fallback_with_invalid_params`), `test_cds_construction.rs:188/168` (negative-spread / zero-notional storage re-reads): remove — behavior covered by `test_negative_spread` and `test_zero_notional`.
- `tests/instruments/convertible/test_greeks.rs:18` (`test_greeks_calculation_success`) and `test_pricing_basic.rs:314` (`test_positive_price`): tautological/`is_ok`-only — dominated by sibling delta/range tests.
- `tests/instruments/deposit/construction.rs:67`, `pricing.rs:223` (`test_npv_matches_value_trait` — compares a method to itself), `pricing.rs:204` (`test_value_trait_implementation`), and `instruments/common/test_helpers.rs:142/643` (`two_years_hence`, `gbp`, both `#[allow(dead_code)]`): remove no-op/tautological code; keep `scaled_tolerance`.
- `tests/cashflows/instrument_bridge.rs` (21 `*_exposes_cashflow_provider_bridge` tests via empty-body `assert_provider`) and `revolving_credit/cashflows.rs:16/46/76` (only `!is_empty`): replace with runtime schedule-content assertions where the cashflow logic is real, delete the placeholder/no-residual ones. Also strip the wall-clock `assert!(elapsed.as_millis() < 100)` from `inflation_linked_bond/test_cashflows.rs:336` (keep `len()==61`).
- `tests/sanity_invariants/test_cds_value_bounds` (`test_cds_parity.rs:240`, 2x-doubled bounds), `test_option_prices_non_negative` (`test_equity_option_parity.rs:297`), `test_fx_option_prices_non_negative` (`test_fx_option_parity.rs:194`): trivially-satisfied non-negativity — remove (covered by property/bound tests and put-call parity).
- `tests/support/{attribution_test_utils.rs, convertible_fixtures.rs}` and `tests/instruments/common/{helpers,metrics}/mod.rs` (empty stubs) plus the unused `TestInstrument` structs in `tests/common/test_utils.rs:14` and `tests/support/test_utils.rs:102`: remove orphaned/never-compiled helper code.

### Coverage holes to add

- **Serde round-trip / schema stability (systemic, multiple slices)** — no serialize→deserialize equality or `deny_unknown_fields` rejection test for `Swaption`/`BermudanSwaption`, `ConvertibleBond`, `Deposit`, `ForwardRateAgreement`, full `CreditDefaultSwap`, or a complete generated `CashFlowSchedule`; add per-type round-trip + unknown-field-rejection tests (major).
- **Orphaned/uncompiled test infrastructure (~120 tests)** — add `pub mod pricer;` to `tests/instruments/common/mod.rs` to activate 27 registry tests incl. `test_price_batch_matches_serial_results`; triage `tests/support/` MC/asian/tree-barrier/bermudan files (`asian_option_analytical.rs`, `tree_barrier.rs`, `tree_bermudan_*.rs`, `mc_*`) — migrate the genuinely-uncovered ones, remove the rest; either complete or delete the broken `tests/instruments/common/mc/` module chain (major).
- **Golden fixtures for registered pricing domains with zero coverage** — `exotics.lookback_option`, `rates.cms_swap`, `rates.cms_spread_option` have full pricers but no golden fixture; add one formula-source fixture each (degenerate lookback→vanilla, ATM CMS swap, ATM CMS spread option) (major).
- **JSON pricing public API + serialization roundtrip variants** — none of `pricer::json` entry points (`price_instrument_json`, `canonical_instrument_json`, `validate_instrument_json`, `list_standard_metrics`, `parse_model_key`) are exercised; `serialization/instrument_roundtrip.rs` omits FxSpot/Tarn/Snowball/BermudanSwaption/CmsSpreadOption/CallableRangeAccrual. Add an e2e json-pricing test (happy + error paths) and extend the roundtrip to all `InstrumentJson` variants (major).
- **Untested instrument behavior with real branching** — Convertible `MandatoryVariable` conversion + inverted-bounds error and anti-dilution `effective_conversion_ratio` (FullRatchet/WeightedAverage); convertible registered metrics never requested (`conversion_value`, `bond_floor`, `ImpliedVol`, `Oas`, cross-gammas, etc.); FRA `observed_fixing` post-fixing path + `validate()` 5 error cases; CDS `DefaultExposure` metric. Add focused tests for each (major).
- **External reference parity gaps** — no QuantLib CDS NPV+CS01 fixture in `test_quantlib_external_parity.rs` (only Bond/IRS/FX-forward); no normal (Bachelier) implied-vol round-trip; no European Hull-White 1F integration test. Add these against documented tolerances (moderate).
- **Determinism / serial≡parallel cross-instrument** — no sanity-layer test asserts bit-identical repeated/serial-vs-parallel `value()` for Bond/IRS/CDS/EquityOption/FxOption, nor `PricerRegistry::price_batch` == serial at the integration tier; add a cross-instrument determinism + batch-vs-serial test (moderate).
- **Calibration paths with no end-to-end coverage** — YoY inflation (`StepParams::Inflation` YoY), cross-currency basis repricing, and HW fitted-parameter accuracy/repricing run only via inline unit tests or are skipped; add `engine::execute` repricing tests. Also add a `fail_on_bad_fit=true` Err-gating test and an `execute_with_diagnostics` `SolverNotConverged` end-to-end (moderate).
- **walk.rs / schema-audit negative paths** — no negative tests for `validate_required_pricing_risk_metrics` (rates/fixed_income missing dv01, credit missing dv01/cs01), missing-screenshot rule, invalid source, `validate_sabr_body` rules, shifted-SABR (negative-rates) golden fixture + serde, or `validate_instrument_type_json`/`contract_specs` unknown-ID errors; FX-only schema-drift coverage should extend to one type per asset class. Add the negative cases (moderate).
- **Cashflow schedule content (runtime)** — FxForward/Ndf/FxSwap two-currency flows, CDS seller-premium sign + upfront `Fee` flow, ConvertibleBond/AgencyMBS/TBA/CMO/BasisSwap/CmsSwap/InflationSwap/YoY schedules are only compile-checked via the bridge; add `verify_provider_contract`-style schedule assertions (moderate).
- **Convention/error constructors and curve dependencies** — `Deposit::from_conventions` / `FRA::from_conventions` unknown-index error paths, FRA `CurveDependencies`/`Deposit::market_dependencies`/`Swaption::market_dependencies` completeness, deposit `DfEndFromQuote`/`QuoteRate` guard branches. Add targeted tests (moderate→minor).

---

## Suggested execution order

1. **Verified quick wins** (4 items above) — delete 2 dead files, wire up the orphaned `pricer` module, delete the `cfg(any())` block. Small, safe, immediate.
2. **Dead/tautological removals** — sweep the `is_ok()`/`is_finite()`-only and constant-vs-itself tests bucket by bucket; many are listed with exact `file:line`. Strip the two wall-clock assertions.
3. **Duplicate consolidation** — collapse the parity/golden/spec duplicate layers (notably swaption `bermudan_integration.rs`, the sanity_invariants parity trio, FRA/deposit market-standard subsumptions).
4. **Major coverage holes** — prioritize the registered-but-untested metric families (largest gap), then serde `deny_unknown_fields` round-trips, serial≡parallel determinism, and the missing alt-model pricer / golden-fixture coverage.

## Caveats

- Counts are pre-consolidation; treat each `file:line` as a candidate to confirm at edit time (verifier already rejected 95 unsafe removals, but line numbers can drift).
- "strengthen rather than delete" items are flagged explicitly — don't blanket-delete weak tests where they are the *only* coverage of a path.
- The orphan-file findings (`revolving_credit.rs`, `pricer/`) are confirmed; a mod-graph sweep is recommended to catch any others the compiler can't.
