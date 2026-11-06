# Equity total-return futures projection

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-04`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

The projected total-return-future quote combines observed index points, accumulated distributions/funding and a simple remaining-term spread adjustment. Accrued components are caller-supplied market scalars, not recomputed financing histories.

## Scope and input domain

Listed total-return-future terms with finite point scalars, positive spot, a unitless spread quoted in basis points and an explicit spread day count. The result is in the terms' settlement currency and quote points.

## Algorithm

Compute `τ=max(year_fraction(as_of,settlement),0)` and `s=spread_bp×10^-4`. The model quote is `S + accrued_distributions − accrued_funding + S s τ`. The model spot derivative is `1+sτ`; spread derivative per basis point is `Sτ×10^-4`. Multiply derivatives by signed listed-contract point delta. Resolved mark/PV delegate to shared listed-future terms, so quoted marks and settlement observations may replace the projection.

## Selection, controls and conventions

The three metrics are resolved FuturesPrice, model Delta and model Spread01. There is no discount curve dependency. Point scalars accept monetary values only in settlement currency or unitless amounts; spread specifically requires unitless data. No residual cashflow schedule is exported.

## Limitations, failures and alternatives

No term-structure integration or funding/day-by-day index reconstruction occurs. A fixed mark can suppress repricing risk while analytic model derivatives remain nonzero. An explicit term-integrated spread or exchange accrual engine would be a different unimplemented method.

## Evidence

Inline tests in [types.rs](../../../../finstack-quant/valuations/equity/src/equity_total_return_future/types.rs) locate projection and metric identity cases. [Hull futures reference](../../../REFERENCES.md#hull-options-futures) supplies general futures context, not independent verification of a specific exchange total-return-index formula.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/equity/src/equity_total_return_future/types.rs)
- [metrics.rs](../../../../finstack-quant/valuations/equity/src/equity_total_return_future/metrics.rs)
