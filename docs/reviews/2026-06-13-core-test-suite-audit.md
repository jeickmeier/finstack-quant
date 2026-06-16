# Finstack Quant Core Crate — Test Suite Audit

**Date:** 2026-06-13
**Scope:** `finstack-quant/core` test suite — ~2,359 test functions (1,269 inline `#[cfg(test)]` in `src/`, 1,090 integration in `tests/`, ~27k lines).
**Method:** 16 parallel domain auditors read both the inline `src/` tests and the owned `tests/` integration files, then each domain's findings were checked by an independent adversarial verifier that re-confirmed every removal candidate against the actual code and re-checked each coverage hole against the *entire* test tree. Of 247 verified claims, 204 were confirmed, 34 downgraded, and 9 rejected (false positives). Removal line-references were additionally spot-checked by hand.

> This is an **analysis document**. No tests were changed. Every removal cites exact `path:line`; verify before deleting.

## Executive summary

| Category | Count | Notes |
|---|---|---|
| Clean removals (duplicates + dead) | **66** | Verifier `confirmed`; safe to delete |
| Consolidations / assertion-tightening | **27** | Verifier `downgraded`; keep-one-of-pair or strengthen, not a blind delete |
| Coverage holes — High | **18** | Untested public API on a path that can corrupt financial output |
| Coverage holes — Medium | **63** | Untested error branch / edge case / serde-stability invariant |
| Coverage holes — Low | **67** | Nice-to-have wire-format and convention pins |
| False positives caught by verification | **9** | Listed at the end — do **not** act on |

### Dominant themes

1. **Inline-vs-integration duplication.** The single largest source of removable tests: a `#[cfg(test)]` test in `src/` re-asserts exactly what an integration test in `tests/` already covers (FX providers, explain opts, rate arithmetic, curve discounting, diff shifts). The integration copy is almost always the superset; the inline copy is the removable one.
2. **Hardcoded duplicates of JSON-driven golden suites.** Realized-variance, day-count, and SVI/SABR golden values are asserted both in a provenance-tracked JSON fixture *and* in hand-written tests. Keep the JSON suite (source of truth), drop the hand-written twin.
3. **Derive-only / tautological tests.** Several tests exercise only `#[derive(Clone/Copy)]`, re-assert a literal they just wrote, or have empty bodies / both-branches-pass `match`es that can never fail.
4. **Untested error branches are the #1 hole.** Most `Result`-returning public functions have happy-path tests but never trigger their `Err` arms (dimension mismatches, non-finite/zero guards, unsupported-version, inverted date ranges). These guards protect against silent NaN/wrong-number propagation.
5. **Serde-stability gaps.** Multiple `deny_unknown_fields` / custom `TryFrom` wire formats (VolCube, MarketContextState envelope, ScalarTimeSeries, Money, characteristic functions, CreditRating) have no field-name golden or reject-unknown test, despite the project's serde-stability standard.

---

## Part A — Tests to remove

### A.1 Clean removals (verifier-confirmed)

#### Math — solvers, summation, linalg, integration

- **[duplicate]** Newton sqrt(2) for f(x)=x^2-2 duplicated across tests/math/solver.rs:131 (newton::finds_root_simple_quadratic, FD solve()), tests/math/solver.rs:146 (newton::with_analytical_derivative, solve_with_derivative), and inline src/math/solver.rs:1136 (test_newton_solver, solve()). Recommendation: remove inline test_newton_solver as pure duplicate of the FD integration case.
  - *Action:* Confirm. Remove inline src/math/solver.rs:1136 test_newton_solver; keep tests/math/solver.rs:131 (FD) and :146 (analytic-derivative, NOT a duplicate).
- **[duplicate]** Transcendental e^x-3x=0 solved by NewtonSolver with analytic derivative duplicated across tests/math/solver.rs:225 (newton::transcendental_equation) and inline src/math/solver.rs:1406 (test_solve_with_derivative_exponential).
  - *Action:* Confirm. Drop one — keep tests/math/solver.rs:225; the inline :1406 adds only a trivial range bound that could be folded in if desired.
- **[duplicate]** adaptive_simpson on sin(10x) over [0,pi] duplicated: tests/math/integration.rs:58 (test_adaptive_simpson_oscillatory) vs inline src/math/integration.rs:1232 (test_adaptive_simpson).
  - *Action:* Confirm removal of inline src/math/integration.rs:1232. It is a weaker, same-function/interval duplicate AND its assertion comment is wrong (claims exact=0). The integration.rs:58 version pins the correct value 0.2.
- **[duplicate]** test_neumaier_accumulator_copy (tests/math/summation.rs:202) and test_neumaier_accumulator_clone (tests/math/summation.rs:257) both move the accumulator after one add() and assert total() preserved; the 'clone' test's comment even says 'Copy, not clone'.
  - *Action:* Confirm. Delete test_neumaier_accumulator_clone (summation.rs:257) as an exact duplicate of test_neumaier_accumulator_copy (summation.rs:202).
- **[dead]** tests/math/summation.rs:257 test_neumaier_accumulator_clone is a duplicate of the copy test and only tests derived Copy (comment notes 'Copy, not clone').
  - *Action:* Confirm removal of summation.rs:257.
- **[dead]** src/math/probability.rs:419 test_sample_from_uniform first assertion `assert!(x1==1 || x2==1 || x1==0 || x2==0)` is tautological (every u8 outcome is 0 or 1); only the u=0.99->(0,0) assertion tests real behavior.
  - *Action:* Confirm. Replace the tautological first assertion with assert_eq!((x1,x2),(1,1)) for u=0.0 and keep the u=0.99->(0,0) check; ideally add the (1,0)/(0,1) middle-bucket cases (see hole).

#### Math — statistics, distributions, characteristic functions

- **[duplicate]** parkinson_variance_golden (tests/math/stats.rs:88) duplicates the JSON golden case parkinson_two_day (tests/golden/data/realized_variance.json) driven by test_realized_variance_golden (tests/golden/variance_tests.rs:47).
  - *Action:* Keep the JSON-driven golden case (canonical, provenance-tracked). Remove parkinson_variance_golden, or repurpose to a >2-bar input for genuinely new coverage. Net: removable duplicate.
- **[duplicate]** garman_klass_variance_golden (tests/math/stats.rs:137) duplicates JSON golden case garman_klass_two_day.
  - *Action:* Keep JSON golden case; drop garman_klass_variance_golden or retarget inputs. Net: removable duplicate.
- **[duplicate]** yang_zhang_includes_open_to_close_component (tests/math/stats.rs:193) duplicates JSON golden case yang_zhang_four_day.
  - *Action:* Golden JSON covers this input. Delete the inline test or retarget it to a behavioral property the golden case omits (e.g. YZ != Rogers-Satchell, or n<3 returns 0). Net: removable duplicate (medium confidence, since it documents a convention; safe to fold that note into the golden JSON notes).

#### Math — interpolation

- **[duplicate]** flat_fwd_basic (interp.rs:182) instantiates interp_basic_tests!() on Interpolator<LogLinearStrategy>, identical to log_linear_df_basic (interp.rs:181). flat_fwd is a legacy alias; no FlatForwardStrategy type exists.
  - *Action:* Confirm removal of flat_fwd_basic (interp.rs:182). Fully redundant with log_linear_df_basic.
- **[duplicate]** extrap_flat_fwd (interp.rs:429) instantiates extrapolation_tests!() on Interpolator<LogLinearStrategy>, identical to extrap_log_linear (interp.rs:428).
  - *Action:* Confirm removal of extrap_flat_fwd (interp.rs:429).
- **[duplicate]** deriv_flat_fwd (interp.rs:586) instantiates derivative_tests!() on Interpolator<LogLinearStrategy>, identical to deriv_log_linear (interp.rs:585).
  - *Action:* Confirm removal of deriv_flat_fwd (interp.rs:586).
- **[duplicate]** derivative_epsilon_defined duplicated verbatim: inline at types.rs:701 and integration at interp.rs:1717; both assert DERIVATIVE_EPSILON == 1e-6.
  - *Action:* Confirm: keep one copy (inline types.rs preferred for locality), delete interp.rs:1717.
- **[duplicate]** extrapolation_default_is_flat_zero (types.rs:668) and default_is_flat_zero (interp.rs:1637) both assert ExtrapolationPolicy::default() == FlatZero with identical bodies.
  - *Action:* Confirm: keep one (inline types.rs), delete interp.rs:1637.
- **[dead]** flat_fwd_specific::matches_log_linear_exactly (interp.rs:703) builds two Interpolator<LogLinearStrategy> from identical inputs and asserts equality -- tautology; claims LogLinear-vs-FlatForward parity but FlatForward is not a distinct strategy.
  - *Action:* Confirm deletion (interp.rs:702-726). Tautological; provides no coverage.
- **[dead]** interp_style_build::build_flat_fwd (interp.rs:274) builds InterpStyle::LogLinear with FlatZero, identical to build_log_linear (interp.rs:214); only the name differs.
  - *Action:* Confirm deletion (interp.rs:274). Redundant with build_log_linear.

#### Math — RNG / Sobol / Brownian bridge

- **[duplicate]** test_pca_ordering_identity (brownian_bridge.rs:435) duplicates test_pca_identity_matrix (sobol_pca.rs:174): both feed a 3x3 identity correlation to pca_ordering and assert all eigenvalues == 1.0 to 0.01 tolerance; sobol_pca version is strictly stronger (also asserts d_eff ~= 3). Recommendation: remove from brownian_bridge.rs.
  - *Action:* Remove test_pca_ordering_identity from brownian_bridge.rs; keep test_pca_identity_matrix in sobol_pca.rs (its home module and strictly stronger).
- **[duplicate]** test_effective_dimension (brownian_bridge.rs:471) duplicates test_effective_dimension_bounds (sobol_pca.rs:210): both assert effective_dimension([1,1,1]) ~= 3.0 and a dominant-eigenvalue vector yields d_eff close to 1; sobol_pca version stronger. Recommendation: remove from brownian_bridge.rs.
  - *Action:* Remove test_effective_dimension from brownian_bridge.rs; keep test_effective_dimension_bounds in sobol_pca.rs.
- **[dead]** test_box_muller_transform (random.rs:370): the leading box_muller_transform(0.5,0.5)->is_finite smoke assertions are subsumed by the dedicated boundary tests (lines 401, 409) and add no coverage; the 500-sample statistical portion is legitimate and should stay. Drop only the leading is_finite smoke lines.
  - *Action:* Optional low-priority cleanup: drop only lines 371-373 (the (0.5,0.5) is_finite smoke asserts); keep the statistical block. Not a correctness risk; safe to leave as-is.

#### Market data — term-structure curves

- **[duplicate]** Inline test_flat_curve_discounting (flat.rs:91, rate 0.10/Act365F, asserts df(0)=1 and df(1)=e^-0.1) is a strict subset of integration tests test_flat_curve_discounting_zero_time (flat_tests.rs:18, df(0)=1) and test_flat_curve_discounting_various_tenors (flat_tests.rs:27, df(1)=e^-0.1 plus more tenors). Delete the inline duplicate.
  - *Action:* Delete inline test_flat_curve_discounting in flat.rs:91-101; integration tests + doctest fully cover it.
- **[duplicate]** builder_rejects_empty_knots exists in both src inline (hazard_curve.rs:1186) and integration (hazard.rs:23); both build HazardCurve::builder with no knots and assert build() errors. Keep only one.
  - *Action:* Keep the integration test (hazard.rs:23, matches Error::Input) and delete the inline duplicate (hazard_curve.rs:1186).
- **[duplicate]** interpolation_consistency_at_knot_points (discount.rs:204) and interpolation_styles_produce_valid_results (discount.rs:250) iterate style lists with InterpStyle::LogLinear duplicated; same pattern in forward.rs:217-222 and inflation.rs:236-241.
  - *Action:* De-duplicate the LogLinear entries in all three style loops. Bonus: the freed slot should likely test PiecewiseQuadraticForward, which these loops never cover.
- **[dead]** test_flat_curve_serde_not_implemented (flat_tests.rs:200) has an empty body — only comments, no assertions; can never fail. Delete it.
  - *Action:* Delete the empty test. A compile_fail doctest would carry real signal if documenting serde absence matters; an empty #[test] does not.

#### Market data — vol surfaces / arbitrage

- **[duplicate]** Inline oob_checked_errors (vol_surface.rs:1243) duplicates the OOB block in integration test_vol_surface_value_checked (vol_surface_tests.rs:20, OOB asserts at lines 34-37). Both assert value_checked returns Err for out-of-bounds expiry and strike. Recommendation: remove the inline test.
  - *Action:* Confirmed duplicate. Remove inline oob_checked_errors; test_vol_surface_value_checked is the superset (covers both axes, both directions, plus the in-bounds branch). Safe to delete the inline test.
