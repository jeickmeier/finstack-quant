# VALUATIONS-OTHER-FIXED-INCOME-13: Agency MBS amortization, prepayment and delayed cashflow ownership

- **Decision status:** Accepted 2026-11-14 — full checked agency MBS projection and cash ownership package.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below. Reconciliation to the accepted contract is pending.
- **Validation status:** Live source and primary references inspected. Independent amortization, mortality, factor, entitlement and dated-pay examples checked. Servicing, market, runtime and host qualification pending.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Accepted decision for checked agency MBS projection and cash ownership

1. **Bind one fixed-rate pool, position and dated opening state.**
   Select a single-class USD agency pass-through backed by
   regular monthly fully amortizing fixed-rate mortgages.
   The projected balance is a homogeneous level-pay proxy.
   It does not reproduce loan-level heterogeneity or imply that
   a weighted average of loan terms reproduces each actual
   payment. Require security/pool identity, exact program and
   issue-document version, original face on the held position
   basis, one dated opening model month and phase, continuing
   face, the exact integer count of remaining modeled
   installments, gross WAC and independent security coupon.
   The covered opening state stores the information cut,
   source/version and any WAM measure used to elect that count.
   If a published WAM supplies the proxy, require a covered
   contemporaneous value and an explicit mapping to the
   integer installment count. Do not reduce that count again
   for seasoning, cash remittance or buyer ownership.

   Require positive original face and covered continuing face
   in `[0, original_face]` in the same currency. Zero collateral
   can still leave an independently owned pending distribution.
   Keep actual collateral latest maturity, security legal final
   terms and modeled final installment distinct and covered.
   Ordinary no-prepayment installments must fit the authorized
   accrual and distribution window. Permit a later scheduled
   legal final than the proxy depletes. An earlier final needs
   a separately authorized literal terminal principal rule
   that discharges all modeled residual on its own economic
   and cash phases. Otherwise fail even if a chosen prepayment
   projection would happen to retire the balance before that
   date. Require the complete horizon before pricing and
   fail resource or represented-date limits. Remove implicit
   `WAM+12`, legal-date truncation and raw issue-date rolling.
   Keep unsupported first or final part periods, interest-only
   mortgage structures, ARMs, negative amortization, guarantee
   timing, servicing advances and borrower-credit behavior
   outside this projection. An actual irregular distribution
   can be imported from its owner. A future irregular first
   distribution needs separately qualified terms and opening
   support, or start the profile at the first covered regular
   month with preceding payable claims supplied separately.
   [SIFMA amortization and WAM conventions](https://www.sifma.org/wp-content/uploads/2017/08/chsf.pdf#page=40),
   [Fannie Mae prospectus, first and final distributions](https://capitalmarkets.fanniemae.com/syndicated/documents/mbs/mbspros/SF_December_1_2025.pdf).

2. **Select exact distribution dates and calendars by security.**
   For the admitted regular book-entry profiles use the
   25th of the month following original accrual for covered
   Fannie Mae fixed-rate single-family securities and Freddie
   Mac fixed-rate UMBS/MBS. Use the following month's 15th for
   separately identified legacy Freddie fixed Gold PCs and
   Ginnie Mae I, and the 20th for Ginnie Mae II. Apply Following
   under the security's qualified fiscal-agent or depository
   business-day calendar. Bind relevant district/agent
   context and calendar version where the terms require them.
   Keep the original accrual month, unadjusted due date,
   adjusted cash date, record date and phase separate. The
   covered ordinary month-end record profile uses the close
   of the last calendar day before the distribution month.
   Require applicable original terms, actual first and final
   distribution coverage and any authorized date exceptions.

   Treat advertised 45/50/55-day labels as descriptions.
   Remove the generic `stated_delay_days` pricing input and its
   division-by-30 or month-day clamp. An explicit custom
   monthly rule can state the month offset from accrual,
   the exact day or explicit month-end rule, adjustment, record
   rule and covered calendar. Require a positive month
   offset and a valid adjusted date after the original
   accrual month. An impossible exact day fails. An
   independently authorized dated schedule can supply exact
   remittance and record phases. Validate complete ordered
   coverage and reject inconsistent or early distributions
   under the supported delayed profile. An issuer name, pool
   category or default `usny` calendar cannot establish a
   security's full payment and record terms.
   [Fannie Mae distribution and record dates](https://capitalmarkets.fanniemae.com/syndicated/documents/mbs/mbspros/SF_December_1_2025.pdf#page=37),
   [Freddie Mac UMBS offering circular](https://capitalmarkets.freddiemac.com/mbs/docs/umbs_mbs_oc_04172024.pdf#page=64),
   [Freddie Mac Gold PC trust terms](https://capitalmarkets.freddiemac.com/mbs/docs/pcagreement_073022.pdf#page=4),
   [Ginnie Mae book-entry remittance](https://www.ginniemae.gov/s3/sites/default/files/mbs_guides/Chapter_15.pdf).

3. **Use the security coupon for investor interest and WAC for principal.**
   Admit finite nonnegative annual gross WAC and security coupon
   in the selected fixed-rate profile. Use the stated net
   security coupon directly on opening continuing face. The
   named full monthly investor accrual uses `1/12` on the
   original 30/360 parent, regardless of actual days or
   adjusted payment date. Remove arbitrary investor accrual
   day count for the named profile. An explicitly qualified
   bespoke accrual requires its own original parent and
   fraction. The monthly borrower rate for homogeneous
   principal is `WAC/12`, also independent of settlement
   accrued and the discount source's time coordinate.

   Known WAC, security coupon and fee disclosures can have
   distinct weighting, as-of dates and publication precision.
   Remove the generic `1e−6` fee-consistency gate and inferred
   zero fee inputs. A same-basis synthetic pool with a declared
   complete exact spread decomposition may validate
   `WAC = coupon + Σ retained_bp / 10000`, including any
   identified spread beyond servicing and guaranty fees.
   Otherwise retain independently authorized fee disclosures
   as provenance with their own precision. The net investor
   coupon remains cash authority. A retained fee is not a
   second deduction from its cash.
   [Fannie Mae contractual fixed rate and distributions](https://capitalmarkets.fanniemae.com/syndicated/documents/mbs/mbspros/SF_December_1_2025.pdf#page=37),
   [SIFMA standard cash equations](https://www.sifma.org/wp-content/uploads/2017/08/chsf.pdf#page=5).

4. **Conserve exact projected par through full amortization.**
   For opening funded face `B>0` and `n≥1` remaining monthly
   installments, scheduled principal is `B` at `n=1`.
   At exact zero WAC with `n>1` it is `B/n`. Otherwise
   use `B r / expm1(n log1p(r))`, with `r=WAC/12`.
   Evaluate the complete expression in scale, including the
   small-rate limit without switching positive rates to zero.
   Do not subtract interest from a rounded payment to get
   scheduled principal. Reject negative WAC in this profile.
   A contractual terminal full-principal override, when
   admitted, separately retires the exact remaining par once.

   Restore scheduled principal `P_s` under the accepted Money
   boundary. Apply checked monthly mortality to the exact
   remaining scheduled base. Set voluntary prepayment
   `P_u=SMM × (B−P_s)` with one restoration, exact zero
   and unit mortality endpoints, and closing balance
   `B'=B−P_s−P_u` on admitted amounts. For the regular
   modeled balance, post principal effects at the close
   of that accrual month, before the next month opens.
   Keep investor payable dates and issuer book-entry
   effects distinct. Check ranges and carry the exact
   residual. Refuse unresolved numerical
   loss of positive principal, unapproved premature
   depletion and unrepresentable effects. Keep all nonzero
   admitted amounts including sub-cent cash. Remove clamps,
   near-zero WAC approximations and magnitude-based row
   suppression. Apply authorized financial rounding only
   at its actual boundary and require reconciled cash and
   stock. Projection ends on exact retirement, with qualified
   payment claims still live. Do not represent resource
   truncation or residual face as a complete priceable strip.
   [Checked Money and explicit rounding](../core-conventions/012-money-decimal-precision.md),
   [principal effects and state](../cashflows/12-schedule-lifecycle.md).

5. **Use explicit mortgage seasoning and monthly mortality.**
   Keep an independently supplied positive applied mortgage
   month for the first projected accrual in the accepted
   convention. A sourced weighted-average loan age or
   otherwise qualified collateral-age proxy needs the actual
   effective month, units and explicit conversion to the
   applied index. Advance by exact calendar-month steps
   on the original accrual grid, with checked arithmetic.
   Forecast months do not acquire age from issue days,
   payment delays, knowledge dates or quote cuts. An authored,
   documented Fannie/Freddie same-month exception may
   repeat applied month one where the benchmark requires it.
   Keep that mapping independent of any initial interest-only
   or principal timing in the security's cash terms.

   Elect constant annual CPR, PSA, or a validated sourced
   annual CPR vector under CASHFLOWS-010. Require all inputs
   and use its strict checked conversion. PSA takes a
   multiplier of one for 100% PSA, with annual
   `CPR=min(speed × min(month,30)/500,1)`. Use monthly
   survival equivalence `SMM=−expm1(log1p(−CPR)/12)`
   with the accepted stable cap and endpoint treatment.
   A vector follows the accepted explicit indexing and
   terminal hold. Constant zero is the no-prepayment election.
   Keep ABS loan-count and lockout profiles outside this
   agency model. Borrower incentive, rate-response and OAS
   shocks are decisions for record 14. Preserve the same
   mortgage-age and principal rules on every admitted path.
   [Shared monthly mortality](../cashflows/10-prepayment-and-mortality.md),
   [SIFMA PSA and same-month pools](https://www.sifma.org/wp-content/uploads/2017/08/chsf.pdf#page=6),
   [Fannie Mae same-month pooling](https://selling-guide.fanniemae.com/sel/c3-2-01/determining-eligibility-loans-pooled-mbs).

6. **Carry exact factor vintages and pending P&I separately.**
   Use a dated, authorized face snapshot or a matching exact
   published factor applied to original position face. Preserve
   factor precision, effective month, source, publication and
   correction version and the actual information cut. Validate
   the explicit authority, including the reconciliation if both
   amounts are supplied on the same basis. Keep issuer factor,
   calculated face and actual book or settlement face separate
   when rounding or reporting law differs. For the covered
   post-distribution-factor profiles `F(m)` represents the
   balance after the principal payable in distribution month
   `m`, even when published before that payment. Thus covered
   adjacent factors can establish total pending principal
   `O × [F(m−1)−F(m)]`, and the pre-distribution balance
   and stated coupon can establish recurring pending interest
   under the applicable terms. Prefer authorized remittance
   cash where actual rounded amounts differ. Require the
   qualified component before publishing it. A factor
   difference alone cannot classify realized scheduled
   versus unscheduled principal.

   Prepare actual determined P&I by distribution and explicit
   ownership/payment state. Keep it apart from the continuing
   projection, even at zero face. Never recreate a prior
   month's payable P&I from current face, modeled WAM or
   today's PSA. Keep an original economic principal effect
   if it falls within the tracked coverage. If opening
   continuing face already reflects that principal, tag the
   pending receipt with zero further effect on that track.
   Preserve both original scheduled due date and authorized
   pending cash date. A past scheduled date establishes
   neither payment nor a new remittance date. An unresolved
   overdue claim fails the dependent valuation. An unreleased
   factor can support a separately elected hypothetical
   projection from a covered earlier opening, with correctly
   advanced dated state and projected labels. Exact current-
   face results require the actual applicable factor or
   independently authorized face. No substitution of a stale
   factor or information from a later vintage.
   [Freddie Mac factor and distribution conventions](https://capitalmarkets.freddiemac.com/mbs/docs/umbs_mbs_oc_04172024.pdf#page=63),
   [Fannie Mae monthly factor disclosure](https://capitalmarkets.fanniemae.com/syndicated/documents/mbs/mbspros/SF_December_1_2025.pdf#page=10).

7. **Select holder and purchaser claims on the same prepared pool.**
   A current holder receives covered pending and projected
   P&I it owns. A purchaser receives the rights established
   at its authorized actual settlement date and phase, with
   the exact acquired original and current face, factor
   vintage and entitlement. Under the covered ordinary
   timely-settlement profile a purchase in the settlement
   month generally acquires that month's accrual and later
   claims, with earlier distributions retained by the seller.
   Use the actual record-date relationship and contractual
   transfer authority. A fail, interim accounting, due bill
   or modified record process needs its own qualified
   transfer cash or right. Do not infer ownership from
   payment date alone, edit paid-period state to mimic
   buyer ownership, or roll a pre-settlement buyer scope
   into a different model origin. Keep eligible unpaid
   same-day cash at the explicit phase with factor one.
   Resolved settled cash is excluded from valuation.

   For an ordinary qualified month-start clean quote, use
   the authorized purchased current face `B_S>0`, exact
   settlement and stated coupon. Set
   `dirty = points × B_S/100 + B_S × coupon × N/360`,
   where `N` is the SIFMA 30/360 count from the eligible
   original month start to the authorized settlement in that
   month. Here the start day is one, so `N=settlement_day−1`,
   including zero on day one and 30 on day 31. Apply the
   authorized rounding and record any separate compensation.
   A direct dirty target needs no unused accrued lookup.
   Preserve the independently elected quote denomination and
   use a matched scope for any original-origin buyer value
   or settlement conversion. A pending seller payment
   changes the owned strip independently of the accrued
   addition. Pool delivery, TBA quantity and factor
   adjustment, commercial settlement and roll economics
   remain with record 15.
   [SIFMA current factor and accrued formula](https://www.sifma.org/wp-content/uploads/2017/08/chsf.pdf#page=46),
   [SIFMA remittance and due bills](https://www.sifma.org/wp-content/uploads/2024/04/TBA_Market_Governance-Uniform_Practices_Manual-Chapter_10_Remittance_Reporting_Payments_Due_Bills_and_Compensation.pdf),
   [TBA and dollar rolls](15-tba-and-dollar-roll.md).

8. **Use one complete prepared schedule across consumers.** Value
   the selected Discounting holder or buyer stream on the
   authorized valuation date, with positive checked dated
   relative nominal factors in native currency and explicit
   same-day inclusion after entitlement. No active cash
   after validated closure needs no discount lookup. Export
   the same cash used for pricing with unique original
   period and claim identities, accrual bounds, record and
   economic phases, actual cash dates, observed or projected
   amounts, and source and factor metadata. Distinguish the
   projected continuing balance effect from issuer book-entry
   principal movement. Keep scheduled and voluntary projected
   rows separate. For actual principal with only an authorized
   total, publish a principal repayment without fabricating
   that split. Exclude that cash from a second balance
   reduction whenever already in opening face. Reconcile
   complete native PV and schedule balances through the
   existing checked owners.

   Admit and restore validated state atomically through the
   canonical Rust product. Reuse the checked monthly
   transition and prepared event dates in deterministic
   projection and record 14's OAS and risk paths, and
   reconcile TBA/roll nested collateral and any structured-
   credit callers. PAC, tranche entitlement and waterfall
   behavior retain their own ownership. Update existing
   Rust, generic Python/WASM instrument and cashflow
   envelopes, schemas, examples and UI consumers together.
   Keep unsupported, failed, observed and projected
   outcomes traceable without duplicating financial logic
   in hosts.
   [Shared dated cashflow export](../valuations-framework/cashflow-export.md),
   [dated discounting](../core-conventions/016-discounting.md),
   [agency MBS rate risk and OAS](14-mbs-risk-oas.md).

## Observed behavior

### Choice and rationale

[The pool kernel](../../../../finstack-quant/valuations/fixed-income/src/mbs_passthrough/pricer.rs) models a homogeneous level-pay mortgage pool using a supplied prepayment specification and agency payment rule. Scheduled mortgage amortization uses monthly WAC, while investor interest uses the explicit accrual day count. Separating these clocks is documented in the source; pool homogeneity and prepayment assumptions still require adoption.

### Scope and algorithm

1. Start from `current_face`. Find the earliest unpaid accrual month from `last_paid_accrual_end`, or infer it by comparing delayed payment dates with `as_of`. The holder path retains a prior month's P&I still in flight.
2. Project monthly while balance is at least `.01`, the period cap is not reached and the legal-maturity stopping condition permits. WAM is remaining maturity at valuation; it shrinks once per projected month and is not reduced again for seasoning. PSA/other seasoning instead uses `floor(days_since_issue/30.4375)`.
3. For balance `B`, remaining payments `n`, monthly mortgage rate `r=WAC/12`, scheduled principal is `B` for `n≤1`; otherwise `max(0,min(B, B r(1+r)^n/((1+r)^n−1) − Br))`. If `r≤1e-12`, use `B/n`.
4. Apply `prepayment = max(B−scheduled,0) × SMM` after scheduled amortization. Interest is `B × net_coupon × accrual_fraction`; ending balance is floored at zero. `SMM` must be finite and in `[0,1]`.
5. Emit separate interest, amortization and prepayment rows. Investor coupon must match `WAC − servicing_bp/10000 − guarantee_bp/10000` within `1e-6`. Discount payments strictly after `as_of` using the nominal curve's dated DF ratio; native value requests WAM+12 as a projection cap.
6. Quote-basis purchasers receive settlement-month accrual onward. `quote_basis_pool` marks prior accrual months paid, excluding the seller's in-flight P&I. Dirty quote adds settlement-month interest accrued from the first day of that month. This differs intentionally from holder NPV.

### Selection and conventions

[Types and payment rules](../../../../finstack-quant/valuations/fixed-income/src/mbs_passthrough/types.rs) supply the sole native Discounting path. FNMA/FHLMC pay on the following month's 25th, GNMA I on the 15th and GNMA II on the 20th, rolled Following on `usny`. Custom stated delay `D` maps to month offset `(D−1)/30` and day `D−30k`; it is not literal calendar-day addition. Default/custom prepayment is supplied explicitly to the pool. Original face affects pool factor; current face drives cashflows.

### Limitations, failure and fallback

The implementation does not model loan-level heterogeneity, defaults/guarantee timing, delinquency advances, burnout, seasoning-adjusted WAC or stochastic house prices. Positive WAM is required; a balance below one cent is treated as retired. A legal maturity earlier than remaining WAM can stop projection before full scheduled retirement, so full retirement is not guaranteed for misaligned inputs. Missing paid-period state is inferred from dates, not from an external servicing ledger. The negative/tiny monthly-rate branch uses straight-line principal rather than a negative-rate annuity.

### Alternatives and tradeoffs

A homogeneous monthly pool is inexpensive and reusable by TBA and MC-OAS. Loan-level simulation and empirical refinancing/burnout models offer different behavior but require borrower data and calibration. Holder receivable valuation and clean-quote spread fitting require distinct cash ownership bases; silently sharing one basis would misprice delayed P&I.

### Evidence

Colocated projection tests and `agency_convention_vector_is_stable` in [types](../../../../finstack-quant/valuations/fixed-income/src/mbs_passthrough/types.rs) check decomposition, amortization and dated agency cases. The [SIFMA formula reference](../../../REFERENCES.md#sifma-mbs-standard-formulas) supplies methodology context. No independent servicing tape was reconciled.

## Independent arithmetic and qualification

- For 120,000 continuing face, two installments, gross WAC
  6%, fixed investor coupon 5.5% and first-month SMM .25,
  the first scheduled principal is `24,000,000/401`; the
  voluntary principal is `6,030,000/401`; and the continuing
  principal is `18,090,000/401`. The last installment retires
  that exact residual in the analytical example. First-month
  investor interest is 550 in the full-month convention,
  including in February. On face one billion and 360
  installments, a positive monthly borrower rate of
  `1e−12` gives first scheduled principal about
  `2,777,777.7772791667`. The exact zero-rate limit is
  about `2,777,777.7777777778`. Face `.009` with one
  payment left retires the full `.009`.
- At 100% PSA, applied months 1, 2, 29 and 30 give SMM
  about `.00016681964`, `.00033394601`, `.00496679153`
  and `.00514301283`, respectively. The SIFMA seasoning
  case for a Ginnie Mae I pool issued in March 1988 uses
  applied month 17 for June 1989. Opening and closing
  factors `.85150625` and `.84732282`, WAM 344 and gross
  WAC 9.5% give SMM about `.00435270490` and CPR about
  `.05099998639`, or approximately 150% PSA.
  [SIFMA seasoning example](https://www.sifma.org/wp-content/uploads/2017/08/chsf.pdf#page=8)
- Under the stated Following profiles and the illustrative
  Federal Reserve business schedule, April 2026 accrual on
  the 25th-day profile pays May 26. January 2026 accrual on
  the 15th-day profile pays February 17. May 2026 accrual
  on the 20th-day profile pays June 22. Program calendar
  coverage and paying-agent exceptions need separate
  qualification. [Federal Reserve holiday calendar](https://www.federalreserve.gov/aboutthefed/k8.htm)
- On original face 1 million, adjacent June and July factors
  `.90` and `.88` and coupon 6% give July pending principal
  20,000 and interest 4,500. At an eligible July 15
  settlement, a 99 clean quote on continuing face 880,000
  plus 14/360 accrued gives dirty `873,253.333333…`
  before financial rounding. With the prior distribution
  owed to the seller, identical continuing rights and a
  factor of .999 to that payment, the holder–buyer PV
  difference is 24,475.50. A next-factor value of zero can
  still leave final pending P&I. Factors .10 and zero at
  the same coupon and original face give 100,500 to the
  entitled holder.

These standalone checks cover selected formulas and dates.
Qualification still requires independent issuer/servicer tapes
and actual factor vintages, support for each security profile,
legal and record-date cases, maturity and principal
reconciliation, quote and PV reproduction, and host outputs.
Repository pricing and host suites were not run.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
