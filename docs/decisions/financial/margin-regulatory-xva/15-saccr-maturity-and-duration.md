# MARGIN-REGULATORY-XVA-015: SA-CCR per-trade maturity factors and supervisory duration

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed** in the stated source scope.
- Validation status: **Evidence inspected/located, not executed.** Selected test bodies were read; no runtime test, build or numerical experiment was run.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

The add-on uses per-trade unmargined maturity factors or one shared MPOR factor for margined books. IR and credit additionally use exponential supervisory duration of the underlying. The single maturity factor reported in the result is a separate summary and does not drive add-on arithmetic.

## Algorithm and conventions

1. Measure underlying start/end offsets from `as_of` using actual calendar days divided by 365. Set `S=max(start_offset,0)`, `E=max(end_offset,S)`, and `SD=max((exp(−0.05S)−exp(−0.05E))/0.05,0)`. This can be exactly zero; there is no positive floor.
2. Unmargined trade maturity is `max(days(option_expiry−as_of),0)/365` for options, otherwise the analogous underlying end date. `MF=sqrt(min(max(M,10/250),1))`: the floor is 0.2 and cap is 1.
3. Margined trade `MF=1.5×sqrt(MPOR_business_days/250)` with the validation floors in record 14. There is no upper cap in this helper. The same MF applies to all trades.
4. The reported unmargined `EadResult.maturity_factor` first averages nonnegative **underlying end-date** maturities equally across all trades, then applies the unmargined formula. It is neither the average of trade MFs nor notional-weighted, and it ignores option expiries. An empty book reports 0.2. Margined reporting uses the actual common MPOR factor.

## Selection and controls

Supervisory duration is selected only for IR/credit. The public scalar unmargined helper itself accepts any f64 without explicit validation; Rust max/min operations can turn NaN into the finite floor. Normal engine inputs derive finite date differences. `maturity_factor_margined` accepts `u32` MPOR directly and relies on the engine's config validator for minimum business-day rules.

## Limitations and failure behavior

The reported factor uses an equal-weighted mean end maturity before a nonlinear transform, and the SD floor is zero. Swapped start/end dates silently give zero SD; matured non-IR/non-credit trades can still receive the 0.2 MF floor.

## Alternatives and tradeoffs

Reporting a notional-weighted average of actual MFs might aid interpretation but would still be descriptive, not reproduce correlated add-ons. Rejecting invalid dates or nonfinite low-level inputs gives a stronger domain contract; keeping permissive scalar helpers requires explicit caller guarantees.

## Evidence and role

Runtime: [src/regulatory/sa_ccr/maturity_factor.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/maturity_factor.rs), [src/regulatory/sa_ccr/add_on.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/add_on.rs) and reporting code in [src/regulatory/sa_ccr/engine.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/engine.rs). Inspected add-on tests derive exponential SD, forward-start offsets, mixed-tenor trade MFs and shared margined MF; swapped-date tests only assert nonnegative results. These verify current formulas and repair behavior, not lifecycle acceptance. [SA-CCR reference index](../../../REFERENCES.md#bcbs-279-saccr).

The implementation establishes observed behavior, not financial adoption or regulatory compliance. Actionable work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
