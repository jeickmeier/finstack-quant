# Risk Models Reference

Review criteria for risk management implementations.

## Value at Risk (VaR)

### Sign convention baseline

- Define VaR on losses unless the API explicitly returns a signed P&L threshold: `loss = -pnl`.
- For positive-loss reporting, `VaR_95` is the 95th percentile of the loss distribution and `ES_95` is the average loss conditional on loss being at or beyond that threshold.
- Flag APIs that return negative VaR without making clear that the value is a P&L quantile.
- Verify percentile orientation carefully: 95% confidence VaR on returns usually corresponds to the 5th percentile of P&L or returns, not the 95th percentile of gains.

### Historical Simulation

- Check that the historical window is appropriate (250 days typical, but regime-dependent).
- Verify return calculation: log returns vs. arithmetic returns. For multi-day VaR, verify scaling.
- Weighting: exponentially weighted (BRW — Boudoukh, Richardson, Whitelaw) or equal weight. If weighted, verify decay factor.
- P&L calculation: full revaluation vs. delta/gamma approximation. Full reval is more accurate but slower — check appropriateness.
- **Corner cases**: What happens when the portfolio has options near expiry? The P&L distribution is non-smooth.

### Parametric VaR

- Check that the distribution assumption is documented and justified (normal, Student-t, etc.).
- Verify correlation matrix construction. Check staleness — correlations should be updated regularly.
- For non-linear portfolios (options): verify delta-gamma approximation includes the gamma term `0.5 Γ ΔS²`.
- Cornish-Fisher expansion: if used for fat-tailed adjustment, verify skewness and kurtosis estimates are robust.

### Monte Carlo VaR

- Verify that the risk factor simulation is consistent with the pricing models.
- Check that the number of scenarios is sufficient for the confidence level (10,000 scenarios for 99% VaR is marginal — check convergence).
- Scenario generation: verify that the correlation structure is preserved (Cholesky or PCA-based).
- Full revaluation: verify that all instruments are repriced under each scenario (not just delta-approximated).

### VaR Validation

- **Backtesting**: Verify Kupiec test (unconditional coverage) and Christoffersen test (independence + coverage).
- **P&L explanation**: Daily P&L should be explainable by risk factors. Unexplained P&L > 10% of total is a red flag.
- **VaR vs. actual**: Check that exceptions cluster analysis is performed. Clustered exceptions indicate model failure.

## Stress Testing

### Scenario Design

- **Historical scenarios**: Verify scenario dates and risk factor moves are correctly sourced. Check that all relevant risk factors are stressed (not just equity, but also rates, vol, correlation, liquidity).
- **Hypothetical scenarios**: Verify that stressed parameters are internally consistent (e.g., if equity drops 20%, vol should increase, correlations should increase).
- **Reverse stress testing**: Given a loss threshold, find the scenario. Check that the optimization is well-posed.

### Implementation Checks

- Verify that stress P&L is computed with full revaluation, not delta approximation.
- Check that stressed Greeks are recalculated (not using base-case Greeks with stressed prices).
- Verify that liquidity stress is modeled: wider bid-ask spreads, reduced volumes, increased market impact.
- For regulatory stress tests: verify that the scenario specification matches the regulatory guidance exactly.
- Prefer typed shock units: rate bp, credit-spread bp, vol points, relative spot percentage, absolute price, FX spot, and curve/key-rate shocks. Flag raw `f64` shocks without unit naming.

## Greeks Computation

### Finite Difference Greeks

- **Bump size**: Too large → approximation error. Too small → numerical noise. Typical: 1bp for rates, 1% relative for spot, 1 vol point for vega.
- **Central vs. forward difference**: Central `(f(x+h) - f(x-h))/(2h)` is O(h²) accurate. Forward `(f(x+h) - f(x))/h` is only O(h). Prefer central.
- **Cross-Greeks**: Verify that cross-gamma (e.g., ∂²V/∂S∂σ) uses consistent bump methodology.
- **Path-dependent options**: Greeks may require pathwise derivatives or likelihood ratio method, not simple bumping. Check that the method is appropriate.

### Algorithmic Differentiation

- **Forward mode (tangent)**: Efficient for few inputs, many outputs. Check seed vector correctness.
- **Reverse mode (adjoint)**: Efficient for many inputs, few outputs (typical for Greeks). Verify tape recording captures all computations. Check for checkpointing in memory-intensive paths.
- **Rust AAD**: If using custom AD, verify operator overloading correctness for all math operations. Check that `f64` operations are all captured by the AD type.

### Greek Aggregation

- Delta/gamma are per-underlying: verify that multi-asset portfolio Greeks are aggregated by underlying, not just summed.
- Cross-gamma matrix: verify symmetry and that the matrix is aggregated correctly across trades.
- VaR-based Greeks: verify that the perturbation is applied to the correct risk factor (e.g., bumping the zero curve, not the par rate).

## Correlation Modeling

### Estimation

- **Sample correlation**: Verify sufficient observations (minimum 2× the dimension of the matrix for stability).
- **Exponential weighting**: Common for capturing regime shifts. Check decay factor (λ = 0.94 for RiskMetrics daily).
- **Shrinkage**: Ledoit-Wolf or similar. Check that the shrinkage target is appropriate (identity, constant correlation, or market factor).

### Matrix Properties

- **Positive semi-definiteness**: Every correlation matrix must be PSD. After any adjustment (nearPD, eigenvalue clipping), verify the result is still a valid correlation matrix (diagonal = 1, off-diagonal ∈ [-1,1]).
- **Spectral decomposition**: If using PCA for dimension reduction, verify the number of factors retained explains sufficient variance (typically 90%+). Check that the reduced matrix is used consistently everywhere.

### Stress Correlation

- Correlation breakdown in stress: verify that stressed correlations are applied consistently.
- Correlation → 1 in stress: check that the limiting behavior is handled (perfect correlation collapses diversification benefit).
- Block correlation: if assets are grouped by sector/asset class, verify that intra-block and inter-block correlations are set consistently.

## Regulatory Capital

### Market Risk (FRTB)

- **Sensitivity-based method**: Verify risk weights and correlation parameters match the regulatory specification.
- **DRC (Default Risk Charge)**: Check that jump-to-default is correctly simulated and that recovery rates are appropriate.
- **RRAO (Residual Risk Add-On)**: Flag any exotic instrument that should be included.

### Counterparty Risk

- **CVA**: Check that the exposure profile is correctly simulated (expected positive exposure, potential future exposure).
- **Wrong-way risk**: Verify that correlation between counterparty default and exposure is modeled.
- **Margin period of risk**: Check that the MPOR assumption matches the collateral agreement.

## Data Quality for Risk

- **Stale data detection**: Check for repeated values in market data feeds. Stale inputs can silently corrupt risk numbers.
- **Missing data handling**: Verify interpolation/extrapolation logic for missing risk factors. Log all synthetic data points.
- **Reconciliation**: Risk factor inputs should be reconciled against front-office marks daily. Flag discrepancies above threshold.
