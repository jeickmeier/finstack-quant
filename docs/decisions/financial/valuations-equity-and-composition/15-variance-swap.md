# Equity variance swap replication, seasoning and variance risk

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-15`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

PV is discounted signed variance-notional times expected realized variance minus strike variance. Historical and forward components are weighted by scheduled realized samples; expected forward variance is option-strip replication, with an explicit flat-volatility override as a separate input path.

## Scope and input domain

Positive variance notional, nonnegative strike variance, explicit observation tenor/calendar/adjustment, positive annualization setting and settlement no earlier than final observation. Historical prices use exact dates; OHLC methods require open/high/low and close series. Adjusted/raw series policy is contract metadata, not an in-pricer corporate-action adjustment.

## Algorithm

1. Generate contractual observation dates and method-specific sample fraction w. Before start, use forward-start variance; after final observation use fully realized variance; during life use `w×realized+(1−w)×forward`. Discount to effective settlement; after settlement return zero.
2. Historical close returns use core realized-variance statistics; OHLC estimator selection delegates to core. Missing series with at least two past close observations errors. With fewer observations, live spot may stand in as a singleton; no realized return then exists.
3. A positive implied-volatility override returns σ². Otherwise require a Black strike surface, compute `F=S exp(-qT)/D`, price options across its strikes with the resolved surface smile and invoke Carr–Madan replication. Insufficient strip coverage errors; no ATM fallback is substituted.
4. Before start, subtract spot total variances: `(Vend Tend−Vstart Tstart)/(Tend−Tstart)`; negative result errors. Intervals/start times ≤ `1e-6` years use variance to end instead.
5. VarianceVega is signed `N D(1−w)` per unit decimal variance; Vega is `2 N D sqrt(Vforward)(1−w)×0.01`. DV01 is one-sided repricing per bp, including curve effects on replication. Notional conversion uses `vega_notional/(2 strike_vol×0.01)`, returning zero for nonpositive strike vol.

## Selection, controls and conventions

Close series defaults to spot ID. Final observation may differ from stated maturity after calendar adjustment; settlement defaults to final observation. The public elapsed-time helper uses calendar year fractions and suppresses date/year-fraction errors to maturity/zero, but pricing uses observation weighting. Cashflow export is a placeholder, not the settlement payoff.

## Limitations, failures and alternatives

Replication is finite-strike and surface-dependent; its vega metric is an aggregate-vol chain rule, not a full surface-bump derivative. Price-series policy does not verify adjusted history. Empty realized-history branches can return zero PV, and the elapsed-time helper's fallback is separate from strict valuation. Model-implied variance or explicit variance curves are alternatives requiring separate approval.

## Evidence

[variance replication](../../../REFERENCES.md#demeterfi-1999-volatility-swaps); [valuation.rs](../../../../finstack-quant/valuations/equity/tests/variance_swap/valuation.rs), [variance_calculation.rs](../../../../finstack-quant/valuations/equity/tests/variance_swap/variance_calculation.rs) and [observation.rs](../../../../finstack-quant/valuations/equity/tests/variance_swap/observation.rs) locate flat-vol, observation and seasoned identities. These do not independently validate finite-strike smile tails or adjusted historical data.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/equity/src/variance_swap/types.rs)
- [pricer.rs](../../../../finstack-quant/valuations/equity/src/variance_swap/pricer.rs)
- [metrics.rs](../../../../finstack-quant/valuations/equity/src/variance_swap/metrics.rs)
