# COVENANTS-04: Scope selection, springing activation and descriptive covenants

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted decisions retain their scope.

**Implementation status:** Observed in the exact source paths identified below, including stated discrepancies and limitations.

**Validation status:** Source and identified test assertions inspected; tests, builds, numerical experiments and external benchmarks not executed. Test existence is not adoption or independent qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Observed evaluation pipeline

`evaluate` validates the entire engine, chooses the applicable set, then evaluates both maintenance and incurrence specifications as snapshot compliance/capacity checks. `evaluate_and_track` explicitly filters by the requested scope before evaluation. Batch forecasting filters by `config.scope` (maintenance by default); a single-covenant forecast always processes its supplied spec irrespective of that config field. Series evaluation includes both scopes and never records history.

Within an applicable set, an inactive covenant returns a pass with `Covenant inactive`. Next, a full active waiver returns a pass with `Waived by lender agreement`. Both branches skip all metric reads. An active nonwaived springing covenant reads and validates its trigger metric, then activates for `Maximum(t)` when `v <= t` or for `Minimum(t)` when `v >= t`; equality activates. An unmet trigger returns a pass with `Springing condition not met`, without actual, threshold or headroom. Missing/nonfinite trigger values fail rather than silently deactivating. Forecasting uses the same short-circuit order and treats inactivity as zero probability with no projected value or headroom.

After those gates, `Negative` and `Affirmative` covenants have no numeric bound/threshold. Point-in-time evaluation returns a pass with no assessment of the restriction/requirement and no unsupported-test detail. An explicit metric on such a covenant is not read; a springing trigger, if present, is still read first. Batch forecasts omit these descriptive tests; single-spec forecasts reject them even when inactive. `CovLite` therefore carries its additional-secured-debt restriction as descriptive text, not an executable authorization test.

## Scope, rationale and alternatives

The source explicitly distinguishes scheduled maintenance from a completed incurrence action: callers must not record failed hypothetical capacity as an actual breach. There is no action object or evidence of execution checked by the engine. `test_frequency` does not create dates, enforce quarter ends, or annualize capex. Trigger values are deterministic observations in both stochastic modes; no stochastic revolver-utilization or joint trigger/ratio process is simulated.

Skipping metric reads for inactive/waived tests supports incomplete model histories without substituting missing values. A tri-state assessed/passed/not-tested result or a dedicated qualitative evaluator would avoid interpreting descriptive and inactive passes as verified contractual compliance, but neither is implemented. Independent governance must determine which covenants can be tested numerically and when an incurrence transaction has actually occurred.

## Evidence

- Owning [scope/default declarations](../../../../finstack-quant/covenants/src/engine/types.rs), [evaluation order](../../../../finstack-quant/covenants/src/engine/covenant_engine.rs), [forecast filtering/activation](../../../../finstack-quant/covenants/src/forward.rs), and [series delegation](../../../../finstack-quant/covenants/src/series.rs).
- Inspected inline [engine tests](../../../../finstack-quant/covenants/src/engine/mod.rs) for finite/NaN triggers and inactivity; [audit regressions](../../../../finstack-quant/covenants/tests/audit_regressions.rs) for `incurrence_requires_explicit_tracking_scope` and `non_testing_reports_do_not_cure_breaches`; forward test `forecast_breaches_generic_skips_non_numeric_covenants`.

Existing source comments document these boundaries but do not independently qualify a legal compliance conclusion.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
