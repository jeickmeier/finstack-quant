# VALUATIONS-RATES-13-INFLATION-SWAPS: Inflation swap CPI ratios and accrual conventions

- ID: VALUATIONS-RATES-13-INFLATION-SWAPS
- Decision status: **Accepted — complete checked inflation swap cashflows and risk package, 2026-11-14**.
- Implementation status: Runtime and caller reconciliation pending. Observed behavior below describes the current implementation.
- Validation status: Source, published references and independent arithmetic reviewed. Runtime, calibration and host qualification pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked inflation swap cashflows and risk

1. **Select the actual contract and CPI reference policy.** Admit a
   single-currency zero-coupon inflation swap or a periodic YoY
   profile with a positive exact Money notional, exact signed Decimal
   fixed quote, and explicit Pay or Receive for the fixed leg.
   The inflation leg takes the opposite direction. There is no
   principal exchange in these profiles. Require complete authored
   terms or a deliberately selected dated convention. Bind the
   exact CPI/RPI index and series, vintage, currency, seasonal
   and rebasing identity, along with distinct typed CPI and
   nominal discount source roles. Require
   explicit no lag, day lag or month lag and Step or Linear under
   [the shared reference decision](../valuations-framework/inflation-references.md).
   Remove fallback lag and interpolation from the index, curve or
   library defaults. Resolve and retain each contractual CPI
   reference date independently of day-count, model and actual
   payment dates. Use the actual contract to anchor the reference;
   moving a payment does not by itself move an observation.
   [FpML contract examples](https://www.fpml.org/spec/fpml-5-11-7-rec-1/html/confirmation/fpml-5-11-examples.html).

2. **Resolve a positive reference at each endpoint on the knowledge
   cut.** Reuse the shared monthly-anchor resolver without a new
   fallback path. Month lag moves the anchor month but retains
   the original contractual day weight; day lag first shifts the
   date and uses the shifted month and weight. Step and exact zero
   weight need only the first anchor. Each required anchor uses the
   exact eligible actual version or, if expressly unpublished, a
   matching expected release known by cut `K` and strictly after
   `K`, plus its qualified unlagged projected level. The
   valuation date `V` does not advance `K`. Require a finer cut
   when date-level end-of-day resolution cannot establish the
   selected information. Reject missing, overdue or incompatible
   inputs without stale carry, source-anchor substitution or
   projecting a required actual fixing. Retain the status and
   provenance of every contributing anchor. Each numerator and
   denominator must be strictly positive and finite. Require a
   common index basis, including any authorized common scale
   conversion; remove the below-50 warning and reject unqualified
   mixed rebasing or seasonal definitions. Use a stable checked
   log ratio and requested gross or increment on that same basis
   for pricing, par, export and diagnostics. Do not require a
   detached ratio or separately rounded annual quote where only a
   complete cash amount is requested.
   [Observed fixing authority](../core-market/inflation-observations.md),
   [projected CPI levels](../core-market/inflation-projection.md),
   [Henrard on lagged and interpolated references](https://quant.opengamma.io/Inflation-Instruments-OpenGamma.pdf).

3. **Give the contractual initial CPI one role.** An explicitly
   authorized fixed initial reference CPI is an already resolved
   positive level for the contract's first reference, on the matched
   index basis and available at the applicable cut. Do not lag or
   interpolate it again. It replaces only the zero-coupon start or
   first chronological periodic denominator. A rolled portfolio
   must not reuse it for the first remaining later coupon. Keep
   it fixed in risk and scenario calculations. Retain independently
   known anchor provenance. Keep the projection source's origin
   level separate from a contractual base.
   [Strata contractual first index value](https://strata.opengamma.io/apidocs/com/opengamma/strata/product/swap/InflationRateCalculation.html).

4. **Resolve actual dated payment and entitlement once.** Use a
   common actual payment date for zero coupon, and a common date
   per paired periodic coupon. Require explicit payment adjustment
   and lag. A literal unadjusted zero-lag policy needs no calendar;
   an adjustment or business-day lag needs its exact calendar.
   No missing-calendar/weekends fallback or implicit roll of
   reference dates. Require the selected payment timing to admit
   authorized payoff determination. Check actual or qualified
   expected availability of every needed anchor against that
   cutoff. A contract that pays before its needed CPI can be
   determined requires a separate qualified settlement law.
   Keep an amount outstanding between accrual
   maturity and its actual payment; an overdue unsettled amount
   needs qualified treatment. Use a single explicit holder cut
   for raw and finalized value, par and cash reporting. The
   ordinary future-payment cut excludes same-day cash; include
   known payable same-day cash only on an explicit entitled cut,
   with factor one. An unresolved projection cannot stand in for
   authorized included same-day cash. A proved expired or
   extinguished claim needs no inactive pricing lookups.
   [Dated discount factors](../core-conventions/016-discounting.md),
   [cashflow export](../valuations-framework/cashflow-export.md).

5. **Price zero-coupon growth on its elected fixed exponent.** Let
   `ℓ=log(I_end/I_start)` and require a checked positive
   fixed exponent `τ` and `k>−1` for the annually compounded
   fixed rate. Use `N expm1(ℓ)` for inflation cash and
   `N expm1(τ log1p(k))` for fixed cash. Apply sign and the
   actual payment factor once; preserve a small signed net amount
   when gross legs nearly cancel. Elect a qualified contextual
   full-span fixed day count, or a product-owned annual grid
   with short-front prorating. The latter uses nominal annual
   boundaries stepped from the same unadjusted maturity anchor,
   an explicit roll/EOM basis, one unit for each full period and
   actual stub days divided by actual days in its uncut annual
   reference period. Payment adjustment does not change this
   grid. An absent or unsupported schedule is an error.
   Generic `OneOne` remains a single contractual unit for a
   nonempty interval; it does not imply this stub convention.
   Retain the actual elected exponent basis and pieces.
   [Accepted day counts](../core-conventions/001-actual-day-counts.md),
   [CFTC-filed annual zero-coupon convention](https://www.cftc.gov/filings/orgrules/rules05282522457.pdf).

6. **Fold the actual periodic YoY coupons.** Prepare each actual
   accrual, reference pair and payment, with explicit frequency,
   grid, stub, roll/EOM, day-count context and calendar where
   required. Retain the prepared fixed accrual `α_i>0`. Under
   this profile the inflation cash is `N expm1(ℓ_i)` and the
   fixed cash is `N k α_i`. Do not multiply the ratio growth
   by `α_i` again, or infer annual growth from a stub.
   A nonannual period or stub is admitted only when explicitly
   defined as that period's CPI ratio with a matching quote
   convention. A true rolling twelve-month reference or another
   floating-accrual convention is a different law. Apply the
   holder sign and actual dated factor to each paired coupon,
   and sum in checked order. The deterministic ratio on
   projected inputs is a model value; it is not a stochastic
   expectation of a ratio, a timing or convexity adjustment, or
   an independent YoY market curve. Keep those models and
   inflation option expiry with the separate owners.
   [Checked schedule grids](../core-conventions/008-schedule-generation.md),
   [inflation options](14-inflation-options.md) and
   [CFTC-hosted periodic convention example](https://www.cftc.gov/filings/orgrules/rules08212619030.pdf).

7. **Return par only on an identifiable replacement quote.** For
   zero coupon with one supported eligible common payment,
   `k_par=expm1(ℓ/τ)`; common notional, sign and discount
   cancel from the formula. For periodic swaps use
   `Σ_i D_i expm1(ℓ_i)/Σ_i D_i α_i` on the same eligible
   periods, with a checked positive representable coefficient.
   An empty payment set or a zero exponent cannot produce a par
   quote. A small positive annuity does not by itself make the
   quote unavailable. Preserve a genuine par zero and reject a
   returned annual ZC quote that rounds onto the inadmissible
   lower limit. Verify the actual returned quote by replacing
   only the fixed rate and repricing the admitted cash law
   within an explicit absolute native-currency residual budget.
   Keep the replacement quote separate from a trade settlement,
   an annualized observed index diagnostic and generic
   horizon breakeven. [Cashflow and par acceptance](../valuations-framework/cashflow-export.md).

8. **Define separate typed inflation and nominal rate risk.** For
   both swap profiles, use the accepted additive shock to original
   annual gross CPI growth parameters, with the source clock,
   original origin level, interpolation, tails and basis retained.
   Shock only projected anchors; hold actual observations, the
   contractual base, release facts, cuts and other inputs fixed.
   Require explicit positive calculation steps and checked
   represented moves on a common scalar shock amplitude. Reprice
   the same selected raw valuation at matched sides and center.
   For represented decimal-rate displacements `+a,−c` with
   `a,c>0`, report `inflation01=(V+−V−)/(10000(a+c))` in
   native currency per positive bp. Report `inflation_convexity`
   as `2[(V+−V0)/a−(V0−V−)/c]/(a+c)` in native currency per
   squared annual decimal rate. Neither is a percentage of PV.
   Apply checked `dv01`, typed `pv01` and exact triangular
   `bucketed_dv01` on the actual nominal discount role with CPI
   held. CPI remains a distinct axis even for similar source
   IDs. Separate authorized market quote replay and mixed risk
   from direct source shocks. Remove the implicit bump and the
   Money-to-binary64 risk shortcut. A proved fixed unpaid
   coupon has no projected CPI exposure and may retain nominal
   discount risk. Report reprice or step failures as errors; no
   silent linear fallback or missing-source zero.
   [CPI parameter shocks](../core-market/inflation-projection.md),
   [rate finite differences](../valuations-framework/finite-difference-rates.md).

9. **Reconcile cashflow, calibration, attribution and host consumers.**
   Use one prepared holder ledger for price, raw reprices,
   leg PV, par and export. Report each leg PV as if that leg were
   received, before applying the trade side. It can be negative
   when that leg has negative cash. Exported holder rows apply
   the actual trade signs once. Retain both contractual
   CPI references and authority, known or projected status, date
   and payment cut.
   Report CPI growth over its actual period, without a fabricated
   single reset or annualized coupon. Export the selected ZC
   exponent and any composite accrual details as product
   provenance; omit optional single-slice accrual metadata that
   cannot represent it. Keep authorized settlement or netting
   distinct from modeled cash. Make standalone amount and rate
   diagnostics agree with the selected basis and truthfully handle
   absent live claims. Share the prepared references and cash
   conventions with [inflation calibration](../calibration-targets/06-inflation-calibration.md),
   whose target and fit remain separately owned. Use a single
   parallel attribution slope only for a matched common move on
   the actual CPI growth basis; a missing or unmatched factor
   cannot be averaged to zero. Reconcile Rust registry and reports,
   existing Python/WASM envelope routes, declarations, schemas and parity.
   Independently qualify fixing vintages, projection coverage, positive
   endpoint and range errors, rebases, leap and nonannual terms,
   same-day cut and delayed cash, par residual and typed risk.
   [Market-change measurement](../core-market/market-change-measurement.md),
   [other-factor attribution](../attribution/10-metric-other-factors.md),
   [cash and accrual semantics](../core-conventions/015-cashflow-semantics.md).

## Observed behavior

### Choice, scope and rationale

Zero-coupon inflation swaps exchange compounded fixed growth against a terminal CPI ratio; YoY swaps exchange each period's CPI change against simple fixed interest. These are deterministic projected-CPI valuations. This formulation is observed, not an approval of curve/fixing conventions or stochastic inflation modeling. The local product does not estimate inflation convexity from a stochastic nominal/real-rate model.

### Algorithm and controls

1. Resolve lag through the shared inflation resolver. Interpolation is instrument override, then index setting, then the scalar interpolation default. Resolve each CPI reference using the valuation date and publication-aware shared helper; an explicit positive `base_cpi` replaces only the start reference.
2. Zero-coupon fixed amount is `N*((1+k)^tau-1)`; inflation amount is `N*(I_end/I_start-1)`. Require `k > -1`. The normal year fraction uses the instrument day count. `OneOne` instead creates unadjusted annual periods with a short front stub, counts full years, and prorates the first year by actual days against its annual reference period.
3. Adjust maturity only when a calendar is supplied; without one the zero-coupon payment remains unadjusted. Discount with `D(as_of,payment)`. Payments on or before valuation have zero PV. Pay means pay fixed/receive inflation; Receive reverses it.
4. Zero-coupon par rate is `(I_end/I_start)^(1/tau)-1`, returning zero for nonpositive tau. Discounting cancels because both legs share payment date.
5. YoY schedules use the specified frequency, `StubKind::None`, unadjusted accrual boundaries, zero payment lag and weekends-only calendar if omitted. For each unpaid period, inflation cashflow is `N*(I_end/I_start-1)` and fixed cashflow `N*k*alpha`; the inflation ratio is not multiplied by alpha again. Par rate is discounted CPI changes divided by discounted accrual annuity, with zero for an annuity below `1e-15` in absolute value.

### Risk, conventions and failures

Positive CPI is the economic input domain. The zero-coupon start reference is checked finite and positive; YoY period starts are checked positive. Nonpositive finite observed ending CPI can pass the shared resolver and is not consistently rejected before price/flow ratios. Zero-coupon par-rate inversion separately rejects a nonpositive final ratio, so pricing and inverse-rate domain enforcement differ. See [observed CPI inputs](../core-market/inflation-observations.md) and [inflation projection](../core-market/inflation-projection.md). Discount and inflation curves have distinct roles. Rates are decimal; the inflation risk helper passes ±0.01 percentage points to `inflation_shift_pct`, representing ±1 bp in decimal inflation rates. Inflation01 is half the up/down PV difference. Inflation convexity divides the second difference by `0.0001^2`. Zero-coupon inflation01 calls the instrument value path; convexity uses the metric repricer. DV01 and triangular bucketed DV01 use the shared rate sensitivity engine. Missing published fixings are governed by the common resolver, not replaced locally by current CPI.

### Alternatives and tradeoffs

Stochastic inflation/nominal-rate convexity and explicit YoY market curves would model risks not present in deterministic ratio projection; they are not alternative branches here. Alternative stub and payment conventions change economics. Whether the no-calendar difference between zero-coupon and YoY products is intended remains an adoption question.

## Evidence

- [Inflation swap implementations](../../../../finstack-quant/valuations/rates/src/inflation_swap/types.rs): ratio resolution, zero-coupon/YoY pricing, cashflow export and par rates.
- [Inflation finite differences](../../../../finstack-quant/valuations/rates/src/inflation_fd.rs) and [metric registration](../../../../finstack-quant/valuations/rates/src/inflation_swap/metrics/mod.rs): bump units, repricing and selected metrics.
- Current instrument wires: [zero-coupon schema](../../../../finstack-quant/valuations/schemas/instruments/1/rates/inflation_swap.schema.json) and [periodic schema](../../../../finstack-quant/valuations/schemas/instruments/1/rates/yoy_inflation_swap.schema.json).
- [Lag tests](../../../../finstack-quant/valuations/rates/tests/inflation_swap/integration/test_lag_policies.rs), [YoY tests](../../../../finstack-quant/valuations/rates/tests/inflation_swap/integration/test_yoy.rs), [par-rate tests](../../../../finstack-quant/valuations/rates/tests/inflation_swap/metrics/test_par_rate.rs): convention, formula and roundtrip evidence; not a stochastic-model benchmark.
- [Deacon, Derry and Mirfendereski](../../../REFERENCES.md#deacon-derry-mirfendereski-2004) and [Kerkhof](../../../REFERENCES.md#kerkhof-2005): canonical reference entries, not proof that every implemented convention matches a particular market contract.

### Evidence limits

YoY raw cashflow construction includes valuation-date flows (`pay < as_of` exclusion), while valuation excludes them and the instrument declares valuation-date exclusion. No independent stochastic inflation accuracy claim is established.

### Independent review evidence

- An April 15 contractual reference with a three-month linear lag
  and January/February CPI `300,306` resolves to `302.8`.
  A payment rolled to April 16 would instead give `303` if
  incorrectly used as the reference. With contractual start CPI
  `250`, the ratio `1.2112` is invariant when both levels
  scale by `2/5`. Scaling only the denominator gives `3.028`
  and uses a mismatched basis.
- A nominal annual grid from February 29, 2024 to March 1, 2026
  with short front ending March 1, 2024, against the uncut
  year from March 1, 2023, gives `τ=2+1/366`. For notional
  one million, rate `.03`, CPI `250→265` and factor `.96`,
  fixed cash is `60,985.68360556`, inflation cash `60,000`,
  pay-fixed PV `−946.25626133` and par `.02952209300698`.
- With a periodic stub and regular period at fixed accruals
  `.25,1`, CPI `250→252→264`, factors `.99,.96`, fixed rate
  `.03` and notional one million, pay-fixed PV is
  `17,409.28571429`. Par is `18772/422625≈.04441762792073`.
  Multiplying the first ratio's growth by its fixed accrual
  again gives par near `.03949837326235` instead.
- An authorized initial CPI of `250` followed by `275,286`
  gives second-period inflation cash `40,000` per million,
  even after the first period is no longer eligible. Reusing
  the initial level would give `144,000`.
- For a par pay-fixed ZC swap with notional one million, known
  base `250`, two-year projected level `265.225`, fixed rate
  `.03` and factor `.95`, a shock `h` to original annual
  growth gives `V(h)=950,000[(1.03+h)^2−1.03²]`. Central ±1
  bp gives inflation01 `+195.7` per bp and curvature
  `1,900,000` per squared decimal rate. Nominal risk is zero
  for the single payment whose legs net exactly at par.
- With both references projected instead, at source years
  `1,2` and annual gross growth parameters `.7,1.5`, the
  shocked ratio is `(1.5+h)^2/(.7+h)`. This decreases to first
  order in a positive parallel shock: pay-fixed inflation01
  is about `−29.08` per bp on one million notional and factor
  `.95`. Pay-fixed exposure does not imply a universal sign.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
