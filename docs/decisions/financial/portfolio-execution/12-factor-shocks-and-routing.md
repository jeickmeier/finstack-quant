# PORTFOLIO-EXECUTION-012 — Factor units, market shocks and dependency routing

- **Decision status:** Draft — observed implementation; adoption of the specific methodology remains pending. Existing accepted architectural and arithmetic requirements continue to apply.
- **Implementation status:** Observed for the source paths identified below; discrepancies and limits are stated explicitly.
- **Validation status:** Source and selected test evidence inspected or located; tests, builds and numerical benchmarks were not executed.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Choice and rationale

A factor definition describes a market transformation; its declared factor type supplies the bump unit while actual market storage determines which instruments can be skipped. These are separate decisions. A rate-like label does not prove that an ID resolves to a discount curve, and dependency optimization must follow the object the generic bump will actually change.

## Bump formulation and controls

Per-factor overrides precede `BumpSizeConfig` type defaults. Rates/credit/inflation/custom factors use basis points; equity/commodity/FX use percent; volatility uses percent interpreted as absolute volatility points. Defaults are one bp or one percent/vol point respectively. The engine rejects non-finite bump size or `|h| < f64::EPSILON`; negative nonzero sizes are accepted.

Mapping is explicit:

- Parallel curve mappings require exact agreement between factor and mapping units, then apply additive parallel bumps to each listed ID.
- Bucketed curves require rate-bp units. For each ordered tenor/weight pair, form a triangular key-rate bump of `h * weight`, taking the immediately preceding/following tenor as support boundaries. The adapter consumes that order; it does not fit a new basis or independently sort the grid.
- Equity spot mappings convert the bump to a standard fraction and apply multiplier `1+fraction` through the generic curve/surface/scalar/series resolver.
- FX mappings convert fraction to percent and bump the specified pair at `as_of`.
- Volatility mappings require unit agreement and apply additive parallel generic-ID bumps. The generic resolver's name-collision precedence remains relevant.

## Exact versus conservative routing

Resolve curve IDs to actual Discount, Forward, Hazard, Inflation or BaseCorrelation storage and create corresponding dependency keys. Price, BasisSpread and Parametric storage, missing curves and unresolvable classifications cause conservative all-position routing. For EquitySpot, only a scalar or series with no same-named curve/surface admits exact routing. VolShift requires an ordinary surface and no same-named curve; other volatility-source representations are not assumed supported by this mapping.

Instrument dependencies are extracted once and credit-index constituents resolved on the supplied market. Extraction/resolution failure includes that instrument for every nonempty mapping. An ambiguous mapping includes all positions. An exact empty mapping produces no affected positions. Any FX mapping intersects every declared FX dependency, including possible triangulation. The engines also reprice/reconvert all rows for FX mappings to capture reporting-currency translation even for instruments without native FX dependencies.

Free portfolio sensitivity/profile functions reject Credit factors with an empty parallel curve list because those mean assignment-driven hierarchy factors. The direct engine APIs do not apply that wrapper check and can return zero columns/profiles for empty mappings. Assignment-based CS01/beta semantics are owned by [the portfolio factor bridge](../portfolio-attribution/12-factor-assignment-and-credit-exposures.md).

## Limitations and alternatives

Exact routing relies on complete instrument declarations and core bump semantics; it is not an independent detection of all state read by a price. Missing market objects still generally fail when the bump is applied; conservative routing does not substitute absent data. Direct engines do not independently validate every factor configuration/axis invariant enforced by a higher-level `FactorModelConfig`.

A declared-type-only router would be simpler but can skip real exposures under misclassified definitions; it is not the observed algorithm. A universal full-reprice router is an implemented conservative outcome, trading work for correctness. The default bump size is a convention, not convergence evidence.

## Evidence

[traits.rs](../../../../finstack-quant/portfolio/src/sensitivity/traits.rs), [delta mapping](../../../../finstack-quant/portfolio/src/sensitivity/delta_engine.rs), [wrapper dispatch](../../../../finstack-quant/portfolio/src/sensitivity/compute.rs), [factor config](../../../../finstack-quant/factor/src/config.rs) and [factor type units](../../../../finstack-quant/factor/src/primitives/factor_types.rs). Inline tests locate actual-storage routing, missing-storage conservative behavior, FX triangulation and unit mismatch. Wrapper tests locate empty-curve credit rejection. These validate routing contracts and finite-shock construction, not economic calibration of factors.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution). This record does not
authorize a runtime change or certify the observed method.
