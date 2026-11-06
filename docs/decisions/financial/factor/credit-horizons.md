# FACTOR-018: Annualized covariance and idiosyncratic horizon scaling

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the stated source scope; limitations and discrepancies below remain open.

**Validation status:** Evidence inspected or located, not executed. No numerical qualification is implied.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Forecast by scaling the calibrated authoritative covariance linearly in a requested year horizon, preserving Ridge/LedoitWolf adjustments. Sample and EWMA states use flat variance forecasts; no new observations or mean reversion are processed.

## Input domain and conventions

Stored covariance and idiosyncratic variances are annualized. `OneStep` and `Unconditional` both mean multiplier 1; `NSteps(n)` means n annualized model periods, not n panel observations. `Years(y)` uses finite nonnegative fractional years. A ten-business-day horizon needs Years(10/252) or the explicit parser denominator.

## Algorithm stages

1. Parse exact trimmed keywords or a JSON descriptor. A numeric years field takes precedence; otherwise read nonnegative u64 n_steps. With positive finite periods_per_year convert n/P to Years, otherwise cast n to usize.
2. Validate the resulting variance multiplier is finite/nonnegative. Verify covariance IDs/order match current config factor definitions.
3. Multiply every stored covariance entry by the horizon and construct a validated FactorCovarianceMatrix. Do not rebuild from static_correlation or vol_state.factors.
4. Idiosyncratic lookup requires the issuer variance state; multiply stored Sample/EWMA variance by the same horizon, reject nonfinite/negative result, and return its square root. EWMA lambda is not used in this forecast.
5. `factor_model_at` clones config, substitutes scaled covariance and caller risk measure, and returns it without invoking full configuration/risk-measure validation.

## Selection and controls

Horizon zero gives zero covariance/volatility after multiplier validation. Parser objects tolerate extra keys and choose years before n_steps; the descriptor method emits a canonical-looking string but does not validate an in-memory invalid Years value. Construction of the forecast wrapper does not validate the entire model.

## Limitations, failures, and fallbacks

Unconditional is a label for unchanged variance, not an estimated long-run mean-reverting limit. Raw n_steps uses an unchecked u64-to-usize cast, so values above u32::MAX differ on a 32-bit target. Scaling may overflow, but covariance construction and idiosyncratic result checks reject nonfinite outputs. A directly supplied invalid risk measure can survive `factor_model_at` until a later consumer validates it. The cloned matching configuration also retains annualized issuer adder volatilities unchanged. The portfolio FactorModel collector squares those matching-row values directly and its residual overlay has no horizon argument. Consequently a non-unit-horizon generated config can combine horizon-scaled systematic covariance with annual residual variance; this cross-boundary consequence is a static source trace, not an executed reproduction. See [portfolio residual risk](../portfolio-attribution/13-credit-residual-risk-and-reporting.md).

## Alternatives and tradeoffs

Panel-frequency-relative horizons, actual day-count conversions, or mean-reverting forecasts would require an explicit clock/state model. Reconstructing covariance from volatility diagnostics would erase systematic regularization and is not an equivalent alternative.

## Implementation and evidence

- [src/credit/forecast.rs](../../../../finstack-quant/factor/src/credit/forecast.rs) — inspected implementation.

[src/credit/forecast.rs](../../../../finstack-quant/factor/src/credit/forecast.rs) tests locate fractional horizon parsing, zero/invalid scales, idiosyncratic overflow, and preservation of calibrated Ridge/LedoitWolf matrices. The mathematical square-root-of-time identity is conditional on the flat covariance assumption; no out-of-sample forecast validation is implied.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models). This record does not
authorize a runtime change or certify the observed method.
