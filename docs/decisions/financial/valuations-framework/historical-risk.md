# VALUATIONS-FRAMEWORK-14 — Historical scenario repricing, directional Taylor risk, VaR and expected shortfall

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in the source paths cited below.
- Validation status: Evidence inspected/located, not executed; no runtime checks or external accuracy comparison performed.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and input domain

Historical risk uses equally weighted supplied scenarios at the current valuation date, either full model repricing or a second-order directional Taylor approximation. Default confidence is 95% and default method is full revaluation. The input is a list of instrument references, not an independently weighted portfolio; instrument economics must already represent the requested position. No horizon scaling, decay weighting, volatility normalization or history construction is performed. `base_date` and `window_days` are descriptive history metadata, not filters.

## Scenario and valuation algorithm

1. Rate/credit shifts are absolute decimal changes, converted to bp. Discount and forward shifts use triangular key-rate bumps whose neighbors come from the standard bucket grid. Equity and FX shifts are relative returns; volatility shifts are additive absolute volatility at exact expiry/strike grid nodes.
2. Repeated curve shifts are applied in successive rounds so the context cannot discard duplicates. Duplicate volatility-node shifts are added before applying one node update. A volatility result below zero or a non-grid point fails rather than clamps. Credit shifts are grouped by hazard curve and passed as tenor quote bumps to a required recalibration provider, using the original market as source and already-shocked market as target; this preserves scenario rate/credit interaction.
3. Replace the historical scenario's date with `as_of`; all prices are valued on that unchanged date. Full revaluation computes each scenario price minus its base price, using compensated summation across instruments. The selected pricing dispatch is preserved. Mixed-currency inputs require an explicit reporting currency; base and shocked PVs use their respective FX markets under cashflow-date conversion, so FX translation risk contributes to P&L.
4. Taylor mode first filters shifts by declared dependencies (and keeps FX shifts when currency conversion is needed). Let `f(a)` be the reporting-currency price with all retained scenario shifts multiplied by `a`. Set `h=1/max(1,max|shift|/unit)`, using unit 1bp for rates/credit and 0.01 for spot/vol. Shrink `h` to half the nearest symmetric positivity boundary for spot/FX/vol. Estimate P&L at full scenario scale as `(f(h)−f(−h))/(2h) + [f(h)−2f(0)+f(−h)]/(2h²)`. This captures directional cross terms through actual joint repricing; it is not a sum of separately stored Greeks.
5. If the symmetric volatility boundary is zero, use forward points at `h/3`, `2h/3`, `h`: with the reduced `h`, first derivative is `(-11f0+18f1−9f2+2f3)/(6h)` and second is `(2f0−5f1+4f2−f3)/h²`; combine first plus half second. Invalid full negative-vol/spot scenarios fail even when a local stencil could be evaluated.

## Tail estimator and conventions

Sort finite scenario P&Ls ascending. With confidence `c` strictly in `(0,1)`, use `p=1−c` and the core linearly interpolated quantile on index `(n−1)p`. VaR is `min(Q(p),0)`: **negative loss P&L**, not a positive loss amount. ES is `min((1/p)∫₀ᵖQ(u)du,0)`, integrating the same piecewise-linear quantile with trapezoids and compensated summation. It is not the arithmetic average of observations below VaR. One observation gives its loss-clipped value; an empty result distribution gives zero. An empty instrument list returns empty/zero risk; nonempty instruments with empty history error. Fewer than 20 scenarios warn but are accepted. HVar and ES metric calculators cache one another and record scenario count.

## Limitations, alternatives, and evidence

Taylor accuracy depends on smoothness, scenario size, dependency completeness, model noise and exact source quote semantics. It still performs scenario-direction repricing per instrument and can require full credit recalibration at stencil points. Full repricing is the implemented alternative. Weighted historical estimators, discrete empirical-tail ES and filtered historical simulation are unimplemented alternatives needing an explicit method decision.

- [var_calculator.rs](../../../../finstack-quant/valuations/core/src/metrics/risk/var_calculator.rs), [market_history.rs](../../../../finstack-quant/valuations/core/src/metrics/risk/market_history.rs), [hvar.rs](../../../../finstack-quant/valuations/core/src/metrics/risk/hvar.rs), and [risk_factors.rs](../../../../finstack-quant/valuations/core/src/metrics/risk/risk_factors.rs) own these algorithms and units. Inline tests cover distribution/formula and market-shift behavior; they were inspected/located, not executed.
- [McNeil–Frey–Embrechts](../../../REFERENCES.md#mcneil-frey-embrechts-qrm) supplies risk-model context; the particular interpolated-tail estimator and directional stencil remain repository choices awaiting approval.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
