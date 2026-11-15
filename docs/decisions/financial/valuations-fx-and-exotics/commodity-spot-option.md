# VALUATIONS-FX-AND-EXOTICS-COMMODITY-SPOT-OPTION: Commodity spot-option Black pricing, exercise trees and Schwartz–Smith simulation

- Record: `VALUATIONS-FX-AND-EXOTICS-COMMODITY-SPOT-OPTION`
- Decision status: **Accepted 2026-11-14** — complete commodity spot-option pricing, exercise, settlement and risk package.
- Implementation status: **Observed** in the owning paths below. Reconciliation to the accepted contract is pending.
- Validation status: Independent Black, lattice, terminal-law and residual-claim calculations checked. Runtime, market sources, numerical and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for commodity spot option pricing and exercise

1. **Book explicit option and exercise terms.** Require holder long or short,
   call or put, finite positive quantity and multiplier, exact commodity and
   price unit, applicable grade and location, currency, strike and cash or
   physical settlement. Bind European, American or Bermudan rights to their
   actual dates and phases. Specify fixing authority, automatic exercise or
   notice terms, and the resulting payment, delivery and invoice dates.
   Live lognormal pricing requires positive prices and strike. An authorized
   realized fixing can be signed where the contract permits it.
   Keep `black76`, `tree` and `monte_carlo_schwartz_smith` as distinct
   selected live routes under their admitted exercise profiles.

