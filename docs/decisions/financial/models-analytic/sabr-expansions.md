# MODELS-ANALYTIC-13: SABR normal, lognormal and shifted asymptotic quotes

**Decision status:** Accepted — complete explicit checked SABR quote package with exact CEV distance and Hagan midpoint time correction, 2026-11-10.

**Implementation status:** Observed. Runtime, consumer, schema and host reconciliation remain open.

**Validation status:** Source, primary references and existing tests inspected. Standalone high-precision formula, domain and limit checks cover selected cases. Production quote, fit and complete-path qualification remain open.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for explicit checked SABR quotes

The complete 24-decision package was accepted on 2026-11-10.

1. **Select one asymptotic SABR quote profile.** Use the SABR
   backbone dF_t=alpha_t (F_t+s)^beta dW_t with stochastic scale
   d alpha_t=nu alpha_t dZ_t and driver correlation rho. Exact beta=0
   has the constant backbone on signed forwards. Evaluate the exact
   short-time leading term in the requested quote coordinate and the
   stated Hagan midpoint time correction. The output is an asymptotic
   European quote. Full transition, boundary and price laws have
   separate qualification requirements.

2. **Reuse canonical immutable parameter admission.** Use
   `SabrParameterData` as the checked payload of `SabrParameters`.
   Require finite alpha>0, beta in [0,1], nu>=0 and rho in (−1,1).
   Keep optional finite signed shift, including `Some(0)` distinct
   from `None`. Use one checked construction route and strict flat
   restoration with required alpha, beta, nu and rho and optional
   shift. Reject unknown and duplicate fields. Remove raw mutable
   parameter fields and contradictory model-only validation.

3. **State the model and quote units.** Alpha is in underlying units
   to the power 1−beta per square root model year. Nu is per square
   root model year. Beta and rho are dimensionless. F, K and s share
   the underlying units. T is the owner's compatible modeled expiry.
   Normal quotes are in underlying units per square root model year.
   Black and shifted Black are relative decimal volatilities per
   square root model year. Clock changes of a sigma quote alone do
   not rebase the SABR law.

4. **Require the output convention.** Use one checked
   `SabrParameters::implied_vol(forward, strike, time_to_expiry, convention)`
   returning `Result<f64>`. Require the existing `VolatilityConvention`
   in every host. Normal allows either shift presence. Unshifted Black
   requires absent shift. Shifted Black requires an explicit source
   shift and an exact matching assertion in the selected convention,
   including explicit zero. Use numeric equality with no tolerance.
   Carry the selected convention and actual shift through the existing
   resolved quote at consumer boundaries. A Normal quote carries no
   displaced-Black metadata even when its source backbone is displaced.

5. **Validate the complete request before limits.** Require finite F
   and K and finite T>0 for every quote. Exact beta=0 Normal admits
   signed F and K and skips translated levels. For every other case
   require mathematical x=F+s and y=K+s in (0,MAX], with s=0
   when absent. Check the complete sums from original inputs. A
   Normal output from positive beta still needs the positive
   backbone domain. Reject T=0. Use T decreasing to zero as a
   leading-term qualification limit.

6. **Use literal model endpoints.** Keep exact beta=0 and beta=1,
   including beta=0 with a requested Black output in its positive
   domain. A small positive beta uses the supplied positive beta.
   Remove the 1e−4 beta snaps and rho endpoint snapping. Use exact
   F=K for the ATM identity and exact nu=0 for the constant-scale
   identity. Near values use the continuous formula.

7. **Preserve original payoff geometry under shift.** Keep signed
   Delta=F−K in checked scale. For positive effective coordinates use
   L=log(x/y), with sign from Delta, and evaluate the magnitude as
   log1p(|Delta|/min(x,y)) in stable scale. Preserve distinct strikes
   when the two individually rounded shifted levels coincide. Resolve
   log M for M=max(x,y) and the geometric midpoint through
   log g=log M−|L|/2. Avoid forming the raw x y product.

8. **Use the exact signed CEV distance.** Define
   D=integral_y^x t^(−beta) dt. For d=1−beta>0 this is
   (x^d−y^d)/d. For exact beta=1 it is L. Exact beta=0 gives
   D=Delta, including signed Normal. Remove the truncated moneyness
   polynomials from the distance and leading factors. This is the
   corrected short-time Black distance and the corresponding normal
   leading coordinate.

9. **Evaluate the distance with a positive divided difference.** Use
   ell=|L| and e(u)=−expm1(−u)/u for u>=0 with e(0)=1.
   Then D=M^d L e(d ell). Use scaled powers, the removable limit
   and the original beta in the exponents. This form avoids direct
   subtraction of nearly equal powers and preserves signed distance
   near ATM and beta=1.

