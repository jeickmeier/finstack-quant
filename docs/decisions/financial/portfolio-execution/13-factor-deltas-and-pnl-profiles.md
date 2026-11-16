# PORTFOLIO-EXECUTION-013 — Central factor deltas and full-repricing P&L grids

- **Decision status:** Accepted 2026-11-15 — complete qualified raw repricing, factor secant, symmetric grid, checked arithmetic, stochastic coupling, deterministic failure and provenance package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent secant, grid, signed profile, intermediate range and draw alignment examples verified. Production pricing, convergence, restoration and host qualification remain pending.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Accepted decision for checked portfolio factor deltas and profiles

1. **Use one qualified repricing setup.** Bind the immutable opening
   portfolio and market, selected model, pricer, settings, provider,
   effective dates and reporting FX basis through the shared executor.
   Build each endpoint independently from the opening state. Require
   finite unscaled raw PV from the selected pricer and apply the endpoint's
   qualified cash FX directly. Reuse requires both native and reporting
   dependency proof, including FX bridge curves. Distinguish a proven
   unaffected zero from a computed zero.
   [Raw endpoints](04-execution-order-and-reduction.md#accepted-decision-for-checked-portfolio-execution-and-nav-reduction)
   and [shock routing](12-factor-shocks-and-routing.md#accepted-decision-for-checked-portfolio-factor-shocks-and-routing).

2. **Define delta as a two-endpoint secant in the factor coordinate.**
   For signed holding multiplier `w`, unscaled reporting-currency raw PVs
   `V+` and `V−`, and admitted factor amplitudes `a+` and `a−`, compute
   `w × (V+ − V−) / (a+ − a−)`. Symmetric amplitudes `±h` have width `2h`
   and require no center PV. Report currency per declared factor unit and
   retain the delivered changes. A separately elected scalar-price Greek
   uses the actual quote endpoints under its owning metric contract.
   [Finite differences](../valuations-framework/finite-difference-rates.md#accepted-decision-for-checked-finite-differences-and-rate-risk).

3. **Require an explicit symmetric P&L grid.** Accept an odd integer count
   from 3 through 1,001 and retire the implicit five-point default. Generate
   consecutive integer multipliers `−m` through `m`. Check each signed
   displacement `k × h` and its delivery before use. Compute
   `w × (V_k − V_0)` and reuse a qualified opening PV wherever its complete
   basis matches. A holding proven unaffected across the grid needs no price
   calls. The center is literal `+0` without a separate reprice. An empty
   factor list needs no base pricing. An empty book still validates the
   requested shocks and returns the declared grid with empty holding rows.

4. **Check the complete difference, scale and quotient.** Preserve
   representable final results even when an intermediate difference, width
   or product would overflow or underflow. Reject an unrepresentable final
   value and a nonzero result lost to zero. Preserve exact zero after
   required endpoint admission, including for a zero-sized holding.

5. **Require verified common draws for stochastic repricing.** The model
   owner establishes matching stream addressing, path groups, time nodes,
   draw slots and antithetic legs at every used endpoint and any required
   base. Equal seeds alone are insufficient. Refuse unqualified coupling.
   A returned secant or profile carries no confidence or convergence claim.
   [Qualified stochastic repricing](../valuations-framework/option-sensitivities.md#accepted-decision-for-checked-option-and-fx-sensitivities).

6. **Make failures atomic and deterministic.** Validate request axes and
   checked output and work counts before allocation. Use the executor's
   one-parallel-axis rule. Report admission failures first, required base
   failures in holding order, and endpoint failures in declared factor,
   displacement and holding order. Preserve typed causes and coordinates.
   An invalid endpoint fails the report. Retire clipping, adaptive retry
   and one-sided substitution as implicit responses to failure.

7. **Preserve units, grid and provenance through restoration.** Publish
   ordered holding and factor axes, reporting currency, factor definitions
   and units, requested and represented displacements, and valuation and
   FX evidence. Derive the grid from admitted count and bump. One bounded
   Rust reader checks shape, finite values and profile identity, including
   the zero center, for existing host readers. Python, WASM and UI preserve
   the report basis. Covariance consumers check definitions and units as
   well as axis order. Structural restore verifies the report contract.
   Verifying its prices requires the historical inputs and replay.
   [Auditable results and replay](../../project/0025-auditable-results-and-replay.md#decision).

## Independent calculation checks

- For `V(x)=100+4x+x²`, `w=−1.5` and `h=2`, the two raw endpoints
  are 112 and 96, giving a delta of `−6`. A five-point grid gives
  `[0, 6, 0, −18, −48]`. The first zero comes from equal PVs, and
  the middle zero is the grid identity.
- For PV endpoints `±1e308`, `w=2` and `h=2`, the final delta `1e308`
  is finite even though naive endpoint subtraction overflows. A
  separate `±1e308` amplitude example gives a secant of 1 even though
  both naive width and difference overflow.
- An identical additive sample at both endpoints cancels in the toy
  stencil. Reading different draw slots from the same seed changes the
  result. Grid sizes 3, 5 and 1,001 give the expected exact integer axes.

## Observed choice and scope at the audit revision

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

Implementation reconciliation and qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution).
