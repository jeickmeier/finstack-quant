# VALUATIONS-FX-AND-EXOTICS-FX-VARIANCE: FX variance swap observation estimator and smile-based forecast

- Record: `VALUATIONS-FX-AND-EXOTICS-FX-VARIANCE`
- Decision status: **Accepted 2026-11-14** — complete FX variance swap observation, forecasting, settlement and risk package.
- Implementation status: **Observed** in the owning paths below. Reconciliation to the accepted contract is pending.
- Validation status: Independent strip identity, interval forecast, fixing, settlement and sensitivity calculations checked. Production data, smile, risk and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked FX variance swap pricing and risk

1. **Require explicit contractual terms and one sampling grid.** Bind the
   ordered FX pair in quote per base, explicit receive or pay variance side,
   positive quote-currency Money notional per `1.00` annual decimal variance,
   nonnegative finite strike variance and the selected estimator. Compile one
   joint base/quote both-open calendar schedule from the authored cadence,
   adjustment, applicable roll and stub. Reject collisions and unsupported
   calendar or sample geometry. Cap the schedule at 100,000 dates inclusive.
   Require finite positive annualization `A` in target samples per year, and
   an explicit payment date at or after the final adjusted observation.
   Preserve annualization, model time and volatility quote time separately.
   [Observation grids](../valuations-framework/observation-grids.md),
   [volatility clocks](../conventions/0045-financial-clocks-and-date-boundaries.md).

2. **Resolve authority and count complete estimator samples.** Bind the
   exact benchmark, source and authorized version, ordered quote and price
   basis, event phase and publication cut. Use a separately selected live
   quote. Preserve exact authorized history and the official first close,
   including when it stands alone. An eligible missing or unresolved required
   observation errors. An explicitly unpublished observation stays pending,
   even with a stored row. Preserve gaps and their scheduled identities.
   Close-to-close uses `N−1` adjacent uncentered squared log returns. Use all
   complete target bars for the selected OHLC estimator on its common window
   and basis. Yang–Zhang requires at least two target bars and the qualified
   preceding close. Report completed and contractual counts and their
   fraction separately from estimator availability. Realized variance uses
   the completed estimator sample basis and is unavailable below its minimum.
   Live expected variance supports close-to-close. The OHLC payoff uses the
   complete qualified contractual history.
   [Fixing authority](../core-market/scalar-series-and-fixings.md),
   [realized variance](../core-numerics/realized-variance.md).

