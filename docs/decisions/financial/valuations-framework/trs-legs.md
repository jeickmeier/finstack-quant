# VALUATIONS-FRAMEWORK-06 — Total-return swap leg lifecycle, financing, and par spread

- Decision status: **Accepted — complete checked TRS legs and par spread package, 2026-11-13**.
- Implementation status: **Observed, with reconciliation pending** across shared legs, both products, reporting and hosts.
- Validation status: Source and primary references inspected; independent bounded cash, timing and spread examples checked. Runtime, product-price and host qualification remain pending.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked TRS legs and par spread

1. **Admit one explicit fixed-notional cash settlement profile.**
   Require one nonnegative currency-bearing fixed notional, explicit
   receive-return or pay-return side, qualified underlying identity,
   matching leg/settlement currencies, actual schedule terms and
   one explicitly bound dated discount source for both legs in
   this profile. Keep discounting separate from financing index
   projection and any underlying carry role. Preserve signed leg
   amounts and validate all terms, including at zero notional.
   Changing or share-reset notionals, principal exchanges, FX or
   quanto features, physical delivery, or joint stochastic funding
   and return dynamics require separate product-owned economics.
   [FpML return-leg terms](https://cdn.fpml.org/spec/fpml-5-13-5-lcwd-1/html/confirmation/schemaDocumentation/schemas/fpml-eq-shared-5-13_xsd/complexTypes/ReturnLeg.html).

2. **Compile dated leg periods once.** Use the accepted
   [CASHFLOWS-001 schedule](../cashflows/01-schedule-periods.md#accepted-decision-contractual-periods-calendars-and-day-count-context)
   to preserve parent identity, raw and actual accrual dates, full
   stub/reference/termination context and actual payment dates.
   Apply the qualified payment adjustment and business-day lag once
   to the contractual payment anchor. Keep return valuation dates,
   financing resets and fixing calendar roles separate. The existing
   shared profile uses the authored common parent grid for regular
   amounts and admits distinct qualified cash component dates.
   Additional independent leg grids require explicit owner support.
   A payment can precede a raw accrual endpoint if the terms establish
   that the amount is determinable by payment. Decline a required
   advance payment with unresolved amount unless a qualified owner
   supplies its actual amount rule. Do not infer a payment date from
   a returned year fraction or re-adjust the compiled overnight
   accrual bounds. Preserve coincident parents as separate entries.
   [FpML interest-leg dates](https://cdn.fpml.org/spec/fpml-5-13-5-lcwd-1/html/confirmation/schemaDocumentation/schemas/fpml-eq-shared-5-13_xsd/complexTypes/InterestLegCalculationPeriodDates.html).

3. **Select outstanding cash by actual payment and entitlement.**
   Freeze the effective valuation date and qualified settlement
   phase. A completed accrual remains eligible until its payment
   is settled or the entitlement ceases. Use an explicit same-day
   selection on the product's cash authority; included current cash
   has factor one and no curve lookup. Exclude known settled cash.
   Past unpaid claims require explicit owner treatment; do not
   assume closure or automatically discount them backwards. Use
   the same payment selection for PV, spread, annuity and export.
   Distinguish economic expiry from final payment. Retain legitimate
   zero residual after validation. [Dated cash and same-day selection](../core-conventions/016-discounting.md#accepted-decision-dated-npv-selection-arithmetic-and-continuous-discounting).

4. **Resolve the return leg on the product's own observation basis.**
   Pass actual dated period, observation and payment identities and
   the frozen information phase to the owner. Its period return is
   a finite signed decimal per unit of fixed notional; cash on the
   admitted payment date is `N×return`. Preserve negative returns.
   Remove the shared initial level of one and context-free signed
   time proxies for lifecycle. An authored initial level may establish
   the first contractual observation where admitted. Use qualified
   exact known reset levels; missing required history errors. A live
   quote may serve a same-day fixing only when the owner explicitly
   qualifies that source and phase. Resolve market projection only for
   eligible unresolved inputs. Separately paid return components keep
   their actual cash dates, or the requested decomposition is
   unavailable. Equity dividends and carry and fixed-income index
   carry assumptions remain with their [equity](../valuations-equity-and-composition/16-equity-trs.md)
   and [fixed-income](../valuations-other-fixed-income/16-fi-index-trs.md) owners.
   [FpML initial and subsequent valuations](https://cdn.fpml.org/spec/fpml-5-13-5-lcwd-1/html/confirmation/schemaDocumentation/schemas/fpml-eq-shared-5-13_xsd/complexTypes/ReturnLegValuation.html).

5. **Use one qualified financing observation path.** Require an
   explicit method in constructors and wire terms: one simple term
   observation or an admitted compounded overnight profile. Plain
   arithmetic averaging is outside this TRS profile. Remove
   constructor inference, omitted-wire defaults, curve-name fixing
   identities and calendar fallbacks. Resolve term fixing date,
   actual index value interval and positive index alpha under
   [CASHFLOWS-006](../cashflows/06-floating-rate-projection.md#accepted-decision-resolved-term-observations-exact-rate-controls-and-strict-failures).
   Use [record 04](overnight-projection.md#accepted-decision-for-checked-overnight-projection-and-quote-risk)
   for full overnight quote periods, weights, lookback, shift and
   cutoff. Require the eligible exact published version; permit a
   projection only on an explicitly eligible unpublished input.
   A known fixing requires no unused projection curve. Preserve
   failures on missing history, unknown phase or failed projection.

6. **Separate native floating growth from spread accrual.** Let
   `A_i` be the nonnegative contractual financing coupon fraction
   with its full date context. For the term path use `H_i=r_i A_i`,
   where the actual index fraction annualizes the observed/projected
   quote `r_i` and governs its positive index gross under CASHFLOWS-006.
   It does not replace `A_i`. For overnight financing use
   record 04's native compounded growth `H_i`. Its observation
   fraction need not equal `A_i`. Use the direct-growth cash profile
   `N(H_i+s A_i)` with signed `s=spread_bp/10000` outside every
   compound factor. Reject a spread law or cash constraint that
   the profile cannot represent. Keep exact authored basis points
   and checked rate and amount arithmetic. Retain valid signed
   cash, including negative financing. Owner-confirmed cash stays
   fixed when the spread is repriced.

7. **Derive annuity and par spread from one prepared basis.** Let
   `R` be return-leg PV, `F_0` the financing intercept and
   `A=ΣD_i N A_i` over the eligible spread-bearing financing rows
   whose spread is replaced. Use each leg's admitted dated factors.
   The intercept includes zero-spread base growth on those rows and
   any eligible confirmed cash held fixed. A known benchmark fixing
   still contributes spread weight when its coupon remains on the
   replaceable contract basis; a confirmed fixed amount does not.
   A zero contractual fraction contributes no spread weight, even
   if its separately qualified overnight growth is nonzero.
   Report `A` in currency per unit decimal spread, with one-bp
   weight `A/10000`. The annuity needs neither return values nor
   floating quotes; resolve only eligibility, basis, weights and
   required factors for an annuity-only request. A value cached or
   seeded by a metric can be reused only on the identical validated
   currency, source, date, eligibility and spread-scope basis.

8. **Quote par as a checked, side-independent replacement spread.**
   On the affine basis `F(s)=F_0+sA`, receive-return PV is
   `R−F(s)` and pay-return PV reverses it. The total replacement
   spread is `10000(R−F_0)/A` bp, not a change from the current
   spread. Do not combine detached Money, notional or annuity from
   a different request. Replace the absolute `1e−12` cutoff:
   exact zero annuity is valid, with par unavailable; a valid
   positive annuity uses checked scaled quotient and reprice.
   Preserve signed fractional bp. Reprice the returned quote on
   the same basis and require the owner's explicit finite positive
   residual tolerance in native currency. Reject unresolved results
   or a failed residual. An affine raw quote does not establish
   par for a nonlinear contractual spread or cash-rounding law.
   Report the selected price basis. Par may be zero when the
   numerator is zero and annuity positive.

9. **Use one checked pricing and cash reporting basis.** Use accepted
   dated NPV, checked native-currency arithmetic and the selected
   [Money boundary](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity).
   Do not treat Money finalized for cash export as an unrounded
   intermediate. Keep per-leg and per-parent cash, status and exact
   source bindings through pricing and export. Apply position sign
   once. Net eligible same-date flows only as a checked cash view
   on matching currency and basis. Report known versus projected
   cash honestly and distinguish a settled empty residual from
   unavailable decomposition. Export required financing observation
   identities separately from distinct successful raw projection
   candidates with full source/phase provenance. Do not use funding
   candidates as synthetic underlying return fixings; roll requests
   need separately qualified hypothetical inputs under
   [CASHFLOWS-016](../cashflows/16-fixing-materialization.md#accepted-decision-qualified-hypothetical-fixing-materialization).

10. **Reconcile both products and the existing public routes.** Use
    the same Rust contract for equity and fixed-income index TRS
    across native and registry prices, annuity and par metrics,
    active dependencies, dated cash exports and the existing Python
    and WASM JSON envelopes. Validate construction and restoration
    with the same terms and propagate checked errors. Keep the
    native callback and prepared state Rust-only. Reconcile the last
    payment boundary with horizon consumers; curve-shock, carry,
    return modeling, index risk proxies and margin remain with their
    existing owners.

## Observed behavior

### Choice, scope, and rationale

`TrsEngine` shares period iteration and discounting across equity and fixed-income TRS. The underlying-specific return model remains in its product crate. The common adapter pays each period's total return and financing at the scheduled, potentially lagged payment date; completed accrual remains outstanding until payment. The same financing projection supplies PV, annuity and par spread so their algebra reconciles.

### Algorithm and conventions

Build accrual periods through the cashflow scheduler using contractual frequency, stub, roll/calendar rules; resolve payment as adjusted accrual end plus business-day lag. Exclude payments on or before `as_of`, including same-day cash. Pass signed start/end year fractions and `initial_level.unwrap_or(1)` to the underlying return model. Reject nonfinite returns; otherwise total-return cash is `N × period_return`, discounted with the total-return leg's date-based discount ratio and accumulated with Neumaier compensation. No clipping of extreme finite returns occurs.

Financing simple rates require exact period-start fixing when start precedes `as_of`. On `as_of`, use an exact published fixing if available, otherwise project `forward.rate_between_dates(start,end,τ)`. Future starts project. For overnight compounding, adjust the accrual endpoints, delegate to [overnight projection](overnight-projection.md), and use its actual accrual fraction. An inverted/empty adjusted overnight interval yields zero rate/accrual and factor one. `SimpleAverage` financing is rejected. The schedule's business-day convention is used when accrual dates are adjusted; otherwise the overnight adapter explicitly uses Modified Following.

Financing cash is `N(P−1)+N s τ`, where simple `P=1+rτ` and `s=spread_bp/10000`. Discount at the financing payment date and accumulate. Exported projected fixings include simple future resets or overnight observations so horizon rolls can hold those rates. The financing annuity is `A=N Σ DF(payment)τ` on exactly the same unpaid periods; `|A|<1e−12` fails. `PV_finance=PV_float+sA`, and par spread is `10000(PV_total_return−PV_float)/A`, independent of swap side. Nonfinite par spread fails. Callers apply receiver/payer sign.

### Controls, limitations, and alternatives

`FinancingLegSpec::new` infers compounding from registered index conventions, falling back to simple on an unknown ID or registry error; later validation rejects simple for a known overnight index. Explicit `with_compounding` overrides construction. Missing calendars fail validation; the internal overnight path recognizes explicit weekends-only and can resolve an empty calendar by currency. The start-level default one is appropriate only where the product model defines that normalization; it is not a historical-level estimate.

Constructor inference is not the serde default: omitted JSON compounding becomes Simple and subsequently fails validation for a known overnight index. Such wire input must explicitly choose the compounded method. The empty-calendar currency fallback is an internal overnight-helper branch; the public engine first constructs its period schedule, which strictly resolves the supplied calendar, so this is not a reachable rescue for an invalid public `TrsScheduleSpec`.

The par-spread helper accepts a `Money` total-return PV but uses its numeric amount with the separately supplied notional; it does not check currency agreement itself. Product-level validation must establish that contract. The financing-annuity Rustdoc says periods ending on/before as-of are excluded, whereas implementation excludes by payment date. The implementation is internally consistent with lagged payments.

Unimplemented alternatives here include payment-date cash inclusion, compounding the financing spread, and pathwise stochastic financing/underlying correlation. None is silently selected by this helper.

## Evidence

- [trs.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/trs.rs): complete engine after the test-only helper, financing projection, PV, annuity and par spread; inline tests are formula/reconciliation evidence, not an independent market benchmark.
- [trs_common.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/parameters/trs_common.rs) and [legs.rs](../../../../finstack-quant/valuations/core/src/instruments/legs.rs): schedule and compounding/default validation.
- [Hull](../../../REFERENCES.md#hull-options-futures) supplies general carry/discounting context. A product-specific source supporting each TRS fixing and payment convention is not established here.
- [Equity TRS pricing](../../../../finstack-quant/valuations/equity/src/equity_trs/pricer.rs) and [fixed-income index TRS pricing](../../../../finstack-quant/valuations/fixed-income/src/fi_trs/pricer.rs) are the live return and export adapters. Both use the financing discount source for the return leg today. [ISDA's disclosure annex](https://www.isda.org/a/AVEDE/equity-derivatives-risk-disclosure-statement-702595828-2.pdf) and [FpML's dated example](https://www.fpml.org/spec/fpml-4-9-5-rec-1/html/fpml-4-9-examples.html) give contract context for signed returns, financing and settlement. Their broader product variants do not define the shared profile.

### Independent review evidence

- With a Friday accrual end and a two-business-day lag on an
  explicit weekends-only calendar, payment is Tuesday. On Monday,
  a completed `1.8%` return and known term financing of `4%`
  for `.25` years, on one million notional at a `.99` factor,
  give return PV `17,820`, base financing PV `9,900` and annuity
  `247,500`. At `+50 bp`, receive-return PV is `6,682.50` and
  replacement par is `320 bp`. On Tuesday, qualified unpaid cash
  has net value `6,750` at factor one. After settlement the
  residual and annuity are zero, and no par quote exists.
- For index factors `1,.99` and index fraction `.25`, the
  projected term quote is `4/99`; with contractual fraction `.20`,
  coupon growth is `4/495`, rather than the full index growth `1/99`.
  For an overnight shift with two `1/360` weights and quotes `.03,.04`,
  native growth is `21001/108000000`. Contractual spread accrual
  `3/360` and spread `.002` give cash `211.12037037` per million.
  Reusing the observation fraction `2/360` for spread gives
  `205.56481481` instead.
- For two common payment dates with factors `.99,.95`, coupon
  fractions `.25,.25`, financing growth `.01,.01125` and signed
  returns `.02,−.005` on one million notional, `R=15,050`,
  `F_0=20,587.50` and `A=485,000`. At `+25 bp`, financing PV
  is `21,800`; receive/pay PVs are `−6,750,+6,750`. If both
  outstanding spreads are replaceable, par is `−11075/97 bp`,
  about `−114.17525773`. Freezing the first confirmed financing
  cash at `10,625` changes the intercept to `21,206.25`, annuity
  to `237,500` and par to `−4925/19 bp`, about `−259.21052632`.
  Each quote zeros the value on its own basis.
- A positive annuity of `1e−14` and PV difference of `2e−16`
  give `200 bp`. A currency-independent absolute cash threshold
  would reject this exact ratio. Separately, a completed return
  from `100` to `95` on `100,000` notional gives `−4,900` PV at
  a `.98` payment factor. A separate dividend cash of `2,000`
  at `.995` brings PV to `−2,910`; dating it with the return
  payment gives `−2,940` instead.

Qualification still requires dated product confirmations and source
versions, term and shifted-overnight spread cash, same-day payment
state, separate component settlement, consistent cashflow/price
reconciliation and existing host-route evidence.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
