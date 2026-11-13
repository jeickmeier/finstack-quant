# MODELS-AND-VALUATIONS-CREDIT-08: Stochastic prepayment, burnout and pool-correlation controls

- Decision status: **Accepted — full package on 2026-11-12**.
- Implementation status: Current behavior observed in the source below. The accepted contract requires Rust and product caller reconciliation.
- Validation status: Independent scalar, integration, regime and matrix examples evaluated. Production, host, market and product qualification were not run.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked prepayment and pool correlation

1. **Require an explicit model and its active inputs.** Select deterministic,
   factor-shocked, refinancing/burnout or two-state regime prepayment. Remove
   the default 100% PSA and implicit shock coefficients. Validate complete
   immutable model inputs and restore through the same checked operations.
   Use positive one-based applied model months where age matters. Delegate
   deterministic SMM, including the ABS count benchmark, to CASHFLOWS-010.
   A factor-shock base uses the owner's annual CPR on that month for
   Constant, PSA, Lockout or annual CPR Vector. Share the cashflow model's
   annual expression and preserve its survival information, avoiding a
   rounded SMM-to-CPR round trip. ABS stays a deterministic count benchmark.
   Reject missing active factors or context, invalid rates or state, and
   unsupported combinations. An inactive feature takes no input or draw.
   [Monthly mortality and benchmark clocks](../cashflows/10-prepayment-and-mortality.md).

2. **Share one checked CPR shock and monthly conversion.** Use one finite
   signed log-CPR loading `b` and one finite standardized factor `Z`. For
   nonnegative base `B` use `CPR=min(1,B*exp(bZ))`, with the annual cap a
   deliberate part of the law. A positive loading raises CPR when `Z`
   rises. The shock has no mean correction, so `B` is the zero-factor
   reference. Use checked scaled or log arithmetic for the full product and
   cap comparison. Convert to `SMM=−expm1(log1p(−CPR)/12)` through the
   accepted cashflow calculation. Preserve uncapped annual survival when
   an intermediate rounded CPR would give one. Keep exact zero and cap
   identities. Reject unresolved required results and remove the tiny-CPR
   floor, hidden 50% SMM limit and failure-to-zero. Factor-only and regime
   variants carry no refinance burnout input.

