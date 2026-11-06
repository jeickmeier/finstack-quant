# MODELS-ANALYTIC-04: Geometric Asian law and arithmetic lognormal moment matching

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

Fixed-strike calls/puts on an unseasoned equal-weight average under constant GBM. Count-based dates are iT/n; explicit fixing-time variants accept positive nondecreasing times, including duplicates.

## Choice and rationale

Geometric averaging is priced through its exact lognormal marginal; arithmetic averaging uses Turnbull–Wakeman-labelled two-moment lognormal matching. Arithmetic is the dispatch default constant. Exactness of the geometric marginal does not make arithmetic matching exact.

## Algorithm

For geometric average, μ=ln S+(r−q−σ²/2) mean(ti), variance=σ² Σij min(ti,tj)/n², evaluated with sorted-time weights 2(n−1−i)+1. Its expected level exp(μ+variance/2) is passed to Black with standard deviation √variance and unit time. Count n=0 means continuous geometric average with σ/√3. Arithmetic first moment is mean[S exp((r−q)ti)]. Second moment uses E[Si Sj]=S² exp((r−q)(ti+tj)+σ² min(ti,tj)); a prefix sum reduces the double sum to O(n), with Neumaier accumulation. Match log variance ln(m2/m1²), price Black on m1 and discount. Degenerate variance returns discounted forward intrinsic.

## Selection and controls

Explicit-time methods derive one rate −ln(df)/T from df>0. Checked dispatch rejects zero fixings; the raw arithmetic count function instead returns 0 for n=0. Raw geometric n=0 is continuous averaging. Expiry shortcut precedes fixing-list/df validation in explicit-time functions. Arithmetic output clamps to [0,df m1] for calls or [0,df K] for puts.

## Conventions

Annual decimal volatility/rates and model-year fixing times; each fixing has equal weight and payment discount is supplied/derived at expiry. No historical fixings or partial accumulated sum are inputs.

## Limitations, failure and fallback

Explicit-time validation does not require fixing times≤payment expiry: a future observation beyond payment is accepted. Duplicate dates are counted with multiple equal weights. Moment matching may misprice arithmetic tails; first/second moments are not a proof of price accuracy.

## Alternatives and tradeoffs

Geometric is an implemented alternative. Monte Carlo, Curran/Levy approximations, and explicit seasoned-average formulas would have different costs and domains; these are not automatic substitutes in this dispatch.

## Implementation and validation evidence

[asian.rs](../../../../finstack-quant/models/src/closed_form/asian.rs) and [dispatch.rs](../../../../finstack-quant/models/src/closed_form/dispatch.rs). `test_second_moment_on_algorithm_matches_brute_force` checks O(n) versus double-sum moments; `test_geometric_continuous_limit_matches_discrete_and_kemna_vorst` checks the limiting formula. Arithmetic positivity/parity checks are weaker than independent pricing evidence. References: [Kemna–Vorst](../../../REFERENCES.md#kemna-vorst-1990), [Turnbull–Wakeman](../../../REFERENCES.md#turnbull-wakeman-1991).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
