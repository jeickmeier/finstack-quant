# Trading Systems Reference

Review criteria for systematic trading, backtesting, and execution code.

## Backtesting Methodology

### Look-Ahead Bias

The most dangerous bug in quant trading code. Check for:

- **Data alignment**: Ensure signals at time t use only data available at time t. Common mistake: using close prices to generate signals that trade at the close.
- **Index membership**: If using a universe filter (e.g., S&P 500 constituents), verify the constituents list is as-of the backtest date, not the current date.
- **Fundamental data**: Earnings, revenue, etc., must use point-in-time data with publication lag. Check for restatement handling.
- **Feature engineering**: Any rolling calculation (moving average, rolling vol) must not include the current observation if the signal is used at the same timestamp.
- **Data joins in SQL**: Verify temporal joins use `WHERE date <= signal_date` with `QUALIFY ROW_NUMBER() OVER (... ORDER BY date DESC) = 1` or equivalent. Flag any equi-join on dates without temporal logic.

### Survivorship Bias

- Verify the backtest universe includes delisted securities.
- Check that returns are calculated correctly for delistings (final return, not just dropped).
- For crypto: include de-listed tokens and exchanges that shut down.
- For fixed income: include defaulted bonds.

### Transaction Cost Modeling

- Check that costs scale with trade size (market impact, not just fixed commissions).
- Verify bid-ask spread is applied correctly: buy at ask, sell at bid. Flag any backtest using mid prices for execution.
- Market impact model: square-root model (`impact ∝ σ√(v/V)`) or linear is common. Verify parameters are realistic.
- Slippage: check that the delay between signal and execution is modeled.
- Borrowing costs for short positions: verify they're included and time-varying.

### Overfitting Detection

- Count the number of free parameters relative to the number of independent observations. Flag parameter-to-observation ratios above 1:50.
- Check for out-of-sample testing. The test set must be truly held out — no iterative peeking.
- Walk-forward analysis: verify that the walk-forward window does not overlap with the training window.
- Multiple testing correction: if many strategies/parameters were tested, flag the need for Bonferroni or FDR correction.
- Verify that performance metrics are computed correctly: Sharpe ratio should use excess returns, annualization should match the return frequency.

## Signal Construction

### Alpha Research Patterns

- **Normalization**: Cross-sectional signals should be z-scored or rank-normalized at each time step. Check for outlier handling.
- **Decay**: Verify signal decay is modeled (exponential or linear). Stale signals should be explicitly handled.
- **Combination**: When combining signals, check for multicollinearity. Verify that the combination weights are estimated out-of-sample.
- **Turnover**: Calculate signal turnover — very high turnover signals are expensive to trade. Flag turnover > 200% annualized without explicit justification.

### Data Handling

- **Missing data**: Check how gaps are handled — forward-fill is common but can introduce bias. Verify no interpolation of future data.
- **Corporate actions**: Splits, dividends, mergers — check that price data is properly adjusted. Verify adjustment factors come from a reliable source.
- **Timestamps**: Verify timezone handling. Market data should be in exchange local time or UTC with explicit conversion. Flag any naive datetime usage.
- **WASM/JS specifics**: `Date` in JS has timezone pitfalls. Prefer epoch milliseconds for all timestamp handling. Verify WASM and JS agree on date arithmetic.

## Execution Algorithms

### Order Management

- Check order lifecycle: new → acknowledged → partial fill → filled / cancelled. Verify all state transitions are handled.
- Verify that the position manager reconciles with the execution engine. Flag any code that tracks positions independently without reconciliation.
- Check for race conditions in async order handling (common in Rust async / JS Promise patterns). Verify that order state mutations are atomic or properly locked.

### Execution Quality

- **TWAP/VWAP**: Check that the volume profile is realistic and time buckets are correct.
- **Implementation shortfall**: Verify the benchmark price capture (decision price, not arrival price if there's a delay).
- **Smart order routing**: Check that venue selection logic considers latency, fill probability, and rebates.

### Risk Controls

- **Pre-trade checks**: Position limits, notional limits, order size limits. Verify these are enforced before the order reaches the market.
- **Kill switch**: Verify the system has a way to cancel all open orders and flatten positions. Test the kill switch path.
- **P&L limits**: Daily loss limits should trigger automatic position reduction. Check that unrealized and realized P&L are both considered.
- **Fat finger checks**: Verify that order prices are within a reasonable band of the market. Flag any code that sends orders without price sanity checks.

## Portfolio Optimization

### Mean-Variance Optimization

- Check that the covariance matrix is estimated robustly (shrinkage estimator, not sample covariance for high-dimensional cases).
- Verify that the covariance matrix is positive semi-definite. If eigenvalue clipping is used, document the threshold.
- Constraints: verify long-only, turnover, sector exposure, and position size constraints are correctly implemented.
- Transaction costs in optimization: verify they're included in the objective, not just as a post-hoc check.

### Risk Parity and Factor Models

- **Risk parity**: Verify the risk contribution calculation is correct. Equal risk contribution ≠ equal weight.
- **Factor exposure**: Check that factor loadings are estimated with appropriate lookback and that factor covariance is updated.
- **Rebalancing**: Verify rebalancing frequency, threshold-based triggers, and that partial rebalancing handles rounding correctly.

## Performance Attribution

- **Brinson attribution**: Verify allocation, selection, and interaction effects sum to total active return.
- **Factor attribution**: Check that factor returns are correctly aligned with portfolio holdings dates.
- **Transaction cost attribution**: Separate implementation shortfall from alpha decay.