- **[duplicate]** Inline builder_validation_errors (vol_surface.rs:1384) duplicates integration test_vol_surface_unsorted_expiries (vol_surface_tests.rs:178) and test_vol_surface_builder_validation (vol_surface_tests.rs:144, wrong-row-length at 169-174). Both assert (a) unsorted expiries -> Err and (b) mismatched row length -> Err. Recommendation: remove the inline test.
  - *Action:* Confirmed duplicate. Remove inline builder_validation_errors; the two integration tests cover both its branches and strictly more. Safe to delete the inline test.
- **[dead]** test_vol_surface_clone (vol_surface_tests.rs:423) exercises only the derived #[derive(Clone)] on VolSurface; it clones and asserts id/expiries/strikes and one interpolated value match. No finstack-quant-specific Clone logic, so it can never fail unless the derive is removed. Recommendation: remove (low priority).
  - *Action:* Confirmed as a near-tautological derive test, but it is harmless and the prior agent's low-priority framing is correct. Removal is optional/judgment-call, not a strong recommendation — do NOT prioritize deletion. If kept, no harm; if cleaned up, fold nothing (no real invariant lost).

#### Market data — context, bumps, diff, hierarchy

- **[duplicate]** test_parallel_discount_shift (diff.rs:646) duplicates test_discount_curve_parallel_shift (diff_tests.rs:80): same USD-OIS curve, same +50bp via exp(-0.005*t), same (shift-50).abs()<5.0, same measure_discount_curve_shift + Standard.
  - *Action:* Confirm: remove the inline duplicate test_parallel_discount_shift; keep the integration test.
- **[duplicate]** test_hazard_curve_shift (diff.rs:687) duplicates test_hazard_curve_parallel_shift (diff_tests.rs:255): identical CORP-01 hazard curve (recovery 0.4, knots (1,0.01)/(5,0.02)/(10,0.025) -> +25bp), both assert (shift-25).abs()<1.0 via measure_hazard_curve_shift + Standard.
  - *Action:* Confirm: drop the inline duplicate; keep the integration test.
- **[duplicate]** test_missing_curve_error (diff.rs:866) duplicates test_discount_curve_missing_error (diff_tests.rs:179): two empty contexts, measure_discount_curve_shift("MISSING",..), assert is_err().
  - *Action:* Confirm: remove the inline duplicate; keep the integration test.
- **[duplicate]** test_tenor_sampling_methods (diff.rs:888) duplicates test_tenor_sampling_with_all_methods (diff_tests.rs:652): both run Standard/Dynamic/Custom against a self-compared market and assert each shift == 0.0.
  - *Action:* Confirm (medium): remove the inline near-duplicate; the integration version with the richer curve subsumes it.
- **[dead]** node_path_is_vec_of_strings (hierarchy.rs:21) is tautological: constructs a Vec<String> literal and asserts .len()==2 and path[0]=="Rates"; invokes no finstack hierarchy logic.
  - *Action:* Confirm: delete. It tests std library behavior only.

#### Market data — scalars, dividends, fixings, DTSM

- **[duplicate]** primitives.rs:563 test_scalar_time_series_empty_error and primitives.rs:637 test_scalar_time_series_error_message_quality both call ScalarTimeSeries::new("TEST", vec![], None) and assert the same empty-input error path; remove the message_quality one.
  - *Action:* Keep test_scalar_time_series_empty_error. Remove test_scalar_time_series_error_message_quality, or repurpose it to assert a specific stable substring (e.g. "too few") rather than length > 10. Low-risk removal.
- **[dead]** primitives.rs:577 test_scalar_time_series_single_point_error asserts nothing definite — its match accepts BOTH Ok(1 obs) AND Err(TooFewPoints), so it can never fail.
  - *Action:* Rewrite to assert the real contract: single-point series succeeds with len()==1 (storage rejects only empty/duplicates). Remove the both-branches-pass structure. Do not simply delete — single-point acceptance is a real invariant worth locking.

#### Dates

- **[duplicate]** Span-rule behavior duplicated: rules.rs rule_span_basic/rule_span_crossing_year_boundary/rule_span_zero_length/rule_span_single_day/rule_span_materialize_year (tests/dates/rules.rs:752-857) vs rules_coverage.rs span_rule_cases (tests/dates/rules_coverage.rs:41-126).
  - *Action:* Confirmed for the five named tests. Keep one representation (the table-driven span_rule_cases is more compact) and delete the five individual span tests in rules.rs, BUT preserve span_len2_cross_year and span_len3_cross_year — they exercise the distinct &[Rule]::is_holiday slice trait path, not just Rule::applies.
- **[duplicate]** Chinese New Year duplicated: rules.rs rule_chinese_new_year(517)+rule_chinese_new_year_materialize(527)+rule_chinese_new_year_known_dates(549) vs rules_coverage.rs chinese_new_year_rules(183).
  - *Action:* Confirmed. Consolidate into a single CNY test holding the materialize-yields-one-Jan/Feb invariant plus the 2020-2025 known-date list (keep one of the two locations).
- **[duplicate]** Qing Ming materialize duplicated: rules.rs rule_qing_ming_materialize(587) vs rules_coverage.rs qing_ming_rules(167).
  - *Action:* Confirmed. Drop one; keep a single Qing Ming materialize-invariant test.
- **[duplicate]** Buddha's Birthday duplicated: rules.rs rule_buddhas_birthday_materialize(623)/rule_buddhas_birthday_applies(651) vs rules_coverage.rs buddhas_birthday_rules(149).
  - *Action:* Confirmed. Keep rules_coverage.rs buddhas_birthday_rules (combines materialize + applies/prev/next over the full year range); drop the two rules.rs tests.
- **[duplicate]** Autumnal equinox + Easter offset (Ascension/Whit) + nth-weekday (5th Mon Dec, 2nd-to-last Fri Nov) + weekday-shift (Election Day, Fri-before-Jun-15) duplicated between rules.rs and rules_coverage.rs.
  - *Action:* Confirmed. Pick one file as the canonical rule-behavior suite and remove the overlapping cases from the other. Genuinely UNIQUE cases in rules_coverage.rs that must be preserved: equinox_rules_out_of_supported_range_do_not_fabricate_dates (228), observed_variants (302), direction_same_day (327), fixed_feb_29_rules (207). Unique in rules.rs: span_len2/len3_cross_year, rule_fixed_observance_crosses_year_boundary (1001), rule_nth_weekday_overflow_does_not_spill_into_next_month (1036), rule_easter_known_dates_2020_2030 (398), rule_good_friday/easter_sunday diff tests.
- **[dead]** tests/dates/rules_serde.rs:176 test_span_rule_skipped is an empty placeholder exercising no code.
  - *Action:* Confirmed dead. Delete it, or replace with a real test that a Vec<Rule> containing only serializable variants round-trips and that #[serde(skip)] on Span does not break sibling-variant serialization (the project standard prefers fixing root cause over deleting, so a real serde-skip assertion is the better replacement).
- **[dead]** src/dates/imm.rs:776 sifma_with_calendar_matches_algorithmic asserts only d1.is_some() and never compares calendar vs algorithmic paths.
  - *Action:* Confirmed near-dead (only guards against None for a table year). Either assert the exact published Mar-2026 Class B date, or genuinely compare the calendar path against the algorithmic fallback for a covered year. As written it adds almost no signal beyond the existing sifma_class_*_from_calendar tests.
- **[dead]** tests/dates/rules_serde.rs:153 test_rule_collection_serde only asserts rules.len()==deserialized.len(), a near-tautology for Vec serde.
  - *Action:* Confirmed weak/near-tautological. Strengthen to assert per-element variant/field equality (as test_rule_serde_roundtrip already does) or delete it; test_rule_serde_roundtrip already covers the substantive case.

#### Cashflow

- **[duplicate]** irr_multiple_sign_changes_is_not_rejected_as_ambiguous (tests/cashflow/irr.rs:473/474) and irr_attempts_ambiguous_multi_root_cashflows (tests/cashflow/irr.rs:753/754) are byte-for-byte identical: same input [-100.0, 320.0, -320.0, 100.0], same irr(&amounts, None) call, same 'multiple sign changes' Err-branch assertion.
  - *Action:* Delete one (keep irr_multiple_sign_changes_is_not_rejected_as_ambiguous, clearer comment). They are a pure copy.
- **[duplicate]** Inline test_xirr_with_daycount_act360 (xirr.rs:744/745) and integration xirr_daycount_act365f_vs_act360 (tests/cashflow/irr.rs:274/275) both use the same -100_000 (2024-01-01) -> +102_500 (2024-07-01) 6-month flow and assert both day-counts positive plus the 360/365 relationship; the integration test is a strict superset.
  - *Action:* Drop the inline test_xirr_with_daycount_act360 (xirr.rs:744); the integration xirr_daycount_act365f_vs_act360 covers the same path more tightly. Low risk.
- **[duplicate]** npv_errors_on_empty_flows (discounting.rs:682/683, ZeroRateCurve) and test_npv_errors_on_empty_flows_with_flat_curve (discounting.rs:874/875, FlatCurve) both assert npv() returns Err on empty flows; the empty-slice guard runs before any curve interaction so the curve is irrelevant.
  - *Action:* Keep one (test_npv_errors_on_empty_flows_with_flat_curve). The choice of curve is irrelevant to the empty-input guard.
- **[dead]** floating_cf_defaults_reset_date_to_payment (tests/cashflow/primitives.rs:80/81) constructs a CashFlow with reset_date: Some(payment) then asserts cf.reset_date == Some(payment) — a literal round-trip with no transform, no validate(), no defaulting logic; the name claims a 'defaults' behavior that does not exist.
  - *Action:* Delete floating_cf_defaults_reset_date_to_payment. The reset-date semantics it purports to cover are already covered by cashflow_accepts_reset_date_equal_to_payment (validate-based) and the after-payment invalid case.

#### Money / currency / FX

- **[duplicate]** Inline simple_provider_identity (providers.rs:352) duplicates integration simple_fx_provider_identity_rates (fx.rs:28); both assert SimpleFxProvider returns 1.0 for identical-currency lookups via the same from==to early-return path.
  - *Action:* Confirmed duplicate. Keep the integration test simple_fx_provider_identity_rates (broader); remove inline simple_provider_identity.
- **[duplicate]** Inline simple_provider_direct_quote (providers.rs:316) duplicates the direct-quote store-and-lookup path covered by integration tests at fx.rs:53.
  - *Action:* Confirmed duplicate; the direct-quote rate() resolution is already asserted at fx.rs:88 (simple_fx_provider_reciprocal_fallback). Remove inline simple_provider_direct_quote.
- **[duplicate]** Inline simple_provider_reciprocal (providers.rs:335) duplicates integration simple_fx_provider_reciprocal_fallback (fx.rs:72); both assert the reciprocal-fallback path, only the literal rate differs.
  - *Action:* Confirmed duplicate. Keep integration simple_fx_provider_reciprocal_fallback; remove inline simple_provider_reciprocal.
- **[duplicate]** Inline simple_provider_not_found (providers.rs:368) duplicates integration simple_fx_provider_new_empty (fx.rs:14); both assert an empty SimpleFxProvider returns Err for a non-identity pair.
  - *Action:* Confirmed duplicate. Keep integration simple_fx_provider_new_empty; remove inline simple_provider_not_found.
- **[duplicate]** Inline currency_mismatch_error (types.rs:835) duplicates integration cross_currency_add_fails_without_convert (money_fx.rs:41); both assert USD.checked_add(EUR).is_err().
  - *Action:* Confirmed duplicate. The inline unit test in types.rs is the canonical home; keep one. Either is removable since both are identical. (Lowest-risk: drop the integration copy, keep the colocated unit test.)
- **[dead]** try_new_handles_large_finite_values (types.rs:1029) only asserts amount()>0.0 for 1e15 — a near-tautological smoke check that overlaps existing large-value regression tests.
  - *Action:* Confirmed weak. Tighten to assert amount()==1e15 (exact), or remove as redundant with amount_does_not_silently_return_zero_for_large_values.

#### Expression engine

- **[duplicate]** Inline test src/expr/ast.rs:565 `expr_id_is_ignored_for_hash_and_equality` duplicates integration tests tests/expr/ast.rs:89 `equality_ignores_id` and tests/expr/ast.rs:112 `hash_ignores_id` (same id-independence invariant on the PartialEq/Hash impl).
  - *Action:* Confirmed. Remove the inline duplicate at src/expr/ast.rs:565 (or repurpose to cover Literal NaN-bit equality, which neither integration test covers). Low risk: id-independence remains covered by two integration tests.
