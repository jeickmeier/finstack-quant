# STATEMENTS-CONTRACTS-006: Metric registration, dependency discovery and namespace rewriting

**Decision status:** Draft (observed implementation; adoption pending).

**Implementation status:** Observed for the source responsibilities stated below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice README](README.md)).

## Choice and input domain

Registry stores reusable formulas under namespace.metric_id. Loading one JSON document is transactional: no definition is committed until all definitions and the document's dependency graph validate. Built-in loading repeats that process over four embedded files, so earlier documents survive a later document failure.

Metric IDs must match ASCII letter/underscore followed by letters/digits/underscores; a name must be nonempty and the formula must parse and compile. Namespace validation only rejects the empty string. The requires list, unit_type, tags and descriptive metadata are stored but are not authoritative dependency or financial-unit validation.

## Algorithm and controls

1. Reject existing or within-document duplicate qualified IDs. Parse and compile every new formula, retaining the AST.
2. Collect all registered and staged unqualified IDs in that same namespace. Extract all AST references, including references inside lag/shift, and retain only exact names in this set.
3. Kahn-sort the staged dependency graph. Dependencies outside its keys are treated as external/already satisfied. Ready nodes are kept in a vector and popped from the end, so otherwise independent ties follow deterministic reverse-ready insertion order. The cycle error reports the residual unscheduled set; it need not be one exact cycle path.
4. Stage StoredMetric entries in the resulting order and extend the live registry only after successful staging.
5. get_dependencies recursively traverses recognized same-namespace references in AST encounter order, with a visited set, appending each dependency after its own dependencies. ModelBuilder::add_metric_from_registry stages that closure and the requested metric, retaining an existing node with the same qualified ID. with_builtin_metrics adds all absent metrics.
6. Builder qualification replaces recognized unqualified metric names in formula text, longest names first. Alphanumeric/underscore/dot adjacency prevents partial or already-qualified replacement. The helper caps each identifier's replacements at 1,000,000.

For same-period model dependency analysis, a separate AST walk omits the first argument of lag/shift when the offset is a nonzero Literal. A zero literal or nonliteral offset retains that input dependency; control-argument references are always retained. Registry dependency discovery deliberately does not apply this temporal exclusion.

## Limitations and alternatives

Fully qualified references such as fin.a are not in the registry's unqualified set {a}; neither qualified nor cross-namespace references are automatically included in the dependency closure or this load-time cycle graph. They remain formula/model references. A self-lag using an unqualified metric name is a registry cycle even though model-level temporal recurrence can be meaningful.

Namespace punctuation and names that collide with function names are not rejected here. Qualification is textual, not an AST node-reference rewrite: a metric called sum can cause sum(...) text to be rewritten as a namespaced function call. These static edge cases limit the currently established naming contract.

AST-based qualification and a graph over fully qualified references would remove those ambiguities, but would change the current contract. Automatically using requires instead could drift from the executed formula; retaining one formula-derived dependency source is preferable unless explicitly changed.

## Evidence

[Registry](../../../../finstack-quant/statements/src/registry/dynamic.rs), [definition validation](../../../../finstack-quant/statements/src/registry/validation.rs), [formula utilities](../../../../finstack-quant/statements/src/utils/formula.rs), [topological sort](../../../../finstack-quant/statements/src/utils/graph.rs), [builder insertion](../../../../finstack-quant/statements/src/builder/model_builder.rs), [registry tests](../../../../finstack-quant/statements/tests/registry_dynamic.rs) and [metric integration tests](../../../../finstack-quant/statements/tests/spec/registry_tests.rs) are the evidence. Inspected tests cover atomic rejected-document behavior, corrected retry, dependency order, duplicate names and built-in/selective insertion parity. No focused runtime probe of qualified-reference or function-name collisions was executed.

Registry contract gaps are distinct from the numerical correctness of each stored metric.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
