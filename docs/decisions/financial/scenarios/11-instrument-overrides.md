# SCENARIOS-011: Instrument price/spread override routing and composite traversal

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed for the stated source scope.** Actual discrepancies and unsupported cases are distinguished below.
- Validation status: **Evidence inspected/located, not executed.** No numerical test, build or external benchmark run is claimed.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Choice and rationale

Observed choice: attach scenario shocks to instrument pricing overrides rather than mutating contractual cashflows or repricing instruments immediately. Type filters match `InstrumentType`; attribute filters require every supplied key/value pair to match metadata, case-insensitively. The engine requires an inventory before any mutation, but an empty inventory is permitted.

## Ordered algorithm and units

Convert input price percentage to decimal `d=pct/100`; spread stays in basis points. Existing optional price shock defaults to zero and accumulates as `(1+old)*(1+d)-1`; spread shocks add `old+bp`. The direct price route requires mutable pricing overrides. The spread route additionally requires `scenario_spread_shock_supported()`; mere possession of an override object is insufficient.

Traverse each top-level instrument in order. If it matches, update that root and stop descending this branch. If it does not match and is a `CompositeInstrument`, clone the composite, materialize each embedded leg from its canonical representation, recursively apply matching child shocks, serialize changed children back into the cloned leg specification, and replace the root if any serialized child changed. This retains resolved leg quantities and other composite state; it does not rebalance positions to offset a stressed mark. The outcome counts changed top-level inventory entries, not the number of matched nested leaves.

For unsupported routing, write `scenario_price_shock_decimal` or `scenario_spread_shock_bp` into instrument metadata and emit `InstrumentShockFallback`. Existing metadata is parsed as f64 or treated as zero when unparseable. Metadata storage alone has **no guaranteed valuation effect**; downstream consumers must explicitly read it. A metadata-only match still counts as changed/applied. Attribute filters with zero matches warn; type filters with zero matches do not.

## Failure, report limits and alternatives

Composite child materialization errors and missing canonical reserialization support are skipped without an error/warning at those sites. A matching root avoids descendant inspection entirely. Recursive mutations operate on a cloned composite, but top-level instruments already processed remain changed if some unrelated later engine operation fails. Mathematical compounding preserves the requested factors for ordinary finite values; accumulated overflow or malformed preexisting overrides are not locally guarded by this adapter.

Alternatives include requiring support and rejecting unsupported matches, applying a pure post-valuation price multiplier, or rebuilding contractual spread fields. They differ in pricing/Greek semantics and should not be silently interchangeable. Attribute filtering uses metadata only; tags or economic dependency discovery do not implicitly participate.

## Evidence

Owning source: [adapters/instruments.rs](../../../../finstack-quant/scenarios/src/adapters/instruments.rs), [instrument dispatch](../../../../finstack-quant/scenarios/src/engine/instrument_shocks.rs). Inspected unit tests assert two -60% price losses retain 16%, two +25bp spread shocks produce +50bp, descendant shocks preserve quantities, and a matching composite root leaves children unshocked. [Warmed-composite regressions](../../../../finstack-quant/scenarios/tests/senior_review_market_regressions.rs) locate nested-cache refresh and retained-state checks. These are routing/composition invariants; actual support and spread repricing remain instrument-family responsibilities.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios). This record does not
authorize a runtime change or certify the observed method.
