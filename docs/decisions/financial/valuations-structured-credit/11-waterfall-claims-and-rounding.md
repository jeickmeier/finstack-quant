# VALUATIONS-STRUCTURED-CREDIT-11: Waterfall claims, fee priority, PIK and integer rounding

- **Decision status:** Draft — observed implementation; adoption pending agreement.
- **Implementation status:** Observed in the owning paths below; scope: standard and custom waterfalls.
- **Validation status:** Evidence inspected/located, not executed. Source and selected assertions establish implemented behavior, not general numerical accuracy or market approval.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Choice and rationale

The waterfall computes each contractual claim then distributes finite interest/principal resources by priority. Integer currency-unit pro-rata allocation preserves cash totals despite rounding. These mechanical properties support reproducibility; fee ranking, claim definitions and PIK capitalization remain financial choices.

## Algorithm and conventions

The standard template pays leading fees, debt interest in seniority/input order, junior fees/reserve replenishment, sequential principal, optional net-WAC carryover and incentive fees, then residual. Principal defaults to principal funding; other tiers default to interest. CLO/CBO defaults permit principal support for leading fees and nondeferrable note interest. Senior notes are nondeferrable by default; callers can override.

Claims include fixed cash, annualized percentage of collateral/special-serviced balance, tranche coupon plus deferred interest, principal to target balance, reserve top-up, net-WAC carryover and incentive fee. Fee annualization defaults to Act/360. Standard trustee fees are annual amounts divided by payment frequency; nonpositive/nonfinite standard bp fees are silently omitted. The payment coupon claim uses the balance after the period's realized-loss allocation, before waterfall principal payment, and the note's day count. Separate accrued-interest metadata preserves the opening pre-writedown balance. Interest shortfall can defer or capitalize as PIK; net-WAC cap uses live collateral WAC less selected fees and tracks cap carryover separately.

Sequential tiers track principal already paid so repeated claims cannot pay the same balance twice. Pro-rata tiers convert available cash and caps to integer currency units, water-fill weighted recipients up to caps, then distribute remainders deterministically. If all weights are zero the allocator uses equal weights. Nearest/floor/ceiling claim rounding are supported; recorded shortfall uses a small numerical threshold.

Incentive fees test the equity XIRR against a hurdle. Hurdle shortfall is found by up to 64 bisections with 0.005 currency-unit width; failed XIRR candidates count as not reaching the hurdle, and a failed final money conversion returns the original candidate. Remaining eligible cash above the shortfall is shared with the manager.

## Failures, alternatives and limitations

Custom waterfalls reject duplicate priorities, invalid references, duplicate interest owners, inconsistent coverage tiers and simultaneous deal-level fees. Equity must use the residual recipient rather than a debt tranche ID. Sequential/pro-rata and funding sources are explicit supported alternatives. Exact fractional allocation without currency rounding would differ economically at small notionals. Nonconventional equity cashflows can break monotonic IRR assumptions; standard-fee omission and equal-zero-weight allocation are observed fallbacks, not agreed permissions.

## Implementation and validation evidence

- [structured_credit/pricing/waterfall.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/waterfall.rs)
- [structured_credit/types/waterfall/mod.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/waterfall/mod.rs)
- [structured_credit/types/waterfall/builder.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/waterfall/builder.rs)
- [structured_credit/types/structured_credit_impl.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/structured_credit_impl.rs)
- [structured_credit/types/setup.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/setup.rs)
- [structured_credit/pricing/simulation_engine/simulate_period/period_waterfall.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/simulate_period/period_waterfall.rs)
- [structured_credit/utils/validation.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/utils/validation.rs)

Existing evidence: [structured_credit/waterfall_golden.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/waterfall_golden.rs); [structured_credit/waterfall_properties.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/waterfall_properties.rs); [structured_credit/unit/components/rounding_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/components/rounding_tests.rs); [structured_credit/unit/coupon_claim_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/coupon_claim_tests.rs); [structured_credit/unit/fee_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/fee_tests.rs); [structured_credit/unit/afc_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/afc_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

Hand-built golden waterfalls are not external legal-document validation.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
