# VALUATIONS-EQUITY-AND-COMPOSITION-20: Property appraisal, DCF, capitalization and unlevered returns

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-20`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

An explicit appraisal overrides modeled property PV. Otherwise the selected method is dated unlevered DCF at a scalar discount rate or direct capitalization of stabilized/first future NOI. Return calculations use purchase outlay and projected sale proceeds, which are separate from the valuation method.

## Scope and input domain

Finite strictly increasing NOI/CapEx dates; CapEx and transaction costs are positive outflow magnitudes. DCF discount rate must exceed -100%; cap rates are positive; terminal growth is in [-100%,20%], disposition percentage in [0,1). Appraisal, purchase/sale amounts and costs must use property currency when consumed.

## Algorithm

For DCF, horizon is explicit sale date or last NOI on/after as-of. Retain flows on/after as-of through horizon, subtract CapEx and merge equal dates. Terminal proceeds prefer explicit sale price; otherwise use `last_NOI_at_or_before_horizon×(1+terminal_growth)/terminal_cap_rate`. Apply percentage disposition costs then subtract cost line items. PV is `Σ flow/(1+r)^t + terminal/(1+r)^T − acquisition_costs`, using the asset day count. Purchase price is excluded from asset PV.

DirectCap returns `stabilized_NOI/cap_rate`, falling back to the first future individual NOI amount. It does not subtract acquisition/CapEx/disposition costs. Appraisal returns its supplied value before model calculation.

Unlevered return flows begin at as-of with negative purchase price plus acquisition costs, then net NOI/CapEx and terminal sale. XIRR uses property day count; multiple is positive dated net cashflows divided by absolute negative dated net cashflows. First cash-on-cash uses the first merged net operating flow over purchase plus acquisition costs. Going-in cap is first future NOI divided by purchase price or base PV; ExitCapRate returns the supplied terminal cap. CapRate01 centrally bumps direct cap or terminal cap by configured rate bp and reports currency per bp; explicit DCF sale price makes that sensitivity inapplicable.

## Selection, controls and conventions

No market curve dependency: modeled discount rate is an instrument scalar. DiscountRate01 shifts only the scalar discount component; direct cap and appraisal are insensitive. Flows on as-of are included undiscounted. Explicit sale truncates subsequent flows; future NOI must still exist even for a terminal-only effective horizon. Cashflow export includes net operating and terminal flows, not purchase/acquisition outlay.

## Limitations, failures and alternatives

Direct-cap and terminal-cap formulas use individual NOI entries as annual amounts without an annualization/frequency guard. Monthly/quarterly schedules can therefore produce materially different implied values. First-period cash-on-cash is not automatically annualized. An appraisal bypasses model assumptions but not every construction invariant. Annual NOI normalization, separate acquisition NAV treatment and explicit terminal stabilized NOI are alternatives requiring adoption.

## Evidence

The [property tests](../../../../finstack-quant/valuations/tests/instruments/equity/real_estate/pricing.rs) contain manual DCF/direct-cap, terminal growth, sale truncation, transaction-cost, appraisal and cap-rate derivative cases. [DCF context](../../../REFERENCES.md#koller-valuation) is general valuation context; a property-specific adopted capitalization/NOI standard has not been established.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/src/instruments/equity/real_estate/types.rs)
- [pricer.rs](../../../../finstack-quant/valuations/src/instruments/equity/real_estate/pricer.rs)
- [cap_rates.rs](../../../../finstack-quant/valuations/src/instruments/equity/real_estate/metrics/cap_rates.rs)
- [returns.rs](../../../../finstack-quant/valuations/src/instruments/equity/real_estate/metrics/returns.rs)
- [sensitivities.rs](../../../../finstack-quant/valuations/src/instruments/equity/real_estate/metrics/sensitivities.rs)
- [mod.rs](../../../../finstack-quant/valuations/src/instruments/equity/real_estate/metrics/mod.rs)
