# VALUATIONS-BONDS-19: Bond carry clock and cash-payment weighted average life

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed in the linked bond production scope; discrepancies are identified explicitly.
**Validation status:** Evidence inspected or located, not executed; no accuracy certification.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and scope

The bond adapter selects a Street-compounded yield clock for the shared carry decomposition, and a separate principal-cash weighted-average-life calculation. These are deterministic reporting calculations. They do not estimate physical default/prepayment probabilities or optimal-exercise life.

## Algorithms

Carry accretion is `A(t)=1/DF_Street(y,t)`, with coupon frequency/day count from the bond. ICMA resolves the quasi-coupon period from cached payment dates; ACT/365L splits at contractual coupon boundaries. The origin is valuation date, even though YTM is quoted at settlement. The shared carry calculator reprices on unchanged curves at the horizon, materializes crossed fixings at their valuation projections, separates income and principal cash, and computes `pull=PV0(A−1)−coupon_income−Σ payment·(A/A_payment−1)`. Roll-down is `PV_horizon−PV0+principal_cash−pull`; total carry adds coupon income and subtracts funding. Missing YTM sets pull to zero with a degenerate diagnostic, while no forward horizon gives zeros. Funding, where a repo curve exists, is `PV0(1/DF_repo−1)`. Horizon selection and period-cash classification remain shared-framework methods.

Bond WAL filters future schedule rows strictly after valuation to positive Amortization and positive Notional cashflows only. Use actual **cash payment date**, ACT/365F time and cash amount: `WAL=Σ principal_cash·t/Σ principal_cash`; no qualifying cash returns zero. PIK, coupons, prepayments and revolver repayments are excluded from this bond metric. A premium/OID redemption weights its actual cash rather than par. This differs intentionally in source from `CashFlowSchedule::wal`, which includes other principal repayments and uses balance-effective dates/par amounts.

## Limits, alternatives and evidence

Cash WAL and economic-balance WAL can materially differ under payment lags, premiums and custom principal kinds. Carry inherits quote/model repricing semantics and does not pin OAS here through the dedicated bond risk view. Settlement-origin carry would be another convention. Name alone does not identify the WAL/carry basis; the underlying schedule and clock must be disclosed.

Owning code: [bond yield clock](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/carry.rs), [metric registration](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/mod.rs), [shared decomposition](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/carry_decomposition.rs), [bond WAL](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/wal.rs). [WAL tests](../../../../finstack-quant/valuations/fixed-income/tests/bond/metrics/wal.rs) locate bullet-maturity and passage-of-time checks; inline tests in the bond carry module locate convention and payment-boundary checks. Reference: [Tuckman–Serrat](../../../REFERENCES.md#tuckman-serrat-fixed-income).

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
