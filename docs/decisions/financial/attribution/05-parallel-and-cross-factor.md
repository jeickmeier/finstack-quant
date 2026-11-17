# ATTRIBUTION-005: Closing-state restoration and pairwise interaction correction

- Decision status: Accepted 2026-11-16 for checked Parallel repricing and cross-factor effects.
- Implementation status: Observed for the source scope listed below. Runtime, cash coverage, pricing, replay and host reconciliation with the accepted contract are pending.
- Validation status: Independent exact examples checked pair signs, zero singles with nonzero interaction, higher-order residual, rate grouping, cash realization and reporting FX. Production attribution and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked Parallel repricing and cross-factor effects

1. Anchor every experiment to the same qualified closing state. At the resolved closing cut, restore each applicable changed factor to its exact opening sources and reprice through the selected pricer and provider. A model-parameter restore uses qualified opening parameters for the same instrument. Apply the same required failure rules under every pair election. Skip only a proven no-op or irrelevant source. Qualified one-sided availability still enters the experiment. [Accepted restoration contract](02-market-factor-restoration.md#accepted-decision-for-checked-economic-factor-families-and-market-restoration).
2. Use held-market Carry under [record 003](03-total-return-carry.md#accepted-decision-for-checked-period-cash-gross-carry-and-funding). For restored state `S` let `W_S` be its qualified holder mark and eligible period cash combined. Compute the single-factor effect as `W_close − W_A` while retaining its price and cash components. Verify that singles and elected pairs account exactly once for `C_close − C_held`. An unsupported cash difference fails complete attribution.
3. Elect rate grouping independently of pair selection. Use combined Rates or an explicitly qualified discount and projection split. A split requires disjoint source ownership, including related fixings and FX bridge inputs. Use the same grouping for singles and pairs. Retire the coupling between all-pairs selection and the rates basis.
4. Elect no pairs, a typed pair list or all applicable pairs. Each pair is unordered. Reject self-pairs, duplicates including reversed duplicates, and overlapping restore scopes. Restore both factors atomically from the closing state. Add the correction `W_A + W_B − W_close − W_AB`. An evaluated zero single-factor effect can still have a nonzero pair effect. Retire the implicit nine-pair list.
5. Use checked Money and bounded widened arithmetic for the full signed combination and accumulation. Retire the fixed `1e−12` pair filter and binary64 cross total. Two-factor closure requires the jointly restored state to reproduce the held basis. Higher-order price interactions can remain in residual even with all applicable pairs. Carry has no separately evaluated time pairs. [Record 013](13-target-currency-and-scaling.md) owns residual qualification.
6. Use the respective staged markets at the closing cut on both sides of a native FX comparison. Apply reporting translation through [record 013](13-target-currency-and-scaling.md#accepted-portfolio-requirement-for-composable-reporting-fx). An elected credit cascade explains the verified standalone Credit effect using its exact restored-credit and closing states. Disclose any par-risk center bridge and final adjustment, and reconcile detail to its parent with cash identified separately. Required detail failure aborts. Explicitly advisory detail may be unavailable with a typed cause. [Credit hierarchy record](11-credit-factor-hierarchy.md).
7. Publish checked Rust evidence with typed factor and pair identities, elected scope, marks and cash, state and cut provenance, skip reasons, actual pricing attempts and reuse. Use serial scheduling by default, with deterministic reduction and error order for independent parallel work. Python and WASM consume the checked result. [Accepted execution contract](01-execution-and-endpoints.md#accepted-decision-for-checked-instrument-attribution-execution-and-endpoints).

Independent examples verified cash single effects of `+1` and `+1` with a pair correction of `−1`, closing to a cash difference of `+1`. Another example closed to P&L of `45` with combined Rates and Credit. Splitting discount and projection left `13` of third-order residual after every pair.

## Observed choice and algorithm

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
