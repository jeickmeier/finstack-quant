# MODELS-ANALYTIC-16: Volatility source interpolation, quote conventions and materialization

**Decision status:** Accepted (complete package adopted 2026-11-10).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Source and caller paths inspected. Independent scalar arithmetic checks support the proposed formulas. Production implementation, independent model qualification and end-to-end host checks remain pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for explicit volatility-source evaluation and materialization

All 27 decisions were accepted on **2026-11-10**. Implementation reconciliation
and independent complete-path qualification remain open.

1. **Resolve one active source.** Dated market use selects a source kind and
   exact ID in its namespace under CORE-MARKET-20. Propagate errors from
   that source. Direct-object evaluation also supports the fixed-expiry
   slice. Scalar and local-model overrides, terminal branches and other
   inactive paths resolve only their own inputs.

2. **Use coordinates specific to each source.** A strike surface takes
   expiry and absolute strike. A tenor surface takes expiry and underlying
   tenor, with the owner qualifying its reference strike, such as ATM
   for an ATM matrix. A fixed-expiry slice
   takes tenor and absolute strike. An adapter that also carries expiry
   must match the slice's stored expiry exactly. A SABR cube takes expiry,
   underlying tenor and absolute strike. Remove the overloaded tenor
   argument and zero-tenor sentinel.

3. **Return the checked quote and metadata together.** Return native annual
   sigma, its precise convention, actual quote displacement when shifted,
   source expiry clock, and requested and evaluated coordinates. Sigma
   grids use their stored quote family. SABR uses the existing
   `VolQuoteType` to select Normal, Black or ShiftedBlack and returns
   the resolved `VolatilityConvention`. Enforce any explicit caller convention
   or displacement assertion against the result. Retire independent
   convention lookup and redundant defaulted or NaN-returning wrappers.

4. **Qualify the FX delivery forward.** Active FX delta evaluation takes
   expiry, absolute strike and a named delivery forward qualified for
   the pair, orientation, delivery and clock under CORE-MARKET-15. The
   dated owner supplies the actual settlement and delivery relationship.
   Record 17 owns quote-space conversion, smile evaluation and its
   boundary controls.

5. **Preserve the actual clocks and coordinates.** Require finite positive
   option expiry and applicable underlying tenor and finite absolute strike.
   Apply ADR-0045 to dated origin, day count and explicit clock bridges.
   Preserve an absent numeric source clock. In a qualified sigma-only
   bridge use times for the same requested physical interval, including
   when the lookup samples a clamped knot. A context roll does not age
   the source. A sigma bridge does not rescale SABR parameters.

6. **Select a grid boundary policy explicitly.** Stored sigma grids and
   SABR cubes admit strict error or flat-coordinate clamp. Clamp only
   the stored axes, and use effective expiry in the interpolation and
   SABR kernels. The fixed expiry of a slice is identity. A SABR cube
   retains the original strike. On a singleton axis strict lookup
   requires exact equality. Clamp can select the singleton for an
   admitted query. Remove the unsupported linear-in-variance
   extrapolation selector where it only invokes a clamp.

7. **Select exact support before computing weights.** An exact coordinate
   on an axis selects its one stored index. An interior coordinate selects
   its actual adjacent pair. Evaluate only the Cartesian product of
   those indices. Check every mathematically active corner, even when its
   ordinary rounded weight is zero. An exact sigma knot returns its
   stored quote and displacement directly.

8. **Use checked convex geometry.** Form both distance ratios from the
   original coordinates with scaled differences where needed. Use their
   tensor products as the mathematical weights. Keep adequate scale
   through complete weighted products and sums, including signed strike
   spans whose direct subtraction overflows and positive weights that
   would disappear if rounded before multiplication.

9. **Interpolate native sigma directly in Vol mode.** Use the convex
   weighted sum of active annual sigmas on strike or tenor surfaces and
   tenor-by-strike slices. The secondary coordinate keeps its declared
   meaning. Preserve literal zero and constant values and retain the
   native relative or absolute units.

