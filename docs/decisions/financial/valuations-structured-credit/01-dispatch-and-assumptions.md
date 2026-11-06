# VALUATIONS-STRUCTURED-CREDIT-01: Deal resolution, dispatch and assumption presets

- **Decision status:** Draft — observed implementation; adoption pending agreement.
- **Implementation status:** Observed in the owning paths below; scope: cash securitization constructors, validation and deterministic/stochastic entry points.
- **Validation status:** Evidence inspected/located, not executed. Source and selected assertions establish implemented behavior, not general numerical accuracy or market approval.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Choice and rationale

The implementation resolves a deal into one normalized collateral representation before simulating its contractual waterfall. Ordinary `Instrument::base_value` and `run_simulation` use projected deterministic cashflows. Stochastic flags describe available specifications; they do not make every ordinary price or metric a Monte Carlo result. Separate stochastic entry points build scenario configuration and execute Monte Carlo. This separation is observed; adopting it as the public model-selection contract remains open.

## Scope and input domain

`StructuredCredit` covers CLO/CBO, RMBS, CMBS, ABS/auto/card terms. Standard constructors exist for a smaller explicit family table, while the builder supports the complete enum. Resolution requires closing < first payment ≤ legal maturity; recovery/prepayment/default specifications validate before pricing. Normalization, same-currency collateral, reserves, custom waterfalls, calls, draws and specialized collateral terms have further validations. A supplied calendar must resolve; schedule conventions differ by deal family.

## Algorithm, selection and controls

1. Generic `CreditModelConfig` deserialization/builder defaults come from the embedded assumption registry: constant CPR 20%, CDR 2%, recovery 60%, and a 12-month recovery lag.
2. `new_standard` chooses a registry profile by deal type, checks pool/family consistency and uses profile frequency and first payment one frequency after closing, capped at maturity. Example profiles include monthly RMBS PSA 100%, CDR 0.6%, recovery 60%, lag 18 months; quarterly CLO CPR 20%, CDR 2%, recovery 60%, lag 12 months; monthly ABS speed 1.5%, CDR 2%, recovery 45%, lag 12 months. Fees remain absent until explicitly set or `with_standard_fees` is called.
3. `resolved_for_pricing` validates dates and credit model, normalizes the pool, and validates initial recovery claims and reinvestment criteria. Instrument lifecycle validation also checks ancillary rules and nested instruments before market access.
4. Projection uses deal day count: RMBS/CMBS 30/360, other families Act/360. Tranche coupon accrual has its own day count. Discounting uses relative factors from the chosen discount curve.
5. Stochastic convenience presets replace prepayment/default specifications and correlation. RMBS uses the Richard Roll specification; CLO/CBO use factor-correlated prepayment and Gaussian-copula defaults; CMBS uses deterministic CPR 2% and Gaussian defaults CDR 2%, correlation 20%; ABS/auto/card require a constant deterministic default model for the convenience preset.

## Limitations, failure and alternatives

The assumption registry validates shape/ranges, not empirical calibration. Its constructor, stochastic, CMO and fee profiles are different data sets; no common economic meaning should be inferred from the word “standard.” Embedded required-profile lookup can panic if packaged data are invalid. Explicit deal inputs and custom waterfall rules are supported alternatives. A model calibrated from observed collateral or market prices is a possible alternative but no such calibration fit is performed by these preset helpers. Generic builder versus standard-constructor behavior and missing-calendar behavior do not establish approved intended uses.

## Implementation and validation evidence

- [structured_credit/assumptions.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/assumptions.rs)
- [structured_credit/types/constructors.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/constructors.rs)
- [structured_credit/types/resolved.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/resolved.rs)
- [structured_credit/types/instrument.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/instrument.rs)
- [structured_credit/types/pricing_methods.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/pricing_methods.rs)
- [structured_credit/types/stochastic.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/stochastic.rs)
- [structured_credit/pricing/stochastic/calibrations.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/stochastic/calibrations.rs)

Existing evidence: [structured_credit/unit/constructors_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/constructors_tests.rs); [structured_credit/unit/validation_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/validation_tests.rs); [structured_credit/unit/stochastic_pricing_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/stochastic_pricing_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

No empirical calibration or approval was established. The embedded JSON data are material methodology dependencies even though the source manifest counts Rust files only.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
