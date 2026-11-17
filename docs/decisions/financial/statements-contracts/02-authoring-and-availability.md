# STATEMENTS-CONTRACTS-002: Builder composition and observation availability

**Decision status:** Accepted for the full builder composition and observation availability contract (2026-11-15).

**Implementation status:** Reconciled (2026-11-16). Node ids and import rows follow the formula identifier rules. A repeated node id, period or availability date is rejected; an existing node changes through `replace_node`, `remove_node` or the atomic `FinancialModelSpec::apply_edits` batch of named `ModelEdit`s. `where_clause` names its target. `import_observations` needs a set timeline, maps unique period labels in any order, keeps missing cells absent and zeros known, and is atomic. `Evaluator::evaluate_with_market` takes an explicit `ObservationCutoff`; a cutoff date needs a model `source_snapshot` and a publication date on every observation in an actual period. The audited sections below describe revision `7e82fd03ad`.

**Validation status:** Duplicate rejection, named edits, sparse and unordered imports, and the April 25 / May 15 revision replay are qualified in the [model regression tests](../../../../finstack-quant/statements/tests/finance_model_regressions.rs) and [builder tests](../../../../finstack-quant/statements/src/builder/model_builder.rs).

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Scope inventory](README.md).

## Accepted decision for authoring, imports and observation availability

1. Use the shared DSL identifier rules and reserved-name rules in authoring and import. Reject duplicate node definitions, observations and availability-date entries for the same node and period on creation or import. Apply changes through explicit named edits that commit atomically. Attaching a `where` clause requires a valid named target and scalar predicate.
2. Attach a forecast to an existing named node while preserving its formula and explicit observations. Promote Value and Calculated nodes to Mixed when attaching a forecast. Create forecast-only nodes explicitly as Mixed.
3. Require a validated explicit period plan for imports. Map columns by period label, allowing unordered, unique columns and sparse observations. Reject any column outside the plan. Check observation types and units, retaining known zero separately from a missing cell. [Model timeline and explicit values](01-model-timeline-and-values.md#accepted-decision-for-model-timelines-values-and-readiness).
4. Point-in-time evaluation uses an identified source snapshot, explicit observation availability dates and an explicit cutoff inclusive through the end of the source day. Retain original and revised observations in separately identified snapshots so a dated evaluation can replay its information set. Keep forecast assumptions distinguishable from published observations. Use explicit full-snapshot mode to evaluate the selected snapshot without a publication cutoff.

For example, a \$100 observation published on April 25 is visible at that day's cutoff. A revision to \$110 published on May 15 is visible in the revised snapshot at the May 15 cutoff. An April 25 replay uses the original snapshot and obtains \$100.

## Audited authoring and availability

ModelBuilder is mutable authoring state, with final validation at build. It stores nodes in insertion order and tracks the last touched node explicitly. NodeSpec carries an optional publication date for each explicit period observation. This slice owns the availability predicate; precedence and the actual evaluation cutoff are owned by the evaluator.

## Audited algorithm and controls

- Setting periods is allowed only while the stored timeline is empty. Both explicit and parsed timelines must be nonempty. Import-column resolution uses an existing timeline or constructs the inclusive PeriodId range from the first through last supplied column, then checks every column belongs to it; interior gaps in columns become timeline slots rather than being silently discarded.
- value/value_money/value_scalar collect period pairs into an IndexMap: repeated period IDs retain the last supplied value. Reusing a node ID through value or compute replaces the whole previous definition and logs a warning. insert_node and mixed-node finalization also replace by key. Generic values infer units later; value_scalar declares Scalar, while value_money declares the currency only if all supplied money values agree.
- compute and MixedNodeBuilder::formula reject empty text and parse/compile before storing it. Other setters can leave incomplete state until the final build.
- forecast attaches to an existing node and promotes Value or Calculated to Mixed, or creates a forecast-only Mixed node. It updates last_node. where_clause applies to that last touched node, including a redefined earlier map entry; with no node it logs and ignores the clause.
- A mixed builder accumulates optional values, forecast, formula, name and rollup. Its build validates the ID and inserts the node into its parent; the parent's build performs full model validation.
- Explicit observation availability defaults to period.end, the exclusive boundary of the half-open interval. A supplied date overrides it. With a cutoff, the observation is visible when availability_date <= cutoff; without a cutoff all explicit observations are visible.

Rollup choices are Sum, PeriodEnd and Formula. Their execution is outside this record; no rollup method is inferred by these setters.

## Limitations and alternatives

Reserved IDs include `period_id`, any ID containing `__cs__`, any ID starting `__`, and bare `if`/`and`/`or`/`not`/`true`/`false`. This is not complete DSL identifier validation: empty or punctuation-bearing model IDs are not rejected by this helper alone, even though formula references use a narrower grammar.

Repeated periods and nodes use replacement rather than conflict rejection. Publication dates need not be at or after the reporting interval, so intentional early availability and accidental look-ahead share the same input representation. A full revision history or publication calendar is not inferred.

The audited last-touched behavior and implicit replacement remain implementation gaps under the accepted named-edit contract.

## Evidence

[Builder](../../../../finstack-quant/statements/src/builder/model_builder.rs), [node availability](../../../../finstack-quant/statements/src/types/node.rs), [builder regressions](../../../../finstack-quant/statements/tests/senior_review_builder.rs), [model regressions](../../../../finstack-quant/statements/tests/finance_model_regressions.rs) and [temporal regressions](../../../../finstack-quant/statements/tests/finance_temporal_regressions.rs) are the evidence. Inline builder tests pin redefined-node targeting, forecast targeting and preservation of existing nodes when metrics are added. Rejected setter tests inspect preservation of prior state.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
