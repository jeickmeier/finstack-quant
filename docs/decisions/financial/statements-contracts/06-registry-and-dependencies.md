# STATEMENTS-CONTRACTS-006: Metric registration, dependency discovery and namespace rewriting

**Decision status:** Accepted for the full metric registration, dependency and model import contract (2026-11-15).

**Implementation status:** Reconciled (2026-11-17). A custom document and the whole built-in catalogue each register atomically; namespaces and metric IDs follow the shared identifier grammar, `cs` is reserved and qualified IDs are unique; each formula's references are bound at registration against staged and committed definitions (unqualified in its own namespace, exact qualified in any namespace, otherwise a model requirement) and stored as a qualified formula, its dependencies and its requirements, which later registrations leave intact; references are qualified in the AST (`StmtExpr::map_node_refs`) and the textual rewriter is deleted; a same-period cycle in a document is rejected while a positive literal `lag` or `shift` reads the past; the dependency closure follows local, qualified and historical references in a deterministic order; a model import adds the metric and its closure atomically, shares a node holding the same unchanged template and rejects a conflicting definition, with overrides made by `replace_node` after import. `requires`, `unit_type` and tags remain descriptive.

**Validation status:** Qualified by a metric ID that is also a function name, a qualified cross-namespace reference in the closure, a later registration that does not rebind an earlier metric, two imports sharing an unchanged template, a conflicting import that adds nothing, and atomic catalogue registration.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Scope inventory](README.md).

## Accepted decision for metric registration and dependencies

1. Register each custom document and the entire built-in catalogue atomically. Validate dotted namespace paths and metric IDs against the shared grammar, reserve the `cs` namespace for capital references and require unique qualified IDs. [Formula identifiers and namespaces](04-dsl-syntax-and-lowering.md#accepted-decision-for-formula-grammar-function-dispatch-and-ast-validation).
2. Pin every formula's reference bindings at registration against staged and committed definitions. An unqualified metric reference binds in its own namespace. An exact qualified reference can bind in any namespace. Unmatched references become model requirements. Later registrations leave pinned bindings intact. Qualify node references in the AST, deriving dependencies from the formula. Treat `requires`, `unit_type` and tags as descriptive metadata.
3. Build deterministic dependency closure, including historical references. For the value input of `lag` or `shift`, only a valid positive integer literal offset makes the dependency past-only. Offset and other control expressions still contribute dependencies. Zero and dynamic offsets keep the value dependency in the same-period graph. Reject future reads and same-period cycles.
4. Import the requested metric and its dependency closure into the model atomically. Share an existing node that uses the same unchanged template, and reject conflicting definitions. Apply intentional overrides through an explicit named edit and the normal model revalidation boundary. [Named atomic edits](02-authoring-and-availability.md#accepted-decision-for-authoring-imports-and-observation-availability) · [Runnable model validation](01-model-timeline-and-values.md#accepted-decision-for-model-timelines-values-and-readiness).

## Audited input domain

The sections below describe the implementation audited before reconciliation.

Registry stores reusable formulas under namespace.metric_id. Loading one JSON document is transactional: no definition is committed until all definitions and the document's dependency graph validate. Built-in loading repeats that process over four embedded files, so earlier documents survive a later document failure.

Metric IDs must match ASCII letter/underscore followed by letters/digits/underscores; a name must be nonempty and the formula must parse and compile. Namespace validation only rejects the empty string. The requires list, unit_type, tags and descriptive metadata are stored but are not authoritative dependency or financial-unit validation.

## Audited algorithm and controls

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

The current builder reads the stored metric namespace, and regression coverage exercises qualification within a dotted namespace. Qualification remains textual, so a function name can still collide with a metric identifier. [Current builder](../../../../finstack-quant/statements/src/builder/model_builder.rs) · [Dotted-namespace regression](../../../../finstack-quant/statements/tests/registry_dynamic.rs).

## Evidence

[Registry](../../../../finstack-quant/statements/src/registry/dynamic.rs), [definition validation](../../../../finstack-quant/statements/src/registry/validation.rs), [formula utilities](../../../../finstack-quant/statements/src/utils/formula.rs), [topological sort](../../../../finstack-quant/statements/src/utils/graph.rs), [builder insertion](../../../../finstack-quant/statements/src/builder/model_builder.rs), [registry tests](../../../../finstack-quant/statements/tests/registry_dynamic.rs) and [metric integration tests](../../../../finstack-quant/statements/tests/spec/registry_tests.rs) are the evidence. Inspected tests cover atomic rejected-document behavior, corrected retry, dependency order, duplicate names and built-in/selective insertion parity. No focused runtime probe of qualified-reference or function-name collisions was executed.

Registry contract gaps are distinct from the numerical correctness of each stored metric.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