10. **Use native leading factors for the selected quote.** Black and
    shifted Black use P_B=alpha M^(−d)/e(d ell), equal to alpha L/D
    away from ATM. Positive-domain Normal uses
    P_N=alpha M^beta e(ell)/e(d ell), equal to alpha Delta/D.
    Exact beta=0 Normal uses P_N=alpha directly, with no translated
    level calculation. At ATM the limits are alpha x^(beta−1) and
    alpha x^beta respectively, or alpha for exact beta=0 Normal.

11. **Share one exact skew factor.** Use z=nu D/alpha and
    chi(z)=integral_0^z [1−2rho v+v²]^(−1/2) dv. The factor is
    R=z/chi(z)>0 with exact R(0)=1. Carry z in checked scale even
    when a standalone quotient or its square exceeds binary64
    range. Use the same chi for every output and preparation path.

12. **Evaluate chi and its ratio with a stable identity.** Let
    s2=(1−rho)(1+rho), S=hypot(z−rho,sqrt(s2)), B=1−rho z and
    W=S+B>0. For B>=0 use the positive sum. For B<0 use
    W=s2 z²/(S−B) in scale. With u=z/sqrt(2W), use
    chi=2 asinh(u) and R=sqrt(2W)/(2 asinhc(u)), where
    asinhc(u)=asinh(u)/u and asinhc(0)=1. Evaluate the divided
    difference near zero and use scaled/log arithmetic for large
    arguments. Remove small-chi floors, endpoint approximations and
    interpolation between different chi values.

13. **Keep the signed Normal zero-beta limit exact in geometry.**
    For that branch z=nu Delta/alpha even if a common shift is
    supplied. The resulting Normal expansion is invariant under
    the model displacement. For other branches preserve the actual
    shifted levels in the distance, leading factors and time term.
    When nu=0 or Delta=0, use exact R=1 and still apply the
    corresponding leading factor and time correction.

14. **Select the Hagan midpoint time factors explicitly.** On
    the positive domain set a=alpha/g^d. Let
    P=d² for Black and shifted Black, and P=−beta(2−beta) for Normal.
    The correction is
    C=[P a²+6 rho beta a nu+(2−3rho²)nu²]/24.
    For exact beta=0 Normal take only the nu² term, including signed
    coordinates. The selected quote is P_B or P_N times R times
    (1+T C). The midpoint correction remains an approximation
    away from ATM even with the exact leading distance.

15. **Resolve the complete correction before accepting it.** Form
    a sqrt(T) and nu sqrt(T) in scale. Use checked signed and
    compensated arithmetic for the complete numerator
    24+P(a sqrt(T))²+6 rho beta(a sqrt(T))(nu sqrt(T))
    +(2−3rho²)(nu sqrt(T))². Skip structurally zero terms. Require
    a resolved strictly positive bracket. A nonpositive bracket
    or unresolved cancellation returns a computation error.

16. **Preserve exact constant-volatility reductions.** For beta=0,
    nu=0 and Normal output, the quote is exactly alpha. For beta=1,
    nu=0 and Black or shifted Black output, it is exactly alpha
    on the admitted positive domain. Other constant-scale cases
    keep their selected CEV leading factor and time correction.
    Evaluate the other quote conventions as their own expansions.

17. **Keep output range and failure semantics explicit.** Restore
    a strictly positive finite sigma from the complete scaled
    product. A positive quote lost to zero rounding, an out-of-range
    result or unresolved required arithmetic returns a computation
    error. Parameter, coordinate and quote mismatch failures are
    validation errors. Remove tiny-alpha zero, positive floors,
    clipping, NaN sentinels and silent convention switching.

18. **Limit the financial claim to the selected expansion.** A
    positive quote establishes that the formula evaluated in its
    domain. There is no universal expiry, wing or vol-of-vol cutoff
    that qualifies approximation error. The expansion supplies no
    arbitrage, exact premium-equivalence or usable-delta guarantee.
    Exact conversion between quotes belongs to a price/inverse
    contract and the relevant valuation or conversion owner.

19. **Use the same scalar and prepared law.** Reuse checked
    strike/forward geometry across prepared evaluations. Evaluate
    residuals, final quotes and required derivatives from this
    same formula and convention. Useful identities are
    chi_z=1/S and chi_rho=z²/(S W). At z=0 use R_z=−rho/2
    and R_rho=0. Preserve genuine one-sided nu=0 sensitivity
    where the caller uses it. A requested derivative outside
    the admitted neighboring domain is unavailable or errors
    according to its owning API. Use no differentiated legacy
    beta snaps or blended chi kernels.

