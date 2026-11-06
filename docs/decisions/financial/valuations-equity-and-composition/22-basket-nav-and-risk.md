# VALUATIONS-EQUITY-AND-COMPOSITION-22: Basket weighted NAV, daily expense drag and constituent shocks

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-22`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

Basket value is a linear combination of constituent values or explicit price scalars, converted to reporting currency and reduced by one day's expense ratio. ConstituentDelta is a quote-value scenario and WeightRisk is a financed reallocation scenario; neither estimates an underlying instrument's full factor sensitivity.

## Scope and input domain

Nonempty constituents, finite signed weights/units and positive days-in-year; notional currency equals reporting currency. Explicit units take precedence over weight per constituent and may coexist with weights in other constituents. Weights are not required to sum to one. Unitless prices are interpreted in reporting currency.

## Algorithm

For each constituent, price its embedded instrument at its native default or read the specified scalar. Convert monetary values to reporting currency using configured FX policy. Sum `price×units/shares_outstanding` for unit-based constituents or `price×weight` otherwise. Subtract `sum×expense_ratio/days_in_year`, then multiply by basket notional amount; the public value path passes shares_outstanding=1.

ConstituentDelta increases a constituent quote by 1% and returns the entire basket PV difference, with zero for absolute price bump≤1e-10. For embedded instruments, it first values the instrument and temporarily substitutes a synthetic bumped price reference. For market-data constituents, it bumps their shared price ID in the market. Components use ticker or constituent ID; total sums component scenario P&Ls.

WeightRisk rejects any explicit-unit constituent. For each target, add 0.0001 to its weight and scale all other weights by `(sum_others−0.0001)/sum_others`, preserving the original net weight budget. Reprice and report currency per 1 bp of weight change. Zero other-weight sum errors; aggregate is the sum of these separate directions.

## Selection, controls and conventions

Default expense day basis is 365.25 and FX policy is CashflowDate at the valuation date. The configured discount curve is declared as a dependency but the NAV formula does not discount basket value. Expense ratio is applied once per valuation, without an elapsed holding period. Basket cashflow export is a placeholder.

## Limitations, failures and alternatives

Basket notional is a multiplicative quantity applied to an already monetary NAV; callers must align scaling and quote units. Validation does not constrain expense_ratio to a finite nonnegative economic range (nonfinite results can fail at Money construction). A market price ID shared by multiple constituents bumps all of them, so separate components can overlap and their sum can double-count shared exposure. Embedded-quote bumps suppress the instrument's own nonlinear factor dynamics. A derivative delta, node-risk aggregation or accrued fee model would be distinct methods.

## Evidence

The [basket tests](../../../../finstack-quant/valuations/tests/instruments/basket/test_basket_comprehensive.rs) explicitly check `constituent_delta_is_pv_change_per_one_percent_move`, financed weight-budget preservation, mixed units/weights, FX and daily fee formulas. These establish the implemented scenario units; no general risk attribution theorem or external NAV reconciliation follows from them.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/src/instruments/exotics/basket/types.rs)
- [pricer.rs](../../../../finstack-quant/valuations/src/instruments/exotics/basket/pricer.rs)
- [constituent_delta.rs](../../../../finstack-quant/valuations/src/instruments/exotics/basket/metrics/constituent_delta.rs)
- [weight_risk.rs](../../../../finstack-quant/valuations/src/instruments/exotics/basket/metrics/weight_risk.rs)
- [mod.rs](../../../../finstack-quant/valuations/src/instruments/exotics/basket/metrics/mod.rs)
