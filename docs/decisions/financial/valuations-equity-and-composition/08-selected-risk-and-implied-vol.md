# VALUATIONS-EQUITY-AND-COMPOSITION-08: Selected-model Greeks, higher derivatives, Dividend01 and implied volatility

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-08`.
- **Decision status:** Accepted 2026-11-14 — complete selected-model Greeks, dividend sensitivity and European implied-volatility package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Source and primary references inspected. Independent signed-yield, higher-derivative and gross-premium inversion checks completed. Market, runtime and host qualification pending.

## Accepted decision for checked selected-model equity risk and implied volatility

1. **Take risk on the selected model and resolved claim.** Bind the exact
   registry/model, active factors, information and lifecycle state, signed
   raw PV, currency and amount basis. Apply native analytic derivatives
   only where the provider differentiates that same prepared valuation on
   the requested basis. Otherwise use qualified comparable reprices on
   the selected model. A missing admitted risk axis is unavailable. A
   proved inactive axis can return structural zero on the defined shock.
   Stochastic reprices require verified common logical draws and
   uncertainty on the complete stencil. [Accepted lifecycle](06-option-lifecycle.md),
   [direct pricing and Greeks](07-black-tree-and-direct-greeks.md),
   [shared option sensitivities](../valuations-framework/option-sensitivities.md).

2. **Use explicit spot and volatility stencils and reporting units.** Use
   the exact active spot factor and checked represented widths for delta
   and gamma. Use the shared full stencils for Black vega and vanna per
   `+0.01` native volatility and volga per `.01²`. A qualified one-sided
   vega may use the full admissible up move when the full down move lies
   outside the volatility domain. Vanna and volga require both full
   sides. Keep the model-sigma, native surface-source and recalibration
   experiments distinct. For exact sparse node vega retain identities,
   actual coverage, raw bucket sum and the residual against the matched
   simultaneous-node shock. No implicit shrinkage, floors or alternative
   model on failure. [Accepted spot and volatility stencils](../valuations-framework/option-sensitivities.md),
   [typed results and provenance](../valuations-framework/metric-orchestration.md).

3. **Define Speed, Charm and Color on named experiments.** On the selected
   valuation, request five spot nodes `S`, `S±h` and `S±2h` from one
   explicit positive calculation step. Require positive strictly ordered
   represented spots and a checked complete stencil. Speed is the third
   derivative at the center of the degree-four interpolant through those
   actual nodes, in holder currency per spot unit cubed. Use checked
   scaled weights and remove adaptive widening. Compare matched delta
   for Charm and matched gamma for Color at two explicit still-live
   model cuts. Use the same represented spot stencil and an admitted
   deterministic dated market policy. Divide closing minus opening by
   positive elapsed model years on the qualified clock. An unsupported
   exercise or expiry crossing leaves this comparison unavailable.
   [Fornberg's finite-difference weights](https://people.sc.fsu.edu/~jburkardt/m_src/differ_arbitrary/fornberg.pdf),
   [shared spot stencils](../valuations-framework/option-sensitivities.md).

4. **Keep local rho and horizon theta on their own bases.** Rho uses a
   declared local rate parameter and reports per positive bp. A curve
   DV01 request reprices the actual dated curve sources under its own
   direction. Registered `theta` reports signed period cash plus closing
   PV less opening PV on explicit cuts and qualified exercise and payment
   state. Remove forced one-day, reporting-day rescaling and backward
   expiry fallback. Local model theta remains a separate diagnostic.
   [Accepted curve risk](../valuations-framework/finite-difference-rates.md),
   [accepted holder horizon](../valuations-framework/carry-theta-breakeven.md).

5. **Use `dividend01` on an actual continuous yield.** On the chosen
   model and signed annual yield `q`, use an explicit two-sided step,
   verify represented `q_up>q>q_down`, and report
   `.0001 × (PV_up−PV_down)/(q_up−q_down)` per positive bp. Hold cash
   events fixed when an elected residual yield is the active factor.
   Remove zero flooring, synthetic equivalent yield and the native
   price bypass. The discrete-only basis may support a separately
   identified event-amount scenario, preserving actual event identities,
   entitled dates and history. Reconcile the shared helper with its
   product callers on their qualified bases. [Identified dividend events](../core-market/dividend-events.md).

6. **Elect a live European Black–Scholes inverse explicitly.** Require an
   observed gross long premium on the exact currency, positive quoted
   quantity, information cut, fixing, settlement and payoff basis. Freeze
   the record 07 European price inputs other than volatility. Admit its
   qualified escrow and deterministic cash-payment lag. The quote model
   is explicitly elected independently of any selected base PV model.
   Early-exercise prices can be flat over admissible volatilities and
   are outside this inverse. Use the existing standalone IV operation.
   A registered `implied_vol` request requires the same explicit quote
   basis, target and solver controls. Return annual decimal Black
   volatility on the model fixing clock. [Accepted live European law](07-black-tree-and-direct-greeks.md),
   [accepted scalar inversion](../models-analytic/implied-volatility-solvers.md).

7. **Check gross premium bounds and the actual inverse result.** With
   `W=quoted_units × D(V,P)` on the admitted payoff and payment basis,
   use original cash-coordinate lower bound `W × intrinsic(F,K)` and
   exclusive upper bound `W×F` for a call or `W×K` for a put. Check the
   upper bound first. Only exact lower-bound equality on the eligible
   live contract gives zero volatility. Require `max_vol`, `vol_tol`,
   `max_iterations`, `price_abs_tol` in the premium currency and
   dimensionless `price_rel_tol` under the shared `IvControls` contract.
   Solve on `[0,max_vol]` through fallible Brent and the stable same-law
   log-OTM objective. At the actual returned annual volatility, require
   both gross premium and independently evaluated same-law OTM time value
   to meet their absolute-plus-relative cash tolerances, each scaled by
   its own target. Keep bounds, bounded search,
   precision and residual failures distinct. Reconcile native and
   registered paths and typed Python/WASM, schema, UI and portfolio
   consumers through one Rust quote operation. [Brent](https://maths-people.anu.edu.au/~brent/pd/rpb005a.pdf),
   [accepted original-cash premium gates](../valuations-rates/06-cap-floor-risk.md#accepted-decision-for-checked-cap-and-floor-risk-and-implied-volatility).

## Observed behavior

### Choice and rationale

Selected-model risk reprices through the metric context for principal Greeks; native providers also expose analytical/tree Greeks. Implied volatility is a quote-equivalent inversion of the native Black/tree calculation, regardless of an alternative model used to produce a base PV.

### Scope and input domain

Live vanilla option risk under a selected registry model, optional active-volatility overrides and configurable bump controls. IV requires positive quantity and a finite nonnegative total premium; exercised/expired options are rejected.

### Algorithm

1. Selected delta/gamma/vega/vanna/volga delegate to generic finite-difference calculators preserving dispatch; selected rho uses a discount-only parallel rate bump. After an effective exercise observation, use lifecycle Greeks, with unused higher Greeks zero.
2. Selected theta uses one-day generic market roll, scaled by `365/days_per_year`. Exactly one calendar day before expiry it instead differences current PV against the previous date with a backwards-rolled market, avoiding a fabricated forward expiry quote.
3. Charm/color difference spot delta/gamma between today and tomorrow divided by the instrument day-count fraction, using unchanged market curves and selected repricing. They return zero if any exercise observation exists or fewer than two day fractions remain. Speed differences gamma at spot offsets ±h using prices at ±2h, ±h and base. Optional adaptive spot bump multiplies the configured bump by `min(1+2|S−K|/K,5)`.
4. Continuous Dividend01 bumps q by ±1 bp, floors down q at zero, normalizes actual width, and calls `instrument.value`. Discrete Dividend01 derives equivalent yield `q=−log(S*/S)/T`, rescales all future cash dividends to each bumped total PV and reprices through context.
5. IV divides premium by quantity, prices a `1e-8` volatility lower bound, rejects targets within `1e-10×max(|target|,1)` of that price, and doubles the upper bracket from 1 to at most 8. Brent uses `1e-12` root tolerance; trial price errors become NaN for the solver, and final premium residual must meet the same price tolerance.

### Selection, controls and conventions

ImpliedVol metric requires a quoted premium; no theoretical PV fallback is supplied. Native vanna uses four spot/vol reprices and volga a second vol difference in percentage-point units; active override bumps are capped at half the override volatility. All shared bump/dispatch owners are outside this slice.

### Limitations, failures and alternatives

Continuous Dividend01 bypasses selected dispatch while the discrete variant preserves it: alternative-model consistency is unresolved. Charm/color use static curves whereas selected theta rolls curves, so they are not derivatives of a single identical time scenario. IV may solve a different economic model from selected base pricing by design. Direct Heston parameter Greeks, alternative IV model inversion and central time derivatives are meaningful alternatives requiring agreement.

### Evidence

Source inline cases in [selected_model.rs](../../../../finstack-quant/valuations/equity/src/equity_option/metrics/selected_model.rs) and [dividend01.rs](../../../../finstack-quant/valuations/equity/src/dividend01.rs); [test_implied_vol.rs](../../../../finstack-quant/valuations/tests/instruments/equity_option/test_implied_vol.rs) and [test_greeks.rs](../../../../finstack-quant/valuations/tests/instruments/equity_option/test_greeks.rs) locate roundtrip and finite-difference comparisons. [Brent](../../../REFERENCES.md#brent-1973) provides the solver reference; roundtripping the same pricer is not independent price validation.

## Independent arithmetic and qualification

- With 100 long call units, spot 100, strike 105, half-year time,
  rate `.035`, volatility `.20` and dividend yield `−.0002`, a full
  1 bp from either side gives dividend PV sensitivity `−.2202453123`
  per positive bp.
- The five unequal offsets `−2.25,−1,0,1.25,2.6` recover Speed `18`
  at zero for `2z⁴/17+3z³+7z²+2z+11`.
- With `F=112`, `K=100`, `W=7`, `T=.25` and volatility `.40`, gross
  call premium is `110.3042842076` and OTM time value `26.3042842076`.
  The original cash interval is `[84,784)`. A `.20` search ceiling
  reaches `88.7537629164` and cannot solve that interior target.

Qualify actual provider/metric routing, sensitivity steps and uncertainty,
dated dividend and horizon effects, market-premium normalization and
inverse error behavior across native and existing host paths. Repository
runtime and host suites remain unrun.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [mod.rs](../../../../finstack-quant/valuations/equity/src/equity_option/metrics/mod.rs)
- [selected_model.rs](../../../../finstack-quant/valuations/equity/src/equity_option/metrics/selected_model.rs)
- [spot_greek_decay.rs](../../../../finstack-quant/valuations/equity/src/equity_option/metrics/spot_greek_decay.rs)
- [dividend_risk.rs](../../../../finstack-quant/valuations/equity/src/equity_option/metrics/dividend_risk.rs)
- [types.rs](../../../../finstack-quant/valuations/equity/src/equity_option/types.rs)
- [dividend01.rs](../../../../finstack-quant/valuations/equity/src/dividend01.rs)
