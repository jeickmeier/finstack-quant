# VALUATIONS-FRAMEWORK-15 — Schedule PV, term fixing coordinates and reconciled cashflow export

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in shared cashflow adapters; projection, balance replay and credit integration are delegated to core/cashflows.
- Validation status: Evidence inspected/located, not executed; no external accuracy comparison performed.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Schedule, clock and fixing algorithm

Ordinary schedule PV delegates dated flows to core NPV on the discount curve's own day count, excluding payment dates on/before valuation. An empty schedule returns zero only when the provider supplies a notional and therefore a currency; otherwise it errors. The raw variant extracts amounts and avoids final Money rounding. The distinct calibration/trade raw helper includes same-day initial exchanges at unit DF, excludes only earlier flows, and uses compensated summation. It still requires the discount curve for an empty schedule.

Term-index validation compares the curve's tenor with the contractual tenor in years, tolerance `1e−8`. A projected term fixing converts its reset date to signed time from the **curve base date** using the curve day count, then reads `curve.rate(t)`. This helper performs no historical-series lookup itself. For ACT/ACT ICMA, the first supplied payment date strictly after valuation anchors a quasi-coupon period one contractual month/year frequency earlier, preserving end-of-month. Unsupported day/week frequencies, no future payment, date arithmetic failure or non-ICMA day count return no inferred period; the downstream frequency-only day-count path can then fail. Input dates are assumed ordered; the helper takes the first matching date, not the minimum.

Payment delay adds strictly positive business days through an explicitly named strict calendar, or weekdays when the calendar is absent. Nonpositive delay returns the original date without resolving a supplied calendar. The shared par-annuity threshold is `1e−12`.

## Export algorithm and conventions

1. Accept only a registered Discounting or HazardRate pair that passes the instrument's cashflow-export eligibility hook. Price once through that selected registry and use its effective date and scenario-adjusted Money PV as reconciliation target. The direct exporter passes `mark_instrument_validated` after the export hook, relying on the documented already-validated-instrument precondition; it does not independently invoke generic `validate_for_pricing` here.
2. Build one schedule and its optional pool/index/balance metadata. Require a declared discount curve even if terms also specify an override. Select per-currency discount curves where supplied; otherwise use the selected default. Hazard mode selects the first declared credit curve and its recovery metadata.
3. Compute date-based relative DFs. Retain past/same-day rows as audit history with DF one and native PV zero. Delegate actual native credit-adjusted row PV to the cashflows kernel. For surviving rows, display conditional survival `Q(T)=clamp(S(T)/S(as_of),0,1)` and nonnegative sequential probability differences `max(Q_previous−Q(T),0)`. Zero/nonfinite survival at valuation fails. These columns follow schedule order; duplicate payment dates have zero subsequent displayed increment, while the delegated kernel owns allocation of default recovery.
4. Convert each native PV through Money into reporting currency using **valuation-date FX**, then apply scenario price shock. Default reporting currency is the first flow's currency; explicit terms can provide it for an empty schedule. Per-flow conversion can round Money independently. Sum row PV with Neumaier compensation and require currency equality plus `|sum−canonical|≤max(1e−6,|canonical|×1e−6)`. Otherwise fail; the exporter never labels a nonreconciling envelope successful.
5. Enrich pool state by payment date, schedule balances by flow position, accrual notional by schedule balance at accrual start, and index ratios by flow date. Pool balances take precedence over schedule balances. Accrual-start balance lookup converts **any** replay error into absent metadata. Missing index-ratio entries remain absent; the adjacent comment promises logging but this lookup contains no warning. These missing fields do not independently invalidate a reconciling PV.
6. Calendar-year ladders group by row currency in first-appearance order and aggregate unshocked native PV, preserving its unit with native amounts. They do not sum reporting-currency/scenario-adjusted PV into native ladders. Pricing explanations use the same export; an unavailable trace becomes a computation-step reason instead of failing the already obtained price.

## Limitations, alternatives

The export is a static decomposition; tree/MC/PDE/exercise value cannot generally be represented by these rows. Reconciliation is a production-kernel consistency check, not independent price validation, and the relative tolerance is one part per million rather than a universal minor-currency-unit guarantee. Possible stricter metadata failures, cashflow-date FX, or model-specific stochastic attribution require separate decisions. Generic raw PV excludes currency checking inherent in the Money path and requires consistent single-currency provider economics.

## Evidence

[cashflow_export.rs](../../../../finstack-quant/valuations/core/src/instruments/cashflow_export.rs), [helpers.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/helpers.rs), [term_fixings.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/term_fixings.rs), [coupon_period.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/coupon_period.rs), [swap_legs.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/swap_legs.rs), and [instrument trait](../../../../finstack-quant/valuations/core/src/instruments/common_impl/traits/instrument.rs) own the inspected behavior. Inline tests cover ladder currency grouping, compensated sums and calendar failures; product export tests supply reconciliation evidence. No test or external market comparison was executed. Underlying credit integration is covered by the [cashflows credit-PV record](../cashflows/15-credit-adjusted-pv.md).

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