3. **Name the refinance response and specify its feedback.** Rename the
   simplified variant `RefinancingBurnout`. Require full-season annual
   base CPR `C0` in `[0,1]`, finite signed annual decimal coupon `c` and
   market refinance rate `r`, finite nonnegative sensitivity `γ` and slope
   `k` per unit decimal rate, and current burnout multiplier `H` in `[0,1]`.
   Use `B=C0*min(month,30)/30*H*max(0,1+γ atan(k(c−r))/(π/2))` before the
   common shock and cap. This base can exceed one before the shock. Make
   the slope explicit and keep the fixed 30-month ramp as part of this
   scenario. At zero sensitivity or slope the refinance multiplier is one.
   Require a caller-supplied initial burnout multiplier, including when
   valuing a seasoned pool. After computing the month's modeled SMM `m`,
   update exactly once using `H'=clip(H*(1+δ−δ*m/e),0,1)` for an explicit
   `δ` in `[0,1]`. Use the named baseline `e=E_Z[SMM]` at the same age,
   coupon equal to refinance rate and `H=1`. This feedback uses the modeled
   rate before product cash allocation. Validate inputs first. Zero `δ` or
   zero `H` is an identity. With active feedback, `m=e=0` leaves `H`
   unchanged and positive `m` with zero `e` fails. Loss of a positive
   baseline to rounding also fails. Use scale-aware threshold
   comparisons without a ratio floor. Below-baseline rates can raise `H`.
   Commit checked age and burnout together only after the month's model
   calculation succeeds. Coefficients and scenario applicability are
   caller-owned. [Richard and Roll's empirical model](https://www.anderson.ucla.edu/documents/areas/fac/finance/1989-1.pdf).

4. **Compute expected SMM under its stated conditioning.** Hold the supplied
   age, refinance rate and burnout fixed where active. Take the expectation
   of the converted monthly probability under a standard-normal factor.
   The refinance baseline is the named special case above. Handle exact
   zero base and zero loading directly. For nontrivial expectations require
   a finite symmetric cutoff `A>0`, an omitted-mass budget in `(0,1)`,
   and explicit checked adaptive Gauss–Legendre order, absolute estimator
   tolerance, depth and total evaluation limits. Require `2Φ(−A)` within
   the tail budget. Orient the integral so the loading magnitude is
   positive, split at zero and the interior annual-cap crossing where
   applicable, and integrate `SMM(z)φ(z)` on `[-A,A]`. The cap region can
   be evaluated from its normal mass directly. Do not renormalize omitted
   tails. Report the result, omitted mass, summed interior estimator and
   work. Bound the combined estimates and work for a mixture as well as a
   single integral. Propagate unresolved arithmetic or failed controls.
   Tail mass bounds the omitted contribution of a unit payoff. The
   interior estimate is a numerical diagnostic. [Checked quadrature](../core-numerics/quadrature.md).

5. **Advance the two-state regime exactly once per applied month.** Require
   ordered annual probabilities `0<=CPR_low<=CPR_high<=1`, monthly Low→High
   probability `u` and High→Low probability `d` in `[0,1]`, and explicit
   log loading. Start Low before the first month. The carried state pairs
   a known regime with its completed model month `n`. For month `n+1`,
   transition from the carried regime using a checked uniform and strict
   comparison with its switch probability. Then use the new regime's
   shocked SMM. Validate age and required inputs before calculation. Advance
   only after the complete calculation succeeds. Repeated, skipped or
   overflowing month transitions fail. Exact switch probabilities zero
   and one are deterministic. A seasoned pool supplies a known state or
   explicitly requests an age-only initialization using the origin law,
   with its own draw independent of the factor when the state is uncertain.
   This initialization assumes no observed survivor history. No stationary
   fallback.

6. **Keep known-state and age-only regime expectations distinct.** For a
   known prior regime, use its one-step transition weights. For the
   low-origin age-only reference at applied month `m`, use
   `P(H_m)=u/(u+d)*(1−(1−u−d)^m)` when `u+d>0`. Origin and `u=d=0`
   give Low. Preserve forced alternation when `u=d=1`. For a nonnegative
   eigenvalue use `log1p`/`expm1` to retain rare transition mass. Near
   alternation use stay mass `(1−u)+(1−d)` and parity to resolve the signed
   power without rounding away small transition mass. Compute both weights
   with checked stable arithmetic. Sample the age-only state using the
   smaller weight and the corresponding uniform tail when near an endpoint.
   Weight already converted SMMs, either
   conditional on the supplied factor or averaged under the explicitly
   selected standard-normal reference independent of the regime transitions.
   An age-only result averages the chain started Low before conditioning on
   pool survival. Multiplying those means across months does not establish
   the survival of the carried-state model.

7. **Keep correlation structure and factor coupling explicit.** For an
   authored asset structure, Flat uses a finite common pair coefficient
   in `[0,1]`. Sectored uses `0<=inter<=intra<=1` for its hierarchical
   global, sector and residual variances `inter`, `intra−inter` and
   `1−intra`, with caller-supplied sector membership when applied. Matrix
   uses an exact ordered nonempty
   label set with unique nonblank labels and checked shape. Require finite
   entries in `[-1,1]`, exact symmetry and unit diagonal, and the accepted
   explicit numerical PSD tolerance. Preserve authored coefficients.
   Remove default correlations and scalar means for sectors and matrices.
   A scalar route requires its own explicitly selected scalar. Separately
   select shared credit/prepayment factor or paired standard normals with
   an explicit signed factor correlation `r_f` in `[-1,1]`. In the latter
   case use `Z_p=r_f Z_c+sqrt((1−r_f)(1+r_f)) E` with independent standard
   normal `E`, exact endpoint cases and checked arithmetic. Zero gives
   independent factors. The coefficient applies to the standardized
   factors and does not identify the correlation of SMM and MDR. Factor
   dynamics and their cross-time coupling remain explicit product inputs.
   [Checked matrix admission](../core-numerics/matrix-factorization.md).

8. **Require shocks to affect an admitted, active parameter.** Asset
   shocks add a finite decimal correlation-point change, with `0.01`
   representing one percentage point. Flat changes its single coefficient.
   A parallel sector shock applies the same delta to intra and inter.
   Distinct sector changes require explicitly authored targets. A matrix
   parallel shock adds the delta to each mirrored off-diagonal pair, with
   labels and diagonal retained. Validate the complete candidate including
   PSD before accepting it. A prepay/default shock targets the actual
   configured two-factor coefficient, never an inactive field or inferred
   matrix value. An unsupported target errors, and zero or ineffective
   changes are never counted as applied. Propagate out-of-range and
   non-PSD failures. Any nearest-correlation repair is a separately
   selected operation with its own controls and evidence.
   [Explicit correlation repair](../core-numerics/nearest-correlation.md),
   [Higham](https://eprints.maths.manchester.ac.uk/232/01/paper3.pdf),
   [scenario correlation shocks](../scenarios/10-correlation-shocks.md).

9. **Reconcile the live routes and preserve their ownership.** Use the
   selected copula's scalar correlation for the CREDIT-07 single-systematic
   default law. Retire implicit deal-type, sector and matrix overrides.
   Current scalar-only revolver and OAS calculations require an explicit
   compatible coefficient on their own economic basis. Refuse an authored
   dependence structure on a route that cannot apply it. In structured
   credit, preserve explicit factor persistence, wire the selected factor
   coupling to actual innovations, and retain stable original month and
   role draw addresses. A regime initialization draw is separate from
   transition draws. For paired replay, apply the checked open midpoint
   uniform complement on the same transition address. Remove dummy draws,
   invented empty-month factors and saturating age offsets. The product
   owns actual ages, stub mapping, competing-event order, survivor
   eligibility and cash. Reconcile authored presets, including resolved
   coupon and provenance for any retained refinance preset. Keep checked
   kernels in Rust and reconcile existing structured-credit Python/WASM
   inputs, schema, declarations, parity and scenario results. Release no
   partial product result on invalid or failed calculations.
   [Pool default contract](07-pool-defaults.md),
   [Philox draw ownership](../models-monte-carlo/02-random-streams.md),
   [structured-credit waterfall](../valuations-structured-credit/15-stochastic-waterfall.md),
   [structured-credit OAS](../valuations-structured-credit/17-oas-scenarios.md).

## Observed behavior

- Factor prepayment converts base SMM back to CPR, substitutes zero on
  errors and for CPR below `1e−10`, and defaults a missing factor to zero.
  The expected-rate helper integrates the SMM survival function in log-SMM
  coordinates with fixed bounds and panels. It gives the expectation after
  limiting rates to their values at the factor cutoffs, and can return NaN
  on failure.
  Refinance uses fixed slope `20`, an implicit factor profile, and an
  at-the-money, unburned baseline. Its feedback suppresses small rates
  and uses a product-clipped monthly rate. Regime and factor paths accept
  a burnout multiplier despite having no burnout update of their own.
- The regime carries its Low/High flag but ignores the applied month when
  sampling a transition. Near alternation, `u+d` can round to two and
  erase a small, positive state probability. The caller initializes the
  burnout multiplier to one and draws a uniform even when the selected
  variant has no transition. It saturates the age offset and can invent
  a zero-factor month for an empty payment interval.
- A scalar override can flatten authored sector or matrix structure and
  replace the copula coefficient. A matrix supplies a synthetic prepay/default
  scalar. The main stochastic route reads factor coupling from its latent
  factor configuration, while the ordinary deal builder does not wire the
  stored pool prepay/default coefficient into that configuration. Asset
  shocks clip, apply half the sector delta to inter-sector correlation,
  and can accept an invalid bumped matrix. The scenario adapter can count
  a reported no-op as an applied change.

## Evidence

Owners: [Prepayment models](../../../../finstack-quant/models/src/credit/pool/prepayment), [expected SMM](../../../../finstack-quant/models/src/credit/pool/mod.rs), [pool correlation](../../../../finstack-quant/models/src/credit/pool/correlation.rs), [structured-credit monthly evaluation](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/stochastic/pricer/engine.rs), [deal factor and correlation selection](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/pricing_methods.rs) and [scenario shocks](../../../../finstack-quant/scenarios/src/engine/instrument_shocks.rs). Inline tests cover selected expectations, state transitions, bump clamps and caller behavior.

### Independent review evidence

- Annual base CPR `.20` and log loading `1` give expected SMM
  `0.08176149769`. Converting expected capped CPR gives `0.02860694159`.
  The uncapped annual product `(1+2^−27)*(1−2^−27)` rounds to one in
  binary64, while its survival complement gives SMM `0.9558058262`.
- At base CPR `.06`, full seasoning, coupon `.06`, `γ=2`, `k=20`,
  no factor shock and `H=1`, refinance rates `.04`, `.06` and `.08`
  give SMM approximately `.00774383`, `.00514301` and `.00261490`.
  With `H=.8`, `δ=.1`, baseline `.6` and modeled SMM `.75`, feedback
  gives `.78`. Feeding a clipped `.50` instead gives `.81333333`,
  reversing the direction of feedback.
- A half/half regime mixture of `.04` and `.80` CPR gives
  `0.06445539049` SMM, while converting the averaged CPR gives
  `0.04437904132`. For `u=.10,d=.20` and state SMM `.10/.50`, carried
  two-month survival is `.7256`; multiplying the age-reference survival
  means gives `.71552`. For `u=1,d=nextDown(1)`, the exact probability
  of High at month two is `2^−53`; the rounded sum `u+d=2` erases it.
- A matrix with off-diagonal coefficients `.8,.8,.3` has determinant
  `.014`. A parallel `+.05` leaves every entry in range and changes
  the determinant to `−.06175`. For sectors of 90 and 10 names with
  `.40` intra and `.10` inter, the equal-name pair mean is
  `0.34545455`, while the current equal average of the two parameters
  is `.25`. Neither average retains the pairwise structure.
- Production qualification still needs range and integration convergence,
  persistent-state replay, paired draws, full matrix and scenario
  admission, product chronology and actual host/wire cases. Empirical
  refinance and regime calibration remain separate from the examples.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
