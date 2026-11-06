# VALUATIONS-OTHER-FIXED-INCOME-10: Revolver contractual state, utilization accrual and fee timing

- **Decision status:** Draft — observed implementation; adoption and rationale require agreement.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below.
- **Validation status:** Source and named evidence inspected or located; tests and numerical experiments were not executed. Evidence limits are stated below.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Choice and rationale

[CashflowEngine](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/cashflow_engine.rs) shares fee and coupon semantics between exact deterministic event replay and simulated utilization paths. The current drawn balance is state at `max(issue_date,as_of)`, not an origination amount to replay with past draws. [Types](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/types.rs), [validation](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/validation.rs) and [utilities](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/utils.rs) define material input and timing contracts. Adoption of the stochastic accrual approximation remains open.

## Algorithm

1. Build contractual unadjusted accrual periods, adjusted payments, term-index resets and observation dates. Observation dates include accrual boundaries, reset dates and dated commitment/margin/fee/LC changes. Default stub is ShortFront; calendar resolution and lag rules are explicit in utilities.
2. Deterministic events are sorted, then all events on/before the anchor reject. Replay future draws/repayments with capacity and balance checks. Initial funding is a negative cashflow only when issue is after valuation. Split coupon periods at events, resets and dated terms. Period-end events affect subsequent accrual, while dated principal flows settle on their own event dates.
3. On each deterministic subinterval charge `drawn × all_in_rate × year_fraction`. Fixed rates include cumulative margin deltas. Term floating resets before `as_of` require exact fixings; future resets project the index and apply gearing/floor/cap/spread rules. Overnight rates use daily compounding/projection and the fixing machinery. Actual/365L retains coupon-period metadata.
4. Fees use commitment minus drawn minus LC for undrawn, drawn for usage, total commitment for facility fees, and LC balance for LC/fronting fees. Fee tier utilization includes LC usage. Select the highest reached ordered threshold, add cumulative dated fee deltas and floor the resulting fee rate at zero. LC fee may inherit the running floating margin. Add dated fixed and commitment-reduction fees. Aggregate coupon/fee amounts to adjusted period payment date.
5. Stochastic paths use mean endpoint utilization on each observation subinterval. Future floating rates come from the reset observation, plus a deterministic index-versus-short-rate basis when needed; historical fixings override simulation. Deterministic forward mode replaces the raw baseline with the contractual projected index while preserving path shocks. Overnight MC cashflows compound the whole coupon window and reuse it across subintervals with equal margins.
6. Changes due to commitment steps book principal on the step date. The remaining utilization-driven net balance change over each coupon period books at its calendar midpoint, or midpoint from `as_of` for the active period. Sub-cent changes carry into subsequent periods. Final drawn balance repays on the adjusted final coupon date.

## Scope and conventions

Currency is the commitment currency; rates are annual decimals, fee/margin inputs basis points. Loan receipts are positive and draws negative. Deterministic history must be rebased into `drawn` before valuation. Current-period interest implicitly uses available anchor state for elapsed pieces; no full historic utilization path is reconstructed. Interest diagnostics are time-weighted rate averages and do not imply that opening balance times displayed rate reproduces a variable-balance coupon.

## Limitations, failure and alternatives

Validation rejects invalid currencies, dates, commitments, LC capacity, fees and incompatible process specifications. Repayment exceeding balance and draw exceeding capacity error. Stochastic utilization is capped for LC availability. Flat-fee convenience construction treats nonpositive commitment/usage fee rates as absent tiers; dated reductions floor fees at zero. Mean endpoint utilization and midpoint net funding do not reconstruct the timing of multiple draws and repayments inside an observation interval. A full daily utilization/funding process is an unimplemented alternative; exact dated draws are the supported deterministic alternative.

## Evidence

[Cashflow tests](../../../../finstack-quant/valuations/fixed-income/tests/revolving_credit/cashflows.rs), [fixing tests](../../../../finstack-quant/valuations/fixed-income/tests/revolving_credit/fixings.rs), [review regressions](../../../../finstack-quant/valuations/fixed-income/tests/revolving_credit/review_fixes.rs), and [fee tests](../../../../finstack-quant/valuations/fixed-income/tests/revolving_credit/metrics/utilization_fee.rs) provide contractual examples and parity evidence, not an external servicing reconciliation. [ISDA](../../../REFERENCES.md#isda-2006-definitions) supports date-convention context.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
