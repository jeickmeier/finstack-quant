# MARGIN-COLLATERAL-IM-018 — Signed variation margin, MTA and rounding

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope below, including the stated limitations and discrepancies.

**Validation status:** Evidence inspected or specifically located; no tests/builds executed. Audit date **2026-11-05**; source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Choice and rationale

Observed: VM uses one signed exposure and one signed collateral account, with the same threshold on either exposure side. Proposed rationale: one implementation supplies both transfer calculation and replay diagnostics to avoid divergent financial signs.

## Input domain and conventions

Exposure>0 means counterparty owes desk. Collateral>0 is held; collateral<0 is posted, including pending agreed calls. All amounts are in CSA base currency; threshold, MTA, rounding and independent amount must be finite/nonnegative. This model has one additive nonnegative IA, not separate independent amounts by posting party.

## Ordered algorithm

1. Compute threshold-adjusted support `R=sign(E)*max(abs(E)-T,0)+IA`, returning positive zero for the zero excess before adding IA. Nonfinite exposure and invalid elections error.
2. Unrounded signed call U=R-C. Check collateral currency. If abs(U)<MTA return zero; equality proceeds.
3. If rounding increment q>0 and U≠0, round abs(U)/q upward when U×R>0 (delivery toward required support), otherwise downward (return); restore U's sign. q=0 disables rounding. MTA is not rechecked after rounding.
4. VmCalculator splits collect=max(call,0), post=max(-call,0), while reporting R as net_exposure and storing raw U, prior C, threshold, IA, MTA and q.
5. `net_margin` is desk cash outflow: positive post, negative collect, positive zero for no transfer. Calls use positive transfer magnitudes and call types for direction.

## Selection, controls and limitations

`VmParameters::with_threshold` chooses daily/T+1/round 10,000/IA 0; registry defaults are separate. VmCalculator validates the CSA and both currencies on every call; no FX, haircut valuation, interest accrual, model IM or portfolio netting is performed by this stage.

## Alternatives and tradeoffs

Asymmetric thresholds/IA, net settlement of separate accounts or threshold based on exposure direction would be different contracts. Rounding before MTA changes transfer triggering; the implementation deliberately evaluates the unrounded magnitude first.

## Source and validation evidence

Inspected assertions include raw 147k below 150k MTA giving zero despite possible rounding; delivery 151001 rounded to 160000, return 151001 rounded to 150000; and negative exposure with overposted collateral returning 155k rounded down to 150k. VM result replay and cashflow-direction regressions are invariant/formula evidence, not independent CSA legal review.

Implementation and data: [src/types/thresholds.rs](../../../../finstack-quant/margin/src/types/thresholds.rs), [src/calculators/vm.rs](../../../../finstack-quant/margin/src/calculators/vm.rs), [src/types/call.rs](../../../../finstack-quant/margin/src/types/call.rs).

Existing evidence: [src/types/thresholds.rs](../../../../finstack-quant/margin/src/types/thresholds.rs), [src/calculators/vm.rs](../../../../finstack-quant/margin/src/calculators/vm.rs), [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Adoption, implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
