# ATTRIBUTION-012: Yield-share credit carry and hierarchy-anchor allocation

- Decision status: Accepted 2026-11-16 for checked rates benchmark and credit carry allocation.
- Implementation status: Observed for the source scope listed below. Runtime, benchmark, cash, model and host reconciliation with the accepted contract are pending.
- Validation status: Independent exact examples checked signed benchmark carry, separate price and cash differences, signed hierarchy weights and a zero denominator. Production attribution and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked rates benchmark and credit carry allocation

1. Elect a product-qualified benchmark with no future default. Reprice the same holder claim at both cuts with zero future intensity on the exact active default sources. Keep contractual terms, known credit events, unpaid entitlements, frozen noncredit inputs and valuation controls fixed. A straight, live deterministic hazard bond is eligible. Other routes require product-owner qualification of the full comparison, including dependent sources and any credit-related pricing spread. Keep the benchmark's own opening mark. Retire first-curve selection, expiry-or-five-year proxies, yield ratios and clamps. [Accepted bond pricing](../valuations-bonds/03-pv-models.md#accepted-decision-for-checked-bond-pv-selection-and-fractional-recovery-of-par).
2. Split gross carry using checked marks and ledgers. Parent gross carry is `G = V_H − V0 + C_H`. Compute rates benchmark carry as `R = B_H − B0 + C_B` with the benchmark's eligible held cash, and credit carry as `G − R`. Show the price and cash differences separately. The selected order assigns joint rates and credit effects to the incremental credit amount. Benchmark cash stays within the explanation. The parent held and closing ledgers retain their roles under [record 003](03-total-return-carry.md#accepted-decision-for-checked-period-cash-gross-carry-and-funding).
3. Preserve signed amounts and the other carry views. Rates and credit reconcile to gross carry even when either is negative or larger than the total. Funding remains the separate signed deduction in net carry. Keep the separately elected income, pull-to-par and roll-down partition on its accepted basis. Retire automatic rates assignment for roll-down and automatic credit assignment at low or negative yields.
4. Elect hierarchy allocation separately. Use the frozen validated model and exact issuer reference from [record 011](11-credit-factor-hierarchy.md#accepted-decision-for-checked-credit-hierarchy-and-spread-replay-allocation). Require opening-cut signed spread components for generic credit, each active level and the issuer adder, and verify reconstruction of the opening reference spread. A stored calibration anchor qualifies only when its date and authority match the opening basis. Preserve validated folded zeros and require all active values. Credit carry allocation uses opening spread levels. Credit P&L hierarchy uses period moves.
5. Allocate credit carry in signed opening-spread proportions. For component `A_k` and verified nonzero `S = Σ_k A_k`, publish `(G − R) × A_k / S` in canonical order as a presentation allocation. Preserve negative weights and disclose gross weight `Σ_k |A_k| / |S|` so near-cancellation is visible. Exact zero makes the proportions undefined. Retire the epsilon and all-to-adder fallback. Check monetary closure and disclose any bounded representation rounding separately. [Accepted Money precision](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity).
6. Publish each elected layer atomically. Elect none, required or advisory separately for the rates and credit split and for hierarchy detail. Required failures propagate. Typed advisory unavailability preserves any independently qualified parent. Validate both ledgers and both levels of monetary closure before publication. Rust retains the benchmark and anchor identities, cuts, marks, cash, signed amounts and actual work for Python and WASM to project. [Accepted execution](01-execution-and-endpoints.md#accepted-decision-for-checked-instrument-attribution-execution-and-endpoints).

Exact examples verified a zero-recovery claim paying `100` in a held survival scenario. Opening discount factor `1.02` and survival `0.90` give gross carry `+8.20`, rates benchmark carry `−2.00` and credit carry `+10.20`. Separately, opening spread components `(80, −10, 20) bp` allocate `+9` of credit carry as `(+8, −1, +2)`.

## Observed choice and inputs

With credit factor model, eligible issuer metadata and a carry detail containing coupon income, split gross carry between rates and credit using an approximate risky-yield share. Use only the first declared opening hazard curve and first opening discount curve. The lookup date is instrument expiry, or five years from opening using metadata calendar/calendar_id (default `usny`) and Following adjustment. Calendar lookup is strict; date-add failure falls back to truncated 5×365.25 days, then opening date if that also overflows.

## Algorithm

1. Read continuous discount zero rate r and instantaneous hazard h at lookup date; individual lookup failures become zero with notes. Define spread approximation `s=h*(1-recovery)` (credit triangle).
2. Denominator is stable when `abs(r+s) > 1e-12*max(abs(r),s,0.001)`. If s>0 and total yield is nonpositive or unstable, set credit share q=1 with a note. Otherwise q=`clamp(s/(r+s),0,1)` for a stable denominator, or zero.
3. Split coupon income and pull-to-par with q; assign all roll-down to rates. Funding remains outside these gross rates/credit carry totals. Preserve original aggregate carry and annotate coupon/roll SourceLines.
4. Allocate credit carry using factor-model anchor spread: generic=`beta_pc*anchor_pc`, each level=`beta_level*anchor_bucket`, plus issuer adder-at-anchor. Missing beta/anchor entries contribute zero. If the sum has absolute value above 1e-15, multiply each component by credit-carry-total/sum. Otherwise allocate all credit carry to the adder. Per-level/per-issuer detail follows the same options as credit P&L.

## Conventions and limitations

This is a yield-share presentation, not a revaluation of credit versus rates carry or a calibrated recovery model. The first-curve choice and assigning every roll-down dollar to rates are explicit approximations. Hierarchy proportions use anchor levels, not observed factor moves; negative anchor components can produce signed allocations. Postprocessing errors become outer request notes and do not change headline credit/carry. Some SourceLines may already be mutated when a later detail construction errors.

## Rationale and alternatives

The source cites credit-triangle motivation; [O'Kane](../../../REFERENCES.md#o-kane-2008) is background for the spread approximation, not approval of this carry allocation. Repricing risk-free/hazard counterfactual carry or term-by-term cashflow allocation are unimplemented alternatives. Tests `carry_partition_includes_pull_to_par`, `per_factor_allocation_sums_to_credit_carry_total`, and `negative_total_yield_routes_coupon_to_credit` are formula/identity checks; independent economic benchmarks remain open.

## Source and evidence

- [credit_decomposition.rs](../../../../finstack-quant/attribution/src/credit_decomposition.rs) — owning implementation.
- [credit_cascade.rs](../../../../finstack-quant/attribution/src/credit_cascade.rs) — owning implementation.
- [detail.rs](../../../../finstack-quant/attribution/src/detail.rs) — owning implementation.

## Follow-up tracking

Runtime reconciliation and validation are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). Production and host
qualification remain pending.
