# VALUATIONS-FRAMEWORK-17 — Inflation lag, publication availability, and reference-index interpolation

- Decision status: **Accepted — complete checked inflation references and information dates package, 2026-11-14**.
- Implementation status: **Observed, with reconciliation pending** across shared inflation reference resolution, product and calibration routes, effective-rate helpers, and hosts.
- Validation status: Source and selected primary references inspected. Independent bounded date, publication, indexation and effective-rate examples checked. Runtime, calibration, product and host qualification remain pending.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked inflation references and information dates

1. **Resolve one explicit contractual reference policy before lookup.**
   Bind the exact index/series, currency association, rebasing and
   seasonal-adjustment basis, contractual reference date, lag, Step or
   Linear rule, fixing selection and knowledge snapshot. Obtain
   contractual terms from the instrument or a deliberately selected
   qualified dated profile. A supported override changes the resolved
   policy explicitly and is validated on the same basis. Missing or
   conflicting terms error. Keep accrual, reference, valuation, payment
   and knowledge dates distinct. Remove lag and interpolation fallback
   through the observed index, curve, or library defaults, including
   out-of-range curve lags becoming no lag. Use one shared Rust resolver
   for contracts that select this first-of-month Step/Linear profile.
   Other authenticated issue rules may reuse exact monthly fixing
   selection but need their own date geometry. Japan's tenth-day
   anchor is outside this profile; do not route it through the TIPS
   mapping. Legacy UK eight-month issues also require their own
   stated observation rule. [Japanese Ministry of Finance](https://www.mof.go.jp/english/policy/jgbs/topics/bond/10year_inflation/coefficient.htm),
   [dated conventions](convention-selection.md#accepted-decision-for-checked-convention-selection-and-dated-sources),
   [CPI projection source](../core-market/inflation-projection.md#accepted-decision-explicit-cpi-levels-and-anchored-annual-parameter-shocks),
   [monthly fixing authority](../core-market/inflation-observations.md#accepted-decision-exact-monthly-fixing-snapshots-and-explicit-availability).

2. **Map contractual dates to exact monthly anchors once.** For a month
   lag `L`, use the first day of the contractual month minus `L` months
   as the first anchor. Compute `w=(day−1)/D` from the original
   contractual month and its Gregorian day count `D`. For a day lag,
   first subtract the declared literal calendar days and compute month
   and weight from the shifted date. No lag is a zero shift. Supported
   zero shifts give the same monthly anchors and result; metadata
   presence or parser spelling does not select a financial branch.
   Check calendar arithmetic and report unsupported lag or date range.
   Step and exact zero weight use only the first anchor. Linear at
   positive weight also uses the next monthly anchor. Return the checked
   positive convex combination `(1−w)I0+w I1` with stable arithmetic
   and exact endpoints. Do not lookup an unused anchor, carry a stale
   print or substitute interpolation on a dated historical series
   or projection curve for this contract calculation.
   [Treasury Appendix B](https://www.treasurydirect.gov/files/laws-and-regulations/auction-regulations-uoc/31-cfr-part-356.pdf),
   [DMO indexation annex](https://www.dmo.gov.uk/media/334d05fo/yldeqns_v4.pdf).

3. **Use an explicit information cutoff and the selected fixing vintage.**
   Resolve contributing anchors against the immutable matching
   snapshot at knowledge date `K`, separate from effective valuation
   `v`. The shared date-level profile is inclusive at end of day.
   A request that needs an intraday before-release state requires
   a separately qualified finer source or is unavailable. Earlier
   information `K<v` and later-informed restatement `K>v` are
   explicit bases. Advancing `v` does not publish or revise anything.
   Match the exact series identity and selected first-release,
   revised/final or contract-frozen version. Actual observation
   availability belongs to that specific version. Use an exact eligible
   actual observation first, even if a known schedule expected it later.
   Do not use a newer vintage or release calendar as though it were
   known at `K`. [Point-in-time fixing snapshots](../core-market/inflation-observations.md#accepted-decision-exact-monthly-fixing-snapshots-and-explicit-availability),
   [BLS release schedule](https://www.bls.gov/schedule/news_release/cpi.htm).

4. **Project only when the selected release facts admit it.**
   For each needed anchor without an eligible actual value, require
   a distinct matching expected-release fact known by `K`. If its
   expected release is strictly after `K`, use the matched unlagged
   source at the exact anchor date, subject to its real origin, clock,
   model and tails. If the release is at or before `K`, the missing
   fixing is overdue and errors. Missing release facts also error. No
   inferred publication date, legacy stale carry, base level substitution
   or generic delayed-release fallback. Preserve each contribution's
   month, weight, source/version, actual availability or expected-release
   and known-on facts, and observed/projected status. A projected anchor
   before the source's admitted origin errors; a flat tail supplies no
   backward coverage. A mixed result
   is known only after all contributing anchors are actual. Select an
   aggregate expected completion date only as the latest outstanding
   release among the required anchors; clearly distinguish it from an
   actual publication timestamp or a model expiry. Ordinary projection
   and projection shocks do not mutate observed data.

5. **Provide a qualified reference for product ratios and cash.** A
   denominator uses the exact contractual base reference or a reference
   explicitly resolved under the same admitted rules for its own date.
   Validate index identity, vintage/basis compatibility, strictly
   positive finite levels and the checked ratio. A source origin or
   latest print is not a contractual base. Remove the universal
   CPI-below-50 warning; rebased positive levels remain valid.
   Product owners retain cash rounding, principal floors, start
   reference and any published replacement, discontinuation or rebase
   rules. Use such a rule only with its actual authority and applicability.
   Keep seasonal modeling with the projection owner and never multiply
   observed prints by forecast seasonality. A nominal promised cash
   amount, projected indexation and a floored payment are distinct.
   [AFT index versions and ratio rules](https://www.aft.gouv.fr/en/oateurois-characteristics),
   [inflation-linked bond indexation](../valuations-other-fixed-income/04-inflation-indexation.md),
   [inflation swap owner](../valuations-rates/13-inflation-swaps.md),
   [cashflow export](cashflow-export.md#accepted-decision-for-checked-cashflow-valuation-and-export).

6. **Keep option information expiry with the selected quote and model.**
   When the payoff's required reference inputs are wholly observed,
   use the owner's admitted deterministic branch without a volatility
   lookup. Actual cash entitlement and discounting still apply. If
   uncertainty remains, require the explicit compatible expiry policy,
   source clock and model clock. A publication-based profile that the
   owner admits selects the latest expected release across every still
   unresolved contributing anchor, including numerator and denominator
   when both matter. An explicitly qualified reference-date quote
   retains its actual reference expiry; it is not a publication date.
   Neither may switch to the other to rescue an invalid or nonpositive
   live expiry. Missing expiry facts error. Preserve provenance, and
   require the owner to qualify the distribution, aggregation of
   information risk and payment-date measure. An expiry date alone
   does not approve deterministic ratios as stochastic means or a
   mixed-anchor ratio as one Black or Normal payoff driver.
   [Inflation option owner](../valuations-rates/14-inflation-options.md),
   [volatility clocks and source bindings](model-resolution.md#accepted-decision-for-checked-valuation-model-inputs-and-controls).

7. **Use the checked DF inverse for the separate effective-rate helper.**
   `BlackScholesInputsDf::r_eff` and the parallel local zero-rate
   helper use the canonical continuous `rate_from_df` on the exact
   model-time interval, with finite strictly positive `T` and dated
   relative `D`. Return checked `−ln(D)/T`. `D>1` can give a negative
   rate. At zero time, `D=1` does not identify a rate; skip the inverse
   in a qualified terminal branch. Invalid, nonfinite or negative
   time, invalid discount and unrepresentable results error instead of
   returning zero. A scalar that reproduces a maturity discount factor
   does not establish a valid intermediate drift for a path model.
   Asian and barrier model owners retain that selection. This helper
   is separate from CPI resolution.
   [Checked compounding and zero-time rules](../core-numerics/compounding.md#accepted-decision-explicit-compounding-conversion-and-periodic-spreads).

8. **Keep construction, calibration, valuation and reporting on one basis.**
   Reconcile zero-coupon/YoY swaps, options, inflation-linked bond
   pricing, preflight and exports, calibration and replay with the
   same resolved reference policy and provenance. Keep calibrated
   source anchors and annual parameters independent of contract lags and
   denominators. A fit's reference plan covers all required anchors
   across its quoted schedule and uses the actual last required projected
   anchor for support. Remove duplicated curve-only or lag-sensitive paths,
   source-side lag/seasonality and implicit interpolation. When a
   standalone projection or a hypothetical horizon is requested, keep
   its source and information state explicit and do not relabel it as
   actual published data. Preserve exact prepared inputs and anchors
   for comparisons, cache reuse and restoration. Reconcile current
   Python/WASM construction and reporting, declarations, schemas and
   parity through canonical Rust. Product/model/calibration choices
   beyond this shared contract and full runtime qualification remain
   open. [Inflation calibration](../calibration-targets/06-inflation-calibration.md),
   [hypothetical horizon](carry-theta-breakeven.md#accepted-decision-for-checked-horizon-theta-carry-and-breakeven).

## Observed behavior

### Choice, scope, and rationale

The shared CPI/RPI helper treats observation lag and publication availability separately. Historical contractual anchors must come from supplied index history; unpublished anchors come from the inflation curve. This avoids silently projecting a missing historical fixing and applies month lag/daily interpolation once. Inflation instrument pricing and option distribution remain in their product/model records.

### Algorithm and controls

Effective lag priority is explicit contract lag, inflation-index lag, projection-curve `indexation_lag_months`, then none. Calendar months or days are subtracted; unsupported lag variants and date underflow fail. A curve lag that cannot fit `u8` becomes none, as does zero. This observed fallback is not an adopted economic convention.

Monthly-anchor mode is selected for a month lag or an index carrying explicit monthly publication dates. A reference anchor is known when its publication date (or reference date if unspecified) is on/before as-of. Known anchors require history; unknown anchors require a same-ID inflation curve. Month lags move the reference month but retain the original contractual month's daily interpolation weight. Day/zero lags first move the reference date, then use that date's month/weight when publication metadata selects monthly mode.

Let `I0` be lagged month's first-day anchor and `I1` the next month's. Step interpolation or day one returns `I0`; linear interpolation returns `I0+w(I1−I0)`, `w=(day−1)/days_in_weight_month`. The reference is known only if both required anchors are known. Its outstanding publication date is the latest unknown-anchor publication when all required dates are supplied, otherwise unspecified. Known anchors are never replaced by projected values because history is missing.

Outside monthly mode, apply the effective lag directly and resolve the resulting date. For historical Step series, the last supplied observation may carry through the remainder of its calendar month; Linear requires right-anchor coverage through the effective date. The helper errors before extrapolating stale history. `BlackScholesInputsDf::r_eff` separately derives `−ln(DF)/T` for a model's volatility clock, returning zero for nonpositive time or discount factor; this convenience accessor does not enforce the stricter positive-DF validator used elsewhere.

The helper's `interpolation` argument controls only the monthly-anchor branch. Outside it, historical resolution uses the index's own interpolation after clearing its lag, and future resolution delegates to the projection curve's `cpi_on_date`. A requested Step therefore does not override a Linear historical index in this branch, and vice versa.

### Limitations and alternatives

Publication schedules must be supplied to represent not-yet-published past reference months. Without them, reference date is the availability boundary, so missing history errors. This is intentionally different from an automatic publication-lag estimate.

## Evidence

- [helpers.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/helpers.rs): production inflation helpers after the first inline test module; `realized_inflation_index_tests` exercise known/future anchors and stale history. Those tests are behavioral, not vendor calibration evidence.
- [Deacon–Derry–Mirfendereski](../../../REFERENCES.md#deacon-derry-mirfendereski-2004) and [Kerkhof](../../../REFERENCES.md#kerkhof-2005) are canonical inflation-model context. The precise publication/default policy remains a repository choice awaiting approval.

The live [indexed bond helper](../../../../finstack-quant/valuations/fixed-income/src/inflation_linked_bond/indexation.rs)
can bypass cutoff admission in its public index ratio and uses first-of-month
geometry in its Japanese branch. Its unpublished-floor path can substitute
a reference date for missing publication data. [Calibration](../../../../finstack-quant/calibration/src/targets/inflation.rs)
still builds source-side lag metadata and a separate quote-anchor plan.
The [inflation option](../../../../finstack-quant/valuations/rates/src/inflation_cap_floor/types.rs)
already distinguishes reference and release expiry, but still requires
matched quote/time authority and removal of implicit reference defaults.

### Independent review evidence

- Treasury's April 15, 1996 example uses January CPI `154.40`,
  February CPI `154.90` and weight `14/30`. The unrounded value is
  `4639/30`; the issuer's reporting rule gives `154.63333`.
  February 29, 2024 with three-month lag and November/December levels
  `285,314` gives `313` using the original February weight `28/29`.
  Using November's denominator gives about `312.07`.
- On March 31, 2024, a literal 31-day lag uses February 29 and weight
  `28/29`, while a one-month lag uses the same February/March anchors
  with weight `30/31`. At levels `200,229` the results are `228`
  and `7070/31`, respectively. On the first day of a month the
  zero-weight second anchor is unnecessary even under Linear.
- For an April 15 contract date, three-month lag uses January and
  February at weight `14/30`. At February 20 knowledge, January
  actual `300` available February 12 takes priority over a February 24
  expected date. A February projection of `306` with a known March 12
  release fact gives `302.8`, even if valuation is March 20 using
  that earlier snapshot. At March 12 knowledge with no actual print
  the anchor errors. A February actual `305` available March 10
  gives `907/3` at the later cutoff. A schedule revision recorded
  after the cutoff cannot qualify projection. Earlier-information
  projection does not by itself qualify a positive live option expiry.
- For Japan's April 10 anchor with January/February values `100,110`,
  the official rule gives `100`; the first-of-month formula gives
  `103`. This is an issue-rule difference even with the same CPI
  series and nominal lag.
- `D=1.01` with `T=.25` gives a valid continuous effective rate
  of about `−.0398013234`. `D=1` at positive time gives zero, but
  at zero time does not identify a rate. Finite positive subnormal
  inputs can still produce an unrepresentable quotient.

These checks use standalone calendar, exact rational and scalar arithmetic.
They do not exercise production valuation, calibration, product rounding
or host execution.

## Relationship to the accepted CPI source boundary

[CORE-MARKET-10's complete accepted package](../core-market/inflation-projection.md#accepted-decision-explicit-cpi-levels-and-anchored-annual-parameter-shocks) removes projection-curve lag
metadata, the months/12 shortcut and its implicit principal ratio. Resolve the
actual contractual/index convention without silently selecting None when that
metadata disappears. The source supplies checked unlagged absolute levels at
actual reference dates; contractual base reference CPI and publication/history
selection retain their owners. A supplied source origin is not automatically a
published monthly print. The fallback above is observed behavior requiring
reconciliation under record10's package accepted on **2026-11-08**. This
record's accepted package governs availability and indexation; runtime
reconciliation and qualification of the accepted source contract remain open.

## Relationship to the accepted monthly fixing boundary

[CORE-MARKET-11's complete accepted package](../core-market/inflation-observations.md#accepted-decision-exact-monthly-fixing-snapshots-and-explicit-availability) selects exact positive
monthly fixing snapshots with explicit version selection and knowledge date K.
It removes source lag/interpolation/seasonal conventions and makes monthly
identity independent of release metadata. The shared resolver uses actual
availability for eligible observations and distinct expected-release facts for
missing future anchors; missing/overdue facts error instead of defaulting to
reference-date publication. Reconcile K separately from valuation date and
apply explicit contractual lag/weights once. The observed fallback above is not
adopted. Record11 was accepted on **2026-11-08** with reconciliation and
qualification pending. Product and model choices beyond this record's accepted scope retain their own status.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
