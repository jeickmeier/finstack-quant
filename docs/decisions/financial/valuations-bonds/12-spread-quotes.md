# VALUATIONS-BONDS-12: Z-spread, discount margin, I/G spreads and OAS inversions

**Decision status:** Accepted — complete checked bond spread package, 2026-11-14. Required quote compounding was previously accepted under O6 on 2026-11-12.
**Implementation status:** Runtime and caller reconciliation pending. Observed behavior is recorded below.
**Validation status:** Source, primary references and independent arithmetic reviewed. Runtime, market, risk and host qualification pending.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision: required quote compounding

Accepted on **2026-11-12** as owner resolution O6 of the core-conventions
reconciliation, within the accepted
[CORE-CONVENTIONS-007 #9](../core-conventions/007-tenor-date-arithmetic.md#accepted-decision-tenor-and-date-arithmetic-conventions)
separation of payment cadence from quote compounding. Runtime
implementation is tracked in the register.

The bond carries a required, independent `Compounding` quote
convention. Spread pricing, quote inversion and spread sensitivities (CS01)
all read that one field. It is never derived from the payment frequency or
any other tenor, and it has no default: a convention preset may state it
explicitly, but no value is computed from the preset's frequency.

## Accepted decision for checked bond spreads and price inversion

1. **Elect the spread experiment and retain the original quote.**
   Use signed annual decimal spreads and explicit bp conversion
   at the output or input boundary. Preserve the accepted required
   bond quote compounding below. Bind one authorized price or
   spread driver, settlement buyer cut and currency. Normalize
   supported price-driving observations to settlement dirty Money
   on the same claim and denomination, with signed accrued for
   clean input. Independently qualified noncircular quote
   conversions may supply that target. Retain the observed
   quote, derived settlement price and selected holder model PV
   as distinct authorities. A literal observed spread can be
   reported as supplied with provenance. Using it to price or as
   a risk center requires the applicable experiment to run.
   Z and curve DM act on a prepared cash strip, I and G compare
   yields, and OAS reprices the selected model. Same-day cash
   eligibility and valuation-to-settlement financing stay with
   the accepted bond settlement contract.
   [Accepted settlement and quote carry](02-settlement-and-clocks.md)
   and [model value and quote marks](03-pv-models.md).

2. **Apply Z-spread to the settlement-native discount curve.**
   Freeze the admitted settlement buyer cash strip and an explicit
   qualified security discount source. For each actual payment
   date `T`, use its checked relative factor `D(S,T)>0` and
   its matching native coordinate `t=c(T)−c(S)` with full
   source context. For Continuous use `log D_z=log D−z*t`.
   For an admitted nominal Periodic(m), add `z` to the implied
   nominal zero rate at that interval and compound back under
   the checked core periodic law, with exact positive integer
   `m` and a strictly positive shifted base. The product admits
   the explicit supported periodic frequencies of the core law
   for this settlement calculation. Simple requires a separately
   adopted spread law. Validate the selected source and basis at
   zero spread and keep the admitted factor unchanged. Entitled
   settlement cash and any qualified native-time plateau with
   factor exactly one are spread-independent. Negative native
   elapsed time and unresolved context fail. A curve with no
   required coupon reference context cannot borrow ICMA or
   ACT/365L coupon times from the bond. Do not change financing,
   the projection curve, or any non-settlement model input.
   [Checked spread arithmetic](../core-numerics/compounding.md)
   and [Strata continuous and periodic zero-spread factors](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/DiscountFactors.html).

3. **Freeze the selected cash path and define curve discount margin.**
   Price the scheduled maturity cash or one explicitly selected,
   independently qualified fixed workout. Require the full
   workout identity, rights, redemption and actual payment dates.
   Any record-11-dependent workout remains unavailable until its
   owner supplies that qualified basis. Keep the same path for
   forward, inverse and a qualified spread shock. Remove the
   implicit selection by a clean pin and the loss of workout
   identity when clearing price-driving inputs. Curve discount
   margin uses the same settlement discount shift for a qualified
   floating bond. Freeze eligible known fixings, authorized
   projected coupons, contractual margin, bounds, allocation
   and selected path at the admitted cut. Bind projection and
   discount roles separately. The resulting curve DM reflects
   their basis and does not generally equal the contractual
   margin when the bond prices at par. Treasury auction DM
   uses a separately specified index-margin formula.
   [Workout ownership](11-workout-yield.md),
   [resolved floating coupons](../cashflows/06-floating-rate-projection.md)
   and [Treasury FRN auction formulae](https://www.govinfo.gov/content/pkg/CFR-2025-title31-vol2/pdf/CFR-2025-title31-vol2-part356.pdf).

4. **Invert Z and curve DM only on an admitted monotone strip.**
   For forward pricing use checked signed settlement cash and
   preserve original claim identity. For this quote calculation
   combine amounts only where the full spread-factor laws
   coincide. Under nominal periodic compounding, a shared
   time alone does not establish that identity. For inversion
   require a positive dirty target exceeding the spread-independent
   cash, nonnegative net spread-sensitive amounts and at least
   one positive spread-sensitive bucket. This gives a strictly
   decreasing mathematical price on the admitted domain.
   Use a finite ordered in-domain bracket, explicit positive
   spread-coordinate tolerance and work budget, and the common
   fallible Brent solver. Reprice the actual returned decimal
   using the same cash and factor law. Require a separately
   supplied positive absolute native-currency dirty-price
   residual tolerance before display rounding. Propagate
   missing data, domain, bracket and failed residual errors.
   Remove hardcoded initial brackets, maturity scaling and any
   silent solver range expansion.
   [Checked root solving](../core-numerics/brent-root.md)
   and [accepted quote-price inversion](10-quoted-yield.md#accepted-decision-for-checked-settlement-yield-and-price-inversion).

5. **Use separately qualified swap and sovereign benchmark yields.**
   I-spread is the selected settlement bond yield minus the
   authorized same-currency swap benchmark rate. G-spread uses
   the authorized same-currency government benchmark yield.
   Elect the maturity or independently qualified fixed workout
   and its explicit benchmark redemption horizon. Preserve
   legal exit and actual cash dates. Require a strictly positive
   matched benchmark horizon, observed information cut,
   quote origin or qualified forward start, source identity,
   benchmark family and explicitly compatible yield basis.
   Use exact source knots or the source's expressly admitted
   bounded linear interpolation on its common dated quote
   coordinates. Reject unqualified tails, missing data,
   mixed pillar families and implicit benchmark or compounding
   fallback. A compatible swap par rate produced from curves
   requires its own projected floating PV and fixed annuity
   under the rates owner. Discount-only telescoping requires
   its proved single-curve conditions. A government par rate
   needs its own qualified sovereign source or government
   instrument convention. Remove the generic discount-curve
   swap-par proxy for G and automatic reuse of bond discount
   calibration quotes for I. Convert benchmark quote units only
   under an authorized same-basis comparison. A special yield
   formula is not made comparable merely by equal numeric
   frequency.
   [Accepted swap pricing](../valuations-rates/03-interest-rate-swaps.md),
   [ICE Swap Rate](https://www.ice.com/iba/ice-swap-rate?showiframe=true)
   and [U.S. Treasury par benchmark methodology](https://home.treasury.gov/policy-issues/financing-the-government/interest-rate-statistics/treasury-yield-curve-methodology).

6. **Restrict the existing I-spread price inverse to an identified path.**
   With fixed maturity or a qualified fixed workout, add the
   benchmark to the input spread, then use the same prepared
   yield-to-dirty-price functional and check the forward
   implied spread on the returned price under explicit
   residual controls. Require the relevant yield law and
   unique inverse admission. G remains a reported metric
   in this package. Leave price-dependent automatic workout
   inversion to a separately derived and approved workout
   contract. A candidate must reproduce the exact workout
   identity, since multiple exits can share a date. A gap or
   multiple prices that satisfy their own workout branches
   cannot be resolved by picking the lowest candidate.
   [Workout basis owner](11-workout-yield.md)
   and [settlement yield and price](10-quoted-yield.md).

7. **Match OAS to the selected model and the original origin.**
   Apply the already accepted model-specific quote law.
   Discounting admits Continuous or nominal Semiannual
   Periodic on its native relative clock. HazardRate uses
   Continuous on ACT/365F for promised and recovery cash.
   Tree admits Continuous or nominal Semiannual Periodic
   nodewise. Sampled RatesCredit admits Continuous only.
   Both-zero RatesCredit also admits checked nodewise
   Semiannual Periodic. Structural Merton's frozen-benchmark
   effective spread is a separate experiment. An existing
   custom registered pricer needs its qualified forward
   spread and buyer-claim contracts. Preserve registry/model
   selection, admitted geometry, historical claim state,
   calibration and market input roles. Apply the spread to
   the selected risky hold and recovery as its owner requires.
   Refit affected continuation on the declared training groups
   and reprice on the declared pricing groups. Reuse only
   unaffected preparation. For settlement pricing use `Q(s)=J(s)/F`
   with qualified financing `F=D_fin(V,S)` and modeled buyer
   claim value `J(s)` at the original valuation date.
   With holder PV `H(s)`, `J(s)=H(s)−Δ(s)`. Resolve
   retained claims and contingent delivery or survival
   on the same basis and recompute any candidate-dependent
   difference. An inverse to target `Q` solves
   `J(s)−F*Q=0`. An absent qualified buyer bridge leaves
   the settlement quote unavailable and may still permit
   a holder PV. Remove pricing restarted at settlement
   under the original valuation snapshot.
   [Accepted model OAS laws](03-pv-models.md),
   [rates-tree rollback](04-rates-tree-exercise.md),
   [joint replay](06-joint-coupon-replay.md),
   [joint policy and sampling](07-lsmc-policy.md)
   and [FHFA on option-adjusted spread and model assumptions](https://www.fhfa.gov/media/33831).

8. **Qualify the OAS solve for its actual price behavior.**
   Reject a structurally spread-insensitive objective.
   For an objective qualified as continuous and strictly
   monotone on the explicit finite ordered bracket, use
   checked fallible Brent with positive coordinate tolerance
   and work budget. For finite-sample policies, call friction
   or other admitted objectives without that guarantee,
   require an expressly elected local-bracket fit. Use a
   bounded checked sign-preserving midpoint search with an
   explicit work budget and stop on the declared residual.
   Both methods admit one isolated exact-zero endpoint or
   strict opposite signs after ordered checks, and reject
   two exact-zero endpoints. Compare the actual returned
   spread after any unit conversion with the original dirty
   target on the native currency residual basis, using a
   separately supplied positive tolerance. Label a local
   match as bracket-selected. A price jump can straddle the
   target with no acceptable match, and another bracket
   may supply a different price match. Report exhausted or
   nonadvancing work with the residual and bracket.
   Propagate failed trial prices directly. Remove numeric
   error sentinels, last-iterate acceptance and silent
   bracket or sampling changes. If elected, apply the
   final pricing half-width gate separately on the fixed
   pricing groups. At a fitted spread this is a sampling
   diagnostic on the used sample, with no independent
   holdout or uncertainty estimate for the spread itself.
   [Residual and solver model boundary](../core-numerics/brent-root.md)
   and [joint OAS trial and replay rules](07-lsmc-policy.md).

9. **Keep spread basis and failures explicit in all consumers.**
   Preserve selected quote, benchmark, discount and actual
   workout identity, time basis, model state and residual
   controls across direct and registry pricing, quote sets
   and admitted spread risk. Use the same settlement Z law
   for supported no-hazard CS01 under ADR 0011. Record 16
   owns bump and metric scope. Distinguish a spread-dependent
   unavailable optional output from an invalid explicit
   request, missing required inputs or failed numerical
   evaluation. Reconcile Rust, Python, WASM and validated
   wire inputs through one calculation authority. Check
   native Money, rate units, quote provenance and actual
   returned-value residuals. Preserve independently valid
   quote or model results under their own scope.
   [Accepted bond CS01](0011-bond-cs01-without-hazard-curve.md)
   and [bond spread and credit risk](16-spread-credit-risk.md).

## Observed behavior

### Choice and conventions

Z-spread/DM discount contractual or selected workout cashflows; OAS reprices the selected model with exercise/default; I/G spread subtract a horizon par benchmark from quoted yield. All public spread quotes are decimal rates; tree OAS solver operates internally in bp. Prices are settlement dirty currency. The alternatives have different economic content and are not interchangeable labels.

### Algorithms and selection

**Z-spread:** cache positive-time settlement-entitled flows, base discount factors and times on the discount-curve day count (ACT/365L uses contractual periods). Use the core Z-spread DF: convert base DF to a nominal rate in the bond's required `quote_compounding`, add Z and compound back in the same convention; the convention is stated on the bond (presets state it per market) and is never derived from the coupon frequency (O6). Option-bearing bonds require an explicit clean quote selecting a workout path, otherwise error. Solve `Σ CF·DF_z−P=0` by fallible Brent from zero; tolerance `1e−10`; initial half-bracket is 1000 bp multiplied by `1+min(years_ACT365F/30,1)`, capped at 3000 bp. The cap is on the initial bracket, not the ultimate root domain.

**DM:** floating-only; clear price overrides, project coupons once and solve the same Z kernel against target. Initial bracket is 500 bp with the same maturity scale, capped 1500 bp; tolerance `1e−10`. DM changes discounting, not the contractual floating spread or projection curve. Option-bearing DM after clearing clean-price driver cannot select a workout and can error.

**I/G spread:** `selected_yield−benchmark_par_rate`; use workout yield/horizon when an explicit clean optional quote supplies it, otherwise YTM/maturity. I-spread uses the bond discount curve; G-spread requires an explicit government curve. If curve calibration contains at least two swap quotes, restate each quote from its index fixed-leg compounding to bond yield frequency, linearly interpolate by remaining tenor, and clamp outside end tenors. Otherwise construct a ShortFront fixed-leg schedule and use `(D_start−D_end)/ΣαD`. Fixed specs use bond day count/frequency; other specs use ACT/ACT annual. Even G-spread calls this same par-rate/swap-quote helper; it does not interpolate a sovereign cash-bond yield directly.

**OAS:** an explicit OAS is returned as-is; otherwise a price-driving quote is required. Convert quote to settlement dirty, remove price drivers, prepare the selected model at settlement and solve price minus target in bp. Default tolerance `1e−6`, max 50 iterations, initial bracket 1000 bp. Standard tree/joint dispatch reuses calibrated preparation; custom registries preserve their dispatch. Trial pricing errors are captured while the numeric objective returns `1e12`; if solve fails the first underlying error is reported. An optional MC CI gate is suppressed during trial solves and checked on the final candidate, without a separate price-residual acceptance check. Hazard and stochastic joint OAS require continuous quotes.

## Evidence and limitations

Owning code: [spread kernel/inverses](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/spread_price.rs), [benchmark par logic](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/annuity.rs), [Z](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/z_spread.rs), [DM](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/dm.rs), [I](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/i_spread.rs), [G](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/g_spread.rs), [OAS](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/oas.rs). Tests here and [quote-engine tests](../../../../finstack-quant/valuations/fixed-income/tests/bond/metrics/quote_engine.rs) are chiefly roundtrip/selection evidence. Reference: [Tuckman–Serrat](../../../REFERENCES.md#tuckman-serrat-fixed-income). Finite-difference risk should not assume all these spreads share units of compounding merely because all are decimal.

## Independent arithmetic and qualification

- Entitled same-day cash 2 plus 110 in one year with a
  4% nominal semiannual base zero gives dirty price
  106.69958358 under a 100 bp semiannual Z-spread.
  A 100 bp continuous shift on the same base factor gives
  106.67654913.
- A six-month floater with frozen index 4% and contractual
  margin 1% pays 102.50 on face 100. A par target implies
  nominal semiannual curve DM of 100 bp if the discount zero
  is 4%, and 200 bp if it is 3%. Projection stays fixed.
- Two potential exits pay 100 at one year and 121 at two
  years. At nominal annual yield their lower-yield branches
  switch at price `10000/121`. With horizon benchmarks 4%
  and 6%, target I-spread 16% has two distinct branch-consistent
  prices, about 83.33333333 and 81.29535071. Swapping the
  benchmarks gives no branch-consistent price.
- With financing .98, dirty target 100 and retained claim
  value 3, matching buyer PV requires 98 and corresponding
  holder PV 101. Comparing the holder PV directly to 98
  changes the spread objective by 3.
- With unit base factors, final cash 110 at year two, and
  a year-one call at 100 with decision-only friction .50,
  target 91.50 has OAS roots about 8.8831% and 9.2071%.
  A bracket within 1 bp of the exercise switch has opposite
  residual signs across the jump and no exact root.

Qualify the separate quote clocks and compounding laws,
detached and same-date cash, complete benchmark provenance,
fixed workout paths, contingent settlement and selected-model
forward/inverse replay. Independently qualify residual controls,
discontinuity and work failures, risk and host routes.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
