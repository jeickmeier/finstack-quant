# MARGIN-COLLATERAL-IM-006 — SIMM nonqualifying residual credit

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope below, including the stated limitations and discrepancies.

**Validation status:** Evidence inspected or specifically located; no tests/builds executed. Audit date **2026-11-05**; source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Choice and rationale

Observed: the CNQ input lacks a rating or sector bucket, so the implementation applies a single residual model. Proposed rationale: retain name-level concentration and signed hedges without inventing absent classification.

## Input domain and conventions

USD credit delta per bp and raw vega, finite amounts, nonempty issuer/index IDs and supported ordinary tenor labels. Nonqualifying classification itself is explicit and cannot carry a CQ sector in its wire representation.

## Ordered algorithm

1. Sort name/tenor entries and sum raw sensitivity per name.
2. Delta uses CT=500,000 and RW=1,300 bp; vega uses CT=70m and RW=0.76. `CR_name=max(1,sqrt(abs(net_name)/CT))`.
3. Weight each original tenor entry with its name's CR. Aggregate using `rho_ij=0.50*min(CR_i,CR_j)/max(CR_i,CR_j)` off diagonal, including different tenors of the same name; diagonal is 1.
4. Floor the quadratic expression at zero before square root. The resulting delta and vega margins later add with CNQ curvature before cross-class aggregation.

## Selection, controls and limitations

This residual classification is an explicit supported limitation, reflected in the final approximate flag. There is no direct credit-spread curve construction, rating migration, default simulation or collateral valuation here. Exact-key offsets occur before concentration; multi-tenor offsets still enter the correlation calculation.

## Alternatives and tradeoffs

A fully classified CNQ model can represent distinct bucket/rating rules; treating all CNQ as residual avoids unsupported inference but may materially change offset and charge behavior. Independent per-name portfolio scenarios can reveal the cost of that simplification.

## Source and validation evidence

Inline test `credit_non_qualifying_delta_calculation` checks 50k×1,300. Historical literal tests pin residual delta/vega weights and thresholds. This is formula/parameter evidence, not a reference implementation comparison.

Implementation and data: [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs), [src/types/otc.rs](../../../../finstack-quant/margin/src/types/otc.rs), [src/types/simm_types.rs](../../../../finstack-quant/margin/src/types/simm_types.rs), [data/margin/simm.v1.json](../../../../finstack-quant/margin/data/margin/simm.v1.json).

Existing evidence: [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs), [tests/simm_schedule_parity.rs](../../../../finstack-quant/margin/tests/simm_schedule_parity.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Adoption, implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
