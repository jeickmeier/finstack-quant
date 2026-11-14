# VALUATIONS-OTHER-FIXED-INCOME-04: Inflation-linked cashflows, publication state and deflation floors

- **Decision status:** Accepted 2026-11-14 — full checked inflation-linked bond indexation and floors package.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Reconciliation pending. The observed entry points and kernels are described below.
- **Validation status:** Live source and primary issue rules inspected. Independent index, cash and option arithmetic checked. Runtime, issue, market and host qualification pending.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Accepted decision for checked inflation-linked bond indexation and floors

1. **Bind one issue profile and its original reference basis.**
   Require explicit security and currency identity, exact CPI or RPI
   series and seasonal/rebase basis, contractual fixing selection,
   actual dated base level and its original issue provenance, and
   eligible real face. Select the issue's authenticated rule edition
   or an explicitly authored analytical contract. Keep a reopening
   on the original contractual denominator. Require a positive
   finite base reference level on the same effective basis as the
   numerator, with any official link applied under the issue rule.
   Reject a country label, assumed standard lag, latest print or
   curve origin as a source of terms. For an authored analytical
   contract require explicit lag, first-of-month Step or Linear rule,
   denominator, rounding, date and floor terms. A named issue with
   unsupported or unverifiable terms is unavailable.
   [Accepted dated convention selection](../valuations-framework/convention-selection.md),
   [exact monthly fixing identity](../core-market/inflation-observations.md#accepted-decision-exact-monthly-fixing-snapshots-and-explicit-availability).

2. **Use exact security-specific reference geometry and rounding.**
   First-of-month three-month Linear uses CPI for months `M−3`
   and `M−2` with weight `(day−1)/days_in_M`. At exact zero
   weight use only the first anchor. For authored calendar-day lags,
   shift the date first, then use its calendar cell. Equal zero
   shifts imply equal results. Adopt these profiles within their
   authenticated terms and cash conventions.

   | Issue profile | Reference, base and applicable rounding | Protection |
   |---|---|---|
   | US TIPS | NSA CPI-U, three-month first-of-month Linear. Base from the original dated date where it differs from original issue. Use the contract-frozen CPI selection and Appendix B truncation to six then rounding to five decimals for reference and ratio. | Maturity principal |
   | Canada RRB | Canada all-items CPI on the required contractual basis, three-month first-of-month Linear. Original issue reference is the base. Carry reference and ratio to six decimals and round five with upward ties under the legal terms. | None. Negative inflation compensation is payable as a reduction of principal. |
   | UK three-month gilt | RPI, three-month first-of-month Linear. First-issue reference is the base. Round reference and ratio to five decimals. Apply the prospectus per-100 cash rounding separately, including the standard six-decimal DMO profile. | None for the admitted standard profile |
   | UK eight-month gilt | Use the one RPI for the month eight months before each relevant contractual date. Base uses the corresponding original-issue month. Apply the prospectus's per-100 coupon/redemption rounding, including directed truncation for applicable issues. | None for the admitted standard profile |
   | French OATi and OAT€i | Distinct French ex-tobacco CPI and euro-area ex-tobacco HICP with the specified versions. Three-month first-of-month Linear. Use the reference at the first issue’s contractual dated date and the issue decree's reference and coefficient rounding, including six-decimal truncation then five-decimal rounding where prescribed. | Maturity principal |
   | Japan JGBi | Japan CPI excluding fresh food. The tenth of month `M` uses CPI for `M−3`. Interpolate by actual days between adjacent tenths; dates before the tenth are in the prior cell. Base is the original issue's applicable tenth, including the short-first-period exception. Use the issue's rounding, including reference to three decimals and coefficient to three for the older through-March-2016 profile or five thereafter. | Maturity principal for the qualified 2013-onward guarantee. Unprotected for earlier terms. |

   Apply the named calculation and rounding in the prescribed
   sequence, including authorized per-denomination cash rounding
   before scaling a position when the issue calls for it. Keep
   original contractual CPI date, adjusted cash date and original
   base distinct. Date coincidences come from the issue terms.
   For example, use the qualifying French issue’s coupon detachment
   date for its CPI reference. Preserve a TIPS final coupon’s
   maturity index basis through a nonbusiness-day payment roll.
   [US payment rules and Appendix B](https://www.govinfo.gov/content/pkg/CFR-2025-title31-vol2/pdf/CFR-2025-title31-vol2-part356.pdf),
   [US Appendix B](https://www.govinfo.gov/content/pkg/CFR-2025-title31-vol2/pdf/CFR-2025-title31-vol2-part356-appB.pdf),
   [Canada legal terms](https://www.canada.ca/en/department-finance/programs/financial-sector-policy/securities/legal-terms-conditions-domestic-debt-securities/real-return-bonds.html),
   [UK DMO cash rules and worked examples](https://www.dmo.gov.uk/media/0ltegugd/igcalc.pdf),
   [French OATi decree](https://www.legifrance.gouv.fr/jorf/id/JORFTEXT000047664410),
   [French OAT€i decree](https://www.legifrance.gouv.fr/jorf/id/JORFTEXT000047858104),
   [Japan MOF reference and rebase rules](https://www.mof.go.jp/english/policy/jgbs/topics/bond/10year_inflation/coefficient.htm),
   [Japan issue design and rounding](https://www.mof.go.jp/jgbs/topics/bond/10year_inflation-indexed/syouhinsekkei.htm),
   [Japan maturity guarantee](https://www.mof.go.jp/english/policy/jgbs/topics/bond/10year_inflation/index.htm).

3. **Resolve each needed monthly anchor at the stated knowledge
   cut.** Reuse accepted exact observation and release selection.
   A selected actual fixing eligible at the end-of-day cutoff `K`
   wins. A required anchor missing from that snapshot may project
   only when its matching expected release was known by `K` and
   is strictly later than `K`. Missing release facts and overdue
   absent prints error. Resolve one-or-two-anchor contributions
   separately; a mixed result remains projected until every needed
   fixing is contractually known. Bind the matching unlagged
   projector explicitly, with its admitted origin, clock and tails.
   Known-only rows need no projection. Projected rows still need
   the matching knowledge and release facts. Remove index-only or
   curve-only shortcuts that bypass the common contract, double
   lag, source-lag and interpolation inference and expiry fallback
   to a reference date. Advancing valuation alone changes neither
   knowledge nor the selected version.

   Validate actual or qualified expected availability against the
   contractual determination date and phase for the payment. A
   known substitute, discontinued series or rebase requires the
   authorized issue-specific rule and recognized data, including
   effective date, linkage, affected period and applicable phase.
   Apply a published substitute under its own contractual authority
   and record it distinctly from a statistical print or a projection.
   Missing or late data cannot invent a substitute or move payment.
   An intraday determination requires adequate finer-grained facts.
   [Accepted inflation reference and publication rules](../valuations-framework/inflation-references.md#accepted-decision-for-checked-inflation-references-and-information-dates),
   [Treasury's October 2025 contingency announcement](https://www.treasurydirect.gov/instit/annceresult/press/preanre/2025/SPL_20251126_1.pdf).

4. **Prepare original coupon parents and real par once.** Use
   the real fixed-coupon bullet schedule with explicit accrual
   rules, nominal reference periods and day-count context, calendar,
   stub/roll and payment conventions. Keep unadjusted accrual
   boundaries, CPI reference, amount determination, payment,
   entitlement and principal retirement on their actual dates and
   phases. Use the qualified whole real coupon, ordinarily
   `N_real × c × α` with annual decimal real coupon `c` and
   original-parent accrual fraction `α`. Reject invalid fractions
   or principal state. Apply the indexation multiplier once.
   Keep coupon and redemption distinct even on the same payment
   date. The original real par retires on its contractual balance
   date. The nominal redemption cash is its independently indexed
   and possibly protected amount. Neither inflation compensation
   nor protection changes the real principal delta. Preserve the
   same schedule and parent context for the unindexed real view.
   [Period and accrual context](../cashflows/01-schedule-periods.md),
   [settlement cash and principal semantics](../core-conventions/015-cashflow-semantics.md).

5. **Apply protection only to its authorized cash.** Elect no
   protection, maturity-principal protection, or an expressly authored
   all-payments multiplier floor on nonnegative eligible real cash.
   Require agreement with the named issue profile. For a wholly
   known payment use its contractual factor `R` after applicable
   reference and ratio rounding, or `max(R,1)` on the protected
   component, then the applicable cash rounding. A protected final
   principal excludes its co-dated coupon unless the contract
   separately protects it. For a future uncertain payment, use
   the exact weighted anchor means and positive fixed denominator
   to form the unrounded forward multiplier `F`. Require each
   projected anchor to be qualified as its CPI expectation under
   the actual nominal payment-date measure. A deterministic
   nominal-rate approximation is admissible only when expressly
   elected and qualified on that payment basis. The live model
   values unrounded indexation and protection where the exact
   legal payoff rounds the index or cash. Require an explicit
   approximation qualification for that difference. Preserve the
   specified legal rounding for known cash. A zero real cash
   component admits zero after contract and entitlement validation
   and introduces no option dependency.
   [Treasury principal-only protection](https://www.treasurydirect.gov/marketable-securities/tips/),
   [related payment-measure qualification](../valuations-rates/14-inflation-options.md#accepted-decision-for-checked-inflation-option-cashflows-expiry-and-risk).

6. **Use one explicit Black floor approximation for unresolved
   protection.** Require the selected gross contractual
   reference/base ratio Black quote at strike `1` in ratio units.
   Bind the exact series, fixed denominator, reference geometry,
   contributing weights, observed levels and unresolved state,
   actual payment date and payment measure. The quote must be
   qualified for that exact state, including a mixed observed and
   projected reference. An annual inflation-growth quote, a single
   monthly-anchor quote, or a quote for the fully unfixed blend
   cannot fill that role. The aggregate lognormal approximation
   does not model separate anchor distributions, their dependence
   or the positive shift induced by an already fixed contribution.

   Select the latest expected release of the distinct required
   unresolved anchors under the accepted snapshot. Require it to
   be compatible with determination and payment, and strictly after
   the valuation date for the live ACT/365F interval. Query the
   selected Black strike source on its own clock, binding calibration
   origin to valuation and expiry to that same interval. Use the
   checked terminal variance bridge
   `σ_model√T_model=σ_source√T_source` and the source's
   qualified lookup policy, without forced clamping or hidden
   extrapolation. A qualified exact-zero sigma gives model
   intrinsic on the projected ratio and retains projected status.
   A wholly observed amount needs no option inputs. When known
   contributions and contractual rounding already prove protection
   cannot pay over the unresolved support, the floor increment is
   zero without a vol lookup, and projected base cash still uses
   its required anchor data.

   Compute modeled multiplier `F + BlackPut(F,1,σ_model,T_model)`
   with an undiscounted unit put. Use the same payment-date
   factor once on the full amount. If the matched state quote,
   chronology or approximation qualification fails, the live
   protected value is unavailable or errors on the corresponding
   missing/invalid input. No intrinsic substitution for a failed
   live model and no automatic joint CPI, rates or hybrid anchor
   model.
   [Black's forward option model](https://doi.org/10.1016/0304-405X(76)90024-6),
   [checked volatility clocks and inputs](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks).

7. **Value the entitled nominal claim on its actual cash date.**
   Require the selected holder/payment cut. The future-payment
   view excludes valuation-day cash. An explicitly entitled
   pre-payment view can include already determined same-day
   amounts with factor one. Preserve fixed unpaid claims beyond
   legal maturity through their actual payment dates. Unresolved
   overdue unpaid or disputed claims require a qualified separate
   state. A verified closed position can return zero without
   inactive lookups. Bind the authorized same-currency default-free
   nominal discount evaluator. For each admitted future row use
   its positive finite dated factor and apply it once to
   known cash or model expected nominal cash. Sum checked raw
   contributions and perform one final PV Money conversion.
   Keep real yield cash unindexed and independent of model PV.
   Issuer default and nominal/real-rate covariance models require
   their own qualified valuations.
   [Dated NPV and phase-aware claim selection](../core-conventions/016-discounting.md),
   [bond entitlement foundations](../valuations-bonds/01-schedule-and-holder-cashflows.md).

8. **Use one prepared row for value, export and shared consumers.**
   Retain row and parent identity, native currency, real face and
   coupon units, actual dates, exact anchor provenance and
   observed/projected status. Carry the unfloored reference ratio,
   known contractual or projected applied multiplier, protection
   increment, nominal amount, authorized rounding, nominal factor
   and PV on that row. Preserve real-par retirement separately.
   Report only qualified known payable amounts or labeled modeled
   expectations. Keep excluded/settled history distinct. Export
   the same decomposition used for PV and qualify reconciliation
   before reporting. Remove a second lookup by payment date and
   suppression of its errors. Resolve only inputs active for the
   requested view or row. Reconcile native and registered valuation,
   runtime validation, dependency declarations, instrument and
   cashflow schemas, Python/WASM and UI through canonical Rust.
   Real-yield quotes, breakeven and bump experiments remain with
   record 05.
   [Accepted cashflow export](../valuations-framework/cashflow-export.md#accepted-decision-for-checked-cashflow-valuation-and-export),
   [real yields and inflation risk](05-inflation-yields-risk.md).

## Observed behavior

### Choice and rationale

[Indexation](../../../../finstack-quant/valuations/fixed-income/src/inflation_linked_bond/indexation.rs) resolves lagged reference CPI using published observations plus projection and prices unpublished deflation protection as an option. [Types](../../../../finstack-quant/valuations/fixed-income/src/inflation_linked_bond/types.rs) generate nominal indexed cashflows discounted on a nominal curve. Using publication state avoids treating future stored observations as known; floor time value distinguishes projected protection from a deterministic maximum. Adoption of this particular option approximation remains open.

### Scope and algorithm

1. TIPS, Canadian, French and Japanese indexation use daily linear interpolation of lagged monthly anchors. Modern UK uses three-month lag with linear interpolation; legacy UK uses eight-month lag with step interpolation. Other UK lags reject. A supplied index must itself have `lag=None` to prevent double lag and must match the expected interpolation.
2. For month lag `L`, anchors are the first day of valuation/payment month minus L months and the next month; weight is `(day−1)/days_in_current_month`. Reference CPI is the anchor interpolation, divided by positive base CPI. Step indexation selects the first-of-month lagged anchor; day lag subtracts calendar days.
3. The market resolver assesses each anchor's publication state independently. Published fixing data takes precedence; unknown anchors project from the inflation curve. Direct index-only and curve-only functions remain explicit alternatives without the market resolver's hybrid selection.
4. Unprotected payment ratio is `F=CPI_forward/base_CPI`. Protected known CPI gives `max(F,1)`. Protected unknown CPI requires `floor_vol_surface_id` with Black-lognormal quote type and strike axis; use Actual/365 Fixed time to publication or, when the resolver supplies no publication date, a lagged-reference-date fallback, volatility at strike one, and `ratio=F+BlackPut(F,K=1,DF=1,sigma,T)`.
5. Build real coupons from unadjusted contractual accrual periods with adjusted payment dates, no payment lag/end-of-month roll. Nominal coupon is `face × real_coupon × accrual_fraction × payment_ratio`. `AllPayments` floors coupons and principal; `MaturityOnly` floors only principal; `None` floors neither. Principal pays on adjusted maturity. Only payments strictly after `as_of` enter projected value. The projected redemption currently uses the negative of indexed/floored cash as its principal delta against real opening par.

### Controls and conventions

Real coupon is annual decimal Decimal; CPI/base CPI are index levels and ratios dimensionless. The bond owns lag; the market index must not duplicate it. Nominal discount DFs value the inflation-projected money, with no real-discount-curve inference. Floor volatility is explicit; the example with no floor surface cannot price unpublished protected cashflows without supplying one.

### Limitations, failure and alternatives

The floor treats an interpolated CPI ratio as one Black-lognormal forward and uses one expiry; it does not separately model joint uncertainty of two CPI anchors or inflation/rate covariance. No automatic volatility fallback substitutes intrinsic value for unknown protection. Cashflow export may log and omit an index-ratio diagnostic lookup failure even after schedule generation. Observed omission is a reporting fallback, not permission to omit pricing inputs. Deterministic inflation-only valuation and a joint nominal/real-rate model are distinct alternatives with different data needs.

### Evidence

Colocated `stored_cpi_is_not_known_before_publication`, `hybrid_ref_cpi_resolves_published_and_projected_anchors_independently`, `protected_unknown_cpi_includes_option_time_value`, adjusted-maturity and protection-scope tests address these branches. `protected_cpi_ratio_matches_independent_stochastic_density_grid` compares the floor formula with a separate numerical density grid; it does not validate an empirical CPI model. [Deacon–Derry–Mirfendereski](../../../REFERENCES.md#deacon-derry-mirfendereski-2004) and [Kerkhof](../../../REFERENCES.md#kerkhof-2005) are reference context.

## Relationship to the accepted fixing and release boundary

[CORE-MARKET-11's complete accepted package](../core-market/inflation-observations.md#accepted-decision-exact-monthly-fixing-snapshots-and-explicit-availability) makes monthly observations
unmodified exact fixing snapshots and keeps lag/weights/denominator at the
contractual resolver. Reconcile the observed index-lag/interpolation guards,
version/rebasing identity and publication classification with an explicit
knowledge date K. Unknown/overdue release facts cannot silently become known
CPI or use a reference date as publication/option expiry. An official delayed
index substitute requires the actual announcement/contract, not a projection
or last print. Record11 was accepted on **2026-11-08**, with reconciliation/
qualification pending; it does not adopt this record's Black
floor approximation, aggregate-release expiry or jurisdiction mapping.

## Independent arithmetic and qualification

- The Treasury Appendix B example reproduces reference CPI
  `154.63333` and `154.65000` and ratio `1.00011`. The UK DMO
  examples reproduce reference `173.77419`, three-month ratio
  `1.10503`, cash `1.381288` per 100 and the qualifying
  eight-month coupon `2.3953` per 100. With synthetic January,
  February and March prints of `100,102,106`, Japanese references
  on May 9, 10 and 11 are `101.933,102.000,102.129`. Generic
  first-of-month Linear gives `103.032` on May 9 on those inputs.
- On an authored example with January 31 CPI reference and
  February 2 adjusted payment, three-month linear anchor levels
  of `270,300,340` for October to December give `9270/31` at
  the contractual reference. Moving CPI to the cash date instead
  gives `2110/7`. With par `1000`, real coupon `.04`, half-year
  accrual, observed ratio `.92`, principal-only protection and
  factor `.98`, coupon `18.40` and principal `1000` give PV
  `998.032`. With ratio `1.12`, nominal principal pays `1120`
  and the real balance still retires by `1000`.
- With `F=.97`, strike one and total standard deviation `.12`,
  the unit Black put is approximately `.0636347840`; density
  integration gives the same value at the quoted precision. The
  protected expected multiplier is `1.0336347840`. In a mixed
  reference `.48+X` with mean `X=.48` and monthly-anchor total
  volatility `.20`, the shifted anchor law gives a put of about
  `.062963`. Using the same volatility on the aggregate ratio
  gives about `.099677`. This shows that anchor volatility and aggregate-reference volatility need
  different quote qualifications.

These standalone checks cover the named arithmetic cases.
Qualification still requires issue and edition fixtures, long/short
periods, legal rounding and holiday dates, historical vintages,
substitute/rebase cases, price and volatility evidence, a quantified
live rounding approximation, and native/registry/export/host
agreement. Repository pricing and host suites were not run.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
