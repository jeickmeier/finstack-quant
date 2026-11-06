# PORTFOLIO-EXECUTION-013 — Central factor deltas and full-repricing P&L grids

- **Decision status:** Draft — observed implementation; adoption of the specific methodology remains pending. Existing accepted architectural and arithmetic requirements continue to apply.
- **Implementation status:** Observed for the source paths identified below; discrepancies and limits are stated explicitly.
- **Validation status:** Source and selected test evidence inspected or located; tests, builds and numerical benchmarks were not executed.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Choice and scope

`DeltaBasedEngine` computes sensitivities from up/down instrument repricing; its name does not mean a weighted sum of stored Greeks. `FullRepricingEngine` returns scenario P&L profiles across a symmetric grid and does not implement a second sensitivity formula. Both use [the same mapping/routing](12-factor-shocks-and-routing.md), holding scale and bumped-market FX.

## Central difference algorithm

For factor `j`, construct independent markets bumped by `+h_j` and `-h_j` from the base snapshot. For each affected position `i`, obtain raw instrument native PV, convert on that bumped market to reporting currency, and calculate

`delta[i,j] = weight[i] * (PV_baseccy(+h_j) - PV_baseccy(-h_j)) / (2 h_j)`.

Weights must be finite, including when there are no factors. Base PV is not required by this centered formula. Resolved unaffected positions return exact zero unless FX translation is possible. Reject non-finite bumped PV or final weighted delta. Market bump, pricing and FX errors propagate; there is no downward-boundary one-sided fallback, clipping or adaptive bump search. Columns execute through ordered factor-parallel mapping while their position loops remain serial.

The matrix row order is caller position order and column order is factor order. Public portfolio wrappers use position IDs and `Position::scale_factor`, so percentage holdings and shorts use the same economics as NAV. Delta units are base-currency PV per configured factor unit: per bp, per 1% spot/FX move, or per one absolute vol point. The raw helper still crosses `Money::new` and f64 boundaries and uses the instrument raw-value interface, not a caller-supplied selected-pricer/config/provider object.

## Full profile algorithm

Require an odd scenario count in `3..=1001`; the wrapper default is five. Grid coordinates are integer **multiples of bump size**, e.g. `[-2,-1,0,1,2]`, not bp/percent values without the config. Price every base position once, even if the factor list is empty. For each nonzero coordinate `k`, build a fresh `k*h_j` market and calculate

`pnl[k,i] = weight[i] * (PV_baseccy(k*h_j) - PV_baseccy(0))`.

The center row is an explicit zero vector without repricing. Unaffected positions are zero unless FX translation applies. Validate finite endpoints and final weighted P&L. Factors execute in ordered parallel mapping; scenario rows and position rows are sequential inside a factor. The returned profile validates unique position IDs, finite shifts/P&Ls and matching dimensions. Deserialize performs that same validation but does not require caller-supplied shifts to be sorted, symmetric, unique or nonempty: those stronger properties belong to the generated grid.

## Wire and evidence limits

`SensitivityMatrixJson` carries reporting currency, ordered IDs and nested position-by-factor rows. Conversion delegates dimension/axis validation to the factor matrix owner and drops the currency tag because the internal matrix itself has no reporting-currency field. Consumers must retain the tag separately. Profile wire data retains base currency but omits the bump configuration needed to turn multiplier coordinates into absolute shocks.

Automatic differentiation, native Greeks, adaptive/one-sided finite differences and stochastic confidence intervals are possible alternatives, not implemented by these engines. Product RNG settings are not explicitly reset here, so common-random-number behavior depends on the instrument model. Empty factors in the full-profile path still require valid base pricing, whereas the delta path only validates weights; this is an observable API difference.

## Evidence

[delta_engine.rs](../../../../finstack-quant/portfolio/src/sensitivity/delta_engine.rs), [repricing_engine.rs](../../../../finstack-quant/portfolio/src/sensitivity/repricing_engine.rs), [compute.rs](../../../../finstack-quant/portfolio/src/sensitivity/compute.rs) and [json.rs](../../../../finstack-quant/portfolio/src/sensitivity/json.rs). Inspected [integration tests](../../../../finstack-quant/portfolio/tests/factor_model_engines.rs) compare bond DV01 and profile differences with the production bond metric; these are same-kernel reconciliation, not independent pricing accuracy. Inline tests locate analytic toy sensitivities, factor-order failures, grid limits, finite weights and cross-currency translation. [Higham](../../../REFERENCES.md#higham-accuracy-and-stability) and [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) provide numerical context; no convergence study was run.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution). This record does not
authorize a runtime change or certify the observed method.
