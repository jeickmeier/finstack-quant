# VALUATIONS-RATES-17-CMS-SPREAD: CMS spread conditional lognormal integration

- ID: VALUATIONS-RATES-17-CMS-SPREAD
- Decision status: **Accepted — complete checked CMS spread cashflows, model and risk package, 2026-11-14**.
- Implementation status: Runtime and caller reconciliation pending. Observed behavior below describes the current implementation.
- Validation status: Source, published references and independent arithmetic reviewed. Runtime, market and host qualification pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked CMS spread cashflows, model and risk

1. **Specify the ordered payoff and cash scale.** Support one
   holder-long, same-currency European option on `L−S`, the
   longer CMS benchmark less the shorter one. Require exact
   positive month/year tenors with longer tenor strictly greater
   than shorter tenor, explicit call or put and signed Decimal
   strike `K` in decimal-rate units. Use positive exact Money
   cash scale `N` per unit of decimal rate and `η=+1` for a
   call or `−1` for a put. Cash on determination is
   `N max(η(L−S−K),0)`. The supported standalone claim has
   one dated payment and a unit accrual multiplier. Remove the
   ignored option day-count field. A periodic CMS spread coupon
   requires its own accruing notional and clock, and a strip,
   premium or other payout belongs to its own product terms.
   [CMS spread payoff](https://quant.opengamma.io/CMS-Spread-Pricing-OpenGamma.pdf)
   and [ICE CMS spread options](https://idd.ice.com/IRHelp/Content/FM/CMS_Spread_Option.htm).

2. **Bind the two benchmarks, their reference swaps and the
   common payment basis.** Use two exact distinct CMS fixing
   identities and an explicitly qualified common contractual
   fixing event. Resolve each benchmark's publication authority,
   reference IRS dates and settlement calendar, actual fixed and
   floating grids, projection and reference-discount roles under
   record 11. The grids and sources can differ. Market bindings
   can be shared only if qualified in both roles. Keep benchmark
   fixing, reference swap start, live quote expiry and actual
   option payment distinct. Use one compatible payment basis and
   its exact dated native discount source. Remove curve-derived
   fixing IDs and currency or name based benchmark defaults.
   [Accepted CMS benchmark and reference terms](11-cms-convexity.md#accepted-decision-for-checked-cms-convexity-pricing-and-risk).

3. **Price the claims allowed by the knowledge and cash cuts.**
   Use the exact eligible published fixing on its admitted
   date and phase, including a same-day published fixing.
   Missing due observations error. An expressly pending live
   observation needs a qualified positive remaining interval
   and matching model/quote inputs. Date equality alone never
   turns a projection into a fixing. When both fixings are
   known, including negative rates, use literal payoff with
   no live reference, volatility or correlation lookup. Where
   exactly one is published, admit the single live marginal
   only if qualified on the same payment basis and conditional
   on the actual information, including the other fixing.
   A stale unconditional quote does not qualify. Respect the
   authorized determination cutoff. Exclude same-day cash on
   the ordinary holder cut. Include entitled known same-day
   cash at factor one on an explicit cut. Keep fixed future
   cash through actual payment, skip inactive lookups for
   extinguished claims and require qualification for overdue
   unpaid claims.
   [Publication and expiry clocks](../conventions/0045-financial-clocks-and-date-boundaries.md)
   and [cashflow valuation](../valuations-framework/cashflow-export.md).

4. **Select the actual live model and each marginal quote.** Use
   explicit `CmsSpreadLognormal` (`cms_spread_lognormal`)
   for the conditional two-lognormal model. Remove its
   `StaticReplication` registration and implicit model choice.
   Each live rate starts with its exact reference forward `F>0`.
   Require a compatible unshifted annual Black quote bound to the
   exact benchmark, reference tenor, forward, origin, clock and
   supported coordinates. Admit a deliberately strike-independent
   bound ATM source, or an explicitly strike-flat source verified
   exactly across its supported evaluated strike slice under its
   admitted interpolation. One ATM quote on a smile or near-equal
   quotes after flooring do not qualify. Require a qualified
   dated aging path for another origin and a variance-preserving
   bridge when the source and model clocks differ. Remove quote
   clamping, min-rate and min-vol floors. Admit exact zero sigma.
   Keep Normal, shifted Black, SABR, other smile maps, and record 12
   replication outside this joint quote model.
   [Volatility binding and clocks](../valuations-framework/model-resolution.md)
   and [CMS replication owner](12-cms-replication.md#accepted-decision-for-checked-cms-smile-replication-and-risk).

5. **Define both payment-measure proxy means and the dependence.**
   For each live leg use record 11's first-order dated flat-yield
   correction `m=F+ℓ(F) F² σ²T` on its own actual reference fixed
   grid and signed delay to this payment. Require finite positive
   `m` and checked total log standard deviation `s=σ√T` on the
   admitted quote interval. The live marginals use these proxy
   means in one proposed payment-measure law. With independent
   standard normals `Z,Y` and explicitly authored latent
   correlation `ρ∈[−1,1]`, use
   `L=m_L exp(s_L Z−s_L²/2)` and
   `S=m_S exp(s_S(ρZ+√((1−ρ)(1+ρ))Y)−s_S²/2)`.
   Correlation is between the Gaussian drivers. It is not the
   Pearson correlation of rate levels, an inferred market
   correlation or a calibrated smile. The first-order means
   and imposed dependence need payment-measure and market
   qualification. Separate marginal qualification does not
   establish joint smile or stochastic covariance consistency.
   [Dated CMS mean correction](11-cms-convexity.md#accepted-decision-for-checked-cms-convexity-pricing-and-risk),
   [Carmona and Durrleman on lognormal spreads](https://carmona.princeton.edu/sites/g/files/toruqf5466/files/documents/spread.pdf)
   and [OpenGamma on CMS payment measures](https://quant.opengamma.io/CMS-Spread-Pricing-OpenGamma.pdf).

6. **Use conditional Black and literal degenerate laws.** Given
   `Z=z`, let `h=L(z)−K`,
   `M=m_S exp(ρs_S z−(ρs_S)²/2)` and
   `w=s_S√((1−ρ)(1+ρ))`. For `h>0` the conditional spread
   call is the unit Black put on `M` at strike `h` and
   total width `w`. The conditional spread put is the Black
   call. For `h≤0` they are exactly zero and `M−h`.
   At zero conditional width use pointwise intrinsic before
   integrating, including at exact `ρ=±1`. Correlation is
   active only with two stochastic marginals. If just the
   long rate is stochastic, use a Black call or put at
   `K+S_fixed` when positive. At a nonpositive threshold
   the call is linear and the put is zero. A single stochastic
   short rate uses the conditional put/call mapping directly.
   Two deterministic rates give intrinsic. A proved constant
   spread under the live law also uses literal intrinsic.
   Exact zero live vol keeps modeled status. For zero strike
   and two live stochastic marginals use the exact exchange
   formula, Black on `m_L,m_S` with total width
   `hypot(s_L−ρs_S,s_S√((1−ρ)(1+ρ)))`.
   [Checked unit option kernels](../models-analytic/forward-normal-shifted.md)
   and [Carmona and Durrleman on the exchange case](https://carmona.princeton.edu/sites/g/files/toruqf5466/files/documents/spread.pdf).

7. **Gate the remaining integration and its tails separately.**
   Evaluate the conditional payoff against the standard-normal
   density on explicitly supplied finite `a<b`. Use accepted
   fallible adaptive 16-node Gauss–Legendre. Require separate
   absolute tail and quadrature estimator budgets in
   undiscounted unit payoff, explicit max depth and a single
   total quadrature evaluation limit across all panels.
   Split at the interior threshold `h=0` when applicable,
   using the checked log ratio. At `ρ=−1` with both widths
   positive the spread is strictly increasing. At `ρ=+1`
   with unequal positive widths it has at most one stationary
   point. Equal widths give a signed scaled lognormal or a
   constant. For the nonconstant endpoint cases, partition
   at any interior stationary point into monotone pieces and
   locate any bracketed exercise crossing with the accepted
   checked bounded solver and explicit root tolerances and work
   limits. Count numerical work for each operation. These
   breakpoints guide sampling and omit no interior area.
   Allocate the estimator budget across panels deterministically,
   require the checked summed estimate to pass and account for
   actual total work. Check the omitted call tail against
   `m_L T(s_L)+max(−K,0)T(0)`, and the put tail against
   `m_S T(ρs_S)+max(K,0)T(0)`, where
   `T(u)=Φ(a−u)+Φ(u−b)`. Use stable normal tails and checked
   scaling. Require a conservative represented upper bound
   for the tail gate. Refuse unresolved breakpoints and
   collapsed required positive widths. A tail bound and an interior estimator describe
   different evidence. Report both with controls and work.
   A failed bound, unusable calculation or exhausted required
   work errors. Remove fixed 20-node pricing and automatic
   order, model or budget substitution. Use checked arithmetic
   at every sample and preserve the requested payoff's
   nonnegative value without clipping or inferring a small
   premium from an unstable parity subtraction. A zero unit
   payoff requires proved support or intrinsic. With two
   positive widths and `|ρ|<1`, the full support makes both
   payoffs strictly positive for every finite strike. A sampled
   zero is therefore unresolved on that branch. Apply the checked
   bounds `C≤m_L+max(−K,0)` and `P≤m_S+max(K,0)`.
   Qualify put-call parity `C−P=m_L−m_S−K` on matched inputs.
   [Adaptive quadrature](../core-numerics/quadrature.md),
   [bounded Brent](../core-numerics/brent-root.md)
   and [normal functions](../core-numerics/distribution-functions.md).

8. **Use one prepared payoff for PV, diagnostics and export.**
   Produce one native dated eligible cash contribution equal to
   `N` times the known or modeled unit payoff. Apply its
   payment discount once and restore Money at the final
   price boundary. Raw risk reprices the same admitted payoff.
   Report both benchmark identities and their observation status.
   For modeled legs retain the actual forward, adjustment, mean,
   quote basis and clock that were evaluated. Report admitted
   latent correlation only where active. Historical rates are
   observed fixings. Settled or inactive metrics carry no invented
   zeros, dates or volatilities. Allow cash export for the
   exact registered model with its complete decomposition.
   Label known payment and modeled expected cash distinctly.
   Remove the placeholder on supported paths and reconcile
   raw and final PV. An unsupported model decomposition remains
   unavailable.
   [Cashflow export](../valuations-framework/cashflow-export.md)
   and [metric provenance](../valuations-framework/metric-orchestration.md).

9. **Bind risk and all existing consumers to the same model.**
   Reprice the admitted raw cash basis with explicit rate steps,
   keeping observations, latent correlation and native quote
   sources and numerical controls fixed. Rebuild each affected live reference
   forward and mean and retain the actual quote coordinate
   convention. Publish checked combined `dv01`, typed
   per-source `pv01` and triangular `bucketed_dv01` in native
   currency per positive bp. Keep reference projection,
   reference discount and payment discount roles explicit.
   A known unpaid amount can retain payment discount risk.
   Leave vega, correlation risk, mixed Greeks, implied
   correlation and quote replay outside this initial
   registration. Attribution may use only a matched experiment
   on the actual source basis. Reconcile the Rust model keys,
   registry, error and trace paths, tagged envelope, existing
   Python and WASM routes and generated schema consumers.
   Qualify flat volatility, publication and settlement, correlation
   endpoints, deep strikes, tail and integration controls,
   large/small variance and holder risk independently.
   [Rate risk](../valuations-framework/finite-difference-rates.md),
   [CMS spread wire](../../../../finstack-quant/valuations/schemas/instruments/1/rates/cms_spread_option.schema.json)
   and [WASM pricing caller](../../../../finstack-quant-wasm/src/api/valuations/pricing.rs).

## Observed behavior

### Choice, scope and rationale

The CMS spread pricer is registered as `StaticReplication`, but its actual algorithm is one-dimensional Gaussian integration of a conditional two-lognormal payoff. It does not replicate an arbitrary pair of smiles. It accepts unshifted Black **flat-strike** marginal surfaces and an explicit Gaussian correlation in `[-1,1]`; SABR and nonflat smiles are rejected.

### Algorithm and controls

1. Resolve both CMS reference swap forwards, historical fixings and conventions as in [CMS conventions](11-cms-convexity.md). If payment is settled, return zero. Past expiry uses observed long/short CMS fixings and deterministic spread intrinsic; expiry exactly today still resolves forward/vol inputs before intrinsic.
2. For each future leg, obtain ATM Black vol, require positive finite forward/vol, and add the same annuity-weight convexity correction used by the approximate CMS pricer. Validate adjusted forward positive. Flatness checks strike-node vols at the requested expiry to `1e-12`; tenor-axis surfaces are naturally strike-flat. Positive vols below `1e-8` are floored to `1e-8`.
3. Treat adjusted rates as lognormal means. With standard normal z for the long rate, `L=mean_L*exp(s_L*z-s_L^2/2)`. The short conditional mean is `mean_S*exp(s_S*rho*z-(s_S*rho)^2/2)`, and conditional log standard deviation is `s_S*sqrt(1-rho^2)`.
4. Conditional on L, a spread call is a Black put on short rate with strike `L-K`; the spread put is a Black call. When `L-K<=0`, use zero for the spread call and `mean_short-(L-K)` for the put.
5. Integrate this conditional payoff with a fixed 20-point Gauss–Hermite rule and multiply by notional and payment discount. No separate coupon-accrual multiplier is applied. Nonfinite/negative expected payoffs error.

### Selection, risk and limitations

The default model and only local pricing registration are StaticReplication; quadrature order is a private default with no public numerical override. Registered curve risks are combined and triangular bucketed DV01; the result also reports both forwards, both convexity adjustments, adjusted spread and correlation. The implied joint distribution uses the supplied Gaussian correlation without calibration. Marginal means are approximated payment-measure means, and covariance/smile-consistent payment-measure mapping is absent. Correlation endpoints reduce conditional volatility to zero.

### Alternatives and tradeoffs

Kirk, normal-spread pricing, Monte Carlo with joint dynamics, and smile-mapped copula integration are unimplemented alternatives. Conditional Black integration removes the inner payoff kink and lowers integration dimension, but this dimensional reduction does not establish fixed-order accuracy near correlation endpoints or deep strikes.

## Evidence

- [Spread pricer and inline tests](../../../../finstack-quant/valuations/rates/src/cms_spread_option/pricer.rs), [validation and selection](../../../../finstack-quant/valuations/rates/src/cms_spread_option/types.rs), [risk registration](../../../../finstack-quant/valuations/rates/src/cms_spread_option/metrics.rs): observed implementation and local behavioral evidence.
- [Black](../../../REFERENCES.md#black-1976), [Hagan CMS](../../../REFERENCES.md#hagan-2003-cms-convexity), [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo): relevant constituent methods. A canonical reference and independent validation for this precise combination were not established.

### Reconciliation evidence

The fully fixed path currently resolves both volatility sources
before it reads the observations. Inactive and fixed pricing can
publish artificial zeros or label observed rates as forwards.
The dependency declaration assigns the spread strike to each
reference swaption quote, while the live price samples its own
reference forward. The cashflow export path is a placeholder.
These paths need one preparation and state-aware dependencies.
[Current pricing and diagnostics](../../../../finstack-quant/valuations/rates/src/cms_spread_option/pricer.rs)
and [dependency and export declarations](../../../../finstack-quant/valuations/rates/src/cms_spread_option/types.rs).

### Independent review evidence

- Known long `−.005`, short `−.010`, call strike `.0025`
  and cash scale one million give cash `2,500` and PV
  `2,400` at factor `.96`. At a two-year payment source
  coordinate, the centered ±1 bp discount reprice gives about
  `−.4800000032` currency units per bp. An explicitly entitled
  known payment on the valuation date has factor one.
- With long observed at `.03`, strike `.01`, and a qualified
  conditional short marginal with mean `.02` and width `.2`,
  the spread call reduces to an ATM Black put on the short
  rate. It is worth `1,545.32008635` for scale one million
  and factor `.97`. If long is observed at `.01` and strike
  is `.015`, that positive-support short law gives exact call
  zero and spread put PV `24,250` at the same scale and factor.
- With means `.04,.03`, widths `.30,.20`, `ρ=.40` and
  strike zero, the exact exchange law gives unit call
  `.0108138840021531` and put `.0008138840021531`. The
  level correlation under this law is about `.39181370148`.
- With means `.04,.02`, widths `.25,.25`, `ρ=1` and
  strike `.02`, the spread is a single lognormal with mean
  `.02` and width `.25`. Exact unit call value is
  `.001989528993204514`. The current 20-node rule gives
  `.002014573330824295`, about `1.2588%` high, a difference
  of `25.04` at cash scale one million and unit discount.
- With means `.04,.025`, widths `.30,.20`, `ρ=.40`,
  and strike `.01`, independent composite Simpson on
  `[−9,9]` using both 65,536 and 131,072 panels gives
  unit call `.00708429594868516` and put
  `.00208429594868516`. Their difference is `.005`.
  With `ρ=.999999`, the same reference gives call
  `.00567786922940347` and put `.00067786922940347`. These
  checks qualify arithmetic under the specified distribution.
  Market fit and production numerical qualification remain pending.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
