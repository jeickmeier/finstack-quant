# CORE-MARKET-14: Volatility quote grids, convention metadata, and bounded shocks

- Decision status: **Draft — observed implementation, adoption pending.**
- Implementation status: **Observed** in the owning paths identified below; discrepancies are stated explicitly.
- Validation status: **Evidence inspected or located, not executed.** Formula checks, invariants, and round trips are distinguished below; no production accuracy certification is implied.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Choice and rationale

Core `VolSurface` stores an expiry/secondary-axis grid, explicit quote convention, interpolation-mode metadata, and mutable shock operations. It does not evaluate an interpolated volatility in this file. The consuming model evaluator is documented in [volatility-source interpolation](../models-analytic/volatility-source-interpolation.md); declaring Vol or TotalVariance here selects metadata that it consumes.

## Domain, algorithm, and controls

The builder defaults to Strike secondary axis, BlackLognormal quotes and Vol interpolation. Axes must be nonempty, finite and strictly increasing (singletons allowed); expiries are nonnegative and strikes/secondary values may be signed. Every row must align to the full rectangular grid; storage is expiry-major row order. Volatility values are finite and nonnegative, including zero. Black/shifted-Black vols are decimal annualized relative vols; Normal quotes are absolute underlying-rate/price volatility in the caller's units. This layer does not convert their numerical scales.

ShiftedBlackLognormal requires one finite displacement per expiry row. Other conventions must not carry displacements. `with_displacements` switches the convention to shifted Black and validates alignment. Neither constructor checks that every strike/forward plus shift is positive; that depends on evaluator inputs. `require_secondary_axis` and `require_quote_type` produce explicit mismatch errors.

Parallel additive standard shocks set `sigma_b=max(sigma+b,0)`; a parallel multiplicative factor must be finite and nonnegative and scales all quotes. Nonparallel generic bumps error. Point absolute bumps clamp requested coordinates to axis bounds, choose the nearest expiry and nearest secondary knot independently, and floor the shifted node at zero. Equal-distance ties select the first minimum in storage order. Revert restores the saved original node using the same coordinate selection.

Bucket percentage shocks select stored nodes whose coordinates match each optional filter within `1e-9*max(abs(grid_value),1)`; omitted filters select all and empty filters select none. Factor is `1+pct/100`, then each selected value is floored at zero. Percentages below -100 are accepted and collapse positive selected quotes to zero, unlike negative factors passed directly to `scaled`, which error. Overflowing results error. Source snapshots remain unchanged for clone-returning operations.

## Limitations and alternatives

Bounds, monotone total variance, strike convexity and calendar-arbitrage conditions are not repaired or enforced here. Normal and lognormal bumps use the same numerical units; consumers must choose meaningful risk shocks. Clipping is nonlinear and can make central finite differences asymmetric at zero. Nearest-node point bumps are different from interpolation-weighted local quote shocks.

Explicit rejection instead of floors, exact-coordinate-only point selection, and arbitrage-aware repair are possible alternatives, not adopted.

## Evidence

Owning code: [vol_surface.rs](../../../../finstack-quant/core/src/market_data/surfaces/vol_surface.rs), [shared axis validation](../../../../finstack-quant/core/src/market_data/surfaces/mod.rs), [context scratch shocks](../../../../finstack-quant/core/src/market_data/context/ops_bump.rs). Inline tests locate displacement/expiry validation and serialization; [lifecycle tests](../../../../finstack-quant/core/tests/market_data/lifecycle.rs) locate invalid-mutation/source-preservation checks. These are domain/invariant evidence, not independent volatility interpolation/arbitrage tests. [Gatheral](../../../REFERENCES.md#gatheral-volatility-surface) is method context if the relevant consumer uses it; grid storage alone makes no model approval claim.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