20. **Bind the selected quote in smiles and fitting.** Checked
    smile construction carries the output convention through ATM,
    strike/grid evaluation and downstream repricing. Calibration
    uses the explicit fit convention and active model shift for
    prepared evaluations, weights, gradients, ATM pin and final
    residuals. Reconcile calibration and smile consumers with
    this formula. Records 14 and 15 own fit and repair policy.

21. **Reconcile actual sources and product pricing.** Apply
    [CORE-MARKET-16](../core-market/sabr-cube-data.md#accepted-decision-canonical-sabr-sources-and-explicit-evaluated-quote-meaning)
    when a cube/source supplies parameters. Preserve the supplied
    model data, exact output metadata and active displacement.
    Remove beta-derived resolution and mismatched fit/publication
    tags in actual swaption, equity and source routes. The
    dated forward, annuity/discount, clock, market provenance,
    settlement and risk chains stay with their owners. Record 16
    owns off-grid interpolation and materialization.

22. **Retire redundant defaults and quote dispatch.** Remove the
    beta-derived quote helper, `supports_negative_rates`, public
    normal/lognormal quote siblings, beta-preset constructors
    and embedded equity/rates parameter presets. Use one constructor
    with the literal underlying parameters and optional shift, with
    getters for those values. Calibration starts remain a separate
    fit decision.

23. **Keep the host and wire contracts canonical.** Reconcile the
    Rust, Python and WASM parameter, quote, smile and calibration
    surfaces, including existing enum wire forms, schema, stubs,
    facade, generated UI contracts, parity ledger and notebook.
    Hosts convert inputs and propagate the checked Rust errors.
    The scalar result uses the requested convention checked in Rust.

24. **Qualify the selected formula and consumers.** Compare
    independent references for the corrected leading terms and
    selected midpoint coefficients, exact ATM and beta endpoints,
    small positive alpha/nu, signed displacement and original
    strike gaps, interior correlations near the endpoints and
    unresolved correction/output range. Qualify prepared value
    and derivatives, correct units and actual product/host paths.
    Existing Hagan/QuantLib numbers are references for their own
    formula. Changed off-ATM outputs require separately derived
    references and fit and pricing qualification.

## Independent bounded reference checks

- Exact beta=0 Normal with F=0, K=1, shift=2^55, alpha=1,
  nu=0.5, rho=0 and T=1 gives approximately 1.06069019938.
  Subtracting separately rounded shifted coordinates gives an
  artificial ATM result of 1.02083333333.
- At F=K=1e−30, alpha=4e−17, beta=0.5, nu=rho=0 and T=1,
  the Black formula gives approximately 0.0400006666667 despite
  the current tiny-alpha cutoff. Exact beta=1 and nu=0 with
  positive coordinates returns alpha even at alpha=2^-60.
- For F=0.015, K=0.001, alpha=0.03, beta=0.5, nu=0.65,
  rho=−0.25 and T=4, the exact distance is about 0.181703421075.
  The selected Normal and Black quotes are about 0.00503410100035
  and 1.00935536264. The simplified Hagan Normal formula gives
  about 0.00483830688778. This measures formula change.
- For rho=1−2^-53 and z=2, chi is about 37.4299477502
  and R is about 0.0534331496626. The current endpoint
  approximation rejects this admitted interior correlation.
  Standalone high-precision comparison of the stable identity and
  defining formula passed 133 skew cases, including both signs
  of z and arguments with magnitude 2^700.
- With shift=2^54, F=1, K=2, beta=1, alpha=0.25, nu=2^52,
  rho=0 and T=2^-104, the shifted Black quote is about
  0.307285511300. The rounded shifted legs coincide and would
  give the artificial ATM value 0.270833333333.
- With beta=1, nu=0, F=0.04, K=0.02, alpha=0.25 and T=5,
  the direct Normal quote is about 0.00711954974605. Bachelier
  inversion of the exact Black premium gives about 0.00712100460789.
  These are separate operations.
- With beta=1, nu=0, alpha=2 and T=6, the Normal time bracket
  is exactly zero. Separately, at F=K=1e308, beta=0.5, nu=0,
  alpha=2^-1074 and T=1, the Black quote is positive in exact
  arithmetic and rounds to zero in binary64. Both cases require
  numerical refusal for different reasons.

## Observed scope and input domain

Positive α, β∈[0,1], ν≥0, ρ∈[−1,1], optional strictly positive shift; T>0. Non-normal branches need positive effective F,K; β≈0 permits crossing zero.

## Observed choice and rationale

Hagan asymptotic expansions with explicit normal/lognormal variants, stable near-ATM/χ formulas and beta snapping. These approximate quote laws are not an arbitrage guarantee or full SABR transition density.

## Observed algorithm

For lognormal, effective F,K include shift; snap β<1e−4 to0 and |1−β|<1e−4 to1. Let z=(ν/α)(FK)^((1−β)/2)ln(F/K), χ=ln[(√(1−2ρz+z²)+z−ρ)/(1−ρ)]. Quote=α/[(FK)^((1−β)/2)(1+(1−β)²L²/24+(1−β)^4L^4/1920)] ×z/χ×(1+T A), with A=(1−β)²α²/[24(FK)^(1−β)]+ρβνα/[4(FK)^((1−β)/2)]+(2−3ρ²)ν²/24. Relative |F−K|<1e−8 uses ATM limit. Normal β=0 uses z=ν(F−K)/α and α(z/χ) × (1+(2−3ρ²)ν²T/24). General normal uses α(FK)^(β/2), log-ratio numerator/denominator series and negative β(2−β) α² correction. χ uses a fourth-order series below1e−5, exact above1e−3 and smoothstep blend between; z/χ has its own cubic series below1e−5. Correlation endpoint logarithmic limits reject undefined domains.

## Observed selection and controls

`implied_vol` returns normal only for β<1e−4, otherwise lognormal/shifted quote. Explicit `implied_vol_normal` or `_lognormal` bypass that convention choice. `rates_standard` fixes β=.5; defaults are equity α=.2,β=1,ν=.3,ρ=−.2 or rates α=.02,β=.5,ν=.3,ρ=0. These are explicit convenience values, not calibrated market data.

## Observed conventions

Normal quote has rate/price units per √year; lognormal quote relative annual units. α dimensions depend on β. Shift changes effective rates, not output currency.

## Observed limitations, failure and fallback

Parameters have public fields; quote methods do not universally re-run parameter validation. Negative/nonfinite calculated quotes error; tiny-alpha ATM branch can return zero. Normal and lognormal endpoint/small-argument thresholds differ. Valid parameter ranges do not ensure approximation accuracy in long expiry/extreme wings.

## Alternatives and tradeoffs

Normal β=0 exact-backbone asymptotic form and explicit normal general-β expansion are supported alternatives; price-equivalent conversion is a separate ATM routine, not these normal expansions.

## Implementation and validation evidence

[expansion.rs](../../../../finstack-quant/models/src/volatility/sabr/expansion.rs) and [parameters.rs](../../../../finstack-quant/models/src/volatility/sabr/parameters.rs). [tests.rs](../../../../finstack-quant/models/src/volatility/sabr/tests.rs) includes Hagan ATM hand-formula, endpoint/χ crossover and fractional-beta independent-formula tests. [vol_models_quantlib_tests.rs](../../../../finstack-quant/models/tests/vol_models_quantlib_tests.rs) distinguishes QuantLib lognormal and mpmath exact-β0 normal references. Reference: [Hagan SABR](../../../REFERENCES.md#hagan-2002-sabr).

Actual consumers include [prepared calibration](../../../../finstack-quant/models/src/volatility/sabr/calibration.rs),
[smiles](../../../../finstack-quant/models/src/volatility/sabr/smile.rs),
[volatility sources](../../../../finstack-quant/models/src/volatility/source.rs),
and [swaption pricing](../../../../finstack-quant/valuations/rates/src/swaption/types/swaption.rs).
See the [Python](../../../../finstack-quant-py/src/bindings/models/volatility.rs)
and [WASM](../../../../finstack-quant-wasm/src/api/models/volatility.rs)
wrappers and the [SABR notebook](../../../../finstack-quant-py/examples/notebooks/01_foundations/market_data/sabr_volatility.ipynb).
The [SABR expansion references](../../../REFERENCES.md#sabr-cev-distance-and-time-corrections)
distinguish the selected leading terms and midpoint correction.

## Relationship to the accepted SABR source boundary

[CORE-MARKET-16's complete accepted package](../core-market/sabr-cube-data.md#accepted-decision-canonical-sabr-sources-and-explicit-evaluated-quote-meaning)
requires exact canonical source parameters and explicit evaluated Normal/Black/
ShiftedBlack output, without beta-derived quote tags, source-node floors/caps or
silent beta snapping. Reconcile source adapters with finite signed displacement
admission and check actual backbone/effective forward/strike domains. A Normal
output request does not remove a fractional-backbone positive-domain restriction;
only a supported exact beta=0 normal method permits crossing zero. Independent
normal/lognormal asymptotic outputs are not exact price-equivalent conversions.
Record 16 was accepted on **2026-11-08**, with reconciliation/qualification
pending. Its acceptance does not adopt this record's complete expansions,
thresholds, range policy or financial accuracy.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