- **[duplicate]** Inline tests src/expr/context.rs:55 and :64 duplicate integration tests/expr/context.rs:11 `simple_context_basic_usage` and :44 `simple_context_duplicate_names`.
  - *Action:* Confirmed. Drop the two inline tests at src/expr/context.rs:55 and :64. Behavior fully covered (and exceeded) by tests/expr/context.rs.
- **[duplicate]** Inline src/expr/eval.rs:705 `eval_with_plan_and_cache_executes_rolling_functions` duplicates integration tests/expr/eval.rs:259 `with_cache_configuration` (both: with_planning + with_cache(1) + cache_budget_mb=Some(1) + RollingSum window=2, assert result[0]=NaN then rolling-sum progression).
  - *Action:* Confirmed (medium). Keep the integration test. Remove or re-scope the inline test at src/expr/eval.rs:705 to assert the no-op cache contract (has_cache()==false and results equal no-cache eval) rather than re-checking rolling-sum arithmetic already covered by rolling_sum_basic/rolling_sum_window_3.

#### Types, errors, config, validation, explain, table

- **[duplicate]** test_explain_opts_default_is_disabled is byte-for-byte identical in src/explain.rs:231 (inline) and tests/infrastructure/explain.rs:6 (integration). Recommendation: delete the inline src copy.
  - *Action:* Confirm. Delete inline src/explain.rs:231; keep tests/infrastructure/explain.rs:6.
- **[duplicate]** test_explain_opts_enabled is byte-for-byte identical in src/explain.rs:238 and tests/infrastructure/explain.rs:13.
  - *Action:* Confirm. Delete inline src/explain.rs:238; keep the integration copy.
- **[duplicate]** Three tests assert the same truncation invariant for CalibrationIteration: src test_trace_push_respects_limits (max=3,5 pushes), integration test_explanation_trace_size_cap (max=3,10 pushes), property_trace_entries_never_exceed_max (max=5,100 pushes). Drop the inline src duplicate.
  - *Action:* Confirm (medium). Drop inline src/explain.rs:245; keep the integration bounded test (:20) and the property test (:133).
- **[duplicate]** Inline display_formatting (rates.rs:946) duplicates rate_display_formatting/bps_display_formatting/percentage_display_formatting which fully supersede it (and add negatives).
  - *Action:* Confirm (high). Delete inline src/types/rates.rs:946 display_formatting.
- **[duplicate]** percentage_arithmetic (rates.rs:917) and percentage_arithmetic_operations (tests:176) are identical.
  - *Action:* Confirm (high). Delete inline src/types/rates.rs:917; keep the integration copy.
- **[duplicate]** bps_arithmetic (rates.rs:897) and bps_arithmetic_operations (tests:106) cover the same operators on Bps(100)/Bps(50); only scalar mul differs (x2 inline vs x3 integration).
  - *Action:* Confirm (medium). Delete inline src/types/rates.rs:897; the integration version covers the same operators.
- **[duplicate]** test_results_meta_default_stamping (metadata.rs:7) and test_results_meta_default_impl (metadata.rs:81) assert the same three properties via the identical code path; merge, keeping default_stamping.
  - *Action:* Confirm (medium). Drop test_results_meta_default_impl (:81); keep test_results_meta_default_stamping (:7), which additionally asserts non-empty version.
- **[dead]** all_model_versions_are_nonempty_and_distinct (versions.rs:66) only checks five hand-written const &str literals for non-emptiness and pairwise distinctness — almost no defect-catching value.
  - *Action:* Confirm at low severity. Acceptable to keep as a cheap drift guard for the audit-trail version strings; removal would not lose coverage of any logic but is not worth the churn.

#### Cross-cutting — serde golden, QuantLib golden, canonical API

- **[duplicate]** Gauss-Hermite moment identities E[X^2]=1 / E[X^4]=3 asserted in 4 places; inline test_gauss_hermite_new_integration (integration.rs:1267) E[X^2]=1 is fully subsumed by canonical_api.rs quadrature_integration_correctness; keep the two integrate_adaptive tests (distinct path).
  - *Action:* Dropping inline test_gauss_hermite_new_integration is safe; its single-order E[X^2]=1 check is subsumed by the canonical order-sweep. Keep the two adaptive-path tests and the canonical sweep.
- **[duplicate]** 30/360 US EOM edge cases triplicated: hardcoded test_thirty360_us_eom_edge_cases (daycount_quantlib_tests.rs:241), the JSON QuantLib suite (driven by test_daycount_quantlib_parity), and dates/daycount.rs unit tests. Recommend deleting the hardcoded function.
  - *Action:* Delete hardcoded test_thirty360_us_eom_edge_cases; the JSON-driven parity suite is the QuantLib source-of-truth and dates/daycount.rs holds unit coverage for the identical four cases.
- **[duplicate]** 30E/360 vs US Feb-28-EOM distinction duplicated: hardcoded test_thirty_e360_no_feb_eom_rule (daycount_quantlib_tests.rs:292) vs dates/daycount.rs thirty_e360_feb28_not_adjusted (274) and thirty_e360_vs_us_difference (202). Recommend deleting the hardcoded function.
  - *Action:* Delete hardcoded test_thirty_e360_no_feb_eom_rule; assertions are fully covered by dates/daycount.rs unit tests (and the JSON suite for the US case).
- **[duplicate]** canonical_api.rs dated_irr_uses_act365f_default (147) and simplicity_parity.rs xirr_trait_matches_ctx_helper_on_act365f_default (273) both pin that xirr(flows, None) uses Act365F default; keep only the deeper simplicity_parity one.
  - *Action:* Keep one default-delegation guard (prefer simplicity_parity:273, deepest target) and remove the other to cut the duplicate, OR keep both but they must be tightened to exact equality (see dead-finding verdicts).

### A.2 Consolidate or tighten (verifier-downgraded — do not blind-delete)

#### Math — solvers, summation, linalg, integration

- BracketHint->size mapping duplicated: tests/math/solver.rs:339-372 assert solver.initial_bracket_size==Some(size) after .bracket_hint(...); inline src/math/solver.rs:1514 (test_all_bracket_hints) asserts Hint::*.to_bracket_size()==size; plus the size assertion in test_bracket_hint_xirr (src:1485).
  - *Action:* Downgrade. The constant-table test overlaps but is not fully redundant (it uniquely pins Xirr=0.5). If consolidating, fold the per-hint constant checks into test_all_bracket_hints and keep the builder-wiring integration tests; do NOT delete test_all_bracket_hints wholesale.
- GaussHermiteQuadrature serde round-trip duplicated: tests/math/solver.rs:440-468 (order_5/7/10) vs tests/math/integration.rs:514 (test_gauss_hermite_serde covering 5/7/10).
  - *Action:* Downgrade to a cleanup, not a clean delete. Removing solver.rs order_5/7/10 is acceptable since integration.rs:514 covers 5/7/10 with stronger assertions, but preserve order_10's JSON "order":5 check (or move it) and keep order_15/order_20.
- tests/math/summation.rs:202 test_neumaier_accumulator_copy only exercises the derived Copy (`let acc_copy = acc;` then asserts total()==5.5), i.e. compiler-guaranteed derive behavior, not summation logic.
  - *Action:* Downgrade from removal of the whole pair to: delete the twin (:257) as a duplicate; keep OR delete :202 at the maintainer's discretion. The numerical invariant is not lost either way, but :202 is the one to retain if exactly one is kept (it is referenced as canonical by the duplicate verdict).
- tests/math/solver.rs:389 solver_convergence_failed_error_variant constructs InputError::SolverConvergenceFailed with literal fields and asserts Display contains '50' and 'iterations' — re-asserts a literal it just wrote, exercising only thiserror Display, not solver behavior; real convergence path covered by newton_error_contains_iteration_count (solver.rs:254) and inline test_brent_max_iterations_returns_error (src:1423).
  - *Action:* Downgrade to optional removal. It is low-value (Display-format echo) but not strictly tautological. Safe to remove given the two genuine-failure tests already cover the Display contract; not a high-priority deletion.

#### Math — statistics, distributions, characteristic functions

- realized_var_method_display_roundtrip (src/math/stats.rs:1337) is dead/unnecessary — pure Display->FromStr round-trip overlapping realized_var_method_from_str_aliases.
  - *Action:* Reject removal. It is low-value but pins a genuine Display<->FromStr canonical-label contract not fully covered by the aliases test. Keep as a stability pin (the finding itself rated this 'low' confidence and recommended keeping).
- test_norm_pdf non-negativity assertion `assert!(norm_pdf(5.0) >= 0.0)` (src/math/special_functions.rs:580) is dead — tests an impossible-to-violate property.
  - *Action:* Reject removal of the test. Optionally replace the single >=0.0 line with a tail-value assertion to pin the far-tail magnitude. Not a test deletion.

#### Math — interpolation

- interp_style_equality (types.rs:720 vs interp.rs:1752) and interp_style_inequality (types.rs:726 vs interp.rs:1764) duplicated inline vs integration; same PartialEq assertions.
  - *Action:* Do NOT delete the integration copies as recommended. If consolidating, delete the WEAKER inline types.rs copies (lines 719-729) and keep the broader integration copies, or merge the extra variants into the inline copies first.
- derivative_epsilon_usage at interp.rs:1723 (and identical inline at types.rs:706) does an FD of local closure f(x)=x*x and tests std arithmetic, not finstack logic.
  - *Action:* Acceptable to delete, but justify as 'redundant with derivative_epsilon_defined' rather than 'tests nothing real'. At minimum delete one of the two duplicate copies. Low risk.

#### Math — RNG / Sobol / Brownian bridge

