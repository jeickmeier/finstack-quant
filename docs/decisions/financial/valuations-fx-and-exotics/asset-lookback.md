# VALUATIONS-FX-AND-EXOTICS-ASSET-LOOKBACK: Lookback extrema, continuous formulas and monitored GBM paths

- Record: `VALUATIONS-FX-AND-EXOTICS-ASSET-LOOKBACK`
- Decision status: **Accepted 2026-11-15** — complete asset lookback terms, observation, pricing, settlement and risk package.
- Implementation status: **Observed** in the owning paths below. Reconciliation to the accepted contract is pending.
- Validation status: Independent seasoned analytic, discrete payoff, retained cash and conditional bridge calculations checked. Runtime, market data, numerical and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for asset lookback pricing and settlement

1. **Book an explicit European cash payoff and monitoring schedule.** Require
   holder long or short, call or put, finite positive quantity, and a
   qualified positive-price equity or spot index in payout currency.
   Require an explicit nullable strike. A finite positive strike selects
   fixed strike and null selects floating. Fixed calls and floating puts
   track the maximum. Fixed puts and floating calls track the minimum.
   Continuous monitoring includes the authored start and expiry. Discrete
   monitoring uses only its exact ordered observations, including expiry
   only when scheduled.
   [Payoff and observation rules](../models-monte-carlo/19-lookback-monitoring.md).

2. **Qualify elapsed extrema and the independent terminal fixing.** Bind the
   exact benchmark, adjustment basis, source version, session and cut.
   Require complete elapsed coverage or evidence of a fresh window. Use
   an eligible current observation to seed a fresh window or advance a
   strictly prior extremum. Check a through-current extremum against the
   eligible observation it already includes. A fixed payoff can become
   known at its final eligible observation before expiry. Floating payoff
   also requires its authorized terminal fixing. Include that fixing in
   the extremum only when it is monitored. Apply the positive part to
   the floating payoff difference.
   [Fixing authority](../core-market/scalar-series-and-fixings.md).

