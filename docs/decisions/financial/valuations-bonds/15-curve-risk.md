# VALUATIONS-BONDS-15: Quote-reproducing curve DV01, key-rate risk and effective convexity

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed in the linked bond production scope; discrepancies are identified explicitly.
**Validation status:** Evidence inspected or located, not executed; no accuracy certification.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and selection

A fixed price quote must be replaced by a model spread before curve-bump repricing can move its value. Plain noncredit quoted bonds are pinned with their solved periodic Z-spread; exercise-bearing or active hazard-model bonds are pinned with OAS on the original market. The selected pricing model/registry is preserved, price-driving overrides cleared, and the context restored after the risk calculation. Existing computed OAS is reused, otherwise solved from the quote. This is a calibrated risk representation, not a claim that the quote identifies a unique structural credit model.

## Algorithms and controls

Parallel `Dv01` has a material selection rule: on default Workout basis, **any price-driving quote or exercise right** selects the signed yield-DV01 formula of record 14. Only plain unquoted bonds use generic combined discount/projection curve bumps. Thus the label `Dv01` does not always mean a curve sensitivity. Under explicit CallableOas, unquoted bonds use generic combined curve DV01; quoted plain bonds use the spread-pinned view and generic curve DV01; quoted optional bonds bump only `tree_discount_curve_id`, falling back to the bond discount curve. Bumps are configured bp and use central currency P&L per one bp; effectively zero bump returns zero.

Bucketed DV01 always uses the quote-reproducing view and delegates triangular key-rate bumps to the framework, independently of Workout versus CallableOas. It can therefore have a different risk basis from default parallel `Dv01`; summing buckets need not recover the quoted-yield number.

Effective duration/convexity for optional CallableOas risk fixes OAS, reprices base and selected-tree-curve shifts of ±25 bp, and computes `D=(P_down−P_up)/(2P0 h)` and `C=(P_up+P_down−2P0)/(P0 h²)`. Near-zero base PV (`1e−10`) returns both zero. Reported bond convexity divides this `C` by 100. Exercise/default are recalculated under each market. This finite shock is distinct from the sensitivity framework's smaller configurable DV01 bump.

## Limits, alternatives and evidence

Shared bump interpolation/recalibration is owned by the framework/core-market records. The bond layer does not hold exercise fixed during model risk or recalibrate OAS on every bump. Generic combined risk and the optional quoted single-tree-curve route can bump different dependency sets. At exercise boundaries these finite differences depend on shock size; stochastic regression/pricing error can enter a central difference. Direct market-quote revaluation and frozen-cashflow risk are meaningful alternatives but answer different questions.

Owning code: [pinning/routing](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/risk_view.rs), [parallel/bucketed DV01](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/dv01.rs), [effective risk](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/effective.rs), [generic curve sensitivity](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/dv01.rs). [Bucket tests](../../../../finstack-quant/valuations/fixed-income/tests/bond/metrics/bucketed_dv01.rs) check that quoted price does not force bucket risk to zero; [duration tests](../../../../finstack-quant/valuations/fixed-income/tests/bond/metrics/duration.rs) inspect risk-basis choice. These establish routing/identities, not bump convergence over the supported instrument domain. Reference: [Tuckman–Serrat](../../../REFERENCES.md#tuckman-serrat-fixed-income).

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
