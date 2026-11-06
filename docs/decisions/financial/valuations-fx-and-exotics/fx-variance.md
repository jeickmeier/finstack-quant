# VALUATIONS-FX-AND-EXOTICS-FX-VARIANCE: FX variance swap observation estimator and smile-based forecast

- Record: `VALUATIONS-FX-AND-EXOTICS-FX-VARIANCE`
- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, limited to the source paths below and their named delegated kernels.
- Validation status: **Evidence inspected/located, not executed**. Code-level formulas and test assertions are evidence of implementation; no external numerical certification or current test pass is claimed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, rationale and scope

FX variance swaps combine exact dated realized observations with forecast squared log returns on each remaining observation interval. The forecast is derived from smile-replicated cumulative variation, not simply ATM volatility squared.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

## Algorithm and conventions

Build adjusted observation dates using the joint base/quote calendar, frequency, BDC and end-of-month flag. Fetch close values on exact dates at or before valuation. Seasoned swaps require historical series; a spot-only seed is allowed only when no earlier observation history is required. Realized close-to-close numerator is the sum of squared log returns, without mean subtraction; annualization depends on frequency/trading-days input. OHLC settlement delegates the chosen core estimator using exact open/high/low/close series.

For each future return interval, compute cumulative variation `W(u)=T_u×V_rep(u)` using strike-axis Black-lognormal surface wings and Carr–Madan option replication. Interval variation is `ΔW=W(end)-W(max(start,as_of))`; materially negative values error, roundoff negatives clamp to zero with `1e-12` scale tolerance. Let carry `c=ln(D_f(start,end)/D_d(start,end))`, and for a partly elapsed interval let `a=ln(spot/latest_fixing)`. Forecast squared return is `ΔW+(a+c-ΔW/2)²`. Sum known and forecast squared returns, divide by the full scheduled return count, and annualize.

Cash payoff is signed variance notional times expected variance minus strike variance, discounted to effective settlement. Remaining-forward variance averages only forecast intervals. Variance vega is `side×N×DF×unrealized_sample_fraction`, a sensitivity to a variance level; ordinary vega is finite-difference smile-surface repricing. Strike-vol metric is `sqrt(strike_variance)` and time-to-maturity stops at final observation, not settlement.

## Selection and controls

Default discounting calls the local expected-variance routine. Close-to-close can be forecast; OHLC estimators are supported only once fully observed and error before settlement while future observations remain. Failed smile replication is an error; ATM-volatility fallback is explicitly disabled.

Surface must be Black lognormal on absolute strike and have valid put/call wings surrounding the forward. Spot lookup prioritizes the FX matrix; only if no matrix is present does it use `spot_id` (or concatenated pair ID) as a scalar. Historical close series defaults to that ID. Valuation cannot precede either discount curve base date. Settlement defaults to the final adjusted observation and cannot precede it. Spot is positive; dates/calendars/settlement and series are validated. Model year fraction, observation annualization and settlement date are separate controls.

## Limitations, failure and fallback

Static replication supplies expected quadratic variation, while the local discrete forecast additionally assumes a lognormal interval mean/second moment using that variation. Partial-return terms and annualization belong to that approximation. Full OHLC forecast models are deliberately unsupported. Smile wings, calendar consistency and boundary carry terms limit the interpretation of forecast variance.

## Alternatives and tradeoffs

ATM variance is simpler but discards smile exposure and is rejected here. A model of discrete sampling under stochastic volatility or estimator-specific OHLC forecasting would change the expectation and is not implemented. Exact settlement estimators remain supported without such a forecast.

## Evidence

- [finstack-quant/valuations/fx/src/fx_variance_swap/metrics.rs](../../../../finstack-quant/valuations/fx/src/fx_variance_swap/metrics.rs)
- [finstack-quant/valuations/fx/src/fx_variance_swap/pricer.rs](../../../../finstack-quant/valuations/fx/src/fx_variance_swap/pricer.rs)
- [finstack-quant/valuations/fx/src/fx_variance_swap/types.rs](../../../../finstack-quant/valuations/fx/src/fx_variance_swap/types.rs)

No separate independent numerical benchmark was established in this slice. Inline tests in the owning implementation provide only the assertions present there; they are not proof of accuracy outside those cases.

Canonical reference index: [Demeterfi](../../../REFERENCES.md#demeterfi-1999-volatility-swaps); [Carr Madan](../../../REFERENCES.md#carr-madan-1999-fft). The index supplies provenance pointers; agreement on the exact convention and validation domain remains open.

## Follow-up tracking

Adoption, implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