3. **Use the checked analytic law on a fully active continuous window.**
   Require qualified history, an eligible origin and constant GBM carry
   and volatility. An expressly elected terminal-matched flat proxy has
   its own path-extrema law. Use one qualified volatility quote on the
   model clock with an explicit ATM coordinate for floating strike.
   Preserve total variance when mapping clocks. Use the actual payment
   factor independently of carry. Literal zero carry and zero volatility
   use their exact shared branches. Future-start continuous or discrete
   monitoring uses the qualified simulation route.
   [Checked continuous lookback kernel](../models-analytic/lookback-extrema.md#accepted-decision-for-continuous-lookback-premiums),
   [Conze and Viswanathan](https://onlinelibrary.wiley.com/doi/abs/10.1111/j.1540-6261.1991.tb04648.x).

4. **Use exact GBM events and conditional extrema in simulation.** Bind all
   required dates and carry breaks to the exact stored knots. Use authored
   discrete observations only. Test the continuous opening node, then sample
   the relevant minimum or maximum on each monitored cell under qualified
   log-affine carry and integrated variance. Reserve one separately
   addressed open uniform per monitored cell. At literal zero variance
   return the exact endpoint extremum. Require explicit run controls and
   report actual work and sampling uncertainty. Qualified serial Sobol
   uses separately addressed extrema uniforms and descriptive replicate
   diagnostics.
   [Monte Carlo extrema](../models-monte-carlo/19-lookback-monitoring.md#accepted-decision-for-checked-monte-carlo-lookback-extrema),
   [Broadie, Glasserman, and Kou](https://business.columbia.edu/sites/default/files-efs/pubfiles/1851/connecting.pdf).

5. **Retain the known claim through actual payment.** Once authorized
   observations and the booked automatic payoff rule establish the amount,
   apply holder sign and quantity once and value the cash to its actual
   payment date. Eligible same-day unpaid cash uses factor one. Partial,
   failed or overdue payment requires a qualified residual. Resolve known
   cash before inactive market inputs. Export one `DerivativeSettlement`
   with extremum and applicable terminal-fixing provenance. Complete cash
   export is unavailable while the payoff is contingent. Confirmed zero
   payoff or discharge gives an empty residual. Use the same Rust state
   through existing Python and WASM instrument envelopes.
   [Cashflow export](../valuations-framework/cashflow-export.md).

6. **Keep Greeks and horizon risk on the qualified state.** Hold recognized
   history and the selected model fixed. Pure spot delta and gamma hold
   sampled volatility fixed. Volatility Greeks use the active quote basis,
   actual represented widths and qualified paired draws for simulation.
   A hypothetical eligible observation can advance an extremum only under
   an explicit scenario. Replacing one already included requires qualified
   pre-current history. Retire the discount-only curve bump labeled `rho`.
   Dated parallel and bucketed DV01 shock each active curve once across its
   roles. Theta across continuous monitoring requires qualified intervening
   extrema or path evidence.
   [Option risk](../valuations-framework/option-sensitivities.md),
   [dated theta](../valuations-framework/carry-theta-breakeven.md).

## Independent calculations

- On 1,000 units at strike `110`, authorized discrete observations `100`
  and `115` give fixed call cash USD `5,000` and PV USD `4,970` at
  factor `0.994`. An off-schedule `145` leaves the extremum unchanged.
  Including terminal `130` as an expressly monitored fixing gives
  USD `20,000` fixed call cash. With terminal `130` unmonitored, floating
  put pays zero and floating call pays USD `30,000` against minimum `100`.
- For a qualified bridge with endpoints `90` and `112`, integrated
  variance `0.0375` and open uniform `0.27`, the conditional maximum is
  `121.537193631798`. Integrating the conditional tail for strike `120`
  gives undiscounted fixed-call value `1.875495176293` per unit, even
  though both endpoints are below strike.
- Reflection-tail integration with spot `105`, strike `104`, historical
  maximum `122` and minimum `87`, carry `0.034`, volatility `0.24`, time
  `0.80`, payment factor `0.956` and quantity `400` gives fixed call and
  put PVs USD `10,164.30440780` and `7,826.50572893`. Floating call and
  put PVs are USD `9,316.02874497` and `8,674.78139177`.

## Observed behavior

### Choice, rationale and scope

The inspected lookback uses recorded and future extrema in continuous
formulas or continuous and discrete GBM simulation.

### Algorithm and conventions

For fixed call the terminal payoff is `Q max(max_observed-K,0)`; fixed put uses `Q max(K-min_observed,0)`. Floating call is `Q max(S_T-min_observed,0)` and floating put `Q max(max_observed-S_T,0)`. A fixed-strike terminal observation enters the extremum only when expiry is monitored; floating payoff always uses terminal spot. After expiry require an explicit terminal fixing, and retain the realized claim.

Analytical pricing combines recorded extremum with current spot (`max` or `min`) and calls the four dedicated Conze/Goldman-style lookback kernels at effective rate `-ln DF/t`, dividend yield, and volatility at strike or current spot for floating strike. Missing continuous-history extremum defaults to current spot; nonfinite/invalid output is checked before constructing money.

MC builds uniform continuous steps or a required-date discrete grid, uses a date-derived GBM drift schedule, seeds the payoff with the supplied historical extremum (otherwise ±infinity), and simulates fixed/floating lookback payoffs. Continuous monitoring enables the shared payoff's Brownian-bridge extremum treatment with `σ`; discrete paths inspect only observation nodes. Discount at expiry. Generic finite differences provide delta/gamma/vega/vanna/volga; rho uses discount-only parallel curve repricing; DV01 uses combined parallel curve repricing, with key-rate variants.

### Selection and controls

Continuous monitoring defaults to `LookbackBSContinuous`; discrete dates default to GBM MC. Explicit analytical pricing rejects discrete monitoring. The relevant extremum is max for fixed call/floating put and min for fixed put/floating call.

Elapsed discrete dates require the relevant observed extremum. Strike is required for fixed lookbacks; extrema/spot/fixing/quantity follow positive domains. Shared GBM config and instrument seed apply. Volatility remains a single expiry/strike (or spot) quote, not a dynamically sampled smile.

### Limitations, failure and fallback

Continuous lookbacks have no explicit history-start requirement, so omission of an extremum means an unseasoned current-spot assumption. Analytical constant-rate and MC datewise-drift paths differ on nonflat curves. Monotonicity tests do not establish continuous-extremum sampling convergence or an adopted post-expiry residual-claim contract.

### Alternatives and tradeoffs

The analytic formulas avoid time-grid bias in their domain; discrete MC represents an actually discrete payoff. Using discretely sampled extrema for continuous claims would change price and is not equivalent. Multi-factor/stochastic-volatility lookbacks are not registered here.

### Implementation evidence

- [finstack-quant/valuations/exotics/src/exotics/lookback_option/metrics.rs](../../../../finstack-quant/valuations/exotics/src/exotics/lookback_option/metrics.rs)
- [finstack-quant/valuations/exotics/src/exotics/lookback_option/pricer.rs](../../../../finstack-quant/valuations/exotics/src/exotics/lookback_option/pricer.rs)
- [finstack-quant/valuations/exotics/src/exotics/lookback_option/types.rs](../../../../finstack-quant/valuations/exotics/src/exotics/lookback_option/types.rs)

- [finstack-quant/valuations/exotics/tests/lookback_option/seasoning.rs](../../../../finstack-quant/valuations/exotics/tests/lookback_option/seasoning.rs): Recorded-extremum monotonicity and spot-clamping identities; invariant evidence rather than an independent full formula grid.

Canonical reference index. [Conze Viswanathan](../../../REFERENCES.md#conze-viswanathan-1991), [Goldman Sosin Gatto](../../../REFERENCES.md#goldman-sosin-gatto-1979), [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo).

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
