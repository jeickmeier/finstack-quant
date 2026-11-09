# CORE-MARKET-21: Typed hierarchy selection and ordered scenario overlap

- Decision status: **Accepted — complete hierarchy identity/selection/overlap package adopted 2026-11-08.**
- Implementation status: **Observed** in the owning paths below; the accepted typed associations, strict selection, stable ordering and state/host reconciliation are unimplemented.
- Validation status: **Source evidence and independent resolver, consumer and state/host reviews inspected; 25 standalone selector/state/80-digit Decimal/JavaScript-order cases executed 2026-11-08; production qualification pending.** Existing tests were inspected or located, not executed. Selection examples do not certify stress calibration, pricing, risk or complete scenario-execution atomicity.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Scope and accepted authority

This record owns MarketDataHierarchy's structural identity, path/tag selection,
typed source associations, deterministic result ordering and the hierarchy
precedence used to expand supplied scenario actions. It preserves
[accepted20's typed targets and atomic context lifecycle](context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle)
and [accepted source-family decisions01–19](README.md). Hierarchy membership
is organizational metadata, not a financial source, quote convention,
instrument dependency or proof that a source exists or is suitable.

[Strict numeric wire/canonical identity](../core-numerics/numeric-wire-and-canonical.md#accepted-decision-strict-numeric-wire-and-canonical-content-identity)
and [bounded contract loading](../core-numerics/contracts-and-diagnostics.md#accepted-decision-bounded-contract-loading-validation-and-diagnostics)
remain the shared ingress/identity owners. This record chooses the hierarchy
shape needed to preserve intentional order; it introduces no second codec,
target registry, generic selection language or transaction framework.

[Scenario hierarchy expansion](../scenarios/03-hierarchy-resolution.md) is the
actual financial consumer. Scenario time-roll sequencing, instrument/statement
mutation and aggregate execution remain at their owners. The
[issuer credit-factor hierarchy](../attribution/11-credit-factor-hierarchy.md)
is a different factor-model structure; this decision does not adopt its beta
ordering, spread replay or allocation. Accepted
[market-change measurement22](market-change-measurement.md) owns comparison
quantities, coordinates and aggregation; selection alone does not qualify them.

## Foundational findings

- One selector can reach a source through multiple matching tagged ancestors.
  Those traversal occurrences describe selection evidence, not independently
  authored shocks. They must not multiply one requested action.
- Separately supplied actions are different: order and repetition can be
  intentional. MostSpecificWins filters hierarchy depth; it is not permission
  to collapse equal-depth actions, infer direct-target overrides or net changes.
- Source identity must include the namespace/family/subkind as accepted20
  requires. A bare X cannot say whether a hierarchy leaf means a discount
  source, scalar price or volatility surface.
- Structural validity and active binding are separate. An unattached hierarchy
  may describe sources not yet loaded; an active selected missing or wrong-kind
  source cannot become a warning-driven skipped shock.
- Intentional tree order cannot depend on JSON object-member order. JavaScript
  can reorder integer-like object keys, and accepted canonical identity sorts
  object keys. Ordered root/child/association sequences carry the actual order.

## Accepted decision: typed selection and explicit ordered overlap

The complete package was accepted on **2026-11-08** following independent
resolver, financial-consumer and state/host reviews. Runtime/caller/host
reconciliation and independent financial qualification remain pending;
acceptance does not certify the observed implementation.

1. **Reuse exact typed source identity.** Replace bare CurveId associations with
   accepted20's canonical core target: exact namespace/family/subkind and
   nonblank ID. One exact typed source may be placed once anywhere in a tree,
   including at most once at a single node. Same ID spelling in distinct
   namespaces remains legal; one concrete curve-store variant per ID still
   applies when binding to a context. Never infer type from the node name,
   tags, ID spelling, context lookup priority or available same-ID alternatives.
   Associate sources at any node, not only terminal nodes. Do not introduce a
   parallel hierarchy-only target type or force core to depend on scenarios.

2. **Use one exact path grammar and checked construction.** A selected path is
   a nonempty ordered sequence of segments. Each segment is nonblank and cannot
   contain slash; the string builder splits on slash and validates the same
   segments used by typed vectors and serialized nodes. Reject leading,
   trailing or repeated slash producing empty segments. Case and all retained
   characters, including surrounding spaces in a nonblank segment, are exact;
   do not trim, case-fold, resolve relative paths, invent wildcard roots or
   add an escape grammar. A root has depth1. Different parents may contain
   children with the same name; duplicate sibling/root names are errors before
   ordered-map collection. Empty hierarchies are structurally valid. Validate
   target/filter/predicate fields at every canonical Rust construction, wire
   and host ingress, including unknown-field rejection. Builder tag/source
   assignment without a selected valid node errors rather than doing nothing;
   a later valid selection cannot clear an earlier construction failure.
   Intentional later builder tag replacement remains explicit supplied order.

3. **Keep local tags as subtree selectors.** Predicates inspect the visited
   node's own tag map; no ancestor/descendant tag merge is performed. A matching
   node selects its entire subtree even if a descendant has a contrary tag.
   All predicates in a filter are ANDed at that one node. Equals and In compare
   exact strings; Exists tests presence, including an explicitly empty value.
   Tag keys are nonblank; values remain literal metadata. An empty In list is
   false. Canonicalize an empty predicate list to the same unfiltered selector
   as an absent filter, so it cannot create deeper specificity or extra actions.
   Traversal continues through nonmatching nodes to find matching descendants.
   These semantics are a deliberate branch-selection rule, not inherited tag
   override semantics or a claim about an external financial standard.

4. **Return one typed target per selector in placement order.** Locate the
   exact selected path and traverse its subtree in stable preorder: sources
   attached to the current node in their supplied order, then children in their
   supplied order. For an unfiltered selector each selected source has the
   target-path depth. For a filtered selector its matched depth is the maximum
   depth of matching ancestor nodes inside the selected subtree whose subtree
   contains that source. Emit the source once at its placement-order position,
   carrying that deepest matching-node depth/path; do not move it to the last
   or deepest traversal occurrence. Leaf placement depth is not specificity.
   Remove per-selector Cumulative occurrence multiplication and the split
   core/scenario interpretation. An optional read-only get_node may still
   return absence; active resolution is fallible and does not silently return
   an empty successful action plan.

5. **Project by declared family, then bind actual active sources strictly.**
   A hierarchy operation's declared source family restricts its structural
   candidate associations; a mixed branch may contain unrelated families
   without consulting those sources. This is an explicit type projection,
   not an availability filter. Missing path, no matching subtree, or no
   applicable typed associations is an active-selection error, including a
   zero-sized shock. Missing hierarchy also errors when hierarchy actions are
   requested. A selected active typed target must exist with its declared kind
   in the actual execution context; absence, wrong-kind state or unsupported
   active action errors without same-ID fallback or warning-driven omission.
   Validate supplied syntax, controls, units and structural selectors before
   precedence; qualify source behavior only for the retained active actions.
   Suppressed candidates and unrelated metadata do not trigger unused-source
   lookup. Do not require every organizational association to exist when a
   standalone hierarchy is built, attached or restored.

6. **Make cross-operation hierarchy precedence explicit.** Keep the documented
   MostSpecificWins default for a scenario, persisting its effective mode in
   canonical state. Each supplied hierarchy operation first produces its
   unique per-selector candidates. MostSpecificWins then finds the maximum
   matched depth across hierarchy operations for each exact typed source and
   retains every candidate at that depth. Distinct operations tied at that
   depth remain distinct ordered actions, even if their payloads are identical.
   Specificity is per whole source, not per parameter bucket or shock geometry:
   a deeper node action can suppress a broad parallel action on that source.
   Cumulative retains one candidate per supplied selector/source at every
   depth; ancestor traversal itself never adds extra copies.

   Direct-target actions are explicitly authored ordered actions outside
   hierarchy specificity. They remain in their supplied positions and compose
   with retained hierarchy actions; do not infer that a direct action overrides
   a hierarchy action or assign it artificial infinite depth. To replace a
   broad hierarchy stress, use an explicit deeper hierarchy selector under
   MostSpecificWins. Repeated direct and equal-depth actions are not conflicts
   to deduplicate. This preserves accepted20's supplied-order action contract.

7. **Preserve action order and reject implicit mode changes.** Replace each
   hierarchy operation's original slot with its retained candidates in stable
   placement order; keep other operation slots unchanged. Never sort active
   actions by depth, ID, units or family, combine equal targets, sum shocks
   first, or remove intermediate-invalid steps. Family owners still validate
   each active action in sequence and own its actual units/domains. Composing
   scenario specifications with different effective resolution modes errors;
   do not silently promote them to Cumulative. Empty composition retains the
   documented default. This chooses overlap only; obsolete hazard quote proxies,
   volatility floors and other source behavior remain subject to accepted
   financial owners rather than being restored by hierarchy dispatch.

8. **Freeze structural membership once and retain exact targets.** Expansion
   uses the declared opening hierarchy snapshot once. Retain the resulting
   typed identities, source operation positions and selection depths through
   explicit conditioning or other execution phases; do not silently reselect
   a changing hierarchy or add newly loaded sources midway. Bind/qualify each
   retained action against the actual market state where that action executes.
   A missing or ineligible target after conditioning errors, without silently
   dropping it or changing the selection basis. Unrelated future sources are
   not added. Context-owned actions use accepted20's private candidate and
   atomic publication, preserving the base on selection/binding/action failure.
   This does not certify atomic instrument, statement, book or external-provider
   mutation, or adopt the broader Draft time-roll ordering/carry methodology.

9. **Carry intentional hierarchy order in arrays.** Canonical roots and
   children are ordered sequences of name/node pairs, with source associations
   in ordered typed sequences. Reuse existing ordered containers and Serde
   sequence support where it meets the contract; validate duplicate names and
   associations before any lossy map collection. Tags remain exact-key maps
   whose iteration does not determine action order. Node/tag object-field order
   is not an alternative source of hierarchy order. Numeric-looking node names
   such as 2 then 1 retain that sequence through Rust, Python and JavaScript.
   Canonical identity therefore retains meaningful array order while sorting
   ordinary object keys under accepted numerics24.

   Reconcile the owning context/calibration/scenario state and schema shape
   through the existing strict loader, and reject the obsolete root/child
   object-map shape. Shared SchemaVersion and ContractDescriptor retain their
   accepted v1-only domains; this record does not introduce a global version2,
   dual reader, migration path or second codec. Raw duplicate object fields
   and tag keys still error before Value/map materialization under accepted
   numerics24/25. Count every supplied node/association before deduplication,
   and apply actual container-depth and resource bounds to the new arrays;
   hierarchy nesting gets no exception to the accepted96-container ceiling.
   Do not claim original order/duplicates can be recovered from a host object
   that has already erased them.

10. **Reconcile existing owners and preserve selection evidence.** Canonical
    Rust owns the target/tree/selector/overlap algorithms. Reconcile core
    builder/resolver/state, calibration's hierarchy transport, scenario specs/
    expansion/composition, existing context and scenario host APIs, Python
    stubs, WASM declarations, schemas, parity ledger and examples together.
    Keep wrappers as conversion/error mapping, without host selection logic,
    bare-ID fallback or alternate policy defaults. Shared loading and structural
    validation apply to direct target JSON as well as nested market state.

    Existing application diagnostics/reports should identify source operation
    position, selector path/filter, resolved typed target, matched depth/path
    and retained/suppressed status sufficiently to explain the action sequence.
    Active selection failures are errors; successful structural selection is
    not portfolio coverage, stress-severity calibration, financial replay or
    independent risk validation. No persistent planner/registry framework is
    required to make those facts reviewable.

## Observed implementation and discrepancies

The current builder parses slash strings and remembers the first invalid-path
error, but tag/curve calls without an active node silently do nothing. Tree
validation rejects empty names and duplicate bare CurveIds globally. Serialized
names containing slash or only whitespace remain possible; namespace-qualified
associations and the accepted common segment validation are absent.

Local predicates and subtree inclusion already behave as described. However,
None and an empty filter differ: the latter matches every visited node. Core
Cumulative retains all ancestor/descendant occurrences. MostSpecificWins keeps
only deepest occurrences and can reorder unique targets relative to placement.
Scenario expansion already deduplicates each selector with MostSpecificWins,
even under scenario Cumulative. The accepted core resolver removes this split.

Scenario overlap already retains all maximum-depth ties, preserves direct
operations and their positions, and uses typed scenario target keys. Missing/
zero matches and missing typed objects currently produce warnings/skips.
Mixed-mode composition silently selects Cumulative. Expansion runs before time
roll; the engine's broader execution is explicitly non-atomic. Accepted20
context staging must not be mistaken for evidence that all execution surfaces
already roll back.

Roots/children currently use IndexMap object maps and curves use bare-ID vectors.
Tree deserialization validates structure, while targets/filters use less strict
derived ingress; Python target construction/from_json has separate unchecked/
plain-Serde routes. Context restore retains hierarchy as metadata without
requiring every association in the market. Preserve that structural/binding
separation. Generated outer hierarchy schema and custom deserialization also
need unknown-field agreement. These are inspected control-flow/schema findings,
not executed production selection or financial defect reproductions.

## Independent examples and qualification limits

On **2026-11-08**, 24 standalone abstract selector/identity/state/80-digit Decimal
cases and one JavaScript object-versus-array order case passed without production
imports. The examples distinguish current control-flow consequences from the
accepted contract. They do not run the Rust resolver, scenario engine, financial
host reconstruction or context lifecycle.

- Same spelling X in price and volatility namespaces is two typed associations;
  repeating the exact same typed association is invalid.
- Under a tagged root R and tagged child A holding X, one selector reaches X
  twice in current core Cumulative but produces one accepted action at depth2.
  An unfiltered selector and an empty-filter selector become identical.
- R with X in child A and Y in child B can currently emit Y,X after deepest
  occurrence filtering; the accepted unique result order remains X,Y with
  depths2,1. A matching region=US parent includes an explicitly region=EU child.
  Conversely, region=US on a parent and rating=A only on a child cannot satisfy
  a two-predicate AND filter at either node.
- On an illustrative positive level100, separately supplied broad +10% and
  deeper +20% actions give120 under MostSpecificWins and132 under Cumulative.
  Two equal-depth +10%/+20% actions also give132; an explicitly authored direct
  action remains present rather than becoming an inferred override.
- Direct native +10 then hierarchy factor1.2 gives132, while the reverse order
  gives130. Repeating authored factor1.1 twice gives121; +10% then -10% gives99,
  not a net-zero shock. These Decimal examples show action composition only,
  without adopting a new price/rate/volatility shock convention.
- A valid structure containing an unavailable source may be stored; actively
  selecting that typed source errors. An unrelated family may be projected
  out, but the absence of a matching-family source cannot be projected away.
  Missing paths/zero applicable matches still error for a zero shock.
- Structural membership frozen to X does not acquire Y after conditioning.
  If X becomes unavailable, execution errors rather than reselecting Y.
  A later candidate failure leaves the owned base unchanged.
- Ordered root names2,1 survive an array representation. A JavaScript plain
  object parsed from roots2,1 can enumerate/stringify as1,2 instead. Repeated
  sibling-name pairs must fail before ordered-map collection; repeated raw
  object fields/tag keys still require lossless shared ingress.

Independent production qualification must cover each constructor/loader/host,
same-ID typed isolation, path/tag grammar and local predicate semantics,
empty-filter equivalence, unique selection and stable placement order, deepest
matching-node metadata, mixed-family projection and strict active binding;
all overlap modes, equal-depth/direct/repeated actions and noncommuting order;
mixed composition, missing/no-match/zero controls, conditioning and atomic
context failure; ordered state/canonical identity, duplicate/resource bounds,
schema agreement, diagnostics and actual Rust/Python/WASM behavior. Source
inspection, abstract traces and serialization success do not establish those
results or certify a financial stress calibration.

## Evidence and alternatives

Owners: [tree/validation](../../../../finstack-quant/core/src/market_data/hierarchy/mod.rs),
[builder](../../../../finstack-quant/core/src/market_data/hierarchy/builder.rs),
[resolver](../../../../finstack-quant/core/src/market_data/hierarchy/resolution.rs),
[scenario expansion](../../../../finstack-quant/scenarios/src/engine/hierarchy.rs),
[scenario validation/composition](../../../../finstack-quant/scenarios/src/spec.rs),
[execution phases](../../../../finstack-quant/scenarios/src/engine/mod.rs),
[calibration transport](../../../../finstack-quant/calibration/src/api/context_builder.rs),
[context state](../../../../finstack-quant/core/src/market_data/context/state_serde.rs),
[Python hierarchy targets](../../../../finstack-quant-py/src/bindings/scenarios/operation_spec/hierarchy.rs),
[Python context state](../../../../finstack-quant-py/src/bindings/core/market_data/context.rs)
and [WASM context state](../../../../finstack-quant-wasm/src/api/core/market_context.rs).
Existing [core hierarchy tests](../../../../finstack-quant/core/tests/market_data/hierarchy.rs)
and [scenario hierarchy tests](../../../../finstack-quant/scenarios/tests/hierarchy_targeting/mod.rs)
were inspected or located, not executed.

[RFC8259](https://datatracker.ietf.org/doc/html/rfc8259#section-4) distinguishes
unordered objects from ordered arrays and discusses duplicate-member ambiguity;
this supports the transport choice, not a financial precedence rule.
[IndexMap's documented ordered iteration and Serde sequence support](https://docs.rs/indexmap/latest/indexmap/)
provide existing implementation building blocks. Selection, strict missing-target
errors, equal-depth/direct composition and mixed-mode rejection are this
contract's explicit design choices. No external pricing standard mandates them.

Inherited tag maps, specificity for direct actions, last-action-wins ties,
automatic duplicate-action removal and warning-only empty selections would be
different contracts. This package chooses local subtree selectors and explicit
ordered authored actions, without adding policy toggles for those alternatives.
Scenario timing and issuer-factor methodologies retain their separate owners;
accepted22 owns the market-difference methodology.

## Follow-up tracking

Runtime reconciliation and independent qualification remain pending and are
tracked in the [core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
