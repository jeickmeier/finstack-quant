# MARGIN-COLLATERAL-IM-022 — Sensitivity table conversion and margin wire boundaries

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope below, including the stated limitations and discrepancies.

**Validation status:** Evidence inspected or specifically located; no tests/builds executed. Audit date **2026-11-05**; source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Choice and rationale

Observed: adapters preserve financial factor identity and output interpretation; they are not general validation or regulatory-compliance engines. Proposed rationale: stable wire rows and replay diagnostics allow clients to inspect the same calculations without duplicating algorithms.

## Input domain and conventions

SensitivityRow carries risk_class, kind, issuer, bucket, tenor, optional expiry and DRC metadata plus an amount. The table envelope does not include the sensitivity container's base currency; callers must retain it and provide it again to from_rows. Amount units depend on risk class/kind.

## Ordered algorithm

1. Export all populated SIMM/FRTB maps to rows. Sort by class, kind, issuer, bucket, tenor, expiry, sector, seniority, optional numeric metadata and amount total order. Zero rows are retained. FRTB up/down curvature becomes two rows; SIMM curvature retains a separate expiry column and pre-SF volatility-weighted vega.
2. IR currency lives in issuer; CQ sector or FRTB bucket in bucket; FX currency/pair in issuer. FRTB GIRR vega encodes option/underlying tenor as A/B; credit/commodity delta encodes tenor/basis as A/B. DRC exports rating bucket, sector, seniority, maturity and P&L adjustment; RRAO distinguishes exotic/other notional.
3. Import matches exact class/kind strings, requires used dimensions, parses currencies/buckets/pairs and calls the financial container's adders. Duplicate map keys sum; curvature halves add; DRC positions remain separate. Missing DRC P&L defaults to 0, while maturity/sector/seniority are required.
4. SIMM curvature rows validate immediately, but ordinary SIMM and all FRTB rows do not get a final container validate call in from_rows. Unused populated dimensions are ignored. Later model engines enforce their own input domains.
5. Tables use stable dimension/measure columns, nullable absent dimensions and an empty metadata map; even empty exports retain schema. SIMM adds expiry, FRTB adds DRC fields.
6. MarginEnvelope requires the exact version tag and one OTC/CSA/call shape via Serde; from_slice performs shape parsing only, not CsaSpec/OtcMarginSpec::validate. Feature-gated schema generation registers input/output/component types, examples and externalized common definitions using the shared core generator.

## Selection, controls and limitations

ImResult constructors default approximation=false; calculators set appropriate provenance explicitly. SIMM detail retains unscaled10-day numbers with separate horizon scale. Many monetary output structs deserialize public derived fields without recalculation; utilization is a notable validated-wire exception. Schema examples are illustrations, sometimes direct constructed outputs, not model-validation evidence.

## Alternatives and tradeoffs

Embedding base currency and input units in table metadata would make detached tables self-describing. Validated import constructors would fail earlier but could prevent intermediate authoring. A structured factor schema avoids delimiter ambiguity; neither changes should be inferred without an agreed public contract.

## Source and validation evidence

Inspected row conversion code and located roundtrip tests for all FRTB adders/DRC and SIMM maps/curvature. Schema parity tests are structural evidence; they do not validate financial claims in prose or show that direct deserialization applies business validation.

Implementation and data: [src/table.rs](../../../../finstack-quant/margin/src/table.rs), [src/schema.rs](../../../../finstack-quant/margin/src/schema.rs), [src/calculators/traits.rs](../../../../finstack-quant/margin/src/calculators/traits.rs), [src/types/simm_types.rs](../../../../finstack-quant/margin/src/types/simm_types.rs), [src/types/serde_validation.rs](../../../../finstack-quant/margin/src/types/serde_validation.rs).

Existing evidence: [src/table.rs](../../../../finstack-quant/margin/src/table.rs), [tests/schema_parity.rs](../../../../finstack-quant/margin/tests/schema_parity.rs), [tests/simplification_contracts.rs](../../../../finstack-quant/margin/tests/simplification_contracts.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Adoption, implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