2. **Use checked dated Black-76 for European pricing.** Select a qualified
   dated outright or matched curve forward on the fixing basis, explicit
   active volatility at strike and a positive fixing horizon. Preserve total
   variance when mapping the volatility quote clock. Discount once to the
   actual payment date and apply position, quantity and multiplier once.
   Exact zero volatility gives discounted forward intrinsic. A live physical
   profile can use this law only with qualified equivalent exercise economics.
   Differently dated asset and invoice legs may require a separate exercise
   payoff. [Checked forward option law](../models-analytic/forward-normal-shifted.md),
   [Black on commodity pricing](https://doi.org/10.1016%2F0304-405x%2876%2990024-6).

3. **Elect an explicit checked tree for early exercise.** Select CRR or
   Leisen–Reimer, qualified observed spot, homogeneous carry and discount,
   literal interval count and checked work budget. An explicitly elected flat
   proxy can match the expiry forward and discount factor, with intermediate
   carry and discount supplied by the chosen model. Propagate carry and clock
   failures. Use authorized exercise nodes and phases, including the origin
   when eligible. CRR requires strict transition support. Leisen–Reimer
   requires an odd count. Reject off-grid required rights. Use the single
   deterministic path at zero volatility. Ordinary intrinsic at a node is
   admitted only when the exercise settlement terms support it.
   [Lattice construction](../models-numerical/equity-binomial-construction.md),
   [dated exercise](../models-numerical/dividends-and-lattice-exercise.md).

4. **Use one explicit European Schwartz–Smith simulation.** Require qualified
   positive spot, an authored initial factor split, explicit pricing-measure
   parameters with provenance, and explicit Philox seed, estimator budget
   and pairing policy. Solve effective long drift directly in log space to
   match the qualified fixing-basis forward. Verify the pin against a declared
   price tolerance. This fits one terminal expectation. Curve calibration
   and risk-premium identification require separate evidence. Use one checked
   exact joint Gaussian terminal transition for this payoff. Discount once
   to actual payment and report actual draws, counts, seed, grid and priced
   uncertainty. Use the same qualified payoff-equivalence restriction for
   physical terms as for the European terminal Black law.
   [Schwartz–Smith transition and pin](../models-monte-carlo/12-schwartz-smith.md),
   [Monte Carlo execution](../models-monte-carlo/01-engine-and-estimation.md),
   [Schwartz and Smith](https://pubsonline.informs.org/doi/abs/10.1287/mnsc.46.7.893.12034).

5. **Resolve exercise and retain the actual obligations.** Use authorized
   fixing and recorded exercise, assignment or lapse, resolving those states
   before inactive live inputs. Apply automatic exercise under the elected
   contractual rule. A cash exercise locks the signed cash through discharge.
   A physical exercise creates the signed commodity delivery and separate
   strike invoice, with each on its actual date. A long call receives the
   commodity and a long put delivers it. Reverse directions for shorts.
   Value the established exchange on its signed linear terms. Eligible same-day
   unpaid cash uses factor one. Partial, failed or overdue obligations require
   a qualified residual. Export known cash as `DerivativeSettlement` and
   attribute the physical invoice and typed delivery separately. Complete
   cash-only export is unavailable while the payoff is contingent or the
   commodity delivery is outstanding.
   [Commodity delivery](commodity-delivery-listed.md#accepted-decision-for-commodity-delivery-and-listed-contracts),
   [cashflow export](../valuations-framework/cashflow-export.md).

6. **Define risk on the active model or claim.** European Black delta and
   gamma use the selected forward unit and its square. Asset vega, vanna
   and volga use the active native annual volatility source and one
   volatility point (`0.01` annual decimal volatility) as the volatility
   reporting increment. Live tree option Greeks are unavailable
   pending a qualified spot stencil and grid stability check. Schwartz–Smith
   forward delta and gamma repin at each represented shock with logically
   paired draws and paired uncertainty. The route has no Black smile Greek.
   Fixed cash retains discount and timing risk. Open physical delivery
   retains linear commodity exposure until fulfilled. Reprice each actual
   curve once across its active roles. Require qualified observations and
   eligible cash and delivery when horizon theta crosses those events.
   Use one canonical Rust state through existing Python and WASM envelopes.
   [Option sensitivities](../valuations-framework/option-sensitivities.md),
   [paired sensitivities](../models-monte-carlo/25-finite-difference-greeks.md),
   [dated theta](../valuations-framework/carry-theta-breakeven.md).

## Independent calculations

- With `F=74`, `K=70`, annual volatility `0.25`, fixing time `0.5`,
  position scale `1,000` and payment factor `0.985`, European Black calls
  and puts are worth USD `7,209.10365878` and `3,269.10365878`. Substituting
  the expiry factor `0.990` gives call value USD `7,245.69809360`.
- A two-interval CRR put with `S=100`, `K=105`, volatility `0.25`,
  discount rate `0.10`, yield `0.03` and time `1` is worth `8.43362846`
  per unit with European exercise and `10.06898167` with the exact midpoint
  exercise right. An odd Leisen–Reimer grid cannot represent this midpoint.
- With `S0=88`, `X0=0.17`, mean reversion `0.6`, short risk shift `0.025`,
  short and long volatility `0.38` and `0.16`, correlation `−0.40`,
  `T=0.8`, forward `92`, strike `90`, payment factor `0.967` and position
  scale `500`, the solved effective long drift is `0.116531641413` and
  terminal log variance `0.0638345963501`. The closed form and direct
  Gaussian integration give call value USD `4,923.18823307` and put
  value USD `3,956.18823307`.
- A long call on 1,000 units at strike `70` with authorized fixing `80`
  locks cash USD `10,000`, worth USD `9,850` at payment factor `0.985`.
  The separate physical election, subsequently marked at delivery forward
  `68` with delivery and invoice factors `0.990` and `0.985`, has residual
  PV USD `−1,630` and forward delta USD `990` per point. Fulfilled delivery
  leaves the invoice with PV USD `−69,860` at remaining factor `0.998`.

## Observed behavior

### Choice, rationale and scope

The inspected implementation uses Black-76 for European options, a
Leisen–Reimer tree for early exercise and a separate two-factor simulation.

### Algorithm and conventions

Resolve a positive quoted forward or curve price at expiry. European price is `Q M DF [FΦ(d1)-KΦ(d2)]` (signed form for put), with expiry/strike volatility. For early exercise resolve spot, recover `r=-ln(DF)/t`, and infer `q=r-ln(F/S)/t`; a near-zero/invalid carry calculation returns `q=r`. Use a Leisen–Reimer binomial tree, default 201 steps, comparing continuation with intrinsic on all American dates or supplied future Bermudan dates.

Recorded exercise resolves the claim before model pricing. Cash exercise locks recorded intrinsic until settlement; physical exercise retains signed `(delivery_forward-K)` exposure until settlement. Post-expiry without a recorded expiry observation errors; on expiry a current spot/forward observation supplies intrinsic. After recorded settlement the claim is zero.

Schwartz–Smith MC requires European exercise and spot. Build the two-factor process from spot, then shift long drift by `ln(F_market/F_model(t))/t` to pin its expected terminal price to the market forward. Use exact Schwartz–Smith steps, uniform configured grid, Philox seed (explicit or 42), parallel MC and European payoff, scaled by quantity×multiplier. Family defaults are 100,000 paths/252 steps; instrument positive path-count override is honored.

Default European delta/vega are analytic forward sensitivities; gamma, vanna and volga reprice quoted forward or the whole price curve and active volatility. Vega is per 0.01 absolute vol, vanna per forward-unit/vol-point, volga per vol-point squared. The selected Schwartz–Smith route only supports forward delta/gamma; Black-surface vega/vanna/volga are rejected because its volatilities are model parameters.

### Selection and controls

The family registers the default `Black76` route, whose instrument method branches by exercise style, plus `MonteCarloSchwartzSmith`. The latter is registered with kappa 1, short-factor vol 0.30, long-factor vol 0.15, correlation 0.30, long drift 0 and short risk premium 0; these are defaults, not a calibration result.

Bermudan dates must be increasing and include a usable future exercise date; non-European exercise requires spot ID. MC is European-only. Diagnostics report scaled standard error, paths, seed and grid; antithetic/Sobol/bridge flags are false for this local engine.

### Limitations, failure and fallback

Registered Schwartz–Smith coefficients are uncalibrated constants. Forward pinning changes drift at each expiry but does not fit the entire forward curve jointly. Early-exercise analytic Greeks are rejected. Near-expiry carry fallback and physical-settlement exposure are separate from the European forward-sensitivity contract.

### Alternatives and tradeoffs

A calibrated term-structure model and dynamic convenience yield can improve cross-maturity consistency at extra parameter/market-data cost. Supported Black and two-factor routes are materially different dynamics; path count alone does not make them equivalent.

### Implementation evidence

- [finstack-quant/valuations/exotics/src/commodity/commodity_spot_option/metrics/mod.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_spot_option/metrics/mod.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_spot_option/metrics/selected_model.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_spot_option/metrics/selected_model.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_spot_option/pricer.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_spot_option/pricer.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_spot_option/types.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_spot_option/types.rs)

- [finstack-quant/valuations/exotics/tests/commodity_spot_option/pricing.rs](../../../../finstack-quant/valuations/exotics/tests/commodity_spot_option/pricing.rs): Black benchmark formula, put-call parity, American/Bermudan ordering and expiry-state assertions.
- [finstack-quant/valuations/exotics/tests/commodity_spot_option/mc_pricing.rs](../../../../finstack-quant/valuations/exotics/tests/commodity_spot_option/mc_pricing.rs): GBM-limit comparison, path-count consistency, seeded replay and MC diagnostic scaling; internal cross-model evidence.

Canonical reference index. [Black 1976](../../../REFERENCES.md#black-1976), [Schwartz Smith](../../../REFERENCES.md#schwartz-smith-2000).

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