10. **Interpolate total variance on the same grid in TotalVariance mode.**
    Return `sqrt(sum(w_ij × T_i × sigma_ij²) / T_effective)`.
    At an exact expiry and throughout a
    fixed-expiry slice this is the weighted RMS of sigma on the active
    axes. Option expiry supplies the time coordinate. Select each grid's
    interpolation mode explicitly, including for a fixed-expiry slice.

11. **Evaluate a complete weighted norm.** Use checked scaled products and
    a scaled sum of squares, preserving representable weighted contributions
    even when raw sigma squared or an isolated weight is outside binary64
    range. Apply no epsilon expiry denominator, sigma floor or intermediate
    saturation. A mathematically zero blend returns zero. Refuse a lost
    positive or nonfinite complete result.

12. **Require a common displacement when blending shifted sigma.** Check
    exact equality across active expiry rows of a surface or active tenor
    rows of a slice. Preserve a row's signed displacement, including
    explicit zero. A request spanning distinct shifts errors because the
    blend needs a common shifted quote convention. Individual rows remain
    available. Normal and Black grids carry no quote displacement.

13. **Check the strike and pricing domains.** On a strike grid enforce
    finite positive original and evaluated K for Black, or finite positive
    original and evaluated K+s for shifted Black. Normal accepts finite
    signed strikes. Strike clamping cannot admit an invalid original
    strike. The pricing owner checks the actual forward and strike pair
    against the returned convention, including overflow and positivity
    after displacement. A tenor surface has no sampled strike.

14. **Preserve complete SABR nodes.** An exact expiry and tenor uses the
    stored alpha, beta, nu, rho, shift Option and construction forward
    directly. Evaluate accepted record 13 at that expiry, original strike
    and stored forward in the selected family. Retain the model's domain
    and range checks.

15. **Define ParameterSpace mode with common beta.** Use the active
    expiry-by-tenor weights to blend alpha, nu, rho and construction forward
    arithmetically. Require one exact beta across those nodes, preserving
    the units of alpha. Preserve that beta. Remove nearest-beta selection,
    floors and caps. Validate the resulting tuple through record 13.

16. **Define shift interpolation for SABR parameters.** All-absent shifts
    give absence. All-present shifts blend as signed parameters with the
    same weights, preserving presence for a zero result. Mixed presence
    fails. This defines one displaced backbone evaluated at the effective
    expiry, interpolated construction forward and original strike. Form
    the checked record 13 convention from its actual shift and verify
    any caller assertion exactly.

17. **Define cube TotalVariance by evaluating rows first.** On each active
    expiry row apply the ParameterSpace tenor rule. Evaluate record 13
    at the row's actual expiry and own resulting construction forward at
    the same original absolute strike. Return the row directly at exact
    expiry. Across two rows combine sigmas using the expiry weights and
    decisions 10–11. Each row may have its own beta and forward. This
    defines a fixed-absolute-strike quote with no combined parameter
    tuple or cross-expiry forward.

18. **Check the output convention across SABR rows.** Normal permits
    independently valid row backbones with different shift Options and
    carries no quote displacement. Black requires absent model shift on
    each row. ShiftedBlack requires the same actual signed displacement
    across evaluated rows. Apply record 13's checks in each row and
    retain source, mode and coordinates for the blended quote.

19. **Keep construction forwards distinct from pricing forwards.** The
    dated product or calibration owner qualifies underlyings and dates
    and matches the construction basis to the use, or invokes an
    explicitly owned frozen-forward, rebased or refitted smile. This
    applies to both rows in variance mode. A live forward cannot
    overwrite stored forwards or qualify a source by itself. Preserve
    caplet and swaption underlying restrictions and CMS surface-only
    restrictions.

