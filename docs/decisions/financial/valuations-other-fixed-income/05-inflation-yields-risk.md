# VALUATIONS-OTHER-FIXED-INCOME-05: Real yields, breakeven inflation and inflation sensitivities

- **Decision status:** Accepted 2026-11-14 — full checked inflation-bond yields, breakeven and risk package.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Reconciliation pending. The observed entry points and kernels are described below.
- **Validation status:** Live source and primary references inspected. Independent quote, duration, breakeven and CPI-risk arithmetic checked. Runtime, issue, market and host qualification pending.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Accepted decision for checked inflation-bond yields, breakeven and risk

1. **Use an authorized real settlement quote on unindexed face.**
   Bind the security’s accepted issue profile, trade and information
   cuts, eligible actual settlement date and phase, authorized buyer
   rights, and positive real quote denomination. Select one native
   real dirty, clean or real yield authority on that same position.
   Real clean points per 100 become whole-position real clean by
   `points × real_face/100`. Convert clean to dirty with signed
   original-parent real cash accrued under the elected convention,
   including its first-ex calculation and rounding where required.
   Direct real dirty uses no unused accrued or index lookup. The real
   quote cash excludes index growth, principal protection and nominal
   forecast amounts. Preserve independent raw nominal holder PV,
   native nominal settlement cash and the real quote. Conversion
   between indexed and real settlement requires the exact issue
   bridge, settlement index ratio and mandated roundings, including
   separate clean and accrued rounding when required. A simple
   division of arbitrary nominal PV does not give a real quote.
   [Accepted indexation and original parents](04-inflation-indexation.md#accepted-decision-for-checked-inflation-linked-bond-indexation-and-floors),
   [bond settlement](../valuations-bonds/02-settlement-and-clocks.md),
   [real and adjusted settlement in US Appendix B](https://treasurydirect.gov/files/laws-and-regulations/auction-regulations-uoc/31-cfr-part-356.pdf).

2. **Elect an actual dated real-yield law.** Use authored cash dates,
   eligible settlement phase and the convention’s exact quote dates
   and times. Preserve stub/quasi-coupon slots, complete original
   coupon parents and intervening grid even where a coupon is
   detached or its amount is zero. Use accepted ICMA reference
   slices and complete coupon-parent ACT/365L context where either
   law applies. Payment lags do not extend the coupon clock. Admit
   entitled immediate cash at factor one only on a qualified quote
   branch. A future payable row with a nonpositive quote time needs
   a separately admitted factor. Keep unsupported date or lifecycle
   geometry unavailable. The quote driver, sovereign label, coupon
   tenor or requested metric cannot select a default convention.

   | Elected quote profile | Dated factor and scope |
   |---|---|
   | Analytical real periodic or continuous | Use authored exact quote times with explicit annual compounding and positive integer quote frequency for periodic. No sovereign market label is inferred. |
   | U.S. Treasury TIPS auction | For the supported regular or qualified reopening Appendix B profile use `D_k=1/(1+y f/2) × (1+y/2)^(-k)`, where `f=r/s` is the prescribed remaining first semiannual reference fraction and `k≥0` counts later full slots. Preserve the full first-period factor at a coupon boundary where required. This is the auction conversion; a secondary-market quote needs its own qualified law. |
   | DMO three-month index-linked gilt, real branch | Use the exact DMO buyer coupon and real accrued rules, explicit quasi-coupon grid, `D_k=(1+y/2)^(-(f+k))`, and the applicable pre-fixing branch. After the redemption-determining RPI is available under the DMO trade information rule, the DMO quote becomes a nominal yield on fixed indexed cash. That phase does not supply a market real yield or real duration. Retain any permitted analytical real quote as a separate explicit basis. |
   | Qualified French CNO annual-effective real quote | Use the unindexed gross price, actual CNO buyer entitlement and nominal schedule exponents with complete real accrual context. Annual-effective real discount is `(1+y)^(-t)` at the precise theoretical quote time. Require the issue and convention edition to match. |

   A legacy UK eight-month quote uses its own published cash and
   assumed-inflation or post-fixing nominal law and is outside this
   real quote package. For Canada RRBs and Japan JGBi, contract
   indexation alone does not select a market quote convention.
   A named market quote outside these qualified profiles is
   unavailable. No generic Street fallback or inference from the
   ordinary nominal JGB yield. Preserve real quote availability
   independently of model PV and nominal cash.
   [Accepted quote coordinates and domains](../valuations-bonds/10-quoted-yield.md),
   [U.S. Treasury rules and worked auction price](https://treasurydirect.gov/files/laws-and-regulations/auction-regulations-uoc/31-cfr-part-356.pdf),
   [DMO price and yield formulas](https://www.dmo.gov.uk/media/334d05fo/yldeqns_v4.pdf),
   [French Bond Association, §§2.3.3–2.3.6](https://cnofrance.org/wp-content/uploads/2021/05/2011_07_15-Normes-Usages-Indexed-Bonds.pdf).

3. **Share checked dirty pricing, yield inversion and local real
   duration.** For time-zero entitled cash `C0` and term-law
   factors `D_i(y)`, use `P(y)=C0+Σ C_i D_i(y)` before display
   rounding. Validate the selected factors and active yield domain.
   Net only amounts with identical factor coordinates. The real
   price-to-yield inverse requires finite positive dirty target,
   strictly positive `target−C0`,
   nonnegative positive-time net cash and at least one positive
   yield-sensitive bucket. Signed yields are admitted when their
   discount bases are admissible. Use the checked one-factor
   inverse where its exact law qualifies. Otherwise use checked
   fallible Brent on an explicit finite in-domain bracket with
   positive coordinate tolerance and work limit. Reprice the
   represented answer and verify an explicit positive absolute
   native real dirty price residual before display rounding.
   Closed or yield-insensitive prices supply no numerical yield.

   `real_duration` is the analytic modified duration
   `−P′(y)/P(y)` in years on this same prepared unindexed price.
   Center at the authorized yield or a checked inversion. Use
   positive finite repriced dirty `P(y)` before reporting rounding.
   Let `g_i=−∂log D_i/∂y`; sum `C_i D_i g_i` and divide by
   `P(y)`. Immediate cash remains in the denominator with zero
   derivative. Reuse the accepted real quote factor derivatives.
   In the TIPS auction profile,
   `g_k=(f/2)/(1+y f/2)+(k/2)/(1+y/2)` with inactive terms omitted.
   Continuous uses `g=t`, and nominal periodic uses
   `g=t/(1+y/m)`. Hold cash, entitlement, settlement and the clock
   fixed. Remove the one-bp default and price floor from duration.
   The result is a quote derivative, separate from nominal curve
   DV01 or an exact one-bp reprice. Compute with checked raw
   arithmetic and publish the actual elected convention and basis.
   [Checked yield price and residual](../valuations-bonds/10-quoted-yield.md),
   [analytic quoted-yield risk](../valuations-bonds/14-yield-risk.md),
   [checked Brent](../core-numerics/brent-root.md).

4. **Compare an explicitly identified nominal coupon-bond yield.**
   For `breakeven_inflation` require an independently authorized
   nominal benchmark bond YTM on comparable currency, issuer
   and credit basis, valuation/information cut, settlement phase
   and positive remaining economic horizon. Carry the two
   bond and quote identities, the actual horizons and the
   qualification of their comparability, cash and compounding
   conventions. No automatic curve-zero extraction, nearest
   maturity, benchmark switching or zero/one-year fallback.
   Require both quoted rates to admit an explicitly qualified annual
   growth conversion. In log growth units, nominal periodic at
   exact positive integer `m` uses `g=m log1p(y/m)`, continuous
   uses `g=y`, and effective annual uses `g=log1p(y)`.
   A qualified semiannual quote keeps that stated annualization
   independently of a fractional-period pricing branch. An
   unsupported simple, legacy assumed-inflation or nominal-only
   real-leg quote does not supply an annual real growth term.
   A zero economic horizon cannot establish comparability even
   if cash remains unpaid. Require finite inputs, admissible
   positive gross growth and a representable output. Compute
   `expm1(g_nominal−g_real)` as a signed effective annual decimal.
   The result is compensation implied by those two yield quotes,
   including any liquidity, coupon timing, lag, index and
   deflation-protection basis differences. A matched zero-coupon
   inflation curve and a horizon-P&L breakeven are separate measures.
   [Bank of Canada coupon benchmark example](https://www.bankofcanada.ca/rates/indicators/key-variables/inflation-indicators/),
   [matched rates by quote family](https://www.federalreserve.gov/pubs/feds/2008/200805/200805pap.pdf),
   [checked annual compounding](../core-numerics/compounding.md),
   [horizon breakeven](../valuations-framework/carry-theta-breakeven.md).

5. **Require an explicit basis for the index-ratio metric.**
   Use an authored contractual reference date or the admitted
   settlement quote date. Keep that date independent of valuation
   and knowledge cut and preserve the issue’s original denominator.
   Publish a positive unfloored dimensionless ratio with the
   resolved reference and fixing provenance. An observed result
   applies the issue’s precision. A live result is the labeled
   unrounded projection-based ratio at that reference and needs
   a qualified projection, without a payment valuation being
   inferred. A protected payment multiplier and a breakeven
   inflation rate are distinct outputs. Remove the use of
   `as_of` as both requested reference and information cut.
   [Accepted bond reference and floor law](04-inflation-indexation.md#accepted-decision-for-checked-inflation-linked-bond-indexation-and-floors).

6. **Shock the actual projected annual CPI-growth inputs.** Use the
   direct additive annual growth shock of CORE-MARKET-10 and
   RATES-13. An original pillar with time `t>0`, level `I_i`
   and source anchor `I_0` has gross growth
   `g_i=(I_i/I_0)^(1/t)`. With annual decimal shock `b`,
   rebuild `I'_i=I_0(g_i+b)^t` on the original native source
   clock, preserving the origin, interpolation, tails and
   identity. Require strictly positive attained gross rates.
   Move only projected anchors of the actual active projection
   binding. Hold the original fixing snapshot, observed values,
   contractual base, release facts, entitlement, discount factors,
   selected floor quote and its sample coordinates fixed. Recompute
   expected indexed cash and the live floor forward consistently
   on each side, applying the same qualified risk basis and cutoff.
   Verify the full directed changes and both scenario admissions.
   Expired or fixed CPI rows can still have nominal discount risk.
   A missing active source, invalid direction or failed reprice
   is an error. A fully fixed admitted ledger with no CPI
   dependence has structural zero CPI risk, even if other
   market curves are present.
   [Accepted direct CPI shock](../core-market/inflation-projection.md#accepted-decision-explicit-cpi-levels-and-anchored-annual-parameter-shocks),
   [inflation swap risk convention](../valuations-rates/13-inflation-swaps.md).

7. **Keep CPI curvature, nominal rates and attribution in their
   declared units.** Use explicit positive calculation steps, raw
   OFI-04 holder reprices and the comparable raw base. With
   represented annual decimal offsets `+a,−c` and `a,c>0`,
   report `inflation01=(V+−V−)/(10000(a+c))` in signed native
   currency per +1 bp. Report `inflation_convexity` as
   `2[(V+−V0)/a−(V0−V−)/c]/(a+c)` in native currency per
   squared annual decimal rate without PV normalization. Remove
   the tiny-PV zero shortcut and use checked differences before
   Money or display rounding. Retain computed zero, structurally
   absent exposure and invalid experiment distinctly.

   Nominal parallel DV01 and exact key-rate buckets move the
   designated active nominal discount source in continuous zero
   rates on its own clock. Hold CPI sources, selected volatility
   and actual cash/entitlement basis fixed. Nominal factor risk is
   signed currency per positive bp even for a known unpaid cash
   claim. Quote replay, floor-vol risk, cross risk and real-curve
   risk need their own admitted experiment. Use the signed
   attribution conversion only for a verified common move of the
   same opening CPI projection basis. For `Δi` in annual decimal
   units, the curvature P&L term is
   `0.5 × inflation_convexity × (Δi)^2` and the first-order term
   is `inflation01 × (10000 Δi)`. Missing data, revisions
   and twists cannot supply the common parallel move. Reconcile
   coverage and residual under the attribution owner.
   [Shared finite-difference rate risk](../valuations-framework/finite-difference-rates.md),
   [comparable market-change rows](../core-market/market-change-measurement.md),
   [original inflation instrument formulations](https://quant.opengamma.io/Inflation-Instruments-OpenGamma.pdf).

8. **Use canonical Rust preparation, risk and result metadata.**
   Reconcile real yield, real duration, breakeven, index ratio and
   nominal/CPI risks in direct and registry routes. Quote-only
   results resolve only required sources. Require nominal
   market inputs only for a selected path that consumes them.
   Share the accepted real schedule and raw selected nominal
   valuation with the applicable consumers. Preserve model,
   quote authority, denomination, information and settlement phases,
   axis and units, actual shocks, residuals and availability.
   Reconcile aggregation, attribution, existing Python/WASM
   envelope and metric routes, generated declarations/schemas
   and UI without duplicating calculation policy. Preserve
   independently available results when another calculation is
   unsupported. A failed required lookup or reprice does not
   masquerade as zero or a missing optional column.
   [Metric orchestration and reporting](../valuations-framework/metric-orchestration.md).

## Observed behavior

### Choice and rationale

The [real-yield path](../../../../finstack-quant/valuations/fixed-income/src/inflation_linked_bond/types.rs) uses unindexed cashflows and clean price per 100 **real face**; model PV separately uses indexed nominal cashflows. [Metrics](../../../../finstack-quant/valuations/fixed-income/src/inflation_linked_bond/metrics/mod.rs) keep real-yield risk and projected-inflation risk distinct. The compounding reconciliation is documented, but the nominal reference chosen for metric breakeven remains a methodological choice.

### Algorithm

1. Construct a real schedule with coupon `face × real_coupon × period_fraction` and unindexed principal at adjusted maturity. Retain coupon-period metadata for accrual and Actual/365L denominator handling. Real-yield preparation selects payment dates on/after `as_of`, but the current yield price summation excludes zero-time rows. Nominal PV excludes same-day payments.
2. Add real accrued to `clean_percent × face/100`. Solve the bond Street-compounding YTM using contractual frequency and dated flow times. Missing/nonpositive quote, no remaining cashflows, invalid timing or solver failure propagates as error.
3. Real duration solves base real yield, reprices real flows at ±yield bump (default 1 bp, caller-positive override), and returns `−(P_up−P_down)/(2h P_dirty)`. Denominator is model dirty price per 100, floored at `1e-6`, not clean quote.
4. Breakeven converts real Street yield to effective-annual yield and computes `(1+nominal_annual)/(1+real_annual)−1`. Direct callers supply a comparable nominal annual yield; the metric instead infers the **zero yield to legal maturity** from the nominal curve `DF^(−1/T)−1`, not a matched coupon-bond yield. The conversion horizon falls back to one year on day-count failure and is floored at `1e-6`; unsupported noninteger frequency is only debug-asserted before rounding.
5. [Inflation01](../../../../finstack-quant/valuations/fixed-income/src/inflation_linked_bond/metrics/inflation01.rs) shifts the inflation curve ±.01 percent (±1 bp annual inflation-rate shock) through the shared bump API and returns half the currency PV difference. When only a published index exists, validate base pricing and leave market unchanged, giving zero projected-inflation risk.
6. InflationConvexity uses the same ±1 bp shock and reports `(P_up+P_down−2P0)/h²` in currency per squared decimal rate, without dividing by PV. Near-zero base PV yields zero. Parallel/bucketed nominal DV01 delegates to shared curve repricing.

### Scope, limitations and alternatives

Real yield is a quote convention, not a stochastic real-rate model. The metric breakeven blends a nominal zero yield with a coupon-bond real yield; it need not equal a market matched-maturity bond breakeven or inflation-swap rate. The inflation bump affects projected anchors; already published fixings are held fixed. Floor optionality may make inflation risk nonlinear. Zero-bond, matched-cashflow and liquidity/seasonality-adjusted breakevens are unimplemented alternatives. Silent conversion-time fallback and frequency rounding require an explicit acceptance decision.

### Evidence

Colocated `breakeven_inflation_uses_consistent_annual_compounding_c8`, `real_duration_uses_dirty_model_price_denominator`, `act_act_isma_real_schedule_and_accrual_need_no_day_count_override`, and `act365l_real_yield_and_duration_retain_coupon_periods` test identities and conventions. [Yield integration cases](../../../../finstack-quant/valuations/fixed-income/tests/inflation_linked_bond/test_yields.rs) and [duration cases](../../../../finstack-quant/valuations/fixed-income/tests/inflation_linked_bond/test_duration.rs) were located, not run. [Inflation reference](../../../REFERENCES.md#deacon-derry-mirfendereski-2004) supplies context.

## Independent arithmetic and qualification

- The Treasury Appendix B reopening with remaining half-period
  fraction `.5`, coupon `3.625` per 100, real yield `.0365` and
  18 subsequent periods gives unrounded real dirty
  `100.7032665544592` and clean `99.7970165544592`. With
  authorized six-decimal rounding, accrued `.906250` and
  ratio `1.01074`, indexed settlement is `101.784820`.
  The local real duration is `7.7305756749895` years.
  A quote for just `101` due in one quarter year at yield `.04`
  gives price `100.0049018406441` and duration `.2450980392`
  under ordinary semiannual compounding, or price `100` and
  duration `.2475247525` under the simple factor.
- For the AFT OAT€i July 2043 reopening settling 1 April 2025,
  the CNO annual-effective profile at real yield `1.676%` gives
  unrounded clean `88.6297901243`, matching the published `88.63`
  at the quoted precision.
  [AFT reopening result](https://www.aft.gouv.fr/fr/publications/communiques-presse/25-mars-2025-resultat-reabondement-par-syndication-loateui-095-25).
- An effective annual nominal yield of `8%` and nominal semiannual
  real yield of `4%` give breakeven `3.8062283737%`. Treating
  the latter as effective annual raises the result by about
  `3.9925472 bp`. Continuous nominal `1.4%` and real `1.8%`
  give an effective annual result of `−0.3992010656%`.
- With notional `1000`, discount `.95`, fixed quarter weight
  on ratio `.98` and projected three-quarter weight on a
  two-year pillar of gross `1.02`, a one-bp shock on both sides
  gives raw PVs `973.889657125`, `974.035` and
  `974.180357125`. Inflation01 is `.14535` and curvature is
  `1425` per squared decimal rate. An otherwise identical
  position scaled by `10^-16` has nonzero curvature
  `1.425×10^-13`. For nominal cash `1030` fixed and unpaid
  in two years with continuous nominal `.02`, CPI risk is
  zero and signed nominal central 1-bp DV01 is approximately
  `−.1979226258`.

Standalone calculations check these numerical cases.
Qualification still requires primary settlement examples across
issue profile, lifecycle and ex-coupon boundaries, comparable
nominal benchmark evidence, stable raw price/risk and held-source
fixtures, and native/registry and host agreement. Repository
pricing and host suites were not run for this review.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
