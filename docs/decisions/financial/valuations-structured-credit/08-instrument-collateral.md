# VALUATIONS-STRUCTURED-CREDIT-08: Contractual instrument collateral, hazards and liquidity

- **Decision status:** Draft — observed implementation; adoption pending agreement.
- **Implementation status:** Observed in the owning paths below; scope: bond, term-loan and revolving-credit collateral schedules.
- **Validation status:** Evidence inspected/located, not executed. Source and selected assertions establish implemented behavior, not general numerical accuracy or market approval.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Choice and rationale

Instrument collateral reuses contractual bond/loan schedules, then overlays pool credit/prepayment behavior and funding. This avoids reproducing those instruments' contract logic, but schedule bucketing and expected default funding remain separate financial choices.

## Algorithm and controls

Prepared collateral schedules are generated once and bucketed into the first deal payment date on or after the contractual flow. Flows after the final deal payment are assigned to the last bucket. Opening bond amortization and loan historical draws adjust exposure; future floating resets are reprojected with the instrument's rate parameters. Non-cash notional changes remain distinct from cash received.

Per-asset hazard curves supply conditional default probability when present; otherwise the configured deal/path default model supplies it. Explicit instrument or hazard-source recovery remains authoritative; otherwise recovery uses the deal/path specification. Realized copula defaults can sample recovery only when an authoritative instrument/hazard recovery is absent. Expected interest uses the half-period default reduction. Nonrevolver collateral can prepay; revolving collateral does not use that prepayment branch. At default, revolvers can draw undrawn facilities through loan-equivalent/letter-of-credit exposure factors. Actual draws are constrained by committed exposure and available deal liquidity.

Funding first uses applicable reserve resources, then eligible principal; capped allocation shares scarce cash across requests. Diagnostics report unfunded draws. A deterministic funding precheck rejects collateral structures that cannot fund contractual required amounts. Stochastic paths may record the shortfall fraction as a risk output instead.

## Conventions, failures and limitations

Positive collateral cash contributes to collections; funding is an outflow. Contract dates map to deal periods, so the simulation is not an exact event-date cash account. Instrument pools reject reinvestment and statistical-only delinquency/card configurations. Instrument-specific market dependencies and calendars are inherited through the collateral projection path; full closure of public deal dependency reporting is not established by that inheritance.

The last-bucket treatment can accelerate collateral flows beyond legal final; it is an observed behavior needing agreement. Expected hazard overlays do not establish that the underlying instrument schedule and credit exposure are a fully joint model. Opening history replay and future funding schedules require consistency, especially when valuation occurs after closing.

## Alternatives and tradeoffs

Statistical asset rows are supported when detailed contracts are unavailable. Exact event-driven cash-account simulation or explicit sale values at legal final would differ from period bucketing and cannot be inferred from these schedules. The chosen reuse reduces contract duplication but adds responsibility for matching instrument and deal timing.

## Implementation and validation evidence

- [structured_credit/types/collateral.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/collateral.rs)
- [structured_credit/pricing/simulation_engine/instrument_flows.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/instrument_flows.rs)
- [structured_credit/pricing/simulation_engine/pool_flow_source.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/pool_flow_source.rs)
- [structured_credit/pricing/simulation_engine/simulate_period/cash.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/simulate_period/cash.rs)

Existing evidence: [structured_credit/unit/instrument_collateral_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/instrument_collateral_tests.rs); [structured_credit/unit/instrument_pool_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/instrument_pool_tests.rs); [structured_credit/unit/funding_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/funding_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

No independent joint contract/default/funding benchmark was established.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
