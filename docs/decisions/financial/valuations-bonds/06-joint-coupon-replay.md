# VALUATIONS-BONDS-06: Joint rates-credit pathwise coupon, principal and recovery replay

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed in the linked bond production scope; discrepancies are identified explicitly.
**Validation status:** Evidence inspected or located, not executed; no accuracy certification.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and scope

Joint pricing calibrates daily ACT/365F discount/survival targets through the final adjusted payment. Zero rate and hazard volatility use deterministic FRP rollback; either positive volatility uses sampled calibrated rates-credit paths, including sampled bullets without regression when there are no rights. Factor calibration/dynamics are owned by the models/framework scopes; this record owns the bond payoff replay.

## Algorithm

Build schedule-owned cash, balance, accrual, reset, payment and exercise events. Exact calendar events must lie on daily grid nodes within `1e−10`. Historical balances seed the path. Floating rows with projected-index metadata are grouped by accrual start/end/payment and replaced by compiled coupon replay; known term fixings remain fixed. An unknown term index uses `F_index,base + ((1/P_node(reset,end)−1)/α − F_discount,base)`: deterministic index/discount basis plus the node's conditional discount-forward shock. End is curve tenor from reset, snapped to the grid; tenor past calibration horizon errors. There is no separately stochastic index basis.

Overnight replay compiles its actual observation/weight schedule, historical fixings and configured fallback, index caps/floors and overnight day basis. Each unknown daily observation similarly adds a conditional discount-forward shock to the projected index rate. Path checkpoints retain sufficient observation history for lookback/lockout. Started coupons capture scheduled accrual-start notional and known resets.

At a step: advance live overnight observations; collect static and floating cash distributions; capitalize floating PIK at accrual end; apply static economic balance events; validate/clamp tiny negative outstanding to zero; observe new term resets; capture new accrual-start notional after same-day balance changes. Separate payment and accrual-end events avoid premature PIK settlement. Accrued state retains unpaid cash and uncapitalized PIK; static ACT/365L uses actual-day elapsed/full-period ratio, other static claims use day-count ratios, and floating claims use compiled coupon accrual. Negative floating PIK is rejected.

For each interval set `q=D_path·exp(−OAS·Δt)·S_path` and recovery `R·N_after_events·W_FRP`. Between decision dates aggregate these into affine continuation `A·V_next+B`, with interim cash/recovery in `B`. Default is integrated conditionally via survival/FRP weights rather than sampled as a default stopping time. Terminal repayment uses replayed outstanding, including pending PIK owed at early payment. Custom schedules with exercise are rejected, as are stochastic floating PIK plus amortization, segmented overnight coupons lacking original full-period scope, and coupons needing future observations after payment.

Static products precompute replay states. When hazard volatility is zero and rate volatility positive, interval terms can be cached by rate node. A failed FRP calculation in that optional cache is marked NaN; lookup then recomputes through the fallible path so the original error is propagated. This is a cache bypass, not acceptance of a NaN financial result.

## Observed discrepancies and alternatives

Static cash entitlement uses the holder filter, but dynamic floating groups are built from schedule rows without the ex-coupon test, and every dynamic cash coupon is paid. This is a source-level discrepancy, not an executed regression. Recovery uses replay outstanding on economic balance dates, whereas the hazard/deterministic path deliberately retains principal exposure until cash repayment. Lagged amortization can therefore give different recovery exposure across routes; the routes do not represent the same recovery-balance timing.

The deterministic-basis conditional-forward model is cheaper than separate rate-index factors but cannot capture basis volatility. A full contractual state engine is the available route for supported floating coupons; rates-only preprojection is restricted to deterministic rates.

## Evidence

Owning code: [routing](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/tree_pricer.rs), [template/replay](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/replay.rs), [observation compilation](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/build.rs), [conditional rates/intervals](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/state.rs). [LSMC tests](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/tests.rs) explicitly inspect lagged/early PIK, economic-date amortization, conditional-forward clocks, segmented-overnight rejection and block replay. These establish code contracts, not independent stochastic pricing accuracy. General references: [interest-rate modeling](../../../REFERENCES.md#andersen-piterbarg-interest-rate-modeling), [credit intensity](../../../REFERENCES.md#jarrow-lando-turnbull-1997).

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
