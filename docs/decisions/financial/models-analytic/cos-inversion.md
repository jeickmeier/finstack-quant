# MODELS-ANALYTIC-10: COS truncation, payoff coefficients and strip inversion

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

European options with a user-supplied log-return characteristic function/cumulants; positive S,K,T and finite discount rate. Wrappers expose BS, Merton jump and variance-gamma models.

## Choice and rationale

Fang–Oosterlee cosine expansion prices puts directly and calls by the CF forward moment. This avoids exponentially growing call-payoff coefficients; the selected finite expansion is not automatically convergence-certified.

## Algorithm

Window [a,b]=c1±L√(c2+√|c4|); reject radicand≤1e−12 or invalid interval. Cache Ak=Re[φ(kπ/(b−a))exp(−ikπa/(b−a))]. Shift window by ln(S/K), integrate put payoff over [aX,clamp(0,aX,bX)] using analytic χ (exp×cos) and ψ (cos) integrals. Sum with half-weight k=0 and factor 2/(b−a), Neumaier accumulation, then multiply K exp(−rT). Call=put+df[S Reφ(−i)−K]. Validate finite CF evaluations/forward moment/discount/output.

## Selection and controls

Default N=128,L=10; N in 1..65536, L finite positive. Wrapper n_terms overrides N; generic `CosPricer` exposes both. Scalar delegates to strip. There is no N/L refinement loop. BS wrapper requires strictly positive σ even when deterministic pricing could be calculated separately.

## Conventions

CF is for log return, not log price; its drift must match the separately supplied discount rate and carry. Cumulants are at the requested maturity. Unit premiums are discounted once.

## Limitations, failure and fallback

All finite negative prices are clamped to zero; materially negative put raw values trigger warning above K×1e−10+1e−12, but are still accepted as zero. Generic CF contract does not enforce φ(0)=1, martingale matching or positive-definite law. Near-deterministic distributions reject instead of falling back.

## Alternatives and tradeoffs

Numerical Fourier quadrature is implemented for Heston in a separate API. Increasing N or L trades accuracy/cost but can require joint tuning; adaptive bounds would be an unimplemented policy.

## Implementation and validation evidence

[cos.rs](../../../../finstack-quant/models/src/fourier/cos.rs) and [mod.rs](../../../../finstack-quant/models/src/fourier/characteristic_function/mod.rs). Inline `cos_matches_bs_call_itm_otm`, `cos_put_matches_bs` and strip/single tests compare against library BS; invalid-input/degenerate-window checks are invariants. Reference: [Fang–Oosterlee](../../../REFERENCES.md#fang-oosterlee-2008).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
