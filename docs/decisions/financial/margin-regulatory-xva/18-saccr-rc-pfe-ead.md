# MARGIN-REGULATORY-XVA-018: SA-CCR collateral treatment, PFE multiplier and capped margined EAD

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed** in the stated source scope.
- Validation status: **Evidence inspected/located, not executed.** Selected test bodies were read; no runtime test, build or numerical experiment was run.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

The engine combines replacement cost and supervisory add-on PFE, then caps a margined calculation at a recalculated unmargined EAD. Collateral is positive when held by the bank. Runtime treats `collateral_value` as variation margin and adds NICA separately. Field scope is distinct from the accepted collateral sign convention.

## Algorithm and conventions

1. `V=Σ signed_MTM`, `C=collateral_value+NICA`. Unmargined replacement cost is `max(V−C,0)`; margined RC is `max(V−C, threshold+MTA−NICA, 0)`.
2. Let A be the sum of class add-ons (records 16–17). If `A<=0`, multiplier is `0.05`; otherwise `mult=min(1, 0.05+0.95×exp((V−C)/(1.9A)))`. PFE is `mult×A`.
3. Initial EAD is `alpha×(RC+PFE)`, default alpha 1.4. For a margined set clone the entire config, change only `is_margined=false`, rerun validation/RC/PFE with unmargined trade MFs, and take `min(initial_EAD,unmargined_EAD)`.
4. Return capped EAD but original margined RC, PFE, multiplier and add-on details. Return a separate summary MF from record 15 and default results metadata. The cap amount and binding state are not explicit output fields.

## Selection and controls

The same VM/NICA/threshold/MTA values remain in the recursive cap config; unmargined RC ignores threshold/MTA but still subtracts VM+NICA. A high-threshold empty margined book can consequently have positive reported RC and zero capped EAD. The public `saccr_ead` convenience wrapper applies an optional validated alpha and delegates to this engine.

## Limitations and failure behavior

When the cap binds, `ead` need not equal reported `alpha×(rc+pfe)`. The displayed components therefore cannot generally reconstruct capped EAD. The source [financial perspectives ADR](../conventions/0044-financial-perspectives-and-boundary-conversions.md) already records held-by-bank positive collateral and the margined RC equation; the VM-only input scope and unmargined NICA extension described here are observed behavior, not newly adopted policy. No final finiteness check prevents overflow of otherwise finite totals/products. Default ResultsMeta does not identify regulatory applicability or alpha/MPOR override provenance.

## Alternatives and tradeoffs

Reporting pre-cap EAD, cap EAD and a binding-cap flag would preserve the actual decomposition; replacing component fields by invented cap-adjusted allocations requires a new allocation policy. Separating VM from NICA at the type boundary reduces double counting. Changing the existing held-by-bank sign would contradict the stated contract.

## Evidence and role

Runtime: [src/regulatory/sa_ccr/replacement_cost.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/replacement_cost.rs), [src/regulatory/sa_ccr/pfe.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/pfe.rs), [src/regulatory/sa_ccr/engine.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/engine.rs); returned contract: [src/regulatory/sa_ccr/types.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/types.rs). Inspected [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs) includes empty high-threshold cap behavior; located engine tests cover multiplier/negative MTM/alpha cases. The cap example confirms branch behavior, not complete policy adoption. [SA-CCR reference index](../../../REFERENCES.md#bcbs-279-saccr).

The implementation establishes observed behavior, not financial adoption or regulatory compliance. Actionable work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
