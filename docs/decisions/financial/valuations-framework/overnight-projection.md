# VALUATIONS-FRAMEWORK-04 — Overnight realized/projected splice and coupon derivatives

- Decision status: **Accepted — complete checked overnight projection and quote risk package, 2026-11-13**.
- Implementation status: **Observed; reconciliation pending** across the overnight engines, product consumers, exports and hosts.
- Validation status: Source, administrator and convention references inspected. Independent bounded examples checked; runtime, dated market and host qualification remain pending.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked overnight projection and quote risk

1. **Require resolved contractual terms and admit methods explicitly.**
   Use a qualified overnight `IndexId`, currency, observation calendar,
   ACT/360 or ACT/365F index basis, and actual ordered accrual endpoints
   `S<E` from the product's schedule. Keep the contractual coupon
   day count, payment date, index value interval and publication
   calendar separate. Require the actual method: plain weighted
   arithmetic, compounded arrears with a nonnegative lookback,
   compounded observation shift, or compounded cutoff. Admit a
   method only where the owner supports those terms; no implied
   mixed variants. Term Simple is a separate operation. Remove
   currency calendar fallbacks and implicit method/basis selection
   from the projector. Exact product or versioned market terms
   may be resolved before calling it and retain their authority.
   Arithmetic is supported for admitted products such as one-month
   SOFR futures; the three-month specification compounds. A swap
   route that does not admit arithmetic reports unsupported terms.
   [One-month CME specification](https://www.cmegroup.com/markets/interest-rates/stirs/one-month-sofr/specs),
   [three-month CME specification](https://www.cmegroup.com/markets/interest-rates/stirs/three-month-sofr/specs),
   [ISDA confirmation templates](https://www.isda.org/a/KcigE/ISDA-2021-Definitions-Consolidated-Confirmation-Templates-COLOUR-CODED.pdf).

2. **Compile actual business-day blocks and keep the full quote interval.**
   Share the [CASHFLOWS-007 block rules](../cashflows/07-overnight-compounding.md#accepted-decision-resolved-overnight-blocks-cumulative-growth-and-parent-allocation)
   for these admitted profiles. Intersect consecutive full
   `[b,nextBusiness(b))` blocks with actual `[S,E)`. Preserve
   the leading preceding-business-day carry for a closed start
   and any trailing partial block. The applicable date selects
   the exact fixing/value date, actual effective and maturity
   dates and positive index fraction `α_i`. Clipping a coupon
   or checkpoint changes only its weight, not the quote interval.
   An ordinary Friday quote may cover Friday to Monday even when
   the coupon uses one day of that block. Fail if required dates,
   calendar coverage or quote geometry cannot be resolved. Check
   offset/date arithmetic, admit at most 100,000 compiled blocks
   per parent and check aggregate request work before allocation.
   Bound the quote cache by the compiled blocks and fail before
   publishing any partial result.
   [New York Fed SOFR methodology](https://www.newyorkfed.org/markets/reference-rates/additional-information-about-reference-rates).

3. **Define the offset and cutoff literally.** Lookback moves
   only the quote by `k` observation business days. Observation
   shift pairs each original full block with its shifted full
   block. Give a clipped overlap of `x` days the exact rational
   weight `x × shifted full days / original full days` before
   conversion to index years. This partial-block rule is an
   explicit project extension for terms that admit it. Zero
   offset is identity. Cutoff `k` counts trailing compiled
   blocks whose quote is replaced, including any partial block
   in the parent block count. Require `0≤k<block_count`.
   Reuse the last uncut quote without merging factor blocks.
   Compile cutoff on the original parent once. Normalize a
   supported externally authored offset using its resolved
   anchor; an end-relative `−2` business-day offset can mean one
   trailing replacement. Reject controls or calendar combinations
   that cannot be mapped to this profile. Offset and cutoff
   counts must fit `0..=i32::MAX` and actual calendar, date and
   work limits. No schema-only 31-day ceiling or whole-coupon
   lockout is admitted.
   [ARRC lookback and shift examples](https://www.newyorkfed.org/medialibrary/microsites/arrc/files/2021/users-guide-to-sofr2021-update.pdf),
   [FpML cutoff definition](https://www.fpml.org/spec/fpml-5-9-6-rec-1/html/reporting/schemaDocumentation/schemas/fpml-ird-5-9_xsd/schema-overview.html).

4. **Select and freeze each distinct quote at the effective close.**
   Under [ADR 0045](../conventions/0045-financial-clocks-and-date-boundaries.md#publication-aware-fixing-eligibility-without-a-universal-cutoff)
   and [CORE-MARKET-12](../core-market/scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings),
   require the exact published annual decimal fixing and admitted
   version when publication makes it required. For an expressly
   unpublished observation, project only if the product and
   information phase permit it. Unknown or inconsistent state,
   missing required history, and failed selected projections
   error with their actual cause. A present row cannot authorize
   a future publication or revision. Use the index's publication
   rules and the frozen snapshot, including any approved
   publication-calendar exception. Retain synthetic observation
   authority only in an explicitly hypothetical scenario under
   [CASHFLOWS-016](../cashflows/16-fixing-materialization.md#accepted-decision-qualified-hypothetical-fixing-materialization).
   Reuse the resolved quote and status at every cutoff reference.
   [New York Fed publication rules](https://www.newyorkfed.org/markets/reference-rates/additional-information-about-reference-rates),
   [Bank of England SONIA policies](https://www.bankofengland.co.uk/markets/sonia-benchmark/sonia-key-features-and-policies).

5. **Project the index rate on its actual period and source clock.**
   Resolve the exact index-to-projection-source association and
   use [CORE-MARKET-02](../core-market/forward-projection.md#accepted-decision-explicit-index-projection-factors-and-contractual-rates)
   for the checked annual rate `expm1(ℓ_i)/α_i`, where `ℓ_i`
   is its stable factor log difference over `S_i,E_i`. Explicit
   product selection may bind one evaluator to projection and discount roles
   where supported; preserve both roles and its actual clock.
   Remove implicit discount substitution, rate-period conversion,
   curve-ID fixing keys and projection eligibility based on
   the curve origin. A permitted unpublished quote can need a
   past value date; the selected evaluator must cover it under
   its own dated query rules. Require only the sources used by
   the qualified observation set. Fully known quotes do not
   require a projection source lookup; an unpaid amount may
   still require a discount source.

6. **Return native growth and annualization; let terms select cash.**
   Write `w_i` for each final day weight in index years and
   `τ=Σw_i>0`. Arithmetic growth is `H=Σr_i w_i`; compound
   growth is `H=Π(1+r_i w_i)−1`. The native annualized
   rate is `H/τ`. Preserve raw quotes and growth separately
   from any product-used calculation. An explicitly supported
   inside spread uses `1+(r_i+s)w_i`; retain the cap/floor's
   inside/outside selection and validate the used factors too.
   An outside spread never changes native growth. Daily index
   bounds, when supported, act on the raw quote before any
   inside spread. Period bounds, gearing, option payoffs and
   balance allocation follow the product's declared ordering
   and accepted cashflow/model owner. Reject conflicting or
   unsupported control combinations. For the direct-growth fixed-notional
   swap-style profile, cash before leg sign is `N(H_used+sA)`
   with contractual coupon fraction `A` and explicit outside
   spread `s`. A product that applies the annualized index rate
   to its own coupon fraction instead uses `N A(H_used/τ+s)`
   for an otherwise linear coupon. Those laws differ when
   `A≠τ`; select by product terms and label any equivalent
   coupon rate by its denominator. If the owner admits `A=0`,
   a direct-growth equivalent annual rate `H_used/A` is unavailable.
   Termination, notional
   changes, payment eligibility, discounting and Money belong
   to the cash and product owners.

7. **Use checked growth arithmetic on every route.** Inherit
   CASHFLOWS-007's stable positive-factor and accumulation rules:
   ordered compensated arithmetic sums, and `log1p`/`expm1`
   or a qualified stable equivalent for compounded increments.
   Admit finite signed rates and strictly positive compounded
   factors and requested gross; do not impose a gross gate on
   an arithmetic sum. Fail on unresolved cancellation, lost
   nonzero results or nonrepresentable required outputs. Apply
   the same price and quote admission rules for price-only and
   risk requests. Remove the
   discount-ratio fast path and its placeholder zeros. Keep
   telescoping as an independent identity for a contiguous run
   of full projected periods on one source with `w_i=α_i`,
   no spread or transforms and no reused or shifted quotes.

8. **Define observation quote derivatives and keep factor identities.**
   Freeze terms, weights and publication state. Shock the annual
   decimal quote of each selected projected observation; a
   parallel shock moves each distinct projected quote once.
   For compound factors `f_i`, let `b_i=w_i` on a shocked
   linear factor and zero otherwise. Initialize `(P,D,J)=(1,0,0)`
   and use simultaneous updates `(P f_i, D f_i+P b_i,
   J f_i+2D b_i)`. The native annualized derivatives are
   `D/τ` and `J/τ`. Calculate requested factor partials as
   `b_i Π_{j≠i} f_j / τ` with checked scaled products, without
   dividing out a rounded small factor. Sum partials for
   repeated references to one quote and retain each original
   factor for owner models. Repeated factors generate second
   derivatives even for one shocked quote. Arithmetic first
   derivative sums the shocked `w_i/τ`; its second is zero.
   Label raw and supported transformed bases separately.
   With daily bounds, a projected quote strictly clamped on the
   used basis has zero local derivative. At an active clamp
   kink the used smooth derivative is unavailable; a constant
   bound interval has zero derivative. Preserve raw risk and
   delegate nonsmooth product risk to the owner. Known or
   hypothetically frozen quotes give exact zero quote risk.
   Unrequested or unavailable sensitivities remain absent.
   Check each requested scalar and row before return. These
   are quote derivatives per unit decimal rate and its square;
   a per-bp first derivative scales by `10⁻⁴`, the second by
   `10⁻⁸`. Curve DV01 and stochastic convexity use their own bases.

9. **Share one qualified state with consumers and exports.**
   Keep required observation identities, quote provenance,
   factor rows and risk rows distinct. Report the latest reused
   value date, applicable determination/publication state and
   payment date separately. A past observation date does not
   prove that a coupon is fixed or give an option's model time.
   Expose the unresolved set and actual intervals to cap/floor
   and rate-future models. A model relying on full factor
   ratios must qualify that condition before using them on
   clipped or reused factors. Export all required identities
   separately from the distinct successful raw projection
   candidates, with index, value date, interval, `α_i`, currency,
   units, source/version, phase and parent association. Reject
   inconsistent same-key candidates; only exact qualified
   duplicates coalesce. Keep hypothetical materialization under
   CASHFLOWS-016. Record actual projection and discount bindings
   for generic repricing. Never classify a row by its derivative
   or silently publish an observed, transformed or synthetic
   value as a projection.

10. **Reconcile one Rust path and preserve product authority.**
    Share the resolved block, raw quote and growth kernel with
    cashflows where terms match. Reconcile IRS, basis and
    cross-currency swaps, TRS, cap/floor, rate futures, credit
    facility consumers, fixing exports, risk/calibration and
    existing instrument and market inputs in Python/WASM.
    Validate complete terms and restored state through the same
    rules, preserve precise errors and update wire contracts,
    declarations, parity and evidence together. Keep the native
    callback/projector and its internal state Rust-only. TRS
    annuity and payment eligibility, option expiry, convexity,
    changing balances, exchange rounding and cash settlement
    remain with their respective owners.

## Observed behavior

### Choice, scope, and rationale

The shared projector builds daily business-calendar observations for OIS, swap, TRS and related coupon consumers. It distinguishes arithmetic averaging, compounded arrears with lookback, observation-shift compounding, and rate cutoff. Each published observation is deterministic and each future observation is curve projected. The separation is necessary for seasoned coupons and forward risk; contractual selection of these conventions still requires agreement.

### Algorithm and conventions

1. Resolve the supplied fixing calendar strictly, or a currency default (USD usny, EUR target2, GBP gblo, JPY jpto, AUD auce, CAD cato, CHF chzh). Unsupported currencies need an explicit calendar. Accrual boundaries can be business-day adjusted independently by the caller.
2. Step from accrual start to the next business day, capped at end. For lookback, move observation endpoints backward by the specified business days but retain unshifted accrual weights. Observation shift moves both observation and weight endpoints. Rate cutoff reuses the observation immediately before the cutoff window for its remaining factors. The final fixing date is the last observation start, with the relevant shift/cutoff applied.
3. An observation strictly before `as_of` requires an exact fixing; one on `as_of` is projected in this routine. A forward curve uses `rate_period(t0,t1) × (t1−t0)/index_accrual`, so the forward curve's day-count axis and index accrual can differ. Projection before the forward curve's base date fails. A discount curve uses `(DF(start)/DF(end)−1)/index_accrual` through its date-based ratio.
4. With factor `f_i=1+(r_i+s_compounded)δ_i`, compute `P=∏f_i` and coupon rate `(P−1)/τ`. Every factor/product must remain positive and finite. The separate arithmetic spread is added as `N(P−1)+N s_arithmetic τ`; it is not compounded by this helper.
5. Let `e_i` be one for projected observations and zero for historical observations. Initialize `(P,D,D2)=(1,0,0)` and update simultaneously: `D2←D2 f_i+2D δ_i e_i`, `D←D f_i+P δ_i e_i`, `P←P f_i`. Output derivatives are `D/τ` and `D2/τ`. Individual forward derivatives use prefix/suffix products, `δ_i e_i ∏_{j≠i}f_j/τ`, avoiding division by a small factor.
6. Arithmetic averaging weights each rate by `δ_i/τ`; sensitivity is the sum of future-observation weights. A nonbusiness start uses the prior business observation for its partial first interval. This separate routine supports arithmetic futures; `SimpleAverage` is rejected by the shared compounded swap-coupon resolver.

### Fast path and controls

For a discount-curve projection with zero compounded spread, zero-lookback arrears and no requested observation exposures, projected products telescope to `DF(start)/DF(end)`. A seasoned coupon first multiplies exact historical factors, then appends the remaining discount ratio. This fast path returns zero derivative fields and an empty observation list; consumers needing derivatives must request exposures and use the explicit observation path. No stochastic-rate convexity adjustment is added here.

The explicit convention takes precedence over registry inference. Unknown registered index IDs yield no convention; known overnight indices reject simple term compounding when the validation helper is invoked. Oversized lookback/shift values fail conversion to signed day counts; the cutoff conversion uses `ok()` and consequently an unrepresentable positive cutoff is treated as absent. That asymmetry is observed, not an endorsed fallback.

A representable cutoff can span the entire coupon: the adapter subtracts cutoff business days from the end and references the preceding business observation without requiring that reference to lie inside accrual. This can freeze every factor to a pre-accrual fixing. The cashflows engine instead rejects cutoff at least as large as its compiled observation count. Valuations also projects over the complete observation-start to next-business-day interval, including weekend/holiday span; cashflows projects one calendar day then carries that rate across its weight. On a nonflat curve those raw observations need not agree even with identical accrual endpoints and no lookback. TRS exports such projected rates by observation date, so combining cross-engine schedules may produce conflicting same-series/date projections; that consequence is a source-based risk, not a reproduced failure.

## Alternatives, evidence

- [overnight.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/overnight.rs): formulas, splice, fast path, derivative recurrence, validation and inline tests.
- [overnight_conventions.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/overnight_conventions.rs): explicit/registry selection and unsupported arithmetic swap coupons.
- [ARRC SOFR guide](../../../REFERENCES.md#arrc-sofr-users-guide), [ISDA 2021](../../../REFERENCES.md#isda-2021-definitions), and [Ametrano–Bianchetti](../../../REFERENCES.md#ametrano-bianchetti-2013) are canonical convention/multicurve context, not evidence that every configured index uses the correct settings.
- [Cap/floor projection](../../../../finstack-quant/valuations/rates/src/cap_floor/pricing/projection.rs), [Hull–White cap/floor adapter](../../../../finstack-quant/valuations/rates/src/cap_floor/hw_pricer.rs), [rate futures](../../../../finstack-quant/valuations/rates/src/ir_future/types.rs) and [IRS projected-fixing export](../../../../finstack-quant/valuations/rates/src/irs/cashflow.rs) show distinct spread, stochastic-time, convexity and export consumers.

### Independent review evidence

- With projection factors Friday `1`, Saturday `.9999` and
  Monday `.9994`, the true three-day quote is `360/4997`.
  A Sunday-to-Monday coupon clip uses `1/360` of that quote,
  giving growth `1/4997`. Projecting a fictitious Friday-to-Saturday
  interval would instead give clipped growth `1/9999`.
- In the ARRC July 2019 example, July 2–5 with a five-business-day
  offset selects June 25 and June 26 quotes of 2.41% and 2.43%.
  Lookback keeps weights `1/360, 2/360` over the July 4 holiday;
  shift uses `1/360, 1/360`. Compounded growth is approximately
  `.00020195348194444` and `.00013444896319444`, respectively.
  Shifted growth divided by its own weight gives a rate of
  `2.4200813375%`; applying that rate to a contractual `3/360`
  instead gives growth-equivalent cash per notional of
  `.00020167344479167`. Separately, shifting an original
  three-day Friday block to a one-day Thursday block gives one
  third of a shifted day for an original one-day clip.
  [ARRC example](https://www.newyorkfed.org/medialibrary/microsites/arrc/files/2021/users-guide-to-sofr2021-update.pdf).
- On a Monday before Friday's publication, projecting 4% for
  Friday and 5% for Monday over respective `3/360,1/360`
  weights gives native rate `.0425041666667`, parallel quote
  derivative `1.0001875` and second derivative `1/240`. After
  Friday's 3% fixing is available on that same Monday, the
  rate becomes `.035003125`, derivative `.2500625` and second
  derivative zero. An eligible-but-missing Friday fixing after
  publication is an error even if the curve could project it.
- For `1/360,3/360,2/360` with one published 1% quote and one
  projected 2% quote reused across the last two factors, native
  rate is `.018334907438271604`. The single projected coordinate
  has first derivative `.8334675956790123` and second derivative
  `.00555570987654321`; exact centered ±1bp stencils recover
  both. Merging the final two factors gives a different rate,
  `.018333796296296296`, and loses the nonzero second derivative.
- Two full contiguous projected blocks with weights `1/360`
  and `3/360` and source factors `1,.998,.997` give compounded
  growth `3/997` and arithmetic growth `1496/497503`. The first
  equals the endpoint ratio less one. The same ratio is not
  the arithmetic result or a valid shortcut for clipped blocks.
  A one-day quote of `1e−16` also illustrates why computing
  `f−1` from the rounded factor loses a representable increment.

Production qualification needs dated published observations and revisions,
full and partial holiday schedules, product cash and spread treatment,
per-quote and curve repricing evidence, and shared runtime and host tests.

## Relationship to the accepted fixing identity and sampling boundary

[CORE-MARKET-12's complete accepted package](../core-market/scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings) selects exact observed
IndexId/series association independently of projection sources. It removes the
core helper's diagnostic-only as_of and leaves publication/phase/version
qualification explicit at this owning adapter under ADR0045. Generic Step/
Linear or right-tail carry never supplies a missing overnight fixing. Reconcile
unknown eligibility versus explicitly permitted projection and preserve causes;
the strict-before-as_of rule above is observed behavior, not a new universal
cutoff. Record12 was accepted on **2026-11-08**, and this record's full
overnight package was accepted on **2026-11-13**. Reconciliation and
qualification remain pending.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
