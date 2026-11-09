# MODELS-ANALYTIC-16: Volatility source interpolation, quote conventions and materialization

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

Stored VolSurface, SABR VolCube, fixed-expiry cube slices and FX delta sources. Storage validation belongs to core; these models functions evaluate their economic content.

## Choice and rationale

Strict and clamped queries are explicit variants. Surface interpolation selects raw-volatility or total-variance mode; cube raw mode interpolates parameters rather than quote values. This is a substantive modeling choice.

## Algorithm

Surface strict lookup brackets expiry/secondary axis; bilinearly interpolate vol, or bilinearly interpolate Tσ² and return √(w/max(T,ε)). Secondary axis can be strike or tenor. Shift metadata interpolates linearly along expiry/tenor with endpoint clamp. Cube parameters α,ν,ρ,shift and forward interpolate bilinearly in expiry/tenor; β is nearest-corner (ties lower), α floors1e−8,ρ clamps±.9999, missing shift contributes zero when any corner is shifted. Evaluate explicit normal/lognormal SABR. TotalVariance cube mode instead computes each bracketing expiry quote at tenor, interpolates Tσ² in expiry then divides by requested T. Fixed-expiry slices interpolate raw vol bilinearly in tenor/strike. Materializers sample the evaluator, carry quote type/displacements, and clamp requested fixed tenor/expiry.

## Selection and controls

Normal surface cannot supply Black; FX cannot supply normal; normal SABR cube is rejected by `VolSource::get_vol`, whereas direct cube explicit output functions can request the lognormal expansion. Cube output enum accepts Normal/Lognormal and rejects explicit ShiftedLognormal because stored shifts apply internally. For FxDelta only, the `tenor` argument is interpreted as forward. Strict surface/cube bounds error; clamped variants keep endpoints and may return NaN on evaluation failure.

## Conventions

Quotes are annual volatilities with normal/lognormal units retained in metadata. `measure_vol_surface_shift` reports percentage points (×100): use explicit point only when both expiry and strike supplied; otherwise average shifts across positive t0 expiries at t0 middle strike, returning zero if no eligible coordinates.

## Limitations, failure and fallback

Nearest β can create discontinuities at cell midpoints; α/ρ repair changes stored interpolation. Parameter interpolation and variance interpolation do not ensure calendar/butterfly freedom. A materialized expiry slice loses the original nonlinear evaluation between strike/tenor nodes. Partial reference-point arguments are ignored by the shift estimator; its middle strike is not necessarily ATM.

## Alternatives and tradeoffs

Raw-vol and total-variance surface/cube modes are supported. Price-space or arbitrage-constrained interpolation is not. Strict lookup errors and flat clamping deliberately have different economic extrapolation assumptions.

## Implementation and validation evidence

[source.rs](../../../../finstack-quant/models/src/volatility/source.rs). [vol_surface_tests.rs](../../../../finstack-quant/models/tests/vol_surface_tests.rs), [vol_cube_tests.rs](../../../../finstack-quant/models/tests/vol_cube_tests.rs) and [market_data_diff_tests.rs](../../../../finstack-quant/models/tests/market_data_diff_tests.rs) locate interpolation/shape/convention and shift regressions; cube quote comparisons use the same SABR kernel. Reference: [SABR](../../../REFERENCES.md#hagan-2002-sabr). Storage provenance and interpolation defaults are separate from evaluator accuracy.

## Relationship to the accepted volatility-grid boundary

[CORE-MARKET-14's complete accepted package](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks) selects explicit sigma
source conventions/axes, positive expiries, known strike/shift domains and the
accepted optional source clock. Reconcile exact-node native sigma and stable
requested-output evaluation without epsilon expiry division or unnecessary
squared-vol overflow/underflow. Preserve actual convention guards and full
clock/displacement metadata through materialization. Record14 does not adopt
this record's remaining off-grid/cube/tail/aging kernels. Record14 was accepted
on **2026-11-08**, with reconciliation/qualification pending; this record's
remaining methodology stays Draft.

## Relationship to the accepted FX forward-input boundary

[CORE-MARKET-15's complete accepted package](../core-market/fx-delta-quotes.md#accepted-decision-one-explicit-fx-quote-profile-and-qualified-forward-inputs)
requires explicit FX pair/profile/clock compatibility and a named qualified
delivery-forward input when the active source is FX delta quotes. Replace the
tenor-as-forward convention and actual caller's zero sentinel; do not infer a
forward from an unrelated tenor or insert one when missing. Resolve the actual
source before its dependencies, so explicit sigma overrides and inactive
branches need no unused FX source/forward. Materialized strike grids retain
their own explicit convention, interpolation and clock; they are not lossless
substitutes for quote-space evaluation. Record 15 was accepted on **2026-11-08**,
with reconciliation/qualification pending; this record's remaining dispatch/
interpolation methodology remains Draft.

## Relationship to the accepted SABR source boundary

[CORE-MARKET-16's complete accepted package](../core-market/sabr-cube-data.md#accepted-decision-canonical-sabr-sources-and-explicit-evaluated-quote-meaning)
keeps SABR cubes as parameter/forward sources with an explicit output request;
Auto/beta inference cannot supply quote units. Preserve exact nodes and shift
Options without alpha/rho repair or missing-shift filling. Off-grid mixed-beta/
shift/forward combinations need an explicitly supported qualified model or
refusal. Require named cube parameter/variance modes, checked failure propagation
and truthful source-clock/coordinate metadata. Materialized sigma grids select
their own interpolation, instead of inheriting a parameter-mode label. Record 16 was accepted on **2026-11-08**, with reconciliation/qualification
pending; this record's complete off-grid/tail/cube kernels remain Draft.

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
