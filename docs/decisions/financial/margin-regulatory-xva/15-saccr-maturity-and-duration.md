# MARGIN-REGULATORY-XVA-015: SA-CCR per-trade maturity factors and supervisory duration

- Decision status: **Accepted 2026-11-17 for checked SA-CCR maturity factors and supervisory duration.**
- Implementation status: Existing behavior is described below. Qualified clock usage, the regulatory duration floor, checked effective notional and applied-factor reporting remain pending.
- Validation status: Primary rules and Rust and host paths checked. Independent arithmetic covered date conversion, short and forward-starting duration, cash and physical settlement, and the range of margined factors. Runtime, schema, host and regulatory qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Accepted decision for checked SA-CCR maturity factors and supervisory duration

1. **Fix the contractual and regulatory time conventions.** Use the
   qualified `M`, `T`, `S` and `E` from the same valuation cut. Measure
   contractual date offsets on ACT/365F. For a time-aware exercise, use
   verified elapsed time between valuation and exercise instants on the
   same fixed 365-day basis. Use 250 business days per regulatory year to
   express the duration floor, maturity floor and MPOR. Carry the qualified
   underlying `E` to the IR maturity bucket.
   [Record 014](14-saccr-inputs-and-supervisory-delta.md#accepted-decision-for-checked-sa-ccr-trade-inputs-and-supervisory-delta) ·
   [Basel CRE52.31–34 and 52.48–53](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/52/inforce/2023-01-01/published/2020-03-27).
2. **Qualify dates and lifecycle before applying the floors.** Reject
   reversed reference periods, nonfinite times and unresolved lifecycle
   states. Set `S = 0` once the referenced period starts and retain the
   qualified `E`. A trade with zero remaining time receives the floor only
   when its qualified lifecycle establishes remaining exposure under the
   selected treatment. Expired or matured trades with unsettled obligations
   require an explicit remaining-exposure treatment.
   [Record 014](14-saccr-inputs-and-supervisory-delta.md#accepted-decision-for-checked-sa-ccr-trade-inputs-and-supervisory-delta).
3. **Floor IR and credit supervisory duration at ten business days.** In
   both margin branches, use `SD_raw = [exp(−0.05S) − exp(−0.05E)] / 0.05`
   and `SD = max(SD_raw, 10/250)`. Evaluate the raw difference as
   `exp(−0.05S) × [−expm1(−0.05(E−S))] / 0.05` with the span derived
   directly from the qualified dates. A live one-calendar-day period
   starting at valuation has raw duration about `0.002740` years and
   applied duration `0.04` years.
   [Basel CRE52.34](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/52/inforce/2023-01-01/published/2020-03-27) ·
   [OSFI supervisory-duration formula](https://www.osfi-bsif.gc.ca/en/guidance/guidance-library/capital-adequacy-requirements-car-2026-chapter-7-settlement-counterparty-risk).
4. **Calculate unmargined maturity per trade or prescribed component.**
   Use `MF = √min(max(M, 10/250), 1)` with `M` in years to the qualified
   end of risk. The Basel six-month-into-five-year swaption example gives
   about `0.707107` for cash settlement and `1.0` for physical settlement.
   A qualifying daily settlement and reset uses the `0.2` unmargined
   factor. Daily collateral margining follows the applicable MPOR branch.
   Both preserve the referenced underlying period.
   [Basel CRE52.31–32, 52.37 and 52.48–49](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/52/inforce/2023-01-01/published/2020-03-27).
5. **Use the qualified effective MPOR for margined contributions.** Apply
   `MF = 1.5 × √(MPOR_business_days/250)` with the estimated and floored
   MPOR selected under record 014. Contributions subject to the same
   effective MPOR share that factor. Apply the calculated value even
   above one. At 250 business days the factor is `1.5`.
   [Record 018](18-saccr-rc-pfe-ead.md) owns mixed-agreement assembly and the
   EAD cap.
   [Basel CRE52.50–53](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/52/inforce/2023-01-01/published/2020-03-27).
6. **Apply each factor once in effective notional.** For IR and credit,
   multiply the qualified reporting-currency size by the applied SD to
   obtain adjusted notional. Other classes use their prepared adjusted
   notional. Multiply adjusted notional by supervisory delta and the
   applicable MF in one checked Rust path. Reject invalid inputs and
   unrepresentable results before aggregation. Records
   [016](16-saccr-ir-addon.md#accepted-decision-for-the-checked-sa-ccr-interest-rate-add-on)
   and [017](17-saccr-non-ir-addons.md) own supervisory weights and offsets.
   [Basel CRE52.30 and 52.33–36](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/52/inforce/2023-01-01/published/2020-03-27).
7. **Replace the reported summary with applied factor evidence.** Return
   actual MF and applicable SD by trade or prescribed component, together
   with the qualified time basis and margin branch. Identify shared MPOR
   factors by the group that used them. A covered empty book yields an
   empty factor collection. Reconcile the existing Rust, wire, Python and
   WASM result surfaces to this contract.

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

The add-on uses per-trade unmargined maturity factors or one shared MPOR factor for margined books. IR and credit additionally use exponential supervisory duration of the underlying. The single maturity factor reported in the result is a separate summary and does not drive add-on arithmetic.

## Algorithm and conventions

1. Measure underlying start/end offsets from `as_of` using actual calendar days divided by 365. Set `S=max(start_offset,0)`, `E=max(end_offset,S)`, and `SD=max((exp(−0.05S)−exp(−0.05E))/0.05,0)`. This can be exactly zero; there is no positive floor.
2. Unmargined trade maturity is `max(days(option_expiry−as_of),0)/365` for options, otherwise the analogous underlying end date. `MF=sqrt(min(max(M,10/250),1))`: the floor is 0.2 and cap is 1.
3. Margined trade `MF=1.5×sqrt(MPOR_business_days/250)` with the validation floors in record 14. There is no upper cap in this helper. The same MF applies to all trades.
4. The reported unmargined `EadResult.maturity_factor` first averages nonnegative **underlying end-date** maturities equally across all trades, then applies the unmargined formula. It is neither the average of trade MFs nor notional-weighted, and it ignores option expiries. An empty book reports 0.2. Margined reporting uses the actual common MPOR factor.

## Selection and controls

Supervisory duration is selected only for IR/credit. The internal scalar unmargined helper itself accepts any f64 without explicit validation. Rust max/min operations can turn NaN into the finite floor. Normal engine inputs derive finite date differences. `maturity_factor_margined` accepts `u32` MPOR directly and relies on the engine's config validator for minimum business-day rules.

## Limitations and failure behavior

The reported factor uses an equal-weighted mean end maturity before a nonlinear transform, and the SD floor is zero. Swapped start/end dates silently give zero SD; matured non-IR/non-credit trades can still receive the 0.2 MF floor.

## Alternatives and tradeoffs

The accepted reporting contract carries the factors used by the checked
calculation. Trade or component identity and shared MPOR group membership
make those factors traceable to the applied clocks.

## Evidence and role

Runtime: [src/regulatory/sa_ccr/maturity_factor.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/maturity_factor.rs), [src/regulatory/sa_ccr/add_on.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/add_on.rs) and reporting code in [src/regulatory/sa_ccr/engine.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/engine.rs). Inspected add-on tests derive exponential SD, forward-start offsets, mixed-tenor trade MFs and shared margined MF; swapped-date tests only assert nonnegative results. These verify current formulas and repair behavior, not lifecycle acceptance. [SA-CCR reference index](../../../REFERENCES.md#bcbs-279-saccr).

Implementation and qualification are tracked in the
[regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
