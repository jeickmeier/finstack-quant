# STATEMENTS-CAPITAL-005: Residual coupon rebuilding after dated balance changes

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed for the source scope and discrepancies stated below.
**Validation status:** Evidence inspected or located, not executed; numerical qualification remains open.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; [scope and file ledger](README.md).

## Choice and rationale

After waterfall close, rebuild future coupons from dated principal changes while preserving interest already earned. Split cash/PIK coupons retain their own economic share. The method modifies the raw schedule rather than applying one multiplier to all future coupons, preserving intra-period amortization, draws, PIK compounding and arrears/advance reservations.

## Algorithm and equations

1. Validate nonnegative new closing and schedule currency. Read original economic outstanding at inclusive `from_date` (initial fallback, or zero before issue). Let `δ=new−old`. If `abs(δ)≤1e−9`, return a clone. Otherwise insert a zero-cash PrePayment row at `from_date` with principal delta `δ`; interest feels that change from the following day, which must exist.
2. Reserve positive claims whose economic date is after `from_date`, keyed by `(payment_date,balance_date)`. Reject wrong currency, negative claims or total reserve exceeding new principal by `1e−6`.
3. Sort existing flows by economic date, except cash-interest rows use accrual end when known. On the same date, other principal events precede interest, and terminal Notional redemption follows interest/PIK. Preserve historical rows as anchors.
4. Rebuild Fixed, Stub, FloatReset and PIK. Cash coupons qualify by future payment date; PIK qualifies by future economic date. Use explicit accrual metadata or infer start from the latest earlier same-kind payment date, falling back to issue date; absence errors. Coupons fully earned by the day after the snapshot retain their amounts, even when paid/capitalized later.
5. Use strict calendar lookup and coupon-period/day-count context. For original principal `P` at accrual start, infer signed component rate `r=C/(P·a)` from original cash amount `C` and accrual factor `a`. If `P=0`, use the row's retained rate. Reject nonfinite rates, nonfinite/nonpositive full year fraction or missing anchors.
6. Split accrual at all balance-change dates. For segment fraction `f=DCF(segment)/DCF(full)`, factor `a_s=a·f` and cumulative balance change `Δ`, set `C_s=C·f+Δ·r·a_s`. Preserve payment date, component kind and accrual metadata; retain `r` even when amount becomes zero. Explicit PIK deltas scale with the revised amount (or proportionally by `f` when original amount is zero). This avoids applying an unsplit full coupon rate independently to both cash and PIK shares.
7. Future positive principal payments are capped by remaining balance after outstanding reservations. Future cash Notional redemption takes all available principal; other repayments use their original amount capped to capacity. Already-paid cash is preserved while its future principal delta can be adjusted. Revised future PIK/principal changes update the balance and the change map for later coupons.
8. Reject unmatched reserves remaining above `1e−6`; reconstruct through `CashFlowSchedule::from_flows`, retaining notional/day-count/meta. State rebuild passes both arrears and advance lots and only visits instruments with residual schedules.

## Limitations and alternatives

This is a rate-inference/residual-interest method for supported schedule rows. It is not a fresh contractual model of optional bond calls, prepayment premiums, OID accretion or independently stochastic future resets. FloatReset rows preserve projected component economics from the supplied schedule. Inference assumes canonical constant-principal segments; malformed/custom accrual metadata can fail rather than produce a guessed coupon.

A full instrument reconstruction could reproject all contractual details but needs an instrument-specific post-prepayment adapter. A proportional scale is simpler but would lose earned-interest protection and dated principal effects. No alternative is adopted here.

## Evidence

- **Implementation:** [residual rebuilding](../../../../finstack-quant/statements/src/capital_structure/residual_schedule.rs), `rebuild_residual_interest`, `principal_change`; [period-close driver](../../../../finstack-quant/statements/src/capital_structure/state.rs), `rebuild_residuals`.
- **Regression evidence:** [finance residual cases](../../../../finstack-quant/statements/src/capital_structure/tests/finance_residual_regressions.rs), split-coupon partial/full paydown, zeroed-component rate retention, both economic/settlement date directions, optional-sweep quarter-end behavior and later-PIK protection; inline tests exercise inferred rates and representability. Internal arithmetic and replay checks; not executed.

The established period-close balance change remains in force; this record does not reopen the waterfall FX/date policy.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-capital-structure).
