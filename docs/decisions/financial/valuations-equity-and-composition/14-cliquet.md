# Cliquet locked returns, reset anchoring and settlement

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-14`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

The cliquet is a long option on locally clipped period returns with additive or multiplicative accumulation and a global nonnegative payoff. Historical returns are locked before simulating future resets.

## Scope and input domain

Increasing nonempty reset dates no later than expiry; ordered local/global bounds, global floor at least zero, finite notional and positive levels. Strictly past resets require initial level and exact fixing; a reset on as-of may use live spot if no fixing is supplied.

## Algorithm

Replay resets ≤ as-of, starting with optional initial level. For every available prior anchor compute `ri=clamp(Si/Sprevious−1,local_floor,local_cap)`, add to locked sum and multiply locked growth by `(1+ri)`. A first reset with no anchor establishes the anchor without adding a return.

If no resets remain, use locked sum or locked growth−1, apply global floor/cap and floor at zero, multiply notional and discount to expiry. Otherwise simulate only through the last future reset using record 12, seed locked return state and the last anchor, and discount the terminal payoff to expiry even when payment is later than the last reset. Dates are inserted into the time grid.

## Selection, controls and conventions

Default `MonteCarloGBM`; explicit `AtmTermGbm`, additive/multiplicative payoff variant. Delta/gamma/vanna use pinned initial spot or same-day fixing. Rho is one-sided discount-curve bump `(PVup−PV)/bump_bp`; fully observed unpaid payoffs retain discount sensitivity. Theta constructs missing fixings for reset dates in `[as_of,rolled_date)` from scenario spot.

## Limitations, failures and alternatives

The nonnegative payoff excludes capital-at-risk negative global floors. Multiplicative clipping is not equivalent to clipping the aggregate growth. Same-day reset fallback differs from autocallable's mandatory same-day fixing. A full-smile forward-start model is an unimplemented alternative to ATM term GBM.

## Evidence

Inline pricing/seasoning cases in [pricer.rs](../../../../finstack-quant/valuations/equity/src/cliquet_option/pricer.rs) and the fully observed rho formula test in [metrics.rs](../../../../finstack-quant/valuations/equity/src/cliquet_option/metrics.rs) locate deterministic and MC regressions. [equity structures reference](../../../REFERENCES.md#overhaus-2007-equity-derivatives) is general context; no independent full-smile cliquet benchmark was established.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/equity/src/cliquet_option/types.rs)
- [pricer.rs](../../../../finstack-quant/valuations/equity/src/cliquet_option/pricer.rs)
- [metrics.rs](../../../../finstack-quant/valuations/equity/src/cliquet_option/metrics.rs)
