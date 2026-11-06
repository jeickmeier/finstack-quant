# MARGIN-COLLATERAL-IM-010 — SIMM expiry scaling and curvature aggregation

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope below, including the stated limitations and discrepancies.

**Validation status:** Evidence inspected or specifically located; no tests/builds executed. Audit date **2026-11-05**; source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Choice and rationale

Observed: curvature consumes separate signed volatility-weighted vega rows and applies expiry scaling before factor netting. Proposed rationale: opposite vegas at different expiries must not cancel before their different short-horizon scaling factors are applied.

## Input domain and conventions

Input amount is signed USD `sigma*dPV/dsigma` before SF, HVR, vega RW or concentration. IR bucket is currency, factor is subcurve and risk tenor is required. CQ requires sector and issuer; CNQ/equity require residual bucket; commodity uses a recognized bucket/name; FX requires bucket `fx` and distinct currency pair. Credit risk tenors are restricted to 1Y/2Y/3Y/5Y/10Y. Expiry must be one of the 12 SIMM tenors.

## Ordered algorithm

1. Validate and canonicalize currencies, credit/commodity buckets and sorted FX pair. Convert expiry to fixed calendar days:2W=14, months=365/12 each, years=365 each. Compute `CVR=raw_vega*0.5*min(1,14/days)`.
2. Group by risk class, bucket, factor and risk tenor (expiry is removed only after scaling). Sort contributions by total numeric order and sum; reject nonfinite factor sums.
3. Within bucket compute K with squared factor correlation. IR correlation is tenor correlation times 1 for same subcurve or0.993 otherwise; CQ uses same-name 0.93/different-name 0.46 or residual 0.50; CNQ 0.50; equity 0; commodity its per-bucket correlation; FX 0.50.
4. Store sum, absolute sum and capped signed sum `S=clamp(sum,-K,K)`. For nonresidual buckets combine K² and squared bucket correlations times capped signed sums. Bucket correlations are IR 0.32, CQ table, commodity matrix; otherwise 0.
5. Define `theta=min(sum/abs_sum,0)` and `lambda=(z²-1)*(1+theta)-theta`, z=2.5758293035489004. Charge=`max(sum+lambda*norm,0)`; abs_sum 0 returns 0. Apply this separately to residual buckets and add their charges to the nonresidual charge.
6. Divide IR curvature by HVR 0.47². Reject nonfinite class margin. Add curvature within each class before cross-class aggregation.

## Selection, controls and limitations

There is no concentration multiplier in this curvature kernel and no multiplication by ordinary vega risk weights. Detailed rows expose already-SF-scaled factor amounts. Factor sums and key order are canonicalized, but the final unguarded floating sums can still require numerical qualification for extreme data.

## Alternatives and tradeoffs

Pricing explicit up/down risk shocks or gamma would be a different model/input contract. Keeping one expiry scalar would be smaller but erase the scale-before-netting distinction. Full product-class separation and more detailed factor grids would widen the supported historical model.

## Source and validation evidence

Inspected hand test uses +1000 at 2W and -1000 at 1Y: expected equity charge `(500-7000/365)*z²`, invariant to input reversal; IR divides that by 0.47². Other inline tests cover class-before-cross-class aggregation and invalid expiry. Source provenance is recorded locally; no licensed comparison was run.

Implementation and data: [src/calculators/im/simm/curvature.rs](../../../../finstack-quant/margin/src/calculators/im/simm/curvature.rs), [src/types/simm_curvature.rs](../../../../finstack-quant/margin/src/types/simm_curvature.rs), [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs), [data/margin/simm.v1.json](../../../../finstack-quant/margin/data/margin/simm.v1.json).

Existing evidence: [tests/production_simm_csa_audit.rs](../../../../finstack-quant/margin/tests/production_simm_csa_audit.rs), [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Adoption, implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
