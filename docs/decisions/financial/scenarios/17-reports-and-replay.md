# SCENARIOS-017: Application manifests, shock units and versioned replay envelopes

- Decision status: **Accepted 2026-11-15 — complete application reporting, invalidation, bounded restoration and replay package.**
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent effect-count, cancellation and scoped canonical identity examples verified, including the existing c1 fixture.** Production replay, restoration and host qualification remain pending.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Accepted decision for checked scenario reports and replay

1. **Give each count one meaning.** Report authored operations,
   active actions after expansion and precedence, changed actions,
   admitted unchanged actions and individual changed effects.
   Retain suppressed hierarchy candidates and configured binding
   outcomes separately. Required failures produce no completed
   report. Optional carry may be incomplete with typed row failures
   and explicit coverage.
   [Accepted scenario execution](02-execution-and-partial-success.md#accepted-decision-for-ordered-and-atomic-scenario-execution) ·
   [Carry coverage](14-carry-aggregation.md#accepted-decision-for-checked-scenario-roll-carry-and-native-aggregation).

2. **Report requested and delivered effects in execution order.**
   Retain action and phase identity, exact typed target, coordinates
   or occurrence path, requested units and value, and actual checked
   native before-and-after state. Retain applicable owner-qualified
   fit, fixing, statement and residual evidence. Preserve the
   sequence of changes, including changes that cancel in the final
   state. Monetary P&L requires actual repricing.

3. **Base invalidation on admitted opening and final state.**
   Keep typed market namespaces, ordered FX scope, inventory
   occurrences, statement inputs and date or shape changes. Omit
   a canceled effect from invalidation only when the owner proves
   the relevant states agree. Record the reason for broader
   repricing when dependency coverage is incomplete. Invalidate
   every FX-dependent position absent authoritative triangulation
   paths. Distinguish native repricing from reporting-currency
   refresh.
   [Shared executor and reuse](../../architecture/0046-portfolio-execution-operational-contract.md#decision-and-scope).

4. **Define independent artifact identities.** The versioned
   specification identifies admitted authored instructions. The
   application snapshot contains the admitted resulting state,
   evaluated statements, inventory and report. The elective replay
   manifest identifies original executable inputs. Use the accepted
   core `c2` canonical digest with explicit schema and canonical
   versions. Keep executable array order. Existing `c1` fixtures
   retain their historical identity.
   [Canonical content identity](../core-numerics/numeric-wire-and-canonical.md#accepted-decision-strict-numeric-wire-and-canonical-content-identity).

5. **Capture the inputs and authorities actually used.** Embed or
   reference immutable opening market, inventory and statement state,
   ordered scenario and bindings, requested and effective dates and
   information cuts, and template lineage. Retain applicable calendar
   editions, fixing provenance, FX quotes and query coverage, original
   calibration sources, effective model and provider settings, producing
   build and relevant stochastic controls. Missing or mismatched required
   evidence is a typed replay error. An admitted output snapshot remains
   inspectable. Compare replay results under the declared guarantee and
   owner-qualified numerical tolerances.
   [Audit and replay policy](../../project/0025-auditable-results-and-replay.md#decision) ·
   [Reproducibility guarantees](../../standards/0021-reproducibility-guarantees.md#decision).

6. **Restore through the bounded Rust validation path.** Require exact
   schema markers, closed shapes and lossless decoding. Reject nested
   duplicate keys before map conversion. Apply market, instrument and
   statement owner admission, then verify dates, counts, effect paths,
   results and provenance agree. Preserve exact Money and distinguish
   absent from empty inventory. Derive horizon summaries from the
   validated underlying result.
   [Bounded contract loading](../core-numerics/contracts-and-diagnostics.md#accepted-decision-bounded-contract-loading-validation-and-diagnostics).

7. **Validate required wire artifacts before publishing success.**
   When a call promises a serializable result, stage and check its
   complete artifact before publication. Rust owns the financial
   result, nullability and typed diagnostics across Python and WASM.
   Generate schemas and host declarations from this contract.
   [Atomic publication](02-execution-and-partial-success.md#accepted-decision-for-ordered-and-atomic-scenario-execution).

## Independent calculation and identity checks

- Starting with A=100 and B=80, a `+100%` action on both, a
  `−50%` action on A and a validated zero action on B give three
  authored and active actions, two changed actions, one unchanged
  action and three effects. A returns to 100 and B ends at 160.
  The three changes remain visible even when the final state of A
  qualifies for reuse.
- The same `+10%` authored instruction yields 110 from 100 and
  121 from 110. Object-key reordering preserves the specification
  digest. Different opening inputs produce different run-input
  digests. Reordering operations changes the specification digest.
- The existing canonical fixture matches its `c1` preimage.
  Substituting the `c2` prefix changes the digest even with the same
  fixture bytes.

## Observed choice and rationale at the audit revision

Observed choice: return enough typed change information for consumers to invalidate/reprice affected instruments, alongside a serializable applied context and ordered shock descriptions. A scenario content hash identifies its input specification; it is not a hash of all data needed to reproduce financial results.

## Ordered reporting semantics

`user_operations` is the original specification length; `expanded_operations` is the count after hierarchy expansion. `operations_applied` counts successful adapter effects or changed top-level instruments, not a universal one-per-request measure. A multi-ID equity request, split node shock or instrument filter can contribute several; an empty/zero bucket or unchanged structure can still count. Warning-only skips often count zero, while a rate binding with no values can count as success. Read these fields together with warnings and changes.

Market changes are keyed by typed target: curve kind+ID, volatility-index ID, base-correlation ID, vol-surface ID, price-scalar ID or ordered FX pair. Keep first-seen target order and deduplicate. Changed inventory indices are inserted in sorted unique order. A changed date sets `as_of_changed` and `all_dirty`; unexpected unclassified effects also set `all_dirty`. No operation currently changes portfolio shape. Statement-input changes do not populate a separate statement dirty-node manifest.

`AppliedShock` records each emitted shock in its own quote space: basis points, percent or absolute units. Discount/inflation key-rate effects report resolved knot time and emitted pillar amplitude; rebuilt forward/commodity/ParCDS/index operations report requested node tenors/amplitudes. Surface buckets include snapped expiries when emitted through their dedicated bump. Commodity curve fields are reported as Percent despite their `bp` input name. Correlations remain decimal absolute changes. **Only price-scalar effects record numeric before/after levels.** Clipping, quote rebootstrap and no-match effects can therefore differ from the requested shock shown in the report.

Report metadata stamps engine configuration's results metadata/rounding mode. `effective_as_of` uses `time_roll.new_date` when present, otherwise the caller's original date. A returned report exists only after successful engine completion; an `Err` after partial mutation carries no completed application report.

## Persistence and validation boundaries

`ScenarioEnvelope` uses exactly `finstack_quant.scenario/1`, rejects unknown fields, and delegates canonical content hashing to core. `from_slice_strict` decodes with caller `LoadLimits`, then performs semantic scenario validation and returns a validation report. `new` and hashing do not themselves validate market dependencies or scenario semantics. Bare `ScenarioSpec::from_json` validates but is not the versioned byte-loader path.

`ApplicationEnvelope::from_contexts` serializes market, optional model and optional canonical instrument envelopes plus report. Unsupported instrument canonical serialization errors rather than dropping the instrument. The envelope does not include an explicit rate-binding map, selected calendar object, scenario specification, engine/provider cache or complete source/provenance hash; a full replay bundle requires those inputs elsewhere. Market snapshot semantics, including quote-only FX restore, are shared core contracts.

The `json-schema` feature publishes scenario envelope/spec, template metadata, application envelope, horizon result and horizon summary schemas/examples. The schema binary only delegates generation to the core tool; it does not implement an alternate pricing or validation engine. Error kind mapping preserves lookup versus validation versus computation distinctions; structured warning variants describe skip/clamp/approximation/evaluation failures, but no unified severity or result-completeness gate is computed.

## Evidence and alternatives

Owning source: [engine/types.rs](../../../../finstack-quant/scenarios/src/engine/types.rs), [effects.rs](../../../../finstack-quant/scenarios/src/engine/effects.rs), [envelope.rs](../../../../finstack-quant/scenarios/src/envelope.rs), [schema.rs](../../../../finstack-quant/scenarios/src/schema.rs), [error.rs](../../../../finstack-quant/scenarios/src/error.rs), [warning.rs](../../../../finstack-quant/scenarios/src/warning.rs). Inspected [canonical contract test](../../../../finstack-quant/scenarios/tests/canonical_contract.rs) compares golden bytes/hash and attribute insertion-order invariance; inspected engine tests assert changed targets, shock units and correlation clamping. [Report serialization](../../../../finstack-quant/scenarios/tests/report_serialization.rs) locates output shape evidence. These validate transport/report contracts, not numerical equivalence across missing replay inputs.

A complete reproducibility bundle, effective-shock report, separate mutation and price-effect counts, and explicit incomplete-result status are alternatives.

## Follow-up tracking

Implementation reconciliation and production qualification are tracked
in the [scope follow-ups](../FOLLOW_UPS.md#scenarios).
