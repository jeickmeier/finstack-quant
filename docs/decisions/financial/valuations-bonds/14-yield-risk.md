# VALUATIONS-BONDS-14: Quoted-yield duration, convexity and signed yield DV01

**Decision status:** Accepted — complete checked quoted-yield bond risk package, including unscaled convexity, 2026-11-14.
**Implementation status:** Runtime and caller reconciliation pending. Observed behavior is recorded below.
**Validation status:** Source, primary reference and independent arithmetic reviewed. Runtime, market and host qualification pending.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked quoted-yield bond risk

1. **Bind one settlement yield experiment.** Prepare the buyer cash
   and authorized payment phase at actual settlement, on the frozen
   valuation and information cuts, with the elected BONDS-10 quote
   law, exact quote coordinates, currency and position amount. Use
   ordinary scheduled maturity on an admitted straight bond. On an
   optional bond, require either an expressly elected scheduled
   maturity scenario assuming no voluntary exercise or one explicitly
   selected, independently qualified fixed exit. Preserve its full
   contractual identity, rights and actual cash dates. A path dependent
   on the draft workout record remains unavailable until qualified.
   Remove the clean-price trigger and default workout selection for
   these measures. Hold scenario, exercise, entitlement, cash amounts,
   projected coupons, fixings and quote clock fixed when differentiating.
   A convention-qualified frozen projected strip is identified as such.
   [Bond settlement and clocks](02-settlement-and-clocks.md),
   [quoted-yield law](10-quoted-yield.md) and [workout ownership](11-workout-yield.md).

2. **Use one authoritative yield center and the corresponding dirty
   price.** Accept an explicit admitted yield on that experiment or
   invert the authorized native settlement dirty amount using the
   BONDS-10 monotone domain and returned-yield residual rules. Convert
   clean through signed accrued only when needed. A separately
   qualified price conversion, including a qualified ASW, Z, DM or OAS
   input, can supply the dirty amount on the same identified claim.
   A selected model can supply a price through an expressly elected
   noncircular settlement financing and buyer-claim bridge. Keep the
   source observation, any derived settlement amount, the repriced
   dirty center `P(y)` and selected holder model PV identifiable.
   Use `P(y)` before reporting rounding where a metric has a price
   denominator. Do not resolve a maturity YTM for a supplied fixed
   workout yield or restamp a derived quote as an observation. A signed
   strip admitted for direct pricing may be differentiated at a supplied
   yield even when its price-to-yield inverse is unavailable.
   [Model and quote authority](03-pv-models.md),
   [spread conversions](12-spread-quotes.md) and
   [asset-swap conversions](13-asset-swaps.md).

