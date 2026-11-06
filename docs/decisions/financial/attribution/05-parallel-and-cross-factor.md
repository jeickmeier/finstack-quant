# ATTRIBUTION-005: Closing-state restoration and pairwise interaction correction

- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the source scope listed below.
- Validation status: Source evidence inspected; named test evidence located, not executed. No numerical accuracy certification.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and algorithm

Parallel attribution separates economic methodology from scheduling. At closing date/state, restore one factor to opening state and define its contribution as `V1 - V(restore A)`. Carry separately reprices opening parameters and frozen opening market at closing date, then applies period cash. The method name does not force parallel execution: Serial is the default policy and parallel maps preserve factor reduction order.

Default factors are combined rates, credit, inflation, correlations, FX, volatility, model parameters and market scalars. With `full_cross_attribution`, discount and forward are separate factors whose contributions both enter rates. Declared-unused families are excluded. A family with no opening data is skipped; if closing data exists a note says its move remains in residual. Opening model parameters default to the current instrument snapshot.

For each eligible pair, restore both opening factors at once and calculate `X_AB = V(restore A)+V(restore B)-V1-V(restore A,B)`. This is the correction added to the sum of closing-state marginal factors, not the opposite mixed-difference sign. Default pairs are Rates×Credit, Rates×Vol, Spot×Vol, Spot×Credit, FX×Vol, FX×Rates, Credit×Vol, Rates×Inflation and Credit×Correlations. Full-cross uses all pairs of active members of its nine-factor set (at most 36); it does not include carry/time cross terms or third/higher interactions. Pair amounts of absolute size at most 1e-12 currency units are omitted from detail and aggregate cross P&L. Pair totals accumulate as f64 before Money construction.

An optional credit hierarchy reprices cumulative shift endpoints on the closing market with opening hazards, then differences consecutive endpoint values. Its additive detail does not replace the independently calculated credit aggregate. See the credit hierarchy record for eligibility and curve-shape treatment.

## Failure, conventions and controls

Closing model-parameter restoration failure is noted and omitted on the default path but fails a full-cross run; market and cross repricing errors propagate. `strict_validation` does not alter this policy. Default tolerances are 1 currency unit and 0.1 percent. Logical endpoint count remains two even with prepared endpoints. Instrument-native FX P&L measures matrix impact inside pricing; reporting translation is separate, although the stamped FX note still uses broader exposure-and-translation wording.

## Rationale and alternatives

Independent factor repricing allows ordered parallel work and explicit pair diagnostics. It is not an exact allocation of all nonlinear interactions, and selecting full-cross changes the rates split and pair set. Waterfall is a supported alternative; higher-order inclusion/exclusion or permutation averaging is unimplemented. The existing rates-credit synthetic test checks the implemented pair identity; no general multi-factor accuracy claim follows.

## Source and evidence

- [parallel.rs](../../../../finstack-quant/attribution/src/parallel.rs) — owning implementation.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). This record does not
authorize a runtime change or certify the observed method.
