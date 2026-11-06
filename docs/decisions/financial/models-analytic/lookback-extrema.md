# MODELS-ANALYTIC-06: Continuous fixed and floating lookbacks with observed extrema

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

Continuous-lookback call/put formulas with supplied historical maximum/minimum, constant carry/volatility. Dispatch validates positive spot/extremum; fixed strikes must be positive, floating strikes only finite and are unused.

## Choice and rationale

Closed-form reflected distributions price extrema and historical intrinsic. An analytic carry-zero branch avoids division by r−q using a 1e−7 switching threshold.

## Algorithm

Repair max to max(history,S) and min to min(history,S). Fixed call first discounts (max−K)+, then replaces strike with max(K,max) for the future maximum premium; fixed put analogously uses min(K,min). Add vanilla-like terms and reflected CDF terms scaled by σ²/[2(r−q)]. Use log-CDF products for image terms. For |r−q|<1e−7 substitute explicit normal-density/log-ratio limit formulas. Floating call uses the analogous minimum expression; floating put is fixed-call maximum premium plus max exp(−rT)−S exp(−qT). At expiry return observed-extremum intrinsic; zero vol updates extremum with deterministic terminal forward. Floor prices at zero.

## Selection and controls

Explicit fixed/floating selector; dispatch default constant is fixed. There is no numerical solver or user-selectable monitoring scheme. Raw functions check finiteness but not the full positive-domain constraints enforced in dispatch.

## Conventions

Prices are per underlying unit; history extrema share spot units. Continuous monitoring over the remaining interval is assumed; annual decimal carry/volatility and supplied year time.

## Limitations, failure and fallback

Silently repairing inconsistent historical extrema changes supplied state. Negative raw times/volatility take deterministic branches, while checked dispatch rejects them. No discrete fixing schedule, stochastic volatility or jump-extrema law is included.

## Alternatives and tradeoffs

Discrete-path MC can represent actual monitoring but introduces discretization and sampling error; it is not the formula selected here. The analytic carry-zero limit avoids numerical cancellation at a chosen threshold.

## Implementation and validation evidence

[lookback.rs](../../../../finstack-quant/models/src/closed_form/lookback.rs) and [dispatch.rs](../../../../finstack-quant/models/src/closed_form/dispatch.rs). `test_r_eq_q_limiting_branch_matches_general_formula`, boundary-continuity tests, seasoned-intrinsic cases and `fixed_strike_lookback_otm_matches_monte_carlo` provide limit/invariant/numerical-comparison evidence. [lookback_commodity_regressions.rs](../../../../finstack-quant/models/tests/lookback_commodity_regressions.rs) is additional regression evidence. [Haug](../../../REFERENCES.md#haug-2007-option-formulas) is the formula catalogue; table-level provenance for every branch is not established.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
