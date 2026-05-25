# Pricing Models Reference

Review criteria for derivatives pricing implementations.

## Black-Scholes Framework

### Common Implementation Errors

- **d₁/d₂ formulas**: Verify `d1 = (ln(S/K) + (r - q + σ²/2)T) / (σ√T)` and `d2 = d1 - σ√T`. Common mistake: using `r` instead of `r - q` (missing dividend yield).
- **Put-call parity**: Use as a consistency check. If `C - P ≠ Se^{-qT} - Ke^{-rT}`, there's a bug.
- **Greeks**:
  - Delta: `N(d1)` for calls — verify `N` is the CDF, not the PDF.
  - Gamma: `n(d1) / (Sσ√T)` — check that `n` is the standard normal PDF.
  - Vega: `Sn(d1)√T` — this is the same for calls and puts.
  - Theta: Most complex Greek — verify each term and sign convention (usually negative for long options).
- **Edge cases**: T→0 (option at expiry), σ→0 (option becomes max(S-K,0) discounted), S=0 (call worth 0, put worth Ke^{-rT}).

### Discrete Dividends

- Escrowed dividend model: Adjust spot by PV of dividends.
- Check that dividend timing is correct relative to the valuation date.
- For large dividends relative to spot, verify no negative adjusted forward.

## Local Volatility

### Dupire's Formula

- `σ_local² = (∂C/∂T + (r-q)K·∂C/∂K + qC) / (0.5K²·∂²C/∂K²)`
- Verify numerical derivatives are stable — use smoothed implied vol surface, not raw market data.
- Check that local vol is positive everywhere. Negative local vol indicates arbitrage in the input surface.
- Grid resolution: local vol is sensitive to strike spacing. Test with refined grids.

### Implementation Checks

- Forward PDE vs. backward PDE: verify the correct one is used for the pricing method.
- Mixing index: if blending local vol with stochastic vol, check the mixing parameter is calibrated, not hardcoded.

## Stochastic Volatility

### Heston Model

- Parameters: v₀ (initial variance), κ (mean reversion), θ (long-run variance), σ_v (vol of vol), ρ (correlation).
- **Feller condition**: `2κθ > σ_v²` ensures variance stays positive. If violated, verify the discretization handles zero-crossing correctly (e.g., absorption, reflection, or full truncation).
- **Characteristic function**: Verify the correct formulation is used (Heston original has a branch-cut issue — use the Albrecher et al. or Lord-Kahl rotation).
- **FFT pricing**: Check that the integration contour avoids singularities. Verify the damping factor α is chosen correctly (0.75 is not always appropriate).
- **Calibration**: Typical to calibrate to vanillas and check exotic prices. Verify the calibration objective weights short-dated and long-dated options appropriately.

### SABR Model

- Parameters: α (initial vol), β (CEV exponent), ρ (correlation), ν (vol of vol).
- **Hagan approximation**: Only accurate for near-ATM. For extreme strikes, use Obloj correction or PDE solution.
- **β selection**: Usually fixed (0 for normal, 0.5 for CIR, 1 for lognormal) based on market convention, not calibrated.
- **Negative rates**: Standard SABR assumes F > 0. For negative rates, use shifted SABR or free-boundary SABR.
- **Backbone**: Check that the ATM vol moves correctly as spot moves (the "backbone" behavior). Miscalibrated SABR gives wrong delta.

## Rate Models

### Short-Rate Models

- **Hull-White**: Verify the time-dependent θ(t) is calibrated to match the initial yield curve exactly. Check mean reversion and vol are consistent with swaption vols.
- **Tree construction**: Verify that trinomial tree probabilities are all in [0,1]. Flag negative probabilities.
- **Bermudan pricing**: Check that exercise decisions are on the correct dates, with proper day-count adjustments.

### LIBOR/SOFR Market Models

- **Drift**: In the terminal measure, only the last forward rate has zero drift. All others have measure-dependent drift corrections — verify these are correct.
- **Correlation**: Factor reduction (PCA) of the correlation matrix. Verify the reduced matrix is still PSD and captures the major modes.
- **Transition to SOFR**: Check that compounding conventions are correct (SOFR is daily compounded, not simple interest like LIBOR).

## Exotic Payoffs

### Path-Dependent Options

- **Asian options**: Arithmetic average has no closed form — verify MC or PDE method. Geometric average as control variate.
- **Barriers**: Check barrier monitoring (continuous vs. discrete). Discrete monitoring requires Broadie-Glasserman-Kou correction. Verify near-barrier behavior (should be smooth, not jagged).
- **Lookback**: Verify running max/min is tracked correctly through the path. Check that floating vs. fixed strike is implemented correctly.
- **Autocallables**: Verify coupon accrual, memory feature, and knock-in put correctly. Check that observation dates align with market conventions.

### Multi-Asset Options

- **Correlation**: Verify the correlation matrix is used consistently throughout the simulation. Check that Cholesky decomposition is applied to the correct matrix (instantaneous correlation, not terminal correlation).
- **Basket options**: Check weighting and rebalancing conventions. Verify quanto adjustments if assets are in different currencies.
- **Worst-of**: Verify that the worst performer is correctly identified at each observation date.

## Calibration Best Practices

- Always verify calibration by repricing the calibration instruments and checking residuals.
- Check that calibration is stable: small perturbations in market data should not cause large parameter changes.
- Log calibration results (parameters, residuals, iterations, convergence flag) for audit trail.
- Interpolated/extrapolated market data used in calibration should be flagged and documented.
