# MARGIN-REGULATORY-XVA-001: FRTB sensitivity units, factor identity, wire reconstruction and provenance

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed** in the stated source scope.
- Validation status: **Evidence inspected/located, not executed.** Selected test bodies were read; no runtime test, build or numerical experiment was run.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

The FRTB engine consumes already-classified, already-converted sensitivities in one reporting currency. It does not extract instrument risk, choose regulatory buckets, compute curvature shocks, perform FX conversion, or establish portfolio completeness. `base_currency` labels the input container; the charge arithmetic does not use it to convert values. The recorded D457/MAR21 provenance is source metadata, not a new verification of a rulebook or jurisdiction.

## Algorithm and conventions

1. GIRR and CSR delta inputs are monetary P&L per one percentage-point rate/spread shift (100 times per-bp DV01/CS01); equity/commodity/FX deltas are P&L per one percent price move. Published-number delta weights multiply these directly. Equity repo uses P&L per percentage-point repo-rate shift. Vega inputs are already `sigma × dV/dsigma`, with decimal vega weights. Curvature is the signed loss-positive, delta-hedged `CVR_up/CVR_down` under the prescribed full shock; no risk weight is subsequently applied to CVR.
2. Full factor keys net matching exposures: GIRR currency/tenor; GIRR vega currency/expiry/underlying-tenor; CSR delta name/bucket/tenor/basis; commodity delta name/bucket/tenor/location; equity separate spot/repo maps; FX ordered currency pairs, with expiry for vega. `add_*` methods sum scalars and each curvature side; DRC/RRAO append positions.
3. Engine validation requires finite numeric inputs, recognized tenor strings, allowed bucket numbers, nonblank name/basis/location/position identifiers, valid DRC positions, and finite RRAO notionals. Allowed tenor strings are exactly `0.25Y, 0.5Y, 1Y, 2Y, 3Y, 5Y, 10Y, 15Y, 20Y, 30Y`; the same lookup validates all delta and vega tenors.
4. JSON uses arrays of factor tuples, sorts map entries lexicographically when serializing, and rejects unknown top-level fields. Reconstruction uses `insert`/`extend`: **the last duplicate tuple key wins**, unlike `add_*` accumulation. DRC/RRAO vector order and repeated identifiers are retained. Deserialization itself does not call financial validation; `calculate` does.
5. Risk weights/correlations come from fixed Rust tables/functions; they are not the margin registry JSON and have no runtime version selector. Source comments record D457/MAR21 and a 2026-08-20 review; equity/commodity comments cite a 2024 publication path. The audit preserves those provenance assertions as assertions. The output stamps default `ResultsMeta`, not the effective regulatory version, jurisdiction, valuation date or sensitivity-source history.

## Selection and controls

`FrtbSensitivities::new` starts all maps/vectors empty. Missing factors therefore contribute zero. A sensitivity container is mutable; mutators defer validation to the engine. Unknown public weight lookups return NaN, while engine validation is intended to reject unsupported buckets before private charge stages. A valid JSON shape or schema match is not financial validity.

## Limitations and failure behavior

Ordered FX pairs are not inverted/canonicalized, same-currency FX pairs are not rejected, and one issuer/name can be assigned multiple risk buckets without a global consistency check. The ten-tenor GIRR lookup is also the accepted vega grid; no remapping to another regulatory grid occurs. No currency/scale completeness check can infer whether a caller supplied raw vega or DV01 incorrectly. The deterministic JSON test proves serialization order, not floating-point charge invariance under arbitrary insertion order.

## Alternatives and tradeoffs

Rejecting duplicate wire rows would surface ambiguous feeds; accumulating duplicates would match mutators but changes current reconstruction. A richer typed factor feed can encode curve, currency and sensitivity units at greater API cost. Automatically deriving buckets/risk from instruments is a distinct capability and is absent here.

## Evidence and role

Runtime and domain contracts: [src/regulatory/frtb/types.rs](../../../../finstack-quant/margin/src/regulatory/frtb/types.rs), [src/regulatory/frtb/wire.rs](../../../../finstack-quant/margin/src/regulatory/frtb/wire.rs), [src/regulatory/frtb/mod.rs](../../../../finstack-quant/margin/src/regulatory/frtb/mod.rs). Fixed parameters: [src/regulatory/frtb/params/mod.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/mod.rs); historical provenance narrative: [data/margin/README.md](../../../../finstack-quant/margin/data/margin/README.md).

[tests/regulatory_determinism.rs](../../../../finstack-quant/margin/tests/regulatory_determinism.rs) contains inspected sorted-JSON/roundtrip and wrong-shape assertions; these are transport evidence, not independent capital calculations. [src/regulatory/frtb/engine.rs](../../../../finstack-quant/margin/src/regulatory/frtb/engine.rs) locates unknown-label/bucket/nonfinite rejection tests. [FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements) supplies the existing canonical pointer; the external standard was not re-certified in this audit.

The implementation establishes observed behavior, not financial adoption or regulatory compliance. Actionable work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
