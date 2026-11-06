# CORE-MARKET-09: Basis-spread level interpolation and rolling

- Decision status: **Draft — observed implementation, adoption pending.**
- Implementation status: **Observed** in the owning paths identified below; discrepancies are stated explicitly.
- Validation status: **Evidence inspected or located, not executed.** Formula checks, invariants, and round trips are distinguished below; no production accuracy certification is implied.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Choice and rationale

`BasisSpreadCurve` interpolates finite spread levels supplied by a caller. Its documented intended convention is the continuously compounded zero-rate difference `z_foreign(t)-z_domestic(t)`, in annual decimal rates. It is a market-data representation, not a cross-currency or rate-basis calibration model. Basis spreads may be signed; consumers must preserve that orientation and define their financial application.

## Algorithm, domain, and controls

The builder requires base date and nonempty input, defaults to Act/365F, Linear interpolation and FlatZero (constant boundary spread), and delegates knot/domain validation to the chosen shared interpolation kernel. Input is not sorted here. MonotoneConvex is explicitly prohibited; logarithmic styles require positive values even though the generic signed-spread representation permits negatives under Linear/Hermite. Builder `knots` replaces prior points rather than extending them.

`spread(t)` evaluates the interpolator, or the saved original interpolator at `t+offset` after a forward roll. It does not convert units or apply spread to discount factors. Although the documentation specifies a continuous zero spread, the curve has no currency, leg, or unit tag enforcing that convention.

Roll changes base by signed calendar days and computes the shift with stored day count. It discards nonpositive remaining maturities, errors when every knot expires, and rebuilds with retained source interpolation. Unlike price/CPI rolling, it does not insert a new zero-time anchor. For a negative cumulative roll offset the shared roll state is dropped and current surviving pillars are reinterpolated. Serialization validates retained source samples against visible pillars at the shared relative 1e-12 tolerance.

## Failure, alternatives

Point lookup returns floating values, so no-extrapolation or invalid query inputs can return NaN. Context rolling supports this curve; generic context bumping explicitly reports unsupported. That is an observed asymmetry, not permission to infer a bump definition. The number of supplied points ultimately must satisfy the shared interpolator even though this builder initially checks only nonempty input.

An additive signed-spread bump, exact carry-preserving backward roll, or typed spread-unit contract would require new decisions. Linear and other shared interpolation choices are implemented alternatives; selecting a discount-factor-style log-forward kernel for raw spreads should require a specific economic rationale.

## Evidence

Owning code and inline tests: [basis_spread_curve.rs](../../../../finstack-quant/core/src/market_data/term_structures/basis_spread_curve.rs), [roll machinery](../../../../finstack-quant/core/src/market_data/term_structures/common/roll.rs), [context dispatch](../../../../finstack-quant/core/src/market_data/context/curve_storage.rs). Inline tests and [heterogeneous lifecycle tests](../../../../finstack-quant/core/tests/market_data/lifecycle.rs) are implementation invariants; no independent basis calibration benchmark is asserted. Shared kernels are documented in [value interpolation](../core-numerics/value-interpolation.md). [Fujii–Shimada–Takahashi](../../../REFERENCES.md#fujii-shimada-takahashi-2010) and [multi-curve bootstrapping](../../../REFERENCES.md#ametrano-bianchetti-2013) provide contextual alternatives, not proof that raw spread interpolation calibrates those models.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