3. **Differentiate the dated factors actually used by the quote.**
   For each BONDS-10 prepared bucket let `PV_i=C_i D_i(y)`,
   `g_i=−∂log(D_i)/∂y`, and `h_i=∂²log(D_i)/∂y²`. Keep cash
   identities and amount-net only at identical quote-law coordinates.
   Let `C0` be buyer-entitled immediate cash at factor one with
   zero quote time and derivatives. Calculate
   `P=C0+ΣPV_i`, `P′=−ΣPV_i g_i`, and
   `P″=ΣPV_i(g_i²+h_i)` with respect to annual decimal `y`.

   | Elected factor branch | Quote time for the cash moment | `g_i` | `h_i` |
   | --- | --- | --- | --- |
   | Nominal periodic, including regular U.S. Street | `t` from the prepared quote | `t/b` with `b=1+y/m` | `(t/b)/(m b)` |
   | Continuous | `t` | `t` | `0` |
   | Simple, including the U.S. Street final simple branch | `t` or the elected final `t_f` | `u=t/(1+y t)` with the branch time | `u²` |
   | Moosmüller | `w+k/m` | `u+v`, where `u=w/(1+y w)` and `v=(k/m)/b` | `u²+v/(m b)` |

   Select Street's final branch from the contractual interval.
   Retain Moosmüller's next contractual anchor even if the coupon
   is zero or detached. When `k=0`, omit the entire periodic term
   and its base-domain test for that bucket. Apply each active
   branch's checked yield domain. A future payment at a nonpositive
   quote coordinate requires the owning convention to provide its
   admitted differentiable factor and explicit quote time for the
   affected measure. Japanese simple yield is a separately identified
   clean-price conversion outside this dated dirty risk profile.
   [OpenGamma yield duration and convexity, section 8](https://quant.opengamma.io/Bond-Pricing-OpenGamma.pdf#page=5)
   and [checked compounding](../core-numerics/compounding.md).

4. **Publish four fixed yield measures in explicit units.**

   | Metric | Definition and basis |
   | --- | --- |
   | `duration_mac` | `Σ t_i PV_i/P` in quote years. Weights are signed on an admitted signed stream. |
   | `duration_mod` | `−P′/P` per annual decimal yield, ordinarily expressed in years. |
   | `convexity` | `P″/P` per squared annual decimal yield, ordinarily expressed in years², with no `/100` scale factor. |
   | `yield_dv01` | `10⁻⁴ P′`, signed native currency on the whole position for an upward one-bp yield direction. |

   The local relative price approximation is
   `ΔP/P ≈ −duration_mod Δy + 0.5 convexity (Δy)²` for `Δy`
   in annual decimal. This explicitly changes the published bond
   convexity magnitude. A named presentation transform can declare
   another scale and retain the canonical result's identity. Nominal
   periodic gives `duration_mod=duration_mac/b` and continuous gives
   equality. Derive the other branches directly. Signed weights can
   make normalized moments negative or place Macaulay outside the
   range of quote times. Yield DV01 is a local derivative in bp units.
   A finite one-bp full reprice is a different result and may be
   outside the factor domain even when the derivative is finite.

5. **Keep zeros, availability and numerical failures exact.**
   Require strictly positive finite raw `P` for normalized metrics.
   The direct currency tangent has no price denominator. It can
   use an admitted signed forward price at an explicit yield even
   when the normalized values are unavailable. An explicitly
   centered claim consisting solely of positive net entitled
   immediate cash has zero moments and derivatives. Inversion
   of such a price does not identify a yield. An empty or extinguished
   claim has no yield measure. Check needed quantities and the
   actual represented result for each requested component. Allow
   true zero from insensitivity or cancellation. Remove arbitrary
   near-zero price, rate-base and frequency repairs or floors. Use
   checked factors, stable operations and checked compensated signed
   accumulation. Scale or regroup products as needed to avoid
   intermediate range loss. Fail on an unrepresentable required
   value or lost nonzero contribution that cannot be recovered.
   A valid tangent need not compute normalized duration or a second
   derivative. Keep intermediates before quote or Money display
   rounding and use checked amount and rate conversions.
   [Checked compounding](../core-numerics/compounding.md),
   [checked sums and moments](../core-numerics/sample-statistics.md)
   and [monetary precision](../core-conventions/012-money-decimal-precision.md).

6. **Give market-curve and effective risk separate measures.**
   The `CallableOas` metric basis does not select an alternative
   calculator or model dependency for these four keys. A qualified
   OAS price conversion may still be the original price authority
   under point 2. Remove yield-price times effective-duration
   multiplication. Record 15 owns the quote-pinned model, curve
   shock and effective-duration/effective-convexity measures and
   the remaining generic `dv01` routing. Keep their dependencies,
   shock and actual repricing assumptions with that owner.
   [Bond curve and effective risk](15-curve-risk.md) and
   [shared yield and curve risk distinctions](../valuations-framework/finite-difference-rates.md).

7. **Carry basis and units through consumers.** Use one Rust
   preparation and consistent component evaluation for the metric
   registry, direct callers and quote-aware reports. Reuse a
   prepared state only for identical authority, cash path, clock,
   yield and snapshot. Report the settlement and scenario, price and
   yield basis, unit, sign, position and currency as applicable.
   Distinguish catalog registration from actual applicability and
   propagate invalid input, missing required data and failed
   numerical evaluation. Preserve independent available results.
   Normalized metrics are intensive. Apply quantity to the native
   currency tangent once. Portfolio aggregation needs an authorized
   common currency and comparable own-yield shock contract. Consume
   quoted duration in weight, TRS and attribution paths only under
   their explicitly admitted basis. Reconcile Rust, Python, WASM,
   schema, catalog and UI declarations and fixtures with the bond
   scale and units. Other products' own metrics retain their
   respective definitions.
   [Shared metric contract](../valuations-framework/metric-orchestration.md)
   and [explicit public convention changes](../valuations-framework/0010-metric-identifiers.md).

## Observed behavior

### Choice and selection

Bond risk defaults to `BulletDiscountable` (`bullet_discountable`), described in the source as the Workout profile. Model-based `CallableOas` is an explicit metric override. The default uses a clean-quoted optional bond's cached worst-workout yield and cashflows. Otherwise it uses YTM and cached settlement-entitled cashflows. The workout path is selected at the center and held fixed. Source comments cite Bloomberg YAS as implementation rationale. Vendor agreement is unverified.

### Formulation and conventions

Time is the bond yield clock from settlement (record 02), nominal yield `y` and frequency `m=max(payments_per_year,1)`. Recompute `P=Σ CF_i (1+y/m)^(−m t_i)` on positive-time flows. Macaulay duration is `Σ t_i PV_i/P`. Modified duration is `D_mac/(1+y/m)`, with near-zero denominator (`1e−12`) an error. Convexity is `Σ PV_i (m t_i)(m t_i+1)/(m+y)^2 /P/100`. The `/100` is a reporting convention: Taylor price change uses `0.5·C_reported·100·dy²`. Weighted sums use compensated accumulation. Near-zero yield price returns zero Macaulay duration and convexity.

`YieldDv01=−P·D_mod·0.0001`, in currency for an upward one-bp quoted-yield move. Long ordinary bonds normally produce negative DV01. Under `CallableOas`, modified duration and convexity of optional bonds instead use the curve-bump method (record 15), but `YieldDv01` still multiplies that effective duration by the yield-basis price. Macaulay duration never switches to a model-based measure. This mixed basis is observed behavior, not an adopted equivalence between yield and curve risk.

### Limitations, alternatives and evidence

Workout is selected once, and its switches are not differentiated. Signed/nonstandard cashflows can yield unusual duration/convexity and nonunique yields. With no explicit clean quote an optional bond's yield-risk basis remains maturity cashflows, even if model PV reflects optimal exercise. Supported alternative is `CallableOas`; continuously compounded yield derivatives or finite differences on a freshly selected workout are different unimplemented conventions.

Owning code: [shared risk basis/cache](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/mod.rs), [Macaulay](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/duration_macaulay.rs), [modified duration](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/duration_modified.rs), [convexity](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/convexity.rs), [yield DV01](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/yield_dv01.rs). [Duration tests](../../../../finstack-quant/valuations/fixed-income/tests/bond/metrics/duration.rs) inspect zero-coupon maturity, the modified/Macaulay identity, finite-difference convexity against the same yield kernel, and workout-versus-OAS selection; these are analytic/internal-consistency checks, not universal vendor agreement. Reference: [Tuckman–Serrat](../../../REFERENCES.md#tuckman-serrat-fixed-income).

Consumer evidence includes the [metric keys](../../../../finstack-quant/valuations/core/src/metrics/ids/instrument.rs),
[unit catalog](../../../../finstack-quant/valuations/core/src/metrics/ids/unit.rs),
[legacy convexity scale assertion](../../../../finstack-quant/valuations/fixed-income/tests/bond/validation/market_benchmarks.rs),
[Python override mapping](../../../../finstack-quant-py/src/bindings/valuations/pricing.rs),
[duration portfolio weighting](../../../../finstack-quant/valuations/src/instruments/composite/spec_support.rs) and
[fixed-income attribution](../../../../finstack-quant/portfolio/src/fi_attribution.rs).

## Independent arithmetic and qualification

- For an admitted U.S. Street final interval, terminal cash `105`
  at quote time `.25` and yield `.10` give `P=4200/41`, Macaulay
  `1/4`, modified `10/41`, unscaled convexity `200/1681`, and
  yield DV01 `−21/8405` in native currency. With the same simple
  factor and expressly entitled immediate cash `5` in a qualified
  core simple quote, dirty price increases to `4405/41`, modified
  falls to `8400/36121` and yield DV01 is unchanged.
- For Moosmüller with `w=.25`, `m=2`, first coupon detached,
  terminal `105` at `k=1` and `y=.10`, the contractual anchor
  gives `P=4000/41`, Macaulay `.75`, modified `620/861`,
  convexity `596600/741321`, and yield DV01 `−248/35301`.
  On an independently admitted case with only `k=0`, `w=.25`,
  `m=2`, `y=−3`, and cash `105`, the active simple factor is
  positive. `P=420`, modified is `1` and convexity is `2`.
  Failure of the unused periodic base has no effect.
- For a two-year nominal semiannual strip at yield `.10`, a
  single terminal cash amount `100` gives `P=100/(1.05)^4`,
  Macaulay `2`, modified `40/21`, convexity `2000/441`, and
  yield DV01 `−P/5250` in native currency. Doubling the position
  doubles the currency tangent and preserves duration and convexity.
- At continuous yield zero, an admitted signed strip `+150` at
  year one and `−50` at year three has `P=100`, zero Macaulay,
  modified and yield DV01, and convexity `−3`. Its price cannot
  supply a yield center through the ordinary monotone inverse.

Exact rational checks reproduce the listed arithmetic. Independent
85-digit central differences of the simple, periodic, continuous
and Moosmüller price formulas agree with their first and second
normalized derivatives on the checked in-domain inputs.

Qualify all elected factor laws with independent price derivatives
and in-domain finite-difference convergence on a fixed cash strip.
Include regular coupon grids, terminal simple, Moosmüller with a
detached anchor, negative yields near active boundaries, quote
and payment lag, identical-date claims with different coordinates,
same-day versus detached cash, signed and tiny positive price cases,
arithmetic cancellation and range failures, explicit optional-bond
scenarios and supported fixed workouts. Check quantity scaling and
the independence of the four yield keys from an effective risk
selection. Compare standalone and batched metrics and existing host
surfaces. Arithmetic references do not establish vendor agreement
or certify runtime or host results.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
