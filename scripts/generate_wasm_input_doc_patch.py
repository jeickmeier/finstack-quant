"""Print an apply-patch payload for missing WASM input documentation.

This read-only helper only proposes Rustdoc for parameters the WASM input-doc
checker reports as missing.  The generated patch must still be applied through
the repository's normal patch workflow and verified by the checker.
"""

from __future__ import annotations

import argparse
from pathlib import Path

from check_wasm_api_input_docs import (
    exported_functions,
    input_paths,
    parameter_description,
)

DESCRIPTIONS = {
    "as_of": "ISO-8601 valuation date used to resolve date-dependent market data.",
    "actual": "Actual realized values aligned one-for-one with the forecast series.",
    "accreted_notional": "Outstanding notional after PIK accrual, in the debt's monetary units.",
    "asset_value": "Current fair value of the firm's assets in monetary units.",
    "asset_vol": "Annualized volatility of firm-asset returns, expressed as a decimal.",
    "base_date": "ISO-8601 curve base date from which time coordinates are measured.",
    "base_hazard": "Reference annual default intensity used by the leverage-to-hazard mapping.",
    "base_leverage": "Positive reference debt-to-assets leverage ratio for the hazard mapping.",
    "calendar": "Holiday-calendar identifier used for business-day adjustments.",
    "confidence": "Tail confidence as a decimal probability, such as 0.95 for 95%.",
    "correlation": "Instantaneous correlation between the documented asset and FX-rate shocks, from -1 to 1.",
    "currency": "ISO-4217 currency code for the monetary amount or market convention.",
    "div_yield": "Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.",
    "dividend": "Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.",
    "date": "ISO-8601 date used by the calculation or market-data lookup.",
    "day": "Calendar day number within the selected month.",
    "dates": "ISO-8601 dates in ascending order, aligned with the supplied observations.",
    "day_count": "Day-count convention identifier used to convert dates into year fractions.",
    "debt_barrier": "Positive debt face value defining the structural-model default barrier.",
    "delta": "Option delta expressed under the surface's documented delta convention.",
    "df": "Discount factor from valuation to expiry, expressed as a positive decimal.",
    "discount_curve_id": "Market-context discount-curve identifier for the instrument currency.",
    "extrapolation": "Extrapolation policy applied outside the last calibrated curve pillar.",
    "exponent": "Positive exponent controlling sensitivity in the documented power-law mapping.",
    "end": "Inclusive ISO-8601 end date or final date of the requested interval.",
    "end_date": "Inclusive ISO-8601 end date of the requested interval.",
    "entity": "Entity identifier used to group ordered time-series observations.",
    "equity_value": "Current market value of equity in the firm's monetary units.",
    "equity_vol": "Annualized equity-return volatility expressed as a decimal.",
    "equity_discount_rate": "Annual equity-holder discount rate used in the nested toggle decision.",
    "freq": "Observation or aggregation frequency token accepted by the Rust API.",
    "forecast": "Forecast values aligned one-for-one with the actual realized series.",
    "frequency": "Observation or aggregation frequency token accepted by the Rust API.",
    "forward": "Forward price or rate in the same quote convention as the strike.",
    "barrier": "Continuously monitored barrier level in the same price units as spot.",
    "barrier_uncertainty": "Lognormal dispersion of the CreditGrades default barrier, not a generic uncertainty score.",
    "direction": 'Barrier direction: `"up"` for an upper barrier or `"down"` for a lower barrier.',
    "knock": 'Barrier activation: `"in"` for knock-in or `"out"` for knock-out.',
    "averaging": 'Asian averaging convention: `"arithmetic"` (default) or `"geometric"`.',
    "num_fixings": "Positive number of equally spaced averaging observations before expiry.",
    "extremum": "Observed running minimum for a call or maximum for a put, in spot-price units.",
    "strike_type": 'Lookback payoff convention: `"fixed"` (default) or `"floating"`.',
    "id": "Stable identifier used to name and retrieve the supplied domain object.",
    "instrument_id": "Stable instrument identifier used for pricing and metric keys.",
    "is_call": "Whether to value a call (`true`) or put (`false`); defaults follow the callable's contract.",
    "b": "Right-hand-side vector of a linear system, aligned with the Cholesky factor dimension.",
    "chol": "Lower-triangular Cholesky factor of the coefficient matrix, in the documented matrix shape.",
    "data": "Non-empty numeric observation array used by the requested statistic.",
    "matrix": "Square numeric matrix in the nested or row-major shape required by this callable.",
    "market": "Market context or JSON payload supplying curves, quotes, and FX data.",
    "market_json": "Canonical market-context JSON supplying curves, quotes, and FX data.",
    "market_history": "Optional serialized historical market snapshots required by historical pricing models.",
    "mean_recovery": "Mean recovery rate at default expressed as a fraction from 0 through 1.",
    "model_json": "Serialized Merton structural-credit model produced by this API's model builder.",
    "metric_node": "Statement metric node identifier selected for the requested analysis.",
    "metrics": "Array of canonical metric identifiers to calculate with the instrument price.",
    "model": 'Pricing-model identifier; use `"default"` for the instrument-native model when supported.',
    "maturity": "ISO-8601 contractual maturity date of the instrument or cashflow.",
    "month": "Calendar month number from 1 through 12.",
    "n": "Positive count controlling the number of observations or results returned.",
    "n_paths": "Number of simulated stochastic paths; larger values improve sampling precision.",
    "n_terms": "Optional positive number of COS expansion terms; omit to use the pricer default.",
    "n_steps": "Number of discrete simulation or forecasting steps.",
    "nested_paths": "Number of nested Monte Carlo paths for continuation-value estimation; must fit JavaScript's safe integer range.",
    "num_paths": "Number of simulated stochastic paths; larger values improve sampling precision.",
    "num_steps": "Number of time steps per simulated path.",
    "notional": "Signed trade notional in the instrument's native currency units.",
    "periods": "Ordered period labels or observations aligned with the supplied data.",
    "period": "Model period label for the requested statement value or calculation.",
    "price": "Price in the documented quote convention for this instrument.",
    "prices": "Row-major price observations aligned with dates and instrument columns.",
    "q": "Continuous dividend yield or foreign rate, expressed as a decimal.",
    "r": "Continuously compounded risk-free rate, expressed as a decimal.",
    "rate": "Interest rate expressed as a decimal, such as 0.05 for 5%.",
    "rate_domestic": "Domestic continuously compounded risk-free rate, expressed as a decimal.",
    "rate_foreign": "Foreign continuously compounded risk-free rate, expressed as a decimal.",
    "rates": "Rate observations expressed as decimals and aligned with the supplied dates.",
    "recovery_rate": "Recovery assumption as a decimal fraction of par.",
    "recovery": "Recovery rate at default expressed as a fraction of par from 0 through 1.",
    "results": "Evaluated statement results used by the requested report or explanation.",
    "risk_free_rate": "Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%.",
    "rho": "Instantaneous correlation between the asset and variance shocks.",
    "seed": "Deterministic random-number seed used to reproduce simulation output.",
    "pricing_options": "Optional JSON pricing overrides accepted by the canonical instrument validator.",
    "pricing_seed": "Independent deterministic seed used for unbiased-pricing sampling.",
    "spot": "Current spot price or exchange rate in the documented quote convention.",
    "start": "Inclusive ISO-8601 start date or initial date of the requested interval.",
    "start_date": "Inclusive ISO-8601 start date of the requested interval.",
    "strike": "Option strike price in the same price units as the underlying.",
    "sigma": "Annualized volatility expressed as a decimal, such as 0.20 for 20%.",
    "theta": "Long-run variance level in the Heston stochastic-volatility model.",
    "ticker": "Ticker label identifying an existing return or price series.",
    "ticker_idx": "Zero-based ticker column index in tickerNames order.",
    "time_to_expiry": "Time remaining to expiry in years on the model's day-count basis.",
    "valuation_date": "ISO-8601 date on which the valuation is performed.",
    "volatility": "Annualized volatility expressed as a decimal, such as 0.20 for 20%.",
    "vol": "Annualized volatility expressed as a decimal, such as 0.20 for 20%.",
    "vol_asset": "Annualized asset-price volatility expressed as a decimal.",
    "vol_fx": "Annualized FX-rate volatility expressed as a decimal.",
    "vol_of_vol": "Annualized volatility of variance in the Heston stochastic-volatility model.",
    "weights": "Decimal portfolio weights aligned one-for-one with the supplied positions.",
    "window": "Positive rolling observation window length in the configured frequency.",
    "interp": "Interpolation method used between calibrated curve or surface pillars.",
    "interpolation_mode": "Volatility-surface interpolation mode used between quoted points.",
    "knots": "Ordered curve-pillar time and value pairs used to calibrate the curve.",
    "projection_grid": "Optional projection-tenor grid used to derive forward-rate periods.",
    "reset_lag": "Reset lag applied between an index fixing date and its effective period.",
    "tenor": "Underlying swap or index tenor measured in years for the quoted surface point.",
    "t": "Time from the curve base date in years on the documented day-count basis.",
    "t1": "Earlier curve time in years used as the start of the forward interval.",
    "t2": "Later curve time in years used as the end of the forward interval.",
    "validation_mode": "Curve-validation mode controlling admissible pillar and forward shapes.",
    "forward_floor": "Optional lower bound applied to implied instantaneous forward rates.",
    "options": "JavaScript options object defining the requested curve construction inputs.",
    "policy": "FX quote-selection policy for resolving direct, inverse, or triangulated rates.",
    "base": "Base currency code of the FX quote, where the rate is quote per base.",
    "quote": "Quote currency code of the FX rate, expressed per unit of base currency.",
    "continuous_rate": "Flat continuously compounded zero rate expressed as a decimal.",
    "expiry_idx": "Zero-based index of the requested expiry pillar in the volatility surface.",
    "expiry": "Time to option expiry in years on the model's annual time basis.",
    "exposures": "Factor-exposure matrix aligned with the supplied observations.",
    "kappa": "Mean-reversion speed of variance in the Heston stochastic-volatility model.",
    "v0": "Initial instantaneous variance in the Heston stochastic-volatility model.",
    "use_parallel": "Whether simulation paths are evaluated in parallel when supported.",
    "basis": "Regression basis family used by the American-option exercise estimator.",
    "basis_degree": "Maximum polynomial degree used by the American-option exercise basis.",
    "actual_var": "Per-position component VaR amounts aligned with the position identifiers.",
    "avg_daily_volume": "Average daily trading volume in the same units as the position size.",
    "base_ccy": "ISO-4217 base currency in which aggregate portfolio values are reported.",
    "cashflows": "Dated cashflow series in chronological order using the documented sign convention.",
    "config": "Configuration payload controlling the calculation's documented optional behavior.",
    "covariance": "Square covariance matrix aligned to the supplied position identifiers.",
    "execution_horizon_days": "Planned execution horizon measured in trading days.",
    "horizon_years": "Return-linking horizon measured in years for annualization.",
    "horizon": "Forward-looking model horizon measured in years.",
    "json": "Canonical JSON string defining the object to deserialize or normalize.",
    "json_str": "Canonical JSON string to validate, parse, or normalize for this API.",
    "market_price": "Observed market price in the instrument's documented quote convention.",
    "participation_rate": "Maximum fraction of average daily volume used for execution.",
    "permanent_impact_coef": "Permanent market-impact coefficient in the execution-cost model.",
    "portfolio": "Built portfolio object whose positions and weights are used by the calculation.",
    "portfolio_var": "Total portfolio VaR used to convert risk-budget shares into absolute amounts.",
    "position_ids": "Ordered position identifiers aligned with all supplied position vectors.",
    "position_pnls": "Scenario-major position P-and-L matrix aligned with the supplied positions.",
    "position_size": "Trade size in shares or notional units for the execution calculation.",
    "position_value": "Current position market value in the relevant currency units.",
    "reference_price": "Optional reference price used to express execution impact in monetary units.",
    "result": "Canonical result payload returned by the corresponding portfolio calculation.",
    "scenario": "Scenario specification describing market-data shocks before revaluation.",
    "snapshots": "Ordered market snapshots used to replay the portfolio through time.",
    "spread_mean": "Mean bid-ask spread in the quote units required by the liquidity model.",
    "spread_vol": "Volatility of the bid-ask spread in the liquidity model's units.",
    "spec": "JavaScript object or JSON payload defining the canonical instrument or calculation specification.",
    "spec_json": "Canonical portfolio specification JSON defining positions, quantities, and base currency.",
    "strict_risk": "Whether unavailable risk metrics are treated as calculation errors.",
    "target_var_pct": "Target decimal share of total portfolio VaR for each position.",
    "target_node": "Statement node identifier whose value is driven toward the target.",
    "target_period": "Model period label in which the goal-seek target is evaluated.",
    "target_value": "Numeric target value the goal-seek routine attempts to reach.",
    "driver_node": "Statement node identifier adjusted by the goal-seek routine.",
    "driver_period": "Model period label of the adjustable goal-seek driver.",
    "update_model": "Whether to return the model with the solved driver value applied.",
    "bounds_lo": "Lower numeric bound allowed for the goal-seek driver.",
    "bounds_hi": "Upper numeric bound allowed for the goal-seek driver.",
    "line_items": "Ordered statement line-item definitions included in the summary report.",
    "groups": "Group labels aligned with values for within-group cross-sectional operations.",
    "op": "Transformation operation identifier supported by the feature-engineering API.",
    "order": "Observation-order key used to sort each entity time series.",
    "other": "Second value series aligned with the primary series for a pairwise transformation.",
    "params": "Operation-specific parameter object defining transformation settings.",
    "time_key": "Cross-sectional time key shared by values evaluated in the same slice.",
    "values": "Numeric values in the order used by the requested numerical operation.",
    "temporary_impact_coef": "Temporary market-impact coefficient in the execution-cost model.",
    "utilization_threshold": "Actual-to-target risk ratio that flags a budget breach.",
    "var": "Base market value-at-risk before adding the liquidity adjustment.",
    "x": "Real-valued input to the requested scalar mathematical function.",
    "y": "Real-valued second input to the requested scalar mathematical function.",
    "volumes": "Trading-volume observations aligned one-for-one with returns.",
    "coupon_due": "Cash coupon amount due at the toggle decision date, in debt monetary units.",
    "lambda": "Annual jump-arrival intensity in the Merton jump-diffusion model.",
    "mu_jump": "Mean log jump size in the Merton jump-diffusion model.",
    "nu": "Variance-Gamma variance-rate parameter; larger values increase tail thickness.",
    "sigma_jump": "Standard deviation of log jump sizes in the Merton jump-diffusion model.",
    "quoted_clean": "Optional observed clean bond price in the schedule's documented price quotation convention.",
    "schedule_json": "Canonical cashflow-schedule JSON used to construct the fixed-income instrument.",
    "p": "Probability input strictly between 0 and 1 for the inverse normal distribution.",
    "distance_to_default": "Optional distance to default, measured as standard deviations from the default point.",
    "hazard_rate": "Annualized instantaneous default intensity, expressed as a decimal.",
    "leverage": "Debt-to-assets leverage ratio used by the structural credit model.",
    "threshold": "Threshold value in the units of the selected credit-state variable.",
    "total_debt": "Total debt face value in the firm's monetary units.",
    "variable": 'Credit-state variable: `"hazard_rate"`, `"distance_to_default"`, or `"leverage"`.',
    "year": "Four-digit calendar year component of the supplied date.",
}


