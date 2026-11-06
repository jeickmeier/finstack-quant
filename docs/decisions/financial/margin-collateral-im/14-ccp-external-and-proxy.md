# MARGIN-COLLATERAL-IM-014 — External CCP IM and conservative exposure proxy

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope below, including the stated limitations and discrepancies.

**Validation status:** Evidence inspected or specifically located; no tests/builds executed. Audit date **2026-11-05**; source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Choice and rationale

Observed: this calculator wraps a supplied CCP IM amount or computes a labelled approximation. It does not implement SPAN, historical simulation or a CCP VaR model. Proposed rationale: an explicit external amount can be used when available while making a fallback estimate's provenance visible.

## Input domain and conventions

External source returns `Option<Money>` and optional MPOR; it has no error-return channel. Fallback requires an explicit `im_exposure_base`, not signed current MTM. CCP names select a methodology by trimmed lowercase substring matching.

## Ordered algorithm

1. Map names in order: LCH (credit/CDS→CDSClear, otherwise SwapClear), ICE (credit/CDS→ClearCredit, otherwise ClearUS), CME, JSCC, Eurex; unmatched names→GenericProxy.
2. Resolve direct parameter override or embedded named params, falling back to the flagged registry default if the key is absent.
3. When an external source exists, apply its MPOR override before asking for its amount. `Some(amount)` wins; `None` falls back. Thus source MPOR can still label a proxy result when no external amount is supplied.
4. Proxy IM=`abs(exposure_base)*conservative_rate`. Embedded rates: LCH SwapClear 2%, LCH CDSClear 8%, CME 3%, ICE Credit 10%, ICE US 5%, JSCC 3%, Eurex 3%, generic 5%; all MPOR 5.
5. Return a single methodology-label breakdown; set approximation=true only for the proxy and log its status. External data returns approximation=false, which means supplied rather than independently certified.

## Selection, controls and limitations

No time scaling is performed here. External nonnegative amount, expected currency and positive MPOR are not checked by this calculator. Direct `CcpParams` overrides also bypass registry scalar validation. `calculate_conservative` returns only Money and cannot itself carry approximation metadata. The local data README explicitly describes these rates as heuristics with no published CCP source.

The name `ICE` selects ClearUS, while registry alias `ice` identifies ClearCredit. The heuristic rates have no established conservative bound or CCP-reference accuracy.

## Alternatives and tradeoffs

A connected, dated CCP service or validated full methodology would remove reliance on the heuristic. Failing closed without external IM is stricter than the current explicit-exposure fallback. Name enums or exact IDs would avoid substring ambiguity but change caller input policy.

## Source and validation evidence

Inspected tests distinguish external amount/proxy flags, named rates, explicit exposure requirements and absence of MTM substitution. They do not establish any rate as conservative for a portfolio or validate the external source's quality.

Implementation and data: [src/calculators/im/clearing.rs](../../../../finstack-quant/margin/src/calculators/im/clearing.rs), [src/calculators/im/mod.rs](../../../../finstack-quant/margin/src/calculators/im/mod.rs), [src/traits.rs](../../../../finstack-quant/margin/src/traits.rs), [data/margin/ccp_methodologies.v1.json](../../../../finstack-quant/margin/data/margin/ccp_methodologies.v1.json), [data/margin/README.md](../../../../finstack-quant/margin/data/margin/README.md).

Existing evidence: [src/calculators/im/clearing.rs](../../../../finstack-quant/margin/src/calculators/im/clearing.rs), [tests/simplification_contracts.rs](../../../../finstack-quant/margin/tests/simplification_contracts.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Adoption, implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
