# MARGIN-COLLATERAL-IM-017 — CSA selection and separate IM collateral terms

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope below, including the stated limitations and discrepancies.

**Validation status:** Evidence inspected or specifically located; no tests/builds executed. Audit date **2026-11-05**; source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Choice and rationale

Observed: model IM and contractual collateral transfer are separate stages. CSA IM terms specify the allocated threshold and custody requirement; they do not select or execute a calculator automatically. Proposed rationale: apply a group allocation once at the intended CSA level and avoid offsetting segregated IM against VM.

## Input domain and conventions

Gross IM is nonnegative Money already calculated at the desired positive MPOR; current collateral is a nonnegative one-way IM account in the same currency. Threshold and MTA must be finite, nonnegative and in that currency. Gross model IM is supplied, not reconstructed from a pricing model here.

## Ordered algorithm

1. Validate CSA VM/IM monetary elections, positive IM MPOR, nonempty ID/curve ID and registered calendar. Validation does not validate the entire eligible-collateral schedule or implement call timing.
2. If no IM terms exist, `apply_im_terms` errors; otherwise target=`max(gross_IM-threshold,0)` and raw_transfer=target-current.
3. Return zero transfer iff abs(raw_transfer)<MTA; equality transfers. There is no IM rounding and no extra MPOR scaling. Positive transfer adds collateral to this one-way account; negative returns excess. Segregation is returned as a custody flag without discounting gross IM.
4. Bilateral SIMM/schedule constructors change only an existing IM methodology; they preserve no-IM agreements. Cleared constructors create a clearing-house CSA with registry defaults, currency-derived `<CCY>-OIS`, zero-lag/one-unit VM rounding and CCP timing.
5. OTC validation requires cleared status iff methodology is ClearingHouse when IM exists. Credit-specific validation additionally requires explicit SIMM CQ-sector or CNQ classification. InternalModel remains an enum and reuses SIMM defaults; it has no calculator in this slice.
6. NettingSetId distinguishes `(counterparty,csa)` bilateral keys from CCP-only cleared keys; this is identity, not an enforceability determination or numerical netting operation.

## Selection, controls and limitations

Default regulatory constructors use SIMM, same numeric50m threshold in any currency and supplied collateral curve string. `with_vm_threshold` preserves frequency/lag but omitted rounding/IA reset to 10,000/0, not previous custom values; `with_im` uses embedded methodology defaults for omitted segregation. Setters check currencies/MPOR but full validation occurs later. Portfolio dispatch/netting and CSA threshold allocation are covered by the portfolio owner.

## Alternatives and tradeoffs

Bilateral per-party thresholds/independent amounts or combined IM+VM MTA elections would need a richer contractual model. Automatic group allocation would require group-wide exposure/state. The current method intentionally relies on caller allocation and one-way accounts.

## Source and validation evidence

Inspected CSA/threshold/OTC tests cover defaults, currency mismatch, no-IM preservation and segregation defaults. Portfolio's existing production threshold test was located as cross-owner evidence, not audited or executed here.

Implementation and data: [src/types/csa.rs](../../../../finstack-quant/margin/src/types/csa.rs), [src/types/otc.rs](../../../../finstack-quant/margin/src/types/otc.rs), [src/types/thresholds.rs](../../../../finstack-quant/margin/src/types/thresholds.rs), [src/types/netting.rs](../../../../finstack-quant/margin/src/types/netting.rs), [src/types/enums.rs](../../../../finstack-quant/margin/src/types/enums.rs), [src/types/mod.rs](../../../../finstack-quant/margin/src/types/mod.rs), [src/traits.rs](../../../../finstack-quant/margin/src/traits.rs), [data/margin/defaults.v1.json](../../../../finstack-quant/margin/data/margin/defaults.v1.json).

Existing evidence: [src/types/csa.rs](../../../../finstack-quant/margin/src/types/csa.rs), [src/types/thresholds.rs](../../../../finstack-quant/margin/src/types/thresholds.rs), [src/types/otc.rs](../../../../finstack-quant/margin/src/types/otc.rs), [tests/simplification_contracts.rs](../../../../finstack-quant/margin/tests/simplification_contracts.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Adoption, implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
