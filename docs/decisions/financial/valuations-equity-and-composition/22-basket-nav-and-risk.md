# VALUATIONS-EQUITY-AND-COMPOSITION-22: Basket value, accrued expense and mark and weight risk

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-22`.
- **Decision status:** Accepted 2026-11-14 — complete basket value, expense and risk package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent examples cover mixed-currency value, accrued expense, shared and embedded mark shocks, and coefficient reallocations. Production pricing, risk and host qualification remain pending.

## Accepted decision for checked basket value, expenses and risk

1. **Define a basket unit and position size explicitly.** Each
   row has a unique stable constituent ID and supplies exactly
   one finite signed lot quantity or fixed linear price
   coefficient per basket unit. Use coefficients as authored.
   Lot and coefficient rows can coexist. Sum translated row
   contributions, deduct the elected accrued expense per
   basket unit and multiply once by a finite signed count of
   basket units. Remove the monetary basket notional and the
   inferred share count. Official ETF NAV and fund-share
   accounting remain with their respective owners.
   [SEC ETF bulletin](https://www.sec.gov/investor/alerts/etfs.pdf).

2. **Qualify marks and translate at the valuation cut.** A
   direct price binds the exact ID, monetary value, native
   currency and quote-lot basis. Resolve each embedded
   instrument on its qualified native position and valuation
   route at the same valuation and information cut. Resolve
   each admitted row, including zero-sized rows. Use checked
   Money arithmetic and explicit valuation-date FX policy for
   the ordered native-to-reporting pair. Same-currency
   translation is identity. Use the qualified source to
   establish the date and basis of price and FX inputs. Remove
   the inert basket discount curve and expose the actual
   direct-price, embedded-market and FX dependencies.
   [Money rules](../core-conventions/012-money-decimal-precision.md) ·
   [FX rules](../core-conventions/013-fx-resolution-and-shocks.md).

3. **Deduct a known accrued amount once.** Require an explicit
   no-expense election or a nonnegative, qualified
   reporting-currency accrued amount per basket unit at the
   valuation cut. Hold this amount fixed in the basket
   scenarios. Remove the automatic one-day expense haircut
   and the basket expense-ratio metric.

4. **Publish the relative mark scenario as `basket_mark_shock`.**
   Replace the legacy basket `constituent_delta`. Multiply the
   relevant prepared mark by 1.01 and report signed
   reporting-currency P&L. Group direct rows by exact price ID
   and shock each group once across all its direct rows. Key
   embedded mark shocks by constituent ID and hold their
   underlying models and markets fixed. Price IDs and
   constituent IDs occupy distinct result-key categories.
   Hold FX, holdings, accrued expense and basket size fixed.
   The headline is the simultaneous shock to all eligible
   prepared marks. Zero marks have zero relative movement.
   Reject unrepresentable nonzero moves and remove the
   small-bump cutoff.
   [Metric identifiers](../valuations-framework/0010-metric-identifiers.md).

5. **Report coefficient reallocation by target.** Admit weight
   risk for all-coefficient baskets. Raise the target by
   `h = 0.0001` and multiply the other coefficients by
   `(S - h) / S`, where `S` is their checked signed sum. Hold
   prepared marks, FX, accrued expense and basket size fixed.
   Preserve the net coefficient budget. Reject a zero or
   unusable donor sum and a multiplier that reverses donor
   signs. Report the signed monetary P&L for each target.
   Remove the total across separate directions. A basket with
   lot rows has no weight-risk result. Cash financing has its
   own basis.

6. **Use canonical Rust validation and export.** Apply the
   same definition validation at construction, restore and
   pricing. Constituent count is the count of authored rows.
   A value breakdown can reconcile marks, FX, accrued expense
   and scale. A complete contractual valuation cashflow
   export requires matched components that reconcile to the
   selected value. Otherwise it is unavailable. Carry the
   canonical results on the existing Python and WASM
   instrument-envelope routes. Composite sizing and history
   remain with records 23 and 24.
   [Valuation cashflow export](../valuations-framework/cashflow-export.md).

## Observed behavior

### Choice and rationale

Basket value is a linear combination of constituent values or explicit price scalars, converted to reporting currency and reduced by one day's expense ratio. ConstituentDelta is a quote-value scenario and WeightRisk is a financed reallocation scenario; neither estimates an underlying instrument's full factor sensitivity.

### Scope and input domain

Nonempty constituents, finite signed weights/units and positive days-in-year; notional currency equals reporting currency. Explicit units take precedence over weight per constituent and may coexist with weights in other constituents. Weights are not required to sum to one. Unitless prices are interpreted in reporting currency.

### Algorithm

For each constituent, price its embedded instrument at its native default or read the specified scalar. Convert monetary values to reporting currency using configured FX policy. Sum `price×units/shares_outstanding` for unit-based constituents or `price×weight` otherwise. Subtract `sum×expense_ratio/days_in_year`, then multiply by basket notional amount; the public value path passes shares_outstanding=1.

ConstituentDelta increases a constituent quote by 1% and returns the entire basket PV difference, with zero for absolute price bump≤1e-10. For embedded instruments, it first values the instrument and temporarily substitutes a synthetic bumped price reference. For market-data constituents, it bumps their shared price ID in the market. Components use ticker or constituent ID; total sums component scenario P&Ls.

WeightRisk rejects any explicit-unit constituent. For each target, add 0.0001 to its weight and scale all other weights by `(sum_others−0.0001)/sum_others`, preserving the original net weight budget. Reprice and report currency per 1 bp of weight change. Zero other-weight sum errors; aggregate is the sum of these separate directions.

### Selection, controls and conventions

Default expense day basis is 365.25 and FX policy is CashflowDate at the valuation date. The configured discount curve is declared as a dependency but the NAV formula does not discount basket value. Expense ratio is applied once per valuation, without an elapsed holding period. Basket cashflow export is a placeholder.

### Limitations, failures and alternatives

Basket notional is a multiplicative quantity applied to an already monetary NAV; callers must align scaling and quote units. Validation does not constrain expense_ratio to a finite nonnegative economic range (nonfinite results can fail at Money construction). A market price ID shared by multiple constituents bumps all of them, so separate components can overlap and their sum can double-count shared exposure. Embedded-quote bumps suppress the instrument's own nonlinear factor dynamics. A derivative delta, node-risk aggregation or accrued fee model would be distinct methods.

### Evidence

The [basket tests](../../../../finstack-quant/valuations/tests/instruments/basket/test_basket_comprehensive.rs) explicitly check `constituent_delta_is_pv_change_per_one_percent_move`, financed weight-budget preservation, mixed units/weights, FX and daily fee formulas. These establish the implemented scenario units; no general risk attribution theorem or external NAV reconciliation follows from them.

## Independent numerical checks

- Two rows at the same USD 50 quote have scalars +2 and −.25.
  A third row has mark EUR −20, coefficient .5 and FX 1.10
  USD per EUR. The gross mark is USD 76.50 per basket unit.
  A USD .75 accrued expense per unit gives USD 757.50 for
  ten units. One shared USD quote shock gives USD +8.75
  across the two USD rows.
- In a separate all-coefficient example, coefficients .2
  and .3 share a USD 100 price ID, and coefficient .5 uses
  an embedded USD 200 mark. Ten basket units with USD .75
  accrued expense per unit give USD 1,492.50. Isolated direct
  and embedded mark shocks give USD +5 and +10, and the
  simultaneous shock gives USD +15. The three coefficient
  reallocations preserve the net budget and give signed USD
  P&Ls of `−1/16`, `−1/14` and `1/10` respectively.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/src/instruments/exotics/basket/types.rs)
- [pricer.rs](../../../../finstack-quant/valuations/src/instruments/exotics/basket/pricer.rs)
- [constituent_delta.rs](../../../../finstack-quant/valuations/src/instruments/exotics/basket/metrics/constituent_delta.rs)
- [weight_risk.rs](../../../../finstack-quant/valuations/src/instruments/exotics/basket/metrics/weight_risk.rs)
- [mod.rs](../../../../finstack-quant/valuations/src/instruments/exotics/basket/metrics/mod.rs)
