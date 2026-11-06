# CORE-MARKET-02: Term-forward projection and arithmetic forward averages

- Decision status: **Draft — observed implementation, adoption pending.**
- Implementation status: **Observed** in the owning paths identified below; discrepancies are stated explicitly.
- Validation status: **Evidence inspected or located, not executed.** Formula checks, invariants, and round trips are distinguished below; no production accuracy certification is implied.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Choice and rationale

The forward curve exposes three different quantities: point rate, arithmetic time-average rate, and simple forward implied by chained projection DFs. Keeping them separate avoids treating an average as a contractual compounded return. The actual construction is explicit left-endpoint simple compounding, not integration of an instantaneous forward.

## Algorithm, domain, and conventions

The builder requires a base date, finite positive tenor, nonnegative reset lag, at least two already-ordered knots and finite rate values. It defaults to Linear interpolation and FlatZero (constant boundary forward). It rejects MonotoneConvex; positive-domain interpolation styles still reject nonpositive source rates. An optional projection grid must start within 1e-12 of zero, be finite, nonnegative, strictly increasing, contain at least two entries and cover the last interpolation knot.

For `df(t)`, start at zero and accumulate `logDF -= log1p(F(start)*dt)` for successive grid boundaries, truncating the final interval to `t`. After the explicit grid, continue in numeric `tenor` steps; without a grid all steps use numeric tenor. Each simple-compounding denominator `1+F(start)*dt` must be finite and positive. Failure to advance time is an error. Exponentiation must produce a positive finite DF. `rate_between(t1,t2)` computes `expm1(logDF(t1)-logDF(t2))/(t2-t1)` with finite, strictly ordered, nonnegative projection times.

`rate_between_dates(start,end,accrual_year_fraction)` rejects historical starts before base and returns projection growth divided by the **contractual accrual fraction**, converting from the curve clock. By contrast, `curve_time` clamps dates at or before base to zero, and `rate_period_between_dates` uses that conversion for an arithmetic average. Reset lag is stored convention metadata; these lookup methods do not apply business-day reset adjustments themselves.

`rate_period(t1,t2)` accepts finite ordered times, including negative times. Intervals up to 1e-12 use `rate(t1)`. Otherwise composite Simpson integration uses 8 panels through 5 years, 16 through 20 years, and 32 above 20, divided by interval length. For transformed curves, the source interpolator is integrated on the shifted clock, multiplied by its scale, and the piecewise-linear shock area is integrated exactly. There is no tolerance-driven refinement or split at source interpolation knots.

## Selection and controls

Identifier inference uppercases/trims and matches substrings. SOFR/FedFunds/EFFR/ESTR/EURIBOR/SARON select Act/360; SONIA/TONA(R)/TIBOR/CORRA/CDOR/AONIA/BBSW/BKBM select Act/365F; recognized currency prefixes supply a fallback, otherwise Act/365F. OIS/overnight names imply reset lag zero; supported term-index substrings imply two business days. Explicit builder fields override inference. Name inference is a convenience policy, not a registry-backed convention guarantee.

`DiscountCurve::to_forward_curve` samples `(DF(t)/DF(t+tenor)-1)/tenor` at source knots and builds a new forward curve (Linear unless overridden), preserving base and day count. This is resampling into a new projection representation; exact reconstruction away from compatible reset grids is not guaranteed. Rebuilding forward knots extends any existing projection grid by numeric tenor and discards the old evaluation transform. Rolling shifts the rate function and surviving grid; it does not assert conditional projection-DF identity within a partially elapsed simple-compounding reset interval.

## Failure, alternatives, evidence

Point-rate lookup may propagate NaN from no-extrapolation or nonfinite queries. Finite negative rates are allowed subject to the selected kernel and positive projection denominators. Supported explicit grids represent contractual reset dates more faithfully than a fixed numeric tenor; a discount-factor-native projection curve or adaptive knot-split integration would be different, unimplemented choices here.

Owning code: [builder](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/builder.rs), [convention inference](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/conventions.rs), [projection/average kernels](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/curve.rs), [transforms](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/transform.rs), and [wire validation](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/wire.rs). Inspected [tests](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/tests.rs) independently multiply simple accrual factors, check off-grid contractual resets, verify decimal contractual growth across Act/360 and Act/365F, distinguish all three rates on a steep curve, and verify exact triangular shock area. These are formula/invariant evidence; no convergence study for the fixed Simpson grid was located. References: [multi-curve construction](../../../REFERENCES.md#ametrano-bianchetti-2013), [Numerical Recipes](../../../REFERENCES.md#press-numerical-recipes).

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
