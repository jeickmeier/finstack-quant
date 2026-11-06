# VALUATIONS-BONDS-11: Workout enumeration, yield to worst and inverse workout pricing

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed in the linked bond production scope; discrepancies are identified explicitly.
**Validation status:** Evidence inspected or located, not executed; no accuracy certification.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and scope

Workout calculations enumerate contractual exits and use deterministic yield discounting; they do not optimize stochastic exercise. Option-bearing forward YTW/workout metrics require an explicit clean-price quote, including lowered return floors. Plain bonds can derive their dirty target from model PV carried to settlement. This separate quotation convention also supplies selected callable duration/spread cashflows.

## Algorithm

Generate call/put window start and end dates plus cashflow dates within each window, restricted to settlement through maturity; unlike tree exercise, ordinary workout enumeration adds no month ends. Calls sort by date then increasing strike. Puts on the same date collapse to the highest strike. Append maturity at the later of contractual maturity and last holder payment.

For each exit keep cashflows up to the exit, including same-settlement cash only when an exercise window contains settlement. Determine outstanding after economic events, adding contractual maturity principal back at maturity. Redemption is strike percentage of outstanding plus accrued, or make-whole-adjusted clean redemption plus accrued; subtract any same-day contractual maturity redemption already retained. Maturity path preserves its actual cashflows and appends zero additional redemption.

Solve each path's Street yield against target dirty price after subtracting immediate settlement cash. With no future cash, yield is negative infinity if immediate recovery is below target, otherwise positive infinity. If immediate cash already meets residual target, yield is positive infinity. Select the strict minimum across **calls, puts and maturity**; tie selection follows enumeration order. A put is thus treated as a candidate adverse exit, not an economically optimal holder exercise strategy. Ladder outputs convert nonfinite yields/spreads to nullable fields and report the selected row.

Inverse YTW computes each candidate path's price at requested yield and returns the minimum. I-spread inversion adds each path's horizon par rate, computes candidate prices, sorts by price, then returns the first candidate whose recomputed worst-yield path has the same horizon. If none is consistent it returns the lowest candidate price; it does not error for inconsistent workout selection.

## Limits, alternatives and evidence

Exercise windows are discretized differently from model exercise; workout ordering can include voluntary put paths and ignores call friction. Make-whole/redemption depend on today's curves rather than future conditional rates. Infinity handling and fallback are implemented quote behavior, not accepted market standards. Optimal call/put game valuation is available in trees/LSMC and should not be conflated with this ladder.

Owning code: [workout algorithms](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/yield_price.rs), [ladder/target](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/workout_ladder.rs), [quoted-workout cache selection](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/spread_price.rs), [YTW metric](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/ytw.rs). Inline tests and [workout tests](../../../../finstack-quant/valuations/fixed-income/tests/bond/metrics/workout_ladder.rs) locate amortization, same-day exercise and row invariants; not rerun. [Fixed-income reference](../../../REFERENCES.md#fabozzi-fixed-income-handbook) is general background.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