3. **Bind cumulative variation to the dated FX smile.** Use the checked
   current-origin unshifted Black absolute-strike strip at each required
   fixing endpoint, with its actual fixing-basis forward and native quote
   clock. Bridge later-value FX with the applicable base and quote factors.
   Preserve total variance when mapping volatility clocks. Use the accepted
   exact forward split, midpoint mesh check, explicit wing policy and an
   aggregate replication work limit. Evaluate each required expiry once per
   prepared valuation. A finite strip leaves omitted wing amounts unknown.
   Endpoint-flat Black wings add a modeled continuation under the shared
   controls. Propagate source, support, resource and replication failures.
   [Dated FX carry](fx-delivery-cip.md),
   [variance replication](../models-analytic/variance-replication.md),
   [Demeterfi et al.](https://emanuelderman.com/wp-content/uploads/1999/02/gs-volatility_swaps.pdf).

4. **Project every pending close-to-close sample.** For `[a,b]`, let `u`
   be the unelapsed start on the admitted event clock. Use integrated
   variation `Q=W(b)−W(u)`, where `W(d)=T_d V_rep(d)` and `W` is zero at
   the valuation origin. Preserve genuine zero and reject negative or
   numerically unresolved differences. Compute `c` as the checked log carry
   between same-basis benchmark forward marks using their actual reference
   value dates. Use `g` as the log move from the official opening fixing to
   the qualified comparable live level for a begun interval, or zero for a
   future interval. Require contiguous history for a partial return and
   qualified event timing. A pending endpoint without an admitted positive
   forecast horizon makes the forecast unavailable. Project each square as
   `Q+(g+c−Q/2)^2`. Whole-contract expected variance is `A/n` times the sum
   of completed and projected squares for `n` contractual samples. The
   remaining-forward diagnostic uses `A/(n−m)` times the projected sum
   while `m<n` and is unavailable at completion. The moment is exact under
   the admitted deterministic-parameter GBM law. Smile-strip variation makes
   it an approximation for the discrete payoff.
   [Equity variance interval precedent](../valuations-equity-and-composition/15-variance-swap.md),
   [Broadie and Jain](https://www.columbia.edu/~mnb2/broadie/Assets/variance_swaps_jumps_200903.pdf).

5. **Resolve final observation and the outstanding payment separately.**
   Apply the signed notional once to expected or final annual variance less
   strike variance. Discount the included quote-currency cash once on its
   actual dated basis. After fixing, known unpaid cash uses only its relevant
   quote discount factor. Eligible same-day unpaid cash uses factor one.
   Established discharge gives zero residual. Partial or overdue cash needs
   qualified residual terms and a valuation basis. Track expiry and checked
   model time to the final adjusted observation separately from payment. Export
   known cash as one signed `DerivativeSettlement` with observation provenance.
   Complete contractual cash export is unavailable while payoff is contingent.
   [Dated discounting](../core-conventions/016-discounting.md),
   [cashflow export](../valuations-framework/cashflow-export.md).

6. **Report risk and diagnostics on the active basis.** `variance_vega`
   is signed notional times the payment factor times `(n−m)/n` per
   `+1.00` annual decimal variance applied to the remaining squared-return
   forecast. `vega` uses checked matched full reprices of the active native
   strike smile per `+0.01` annual volatility. Use signed parallel and
   bucketed DV01 and shock each actual curve once across its active carry,
   replication and payment roles. Hold qualified fixes and terms fixed.
   Variance and smile risk are zero after final fixing. Fixed unpaid cash
   retains its applicable quote discount risk. Report strike volatility as
   `sqrt(strike_variance)` in decimal units. Use the same prepared basis for
   admissible metrics, pricing and export. Reconcile the existing Rust and
   schema routes, typed WASM and Python JSON envelope.
   [Volatility risk](../valuations-framework/option-sensitivities.md),
   [dated DV01](../valuations-framework/finite-difference-rates.md).

## Independent calculations

- With `A=12`, three returns, one official log return `0.03`, the
  partially elapsed move `0.04`, remaining carries `0.01` and `0.005`,
  and incremental variations of `0.01` each, the pending squares are
  `0.012025` and `0.01`. Independent flat-Black strip integration
  reproduced cumulative `W` of `0.01` and `0.02`. Realized-sample variance
  is `0.0108`, remaining-forward variance `0.13215` and whole-contract
  expected variance `0.0917`. Receiving variance on USD 1,000 at strike
  `0.03` with payment factor `0.94` gives PV USD `57.998` and variance vega
  USD `626.6666667` per `+1.00` annual decimal variance.
- Official EUR/USD closes `1.10`, `1.11` and `1.12` with `A=126` give
  progress `1/2` and realized-sample variance `0.010319339890133546` before
  the final close is eligible. At completion, annual variance is
  `0.010227203544425829`. Receiving on USD 1 million at strike `0.01`
  fixes cash of USD `227.2035444258293` and PV of USD `224.9315089815710`
  at payment factor `0.99`. An eligible unpaid due-day claim uses factor one.

## Observed behavior

### Choice, rationale and scope

The inspected FX variance swap combines exact dated realized observations
with forecast squared log returns derived from smile-replicated cumulative
variation.

### Algorithm and conventions

Build adjusted observation dates using the joint base/quote calendar, frequency, BDC and end-of-month flag. Fetch close values on exact dates at or before valuation. Seasoned swaps require historical series; a spot-only seed is allowed only when no earlier observation history is required. Realized close-to-close numerator is the sum of squared log returns, without mean subtraction; annualization depends on frequency/trading-days input. OHLC settlement delegates the chosen core estimator using exact open/high/low/close series.

For each future return interval, compute cumulative variation `W(u)=T_u×V_rep(u)` using strike-axis Black-lognormal surface wings and Carr–Madan option replication. Interval variation is `ΔW=W(end)-W(max(start,as_of))`; materially negative values error, roundoff negatives clamp to zero with `1e-12` scale tolerance. Let carry `c=ln(D_f(start,end)/D_d(start,end))`, and for a partly elapsed interval let `a=ln(spot/latest_fixing)`. Forecast squared return is `ΔW+(a+c-ΔW/2)²`. Sum known and forecast squared returns, divide by the full scheduled return count, and annualize.

Cash payoff is signed variance notional times expected variance minus strike variance, discounted to effective settlement. Remaining-forward variance averages only forecast intervals. Variance vega is `side×N×DF×unrealized_sample_fraction`, a sensitivity to a variance level; ordinary vega is finite-difference smile-surface repricing. Strike-vol metric is `sqrt(strike_variance)` and time-to-maturity stops at final observation, not settlement.

### Selection and controls

Default discounting calls the local expected-variance routine. Close-to-close can be forecast; OHLC estimators are supported only once fully observed and error before settlement while future observations remain. Failed smile replication is an error; ATM-volatility fallback is explicitly disabled.

Surface must be Black lognormal on absolute strike and have valid put/call wings surrounding the forward. Spot lookup prioritizes the FX matrix; only if no matrix is present does it use `spot_id` (or concatenated pair ID) as a scalar. Historical close series defaults to that ID. Valuation cannot precede either discount curve base date. Settlement defaults to the final adjusted observation and cannot precede it. Spot is positive; dates/calendars/settlement and series are validated. Model year fraction, observation annualization and settlement date are separate controls.

### Limitations, failure and fallback

Static replication supplies expected quadratic variation, while the local discrete forecast additionally assumes a lognormal interval mean/second moment using that variation. Partial-return terms and annualization belong to that approximation. Full OHLC forecast models are deliberately unsupported. Smile wings, calendar consistency and boundary carry terms limit the interpretation of forecast variance.

### Alternatives and tradeoffs

ATM variance is simpler but discards smile exposure and is rejected here. A model of discrete sampling under stochastic volatility or estimator-specific OHLC forecasting would change the expectation and is not implemented. Exact settlement estimators remain supported without such a forecast.

### Implementation evidence

- [finstack-quant/valuations/fx/src/fx_variance_swap/metrics.rs](../../../../finstack-quant/valuations/fx/src/fx_variance_swap/metrics.rs)
- [finstack-quant/valuations/fx/src/fx_variance_swap/pricer.rs](../../../../finstack-quant/valuations/fx/src/fx_variance_swap/pricer.rs)
- [finstack-quant/valuations/fx/src/fx_variance_swap/types.rs](../../../../finstack-quant/valuations/fx/src/fx_variance_swap/types.rs)

The original audit inspected source and inline assertions. The independent calculations above cover the stated examples. Production qualification is pending.

Canonical reference index. [Demeterfi](../../../REFERENCES.md#demeterfi-1999-volatility-swaps), [Carr Madan](../../../REFERENCES.md#carr-madan-1999-fft).

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
