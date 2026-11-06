# VALUATIONS-BONDS-05: MOIC/XIRR return floors as issuer exercise-price constraints

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed in the linked bond production scope; discrepancies are identified explicitly.
**Validation status:** Evidence inspected or located, not executed; no accuracy certification.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and domain

A return floor creates or raises issuer-call redemption prices over an interior-life protection window. It is not a guaranteed minimum held-to-maturity return. Default issue cost is par; callers can supply a positive finite currency amount matching notional or percentage of initial par. MOIC must be positive; XIRR must exceed −1. Full protection means issue+1 day through maturity−1 day; From/Between windows are intersected with that life. Day count defaults ACT/365F.

## Algorithm and ordering

Using issue date `t0` and issue investment `V0`, collect positive Fixed/FloatReset/Stub coupons and Amortization distributions after issue, including cash on the exercise day. Ignore PIK as a distribution; include it in outstanding. For candidate `t`, required dirty redemption is `m·V0−Σ C_i` for MOIC, or `(V0−Σ C_i/(1+r)^τ_i)(1+r)^τ_t` for XIRR. Subtract accrued once, divide by after-event outstanding and floor the clean strike at 100% of outstanding. No extra right is created when outstanding is zero.

Deterministic lowering uses the tree candidate rule (endpoints, cash events, month ends), plus contractual call endpoints. At every contractual call date take the greater of its clean strike and the floor, preserving make-whole terms. Add floor-only call dates elsewhere in the protected window and preserve holder puts. `effective_for_pricing` replaces `return_floor` with this schedule so deterministic trees and workout calculations consume the same lowered rights.

Stochastic rates-credit keeps the floor as path state: it accumulates historical and simulated positive distributions, target-discounted distributions, PIK/outstanding and accrued cash/PIK. Required clean redemption is `max(required_dirty−accrued,outstanding,0)`; contractual calls and make-whole values are raised to that floor, then accrued is added. This prevents using future realized coupons to decide the floor today.

## Limitations, alternatives and evidence

Negative coupon cash, fees and prepayments are not counted among protected distributions by these filters. The floor imposes issuer exercisability where no contractual call previously exists. Historical distributions are reconstructed from the schedule/market rather than supplied as investor cash history. These are material adoption choices, especially for seasoned PIK/FRNs and unusual cashflow kinds. A maturity guarantee, investor-specific acquisition return or constraint only on existing calls is a different, unimplemented contract.

Owning code: [types/validation](../../../../finstack-quant/valuations/fixed-income/src/bond/types/return_floor.rs), [lowering](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/return_floor.rs), [stochastic state formula](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/state.rs), [pathwise exercise](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/replay.rs). Inline return-floor tests inspect amortizing balances, date windows, accrued and merged rights; LSMC test `return_floor_subtracts_partial_accrual_before_adding_it_once` provides an explicit arithmetic invariant. They are not legal-contract validation. [Fixed income reference](../../../REFERENCES.md#fabozzi-fixed-income-handbook) is general background; no product-specific source accepting this floor contract was located.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
