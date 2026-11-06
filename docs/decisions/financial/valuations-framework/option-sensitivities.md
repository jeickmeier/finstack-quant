# VALUATIONS-FRAMEWORK-11 — Native-model and common-random-number option Greeks, FX01 and surface buckets

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in the source paths cited below.
- Validation status: Evidence inspected/located, not executed; no runtime checks or external accuracy comparison performed.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and actual selection

The option Greek registry can use a native product provider, a selected-model provider, or generic finite differences. A nondefault selected model calls `selected_model_greek`; its default implementation errors, preventing an automatic native-model substitution. A provider may explicitly return `None` when native Greeks remain applicable after lifecycle resolution. Sparse unavailable Greeks stay unavailable, not zero. Non-daily theta requests route to the common horizon roll. Native daily theta stores zero cash carry, the returned theta as roll-down, and one day of horizon.

## Finite-difference algorithm and units

Generic spot delta/gamma use the first declared market scalar, require positive finite spot, and take an absolute shock `h=max(S×relative_bump,1e−8)`, applying exactly the matching relative shock. Delta is `(V+−V−)/(2h)` and gamma `(V+−2V0+V−)/h²`. Clone the instrument with `mc_seed_scenario="greeks_crn"` and price every stencil point with the same dispatch and seed scenario, including recomputing the base for gamma. A missing mutable override container errors for these generic calculators; common random numbers do not guarantee low bias or sufficient paths.

Vega bumps the active scalar vol override or all present declared surfaces by absolute `k`. It reports `(V+−V−)/(2k×100)`, per one volatility point. If relevant minimum vol is at most `k`, switch to first-order `(V+−V0)/(k×100)`. The relevant footprint is the two strike columns bracketing each declared reference strike across all expiries, or the full surface when no reference strike is declared. This optimization assumes that footprint describes the actual pricer lookup. Missing all declared surfaces errors; partially present surfaces are the bumped set.

Volga uses `(V+−2V0+V−)/(100k)²`; vanna uses four spot/vol corners divided by `4h×100k`. Their symmetric volatility step shrinks to at most half the relevant positive minimum vol; zero active vol errors. They return zero on/after expiry. Nonfinite results fail. Selected Hull–White rate delta/gamma use parallel rate shocks but normalize by the decimal-rate width, yielding per-rate-unit or per-rate-unit-squared results rather than DV01.

FX01 applies a fixed ±1% relative move to instrument-provided quote overrides when both directions are supported, otherwise to all declared FX pairs. It reports `(V+−V−)/2`, PV per 1% move. No FX dependencies gives zero. This fixed shock is separate from the configurable generic spot shock.

## Surface bucket algorithm

`KeyRateVega` uses all actual surface nodes by default; requested coordinates must be unique increasing exact source nodes. For each surface/expiry/strike, bump only that node, centrally or one-sided when the surface minimum is below the shock. Combine heterogeneous strike grids in a union-column matrix with zero in absent cells. Return the raw sum; record `BucketedVegaResidual=parallel_vega−raw_sum` and warn if its magnitude exceeds 10% of the parallel amount. No proportional rescaling forces reconciliation. An active scalar override produces parallel quote vega while market-surface node shocks can remain zero; the residual exposes the uncovered quote risk.

## Limits, alternatives and evidence

Analytic/pathwise/AAD Greeks are alternatives only where product providers implement them. Matching a production closed form or reusing a seed is useful but does not independently validate the stencils around exercise, barrier and volatility boundaries.

- [fd_greeks.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/fd_greeks.rs), [option_greeks.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/option_greeks.rs), [provider trait](../../../../finstack-quant/valuations/core/src/instruments/common_impl/traits/option_greeks.rs), [vega.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/vega.rs) and [fx01.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/fx01.rs) contain the inspected implementation and inline behavioral/formula tests.
- [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) provides common-random-number context; [Hull](../../../REFERENCES.md#hull-options-futures) provides Greek conventions. Neither establishes adoption of these particular grids, shock sizes or fallback thresholds.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
