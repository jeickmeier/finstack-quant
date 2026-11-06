# CASHFLOWS-004 — Scheduled principal amortization

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed**, scoped below. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: [slice audit](README.md).

## Choice and input domain

The builder prepares one amortization rule from all fixed and floating economic accrual boundaries, including coupon windows and rate groups. It distinguishes contractual amortization from final redemption. Opening and target notionals are same-currency nonnegative amounts; percentage inputs are finite decimals in `[0,1]`. Default is no scheduled amortization. No level-payment annuity solver is present in this slice.

## Formula and branch inventory

Let `B` be live principal after same-date PIK and explicit events, `B0` original notional, and `n` the number of distinct coupon accrual boundaries.

| Rule | Prepared amount and emission |
|---|---|
| None | No scheduled principal reduction. |
| LinearTo(F) | Require `0 ≤ F ≤ B0`; equal installment `(B0-F)/n`. On the last boundary replace it with `max(B-F,0)`, so PIK/draws can change the terminal scheduled amount. |
| StepRemaining | Strictly increasing dated targets. At each supplied date pay `B-target`; reject target above the live balance. A target above original principal can be valid after funding/PIK. |
| PercentOfOriginalPerPeriod(p) | Pay `B0 × p` on each boundary, capped at live balance. |
| PercentOfRemainingPerPeriod(p) | Pay `B × p` on each boundary, including that date's PIK/events in the base. |
| LinearBetween(start,end) | End must be a coupon boundary; start must be on/after issue. Count boundaries in `(start,end]`; capture equal installment from balance after all start-date movements. Pay it on each eligible boundary and sweep all remaining principal on `end`. |
| CustomPrincipal | Sum nonnegative same-date supplied amounts in Money; reject an amount above live balance. Zero entries do not create repayments. |

`LinearBetween` adds later Notional draws to the installment only **after** the current date's scheduled payment, dividing the added draw by remaining boundaries strictly after its funding date and on/before end. Draws at the end enter the final sweep. Early repayments reduce live balance; the fixed installment is capped, and final payoff includes new PIK. The installment update specifically recognizes positive `Notional` events, not every possible positive delta kind.

All variants emit at the matching coupon payment date when the economic date is a coupon boundary, otherwise on the raw supplied date. `principal_date` records the economic date. The common emitter caps computed repayment at outstanding, suppresses nonpositive amounts and subtracts it in Decimal. Explicit CustomPrincipal excess and StepRemaining increases are errors rather than silent caps. Any residual is later redeemed if automatic principal exchange is enabled.

## Selection, validation and limitations

Schedule-based rules require at least one coupon boundary; no coupon schedule means an error for LinearTo, either percentage rule or LinearBetween. Ambiguous payment dates for the same accrual boundary error. Dated rules must lie within issue/raw maturity and also not after effective terminal accrual. LinearBetween requires a nonempty ordered window and an aligned end. Fee periods do not create amortization cadence.

The prepared arithmetic uses Decimal; percentages and Money inputs cross through checked f64-to-Decimal conversion. No per-period currency-minor-unit rounding or contractual payment-rounding mode is selected here. Changing the number of coupon windows can change equal installment amounts. A calendar-month loan amortizer or annuity schedule is an unimplemented alternative in this builder.

## Evidence

- [amortization spec and validation](../../../../finstack-quant/cashflows/src/builder/specs/amortization.rs), [preparation](../../../../finstack-quant/cashflows/src/builder/orchestrator.rs), [emission](../../../../finstack-quant/cashflows/src/builder/emission/amortization.rs), [later-draw update](../../../../finstack-quant/cashflows/src/builder/pipeline.rs).
- Located hand-computed/identity tests in [amortization.rs](../../../../finstack-quant/cashflows/tests/cashflows/builder/amortization.rs): equal principal, geometric remaining balance, maturity residual and issue-date emission. [compiler_regressions.rs](../../../../finstack-quant/cashflows/tests/cashflows/builder/compiler_regressions.rs) locates `linear_between_installments_use_the_balance_entering_the_window`, `linear_between_later_draw_starts_repayment_after_its_funding_boundary` and adjusted-endpoint rejection.
- No contract-specific primary reference or independent vendor amortization comparison was located. The stated algorithm boundaries remain observed behavior, not adopted policies.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
