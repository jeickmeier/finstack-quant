# VALUATIONS-BONDS-10: Street, Moosmüller and Japanese yield pricing and inversion

**Decision status:** Accepted — complete checked settlement yield and price inversion package, 2026-11-14.
**Implementation status:** Runtime and caller reconciliation pending. Observed behavior is recorded below.
**Validation status:** Source, primary market and numerical references, and independent arithmetic reviewed. Runtime, market-specific quote, risk and host qualification pending.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked settlement yield and price inversion

1. **Elect the quote convention and one settlement claim.** Use
   the accepted buyer settlement, information cut, price authority
   and cash entitlement. Require an explicit yield convention in
   the instrument or a qualified market preset and retain its
   provenance. Yield is a signed annual decimal in the selected
   convention. Supply periodic quote frequency as a positive
   integer when the law requires it. A named convention may
   require compatibility with the contractual coupon grid.
   Coupon tenor, currency and a generic yield metric do not select
   the law or frequency. Bind a single original price or yield
   driver and preserve that identity across normalization. An
   ordinary maturity yield discounts a qualified scheduled
   maturity scenario. It can use frozen projected cash when
   expressly qualified and labelled. For an optional bond this
   scenario assumes no voluntary exercise, where the convention
   admits it. Workout selection, realized returns, curve PV and
   OAS use their own admitted laws. Remove default Street
   dispatch and conflicting or inferred quote conventions.
   [Accepted settlement and quote clocks](02-settlement-and-clocks.md#accepted-decision-for-checked-bond-settlement-quote-carry-and-clocks),
   [price authority](03-pv-models.md),
   [required quote compounding](12-spread-quotes.md#accepted-decision-required-quote-compounding)
   and [workout yield](11-workout-yield.md).

2. **Resolve exact cash eligibility and the selected quote coordinates.**
   Use actual payment dates and authorized phases for the buyer's
   cash. Build the yield dates and applicable elapsed fractions
   from the elected convention, including the contractual first
   broken period and nominal or quasi-coupon slots. Retain
   qualified unadjusted or accrual coordinates where the convention
   requires them. Accepted ICMA uses the authored reference grid.
   Accepted ACT/365L uses complete original actual coupon parents
   and the coupon end-year denominator for each contained slice.
   Payment lag does not extend coupon-period coverage.
   Keep distinct claims until their own quote coordinates are
   established. Exclusion, zero cash, split components and PIK
   never renumber the contractual grid. Include explicitly entitled
   immediate settlement cash at factor one on an admitted quote
   branch. A past or zero quote coordinate for a future payment
   requires a qualified convention branch. Without one that
   quote is unavailable even when the cash carries model PV.
   Reject missing context and unresolved, invalid or collapsed
   distinct required coordinates. Remove raw date inference for
   coupon-context day counts and same-payment-date grouping before
   yield-coordinate resolution.
   [Day-count context](../core-conventions/002-coupon-context-day-counts.md)
   and [UK DMO quasi-coupon and buyer cash rules](https://www.dmo.gov.uk/media/334d05fo/yldeqns_v4.pdf).

3. **Use the checked nominal periodic, continuous and simple rate laws.**
   On admitted positive quote time `t`, use
   `exp(−m*t*log1p(y/m))` for nominal periodic,
   `exp(−y*t)` for continuous and `1/(1+y*t)` for simple.
   Require `y>−m` for periodic, strictly positive simple
   denominators at every active time, and a finite yield for
   all three. Use the checked core arithmetic. Retain finite
   negative yields and admitted positive factors above one.
   Remove the scalar Street alias for uniform periodic compounding,
   repaired frequency and negative-yield warning threshold.
   A venue-labelled quote requires complete market qualification
   of its own schedule, day count, cash and rounding. The generic
   periodic formula alone supplies no venue certification.
   [Accepted compounding](../core-numerics/compounding.md)
   and [DMO conventional gilt equations](https://www.dmo.gov.uk/media/334d05fo/yldeqns_v4.pdf).

4. **Give U.S. Street its explicit final-period branch.** For the
   qualified fixed cash-pay bullet profile, require the selected
   U.S. Street day count, regular contractual quote grid, and
   explicit `m` compatible with the grid. Identify the final
   interval from the contractual schedule through redemption.
   Let `f` be the resolved positive remaining fraction of the
   first reference period and `k` the number of full scheduled
   periods after that slot. Outside the final interval use the
   nominal periodic law at `t=(f+k)/m` for each qualified slot.
   In the final interval discount the terminal bucket at
   `1/(1+y*t_f)` with `t_f=f/m`. Validate
   this active simple domain. Select the branch from the
   contractual interval, independent of how many payments remain
   eligible or nonzero. Reject unsupported irregular or off-grid
   cash geometry. The profile is for terms that use this
   secondary-market rule. Treasury auction conversion and UK
   DMO redemption yields have their own conventions.
   [Henrard on U.S. Street and final-period formulas](https://quant.opengamma.io/Bond-Pricing-OpenGamma.pdf)
   and [U.S. Treasury auction formulae](https://www.treasurydirect.gov/files/laws-and-regulations/auction-regulations-uoc/31-cfr-part-356.pdf).

5. **Give Moosmüller its contractual next-coupon anchor.** For
   this regular-grid nominal profile, let `w>0` be the
   qualified quote fraction in years from settlement to the
   next strictly future contractual coupon slot. Use
   `D_k=1/(1+y*w) * (1+y/m)^(-k)`, with exact `k≥0` full
   scheduled periods after the anchor. The first interval
   uses at most one qualified nominal period. Require complete
   grid coverage, regular later intervals and on-grid yield
   coordinates for every discounted cash claim. Decline long
   first periods, later stubs and irregular payment coordinates
   outside this profile. Retain the anchor when its coupon
   is zero, absent or detached. Require `1+y*w>0`; also require
   `1+y/m>0` when an active exponent `k>0` uses it. For entitled
   immediate cash use the separate settlement branch. Remove
   the context-free fractional-time approximation and ranking
   by positive payment rows. A lone payment at `k>0` still
   uses the full factor and its matching inverse solve.
   [Filed Moosmüller yield example](https://www.hypovereinsbank.de/content/dam/hypovereinsbank/ueber-uns/pdf/investor-relations/Emissionen/2017/HV2AMX-Final-Terms-20170125.pdf).

6. **Keep Japanese simple yield as a scoped clean-price conversion.**
   Elect an explicitly identified Japanese-style simple ACT/365F
   profile for a qualified straight fixed-level cash-pay bullet
   with par redemption and eligible settlement. Use positive
   remaining life `T=actual_days(S,maturity)/365`, annual
   coupon decimal `c` and positive clean price `P` per 100.
   Compute `y=(100*c+(100−P)/T)/P` and the inverse
   `P=100*(1+c*T)/(1+y*T)`. Require finite inputs and strictly
   positive numerator and denominator growth terms and
   representable finite outputs on both directions. Preserve
   valid signed yields. Convert clean to dirty only under
   record 02 when that output is requested. Use checked
   arithmetic and verify a returned yield by repricing on the
   same clean basis with an explicit native-currency residual
   tolerance using its qualified denomination. Exclude floating,
   step-up, PIK, amortizing, custom
   cash and embedded exercise terms from this simple profile.
   This formula estimates annual coupon plus pull to par on the
   declared simple basis. Japannext's day-count switch at the
   first settlement anniversary is a different venue convention
   outside the elected fixed ACT/365F profile. Apply any market
   display rounding separately and only when expressly selected.
   [Japan Ministry of Finance yield formula](https://www.mof.go.jp/english/policy/jgbs/publication/debt_management_report/2025/esaimu2025.pdf)
   and [Japannext trading rules, Appendix 1](https://www.japannext.co.jp/pub_data/pub_onboarding/JNX_Trading_Rules_Bonds_2.01_EN.pdf).

7. **Share one dirty price functional and admit only monotone inverses.**
   Validate the original claims, terms, currency and quote geometry
   before arithmetic. For this quote operation, amount-net only
   claims with identical discount-law coordinates, using exact
   same-currency amounts. Preserve original identities for
   reporting. Drop exact zero net buckets only from calculation
   dispatch after grid preparation. Accumulate
   `P_dirty=C0+Σ C_i D_i(y)` with checked signed arithmetic.
   Here `C0` is the entitled time-zero net cash. Use the same
   prepared terms and whole-position dirty price basis in both
   directions. Require a positive dirty target, positive
   `P_dirty−C0`, nonnegative positive-time net amounts and at
   least one positive yield-sensitive bucket for inversion.
   This gives at most one solution on the selected domain.
   Signed gross pricing is supported where the elected prepared
   law admits it. A negative future net bucket or a
   yield-insensitive claim cannot use this ordinary inverse.
   A failed, absent or nonfinite yield has no zero or infinity
   substitute. Clean prices use the accepted signed accrued
   bridge and qualified denomination. A dirty-only operation
   requires no unused accrued lookup.
   [Settlement price normalization](02-settlement-and-clocks.md)
   and [distinct IRR and XIRR contracts](../core-conventions/017-irr.md).

8. **Solve with matched analytic or bounded checked arithmetic.**
   Use a checked single effective positive-time bucket inverse
   when the full factor is one core rate law. With amount
   `C>0` and positive future target `Q=P_dirty−C0`, set
   `L=log(C/Q)` stably and use `L/t` for continuous,
   `m*expm1(L/(m*t))` for periodic, or `expm1(L)/t` for
   simple. Use the latter for the U.S. Street final simple law
   and for Moosmüller only when `k=0` with time `w`.
   For other admissible cases use the common fallible Brent
   kernel with an explicit finite ordered in-domain bracket,
   positive yield-coordinate tolerance and positive work budget.
   Apply its ordered endpoint and sign rules and propagate
   pricing or range errors. Remove heuristic coupon-rate
   guesses, silent bracket repair and row-count shortcuts.
   Re-evaluate the actual represented returned yield under its
   elected quote domain and original price functional. Require
   an explicit positive absolute dirty price residual tolerance
   in the native currency before display rounding, including
   for the analytic path. No parameter-width-only acceptance.
   Restore Money with the authorized arithmetic and apply
   contractual quote or settlement rounding once at the
   specified boundary.
   [Checked scalar solver](../core-numerics/brent-root.md)
   and [Brent's original zero-finding method](https://maths-people.anu.edu.au/brent/pub/pub005.html).

9. **Keep price, yield and dependent calculations on their declared basis.**
   Preserve authority for the original quote, derived clean and
   dirty values and the selected model PV. Remove internal
   re-stamping of a derived clean price as a new source quote.
   Share prepared cash, exact yield law and units across
   quote sets, direct conversions, pricing and the existing
   Python and WASM routes. Dependent workout, spread, duration,
   convexity and carry consumers retain the selected quote law
   or explicitly qualify their own. They cannot mark a different
   kernel as the same YTM. Inflation-linked and other bond
   families keep their own admitted quote and cash scenarios
   when reusing the arithmetic. Report unsupported optional
   outputs with their availability reason. Propagate failures
   for requested invalid inputs, missing required data and
   failed numerical evaluation. Validate complete quote terms,
   schema/state restoration and host conversion. Keep timing,
   inversion and rounding authority in Rust. Market-specific
   quote and independent price/solver qualification remain
   distinct from self-repricing.
   [Workout](11-workout-yield.md), [spread quotes](12-spread-quotes.md),
   [yield risk](14-yield-risk.md) and [carry](19-carry-and-life.md).

## Observed behavior

### Choice, domain and selection

Default YTM is Street nominal yield compounded at coupon frequency, using settlement-entitled contractual cashflows and the time rules in record 02. A separate Moosmüller metric and public generic `Rate(Compounding)` kernel are supported. These are quote-convention calculations, not stochastic discounting. Coupon rate only seeds the inverse solve; arbitrary projected/signed holder cashflows can be supplied, without a uniqueness guarantee for signed IRRs.

### Algorithms

Street DF is `(1+y/m)^(−m t)` with `m=max(payments_per_year,1)`, requiring positive compounding base. For Moosmüller full-path pricing, first positive cashflow time `w` supplies simple-interest `1/(1+yw)`, then each distinct positive payment date increments a coupon index and contributes `(1+y/m)^(−(k−1))`; cash and principal on one date share the index. Standalone `df_from_yield` infers `w` from fractional periods instead, so irregular-date calls should use the complete schedule kernel. Nonpositive time returns DF one in that helper, while price summation omits nonpositive-time flows. Generic Rate compounding delegates the core compounding DF and rejects nonfinite results. Yield below −50% warns but is not rejected solely for that threshold.

YTM inverse requires positive target and nonempty flow list. A single positive dated flow uses closed-form Rate inverse, simple inverse for Moosmüller, or Street periodic inverse. Otherwise Brent solves `price(y)−target` with yield-axis tolerance `1e−12` and 50 iterations, initialized by current yield plus half annualized pull-to-par, clamped to [−1,10] only as a starting guess. Pricing-domain failures and nonconvergence propagate; no false residual is substituted.

Japanese simple yield is fixed-spec-only and uses ACT/365F remaining life `n`, positive clean price percentage `P`: `y=(100c+(100−P)/n)/P`; inversion is `P=100(1+c n)/(1+y n)`, requiring a positive denominator. It ignores the detailed timing of remaining coupons, and no model-based option adjustment is included.

### Failure, alternatives and located evidence

Zero-price yield is undefined; no positive future time or mixed-sign flows can undermine uniqueness. The one-flow shortcut counts supplied rows, so an extra zero or expired row changes algorithm selection. ACT/365L has explicit coupon-period constraints rather than generic date-only fallback. Numerical tolerance is not independent quote accuracy.

Owning code: [yield/price kernels](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/yield_price.rs), [solver](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/ytm_solver.rs), [YTM metric](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/ytm.rs), [Japanese metric](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/mod.rs), [quote type](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/types.rs). Inline tests include scalar formulas and round trips; [market benchmarks](../../../../finstack-quant/valuations/fixed-income/tests/bond/validation/market_benchmarks.rs) include broad expected ranges and self-repricing, which must not be described as vendor certification. [Fabozzi](../../../REFERENCES.md#fabozzi-fixed-income-handbook) and [ICMA](../../../REFERENCES.md#icma-rule-book) are general references; exact market/security applicability and Moosmüller irregular-schedule provenance remain open. Continuous, periodic and simple alternatives are implemented explicitly; selecting one changes the quote meaning.

The [Python quote set](../../../../finstack-quant-py/src/bindings/valuations/bond_quotes.rs)
and [WASM quote routes](../../../../finstack-quant-wasm/src/api/valuations/typed/fixed_income.rs)
consume these results. The [yield risk dispatcher](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/mod.rs)
and existing carry callers require explicit reconciliation with
the selected settlement yield law.

## Independent arithmetic and qualification

- At 10% nominal semiannual yield, a single final amount
  of 105 at quote time .25 gives nominal periodic price
  102.46950766 and U.S. Street final-simple price 102.43902439.
- For Moosmüller, `w=.25,m=2,y=.10` and a detached first
  cash coupon leave the following 105 payment at `k=1`.
  Its price is `4000/41≈97.56097561`. Reanchoring
  to the first eligible cash at quote time .75 gives
  97.67441860, a different result.
- Cash of 4 and 102 paid on the same actual date have
  authorized quote times .25 and .75. At 10% nominal
  semiannual yield the combined PV is 98.70532166. Grouping
  both at .25 instead gives 103.44540773.
- Signed future flows +230 at year one and −132 at year
  two both price to 100 at yields .10 and .20 with nominal
  annual compounding. In a separate case, immediate cash 2
  and same-coordinate future cash 6 and 100 in one year
  give an analytic annual yield of .06 from dirty target 102.
- From 2023-07-01 to 2024-07-01, a Japanese-style clean
  price of 98 and coupon .02 give 4.07605665% with fixed
  ACT/365F. A no-leap fraction of one year gives 4.08163265%.

Qualify against independent irregular and regular quote cases,
known same-day and detached cash, both clean/dirty directions,
large and small signed rates and actual returned-value residuals,
the named market profiles and their unsupported geometries,
dependent yield-risk and carry, and Rust and host parity.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