20. **Make cube materialization strict and explicit.** Require output ID,
    quote family, output-grid interpolation, fixed tenor or expiry, and
    a nonempty ordered unique absolute-strike grid. Use the source's
    existing expiry or tenor axis for the other dimension. Require the
    fixed coordinate within the source domain, including supported
    interior interpolation. Use the same checked evaluator under strict
    policy for all nodes. Check resources and validate the complete
    output before returning it. Preserve node-specific failures.

21. **Keep the two materialized shapes and their provenance.** A fixed
    tenor produces an expiry-by-strike surface. A fixed expiry produces
    a tenor-by-strike slice. Carry the actual clock, quote family and
    displacements, consistent within each sampled row. Keep the source
    identity and construction parameters and forwards available through
    provenance, with evaluation mode, fixed coordinate, output grid and
    selected interpolation. Preserve this metadata through checked state.

22. **Treat sampled grids as derived quote artifacts.** Sampled nodes match
    the selected source. Between nodes the output uses its own interpolation
    and boundary policy. SABR repricing, original risk, ongoing fit and
    arbitrage claims require separate qualification. FX strike
    materialization also retains the clock, output convention and
    interpolation and uses qualified row delivery forwards under
    CORE-MARKET-15. Record 17 owns its sampling details.

23. **Measure changes at an explicit comparable reference.** Require
    qualified opening and closing evaluations with coordinates for each
    snapshot, compatible axes, underlying and quote convention, the
    same effective displacement if shifted and a common qualified annual
    volatility basis. Use strict sampling. Return closing minus opening
    sigma in native units. Remove optional coordinates, the middle-strike
    average, generic multiplication by 100 and the empty zero fallback.

24. **Reconcile the actual attribution callers.** Taylor supplies a complete
    reference and uses a common native unit for movement and bump
    sensitivities. Metrics attribution owns reference selection and any
    common-move test for aggregate risk, with refusal where bucketed
    risk is required. Attribution owns dated endpoint mapping and any
    reporting-unit conversion. Preserve missing-reference and source
    errors.

25. **Reconcile existing product and host paths.** Update shared valuation
    resolution, the actual swaption, cap/floor, FX and strike-surface
    callers, and swaption calibration's slice materializer. FX vanna
    and volga use the same admitted source as PV, subject to existing
    source-specific bump eligibility. Reconcile Dupire and arbitrage
    diagnostics with the checked quote path and their own status rules.
    Update existing Python, WASM and UI evaluation and sampling,
    schemas, stubs, declarations, facade and parity metadata. Generic
    dispatch stays Rust-only. The fixed-expiry slice remains a direct
    artifact where there is no registered market source kind.

26. **Qualify the numerical and financial limits.** Use typed errors for
    incompatible conventions, support, coordinates and arithmetic. The
    interpolation modes and flat tails carry no calendar, butterfly or
    wing-accuracy certification. Fixed absolute strike differs from
    the forward log-moneyness in the classic calendar criterion.
    Calibration, smile repair, scenario shocks, aging and product risk
    stay at their respective owners.

