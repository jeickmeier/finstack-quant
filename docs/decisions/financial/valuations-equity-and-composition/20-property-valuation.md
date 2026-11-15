# VALUATIONS-EQUITY-AND-COMPOSITION-20: Property appraisal, DCF, capitalization and unlevered returns

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-20`.
- **Decision status:** Accepted 2026-11-14 — complete property appraisal, valuation, unlevered return and rate risk package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent capitalization, DCF, investment return and rate-risk checks completed. Production pricing, appraisal qualification and host validation remain pending.

## Accepted decision for checked property valuation and unlevered returns

1. **Select appraisal, direct capitalization or DCF explicitly.**
   A selected appraisal must match the property interest, native
   currency, effective date and valuation premise. Any date
   adjustment requires a qualified basis. Resolve the active
   method's inputs. Keep purchase price and buyer acquisition
   costs in the investor-return ledger. Use the selected
   unlevered property basis for standalone asset value across
   all three methods.
   [RICS property DCF guidance](https://www.rics.org/profession-standards/rics-standards-and-guidance/sector-standards/valuation-standards/discounted-cash-flow-valuation).

2. **Require qualified annual income for capitalization.** Direct
   cap divides positive stabilized annual NOI by a positive
   direct cap rate. Terminal cap uses separately qualified
   stabilized annual NOI for the first 12 months after ownership
   exit and a matched positive terminal cap rate. An annual
   amount can be authored or prepared from a complete qualified
   annual schedule. Keep interim NOI and CapEx on their actual
   cash basis and dates. Remove first- and last-row NOI fallbacks
   and implicit growth in the terminal calculation.
   [Federal Reserve CRE capitalization guidance](https://www.federalreserve.gov/supervisionreg/srletters/SR2305.pdf#page=38).

3. **Discount the owner-entitled operating and exit cash.** Use
   an explicit ownership horizon, authored discount clock and
   finite annual effective discount rate greater than −100%.
   Include qualified NOI less CapEx and eligible unpaid same-day
   cash at factor one. Elect a qualified gross sale amount or
   terminal capitalization. Apply an elected disposition
   percentage to gross proceeds and deduct separately identified
   seller costs once, using actual dates for proceeds and costs.
   Preserve an explicitly retained receivable or obligation with
   settlement after ownership transfer. A qualified terminal-only
   DCF is available. Keep payment, entitlement and ownership
   dates explicit and preserve the signed net amount.
   [Accepted dated discounting](../core-conventions/016-discounting.md).

4. **Build unlevered returns on the actual acquisition basis.**
   Use the authored acquisition date and positive purchase
   consideration, with acquisition costs on their qualified cash
   dates. Retain settled operating history and admitted remaining
   projections through exit. Include net sale proceeds once on
   their actual payment basis. Use canonical ACT/365F XIRR, with
   the accepted equal-growth ACT/360 quote when elected. The
   unlevered multiple divides positive dated net cash by absolute
   negative dated net cash, including later outlays. First-period
   cash-on-cash is the first complete authored operating period's
   NOI less CapEx over purchase plus acquisition costs, in that
   period's units. Going-in cap uses qualified annual acquisition
   NOI over positive purchase price. Report the exit cap only
   on the selected terminal-cap basis.
   [NCREIF/PREA property-level IRR guidance](https://reportingstandards.info/wp-content/uploads/2022/04/rs-handbook-vol-ii-manuals-performance-and-risk.pdf#page=57),
   [accepted XIRR](../core-conventions/017-irr.md).

5. **Reprice active rates and use the selected export basis.**
   `real_estate::cap_rate01` shocks the active direct or terminal
   capitalization rate. `real_estate::discount_rate01` shocks the
   active DCF scalar. Require an explicit positive centered step
   and checked represented endpoints. Report signed currency
   per positive bp from the same raw selected value, holding
   cash, annual NOI and remaining rates and cost assumptions
   fixed. Inactive axes are unavailable. Export dated amounts
   with their operating, disposition or acquisition roles and
   observed or projected status. A separately scoped forecast
   can accompany appraisal or direct capitalization. A complete
   valuation export must reconcile to the selected price.
   Preserve these bases through the existing Rust, Python, WASM
   and schema routes. Record 21 owns financing and levered equity.
   [Accepted finite differences](../valuations-framework/finite-difference-rates.md),
   [accepted cashflow export](../valuations-framework/cashflow-export.md),
   [levered property](21-levered-property.md).

## Observed behavior

### Choice and rationale

An explicit appraisal overrides modeled property PV. Otherwise the selected method is dated unlevered DCF at a scalar discount rate or direct capitalization of stabilized/first future NOI. Return calculations use purchase outlay and projected sale proceeds, which are separate from the valuation method.

### Scope and input domain

Finite strictly increasing NOI/CapEx dates; CapEx and transaction costs are positive outflow magnitudes. DCF discount rate must exceed -100%; cap rates are positive; terminal growth is in [-100%,20%], disposition percentage in [0,1). Appraisal, purchase/sale amounts and costs must use property currency when consumed.

### Algorithm

For DCF, horizon is explicit sale date or last NOI on/after as-of. Retain flows on/after as-of through horizon, subtract CapEx and merge equal dates. Terminal proceeds prefer explicit sale price; otherwise use `last_NOI_at_or_before_horizon×(1+terminal_growth)/terminal_cap_rate`. Apply percentage disposition costs then subtract cost line items. PV is `Σ flow/(1+r)^t + terminal/(1+r)^T − acquisition_costs`, using the asset day count. Purchase price is excluded from asset PV.

DirectCap returns `stabilized_NOI/cap_rate`, falling back to the first future individual NOI amount. It does not subtract acquisition/CapEx/disposition costs. Appraisal returns its supplied value before model calculation.

Unlevered return flows begin at as-of with negative purchase price plus acquisition costs, then net NOI/CapEx and terminal sale. XIRR uses property day count; multiple is positive dated net cashflows divided by absolute negative dated net cashflows. First cash-on-cash uses the first merged net operating flow over purchase plus acquisition costs. Going-in cap is first future NOI divided by purchase price or base PV; ExitCapRate returns the supplied terminal cap. CapRate01 centrally bumps direct cap or terminal cap by configured rate bp and reports currency per bp; explicit DCF sale price makes that sensitivity inapplicable.

### Selection, controls and conventions

No market curve dependency: modeled discount rate is an instrument scalar. DiscountRate01 shifts only the scalar discount component; direct cap and appraisal are insensitive. Flows on as-of are included undiscounted. Explicit sale truncates subsequent flows; future NOI must still exist even for a terminal-only effective horizon. Cashflow export includes net operating and terminal flows, not purchase/acquisition outlay.

### Limitations, failures and alternatives

Direct-cap and terminal-cap formulas use individual NOI entries as annual amounts without an annualization/frequency guard. Monthly/quarterly schedules can therefore produce materially different implied values. First-period cash-on-cash is not automatically annualized. An appraisal bypasses model assumptions but not every construction invariant. Annual NOI normalization, separate acquisition NAV treatment and explicit terminal stabilized NOI are alternatives requiring adoption.

### Evidence

The [property tests](../../../../finstack-quant/valuations/tests/instruments/equity/real_estate/pricing.rs) contain manual DCF/direct-cap, terminal growth, sale truncation, transaction-cost, appraisal and cap-rate derivative cases. [DCF context](../../../REFERENCES.md#koller-valuation) is general valuation context; a property-specific adopted capitalization/NOI standard has not been established.

## Independent numerical checks

- Four qualified quarterly NOI amounts of 20, 22, 23 and 25
  give annual NOI 90 and direct-cap value 1,200 at .075.
  Using only the first quarter gives 266.6666666667.
- A separate DCF uses NOI 80 and 96, CapEx 20 and 10,
  over two years, and post-exit annual NOI 105 at terminal cap
  .075. Gross sale is 1,400 and net sale is 1,360 after 2%
  and fixed seller costs of 12. At discount .10, standalone PV
  is 1,249.586776859504 and centered terminal cap sensitivity is
  −1.511848417758 per positive bp.
- A separate dated investor ledger with purchase 1,000 and
  acquisition costs 20 gives signed net cash
  `[−1020,−5,20,75,1122]` on 2025-01-01, 2025-02-01,
  2025-04-01, 2025-12-31 and 2026-01-01.
  Its XIRR is .190172048733 and multiple is 1.187317073171.
  The first complete operating period gives cash-on-cash
  `15/1020`, while its first individual operating payment is −5.

Production pricing, appraisal qualification, rate risk and host
validation remain pending.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/src/instruments/equity/real_estate/types.rs)
- [pricer.rs](../../../../finstack-quant/valuations/src/instruments/equity/real_estate/pricer.rs)
- [cap_rates.rs](../../../../finstack-quant/valuations/src/instruments/equity/real_estate/metrics/cap_rates.rs)
- [returns.rs](../../../../finstack-quant/valuations/src/instruments/equity/real_estate/metrics/returns.rs)
- [sensitivities.rs](../../../../finstack-quant/valuations/src/instruments/equity/real_estate/metrics/sensitivities.rs)
- [mod.rs](../../../../finstack-quant/valuations/src/instruments/equity/real_estate/metrics/mod.rs)
