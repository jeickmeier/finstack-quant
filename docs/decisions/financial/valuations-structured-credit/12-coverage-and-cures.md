# VALUATIONS-STRUCTURED-CREDIT-12: Coverage collateral values, ratios and cure diversion

- **Decision status:** Draft — observed implementation; adoption pending agreement.
- **Implementation status:** Observed in the owning paths below; scope: OC, IC and borrowing-base tests inside the waterfall.
- **Validation status:** Evidence inspected/located, not executed. Source and selected assertions establish implemented behavior, not general numerical accuracy or market approval.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Choice and rationale

Coverage tests use a period snapshot and divert available interest at their actual waterfall position. The cure is calculated against the principal order the deal will pay. This makes position and numerator definition part of the methodology, not merely report formatting.

## Algorithm and conventions

OC numerator combines performing collateral adjusted for rating/discount/CCC rules, principal cash, restricted qualifying accounts and modeled value of pending default claims. Default claims carry recovery value by default or configured market-value percent of defaulted par. The CLO helper uses a 7.5% CCC threshold and an 80% discount-obligation price threshold; it is not automatically a universally applicable covenant. Additional CCC excess reduction is applied beyond ordinary row adjustments. Unrated instrument/new-cohort handling differs from rated rows; eligibility is not established by those differences.

OC divides by the tested note plus all notes senior in payment priority. IC uses interest collections less senior fees divided by current coupon/deferred claims of tested and senior classes. The no-market helper can use floating spread alone; the simulation has market-aware claims. Zero denominator produces an absent/degenerate result rather than an ordinary finite ratio.

A failed test sizes the principal paydown needed to cure, traversing actual principal recipients. Tests sharing a tier divert once based on the maximum required cure, not the sum. Diversion is limited by available interest and optional percent cap. `PayDownSenior` immediately pays senior principal; `Reinvest` transfers interest to the principal account for permitted use. Placement defaults after the tested tranche's interest, with explicit alternatives after another tranche or junior fees.

Legacy tranche triggers retain breach state and cure hysteresis: once breached they remain so until `cure_level` (or the trigger level) is reached. Consequences can divert, trap, accelerate or stop reinvestment. Explicit test tiers must follow relevant interest and share action/diversion cap within a tier.

## Limitations and alternatives

Ratios are multiples (1.2 = 120%); haircut fractions differ from pct thresholds. Coverage math is contractual and cannot be approved solely by comparing it with a generic CLO example. Market-value and recovery valuation, test placement/action and custom thresholds are supported alternatives. The expected collateral valuation and forward coupon estimates do not establish stress calibration; snapshot ratios do not represent intraperiod breaches.

## Implementation and validation evidence

- [structured_credit/pricing/coverage_tests.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/coverage_tests.rs)
- [structured_credit/pricing/waterfall.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/waterfall.rs)
- [structured_credit/pricing/simulation_engine/triggers.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/triggers.rs)
- [structured_credit/pricing/simulation_engine/simulate_period/losses.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/simulate_period/losses.rs)
- [structured_credit/types/waterfall/coverage.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/waterfall/coverage.rs)
- [structured_credit/types/tranches.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/tranches.rs)

Existing evidence: [structured_credit/unit/coverage_rules_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/coverage_rules_tests.rs); [structured_credit/unit/coverage_position_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/coverage_position_tests.rs); [structured_credit/unit/trigger_consistency_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/trigger_consistency_tests.rs); [structured_credit/unit/components/coverage_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/components/coverage_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
