# SCENARIOS-011: Instrument price/spread override routing and composite traversal

- Decision status: **Accepted 2026-11-15 — complete exact selectors, checked price and spread overrides, composite routing and atomic effects package.**
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent price composition, representability, spread repricing and composite contribution calculations verified.** Production routing, restoration and host qualification remain pending.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Accepted decision for checked scenario instrument overrides

1. **Use exact selectors.** Admit a nonempty duplicate-free set of exact
   typed instrument kinds and match any member. Alternatively admit a
   nonempty map of metadata predicates and require every pair to match
   the same instrument. Metadata keys and values use exact case-sensitive
   comparison. Keys must be nonblank. An empty value differs from a
   missing value. Reject duplicate raw JSON or host predicate keys before
   materialization as a map. Missing inventory, zero matches and an
   unsupported selected occurrence are distinct errors, including at zero.
   [Accepted scenario specification](01-specification-and-composition.md#accepted-decision-for-checked-scenario-specifications-and-ordered-composition).

2. **Apply the relative price multiplier once to the signed mark.**
   Require finite percent `p ≥ −100`. Compose factors `1+p/100` in
   scenario order and retain the canonical decimal shock. Compute the
   near-wipeout factor stably and check accumulation and representation.
   Reject overflow, loss of required nonzero values and positive factors
   that would encode as a full wipeout. An explicit `−100%` gives zero
   after the selected base valuation succeeds. Apply the accumulated
   factor once through the shared valuation boundary. Standalone quote
   and accrual measures use their own bases.
   [Accepted valuation lifecycle](../valuations-framework/pricing-lifecycle.md#accepted-decision-for-checked-valuation-dispatch-and-lifecycle).

3. **Bind spreads to the qualified settlement Z-spread route.**
   Require the actual selected bond Discounting dispatch and an explicit
   qualified Z center. Add signed bp changes in action order and convert
   the total once to annual decimal before adding it to that center.
   Use the elected cash path, source clock, compounding and settlement
   carry. Reject an absent center, a conflicting price pin or an
   unsupported route. Zero preserves the qualified center. OAS, credit
   and contractual coupon changes have separate operations or owners.
   [Bond model contract](../valuations-bonds/03-pv-models.md) ·
   [Settlement spread law](../valuations-bonds/12-spread-quotes.md).

4. **Stop at the first match on each composite branch.** Traverse
   top-level inventory order followed by authored leg order. A matched
   composite receives one aggregate price multiplier. A matched spread
   target requires an admitted route at that node. Descend through
   unmatched composites and validate the full tree under the sizing
   owner's eight-level and 64-occurrence limits. Preserve frozen
   quantities, signs, sizing inputs and sizing authority. Equal repeated
   IDs within a tree must have equal effective definitions. Reuse the
   admitted replacement across selected occurrences. Fail if a branch
   stop would leave conflicting definitions within a tree.
   [Composite sizing and identity](../valuations-equity-and-composition/23-composite-sizing.md).

5. **Reconcile the adjusted headline and breakdown.** Use the same
   valuation tree, quantities, FX and applicable multipliers for the
   headline and its contributions. Price each selected aggregate with
   its own multiplier once. Measures of the adjusted mark use the same
   adjustment at every repricing endpoint. Contractual cash and frozen
   holdings use their contractual and sizing bases.
   [Composite valuation](../valuations-equity-and-composition/24-composite-valuation-history.md).

6. **Publish checked effects atomically.** Remove metadata fallback
   and silent child decode or re-encode skips. Stage and validate the
   full candidate before publication. Record operation, root and leg
   path, requested units and actual before-and-after override. Count
   represented changes, distinguish valid identities and deduplicate
   affected roots for repricing. Monetary P&L requires an actual
   reprice. Rust owns admission and errors across Python and WASM.
   Record 017 owns the report and replay representation.
   [Accepted execution](02-execution-and-partial-success.md#accepted-decision-for-ordered-and-atomic-scenario-execution) ·
   [Reporting record 017](17-reports-and-replay.md).

## Independent calculation checks

- Two `−60%` price actions leave `0.4²=0.16` of the signed mark.
  A mark of `−100` becomes `−16`. Two price actions of
  `−99.99999999999999%` leave a positive factor whose binary64
  decimal-shock encoding rounds to `−1`. The latter must fail.
- With settlement at valuation, one payment of 105 after one year,
  a continuous 3% curve and a 100 bp Z center, two `+25 bp` actions
  move the center to 150 bp. The dirty value changes from
  `105 exp(−.04) = 100.882891` to `105 exp(−.045) = 100.379736`.
- If a parent's mark is `2 × 10 + 50 = 70`, a `+10%` multiplier
  on the first child composite makes its contribution `22` and
  the parent mark `72`.

## Observed choice and rationale at the audit revision

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

Implementation reconciliation and production qualification are tracked
in the [scope follow-ups](../FOLLOW_UPS.md#scenarios).