- test_pca_ordering_sorted (brownian_bridge.rs:448) duplicates test_pca_high_correlation (sobol_pca.rs:191): both run pca_ordering on a high-correlation 3x3 matrix and assert eigenvalues are sorted descending with a dominant first eigenvalue; sobol_pca version additionally checks d_eff<2.0. Recommendation: remove from brownian_bridge.rs (consider folding its descending-sort assertion into the kept test if not already present).
  - *Action:* Do not delete outright. Either keep test_pca_ordering_sorted, or first add an explicit full descending-sort assertion to test_pca_high_correlation in sobol_pca.rs (the function's home), then remove the bb copy. Only the dominant-first portion is truly redundant.

#### Math — volatility models

- sabr.rs:984 (sabr_try_implied_vol_errors_on_degenerate_inputs) and sabr.rs:1232 (sabr_invalid_inputs_return_nan) both assert implied_vol_lognormal with a negative forward returns NaN; recommend dropping line 984.
  - *Action:* Do not delete either test. Both pin a real invariant. Optionally trim the single redundant assertion at sabr.rs:984 (keep the try_* assertions 985-990), but this is cosmetic and low priority; leaving it is harmless. Keep sabr_invalid_inputs_return_nan as the canonical infallible NaN test.

#### Market data — term-structure curves

- Inline survival_monotone_decreasing (hazard_curve.rs:1095), integration test_hazard_curve_sp (hazard.rs:202), and survival half of survival_and_default_probabilities (hazard.rs:44) all assert the same survival-monotonicity invariant with no distinguishing edge case, subsumed by sp_analytical_verification_constant_hazard. Drop survival_monotone_decreasing and test_hazard_curve_sp.
  - *Action:* test_hazard_curve_sp (hazard.rs:202) is a defensible removal (pure subset). KEEP survival_monotone_decreasing (hazard_curve.rs:1095) for its unique sp(6.0) extrapolation assertion, or fold an explicit extrapolation check into an analytical test before deleting.
- clone_works_for_all_interp_styles / clone_is_panic_free_and_equivalent / clone_works_with_extrapolation (inflation.rs:62; same pattern forward.rs:25/70/107) primarily exercise derived Clone with no custom logic; consider collapsing or dropping.
  - *Action:* Low-value but NOT dead. These re-exercise the interpolator post-clone. Acceptable to collapse three-per-type into one; do not delete outright as dead.

#### Market data — context, bumps, diff, hierarchy

- test_discount_curve_zero_shift (diff_tests.rs:159), test_tenor_sampling_with_all_methods (diff_tests.rs:652) and test_discount_curve_dynamic_sampling (diff_tests.rs:194) all re-assert the same-market zero-shift invariant for discount curves with only trivial knot differences.
  - *Action:* Downgrade: keep test_discount_curve_dynamic_sampling (distinct Dynamic knot-extraction path); the pure Standard zero-shift assertion is mildly redundant with the all-methods test but harmless. Low priority, do not delete.
- test_standard_tenors_constant (diff_tests.rs:643) pins the literal contents of STANDARD_TENORS (len 9, [0]==0.25, [4]==3.0, [8]==30.0); a low-value change-detector, STANDARD_TENORS is not serialized.
  - *Action:* Downgrade: low priority. Acceptable to keep as a cheap guard on a public const, or drop. Not a clear deletion.
- empty_hierarchy_has_no_roots (hierarchy.rs:6) merely verifies the Default/IndexMap is empty; minimal finstack logic.
  - *Action:* Downgrade: keep as a trivial smoke test (it pins the empty-construction invariant via the real API). Not dead; very low value only.

#### Market data — scalars, dividends, fixings, DTSM

- primitives.rs:647 test_scalar_time_series_large_dataset only asserts creation succeeds and observations().len() >= 25 for a ~30-point series; loose tautology / smoke test.
  - *Action:* Do not delete outright (medium-confidence). Strengthen by adding a value_on lookup at an interpolated date with an asserted expected value, converting it into meaningful boundary coverage. Removal would lose only a weak no-panic guard.

#### Credit

- lgd/mod.rs:176 (seniority_recovery_stats_accepts_binding_strings) and registry.rs:488 (registry_preserves_known_agency_values) both pin the S&P senior-secured recovery mean = 0.53; only the access path differs. Low confidence; recommendation is keep-both / trim redundant magnitude assertion.
  - *Action:* Keep BOTH tests; do NOT delete. Optional cosmetic: in lgd/mod.rs assert equality against the registry value rather than re-pinning 0.53, but the binding string-parse contract it covers is unique. No removal warranted.

#### Dates

- Act/365L end-on-Feb29->366 duplicated: inline act365l_period_ending_on_feb29_uses_366 (src daycount.rs:1630) + act365l_single_day_feb29 (tests daycount.rs:425) + integration act365l_period_ending_on_feb29 (tests daycount.rs:349).
  - *Action:* Downgrade from duplicate-deletion to keep-all. The three share intent but exercise distinct day-counts at a numerically sensitive boundary (the 2026-06-09 review fixed a [start,end) vs (start,end] bug here). At most, the inline 28-day case is the weakest, but it is the only INLINE assertion of the fix and is cheap; no deletion recommended.
- src/dates/imm.rs:709 sifma_default_class_is_b only verifies the #[default] derive, overlapping sifma_default_is_class_b.
  - *Action:* Downgrade. sifma_default_class_is_b is a cheap convention-pin of the #[default] attribute (low value but not harmful); it is acceptable to keep. The finding's claim that it overlaps sifma_default_is_class_b is inaccurate — they test different things. No action needed; if trimming, drop the derive-only one, keep the behavioral one.

#### Cashflow

- test_xirr_basic (xirr.rs:619/620), test_unified_irr_api XIRR half (xirr.rs:598/599), and test_xirr_with_daycount_act365f (xirr.rs:724/725) all use the same -100_000 (2024-01-01) -> +110_000 (2025-01-01) flow and assert the same golden expected=(1.1)^(365/366)-1.0; the core XIRR assertion is redundant across all three.
  - *Action:* Keep all three tests. Optionally trim the duplicated golden literal out of test_unified_irr_api so it focuses on the periodic-vs-dated API surface; keep test_xirr_basic as the golden anchor and test_xirr_with_daycount_act365f for the Act365F==default invariant. Do NOT delete any test.
- cashflow_size_is_reasonable (src/cashflow/primitives.rs:481/482) asserts size_of::<CashFlow>() <= 56, pinning a memory-layout implementation detail, not financial correctness or a public invariant.
  - *Action:* Keep. It is a low-value but intentional, documented memory-layout budget for a hot type; deletion is not warranted and removing it loses a regression guard against struct bloat. Not dead code.

#### Money / currency / FX

- try_new_handles_negative_zero (types.rs:1014) effectively tests IEEE -0.0==0.0 (per its own comment) rather than finstack-quant-specific logic.
  - *Action:* Downgrade from 'dead' to weak. Do NOT delete outright; instead strengthen to assert a money-specific canonical form (e.g. serialized amount is "0" not "-0"), or accept as a cheap boundary check. Removal is not airtight.

#### Expression engine

- tests/expr/eval.rs:260 `with_cache_configuration` is unnecessary: with_cache/cache_budget_mb are documented no-ops, the test only re-verifies plain RollingSum arithmetic, asserting nothing about caching.
  - *Action:* Downgraded from dead to a tighten-the-assertion suggestion. Do NOT delete. Add `assert!(!compiled.has_cache())` and assert equality vs a no-cache eval so the no-op contract is genuinely pinned. Overlaps duplicate group 3 above.

#### Types, errors, config, validation, explain, table

- test_attributes_serde_roundtrip (attributes.rs:130) swallows serde errors via is_ok()+unwrap_or_default(), so a serialization failure could silently substitute Attributes::default() and pass vacuously for empty inputs.
  - *Action:* Downgrade from dead to a hygiene nit: the test pins real roundtrip fidelity. Optionally tighten to .expect() and add a deny_unknown_fields negative case. Do not delete.

#### Cross-cutting — serde golden, QuantLib golden, canonical API

- GaussHermiteQuadrature::new() order-validation tested 3x: canonical_api.rs new_supported_orders (239, Ok orders [5,7,10,15,20]+points.len check), new_rejects_unsupported_orders (314, Err list), inline integration.rs:1242 test_gauss_hermite_new_returns_result (Ok+Err+message). Recommend dropping new_rejects_unsupported_orders as redundant with inline Err coverage.
  - *Action:* Do NOT delete new_rejects_unsupported_orders; its near-boundary order list (4,6,9,11..19,21) is distinct coverage the inline test lacks. At most consolidate the duplicated Ok-order list, but keep the broad Err sweep.
- dated_irr_uses_act365f_default (canonical_api.rs:147) is near-tautological: xirr(flows,None) == xirr_with_daycount(flows,Act365F,None) by definition, so |diff|<1e-6 can never fail.
  - *Action:* Do not delete as 'dead'. Either remove as the redundant half of the duplicate pair (keeping simplicity_parity:273), or tighten the assertion to exact equality (==) so the real default-day-count invariant is meaningfully guarded. The loose float tolerance provides no extra signal.
- xirr_trait_matches_ctx_helper_on_act365f_default (simplicity_parity.rs:273) is the same delegation-chain tautology one level deeper; |diff|<1e-12 cannot fail.
  - *Action:* Keep as the single delegation guard but tighten to exact equality (==) so it can actually catch a changed default/context; do not delete (it guards a real wire/behavior invariant).

---

## Part B — Coverage holes

### B.1 High priority

**Math — solvers, summation, linalg, integration**

- `finstack_quant_core::math::linalg::symmetric_eigen` — The public symmetric_eigen function is covered only by a rustdoc example (doctests are excluded from the project test run). No #[cfg(test)]/integration test asserts eigenvalues/eigenvectors for a known matrix, the n==0 empty path, or the DimensionMismatch error (matrix.len()!=n*n).
  - *Add:* Add unit tests: (1) diagonal matrix -> eigenvalues equal diagonal; (2) known 2x2 symmetric matrix -> verify A v = lambda v per returned (val, vec); (3) n=0 -> (empty, empty); (4) wrong-length slice -> Err(DimensionMismatch).

**Math — statistics, distributions, characteristic functions**

- `finstack_quant_core::math::stats::realized_variance (CloseToClose)` — The NaN-detection Err branch (src/math/stats.rs:468-472) for non-positive/non-finite prices producing undefined log returns is never hit; tests only pass strictly positive close series. Also the n<2 early `Ok(0.0)` return (line 459) is untested.
  - *Add:* realized_variance(&[100.0, 0.0], CloseToClose, 252.0).is_err() (zero price), realized_variance(&[100.0, -5.0], ...).is_err(), and realized_variance(&[100.0], CloseToClose, 252.0) == Ok(0.0). A wrong-variance output from a bad price tick directly corrupts vol estimates.

**Math — interpolation**

- `ExtrapolationPolicy::None NaN path (utils.rs:33,42; all strategies)` — No test builds with ExtrapolationPolicy::None and queries out of bounds; the NaN arms in all five strategies are never hit. grep in tests returns zero matches (only Display/FromStr roundtrip at types.rs:782).
  - *Add:* Per strategy, build with None and assert interp and interp_prime are NaN below first and above last knot, correct in-bounds.
- `PiecewiseQuadraticForwardStrategy interp_prime and extrapolation helpers (strategies.rs:462-546)` — PQF appears only in interp_basic_tests (interp.rs:185-188), not extrapolation_tests or derivative_tests. Its analytical interp_prime and FlatForward/FlatZero branches (flat_forward_df, flat_forward_df_prime, boundary_df, boundary_slope) are never called.
  - *Add:* Add extrapolation_tests and derivative_tests for the PQF interpolator plus numerical-vs-analytical derivative and FlatForward value/slope checks; wrong derivative corrupts forwards/DV01.

**Math — RNG / Sobol / Brownian bridge**

- `Pcg64Rng::new_with_stream determinism - finstack-quant/core/src/math/random.rs:237` — Stream reproducibility is not asserted. test_pcg64_rng_streams_independent (line 484) only checks that two DIFFERENT streams differ; test_pcg64_rng_deterministic (line 451) only covers new(). No test asserts that two Pcg64Rng::new_with_stream(seed, stream) with the SAME (seed, stream) produce identical sequences - the core determinism invariant for parallel Monte Carlo paths.
  - *Add:* Add a test: two Pcg64Rng::new_with_stream(42, 7) must yield identical uniform() sequences over 100 draws. This pins the seed+stream reproducibility that parallel MC relies on.
- `SobolRng scramble determinism - finstack-quant/core/src/math/random/sobol.rs:115` — No test asserts that two SobolRng::try_new(dim, seed) with the SAME non-zero scramble seed produce identical scrambled sequences. test_owen_scrambling (line 534) only checks scrambled != unscrambled. Reproducibility of the Owen-scrambled sequence (required for deterministic randomized-QMC error bars) is unverified.
  - *Add:* Add a test: two SobolRng::try_new(3, 12345) must produce identical next_point() sequences over many points; and a different scramble seed must differ.

**Market data — term-structure curves**

- `BaseCorrelationCurve arbitrage/smoothing API (src/market_data/term_structures/base_correlation.rs:473,528,560,573,636,673,721)` — `validate_arbitrage_free`, `is_monotonic`, `apply_smoothing` (all 4 SmoothingMethod variants: None/IsotonicRegression/StrictMonotonic/WeightedSmoothing), `make_arbitrage_free`, and `apply_bucket_bump` have NO #[test] coverage anywhere in the test suite (only doctests). The PAVA isotonic-regression and weighted-smoothing math is non-trivial. The builder's rejection of non-monotonic / out-of-[0,1] correlations (build() error path at line 843) and the `allow_non_monotonic()` bypass are also untested by #[test].
  - *Add:* Add tests/market_data/curves/base_correlation.rs cases: (1) build() rejects a non-monotonic curve [(3,0.5),(7,0.4)] and accepts it with allow_non_monotonic(); (2) build() rejects correlation outside [0,1]; (3) IsotonicRegression on [(3,0.5),(7,0.4),(10,0.6)] yields a monotonic curve matching the known PAVA pool-average; (4) StrictMonotonic and WeightedSmoothing produce monotonic output; (5) make_arbitrage_free is a no-op clone when already arbitrage-free; (6) apply_bucket_bump shifts only matching detachment points and clamps to [0,1].
- `ForwardCurve::df / df_on_date_curve implied projection discount factor (src/market_data/term_structures/forward_curve.rs:441,506)` — The simple-rate chaining algorithm `DF(t+dt)=DF(t)/(1+avg_fwd*dt)` (the implied projection DF) has zero test coverage. Neither the numeric value, the t=0 ⇒ 1.0 case, the negative-t / non-finite error path, nor the non-finite/non-positive denom Validation error are exercised. `df_on_date_curve` is also untested.
  - *Add:* On a flat 5% Act/360 simple-forward curve, assert df(t) matches the analytical chained accrual 1/Π(1+r*tau); assert df(0.0)==1.0; assert df(-0.5) and df(NaN) return Err; build a pathological curve (large negative forward making 1+avg*dt<=0) and assert the Validation error fires. Add coverage for both Linear (endpoint-average fast path) and a non-linear style (rate_period/Simpson path).

**Market data — vol surfaces / arbitrage**

- `VolCube serde (RawVolCube TryFrom/Into, deny_unknown_fields, interpolation_mode #[serde(default)])` — VolCube has a custom serde representation (`#[serde(try_from = "RawVolCube", into = "RawVolCube")]`, vol_cube.rs:103-159) with `#[serde(deny_unknown_fields)]` and a `#[serde(default)]` interpolation_mode, but there is NO serialize/deserialize round-trip test, NO deny-unknown-fields rejection test, and NO legacy-payload (missing interpolation_mode) default test anywhere. VolCube also has zero inline #[cfg(test)] tests.
  - *Add:* Add a VolCube serde test: build a cube, serde_json round-trip it, assert axes/params/forwards/interpolation_mode survive; assert an unknown field is rejected; assert a payload omitting `interpolation_mode` deserializes to VolInterpolationMode::Vol. Mirrors the existing VolSurface `quote_type_serde_round_trips_and_defaults_to_black` test.
- `SviArbitrageCheck::check_butterfly_density (SviButterflyCondition violation path)` — No test ever triggers a butterfly-density violation. `check_butterfly_density` is only reached via `check_all` in `svi_clean_params_pass_all_checks` (mod.rs:858), which asserts the slice is clean (zero violations). The g(k)<0 detection branch (svi.rs:127-140) and the SviButterflyCondition violation type are never asserted to fire. A bug in the analytical SVI density formula (w', w'', or g(k)) would not be caught.
  - *Add:* Add an SVI slice whose Gatheral-Jacquier density g(k) goes negative (e.g. large b, small sigma producing a sharp wing) and assert `check_butterfly_density` returns at least one ArbitrageType::SviButterflyCondition violation at the expected k; also confirm clean params produce none.

**Market data — scalars, dividends, fixings, DTSM**

- `InflationIndex::ref_cpi_months_lag (src/market_data/scalars/inflation_index.rs:383)` — The entire reference-CPI TIPS interpolation rule is untested. No test in src or tests/ calls ref_cpi_months_lag. The documented contract (divisor = days in the SETTLEMENT month, weight = (day-1)/D(m), anchors at first-of-month minus lag_months) is exactly the kind of off-by-one/month-length logic that silently produces wrong inflation accretion. The InvalidDateRange error branch (from_calendar_date day=1 failure) is also unreachable in tests.
  - *Add:* Add a unit test with monthly first-of-month CPI observations and assert RefCPI on a mid-month date matches the hand-computed CPI(m-L) + (day-1)/days_in_month * [CPI(m-L+1)-CPI(m-L)], including a month-end date (e.g. day 31 in a 31-day vs 28-day month) to lock the settlement-month divisor, and a first-of-month date (weight 0).
- `InflationIndex::with_seasonality / apply_seasonality (src/market_data/scalars/inflation_index.rs:343, 472)` — Seasonality is never exercised. No test sets seasonal factors, so value_on/ratio with a non-None seasonality array (base_value * factors[month-1]) and the month-index arithmetic (date.month() as usize - 1) are uncovered. A wrong month index (e.g. off-by-one) or factor application would not be caught.
  - *Add:* Construct an InflationIndex with a known [f64;12] seasonality (e.g. 1.0 everywhere except a distinct factor for one month), then assert value_on for a date in that month equals base*factor and a date in another month is unaffected. Also assert the seasonality survives the builder path.
- `YieldPanel::new validation branches (src/market_data/dtsm/types.rs:107)` — Most error branches are never triggered: non-ascending/duplicate tenors, non-positive/non-finite tenor, yields.ncols() != tenors.len() mismatch, nrows() < 2, dates length mismatch, and non-finite yield value. Inline DTSM tests only construct valid panels (pca_too_few_tenors routes through Self::new with vec![1.0] but is guarded by an if-let and may not assert). These guards protect every downstream model.
  - *Add:* Add focused unit tests for each YieldPanel::new error: tenors=[2.0,1.0] (not ascending), tenors=[-1.0,...] (non-positive), a 3-col yield matrix with a 2-tenor grid (col mismatch), a single-row matrix (nrows<2), a dates vec of wrong length, and a matrix containing f64::NAN. Assert each returns Err(Validation).

**Cashflow**

- `finstack_quant_core::cashflow::npv_amounts / npv_amounts_with_ctx (discounting.rs:446,462)` — Error branches are never triggered: (1) empty cash_flows -> InputError::TooFewPoints (discounting.rs:469-471); (2) non-finite discount_rate or (1.0 + discount_rate) <= 0.0 -> InputError::Invalid (discounting.rs:484-486). Every existing npv_amounts test passes valid, non-empty inputs.
  - *Add:* Assert Err for empty input, for discount_rate <= -1.0, and for non-finite discount_rate.

**Expression engine**

- `Binary`/`division-by-zero (src/expr/eval.rs:514)` — The documented convention that the binary `/` operator returns NaN for any zero divisor (distinct from pct_change which returns ±inf) is never asserted. The only Div test (tests/expr/eval.rs:116) uses non-zero divisors.
  - *Add:* Evaluate Expr::bin_op(Div, column_a, column_b) where b contains a 0.0 and assert that position is NaN (and contrast with pct_change ±inf already tested at functions.rs:263).
- `Invalid window/step parameter -> all-NaN convention (src/expr/eval_functions.rs:50 validate_window)` — validate_window rejects window < 1, fractional, non-finite by returning None, and callers then emit all-NaN. No test passes literal(0.0), literal(-1.0) for a window, or a fractional window to a rolling_* / lag function to assert the all-NaN output. (The negative literal at functions.rs:161 is a Shift count, a different code path.)
  - *Add:* Evaluate RollingMean(column, literal(0.0)) and RollingMean(column, literal(1.5)) and assert every output is NaN; also Lag(column, literal(0.0)).

**Types, errors, config, validation, explain, table**

- `finstack_quant_core::types::Attributes::matches_selector (attributes.rs:77)` — Selector-matching logic is completely untested in the core domain despite being called from production code (statements-analytics peer_set.rs and valuations instruments traits). Untested branches: '*' wildcard true; 'tag:<x>' present/absent; 'meta:k=v' equal/not-equal/missing-key; meta spec without '='; and the unrecognized-prefix -> false forward-compat path.
  - *Add:* Build Attributes with a tag and a meta entry, then assert matches_selector("*")==true, "tag:energy"==true, "tag:missing"==false, "meta:region=NA"==true, "meta:region=EU"==false, "meta:missing=x"==false, "meta:region" (no '=')==false, and "unknown:foo"==false.

### B.2 Medium priority

**Math — solvers, summation, linalg, integration**

- `finstack_quant_core::math::linalg::cholesky_decomposition_into` — This public function has no direct test. It is only exercised indirectly through the LM solver's internal normal-equations path (solver_multi.rs:612). Its dedicated error branches (DimensionMismatch when l.len()!=n*n, NotPositiveDefinite, Singular) and its equivalence to cholesky_decomposition are never asserted.
  - *Add:* Run cholesky_decomposition_into on a 2x2/3x3 SPD matrix and assert the buffer equals cholesky_decomposition output; add an error case where the output buffer length != n*n (DimensionMismatch) and a non-PD matrix (NotPositiveDefinite).
- `finstack_quant_core::math::linalg::cholesky_solve` — The DimensionMismatch error branch (chol.len()!=n*n or x.len()!=n) and the singular-diagonal Invalid branch are never triggered. Only happy-path solves and the relative-threshold scaled case are tested. A near-zero diagonal returning InputError::Invalid guards against NaN/Inf solutions and is unexercised.
  - *Add:* Pass a Cholesky factor with a (near-)zero diagonal and assert cholesky_solve returns Err; add a dimension-mismatch test (b.len()!=sqrt(chol.len())).
- `finstack_quant_core::math::integration::gauss_legendre_integrate_adaptive (globally-acceptable leaf branch)` — The fail-loud branch (err>orig_tol at max_depth) is covered, but the adjacent branch — err<=orig_tol but >halved leaf tol at max_depth, which must return Ok(i2) instead of erroring — is not explicitly pinned. This is the 'locally unconverged but globally acceptable' path (integration.rs:941-948).
  - *Add:* Construct an integrand/tolerance/max_depth where a leaf's local error exceeds the halved budget but the top-level error is within orig_tol, and assert Ok with result within orig_tol (not an error).
- `finstack_quant_core::math::solver_multi::LevenbergMarquardtSolver::minimize (NumericalFailure path)` — scalar_solution_to_result maps LmTerminationReason::NumericalFailure -> SolverConvergenceFailed unconditionally, but no test drives the solver into NumericalFailure (objective returning NaN/Inf, or a singular Jacobian the damping floor cannot rescue). Only MaxIterations/StepTooSmall and converged paths are tested.
  - *Add:* Add a minimize() test with an objective returning NaN at the initial point (or a degenerate Jacobian) and assert Err(SolverConvergenceFailed); confirms no best-guess is silently returned.
- `finstack_quant_core::math::solver::BrentSolver::solve_in_bracket` — The public solve_in_bracket entry point (src:951, recommended when a valid bracket is known) has no test. Its a<=b vs a>b ordering normalization and the early-root-at-endpoint shortcuts (flo==0 -> Ok(lo), fhi==0 -> Ok(hi), src:993-998) are never directly asserted.
  - *Add:* Add tests: (1) solve_in_bracket with a>b (reversed bracket) finds the root; (2) endpoint exactly a root returns that endpoint; (3) same-sign endpoints return Err.
- `finstack_quant_core::math::time_grid::map_date_to_step / map_dates_to_steps` — These public date->step mapping functions (src:329, 351) have no inline or integration tests. Only the year-fraction variant map_exercise_dates_to_steps is tested. The degenerate ttm<=0 / steps==0 -> 0 branch and the clamp-to-ttm behavior are unexercised.
  - *Add:* Map a base/event/maturity date triple with a known day-count to the expected step index, plus a degenerate case (maturity before base, or steps=0) asserting 0.
- `finstack_quant_core::math::probability::CorrelatedBernoulli::sample_from_uniform (branch coverage)` — Only boundary u=0.99 -> (0,0) and a tautological u=0.0 check exist (probability.rs:419). The middle branches (1,0) and (0,1), which depend on the cumulative thresholds p11, p11+p10, p11+p10+p01, are never asserted, so an off-by-one in the cumulative inverse-CDF would not be caught.
  - *Add:* For a known distribution (e.g. p1=p2=0.5, corr=0) choose u values landing in each of the four cumulative buckets and assert the exact (x1,x2) pair for each.

**Math — statistics, distributions, characteristic functions**

- `finstack_quant_core::math::stats::realized_variance_ohlc` — The Err(Validation) branch for unequal slice lengths (open/high/low/close length mismatch, src/math/stats.rs:514-523) is never triggered. Every caller in tests passes equal-length slices.
  - *Add:* Assert realized_variance_ohlc(&[1.0,2.0], &[1.0], &[1.0], &[1.0], Parkinson, 252.0).is_err() and that the message mentions 'same length'. Mismatched OHLC vectors are a realistic data-ingestion bug that would otherwise produce a silent panic or wrong slice indexing if the guard regressed.
- `finstack_quant_core::math::stats::OnlineStats::merge` — Both early-return branches — merging when other.count==0 (no-op, line 706) and when self.count==0 (adopt other, line 709) — are never exercised; existing merge test (stats.rs:994) only merges two non-empty accumulators.
  - *Add:* Merge an empty OnlineStats into a populated one and vice versa; assert count/mean/variance equal the populated accumulator. These guards protect parallel reductions where some chunks are empty.
- `finstack_quant_core::math::characteristic_function::{BlackScholesCf, MertonJumpCf, VarianceGammaCf} serde` — All three CF structs derive serde::Serialize/Deserialize (black_scholes.rs:18, merton.rs:20, variance_gamma.rs:36) but there is no serialization round-trip or field-name test. Per project serde-stability standards these wire formats are unpinned, and VG specifically warns that struct-literal/deserialize bypasses validate().
  - *Add:* serde_json round-trip each CF struct asserting field names (r,q,sigma,nu,theta,lambda,mu_j,sigma_j) and value equality; for VG, deserialize a martingale-violating param set and assert validate() then returns Err.
- `finstack_quant_core::math::characteristic_function::MertonJumpCf::cumulants` — Merton closed-form c3/c4 (merton.rs:56-59) are never validated against numerical differentiation. numerical_cumulants_match_closed_form_for_bs (mod.rs:242) covers BS (c3=c4=0) and the VG test (variance_gamma.rs:178) covers VG c1/c2, but the Merton non-zero skew/kurtosis cumulants used by the COS truncation range are unchecked.
  - *Add:* For a jumpy MertonJumpCf (lambda>0, mu_j!=0), assert cumulants() c1..c4 match cumulants_from_cf within ~1e-5; a wrong c3/c4 mis-sizes the Fourier integration range for skewed/fat-tailed pricing.

**Math — interpolation**

- `MonotoneConvexStrategy::with_epsilon (strategies.rs:964-979)` — Never called by any test. Error branch (epsilon not in (0,1e-6], strategies.rs:968) and non-default-epsilon success path both uncovered.
  - *Add:* Assert with_epsilon at 0.0 and 1e-3 return Err; a valid epsilon builds and epsilon() returns it and round-trips knot DFs.
- `Interpolator::new knot-spacing rejection (validate_knot_spacing, KnotSpacingTooSmall)` — Only tested at inline-unit level on validate_knot_spacing (utils.rs:167-193). No public-API test passes near-coincident knots through new/build.
  - *Add:* Build each strategy via new with knots 1.0 and 1.0 plus 1e-16 and 2.0; assert is_err().

**Math — RNG / Sobol / Brownian bridge**

- `RandomNumberGenerator::next_u64 (Pcg64Rng override) - finstack-quant/core/src/math/random.rs:272` — Pcg64Rng::next_u64 (delegates to inner.next_u64) has zero test coverage. No test in core or sobol_golden.rs calls next_u64. It is a public API method used to produce 64-bit integers, but neither its determinism (same seed -> same u64 stream) nor its range/distribution is asserted.
  - *Add:* Add a test: two Pcg64Rng::new(42) must yield identical next_u64() sequences (determinism), and a third with a different seed must differ. Optionally assert full 64-bit spread (high and low 32 bits both vary across a sample).
- `SobolRng::fill_u01 - finstack-quant/core/src/math/random/sobol.rs:301` — fill_u01 has no test. It is public, chunks a buffer by dimension and fills consecutive Sobol points, and carries a debug_assert that the buffer length is a multiple of dimension. Neither the happy path (output equals successive next_point values, all in (0,1)) nor the multiple-of-dimension contract is asserted anywhere.
  - *Add:* Add a test filling a buffer of length 2*dimension and asserting it equals two next_point() calls from a freshly-reset SobolRng, and that all values are in (0.0, 1.0).

**Math — volatility models**

- `HestonParams::require_feller (finstack-quant/core/src/math/volatility/heston.rs:428)` — This public builder method returns crate::Result<Self> with both an Ok branch (Feller condition satisfied) and an Err branch (2*kappa*theta <= sigma^2). Neither branch is exercised by any inline or integration test (grep across the whole workspace finds only the definition). The companion satisfies_feller_condition() is tested at heston.rs:1234, but require_feller — the fallible variant that constructs an error message directing users to satisfies_feller_condition — has zero coverage.
  - *Add:* Add a test: HestonParams::new(0.04,2.0,0.04,0.3,-0.5).unwrap().require_feller() is Ok (2*2*0.04=0.16 > 0.09); and HestonParams::new(0.04,0.5,0.04,0.5,-0.5).unwrap().require_feller() is Err (0.04 < 0.25). Optionally assert the error message mentions Feller/with_shift guidance.
- `LocalVolSurface::from_implied_vol_smoothed (finstack-quant/core/src/math/volatility/local_vol.rs:311)` — This entire public method is untested. Both behaviors are uncovered: (1) the negative-sigma_strikes Err branch (lines 317-321, returns Error::Validation), and (2) the actual Gaussian-smoothing code path (lines 338-373) which builds a smoothed VolSurface and re-extracts local vol. The sigma_strikes==0 short-circuit to from_implied_vol (line 322) is also untested. Smoothing is the method's whole reason to exist and no test confirms it regularises noise or even runs.
  - *Add:* Add tests: (a) from_implied_vol_smoothed(&surface, 100.0, 0.0, -1.0) returns Err; (b) with sigma_strikes=0.0 it equals from_implied_vol grid-for-grid; (c) on a noisy surface, sigma_strikes>0 produces positive/finite local vols and a smoother strike profile (e.g. smaller second-difference) than the unsmoothed extraction.
- `SabrParams::check_density warning-emitting branch (finstack-quant/core/src/math/volatility/sabr.rs:466)` — check_density is exercised for the no-warning case (test_sabr_density_check_normal_params asserts warnings.is_empty(), sabr.rs:1260) and for an unasserted extreme case (test_sabr_density_check_extreme_nu, sabr.rs:1238, explicitly does NOT assert on the result). No test asserts that a negative-density (butterfly-arbitrage) configuration actually produces a non-empty Vec<DensityWarning> with the expected strike/value fields. The positive-detection path of this risk-management check is therefore unverified.
  - *Add:* Find SABR params (high nu, long expiry, wing strikes) that provably yield negative risk-neutral density and assert check_density returns at least one DensityWarning, validating the strike and that the warned density is negative.

**Market data — term-structure curves**

- `ForwardCurve::rate_period numerical averaging (src/market_data/term_structures/forward_curve.rs:381)` — Only the reversed-times NaN/debug-assert path is tested (forward_curve.rs:989/996). The actual Simpson's-rule average-forward value, the dt<=1e-12 ⇒ rate(t1) shortcut, and the segment-count branches (n=8/16/32 for dt>5,>20) are untested — so a regression in the integration would not be caught.
  - *Add:* On a linear forward curve, assert rate_period(t1,t2) equals the exact integral average of the linear interpolant; on a flat curve assert it equals the flat rate exactly; assert rate_period(t,t) returns rate(t); cover a long interval (dt>20) to exercise the n=32 branch.
- `DiscountCurve day-count inference from curve ID (src/market_data/term_structures/common/conventions.rs:67; discount_curve.rs:1124)` — `infer_discount_curve_day_count` (SOFR/ESTR/EURIBOR ⇒ Act360, SONIA/TONAR ⇒ Act365F, leading-currency fallback, synthetic ⇒ Act365F) is NOT asserted for DiscountCurve (forward curve has builder_infers_market_conventions_from_curve_id, but discount has nothing). The docs flag a 'build-vs-query basis trap' where renaming the ID silently shifts every pillar ~1.4%.
  - *Add:* Assert DiscountCurve::builder("USD-SOFR").day_count()==Act360, builder("GBP-SONIA").day_count()==Act365F, builder("EUR-ESTR").day_count()==Act360, and a synthetic ID like "TEST" ⇒ Act365F.

**Market data — vol surfaces / arbitrage**

- `VolCube::materialize_grid` — materialize_grid (vol_cube.rs:603-626), the full (expiry x tenor x strike) flattening, has no test. Neither the happy path (output length == n_exp*n_ten*n_str, values match cube.vol) nor the empty-strikes Err path (`InputError::TooFewPoints`) is exercised. materialize_tenor_slice and materialize_expiry_slice are tested, but the 3D materialization is not.
  - *Add:* Build a 2x2 cube, call materialize_grid(&strikes), assert length == 2*2*n_strikes and that out[0] equals cube.vol(expiries[0], tenors[0], strikes[0]) (floored); separately assert materialize_grid(&[]) is an Err.
- `Arbitrage standalone grid wrappers: check_butterfly_grid, check_calendar_spread_grid, check_local_vol_density_grid` — Only the aggregate `check_surface_grid` is tested (mod.rs:486, 533). The three single-check grid wrappers (mod.rs:249, 269, 293) have no direct test. check_local_vol_density_grid has a forward_prices-length-mismatch Err branch (mod.rs:299-305) and the other two route through expand_forward_prices, none of which is exercised via these wrappers.
  - *Add:* For each wrapper, feed a known-violating grid (e.g. butterfly_violation / calendar_spread_violation rows) and assert the expected violation type appears; for check_local_vol_density_grid additionally assert that a forward_prices vector whose length != expiries.len() returns Err.

**Market data — context, bumps, diff, hierarchy**

- `MarketContextState::try_from (finstack-quant/core/src/market_data/context/state_serde.rs:416)` — The unsupported-version rejection branch: `if !(1..=MARKET_CONTEXT_STATE_VERSION).contains(&state.version)` returning Err(Validation). All tests use version 1 or 2 (valid). Neither version 0 nor a future version (e.g. 3) is ever fed to try_from / deserialize.
  - *Add:* Construct a MarketContextState (or JSON) with version 0 and with version MARKET_CONTEXT_STATE_VERSION+1; assert MarketContext::try_from / serde deserialize both return Err with the 'Unsupported MarketContextState version' message.
- `MarketContextState deny_unknown_fields (finstack-quant/core/src/market_data/context/state_serde.rs:156)` — `MarketContextState` is `#[serde(deny_unknown_fields)]` but the strict-inbound `assert_strict_inbound` harness in tests/market_data/serde.rs is only applied to individual curve/surface states, never to the full MarketContextState envelope. An unknown top-level field on a persisted context snapshot is not verified to be rejected.
  - *Add:* Serialize a MarketContextState to JSON, insert an unknown top-level key, and assert serde_json::from_value::<MarketContextState> errors — guards the snapshot wire contract.
- `MarketContext::roll_forward error path (finstack-quant/core/src/market_data/context/ops_roll.rs:73 / mod.rs:599 roll_forward_storage)` — roll_forward returns Result and propagates errors when a curve has too few remaining points after the time roll. No test rolls forward far enough (or with a sparse curve) to trigger the Err branch; all roll_forward tests use 30-day rolls that succeed.
  - *Add:* Build a discount curve whose last knot is < N days out, call roll_forward(N+large) so expired points leave <2 knots, and assert it returns Err.
- `MarketContext::bump_observed / roll_forward_observed + ContextMutationInfo (finstack-quant/core/src/market_data/context/ops_bump.rs:138, ops_roll.rs:81)` — The *_observed variants and the returned ContextMutationInfo.invalidated_credit_indices / has_invalidations() are never called in the core test suite. The scenario where a base-correlation/curve bump or a cross-type replacement actually populates invalidated_credit_indices (and is observed) is not asserted.
  - *Add:* Set up a credit index whose hazard curve is replaced by a cross-type (discount) curve via bump_observed/insert, and assert the returned ContextMutationInfo.invalidated_credit_indices contains the index id and has_invalidations() is true.
- `MarketContext snapshot-restore mutation helpers (finstack-quant/core/src/market_data/context/stats.rs:208-293)` — retain_curves_mut, retain_series_mut, replace_surfaces_mut, replace_vol_cubes_mut, replace_fx_delta_vol_surfaces_mut, and clear_market_scalars_mut (all drop-and-replace snapshot-restore primitives) have no core-crate test. A bug here would silently corrupt P&L-attribution snapshot restores.
  - *Add:* For each: seed a context, apply the retain/replace/clear, and assert the surviving vs dropped entries match the predicate / replacement set exactly.
- `diff::measure_scalar_absolute_shift and currency-mismatch / NaN guards (finstack-quant/core/src/market_data/diff.rs:603, 546, 495)` — measure_scalar_absolute_shift (absolute, not percentage) is never tested in the core suite. Also untested: the non-finite-shift Err guards in measure_scalar_shift (diff.rs:578) and measure_scalar_absolute_shift (diff.rs:623), and the rate_t0==0 zero-rate guard in measure_fx_shift (diff.rs:516).
  - *Add:* Test measure_scalar_absolute_shift returns value_t1 - value_t0 in native units for both Unitless and Price; test measure_fx_shift errors when the t0 FX rate is 0; test the non-finite guard triggers (e.g. inf scalar) producing Err.

**Market data — scalars, dividends, fixings, DTSM**

- `InflationIndex::apply_lag InflationLag::Days (src/market_data/scalars/inflation_index.rs:462)` — Only InflationLag::Months and the default None are exercised (test_with_lag, serde roundtrip). The Days(days) branch (date.checked_sub) and its InvalidDateRange error on underflow are untested, despite InflationLag::Days(90) being constructed in test_builder_pattern without ever calling value_on.
  - *Add:* Add a test using with_lag(InflationLag::Days(n)) and assert value_on resolves to the observation n calendar days earlier; optionally trigger the checked_sub underflow with an extreme date to cover the InvalidDateRange branch.
- `YieldPanel::from_rows and from_yield_changes (src/market_data/dtsm/types.rs:65, 84)` — from_rows is never called in any test. from_yield_changes is only exercised indirectly through YieldPca::fit_yield_changes; the ragged/empty-rows error path in rows_to_dmatrix and the synthetic-tenor-grid integration logic are not directly asserted.
  - *Add:* Add tests: from_rows with valid row-major data equals the equivalent DMatrix-built panel; from_rows with ragged rows returns Err; from_yield_changes reconstructs levels with the expected synthetic ascending tenor grid (1.0..=n) and width.
- `ScalarTimeSeries serde strictness (RawScalarTimeSeries, src/market_data/scalars/primitives.rs:497)` — RawScalarTimeSeries carries #[serde(deny_unknown_fields)] but no test verifies an unknown field is rejected. By contrast DieboldLi and all curves have explicit reject-unknown-fields tests via assert_strict_inbound. The existing scalar_time_series_roundtrip (tests/market_data/serde.rs:63) only checks a clean roundtrip. A regression dropping deny_unknown_fields would go unnoticed.
  - *Add:* Add an assert_strict_inbound(&series) test (or inline equivalent) for ScalarTimeSeries confirming an extra top-level JSON field fails deserialization, mirroring the curve tests.
- `ScalarTimeSeries linear interpolation past series end (src/market_data/scalars/primitives.rs:441)` — The branch returning the last value for query dates strictly after the final observation (idx >= date_vec.len()) under Linear interpolation is not directly asserted. Step LOCF after-end is implicitly covered, but the linear flat-extrapolation behavior (and the .last().ok_or TooFewPoints fallback) is uncovered, so silent wrong extrapolation would not be caught.
  - *Add:* Build a 2-3 point series with Linear interpolation and assert value_on for a date after the last observation returns the last observed value (flat extrapolation), and that value_on between points still interpolates.
- `fixings::require_fixing_value_bounded (src/market_data/fixings.rs:122)` — This public helper has no test. Its three outcomes (resolve within window, error when stale beyond max_staleness_days, error when series is None) are uncovered, even though require_fixing_value and require_fixing_value_exact each have dedicated tests. The underlying value_on_or_before is tested on ScalarTimeSeries, but the wrapper's error-message construction and None handling are not.
  - *Add:* Mirror the require_fixing_value tests: assert it returns the prior value within the window, errors with a message mentioning the staleness limit when too old, and errors when series is None.

**Credit**

- `SeniorityClass::FromStr and CollateralType::FromStr (finstack-quant/core/src/credit/lgd/seniority.rs:50-73, finstack-quant/core/src/credit/lgd/workout.rs:74-95)` — The Err branch of both FromStr impls (unknown label) is never triggered. Tests only feed valid strings (e.g. 'senior-secured', 'real-estate' via lgd/mod.rs). The detailed 'unknown seniority class'/'unknown collateral type' validation errors are not asserted, nor are the alias paths (e.g. '1st_lien_secured' vs 'first_lien_secured', 'ip' for IntellectualProperty, 'sub'/'junior' shortcuts).
  - *Add:* Add tests asserting "not-a-class".parse::<SeniorityClass>().is_err() and "bogus".parse::<CollateralType>().is_err(), plus a couple of alias round-trips ('1st_lien_secured', 'ip', 'junior') to lock the accepted-string contract that the Python/WASM bindings depend on.
- `GeneratorMatrix::from_transition_matrix error branches (finstack-quant/core/src/credit/migration/generator.rs:236-252)` — Two extraction error paths are never triggered: ComplexEigenvalues (matrix_log when P has complex eigenvalues / no real Schur form) and NoValidGenerator (an eigenvalue <= 0, e.g. an oscillatory/negative-eigenvalue transition matrix). RoundTripError IS effectively covered (the loosened-tolerance test at migration/tests.rs:331 documents the default 1e-2 tolerance is exceeded), but the bare ComplexEigenvalues/NoValidGenerator returns are not.
  - *Add:* Construct a 2x2 row-stochastic matrix with a negative eigenvalue (e.g. a strongly anti-persistent matrix like [[0.1,0.9],[0.9,0.1]] embedded with an absorbing state, or a 3-state cyclic matrix) and assert from_transition_matrix returns Err(NoValidGenerator) or Err(ComplexEigenvalues).
- `Whole credit domain — integration / cross-module tests (no tests/credit directory exists)` — There are no end-to-end integration tests for credit. Realistic pipelines spanning modules are untested: scoring -> MasterScale.map_score -> grade (only a single inline case at pd/tests.rs:695 wires Altman into a master scale); TransitionMatrix -> GeneratorMatrix -> projection -> PdTermStructure::from_transition_matrix as one flow; SeniorityCalibration/WorkoutLgd/EAD/Downturn composed into a full LGD-x-EAD loss number. Determinism of MigrationSimulator::empirical_matrix across runs (same seed) is only partially covered (simulate is checked at migration/tests.rs:458; empirical_matrix reproducibility is not).
  - *Add:* Add a finstack-quant/core/tests/credit/ integration file building an example-equivalent workflow: extract a generator from the 7x7 reference matrix, project a term structure, map a scored firm to a master-scale grade, and compute a downturn-adjusted workout LGD on an EAD — asserting key aggregates. Add a determinism test asserting empirical_matrix(N, seed) == empirical_matrix(N, seed) for the same seed.

**Dates**

- `DayCount::signed_year_fraction (finstack-quant/core/src/dates/daycount.rs:811)` — No test in the dates domain calls signed_year_fraction; the negative branch (end < start producing a negated fraction), the zero branch (start == end -> 0.0), and the guarantee that it never returns InvalidDateRange are all unexercised here. The doctest is build-only-style and other crates only use the happy path.
  - *Add:* Add a dates test asserting signed_year_fraction(base, past) < 0, signed_year_fraction(base, future) > 0, signed_year_fraction(base, base) == 0.0, and that -signed == year_fraction for the swapped order, for at least Act365F and Act360.
- `DayCount::year_fraction inverted-range error (finstack-quant/core/src/dates/daycount.rs:732)` — The start > end -> Err(InvalidDateRange) branch of the public year_fraction is never asserted (the existing is_err() tests at daycount.rs:455 and :700 cover missing-frequency and missing-calendar, not inverted dates).
  - *Add:* Assert DayCount::Act360.year_fraction(end, start, ctx) returns Err matching InputError::InvalidDateRange for end < start.
- `year_fraction_bus252 zero-basis error (finstack-quant/core/src/dates/daycount.rs:1230)` — InputError::InvalidBusBasis is returned when ctx.bus_basis == Some(0) but no test ever passes bus_basis: Some(0); only default/None (252) and Some(260) (serde test) are exercised.
  - *Add:* Call DayCount::Bus252.year_fraction with DayCountContext { calendar: Some(&TARGET2), bus_basis: Some(0), .. } and assert Err(InvalidBusBasis { basis: 0 }).
- `Tenor::from_years (finstack-quant/core/src/dates/tenor.rs:240)` — No dates-domain test. Untested branches: integer-month rounding to Years (multiple of 12), integer-month to Months, fractional -> Days under each day-count branch (360 vs 365 vs 365.25), and the negative/non-finite guard returning Tenor::new(0, Days).
  - *Add:* Add cases: from_years(1.0, ActAct) -> 1Y; from_years(0.5, Act365F) -> 6M; from_years(0.25,..) -> 3M; a fractional value -> Days using Act360 vs Act365F vs default(365.25); from_years(-1.0,..) and from_years(f64::NAN,..) -> 0D.
- `Tenor::from_payments_per_year error path (finstack-quant/core/src/dates/tenor.rs:650)` — The happy path (4 and 2) is covered only by the schedule_iter.rs doctest. The error branches -- payments == 0 and payments that do not divide 12 (e.g. 5, 7) -- are never asserted.
  - *Add:* Assert from_payments_per_year(0).is_err(), from_payments_per_year(5).is_err(), and from_payments_per_year(12) -> Tenor::monthly().
- `act_act_isma_year_fraction_with_reference_period error branches (finstack-quant/core/src/dates/daycount.rs:884)` — Only the deep-recursion error (inline test) and two happy stub paths (integration) are tested. The start > end -> InvalidDateRange branch, the reference_start >= reference_end -> InvalidDateRange branch, and the period_months == 0 (zero-length reference) -> Invalid branch are never triggered.
  - *Add:* Add cases: reversed accrual (start>end) -> Err; reversed reference (ref_start>=ref_end) -> Err; reference period spanning <1 month so months_until==0 -> Err.

**Cashflow**

- `finstack_quant_core::cashflow::npv_with_options invalid-df_base guard (discounting.rs:360-364)` — The branch returning Error::Validation when df_base = disc.df(t_base) is non-finite or <= 0.0 is never exercised. No test supplies a curve whose discount factor at the valuation date is zero/negative/NaN.
  - *Add:* Implement a tiny Discounting fixture whose df() returns 0.0 (or a negative/NaN) at the valuation-date abscissa and assert npv(...) returns Err. This guards against silent division-by-zero / NaN PV propagation in the relative-DF normalization.
- `finstack_quant_core::cashflow::CFKind::is_interest_like (primitives.rs:298-311)` — This public predicate has zero tests anywhere (confirmed via grep across src and tests). The matches! arm (Fixed|FloatReset|InflationCoupon|Stub) could regress silently if a variant is added/removed.
  - *Add:* Assert is_interest_like() is true for Fixed, FloatReset, InflationCoupon, Stub and false for a representative non-interest set (Notional, Fee, Amortization, PIK, Recovery, VariationMarginPay).
- `finstack_quant_core::cashflow::npv / npv_with_options day-count failure path (discounting.rs:358,375)` — The documented error condition 'day count year fraction calculation fails (e.g. MissingCalendarForBus252)' is never triggered. Only the Bus/252 SUCCESS path (npv_with_bus252_context_counts_business_days, discounting.rs:526) is tested; calling npv with DayCount::Bus252 and a default (no-calendar) context to provoke the Err is absent.
  - *Add:* Call npv_with_ctx(&curve, base, Some(DayCount::Bus252), DayCountContext::default(), &flows) and assert it returns Err (propagated MissingCalendarForBus252).

**Money / currency / FX**

- `Money::from_decimal (src/money/types.rs:258)` — The Err(ConversionOverflow) branch when a Decimal cannot convert back to f64 is never triggered. Only the happy path is exercised (rounding.rs:94/100 via valid in-range values). The error path that guards against holding a non-round-trippable value has zero coverage.
  - *Add:* Construct a rust_decimal::Decimal at the extreme end of its range (or one whose to_f64() returns None) and assert Money::from_decimal returns Err(Error::Input(InputError::ConversionOverflow)).
- `Money::checked_mul_f64 / Money::checked_div_f64 (src/money/types.rs:483,513)` — These public fallible scalar operators have NO integration test and are not exercised by any inline #[cfg(test)] test — only by doctests. The Err paths (NaN/Inf for mul, zero/Inf for div) and the success path are unverified outside doc examples.
  - *Add:* In tests/money/rounding.rs or money_fx.rs add: checked_mul_f64(2.0) success, checked_mul_f64(f64::NAN)/INFINITY err, checked_div_f64(2.0) success, checked_div_f64(0.0)/INFINITY/NAN err, asserting the specific InputError variants.
- `FxMatrix::validate_triangular (src/money/fx/matrix.rs:617)` — Only the violation case is tested (money_fx.rs:448). The Ok path for a consistent triangle (cycle product ~ 1) is untested, and the input-validation error branch (negative or non-finite tolerance_bps at matrix.rs:618) is never exercised.
  - *Add:* Add (a) a consistent set of quotes (EUR/USD, USD/GBP, GBP/EUR product==1) asserting validate_triangular(5.0).is_ok(); and (b) validate_triangular(-1.0) and validate_triangular(f64::NAN) returning Err(Error::Validation).
- `FxConversionPolicy serde JSON (src/money/fx/types.rs:10-22)` — FromStr/Display roundtrip is covered (fx/mod.rs:66), but the serde snake_case wire names (rename_all="snake_case": cashflow_date, period_end, period_average, custom) are never asserted via serde_json. A rename of a variant would silently break stored FxQuery/FxPolicyMeta payloads.
  - *Add:* Assert serde_json::to_string(&FxConversionPolicy::CashflowDate)=="\"cashflow_date\"" for each variant and that from_str round-trips, locking the golden wire names.
- `Money serde deny_unknown_fields + field-name golden (src/money/types.rs:114)` — Money derives Serialize/Deserialize with #[serde(deny_unknown_fields)] but no test asserts the field names {amount, currency} nor that an unknown field is rejected. rounding.rs:79 only checks the amount substring "0.1" appears. A field rename or relaxed deny would not be caught.
  - *Add:* Round-trip a Money through JSON asserting exact {"amount":"...","currency":"USD"} shape, and assert deserializing a payload with an extra unknown field returns Err.
- `Money Mul/Div/MulAssign/DivAssign operators (src/money/types.rs:639,656,786,792)` — The infix `*` and `/` operators on Money and the `*=`/`/=` assign operators have no test asserting their happy-path arithmetic. Only the inline panic tests cover `/0.0` and `*NaN` (types.rs:876,882). A regression in repr_mul_f64/repr_div_f64 wiring (e.g. wrong currency preservation) would go unnoticed for the success path.
  - *Add:* Assert (Money::new(100,USD)*2.5).amount()==250 and currency preserved; (Money::new(100,USD)/4.0).amount()==25; and*=/ /= mutate in place correctly.

**Expression engine**

- `CompiledExpr::eval_abs / Function::Abs (src/expr/eval_functions.rs:757)` — Function::Abs has no test anywhere in src or tests. abs of negatives/positives/zero/NaN element-wise is never asserted.
  - *Add:* Evaluate Expr::call(Function::Abs, [column]) over [-3.0, 0.0, 4.0, NaN] and assert [3.0, 0.0, 4.0, NaN].
- `CompiledExpr::eval_sign / Function::Sign (src/expr/eval_functions.rs:765)` — Function::Sign is never tested. The -1/0/+1 mapping and the explicit NaN->NaN branch are uncovered.
  - *Add:* Evaluate Expr::call(Function::Sign, [column]) over [-2.5, 0.0, 7.0, NaN] and assert [-1.0, 0.0, 1.0, NaN].
- `BinOp arithmetic/logical operators Mod, And, Or, Eq, Ne, Ge, Le (src/expr/eval.rs:521-584)` — Only Add/Sub/Mul/Div/Gt/Lt are exercised in tests/expr/eval.rs. Mod (%), And, Or, Eq, Ne, Ge, Le evaluation (and the non-zero-as-true logical semantics) have no test.
  - *Add:* Add element-wise tests for each: e.g. Mod of [5,7]%[2,3]=[1,1]; And/Or with 0.0 vs non-zero operands; Eq/Ne/Ge/Le returning 1.0/0.0.
- `UnaryOp::Not (src/expr/eval.rs:597)` — Only UnaryOp::Neg is exercised (eval.rs inline if/binop/unary test). UnaryOp::Not (0.0 -> 1.0, non-zero -> 0.0) is never tested.
  - *Add:* Evaluate Expr::unary_op(Not, column) over [0.0, 1.0, -2.0] and assert [1.0, 0.0, 0.0].
- `CSRef evaluation error path (src/expr/eval.rs:382)` — Evaluating an ExprNode::CSRef via core::expr returns a Validation error ('capital-structure references require the statements evaluator'). No test triggers this Err branch (CSRef appears only in a serde deny-unknown-fields test).
  - *Add:* CompiledExpr::new(Expr::cs_ref("debt","total")).eval(...) and assert Err(Error::Validation) is returned.
- `CompiledExpr::try_new_scalar (src/expr/eval.rs:192)` — The public constructor try_new_scalar (fail-fast rejection of statements-layer functions at compile time) has no test. ast_walk::ensure_scalar_evaluable is tested via inline tests, but try_new_scalar's Ok/Err wrapping is not exercised through the public API.
  - *Add:* Assert try_new_scalar(Expr::call(Function::Ttm, ...)) is Err(Validation) and try_new_scalar(Expr::call(Function::RollingMean, ...)) is Ok.
- `Function::Quantile at q != 0.5 and q clamping (src/expr/eval_functions.rs:814)` — quantile is only tested at q=0.5 (median equivalence). The linear-interpolation result at q=0.0/0.25/0.9/1.0, the clamp of q outside [0,1], and the all-NaN-input -> NaN branch are untested.
  - *Add:* Evaluate Quantile(column, literal(0.0)) -> min, literal(1.0) -> max, literal(1.5) (clamps to 1.0) -> max, and an all-NaN column -> NaN.
- `Rolling NaN policy divergence (src/expr/eval_functions.rs rolling_mean/sum vs min/max/count)` — The documented split — rolling_mean/rolling_sum/rolling_std/rolling_var PROPAGATE a NaN in the window (window result NaN), while rolling_min/rolling_max/rolling_count SKIP NaNs — is only tested for rolling_median (functions.rs:1303). rolling_mean/sum NaN-propagation and rolling_min/max/count NaN-skipping have no test.
  - *Add:* rolling_sum([1,NaN,3], win=2) -> [NaN, NaN, NaN] (propagate); rolling_min([1,NaN,3], win=2) -> finite skip values; rolling_count([1,NaN,3], win=2) -> counts finite-only.

**Types, errors, config, validation, explain, table**

- `finstack_quant_core::validation (validation.rs require / require_or / require_with)` — All three public helpers have zero tests anywhere (no inline mod tests, no tests/ references, no src callers). Both the Ok(()) (condition true) and Err branch (Error::Validation / passed-through error) are uncovered, including that require_with does NOT evaluate the closure when the condition holds.
  - *Add:* Add unit tests: require(true, "m").is_ok(); require(false, "m") yields Error::Validation("m"); require_or(false, InputError::Invalid) yields the wrapped error; require_with(true, || panic!()) does not call the closure and is Ok; require_with(false, || "lazy".into()) yields Error::Validation("lazy").
- `finstack_quant_core::types::Bps::try_new and TryFrom<f64> for Bps (rates.rs:417, 747)` — Neither the success nor the failure path of Bps::try_new / Bps::try_from(f64) is exercised anywhere (Rate::try_from_decimal and Percentage::try_new are hit by proptests, but Bps's fallible constructor is not). Rounding behavior (e.g. 24.6 -> 25) and NaN/±Inf rejection are uncovered.
  - *Add:* assert_eq!(Bps::try_new(24.6).unwrap(), Bps::new(25)); assert!(Bps::try_new(f64::NAN).is_err()); assert!(Bps::try_new(f64::INFINITY).is_err()); assert_eq!(Bps::try_from(50.0_f64).unwrap(), Bps::new(50)).
- `finstack_quant_core::table::TableColumnData typed accessors (table.rs:233-254 macro: as_strings/as_f64/as_u32/as_i64 and as_nullable_*)` — Eight non-null + eight nullable accessor methods (generated by decl_typed_accessors!) plus TableColumn's delegating wrappers, plus TableColumn::is_empty/with_metadata and TableEnvelope::is_empty/column on the happy path, have no tests in the core domain. Only length-validation and dup-name errors and a single roundtrip are tested. A wrong-variant accessor must return None and the matching variant must return the slice.
  - *Add:* Build TableColumnData::Float64(vec![1.0,2.0]); assert `as_f64()==Some(&[1.0,2.0][..])` and as_strings()==None and as_nullable_f64()==None; repeat for String/UInt32/Int64 and their nullable variants; assert TableColumn::is_empty() on empty data and TableEnvelope::is_empty()/column(name) lookups.
- `finstack_quant_core::types::CreditRating & RatingFactorTable serde stability (ratings.rs:82, 505)` — Both derive Serialize/Deserialize but no test exercises a serde roundtrip. CreditRating is used as a HashMap key inside RatingFactorTable (serialized as a map-key string); a rename/variant-name change would silently break the wire format and any persisted rating tables with no failing test.
  - *Add:* Roundtrip CreditRating::BBBMinus through serde_json and assert equality and the exact field/variant string (e.g. \"BBBMinus\"); serialize RatingFactorTable::moodys_standard().unwrap(), deserialize, and assert get_factor(CreditRating::B)==2720.0 survives the roundtrip.

### B.3 Low priority (wire-format & convention pins)

Mostly serde golden-string coverage for additional enum variants and convention pins. Full list available in the raw audit; representative items:

*(67 low-priority holes catalogued across domains — see raw findings for the complete enumeration.)*

---

## Part C — False positives caught by verification (do NOT act on)

The adversarial verifier rejected these auditor claims after checking the actual code:

- **[md-surfaces / hole]** FxDeltaVolSurface custom serde (TryFrom RawFxDeltaVolSurface, deny_unknown_fields, validate() on deserialize) has NO round-trip test, NO deny-unknown-fields test, NO malformed-payload (non-monotonic e
  - *Why rejected:* The claim is factually wrong — these tests DO exist in tests/market_data/serde.rs. (1) Round-trip + deny-unknown-fields: fx_delta_vol_surface_rejects_unknown_fields (serde.rs:726-738) builds an FxDeltaVolSurface and calls assert_strict_inbound, which (serde.rs:592-611) serializes, asserts the origin
- **[md-scalars / duplicate]** primitives.rs:536 series_step_and_linear and inflation_index.rs:649/666 test_step_interpolation/test_linear_interpolation assert the same underlying step/linear behavior since InflationIndex delegates
  - *Why rejected:* Read primitives.rs:536-561 (series_step_and_linear, raw ScalarTimeSeries) and inflation_index.rs:649-681 (test_step_interpolation/test_linear_interpolation on InflationIndex). The inflation_index tests exercise the InflationIndex wrapper which adds the apply_lag (lag=None default) and apply_seasonal
- **[md-scalars / dead]** storage.rs:181 test_storage_iteration largely confirms std zip/collect ordering already covered by test_storage_creation_and_sorting.
  - *Why rejected:* Read storage.rs:180-191 and 125-142. test_storage_iteration is the ONLY test that exercises the private iter() method (storage.rs:113-118); test_storage_creation_and_sorting uses date()/value() index accessors, never iter(). iter() is consumed by ScalarTimeSeries::observations (primitives.rs:463-471
- **[dates / duplicate]** CNY known historical dates 2020-2025 asserted both at rule level (rules.rs rule_chinese_new_year_known_dates:549) and calendar level (calendars.rs check_cny_dates / early-late tests:393); the 2020-202
  - *Why rejected:* False premise. calendars.rs check_cny_dates is only called for 1970s (line 394: 1970/1975/1980/1989) and 2100s (line 399: 2101/2125/2150) — NOT for 2020-2025. The only 2020-2025 CNY assertions at calendar level are scattered in the calendar must_have tables and cover ONLY 2024-02-10 (lines 173,189,2
- **[money / dead]** try_new_handles_very_small_values (types.rs:1021) asserts amount()==1e-15, a no-op round-trip that exercises no rounding/scaling logic.
  - *Why rejected:* types.rs:1020-1026: asserts try_new(1e-15).amount()==1e-15. This pins a real invariant — that DEFAULT construction (cfg=None) performs NO ingest rounding and preserves sub-cent precision (new_finite at types.rs:292 uses Decimal::from_f64 with no rounding when cfg is None). A regression that re-intro
- **[money / hole]** Money::new_with_config ingest rounding for a 0-decimal currency (JPY/KRW) is untested; suggested test Money::new_with_config(100.7, JPY, default cfg) should yield amount()==101.
  - *Why rejected:* The suggested test is based on a FALSE premise. config.rs:490-495 ingest_scale = max(6, ccy.decimals()) — so default ingest_scale(JPY) is 6, NOT 0. Money::new_with_config(100.7, JPY, default cfg) would preserve 100.7, NOT round to 101. Default Money::new (cfg=None) does NO ingest rounding at all (ne
- **[expr / dead]** tests/expr/functions.rs:744 `binary_op_missing_tail_yields_nan` is unnecessary in module ewm_operations: misplaced and overlaps the eval.rs column-length contract / eval_unary_op tail-fill paths.
  - *Why rejected:* Confirmed the test (functions.rs:745-762) sits inside `mod ewm_operations` (begins line 654, next module statistical_operations at 1073) and tests BinOp::Add ragged-length tail NaN, unrelated to EWM — so the misplacement is real. BUT the overlap claim is false: grepping all of src/ and tests/ found
- **[types-infra / duplicate]** test_trace_serialization (src:264) and test_explanation_trace_serialization (integration:43) assert the same intent; the inline one is redundant because test_cashflow_pv_entry already covers the Calib
  - *Why rejected:* The stated rationale is factually wrong. The inline test_trace_serialization (src/explain.rs:264-288) serializes a CalibrationIteration entry and is the ONLY test asserting `"kind": "calibration_iteration"` (src:281) via to_json_pretty. The integration test (tests/infrastructure/explain.rs:43) seria
- **[cross-cutting / hole]** Act360/Act365F QuantLib golden cases: parse_convention supports Act360 and Act365F, but the JSON fixture (daycount_quantlib.json) appears to contain no Act360/Act365F cases, so these two conventions g
  - *Why rejected:* FACTUALLY FALSE. daycount_quantlib.json DOES contain an Act360 case (line 212, start 2025-01-01 end 2025-04-01) and an Act365F case (line 225, start 2025-01-01 end 2026-01-01). grep -c of conventions confirms exactly 1 Act360 and 1 Act365F case present. parse_convention (daycount_quantlib_tests.rs:1

---

## Appendix — per-domain counts

| Domain | Dup | Dead | Holes (H/M/L) | Confirmed removals |
|---|---|---|---|---|
| Math — solvers, summation, linalg, integration | 6 | 4 | 1/7/3 | 6 |
| Math — statistics, distributions, characteristic functions | 3 | 2 | 1/4/6 | 3 |
| Math — interpolation | 6 | 4 | 2/2/0 | 7 |
| Math — RNG / Sobol / Brownian bridge | 3 | 1 | 2/2/3 | 3 |
| Math — volatility models | 1 | 0 | 0/3/2 | 0 |
| Market data — term-structure curves | 4 | 2 | 2/2/4 | 4 |
| Market data — vol surfaces / arbitrage | 2 | 1 | 3/2/5 | 3 |
| Market data — context, bumps, diff, hierarchy | 5 | 3 | 0/6/5 | 5 |
| Market data — scalars, dividends, fixings, DTSM | 2 | 3 | 3/5/3 | 2 |
| Credit | 1 | 0 | 0/3/7 | 0 |
| Dates | 7 | 4 | 0/6/8 | 8 |
| Cashflow | 4 | 2 | 1/3/5 | 4 |
| Money / currency / FX | 5 | 3 | 0/6/5 | 6 |
| Expression engine | 3 | 2 | 2/8/3 | 3 |
| Types, errors, config, validation, explain, table | 8 | 2 | 1/4/4 | 8 |
| Cross-cutting — serde golden, QuantLib golden, canonical API | 5 | 2 | 0/0/4 | 4 |