def readable_name(parameter: str) -> str:
    """Convert a snake-case parameter name into prose."""
    return parameter.replace("_", " ")


def description(parameter: str, function: str) -> str:  # noqa: PLR0911
    """Return a substantive, domain-oriented default description."""
    if parameter == "q" and "quantile" in function:
        return "Quantile probability from 0 through 1 used to select the order statistic."
    if parameter == "maturity" and function.endswith("_cos_price"):
        return "Time to option expiry in years."
    if parameter == "theta" and function == "vg_cos_price":
        return "Variance-Gamma drift parameter controlling skew in log returns."
    if parameter == "n" and (function.endswith("_flat") or function.startswith("validate_correlation")):
        return "Positive square-matrix dimension; flat arrays must contain n x n entries."
    if parameter in {"x", "y"} and ("correlation" in function or "covariance" in function):
        return "Numeric observation series aligned one-for-one with the other series."
    if parameter == "direction" and function == "toggle_exercise_threshold_json":
        return 'Threshold comparison: `"above"` selects PIK above the level and `"below"` below it.'
    if parameter == "spec_json" and function == "dynamic_recovery_at_notional":
        return "Serialized DynamicRecoverySpec JSON defining the notional-to-recovery mapping."
    if parameter == "spec_json" and function.startswith("endogenous_hazard"):
        return "Serialized EndogenousHazardSpec JSON defining the leverage-to-hazard mapping."
    if parameter == "spec_json" and function == "transform_panel":
        return "Canonical panel-transformation JSON specifying input columns, operations, and parameters."
    if parameter in DESCRIPTIONS:
        return DESCRIPTIONS[parameter]
    if parameter.endswith("_json"):
        noun = readable_name(parameter.removesuffix("_json"))
        return f"Canonical JSON payload representing the {noun} consumed by this API."
    if parameter.endswith("_id"):
        noun = readable_name(parameter.removesuffix("_id"))
        return f"Stable {noun} identifier used to select the required domain object."
    if parameter.endswith("_curve"):
        noun = readable_name(parameter.removesuffix("_curve"))
        return f"Curve data used as the {noun} input for this calculation."
    if parameter.startswith(("is_", "include_")):
        return f"Boolean option controlling whether {readable_name(parameter)} applies."
    return (
        f"{readable_name(parameter).capitalize()} supplied to {function.replace('_', ' ')}; "
        "follow the type and convention required by the surrounding API."
    )


def patch_for(path: Path) -> list[str]:
    """Return patch hunks that add only missing parameter descriptions."""
    lines = path.read_text(encoding="utf-8").splitlines()
    hunks: list[str] = []
    for function in exported_functions(path):
        missing = [
            parameter
            for parameter in function.parameters
            if parameter_description(function.docstring, parameter) is None
        ]
        if not missing:
            continue
        index = function.line - 1
        source_line = lines[index]
        indent = source_line[: len(source_line) - len(source_line.lstrip())]
        docs = [f"{indent}/// @param {parameter} - {description(parameter, function.name)}" for parameter in missing]
        hunks.extend([
            "@@",
            f"-{source_line}",
            *[f"+{doc}" for doc in docs],
            f"+{source_line}",
        ])
    return hunks


def main() -> None:
    """Print a patch for one or more WASM Rust files."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="+", type=Path)
    args = parser.parse_args()
    paths = input_paths(args.paths)
    print("*** Begin Patch")
    for path in paths:
        hunks = patch_for(path)
        if hunks:
            print(f"*** Update File: {path}")
            print("\n".join(hunks))
    print("*** End Patch")


if __name__ == "__main__":
    main()
