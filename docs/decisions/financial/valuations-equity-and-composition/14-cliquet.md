# VALUATIONS-EQUITY-AND-COMPOSITION-14: Cliquet locked returns, reset anchoring and settlement

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-14`.
- **Decision status:** Accepted 2026-11-14 — complete dated cliquet reset, settlement and risk package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent clipping, accumulation, payment, holder theta and one-reset price and risk checks completed. Production pricing, replay, risk and host qualification remain pending.

## Accepted decision for checked dated cliquet pricing and risk

1. **Specify one immutable option contract and strike-setting event.**
   Elect additive or multiplicative accumulation, a strictly ordered
   nonempty period-end schedule ending no later than the payment date,
   finite local bounds `−1≤L≤C`, finite global bounds `0≤L_g≤C_g`
   and exact signed Money notional in the payment currency. The long unit
   payoff is one nonnegative option amount. Validate terms before any
   literal-zero shortcut. Strike setting establishes a qualified positive
   opening anchor and earns no period return even if the local floor
   is positive. An already-set strike needs its qualified level before
   the first period end.
   [Accepted cliquet payoff](../models-monte-carlo/21-cliquet.md),
   [CBOE cliquet terms](https://www.sec.gov/rules/sro/cboe/2015/34-75425.pdf).

2. **Replay qualified resets with the same transition used in simulation.**
   Require one unique, positive, publication-qualified fixing per
   eligible period end, including a published same-day reset. Preserve
   a required unpublished reset as pending and require a qualified
   prospective event model to price it. Remove generic same-day spot
   substitution and inferred strike anchors. Prepare one checked state
   containing the consumed prefix, actual last fixing and elected
   accumulator. Replay and live simulation update the accumulator
   and anchor atomically. Live spot starts the next path, while the
   previous contractual fixing anchors the next period return.
   [Accepted information cut](../conventions/0045-financial-clocks-and-date-boundaries.md),
   [accepted path preparation](12-path-gbm.md).

3. **Clip locally before accumulation and apply global limits once.**
   Compute `r=(S−A)/A` using the actual preceding fixing `A`
   and current fixing `S`. Clip to the finite local bounds in
   checked scale and advance the anchor to the actual `S` even
   when clipping binds. Additive mode uses the checked signed sum
   of the clipped returns `l`. Multiplicative mode uses the checked
   product of `1+l` minus one, retaining the directly resolved
   `S/A` for unclipped periods. Preserve a positive factor whose
   simple return rounds to `−1` and handle a genuinely zero
   factor exactly. Continue validating required resets after zero
   growth. Apply the global floor and cap only to the completed
   aggregate in checked scale. Multiply by signed Money notional once.
   [Accepted local and global arithmetic](../models-monte-carlo/21-cliquet.md),
   [accepted Money arithmetic](../core-conventions/012-money-decimal-precision.md).

4. **Price through the last live reset and settle one dated claim.**
   Use record 12's explicit fixed-budget `atm_term_gbm` profile
   under `monte_carlo_gbm`, with exact reset nodes and
   positive live geometry. Discount the option cash once on its actual
   payment basis. When all required resets are fixed, finalize the
   same payoff and price known cash directly without spot, volatility
   or simulation. Include eligible unpaid same-day cash at factor
   one under the elected phase. Establish paid or discharged status
   from evidence and require qualified treatment for overdue amounts,
   preserving their original due date. Export the known signed
   claim with its due date and status. A full deterministic export
   requires the payoff to be fixed and reconciled to the actual
   pricing basis.
   [Accepted live GBM](12-path-gbm.md),
   [accepted cashflow export](../valuations-framework/cashflow-export.md).

5. **Use paired selected-model risk and dated DV01.** Freeze the
   strike-setting level, preceding reset anchor and observed history
   across all stencil legs. On the surface route, rebuild responsive
   ATM inputs for same-cut spot and curve shocks on the held
   surface. Compute paired fixed-sample 95% uncertainty from the
   complete within-group stencil in the requested metric's units.
   Apply an elected positive risk half-width tolerance after the
   full fixed budget, with a precision error on a miss. Keep
   sampling, finite-bump and model effects separately qualified.
   Retire the one-sided curve bump registered as `rho`. Use
   signed parallel and bucketed DV01 on actual dated sources.
   Apply a source shock once across its active carry and payment
   roles. A fixed unpaid claim retains any active discount risk.
   [Accepted paired uncertainty](../models-monte-carlo/25-finite-difference-greeks.md),
   [accepted dated DV01](../valuations-framework/finite-difference-rates.md).

6. **Use explicit cuts and one held-spot scenario for horizon theta.**
   Require a positive civil horizon and explicit opening and closing
   information and settlement phases. Elect held opening spot for
   newly eligible reset identities. Replay those observations with
   private labelled hypothetical provenance, retaining actual
   history and anchors. Include a closing-date reset only at its
   admitted phase. Use qualified closing market views for a live
   remainder. Derive period cash and closing claims from this same
   replay and settlement policy and report `C+V_H−V_0`. Preserve
   checked raw prices, cash, provenance and diagnostics through the
   existing Rust, Python, WASM and schema routes.
   [Accepted holder theta](../valuations-framework/carry-theta-breakeven.md),
   [accepted selected-model risk](08-selected-risk-and-implied-vol.md).

## Observed behavior

### Choice and rationale

The cliquet is a long option on locally clipped period returns with additive or multiplicative accumulation and a global nonnegative payoff. Historical returns are locked before simulating future resets.

### Scope and input domain

Increasing nonempty reset dates no later than expiry; ordered local/global bounds, global floor at least zero, finite notional and positive levels. Strictly past resets require initial level and exact fixing; a reset on as-of may use live spot if no fixing is supplied.

### Algorithm

Replay resets ≤ as-of, starting with optional initial level. For every available prior anchor compute `ri=clamp(Si/Sprevious−1,local_floor,local_cap)`, add to locked sum and multiply locked growth by `(1+ri)`. A first reset with no anchor establishes the anchor without adding a return.

If no resets remain, use locked sum or locked growth−1, apply global floor/cap and floor at zero, multiply notional and discount to expiry. Otherwise simulate only through the last future reset using record 12, seed locked return state and the last anchor, and discount the terminal payoff to expiry even when payment is later than the last reset. Dates are inserted into the time grid.

### Selection, controls and conventions

Default `MonteCarloGBM`; explicit `AtmTermGbm`, additive/multiplicative payoff variant. Delta/gamma/vanna use pinned initial spot or same-day fixing. Rho is one-sided discount-curve bump `(PVup−PV)/bump_bp`; fully observed unpaid payoffs retain discount sensitivity. Theta constructs missing fixings for reset dates in `[as_of,rolled_date)` from scenario spot.

### Limitations, failures and alternatives

The nonnegative payoff excludes capital-at-risk negative global floors. Multiplicative clipping is not equivalent to clipping the aggregate growth. Same-day reset fallback differs from autocallable's mandatory same-day fixing. A full-smile forward-start model is an unimplemented alternative to ATM term GBM.

### Evidence

Inline pricing/seasoning cases in [pricer.rs](../../../../finstack-quant/valuations/equity/src/cliquet_option/pricer.rs) and the fully observed rho formula test in [metrics.rs](../../../../finstack-quant/valuations/equity/src/cliquet_option/metrics.rs) locate deterministic and MC regressions. [equity structures reference](../../../REFERENCES.md#overhaus-2007-equity-derivatives) is general context; no independent full-smile cliquet benchmark was established.

## Independent numerical checks

- Strike-set level 100 and resets 125, 75 and 120 give clipped
  returns .15, −.20 and .15 at local bounds `[−.20,.15]`.
  Additive return is .10 and compounded return .058. With
  global bounds `[.04,.075]`, notional −1,000 and payment
  factor .96, the respective cash amounts are −75 and −58 and
  their PVs are −72 and −55.68. Multiplicative holder theta to
  the payment date is −2.32 on either side of the payment
  phase, with the signed −58 present once between closing
  PV and period cash.
- A one-reset benchmark uses spot 103, contractual anchor 100,
  notional 1,000, local bounds `[−.10,.05]`, global bounds
  `[0,.04]`, half-year horizon, fixing factor .99, yield .01,
  Black volatility .20 and payment factor .97. The equivalent
  capped Black call spread gives PV 19.931274653260, analytic
  fixed-anchor delta 1.058629033450 and centered one-unit
  spot difference 1.057849523861.

Repository pricing, replay, risk and host suites remain unrun for
this review.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/equity/src/cliquet_option/types.rs)
- [pricer.rs](../../../../finstack-quant/valuations/equity/src/cliquet_option/pricer.rs)
- [metrics.rs](../../../../finstack-quant/valuations/equity/src/cliquet_option/metrics.rs)
