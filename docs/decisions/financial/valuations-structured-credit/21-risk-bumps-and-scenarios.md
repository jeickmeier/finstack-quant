# VALUATIONS-STRUCTURED-CREDIT-21: Risk bumps, breakeven default and scenario grids

- **Decision status:** Draft — observed implementation; adoption pending agreement.
- **Implementation status:** Observed in the owning paths below; scope: rate/spread/credit sensitivities and deterministic scenario summaries.
- **Validation status:** Evidence inspected/located, not executed. Source and selected assertions establish implemented behavior, not general numerical accuracy or market approval.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Choice and rationale

Risk metrics use several distinct experiments. Fixed-cashflow duration/convexity measures discount sensitivity; effective summary metrics rebuild cashflows under shifted discount and forward curves; credit-model bumps rerun the deal. Sharing the label “duration” does not make these methods equivalent.

## Algorithm and numerical conventions

Macaulay duration is PV-weighted Act/365F payment time. Modified duration uses a one-sided discount bump, default +1 bp, on fixed cashflows; convexity uses symmetric shifts, default ±10 bp. Both accept a finite positive `ytm_bump_bp`, with registry overrides from bump configuration. Tranche-summary effective duration/convexity shifts all discount and forward curves by ±1 bp and reruns the projection, including floating/reset/trigger effects. Registered DV01 delegates to the common combined-curve calculator; key-rate DV01 uses triangular buckets.

Z-spread CS01 is P&L from a +1 bp spread shift. Spread duration/convexity use fixed-flow discount-spread sensitivity. Bucketed CS01 distributes cashflow sensitivity linearly by payment time across buckets; it is not an independent node-by-node spread-curve repricing. Its total can reconcile while local market-curve behavior remains untested.

Default01 and Prepayment01 reproject up/down models and return `(PV_up - PV_down) / achieved_width × 0.0001`, targeting a 1 bp annual-equivalent probability change and transforming SDA/PSA peak multipliers and vectors as needed. Vector bump normalization uses maximum achieved change; cumulative-loss inputs scale their terminal loss. Constant upward model perturbations are not all capped at one at the bump boundary, so validation can reject edge cases. Recovery01 uses clamped up/down recovery changes and `(PV_up - PV_down) / achieved_width × 0.01`, one percentage point rather than one bp; Severity01 reverses recovery direction. Severity-vector effective recovery in helper calculations is an unweighted mean of severities.

Breakeven CDR bisects [0,1] under an assumed monotonic loss relation, with 1e-5 rate tolerance and loss tolerance 1e-4 of original notional. Scenario tables enumerate caller grids with a 10,000-cell cap; constant scenario defaults/prepayment replace deal models, while asset-level overrides remain active. They are stress scenarios, not a calibrated probability distribution.

## Domain, failures and alternatives

Currency/finite guards apply to returned summaries. Zero PV, zero principal and no-solution cases use method-specific errors or zeros; caller intent must choose the desired risk experiment. Supported alternatives are fixed-flow versus reprojected metrics and explicit scenario grids. Independently bumped spread curves and a uniform boundary-probability policy would be different methodologies; central credit-model bumps with achieved-width normalization are already implemented.

## Implementation and validation evidence

- [structured_credit/metrics/risk/duration.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/duration.rs)
- [structured_credit/metrics/risk/convexity.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/convexity.rs)
- [structured_credit/metrics/risk/spreads.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/spreads.rs)
- [structured_credit/metrics/risk/default01.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/default01.rs)
- [structured_credit/metrics/risk/prepayment01.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/prepayment01.rs)
- [structured_credit/metrics/risk/recovery01.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/recovery01.rs)
- [structured_credit/metrics/risk/severity01.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/severity01.rs)
- [structured_credit/metrics/risk/breakeven_cdr.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/breakeven_cdr.rs)
- [structured_credit/metrics/risk/mod.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/mod.rs)
- [structured_credit/metrics/scenario.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/scenario.rs)
- [structured_credit/metrics/summary.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/summary.rs)
- [structured_credit/metrics/mod.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/mod.rs)

Existing evidence: [structured_credit/unit/metrics/risk_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/metrics/risk_tests.rs); [structured_credit/test_bucketed_dv01.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/test_bucketed_dv01.rs); [structured_credit/unit/config_sensitivity_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/config_sensitivity_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

Convergence and independent economic risk references were not established.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
