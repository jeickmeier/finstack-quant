# IRS cashflow projection, convention resolution and sensitivity bases

- ID: VALUATIONS-RATES-03-INTEREST-RATE-SWAPS
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed in the owning paths below.
- Validation status: Evidence located/inspected, not executed; no universal accuracy qualification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and rationale

IRS valuation discounts separately generated fixed and floating coupon schedules with one shared collateral discount curve. Term floating projection and overnight observation compounding are distinct paths. Equal forward/discount identifiers permit a single-curve compounded OIS path; otherwise projection requires the forward curve. There is no principal exchange in the swap schedules.

## Algorithm and controls

1. Resolve negative reset/payment-lag sentinels from the forward-ID convention registry; an unresolved sentinel errors. Both-endpoints-at-month-end automatically enables EOM even if the stored flag was false. Fixed/float accrual-date adjustment flags separately control schedule generation.
2. Fixed coupon is `N*K*alpha`; classify an irregular front/back stub from expected tenor stepping and EOM boundaries. The annuity sums `alpha*D(as_of,pay)` over payments strictly after valuation, using Neumaier summation.
3. Simple floating coupons use the cashflow builder with unit gearing, spread included, no cap/floor and Error fallback when a market exists. A schedule-only call without market uses SpreadOnly as a representation fallback, not a valid market forecast.
4. Compounded arrears/observation-shift/rate-cutoff schedules use the shared overnight projector. Resolve fixing calendar from explicit fixing calendar, leg calendar or supported currency convention. Adjust observation accrual boundaries and skip collapsed intervals; ACT/365L retains the original coupon reference interval. Historical observations and remaining projected observations determine the compound factor. Add spread simply outside compounding and retain projected-fixing metadata.
5. Single-curve compounded OIS uses a same-ID forward curve if present, otherwise the discount curve as overnight projection source. Normal dual-curve pricing requires the forward. Sum only future coupon payments; continue past raw maturity when payment lag leaves unpaid coupons. Pay-fixed PV is floating PV minus fixed PV; Receive reverses it.
6. ParRate is floating PV divided by `N*fixed_annuity`; near-zero annuity errors. Leg-PV metrics are unsigned leg values. SimpleAverage is rejected. Discount IDs must match across legs; mismatched raw leg date ranges warn rather than error. Notional must exceed `1e-6`; fixed decimal rate magnitude above 100 errors.

## Risk and margin algorithms

DV01 uses central quote-bump calibration replay when recipes exist, including a discount-only replay for single-curve OIS with no separate forward object; otherwise generic combined curve bumps. Per-curve PV01 and triangular bucketed DV01 use the generic curve-bump engine. IR convexity centrally bumps all existing declared rate curves and divides by squared decimal bump. IR cross-gamma uses four discount/forward bumps; equal IDs or missing participating curves returns zero.

SIMM first sums repriced bucketed DV01 into tenor buckets. If no repriced sensitivity is available, it warns and uses `abs(N)*remaining_calendar_years*DURATION_APPROXIMATION_FACTOR*1bp`, signed by payer/receiver, distributed by overlap lengths in fixed 6M–30Y buckets. Expiry for this margin path is float-leg end. A partially available nonempty set is accepted as repriced, so completeness depends on the shared margin helper. VM uses the instrument value.

## Alternatives and tradeoffs

Quote calibration sensitivity answers a different perturbation question from direct zero/forward-curve shifts. A hard failure on unavailable margin risk would avoid substituting the duration proxy; that is a policy choice, not the current branch. Overnight spread compounding and alternate fixed/accrual calendar policies would change contract economics.

## Evidence

- [IRS types/conventions](../../../../finstack-quant/valuations/rates/src/irs/types.rs), [cashflow projection](../../../../finstack-quant/valuations/rates/src/irs/cashflow.rs), [PV](../../../../finstack-quant/valuations/rates/src/irs/pricer.rs), [annuity](../../../../finstack-quant/valuations/rates/src/irs/metrics/annuity.rs), [metric selection/par](../../../../finstack-quant/valuations/rates/src/irs/metrics/mod.rs), [DV01](../../../../finstack-quant/valuations/rates/src/irs/metrics/dv01.rs), [convexity/cross-gamma](../../../../finstack-quant/valuations/rates/src/irs/metrics/ir_convexity.rs), [margin](../../../../finstack-quant/valuations/rates/src/irs/marginable.rs).
- [Compounding accuracy](../../../../finstack-quant/valuations/rates/tests/irs/test_compounding_accuracy.rs), [cashflow reconciliation](../../../../finstack-quant/valuations/rates/tests/irs/cashflows.rs), [par-rate tests](../../../../finstack-quant/valuations/rates/tests/irs/metrics/par_rate.rs), [margin tests](../../../../finstack-quant/valuations/rates/tests/irs/integration/margin.rs): formula, lifecycle and risk invariants.
- [Ametrano/Bianchetti](../../../REFERENCES.md#ametrano-bianchetti-2013), [ARRC](../../../REFERENCES.md#arrc-sofr-users-guide), [ISDA](../../../REFERENCES.md#isda-2006-definitions).

## Evidence limits

In same-ID OIS, the projection path uses an existing forward object but `market_dependencies` omits forward dependency based on equal IDs alone. No independent certification of margin proxy or partial-bucket completeness is established.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
