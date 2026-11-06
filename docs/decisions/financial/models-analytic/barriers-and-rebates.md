# MODELS-ANALYTIC-05: Continuous single barriers, touch probability and rebate timing

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

Up/down knock-in/out calls/puts under constant GBM, a single continuous barrier, and optional expiry/hit rebate helpers. Checked dispatch requires positive S,K,H and nonnegative finite T,σ.

## Choice and rationale

Reiner–Rubinstein-style image terms price knock-ins; knock-outs use vanilla minus knock-in. Separate first-passage probability and discounted-hit calculations distinguish rebate cash-flow timing.

## Algorithm

Already-hit is inclusive S≥H (up) or S≤H (down); knock-in becomes vanilla and knock-out zero. Otherwise compute μ=(r−q−σ²/2)/σ², four direct/image terms A,B,C,D using x,x1,y,y1, and select combinations by call/put, barrier side and K versus H. Reflection products use log normal CDF to limit overflow. Knock-out is max(vanilla−knock-in,0). Touch probability sums a direct normal CDF and reflected CDF and clamps [0,1]. Zero vol follows deterministic S exp((r−q)t), including deterministic hit time. Expiry rebates are df×rebate×hit probability for knock-outs or survival probability for knock-ins. At-hit knock-out value uses first-passage Laplace transform with √(ν²+2rσ²). If this discriminant is negative, adaptive Simpson on normalized survival uses tolerance 1e−12/depth 48. Expired at-hit knock-out rebate is zero because paid earlier; knock-in failure rebate remains at expiry.

## Selection and controls

Barrier type and call/put are explicit; `with_df` derives a constant rate. Knock-in AtHit requests still route to expiry rebate because failure is known only at expiry. `expired_barrier_payoff` takes caller-supplied breach history. Checked convenience dispatch validates more than raw helpers, which validate mainly S,K,H or use arithmetic directly.

## Conventions

Rates are continuous annual decimals, σ relative annual volatility, T years. Rebate helpers return per-unit present value and do not implicitly add it to barrier option price. Historical breach state is not inferred from current spot alone.

## Limitations, failure and fallback

Only continuous single-level monitoring is represented. Raw helper invalid inputs can yield NaN; numerical integration failure returns NaN. Hit discount is clamped to [0,max(1,exp(−rT))]; knockout flooring can hide small parity errors. No discrete-monitoring correction is present.

## Alternatives and tradeoffs

Discrete trees/MC with bridge corrections are possible alternatives, not selected here. Expiry and hit rebate variants are implemented and materially different under nonzero rates.

## Implementation and validation evidence

[barrier.rs](../../../../finstack-quant/models/src/closed_form/barrier.rs) and [dispatch.rs](../../../../finstack-quant/models/src/closed_form/dispatch.rs). Inline Haug-labelled cases, eight-way in/out parity and deterministic/timing regressions are located; `test_haug_2007_down_in_call` is reference-labelled evidence whose original table provenance is not requalified. Reference: [Reiner–Rubinstein](../../../REFERENCES.md#reiner-rubinstein-1991).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
