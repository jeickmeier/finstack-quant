# COVENANTS-01: Contract terms, identity and metric selection

**Decision status:** Accepted 2026-11-17 for explicit contract terms and metric bindings.

**Implementation status:** Exact label identity and explicit metric lookup are implemented. Removing implicit metric, denominator and contract-term defaults, aligning source validation and preserving contractual monetary precision remain pending.

**Validation status:** All 115 Covenants Rust tests passed on 2026-11-17 against the observed implementation. The accepted changes and host-entry parity still need qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Accepted decision

1. Bind each numeric covenant to one explicit, exact metric ID for both assessment and forecast. Require an explicit denominator ID for net leverage. Remove implicit built-in, custom and basket metric lookup and the automatic `ebitda` denominator. A missing selected metric stays missing, even when the source supplies another conventional ID. [COVENANTS-03](03-comparison-and-headroom.md) owns other denominator requirements and ratio validity.
2. Require operative scope, activation, test frequency and an explicit cure-period choice, including an explicit choice of no cure period. Retire the generic 30-calendar-day cure default. Keep the exact, nonblank covenant label as the instance identity in reports, waivers and breach history. Enforce unique labels within each effective set. [COVENANTS-05](05-breach-and-cure.md) owns cure mechanics and [COVENANTS-11](11-package-templates.md) owns named template terms.
3. Reject blank metric IDs and duplicate keys at ingestion before collection can overwrite values. Reads for an applicable test distinguish a missing observation from an invalid nonfinite value. Construction, restoration, assessment and forecast share Rust validation for their applicable inputs after host type conversion.
4. The producer supplies calculation-ready measures with the contract definition, adjustments, units, currency and reporting period. Preserve Decimal or `Money` precision wherever the agreement requires exact monetary comparison. The [statement adapter contract](../statements-analytics-credit/11-covenant-statement-adapter.md#accepted-decision-for-verified-statement-covenant-assessments) and [monetary precision convention](../conventions/0020-numerical-representation-and-rounding.md) govern these boundaries.

## Observed method and controls

The engine consumes already calculated scalar covenant measures; it does not construct debt/EBITDA, net debt, interest coverage, fixed-charge coverage, DSCR, asset coverage, capex, liquidity or basket usage from components. Ratios are in turns, yield/LTV template metrics in decimal fractions, and monetary inputs in the caller's common reporting-currency scale. There is no currency field, FX conversion, period aggregation, annualization, LTM construction or accounting-adjustment reconciliation. A metric identifier conventionally names a statements node, but this crate has no statements dependency.

Lookup precedence in `spec_metric_name` is exact explicit `metric_id`, then the built-in default name, then the `Custom.metric` or `Basket.name`. An explicit missing identifier never falls back. Defaults are `debt_to_ebitda`, `interest_coverage`, `fixed_charge_coverage`, `total_leverage`, `senior_leverage`, `asset_coverage`, `dscr`, `net_debt_to_ebitda`, `capex`, and `liquidity`. `HashMapMetricSource::from_pairs` collects a map, so repeated identifiers replace earlier values; `insert` also replaces. Values are not checked on insertion, and unavailable reads return `NotFound`. Evaluation rejects nonfinite values only for inputs actually read. `from_json` instead requires an object of JSON numbers and rejects nonnumeric values even for unused keys.

`Covenant::new` selects active maintenance status, a 30-calendar-day cure period, no springing trigger and no consequences. `test_frequency` is descriptive metadata. Labels are exact report/history/waiver identities; only all-whitespace labels are rejected, with no trimming or case normalization. Type-discriminant IDs and human-readable descriptions are not instance identities. Duplicate labels are rejected within each top-level/window set, while reuse across distinct windows is allowed. Explicit metrics and denominator names must not be empty; custom/basket fallback names are not equivalently checked.

`CovenantSpec::new` and `with_metric` automatically attach `ebitda` as denominator only for net debt/EBITDA. A caller may override it for covenant-adjusted earnings. Public-field construction and ordinary derived serde do not run these constructors: a missing net denominator becomes `None` and subsequently fails validation. Thresholds need only be finite, including negative values; templates impose a narrower nonnegative domain.

## Rationale, alternatives and limitations

The source separates calculation-ready measures from contract tests and uses labels to prevent same-type report collisions. Producers own EBITDA, currency, cash-netting and period definitions. The engine compares the supplied measure under the accepted [units and signs](../conventions/0007-units-and-sign-conventions.md), [missing and invalid distinctions](../conventions/0009-missing-and-invalid-data.md), and [monetary precision rules](../conventions/0020-numerical-representation-and-rounding.md).

## Evidence

- Owning [metric source](../../../../finstack-quant/covenants/src/metric.rs), [terms and constructors](../../../../finstack-quant/covenants/src/engine/types.rs), [metric precedence](../../../../finstack-quant/covenants/src/engine/helpers.rs), and [engine validation](../../../../finstack-quant/covenants/src/engine/covenant_engine.rs).
- Inspected `explicit_missing_metric_never_falls_back_to_type_default` in [forward tests](../../../../finstack-quant/covenants/src/forward.rs), `metric_less_spec_shares_net_leverage_denominator_and_round_trips_through_json` and `evaluate_engine_with_source_matches_typed_evaluate_for_non_finite_metrics` in [audit regressions](../../../../finstack-quant/covenants/tests/audit_regressions.rs), and numeric-map tests in `metric.rs`.

No independent legal-contract or financial-input qualification was located in the inspected evidence.

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[covenant follow-ups](../FOLLOW_UPS.md#covenants).
