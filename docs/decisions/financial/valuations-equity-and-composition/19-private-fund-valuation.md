# Private-fund residual valuation, LP performance and parameter shocks

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-19`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

Fund PV is future net LP waterfall cashflows plus an optional present NAV mark. Performance metrics combine realized LP allocations with this residual value. The same request shares its allocation ledger, so metrics use one consistent contractual allocation.

## Scope and input domain

The fund's event and waterfall domains are record 18. Optional NAV must match fund currency and cannot coexist with future distribution/proceeds events; future contributions may remain. A configured discount curve prices future net cashflows; omitted curve deliberately means undiscounted summation.

## Algorithm

PV retains LP cashflows strictly after as-of, discounts them through the configured curve when present, and adds NAV. Fully realized funds with no NAV therefore have zero residual value. LP IRR solves dated historical net LP flows through as-of plus current residual PV at as-of when its magnitude exceeds 1e-6. DPI is paid LP distributions/contributions; TVPI adds residual PV to the numerator. Both return zero when contributions are at most 1e-6. GPCarryTotal sums GP payments through as-of; CarryAccrued reads the last cumulative gross-carry ledger row.

Carry01 shifts every catch-up/promote GP share by ±1 bp, resetting promote LP share to its complement. It returns half the PV difference, using a one-sided difference at [0,1] boundaries and zero if neither side is possible. Hurdle01 moves all preferred/promote hurdles ±1 bp and returns half the PV difference. Nav01 scales every distribution/proceeds event, including historical events, and NAV by ±1%, preserving contributions, reruns the waterfall and returns half the PV difference.

## Selection, controls and conventions

XIRR uses waterfall day count and shared solver defaults. The schedule export returns full net LP ledger flows, labelled contractual with ACT/365F schedule metadata even when waterfall IRR day count differs. The quoted 01 outputs are currency changes for the specified bp or percent shock, not unscaled derivatives.

## Limitations, failures and alternatives

Residual PV can be negative because future contributions remain. LP IRR/TVPI depend on projected cashflow PV and discount choice when explicit NAV is absent. Nav01 is a full realization/NAV scaling scenario, not solely a shock to today's NAV; historical changes can alter future carry. Hurdle01 has no one-sided boundary repair. Alternative performance bases, fee accounting and NAV-only shocks require agreement.

## Evidence

Inline [types.rs](../../../../finstack-quant/valuations/equity/src/pe_fund/types.rs) checks realized zero value, projected-flow/NAV modes and currency errors; [metrics.rs](../../../../finstack-quant/valuations/equity/src/pe_fund/metrics.rs) includes exact near-zero-rate IRR, fully realized DPI/TVPI identities and request-local ledger sharing. These are local examples and solver consistency, not independent fund administrator reconciliation.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/equity/src/pe_fund/types.rs)
- [metrics.rs](../../../../finstack-quant/valuations/equity/src/pe_fund/metrics.rs)
