# MARGIN-COLLATERAL-IM-005 — SIMM qualifying-credit issuer and sector aggregation

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope below, including the stated limitations and discrepancies.

**Validation status:** Evidence inspected or specifically located; no tests/builds executed. Audit date **2026-11-05**; source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Choice and rationale

Observed: CQ credit uses 12 sector buckets plus residual, with name identity retained across tenor entries. Proposed rationale: allow issuer hedges without treating all issuers or residual exposures as interchangeable.

## Input domain and conventions

Signed USD CS01 per bp for delta and compatible raw volatility-weighted vega for vega. Sector assignment is caller supplied, including investment-grade/high-yield distinctions. Missing sector is not inferred by the calculator.

## Ordered algorithm

1. Group and sort `(name,tenor)` entries within each sector. Sum raw amounts per name across tenors.
2. For delta, `CR_name=max(1,sqrt(abs(raw_name)/CT_sector))`: sovereign/high-yield-sovereign CT=1m, other sectors/residual CT=170k. Delta weights by the enum order are 75,90,84,54,62,48,185,343,255,250,214,173,343 bp. For vega, use CQ CT=360m and RW=0.76.
3. Form entry `WS=raw*RW*CR`. Within a nonresidual sector use correlation 0.93 for same issuer, 0.46 for different issuers, multiplied by `min(CR_i,CR_j)/max(CR_i,CR_j)`; residual uses 0.50 regardless of name. Quadratic diagonal is always 1.
4. Compute sector K and cap the signed sum to ±K. Combine nonresidual sectors with the registry's pairwise correlations and floored quadratic norm.
5. Add residual K linearly outside sector diversification. Delta and vega produce independent component margins that add within the credit-qualifying risk class.

## Selection, controls and limitations

Default same-issuer correlation is below one even for separate tenors of a single issuer. Exact identical keys already net during input construction. The public ordinary credit maps admit every general SIMM tenor; the numerical formula has no separate tenor-correlation term. OtcMarginSpec::validate_for_credit requires an explicit qualifying sector or nonqualifying choice when SIMM is selected.

## Alternatives and tradeoffs

A finer credit factor/curve basis representation could separate index and single-name behavior or tenor-specific rules. Treating all CQ as one scalar would lose sector diversification and concentration, while forcing all unknowns to residual would change classification responsibility.

## Source and validation evidence

Inspected hand vector `credit_concentration_uses_raw_name_sensitivity` gives 100k financial CS01 ×90=9m below its raw threshold. Inline SIMM tests exercise CQ issuer, sector, residual, vega and concentration behavior; local parameter provenance gives source locations but was not independently re-fetched.

Implementation and data: [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs), [src/types/otc.rs](../../../../finstack-quant/margin/src/types/otc.rs), [src/types/simm_types.rs](../../../../finstack-quant/margin/src/types/simm_types.rs), [data/margin/simm.v1.json](../../../../finstack-quant/margin/data/margin/simm.v1.json).

Existing evidence: [tests/production_simm_csa_audit.rs](../../../../finstack-quant/margin/tests/production_simm_csa_audit.rs), [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Adoption, implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
