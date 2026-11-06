# COVENANTS-01: Contract terms, identity and metric selection

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted decisions retain their scope.

**Implementation status:** Observed in the exact source paths identified below, including stated discrepancies and limitations.

**Validation status:** Source and identified test assertions inspected; tests, builds, numerical experiments and external benchmarks not executed. Test existence is not adoption or independent qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Observed method and controls

The engine consumes already calculated scalar covenant measures; it does not construct debt/EBITDA, net debt, interest coverage, fixed-charge coverage, DSCR, asset coverage, capex, liquidity or basket usage from components. Ratios are in turns, yield/LTV template metrics in decimal fractions, and monetary inputs in the caller's common reporting-currency scale. There is no currency field, FX conversion, period aggregation, annualization, LTM construction or accounting-adjustment reconciliation. A metric identifier conventionally names a statements node, but this crate has no statements dependency.

Lookup precedence in `spec_metric_name` is exact explicit `metric_id`, then the built-in default name, then the `Custom.metric` or `Basket.name`. An explicit missing identifier never falls back. Defaults are `debt_to_ebitda`, `interest_coverage`, `fixed_charge_coverage`, `total_leverage`, `senior_leverage`, `asset_coverage`, `dscr`, `net_debt_to_ebitda`, `capex`, and `liquidity`. `HashMapMetricSource::from_pairs` collects a map, so repeated identifiers replace earlier values; `insert` also replaces. Values are not checked on insertion, and unavailable reads return `NotFound`. Evaluation rejects nonfinite values only for inputs actually read. `from_json` instead requires an object of JSON numbers and rejects nonnumeric values even for unused keys.

`Covenant::new` selects active maintenance status, a 30-calendar-day cure period, no springing trigger and no consequences. `test_frequency` is descriptive metadata. Labels are exact report/history/waiver identities; only all-whitespace labels are rejected, with no trimming or case normalization. Type-discriminant IDs and human-readable descriptions are not instance identities. Duplicate labels are rejected within each top-level/window set, while reuse across distinct windows is allowed. Explicit metrics and denominator names must not be empty; custom/basket fallback names are not equivalently checked.

`CovenantSpec::new` and `with_metric` automatically attach `ebitda` as denominator only for net debt/EBITDA. A caller may override it for covenant-adjusted earnings. Public-field construction and ordinary derived serde do not run these constructors: a missing net denominator becomes `None` and subsequently fails validation. Thresholds need only be finite, including negative values; templates impose a narrower nonnegative domain.

## Rationale, alternatives and limitations

The source documents separation of calculation-ready measures from contract tests and uses labels to prevent same-type report collisions. It does not establish that a particular supplied EBITDA, currency, cash-netting policy or reporting period matches a legal agreement. Explicit component construction with typed currency/period metadata would improve provenance but moves upstream accounting responsibility into this crate; that is an unadopted alternative. Accepted [units/signs](../conventions/0007-units-and-sign-conventions.md), [missing/invalid distinctions](../conventions/0009-missing-and-invalid-data.md), and [monetary boundaries](../conventions/0020-numerical-representation-and-rounding.md) remain applicable; a floating-point amount test is not certified contractual monetary arithmetic.

## Evidence

- Owning [metric source](../../../../finstack-quant/covenants/src/metric.rs), [terms and constructors](../../../../finstack-quant/covenants/src/engine/types.rs), [metric precedence](../../../../finstack-quant/covenants/src/engine/helpers.rs), and [engine validation](../../../../finstack-quant/covenants/src/engine/covenant_engine.rs).
- Inspected `explicit_missing_metric_never_falls_back_to_type_default` in [forward tests](../../../../finstack-quant/covenants/src/forward.rs), `metric_less_spec_shares_net_leverage_denominator_and_round_trips_through_json` and `evaluate_engine_with_source_matches_typed_evaluate_for_non_finite_metrics` in [audit regressions](../../../../finstack-quant/covenants/tests/audit_regressions.rs), and numeric-map tests in `metric.rs`.

No independent legal-contract or financial-input qualification was located in the inspected evidence.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
