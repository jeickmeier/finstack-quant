# VALUATIONS-STRUCTURED-CREDIT-03: Statistical collateral cashflow projection

- **Decision status:** Accepted 2026-11-15 — complete statistical collateral exposure, cash order, amortization, coupon and recovery package.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent stub, target capacity, cash and term-fixing calculations checked. Production pricing, reporting and host qualification remain pending.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Accepted decision for statistical collateral projection

1. **Compound conditional mortality over actual exposure.** Select the
   qualified loan, cohort or deal seasoning anchor for each assumption.
   Split actual contractual exposure at the corresponding model-month
   boundaries. For each intersection use its actual days divided by the
   days in that model month as weight `w_m`. Form survival from the
   product of `(1-p_m)^w_m` for conditional SMM or MDR. Use checked
   log survival arithmetic and preserve zero and full mortality and
   required positive residual survival at the representation boundary.
   Stop at contractual maturity and keep payment rolls separate from
   exposure. Remove the ending-month-rate shortcut across changing
   model-month rates.
   [SIFMA mortgage prepayment conventions](https://www.sifma.org/wp-content/uploads/2017/08/chsf.pdf),
   [Accepted mortality](../cashflows/10-prepayment-and-mortality.md).

2. **Use dated original-par targets as amounts.** Cumulative-loss and
   timing laws require an expressly fixed original-par reference and
   qualified target endpoints. Post each due monthly gross default
   increment once, including endpoints in a stub. Check it against
   actual eligible par and reduce available balance before the next
   increment. Reject infeasible demand and overlapping target authority
   over a reference cohort. Keep reference severity for gross-demand
   conversion separate from actual recovery assumptions.
   [Accepted default and recovery assumptions](../cashflows/11-default-and-recovery-assumptions.md).

3. **Apply default, scheduled principal and prepayment in order.** Apply
   conditional default to opening performing par, scheduled principal to
   its survivors and prepayment to the post-scheduled balance. At ordinary
   maturity pay remaining survivor par once as scheduled principal.
   Use full contractual day-count context and midpoint conditional default
   for interest. Prepayments earn interest through interval end. Use
   designated default dates for interest on dated target amounts. Split
   or clip exposure at contractual boundaries and retain separately
   authorized opening accrued claims. Conditional default-first cash
   ordering and midpoint interest form a grid-dependent period model.
   [FHFA post-scheduled prepayment formula](https://www.fhfa.gov/sites/default/files/2023-05/Investment_Portfolio_Management_Module_Final_Version_1_0-508.pdf).

4. **Separate contractual installments from cohort attrition.** Use
   explicit dated scheduled principal or a qualified contractual
   installment. Derive a regular level-pay installment from the applicable
   periodic rate and actual remaining payment count with an exact zero
   rate branch. Interest-only periods have zero scheduled principal.
   Amortization beyond contractual maturity leaves a balloon. Reduce
   this period's aggregate cohort installment by default survival and
   future installments by actual whole-loan prepayment survival.
   A blocked prepayment leaves that part of the installment unchanged.
   Require explicit terms for irregular periods, resets that recast
   payments, modifications, extensions and any capitalization of unpaid
   interest. Qualify carry or recomputation against the applicable terms.

5. **Use a single floating coupon contract.** Require exact index identity,
   fixing calendar and lag, actual value dates and index accrual, apart
   from coupon accrual and source curve clock. Read authorized published
   fixings by benchmark identity. Project only qualified pending
   observations from dated curve factors. Apply contractual index bounds,
   then spread, then any explicit all-in bounds on all branches. Preserve
   signed rates. Remove the stored-rate fallback and projected-only zero
   floor. A negative payable coupon requires a qualified funded outflow
   or an error.
   [Accepted floating projection](../cashflows/06-floating-rate-projection.md),
   [ARRC term-rate conventions](https://www.newyorkfed.org/medialibrary/Microsites/arrc/files/2021/Term_SOFR_Avgs_Conventions.pdf).

6. **Date recoveries from the default vintage.** For conditional defaults
   use contractual exposure end as the modeled vintage. For dated target
   defaults use the designated endpoint and for known claims use their
   actual date. The midpoint convention affects interest only. Select and
   preserve recovery authority at default, then use the checked calendar-
   month lag and qualified cash placement. Carry gross defaulted par
   even with zero recovery, initialize known claims once and propagate
   arithmetic and date errors. The terminal owner determines release,
   call and final loss treatment.
   [Terminal cash and loss recognition](14-loss-recognition-and-terminal-cash.md#accepted-decision-for-loss-recognition-and-terminal-cash).

7. **Use the checked Rust rules across qualified routes.** The
   deterministic route supplies expected flows. Stochastic and OAS
   routes supply qualified path assumptions and reuse cash and balance
   rules as applicable. Future shocks preserve authorized fixings.
   Use the same validation and cash classification in Rust and existing
   Python and WASM interfaces. Delinquency, workout, commercial-mortgage
   and card-specific rules remain with their owners.

## Independent calculations

- With a 2024-01-31 month anchor, exposure from 2024-02-15 to 2024-03-15
  takes weights 14/29 and 15/31 across monthly rates 1% and 4%.
  Compounded mortality is 2.43042417%, compared with 3.86913520% using
  the ending rate throughout.
- Fixed original par 100, opening eligible par 75 and cumulative target
  increments 10% and 20% post gross defaults of 10 and 20 and leave
  par 45. Compounding two rates both formed against 75 would leave
  47.66666667 and miss the target.
- Opening par 1,000, annual coupon 12%, quarter accrual, installment
  150, period default 10%, prepayment 20% and recovery 40% produce
  interest 28.50, defaulted par 100, scheduled principal 108 and
  prepaid par 158.40. Ending par is 633.60, future aggregate installment
  108 and recovery 40 due separately. A following maturity slice with
  fraction 15/360 and default 5% leaves balloon par 601.92.
- A 2025-07-07 value start with 2025-07-04 closed and two business
  days' fixing lag fixes on 2025-07-02. A −60 bp fixing or projection
  with −50 bp index floor and +25 bp spread gives −25 bp. An expressly
  supplied zero all-in floor gives zero in either branch.

## Observed implementation and scope

Statistical collateral is an expected-balance recursion. Default, scheduled principal and prepayment compete for the same balance in an ordered period calculation. The following sections describe the observed paths. The accepted contract above governs reconciliation.

## Algorithm and conventions

1. Compute seasoning from origination/acquisition (or closing). For integer multi-month periods, aggregate each underlying month's SMM/MDR by multiplying survival; the period receives `1 - product(1 - monthly_rate)`. Contractual exposure counts whole calendar months plus the actual fraction of the next calendar month. For monthly and noninteger stub periods, the monthly-rate helper samples the end-of-period seasoning rate; downstream survival raises it to that fractional month count.
2. Apply default to opening performing balance first. Interest is approximately opening balance × annual coupon × day-count accrual × `(1 - 0.5 × period default probability)`, with special-servicing/appraisal adjustments when configured. Thus defaults occur halfway through the interest period in expectation; prepaid principal otherwise earns the period's interest.
3. For amortizing asset types, compute scheduled payment from an explicit contractual payment or stable level-payment annuity. The helper uses `ln_1p`/`exp_m1` forms, a zero-rate branch and at least one remaining period. Interest-only terms suppress scheduled amortization; remaining term may exceed contractual maturity, creating a balloon.
4. Subtract scheduled principal from default survivors, then apply prepayment to the surviving balance. At contractual maturity remaining balance pays as a balloon unless CMBS or NPL terms alter it.
5. Enqueue recovery proceeds and losses with the configured recovery lag. Cumulative loss/timing specifications and per-asset overrides pass through their own probability transformations; existing default claims initialize a separate recovery queue.

Fixed rates are annual decimals. Floating row coupons use a forward-curve identifier, its tenor/reset lag and a weekends-only reset calendar. The index floor is in bp and spread is added after it. Future reset coupons project from the curve and floor the resulting all-in rate at zero. Historical observed fixing plus spread is not given that final zero floor; if no fixing series exists at all, the stored all-in asset rate is returned directly. An existing series missing the required observation errors. Thus negative-rate treatment differs by historical/projected branch.

## Selection, limitations and alternatives

`DeterministicPoolFlowSource` uses model expectations. Stochastic and OAS sources reuse the same cashflow/waterfall engine with path-specific hazards, prepayment and recovery. Explicit contractual-payment and nonamortizing branches are supported. Daily event-time default/prepayment, stochastic borrower-level payment behavior, and full cohort recalibration are not implicit alternatives.

The default/prepay ordering and half-period interest adjustment are approximations. The inspected evidence does not establish cached level-payment and term-reset accuracy for modifications and extended slices. Noninteger stubs apply the final month's model rate over the fractional exposure rather than integrating a varying seasoning curve. Recovery scheduling and the different ordinary-terminal versus call treatment are governed by the terminal-policy record.

## Implementation and validation evidence

- [structured_credit/pricing/simulation_engine/pool_flows.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/pool_flows.rs)
- [structured_credit/pricing/simulation_engine/pool_flow_source.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/pool_flow_source.rs)
- [structured_credit/pricing/simulation_engine/period_helpers.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/period_helpers.rs)
- [structured_credit/utils/amortization.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/utils/amortization.rs)
- [structured_credit/pricing/simulation_engine/recovery.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/recovery.rs)

Existing evidence: [structured_credit/unit/amortization_term_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/amortization_term_tests.rs); [structured_credit/unit/seasoning_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/seasoning_tests.rs); [structured_credit/unit/index_floor_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/index_floor_tests.rs); [structured_credit/unit/stochastic_pricing_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/stochastic_pricing_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

Reference context: [MBS formulas](../../../REFERENCES.md#sifma-mbs-standard-formulas). These references identify the method family; they do not certify the preset parameters or every implementation branch.

The inspected stochastic_waterfall_matches_independent_cashflow_vectors assertion checks explicit one/four-quarter expected vectors to 1e-6; it does not validate heterogeneous borrower timing or lagged-recovery market accuracy.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
