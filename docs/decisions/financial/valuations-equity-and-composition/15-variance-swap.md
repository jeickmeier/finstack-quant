# VALUATIONS-EQUITY-AND-COMPOSITION-15: Equity variance swap replication, seasoning and variance risk

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-15`.
- **Decision status:** Accepted 2026-11-14 — complete equity variance swap observation, forecasting, settlement and risk package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent interval forecast, forward-start, sensitivity and unit-conversion checks completed. Production pricing, market source, strip tails, risk and host qualification remain pending.

## Accepted decision for checked equity variance swap pricing and risk

1. **Require one contractual schedule, estimator and observation basis.**
   Bind positive Money variance notional, explicit pay or receive side,
   finite nonnegative strike variance, explicit annualization `A`
   in samples per year, and an explicit payment date at or after the
   final adjusted observation. Use the accepted exact observation
   schedule and publication-qualified raw or adjusted history.
   Require the price and adjustment basis to agree with the selected
   live spot, carry and option inputs. Count completed samples `m`
   against the full contractual count `n`. Preserve a qualified first
   close when no return is complete. Missing required eligible data
   fails. Unpublished observations remain pending. Report realized
   variance as unavailable before the elected estimator's minimum.
   Live forecasting supports close-to-close squared log returns.
   Parkinson, Garman–Klass, Rogers–Satchell and Yang–Zhang use
   their complete qualified OHLC histories for settlement, including
   a preceding close where the estimator requires it.
   [Accepted observation rules](../valuations-framework/observation-grids.md),
   [accepted realized estimators](../core-numerics/realized-variance.md),
   [accepted fixing authority](../core-market/scalar-series-and-fixings.md).

2. **Select one flat or strip-based variation source explicitly.**
   A finite nonnegative annual Black volatility on the model clock
   gives integrated variation `σ²Δt` directly. Exact zero
   volatility gives zero modeled stochastic variation. Deterministic
   carry can still contribute to a contractual squared return. The
   strip route uses a qualified current-origin unshifted Black
   strike surface, actual dated forwards and the native quote clock.
   Preserve total variance when mapping Black quotes to the model
   clock. Use checked OTM replication, exact forward insertion,
   the accepted midpoint mesh check and an explicit wing policy.
   Finite strips disclose unknown omitted wing amounts. Endpoint-flat
   Black wings supply a modeled continuation under their own numerical
   controls. Enforce an explicit aggregate replication work limit
   over the required expiries and evaluate each needed expiry once on
   the prepared valuation. Propagate source, support, resource and
   numerical failures through the elected route.
   [Accepted strip and wing policy](../models-analytic/variance-replication.md),
   [accepted volatility clocks](../conventions/0045-financial-clocks-and-date-boundaries.md),
   [Demeterfi et al.](https://emanuelderman.com/wp-content/uploads/1999/02/gs-volatility_swaps.pdf).

3. **Project each remaining close-to-close sample.** For pending
   interval `[a_i,b_i]`, let `u_i=max(V,a_i)` on the admitted
   observation basis. Let `Q_i` be integrated variation over
   `[u_i,b_i]`, and `c_i` the checked dated log carry on that
   remainder. Use the source-relative discount log ratio and signed
   proportional yield on the model clock. If the interval has begun,
   `g_i` is the log move from its official opening close to qualified
   live spot. Otherwise `g_i=0`. A live quote supplies a modeled
   partial return and never historical fixing authority. Require
   qualified event geometry and contiguous history for that partial
   interval. Use projected square `Q_i+(g_i+c_i−Q_i/2)^2`.
   The expected annualized variance is `A/n` times the sum of
   completed squared log returns and projected squares. The
   integrated variation comes from the selected flat source or
   differences of cumulative replication `W(d)=T_d V_rep(d)`,
   with `W(V)=0`. Before the start, subtract cumulative variation
   at the actual adjusted endpoints. Remove the `1e-6` shortcut,
   preserve genuine zero, and reject negative or unresolved
   differences and incompatible clock geometry. This second moment
   follows from the admitted deterministic-parameter GBM law. With
   strip variation it is an interval approximation for the discrete
   payoff whose market and jump behavior requires qualification.
   [Accepted strip boundary](../models-analytic/variance-replication.md),
   [Broadie and Jain](https://www.columbia.edu/~mnb2/broadie/Assets/variance_swaps_jumps_200903.pdf).

4. **Separate final observation, outstanding claim and payment.**
   Apply signed variance notional once to expected annualized
   variance less contractual strike variance. Discount the
   included payment once on its actual dated basis. With all
   observations complete, price and export the known signed cash
   without live spot or volatility. Include eligible unpaid same-day
   cash at factor one and require evidence of payment or discharge.
   An overdue amount needs a qualified valuation basis. Use one
   prepared state for expected variance, realized variance, progress
   and payoff. Report checked model time to the actual final
   observation separately from payment. Retire the elapsed-fraction
   helper with suppressed date and clock errors.
   [Accepted dated discounting](../core-conventions/016-discounting.md),
   [accepted cashflow export](../valuations-framework/cashflow-export.md).

5. **Give each sensitivity and notional conversion a precise basis.**
   `variance_vega` is the signed PV sensitivity per `+1.00`
   annualized decimal variance in the forecast of the remaining
   contractual squared returns. Its coefficient is signed notional
   times the payment factor times `(n−m)/n`, with no remaining
   variance exposure once all observations are fixed. Use
   `vega` for checked full reprices of the active scalar or elected
   native Black surface shock per `+0.01` annual volatility.
   Retire the aggregate square-root shortcut under that key.
   Use signed parallel and bucketed DV01 on the actual dated curve,
   shocking each actual source once across its active forward,
   replication and discount roles. Retain discount risk on fixed
   unpaid cash. Keep the checked quote-notional factor
   `2√K_var×.01` separate from risk repricing. Contract strike zero
   is allowed. The inverse notional conversion requires strictly
   positive strike volatility. Reconcile decimal variance and
   volatility-point units, checked prepared state and metric
   provenance through existing Rust, Python, WASM and schema routes.
   [Accepted volatility risk](../valuations-framework/option-sensitivities.md),
   [accepted dated DV01](../valuations-framework/finite-difference-rates.md).

## Observed behavior

### Choice and rationale

PV is discounted signed variance-notional times expected realized variance minus strike variance. Historical and forward components are weighted by scheduled realized samples; expected forward variance is option-strip replication, with an explicit flat-volatility override as a separate input path.

### Scope and input domain

Positive variance notional, nonnegative strike variance, explicit observation tenor/calendar/adjustment, positive annualization setting and settlement no earlier than final observation. Historical prices use exact dates; OHLC methods require open/high/low and close series. Adjusted/raw series policy is contract metadata, not an in-pricer corporate-action adjustment.

### Algorithm

1. Generate contractual observation dates and method-specific sample fraction w. Before start, use forward-start variance; after final observation use fully realized variance; during life use `w×realized+(1−w)×forward`. Discount to effective settlement; after settlement return zero.
2. Historical close returns use core realized-variance statistics; OHLC estimator selection delegates to core. Missing series with at least two past close observations errors. With fewer observations, live spot may stand in as a singleton; no realized return then exists.
3. A positive implied-volatility override returns σ². Otherwise require a Black strike surface, compute `F=S exp(-qT)/D`, price options across its strikes with the resolved surface smile and invoke Carr–Madan replication. Insufficient strip coverage errors; no ATM fallback is substituted.
4. Before start, subtract spot total variances: `(Vend Tend−Vstart Tstart)/(Tend−Tstart)`; negative result errors. Intervals/start times ≤ `1e-6` years use variance to end instead.
5. VarianceVega is signed `N D(1−w)` per unit decimal variance; Vega is `2 N D sqrt(Vforward)(1−w)×0.01`. DV01 is one-sided repricing per bp, including curve effects on replication. Notional conversion uses `vega_notional/(2 strike_vol×0.01)`, returning zero for nonpositive strike vol.

### Selection, controls and conventions

Close series defaults to spot ID. Final observation may differ from stated maturity after calendar adjustment; settlement defaults to final observation. The public elapsed-time helper uses calendar year fractions and suppresses date/year-fraction errors to maturity/zero, but pricing uses observation weighting. Cashflow export is a placeholder, not the settlement payoff.

### Limitations, failures and alternatives

Replication is finite-strike and surface-dependent; its vega metric is an aggregate-vol chain rule, not a full surface-bump derivative. Price-series policy does not verify adjusted history. Empty realized-history branches can return zero PV, and the elapsed-time helper's fallback is separate from strict valuation. Model-implied variance or explicit variance curves are alternatives requiring separate approval.

### Evidence

[variance replication](../../../REFERENCES.md#demeterfi-1999-volatility-swaps); [valuation.rs](../../../../finstack-quant/valuations/equity/tests/variance_swap/valuation.rs), [variance_calculation.rs](../../../../finstack-quant/valuations/equity/tests/variance_swap/variance_calculation.rs) and [observation.rs](../../../../finstack-quant/valuations/equity/tests/variance_swap/observation.rs) locate flat-vol, observation and seasoned identities. These do not independently validate finite-strike smile tails or adjusted historical data.

## Independent numerical checks

- With three scheduled close intervals, annualization 4, completed
  prices 98 and 100, and current spot 105 inside the next interval,
  use remaining endpoint times .25 and .50, discounts .98 and .955,
  continuous yield .02 and cumulative variation .012 and .020.
  The projected squares are .0153631731433 and .00828362706779.
  The expected annual variance is .0320732661254. Receive-variance
  notional 1,000, strike variance .03 and payment factor .94 give
  PV +1.948870157874. On the same inputs the observed aggregate
  blend gives variance .0272108658439 and PV −2.621786106693.
- At forward-start endpoints .25 and .50 with annual volatilities
  .30 and .25 on one clock, cumulative variation increases from
  .02250 to .03125. The interval annualized continuous variance is
  .035 before the discrete squared-mean term.
- A quote notional of 100,000 per positive volatility point at
  strike volatility .20 converts to 25,000,000 per unit decimal
  variance, or 2,500 per squared volatility point. A fixed
  payment of 12,500 at factor .96 and native curve duration 1.4
  gives centered 1 bp DV01 of −1.680000005488.

Production pricing, history and smile qualification, model and
tail error, risk and host validation remain pending.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/equity/src/variance_swap/types.rs)
- [pricer.rs](../../../../finstack-quant/valuations/equity/src/variance_swap/pricer.rs)
- [metrics.rs](../../../../finstack-quant/valuations/equity/src/variance_swap/metrics.rs)
