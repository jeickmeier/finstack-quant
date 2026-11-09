# MODELS-ANALYTIC-14: Deterministic multistart SABR fit, weights, shifts and ATM pinning

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

Single-expiry fit to ≥3 aligned positive volatility quotes with fixed user β. Normal β0 permits negative rates without shift; shifted calibration works on shifted positive F,K.

## Choice and rationale

Fit transformed parameters with deterministic Levenberg–Marquardt multistart and explicit relative-quote acceptance. Optional ATM pinning solves α inside a two-parameter fit; this is a numerical Newton solve, despite a source comment calling it analytic.

## Algorithm

Weights use market Black or Bachelier vega, divide by max, floor relative weights at1e−8, normalize total, and residual factor=sqrt(weight/total)/market_vol. Nine starts combine ν={.15,.4,.9},ρ={−.6,0,.6}, with α seeded from ATM quote×F^(1−β) (normal α=quote). Optimize log α and logistic ν∈(.001,2),ρ∈(−.99,.99). Invalid model evaluations get residual1e6. Choose lowest residual among candidates passing finite residual norm and every quote relative error≤configured tolerance. ATM quote sorts strikes, interpolates variance in strike, and clamps to endpoint outside grid. Pinning solves α by up to50 Newton steps, relative α bump1e−6, derivative floor1e−14, ±50% step cap; then fits ν,ρ and rechecks every quote.

## Selection and controls

Default relative tolerance1e−4,max iterations2000; high_precision1e−8/200. Inner LM tolerance=min(config,1e−10). Auto shift only triggers when min rate<0; select first of .005,.01,.02,.03,.04 providing .001 headroom; larger need errors. None and Fixed are explicit alternatives. Zero fixed shift means unshifted; nonzero final shift must satisfy parameter validation. Outcome records quote errors, winning start, iterations and bounds proximity; residual evaluations belong to winning start, not total.

## Conventions

Input quotes must already match β/shift convention; no price conversion is performed. Relative tolerance scales with each quote, normal or lognormal. Shift and rate are decimals.

## Limitations, failure and fallback

Duplicate strikes are not rejected and conflicting duplicate quotes can fail fit. Auto shift does not fix exactly zero lognormal rates. ν bounds exclude deterministic ν=0. Pinning an interpolated ATM can conflict with an otherwise exactly representable discrete smile and cause rejection. No smoothing, cross-expiry calibration or arbitrage constraint is in this objective.

## Alternatives and tradeoffs

Free α and ATM-pinned variants are supported. Alternative price/vega weighting, β calibration or global no-arbitrage surface fitting are unimplemented choices.

## Implementation and validation evidence

[calibration.rs](../../../../finstack-quant/models/src/volatility/sabr/calibration.rs) and [tests.rs](../../../../finstack-quant/models/src/volatility/sabr/tests.rs). Deterministic repeatability/known-smile recovery tests are synthetic round trips; explicit rejected-stalled-start tests check final acceptance. Reference: [SABR formulation](../../../REFERENCES.md#hagan-2002-sabr); the exact bounds/weights/multistart policy lacks separately established adoption provenance.

## Relationship to the accepted SABR source boundary

[CORE-MARKET-16's complete accepted package](../core-market/sabr-cube-data.md#accepted-decision-canonical-sabr-sources-and-explicit-evaluated-quote-meaning)
preserves nu=0 and exact canonical parameter units/domains in published sources.
A fitter's selected positive-nu bounds do not invalidate stored deterministic
limits or authorize post-fit source floors, caps or retagging. Preserve actual
fit quote conventions, displacement, source clock and construction provenance;
separate those input conventions from a later explicit evaluated output.
Record 16 was accepted on **2026-11-08**, with reconciliation/qualification
pending. Its acceptance does not adopt this record's objective, bounds,
multistart, ATM pinning or fit acceptance policies.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
