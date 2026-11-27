# MARGIN-REGULATORY-XVA-018: SA-CCR collateral treatment, PFE multiplier and capped margined EAD

- Decision status: **Accepted 2026-11-17 for checked collateral recognition, replacement cost, PFE, and the margined EAD cap.**
- Implementation status: Observed behavior is retained below. Separate collateral recognition, checked assembly, and auditable cap reporting require reconciliation.
- Validation status: Primary Basel provisions and the runtime paths were inspected. Independent arithmetic checked a binding cap. Mixed agreements, collateral treatment, and cross-host behavior still require qualification.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Accepted decision

1. Qualify recognized collateral separately for each basis. Use signed, non-overlapping VM and NICA, with collateral held by the bank positive. Use margined RC = max(V − C, TH + MTA − NICA, 0) and unmargined RC = max(V − C, 0), using eligible haircut-adjusted collateral on the applicable basis. For a bank-only posting agreement, retain recognized posted VM with a negative sign on the unmargined basis. Apply the dedicated rules for mixed margin agreements and collateral shared across netting sets. [ADR 0044](../conventions/0044-financial-perspectives-and-boundary-conversions.md#accepted-amendment-explicit-collateral-components) · [Basel CRE52.10–19 and 52.74–77](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/52/inforce/2023-01-01/published/2020-06-05).
2. Use checked arithmetic for the Basel multiplier and EAD. For a positive aggregate add-on, apply the Basel multiplier with its 5% floor. A zero add-on gives zero PFE. Ordinary Basel SA-CCR uses alpha 1.4. Cap margined EAD at the EAD of the same trade set recalculated using qualified unmargined collateral and maturity factors. [Trade and rulebook admission](14-saccr-inputs-and-supervisory-delta.md) · [Basel CRE52.1–2 and 52.20–23](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/52/inforce/2023-01-01/published/2020-06-05).
3. Report margined RC and PFE as pre-cap components. Report pre-cap EAD, the unmargined cap, its binding state, and final EAD. Retain the effective rulebook and collateral provenance needed to replay both calculations.

For margined RC of 75 and PFE of 50, pre-cap EAD is 175. Unmargined RC of 90 and PFE of 20 produce a binding cap and final EAD of 154.

## Observed choice and domain

The engine combines replacement cost and supervisory add-on PFE, then caps a margined calculation at a recalculated unmargined EAD. Collateral is positive when held by the bank. Runtime treats `config.collateral` as variation margin and adds NICA separately. Field scope is distinct from the accepted collateral sign convention.

The shared public fields and derived collateral are governed by the accepted [ADR 0044 collateral-input package](../conventions/0044-financial-perspectives-and-boundary-conversions.md#accepted-amendment-explicit-collateral-components).

## Algorithm and conventions

1. `V=Σ signed_MTM`, `C=collateral+NICA`. Unmargined replacement cost is `max(V−C,0)`; margined RC is `max(V−C, threshold+MTA−NICA, 0)`.
2. Let A be the sum of class add-ons (records 16–17). If `A<=0`, multiplier is `0.05`; otherwise `mult=min(1, 0.05+0.95×exp((V−C)/(1.9A)))`. PFE is `mult×A`.
3. Initial EAD is `alpha×(RC+PFE)`, default alpha 1.4. For a margined set clone the entire config, change only `is_margined=false`, rerun validation/RC/PFE with unmargined trade MFs, and take `min(initial_EAD,unmargined_EAD)`.
4. Return capped EAD but original margined RC, PFE, multiplier and add-on details. Return a separate summary MF from record 15 and default results metadata. The cap amount and binding state are not explicit output fields.

## Selection and controls

The same VM/NICA/threshold/MTA values remain in the recursive cap config; unmargined RC ignores threshold/MTA but still subtracts VM+NICA. A high-threshold empty margined book can consequently have positive reported RC and zero capped EAD.

## Limitations and failure behavior

When the cap binds, `ead` need not equal reported `alpha×(rc+pfe)`. The displayed components therefore cannot generally reconstruct capped EAD. The source [financial perspectives ADR](../conventions/0044-financial-perspectives-and-boundary-conversions.md) already records held-by-bank positive collateral and the margined RC equation; the public `variation_margin`/`nica` representation is now accepted, while the observed old name/defaults require reconciliation and the accepted unmargined treatment requires reconciliation. No final finiteness check prevents overflow of otherwise finite totals/products. Default ResultsMeta does not identify regulatory applicability or alpha/MPOR override provenance.

## Alternatives and tradeoffs

Reporting pre-cap EAD, cap EAD and a binding-cap flag would preserve the actual decomposition; replacing component fields by invented cap-adjusted allocations requires a new allocation policy. Separating VM from NICA at the type boundary reduces double counting. Changing the existing held-by-bank sign would contradict the stated contract.

## Evidence and role

Runtime: [src/regulatory/sa_ccr/replacement_cost.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/replacement_cost.rs), [src/regulatory/sa_ccr/pfe.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/pfe.rs), [src/regulatory/sa_ccr/engine.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/engine.rs); returned contract: [src/regulatory/sa_ccr/types.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/types.rs). Inspected [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs) includes empty high-threshold cap behavior; located engine tests cover multiplier/negative MTM/alpha cases. The cap example confirms branch behavior, not complete policy adoption. [SA-CCR reference index](../../../REFERENCES.md#bcbs-279-saccr).

Implementation reconciliation and independent qualification are tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
