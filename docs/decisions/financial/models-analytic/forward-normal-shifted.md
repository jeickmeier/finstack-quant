# MODELS-ANALYTIC-02: Forward Black, normal and shifted pricing and sensitivities

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

European forward calls/puts; Black requires F,K>0, normal permits finite negative F,K, shifted Black requires F+s,K+s>0. Volatility and time are finite and nonnegative.

## Choice and rationale

Black uses lognormal forward dynamics, Bachelier arithmetic forward dynamics; shifted Black translates both F and K by a fixed displacement. These are explicit variants, not automatic negative-rate fallbacks.

## Algorithm

Black uses d1=[ln(F/K)+σ²T/2]/(σ√T), d2=d1−σ√T and F N(d1)−K N(d2). Normal uses d=(F−K)/(σN√T), C=(F−K)N(d)+σN√T φ(d). Puts use direct negative-tail expressions. Zero vol/time returns intrinsic. Delta is N(d1) or N(d), gamma φ(d1)/(Fσ√T) or φ(d)/(σN√T), vega F√Tφ(d1) or √Tφ(d). At zero vol/time delta(call)=1 when F≥K, put delta=call−1, gamma/vega=0. Shifted price and vega call the Black kernels on translated coordinates.

## Selection and controls

`black76_price` multiplies by caller df>0; `bachelier_price` and shifted price remain undiscounted. `bachelier_price_with_annuity` applies the supplied multiplier without separate annuity validation. `black76_greeks` and `bachelier_greeks` return unit, undiscounted forward Greeks. Raw leaf functions return NaN on invalid domain; checked wrappers return Result errors.

## Conventions

Lognormal σ is relative annual volatility; normal σN has forward units per √year. Vega is per unit volatility (not divided by 100). A shifted quote must retain displacement metadata and must not be interpreted as ordinary Black on negative inputs.

## Limitations, failure and fallback

No automatically chosen shift, annuity, discount curve or sensitivity to the shift is provided. Deterministic ATM deltas are conventions, not differentiability claims. Extremely scaled coordinates can overflow despite finite individual inputs.

## Alternatives and tradeoffs

Normal and shifted Black both support negative unshifted rates, with different tail and unit conventions. Translation retains lognormal upper/lower support constraints; normal is unbounded. Adoption by product remains outside these standalone formulas.

## Implementation and validation evidence

[black.rs](../../../../finstack-quant/models/src/closed_form/black.rs), [bachelier.rs](../../../../finstack-quant/models/src/closed_form/bachelier.rs), [forward.rs](../../../../finstack-quant/models/src/closed_form/forward.rs). Inline parity, positive-vega and zero-vol boundary tests establish identities; `black76_price_matches_discounted_leaf_formula` is a production-kernel comparison. [vol_models_quantlib_tests.rs](../../../../finstack-quant/models/tests/vol_models_quantlib_tests.rs) contains independent price fixtures. References: [Black 1976](../../../REFERENCES.md#black-1976), [Bachelier](../../../REFERENCES.md#bachelier-1900).

## Relationship to the accepted quote-grid boundary

[CORE-MARKET-14's complete accepted package](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks) retains relative Black
sigma versus Normal sigma in actual underlying units, explicit displacement
and literal source coordinates. Black/shifted-Black adapters require finite
positive transformed forward/strike coordinates; Normal retains finite signed
inputs. Attaching shifts cannot retag unchanged quotes as a price-preserving
conversion. Keep actual zero-time/zero-vol pricing branches at this model owner,
without fabricated implied-vol grid nodes. Record14 was accepted on
**2026-11-08**, with reconciliation/qualification pending; this record's
remaining formula/limit policies stay Draft.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
