# COVENANTS-04: Scope selection, springing activation and descriptive covenants

**Decision status:** Accepted 2026-11-17 for explicit scope, activation outcomes and qualitative assessment ownership.

**Implementation status:** Inclusive springing triggers and short-circuiting of inactive and waived tests are implemented. Explicit scope across entry points, completed-action evidence, distinct assessment outcomes and forecast coverage remain pending.

**Validation status:** The 115-test Covenants Rust baseline and all three focused springing tests passed on 2026-11-17 against current behavior. The accepted scope and outcome changes still need qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Accepted decision

1. Require explicit maintenance or incurrence scope for multi-covenant assessment and forecast. A single-spec forecast uses the supplied spec's scope. The caller provides test dates under the agreed cadence. Hypothetical incurrence assessment is read-only. Tracking incurrence requires caller evidence of a completed action, including its ID and occurrence date.
2. Apply the same gates in assessment and forecast in this order: inactive, full waiver, springing trigger, assessment. Inactive and waived tests skip metric reads. Inclusive trigger comparisons activate at equality. An unmet trigger produces an untriggered outcome. A missing trigger makes assessment incomplete, and invalid trigger data yields a validation error. Forecasts, including stochastic forecasts, use the supplied trigger observation or projection as a deterministic input.
3. Keep applicable, triggered affirmative and negative covenants visible as requiring external assessment. Reserve engine pass or fail for assessed tests. Inactive, waived, untriggered and external-assessment cases carry no engine-generated headroom or breach probability. Show the covered tests, dates and outcomes in forecast results so readers can interpret an empty breach list. [Statement assessment contract](../statements-analytics-credit/11-covenant-statement-adapter.md#accepted-decision-for-verified-statement-covenant-assessments).

## Observed evaluation pipeline

`evaluate` validates the entire engine, chooses the applicable set, then evaluates both maintenance and incurrence specifications as snapshot compliance/capacity checks. `evaluate_and_track` explicitly filters by the requested scope before evaluation. Batch forecasting filters by `config.scope` (maintenance by default); a single-covenant forecast always processes its supplied spec irrespective of that config field. Series evaluation includes both scopes and never records history.

Within an applicable set, an inactive covenant returns a pass with `Covenant inactive`. Next, a full active waiver returns a pass with `Waived by lender agreement`. Both branches skip all metric reads. An active nonwaived springing covenant reads and validates its trigger metric, then activates for `Maximum(t)` when `v <= t` or for `Minimum(t)` when `v >= t`; equality activates. An unmet trigger returns a pass with `Springing condition not met`, without actual, threshold or headroom. Missing/nonfinite trigger values fail rather than silently deactivating. Forecasting uses the same short-circuit order and treats inactivity as zero probability with no projected value or headroom.

After those gates, `Negative` and `Affirmative` covenants have no numeric bound/threshold. Point-in-time evaluation returns a pass with no assessment of the restriction/requirement and no unsupported-test detail. An explicit metric on such a covenant is not read; a springing trigger, if present, is still read first. Batch forecasts omit these descriptive tests; single-spec forecasts reject them even when inactive. `CovLite` therefore carries its additional-secured-debt restriction as descriptive text, not an executable authorization test.

## Scope, rationale and alternatives

The source explicitly distinguishes scheduled maintenance from a completed incurrence action: callers must not record failed hypothetical capacity as an actual breach. There is no action object or evidence of execution checked by the engine. `test_frequency` does not create dates, enforce quarter ends, or annualize capex. Trigger values are deterministic observations in both stochastic modes; no stochastic revolver-utilization or joint trigger/ratio process is simulated.

Skipping reads for inactive or waived tests supports sparse model histories. Explicit outcomes expose gaps in assessment coverage. The caller supplies completed-action evidence; the external assessment owner determines qualitative compliance.

## Evidence

- Owning [scope/default declarations](../../../../finstack-quant/covenants/src/engine/types.rs), [evaluation order](../../../../finstack-quant/covenants/src/engine/covenant_engine.rs), [forecast filtering/activation](../../../../finstack-quant/covenants/src/forward.rs), and [series delegation](../../../../finstack-quant/covenants/src/series.rs).
- Inspected inline [engine tests](../../../../finstack-quant/covenants/src/engine/mod.rs) for finite/NaN triggers and inactivity; [audit regressions](../../../../finstack-quant/covenants/tests/audit_regressions.rs) for `incurrence_requires_explicit_tracking_scope` and `non_testing_reports_do_not_cure_breaches`; forward test `forecast_breaches_generic_skips_non_numeric_covenants`.

The named tests passed against current behavior. Test the accepted status distinctions, missing and invalid triggers, and scope and action-evidence enforcement across entry points.

## Follow-up tracking

Implementation reconciliation and qualification are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
