# STATEMENTS-CONTRACTS-002: Builder composition and observation availability

**Decision status:** Draft (observed implementation; adoption pending).

**Implementation status:** Observed for the source responsibilities stated below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Source baseline and audit date: [slice README](README.md).

## Choice and input domain

ModelBuilder is mutable authoring state, with final validation at build. It stores nodes in insertion order and tracks the last touched node explicitly. NodeSpec carries an optional publication date for each explicit period observation. This slice owns the availability predicate; precedence and the actual evaluation cutoff are owned by the evaluator.

## Algorithm and controls

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

A strict authoring mode could reject duplicate observations or unreferenceable IDs; an explicit revision mechanism could retain provenance. Neither alternative is supplied here. The current last-touched behavior makes fluent editing predictable but does not guarantee that a where clause was intended for that node.

## Evidence

[Builder](../../../../finstack-quant/statements/src/builder/model_builder.rs), [node availability](../../../../finstack-quant/statements/src/types/node.rs), [builder regressions](../../../../finstack-quant/statements/tests/senior_review_builder.rs), [model regressions](../../../../finstack-quant/statements/tests/finance_model_regressions.rs) and [temporal regressions](../../../../finstack-quant/statements/tests/finance_temporal_regressions.rs) are the evidence. Inline builder tests pin redefined-node targeting, forecast targeting and preservation of existing nodes when metrics are added. Rejected setter tests inspect preservation of prior state.

Exclusive period-end observation visibility differs from inclusive last-day cash/balance snapshots.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