27. **Require production and independent evidence.** Exercise exact,
    one-axis, singleton and boundary behavior, extreme weights, signed
    shift and mixed beta domains, different forwards in both cube modes,
    source/clock/forward failures, materialized node agreement and
    off-grid differences, atomic failure, native attribution units and
    host parity. Independently qualify complete pricing paths. See the
    [primary references](../../../REFERENCES.md#volatility-source-interpolation-and-materialization).

## Independent arithmetic checks

| Fixture | Result |
|---|---|
| Expiry rows (1, 0.2), (3, 0.4), sampled at T=2 | Vol = 0.3, TotalVariance = √0.13 ≈ 0.3605551275464 |
| Same expiry rows, clamp T=4 | Boundary sigma = 0.4 using effective T=3 |
| Fixed expiry, strike midpoint with sigmas 0.1 and 0.3 | TotalVariance = √0.05 ≈ 0.2236067977500 |
| Fixed expiry, strikes 0 and binary64 MAX, sigmas 0 and MAX, query minimum subnormal | Vol = minimum subnormal, TotalVariance ≈ 2.98023223877e−8 despite an independently rounded right weight of zero |
| Black SABR with beta=1, nu=0, expiry/alpha rows (1, 0.1), (3, 0.3), sampled at T=2 | ParameterSpace = 0.2, TotalVariance = √0.07 ≈ 0.2645751311065 |
| Shifted sigma rows with displacements 0.01 and 0.02 | Each exact row retains its own shift, and a cross-row shifted quote blend errors |

The scalar checks establish the distinctions above. Production code and
independent pricing and host qualification remain pending.

## Observed implementation and evidence

### Scope and input domain

Stored VolSurface, SABR VolCube, fixed-expiry cube slices and FX delta sources. Storage validation belongs to core; these models functions evaluate their economic content.

### Choice and rationale

Strict and clamped queries are explicit variants. Surface interpolation selects raw-volatility or total-variance mode; cube raw mode interpolates parameters rather than quote values. This is a substantive modeling choice.

### Algorithm

Surface strict lookup brackets expiry/secondary axis; bilinearly interpolate vol, or bilinearly interpolate Tσ² and return √(w/max(T,ε)). Secondary axis can be strike or tenor. Shift metadata interpolates linearly along expiry/tenor with endpoint clamp. Cube parameters α,ν,ρ,shift and forward interpolate bilinearly in expiry/tenor; β is nearest-corner (ties lower), α floors1e−8,ρ clamps±.9999, missing shift contributes zero when any corner is shifted. Evaluate explicit normal/lognormal SABR. TotalVariance cube mode instead computes each bracketing expiry quote at tenor, interpolates Tσ² in expiry then divides by requested T. Fixed-expiry slices interpolate raw vol bilinearly in tenor/strike. Materializers sample the evaluator, carry quote type/displacements, and clamp requested fixed tenor/expiry.

### Selection and controls

Normal surface cannot supply Black; FX cannot supply normal; normal SABR cube is rejected by `VolSource::get_vol`, whereas direct cube explicit output functions can request the lognormal expansion. Cube output enum accepts Normal/Lognormal and rejects explicit ShiftedLognormal because stored shifts apply internally. For FxDelta only, the `tenor` argument is interpreted as forward. Strict surface/cube bounds error; clamped variants keep endpoints and may return NaN on evaluation failure.

### Conventions

Quotes are annual volatilities with normal/lognormal units retained in metadata. `measure_vol_surface_shift` reports percentage points (×100): use explicit point only when both expiry and strike supplied; otherwise average shifts across positive t0 expiries at t0 middle strike, returning zero if no eligible coordinates.

### Limitations, failure and fallback

Nearest β can create discontinuities at cell midpoints; α/ρ repair changes stored interpolation. Parameter interpolation and variance interpolation do not ensure calendar/butterfly freedom. A materialized expiry slice loses the original nonlinear evaluation between strike/tenor nodes. Partial reference-point arguments are ignored by the shift estimator; its middle strike is not necessarily ATM.

### Alternatives and tradeoffs

Raw-vol and total-variance surface/cube modes are supported. Price-space or arbitrage-constrained interpolation is not. Strict lookup errors and flat clamping deliberately have different economic extrapolation assumptions.

### Implementation and validation evidence

[source.rs](../../../../finstack-quant/models/src/volatility/source.rs). [vol_surface_tests.rs](../../../../finstack-quant/models/tests/vol_surface_tests.rs), [vol_cube_tests.rs](../../../../finstack-quant/models/tests/vol_cube_tests.rs) and [market_data_diff_tests.rs](../../../../finstack-quant/models/tests/market_data_diff_tests.rs) locate interpolation/shape/convention and shift regressions; cube quote comparisons use the same SABR kernel. Reference: [SABR](../../../REFERENCES.md#hagan-2002-sabr). Storage provenance and interpolation defaults are separate from evaluator accuracy.

The shared pricing resolver currently probes same-ID source kinds. FX
vanna and volga separately read a plain surface. The swaption extrapolation
selector also exposes linear variance while its consumer selects clamp.
Taylor currently uses an opening expiry coordinate for both snapshots.
See the [market resolver](../../../../finstack-quant/valuations/core/src/market/volatility.rs),
[FX pricing and risk](../../../../finstack-quant/valuations/fx/src/fx_option/types.rs),
[swaption resolver](../../../../finstack-quant/valuations/rates/src/swaption/types/swaption.rs),
[Taylor](../../../../finstack-quant/attribution/src/taylor.rs),
[metrics attribution](../../../../finstack-quant/attribution/src/metrics_based/context.rs),
[Dupire](../../../../finstack-quant/models/src/volatility/dupire.rs) and
[arbitrage checks](../../../../finstack-quant/models/src/volatility/arbitrage/checks.rs).

## Relationship to the accepted volatility-grid boundary

[CORE-MARKET-14's complete accepted package](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks) selects explicit sigma
source conventions/axes, positive expiries, known strike/shift domains and the
accepted optional source clock. Reconcile exact-node native sigma and stable
requested-output evaluation without epsilon expiry division or unnecessary
squared-vol overflow/underflow. Preserve actual convention guards and full
clock and displacement metadata through materialization. Record 14 was accepted
on **2026-11-08** with reconciliation and qualification pending. This record
owns the accepted interpolation policy.

## Relationship to the accepted FX forward-input boundary

[CORE-MARKET-15's complete accepted package](../core-market/fx-delta-quotes.md#accepted-decision-one-explicit-fx-quote-profile-and-qualified-forward-inputs)
requires explicit FX pair/profile/clock compatibility and a named qualified
delivery-forward input when the active source is FX delta quotes. Replace the
tenor-as-forward convention and actual caller's zero sentinel; do not infer a
forward from an unrelated tenor or insert one when missing. Resolve the actual
source before its dependencies, so explicit sigma overrides and inactive
branches need no unused FX source/forward. Materialized strike grids retain
their own explicit convention, interpolation and clock; they are not lossless
substitutes for quote-space evaluation. Record 15 was accepted on **2026-11-08**
with reconciliation and qualification pending. This record covers the
dispatch boundary and record 17 owns detailed FX smile methodology.

## Relationship to the accepted SABR source boundary

[CORE-MARKET-16's complete accepted package](../core-market/sabr-cube-data.md#accepted-decision-canonical-sabr-sources-and-explicit-evaluated-quote-meaning)
keeps SABR cubes as parameter/forward sources with an explicit output request;
Auto/beta inference cannot supply quote units. Preserve exact nodes and shift
Options without alpha/rho repair or missing-shift filling. Off-grid mixed-beta/
shift/forward combinations need an explicitly supported qualified model or
refusal. Require named cube parameter/variance modes, checked failure propagation
and truthful source-clock/coordinate metadata. Materialized sigma grids select
their own interpolation. Record 16 was accepted on **2026-11-08** with
reconciliation and qualification pending. The accepted rules above use
its accepted storage semantics.

## Relationship to the accepted context lifecycle boundary

[CORE-MARKET-20's accepted package](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle)
requires explicit active source kind/namespace and exact ID rather than cube/
surface/FX-delta priority probing. Resolve only the actual selected source;
explicit scalar/model overrides and inactive branches do not inspect unused
same-ID artifacts. Conditioning/cloning preserves original quote/clock metadata,
not an automatic aged-smile or current-fit interpretation. Accepted record20
does not adopt this record's complete off-grid, materialization,
convention-conversion or volatility-aging models.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
