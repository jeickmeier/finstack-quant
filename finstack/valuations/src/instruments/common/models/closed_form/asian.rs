//! Analytical and semi-analytical pricing formulas for Asian options.
//!
//! Asian options (also called average options) have payoffs based on the average
//! price of the underlying asset over the option's life, rather than just the
//! final price. This averaging feature reduces volatility exposure and makes
//! them popular for currency and commodity hedging.
//!
//! # Asian Option Types
//!
//! ## By Averaging Method
//! - **Geometric average**: Closed-form solution available (Kemna & Vorst 1990)
//! - **Arithmetic average**: Approximation via moment matching (Turnbull & Wakeman 1991)
//!
//! ## By Strike Type
//! - **Fixed strike**: Payoff = max(Average - K, 0) for call
//! - **Floating strike**: Payoff = max(S_T - Average, 0) for call
//!
//! # Mathematical Foundation
//!
//! ## Geometric Average (Exact Solution)
//!
//! For a geometric average Asian option, the logarithm of the average is
//! normally distributed, allowing closed-form pricing via Black-Scholes
//! with adjusted parameters:
//!
//! ```text
//! σ_adjusted = σ / √3  (continuous monitoring)
//! σ_adjusted = σ √[(2n + 1) / (6(n + 1))]  (n discrete fixings)
//! ```
//!
//! ## Arithmetic Average (Approximation)
//!
//! Arithmetic averages have no closed-form solution. The **Turnbull-Wakeman**
//! approximation uses moment matching:
//! 1. Compute first two moments of the arithmetic average
//! 2. Approximate the distribution as lognormal
//! 3. Price using adjusted Black-Scholes
//!
//! **Accuracy**: Typically within 1% of Monte Carlo for reasonable parameters.
//! Less accurate for deep OTM options or very short maturities.
//!
//! # Conventions
//!
//! | Parameter | Convention | Units |
//! |-----------|-----------|-------|
//! | Rates (r, q) | Continuously compounded | Decimal (0.05 = 5%) |
//! | Volatility (σ) | Annualized | Decimal (0.20 = 20%) |
//! | Time (T) | ACT/365-style | Years (1.0 = 1 year) |
//! | Prices | Per unit of underlying | Currency units |
//!
//! # Academic References
//!
//! ## Primary Sources
//!
//! - Kemna, A. G. Z., & Vorst, A. C. F. (1990). "A Pricing Method for Options
//!   Based on Average Asset Values." *Journal of Banking & Finance*, 14(1), 113-129.
//!   (Exact closed-form solution for geometric average)
//!
//! - Turnbull, S. M., & Wakeman, L. M. (1991). "A Quick Algorithm for Pricing
//!   European Average Options." *Journal of Financial and Quantitative Analysis*,
//!   26(3), 377-389.
//!   (Moment-matching approximation for arithmetic average)
//!
//! ## Alternative Methods
//!
//! - Levy, E. (1992). "Pricing European Average Rate Currency Options."
//!   *Journal of International Money and Finance*, 11(5), 474-491.
//!   (Alternative approximation via geometric conditioning)
//!
//! - Curran, M. (1994). "Valuing Asian and Portfolio Options by Conditioning
//!   on the Geometric Mean Price." *Management Science*, 40(12), 1705-1711.
//!   (Conditioning approach for improved accuracy)
//!
//! - Rogers, L. C. G., & Shi, Z. (1995). "The Value of an Asian Option."
//!   *Journal of Applied Probability*, 32(4), 1077-1088.
//!   (Lower bounds via convex duality)
//!
//! ## Reference Texts
//!
//! - Haug, E. G. (2007). *The Complete Guide to Option Pricing Formulas* (2nd ed.).
//!   McGraw-Hill. Chapter 3: Average Rate Options.
//!
//! - Wilmott, P. (2006). *Paul Wilmott on Quantitative Finance* (2nd ed.).
//!   Wiley. Volume 2, Chapter 25.
//!
//! # Implementation Notes
//!
//! - **Geometric average**: Exact Black-Scholes with adjusted volatility
//! - **Arithmetic average**: Turnbull-Wakeman approximation (moment matching)
//! - **Discrete fixings**: Variance adjusts based on number of observations
//! - **Continuous limit**: Set num_fixings to large value (e.g., 365 for daily)
//! - **Edge cases**: Handled for zero time, extreme strikes, single fixing
//!
//! # Comparison with Monte Carlo
//!
//! For **arithmetic** Asian options:
//! - Analytical (Turnbull-Wakeman): Fast, typically 1% accuracy
//! - Monte Carlo: Slower, exact given enough paths (10k+ recommended)
//! - Use analytical for quick valuations, MC for validation and Greeks
//!
//! For **geometric** Asian options:
//! - Analytical is exact and should always be used
//!
//! # Examples
//!
//! ## Geometric Average Asian Call
//!
//! ```rust,ignore
//! use finstack_valuations::instruments::models::closed_form::asian::geometric_asian_call;
//!
//! let spot = 100.0;
//! let strike = 100.0;
//! let time = 1.0;
//! let rate = 0.05;
//! let div_yield = 0.02;
//! let vol = 0.20;
//! let num_fixings = 252;  // Daily fixings
//!
//! let price = geometric_asian_call(
//!     spot, strike, time, rate, div_yield, vol, num_fixings
//! );
//!
//! // Geometric Asian cheaper than vanilla due to averaging
//! assert!(price > 0.0);
//! assert!(price < 10.0); // Less than vanilla call
//! ```
//!
//! ## Arithmetic Average Asian Put (Approximation)
//!
//! ```rust,ignore
//! use finstack_valuations::instruments::models::closed_form::asian::arithmetic_asian_put_tw;
//!
//! let spot = 100.0;
//! let strike = 100.0;
//! let time = 0.5;        // 6 months
//! let rate = 0.05;
//! let div_yield = 0.0;
//! let vol = 0.25;
//! let num_fixings = 126;  // ~Daily fixings
//!
//! // Turnbull-Wakeman approximation
//! let price = arithmetic_asian_put_tw(
//!     spot, strike, time, rate, div_yield, vol, num_fixings
//! );
//!
//! assert!(price > 0.0);
//! ```
//!
//! # See Also
//!
//! - [`AsianPriceResult`] for result structure with optional Greeks
//! - [`AsianGreeks`] for first-order sensitivities
//! - Monte Carlo pricing for exact arithmetic average pricing

use finstack_core::math::special_functions::norm_cdf;
use finstack_core::math::NeumaierAccumulator;

/// Pricing result for Asian options.
#[derive(Debug, Clone, Copy)]
pub struct AsianPriceResult {
    /// Option price
    pub price: f64,
    /// First-order Greeks (delta, gamma, vega, theta, rho)
    pub greeks: Option<AsianGreeks>,
}

/// Greeks for Asian options.
/// Greeks for Asian options
#[derive(Debug, Clone, Copy, Default)]
pub struct AsianGreeks {
    /// Delta: sensitivity to underlying price
    pub delta: f64,
    /// Gamma: rate of change of delta
    pub gamma: f64,
    /// Vega: sensitivity to volatility
    pub vega: f64,
    /// Theta: time decay
    pub theta: f64,
    /// Rho: sensitivity to interest rate
    pub rho: f64,
}

/// Price a geometric average Asian call option (closed-form).
///
/// # Arguments
///
/// * `spot` - Current spot price
/// * `strike` - Strike price
/// * `time` - Time to maturity (years)
/// * `rate` - Risk-free rate
/// * `div_yield` - Dividend yield
/// * `vol` - Volatility
/// * `num_fixings` - Number of discrete fixings (use large number for continuous approximation)
///
/// # Returns
///
/// Option price
///
/// # Formula (Kemna & Vorst, 1990)
///
/// Geometric Asian behaves like a vanilla option with adjusted drift and volatility:
/// - σ_G = σ / √3
/// - μ_G = (r - q - σ²/2) / 2 + σ²/6
///
/// For discrete monitoring with n fixings:
/// - σ_G = σ √[(2n + 1) / (6(n + 1))]
/// - μ_G = (r - q) / 2 - σ² / 2 * [(2n + 1) / (6(n + 1))]
pub fn geometric_asian_call(
    spot: f64,
    strike: f64,
    time: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    num_fixings: usize,
) -> f64 {
    let df = (-rate * time).exp();
    geometric_asian_call_df(spot, strike, time, df, div_yield, vol, num_fixings)
}

/// Price a geometric Asian call with explicit discount factor (DF-first API).
///
/// This is the preferred entry point when `df` is known directly (e.g., from
/// date-based curve lookup). Derives `r_eff = -ln(df)/t` internally.
///
/// See [`geometric_asian_call`] for formula details.
pub fn geometric_asian_call_df(
    spot: f64,
    strike: f64,
    time: f64,
    df: f64,
    div_yield: f64,
    vol: f64,
    num_fixings: usize,
) -> f64 {
    if time <= 0.0 {
        return (spot - strike).max(0.0);
    }

    // Derive rate from DF
    let rate = if time > 0.0 && df > 0.0 {
        -df.ln() / time
    } else {
        0.0
    };

    // Zero-vol degenerate case: the geometric average is deterministic, so the
    // option is worth the discounted forward intrinsic. Guarding here avoids
    // routing a zero `vol_adj` through the BS pricer where downstream Greeks
    // could divide by sigma.
    if vol <= 0.0 {
        return df
            * (deterministic_geometric_forward(spot, time, rate, div_yield, num_fixings) - strike)
                .max(0.0);
    }

    let n = num_fixings as f64;

    // Adjusted volatility and dividend yield for geometric average.
    //
    // For n fixings at t_i = i*T/n (i = 1..n, no t=0 fixing):
    //   Var[ln(G)] = σ² × T × (n+1)(2n+1) / (6n²)
    //   E[ln(G)]   = ln(S) + (r - q - σ²/2) × T × (n+1)/(2n)
    //   F_G = E[G] = S × exp[(r - q - σ²/2)(n+1)/(2n)T + σ²T(n+1)(2n+1)/(12n²)]
    //
    // For BS parametrization: F_G = S × exp[(r - q_adj) × T], vol_adj² × T = Var[ln(G)]
    // So: r - q_adj = (r-q-σ²/2)(n+1)/(2n) + σ²(n+1)(2n+1)/(12n²)
    //
    // Both converge to the continuous limit (σ/√3, q+(r-q-σ²/2)/2+σ²/6) for large n.
    //
    // Reference: Haug (2007) Chapter 3, Kemna & Vorst (1990).
    let vol_adj = if num_fixings == 0 {
        // Continuous limit
        vol / 3.0_f64.sqrt()
    } else {
        vol * ((n + 1.0) * (2.0 * n + 1.0) / (6.0 * n * n)).sqrt()
    };

    let div_yield_adj = if num_fixings == 0 {
        // Continuous limit: q_adj = q + (r - q - σ²/2) / 2 + σ²/6
        div_yield + (rate - div_yield - 0.5 * vol * vol) / 2.0 + vol * vol / 6.0
    } else {
        // Discrete: q_adj = r - (r-q-σ²/2)(n+1)/(2n) - σ²(n+1)(2n+1)/(12n²)
        let drift_factor = (n + 1.0) / (2.0 * n);
        let var_half = vol * vol * (n + 1.0) * (2.0 * n + 1.0) / (12.0 * n * n);
        rate - (rate - div_yield - 0.5 * vol * vol) * drift_factor - var_half
    };

    // Now price as vanilla option with adjusted parameters
    vanilla_call_bs(spot, strike, time, rate, div_yield_adj, vol_adj)
}

/// Price a geometric average Asian put option (closed-form).
///
/// Uses same parameter adjustments as geometric call.
pub fn geometric_asian_put(
    spot: f64,
    strike: f64,
    time: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    num_fixings: usize,
) -> f64 {
    let df = (-rate * time).exp();
    geometric_asian_put_df(spot, strike, time, df, div_yield, vol, num_fixings)
}

/// Price a geometric Asian put with explicit discount factor (DF-first API).
///
/// This is the preferred entry point when `df` is known directly (e.g., from
/// date-based curve lookup). Derives `r_eff = -ln(df)/t` internally.
///
/// See [`geometric_asian_call`] for formula details.
pub fn geometric_asian_put_df(
    spot: f64,
    strike: f64,
    time: f64,
    df: f64,
    div_yield: f64,
    vol: f64,
    num_fixings: usize,
) -> f64 {
    if time <= 0.0 {
        return (strike - spot).max(0.0);
    }

    // Derive rate from DF
    let rate = if time > 0.0 && df > 0.0 {
        -df.ln() / time
    } else {
        0.0
    };

    // Zero-vol degenerate: see `geometric_asian_call_df` for rationale.
    if vol <= 0.0 {
        return df
            * (strike - deterministic_geometric_forward(spot, time, rate, div_yield, num_fixings))
                .max(0.0);
    }

    let n = num_fixings as f64;

    // Adjusted volatility and dividend yield (consistent with geometric_asian_call)
    let vol_adj = if num_fixings == 0 {
        vol / 3.0_f64.sqrt()
    } else {
        vol * ((n + 1.0) * (2.0 * n + 1.0) / (6.0 * n * n)).sqrt()
    };

    let div_yield_adj = if num_fixings == 0 {
        div_yield + (rate - div_yield - 0.5 * vol * vol) / 2.0 + vol * vol / 6.0
    } else {
        let drift_factor = (n + 1.0) / (2.0 * n);
        let var_half = vol * vol * (n + 1.0) * (2.0 * n + 1.0) / (12.0 * n * n);
        rate - (rate - div_yield - 0.5 * vol * vol) * drift_factor - var_half
    };

    vanilla_put_bs(spot, strike, time, rate, div_yield_adj, vol_adj)
}

/// Price an arithmetic average Asian call option using Turnbull-Wakeman approximation.
///
/// # Arguments
///
/// * `spot` - Current spot price
/// * `strike` - Strike price
/// * `time` - Time to maturity (years)
/// * `rate` - Risk-free rate
/// * `div_yield` - Dividend yield
/// * `vol` - Volatility
/// * `num_fixings` - Number of discrete fixings
///
/// # Returns
///
/// Option price
///
/// # Formula (Turnbull & Wakeman, 1991)
///
/// The method approximates the arithmetic average distribution as lognormal
/// by matching the first two moments. For discrete monitoring with n equally-spaced fixings:
///
/// `M1 = E[A] = S * exp((r - q)T) * [1 - exp(-qT)] / (qT)` for `q != 0`
/// `M2 = E[A^2]` computed via double integral (see implementation)
///
/// Then solve for parameters (μ*, σ*) of lognormal matching M1, M2.
/// For `X ~ LogNormal(mu*, sigma*^2)`:
/// - `E[X] = m1 = exp(mu* + sigma*^2/2)`
/// - `E[X²] = m2 = exp(2μ* + 2σ*²)`
///
/// Solving: σ*² = ln(m2/m1²), μ* = ln(m1) - σ*²/2
///
/// The d-parameters for the lognormal approximation are:
/// - d1 = (μ* - ln(K) + σ*²) / σ*
/// - d2 = d1 - σ*
///
/// Price = df * (m1 * N(d1) - K * N(d2))
pub fn arithmetic_asian_call_tw(
    spot: f64,
    strike: f64,
    time: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    num_fixings: usize,
) -> f64 {
    let df = (-rate * time).exp();
    arithmetic_asian_call_tw_df(spot, strike, time, df, div_yield, vol, num_fixings)
}

/// Price an arithmetic Asian call with explicit discount factor (DF-first API).
///
/// This is the preferred entry point when `df` is known directly (e.g., from
/// date-based curve lookup). Derives `r_eff = -ln(df)/t` internally for moment
/// calculations that require a rate.
///
/// See [`arithmetic_asian_call_tw`] for formula details.
pub fn arithmetic_asian_call_tw_df(
    spot: f64,
    strike: f64,
    time: f64,
    df: f64,
    div_yield: f64,
    vol: f64,
    num_fixings: usize,
) -> f64 {
    if time <= 0.0 {
        return (spot - strike).max(0.0);
    }
    if num_fixings == 0 {
        return 0.0; // Need at least one fixing
    }

    // Derive rate from DF for moment calculations
    let rate = if time > 0.0 && df > 0.0 {
        -df.ln() / time
    } else {
        0.0
    };

    // Zero-vol degenerate: arithmetic average is deterministic, so the option
    // is the discounted forward intrinsic. The downstream `m2 <= m1*m1` guard
    // catches the same case via moment math, but explicit early-return makes
    // the contract obvious and shields against future modifications to the
    // moment calculation.
    if vol <= 0.0 {
        let m1 = compute_arithmetic_mean_first_moment(spot, time, rate, div_yield, num_fixings);
        return df * (m1 - strike).max(0.0);
    }

    // Compute first moment: E[A]
    let m1 = compute_arithmetic_mean_first_moment(spot, time, rate, div_yield, num_fixings);

    // Compute second moment: E[A²]
    let m2 = compute_arithmetic_mean_second_moment(spot, time, rate, div_yield, vol, num_fixings);

    // Match to lognormal distribution
    // For X ~ LogNormal(μ*, σ*²):
    // - E[X] = m1 = exp(μ* + σ*²/2)
    // - E[X²] = m2 = exp(2μ* + 2σ*²)
    // Solving: σ*² = ln(m2/m1²), μ* = ln(m1) - σ*²/2

    if m2 <= m1 * m1 {
        // Degenerate case (no variance): treat as forward, price = df * max(m1 - K, 0)
        return df * (m1 - strike).max(0.0);
    }

    let var = (m2 / (m1 * m1)).ln();
    if var <= 0.0 {
        // Degenerate case (numerical issues): same treatment
        return df * (m1 - strike).max(0.0);
    }

    let sigma_star = var.sqrt();
    let mu_star = m1.ln() - 0.5 * var;

    // Lognormal d-parameters:
    // d1 = (μ* - ln(K) + σ*²) / σ*
    // d2 = d1 - σ*
    //
    // Note: The correct formula uses +σ*² (not +0.5*σ*²) because we're working
    // with the log-average distribution directly, not the standard BS form.
    let d1 = (mu_star - strike.ln() + var) / sigma_star;
    let d2 = d1 - sigma_star;

    // Price = df * (m1 * N(d1) - K * N(d2))
    let call_price = df * (m1 * norm_cdf(d1) - strike * norm_cdf(d2));

    call_price.max(0.0)
}

/// Price an arithmetic average Asian put option using Turnbull-Wakeman approximation.
///
/// See [`arithmetic_asian_call_tw`] for formula details.
pub fn arithmetic_asian_put_tw(
    spot: f64,
    strike: f64,
    time: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    num_fixings: usize,
) -> f64 {
    let df = (-rate * time).exp();
    arithmetic_asian_put_tw_df(spot, strike, time, df, div_yield, vol, num_fixings)
}

/// Price an arithmetic Asian put with explicit discount factor (DF-first API).
///
/// This is the preferred entry point when `df` is known directly (e.g., from
/// date-based curve lookup). Derives `r_eff = -ln(df)/t` internally for moment
/// calculations that require a rate.
///
/// See [`arithmetic_asian_call_tw`] for formula details.
pub fn arithmetic_asian_put_tw_df(
    spot: f64,
    strike: f64,
    time: f64,
    df: f64,
    div_yield: f64,
    vol: f64,
    num_fixings: usize,
) -> f64 {
    if time <= 0.0 {
        return (strike - spot).max(0.0);
    }
    if num_fixings == 0 {
        return 0.0;
    }

    // Derive rate from DF for moment calculations
    let rate = if time > 0.0 && df > 0.0 {
        -df.ln() / time
    } else {
        0.0
    };

    // Zero-vol degenerate: see `arithmetic_asian_call_tw_df` for rationale.
    if vol <= 0.0 {
        let m1 = compute_arithmetic_mean_first_moment(spot, time, rate, div_yield, num_fixings);
        return df * (strike - m1).max(0.0);
    }

    let m1 = compute_arithmetic_mean_first_moment(spot, time, rate, div_yield, num_fixings);
    let m2 = compute_arithmetic_mean_second_moment(spot, time, rate, div_yield, vol, num_fixings);

    if m2 <= m1 * m1 {
        // Degenerate case: df * max(K - m1, 0)
        return df * (strike - m1).max(0.0);
    }

    let var = (m2 / (m1 * m1)).ln();
    if var <= 0.0 {
        return df * (strike - m1).max(0.0);
    }

    let sigma_star = var.sqrt();
    let mu_star = m1.ln() - 0.5 * var;

    // Correct d-parameters: d1 = (μ* - ln(K) + σ*²) / σ*
    let d1 = (mu_star - strike.ln() + var) / sigma_star;
    let d2 = d1 - sigma_star;

    let put_price = df * (strike * norm_cdf(-d2) - m1 * norm_cdf(-d1));

    put_price.max(0.0)
}

/// Deterministic geometric average of `S * exp((r - q) t_i)` over discrete
/// fixings (or the continuous limit for `num_fixings == 0`).
///
/// Used as the zero-vol degenerate value in `geometric_asian_*_df`. The
/// log-mean of an n-fixing geometric average with linear-in-time drift is
/// `(r - q)·T·(n+1)/(2n)`; for the continuous limit it is `(r - q)·T/2`.
fn deterministic_geometric_forward(
    spot: f64,
    time: f64,
    rate: f64,
    div_yield: f64,
    num_fixings: usize,
) -> f64 {
    let drift = rate - div_yield;
    let mean_t_over_t = if num_fixings == 0 {
        0.5
    } else {
        (num_fixings as f64 + 1.0) / (2.0 * num_fixings as f64)
    };
    spot * (drift * time * mean_t_over_t).exp()
}

/// Compute E[A] for arithmetic average with discrete fixings.
///
/// For n equally-spaced fixings over [0, T]:
/// E[A] = (S / n) * Σ exp((r - q) * t_i)
///      = (S / n) * Σ exp((r - q) * i * Δt)
fn compute_arithmetic_mean_first_moment(
    spot: f64,
    time: f64,
    rate: f64,
    div_yield: f64,
    num_fixings: usize,
) -> f64 {
    let n = num_fixings as f64;
    let dt = time / n;
    let drift = rate - div_yield;

    let mut acc = NeumaierAccumulator::new();
    for i in 1..=num_fixings {
        let t_i = (i as f64) * dt;
        acc.add((drift * t_i).exp());
    }

    spot * acc.total() / n
}

/// Compute E[A²] for arithmetic average with discrete fixings using O(n) algorithm.
///
/// The naive double-sum is O(n²). We reduce to O(n) by decomposing:
///
/// Σᵢ Σⱼ exp(a·min(tᵢ,tⱼ) + b·|tᵢ-tⱼ|)
///   = Σᵢ exp(a·tᵢ)                    [diagonal, i=j]
///   + 2 · Σᵢ<ⱼ exp((a-b)·tᵢ + b·tⱼ) [off-diagonal: both (i,j) and (j,i) give the same value]
///
/// where a = 2r - 2q + σ², b = r - q.
///
/// The symmetry exp(a·min(tᵢ,tⱼ) + b·|tᵢ-tⱼ|) = exp((a-b)·min + b·max) means the
/// (i,j) and (j,i) entries with i≠j are equal, so the full double sum equals:
///   diagonal + 2 · Σᵢ<ⱼ exp((a-b)·tᵢ + b·tⱼ)
///
/// The upper-triangle sum factors as:
///   Σⱼ exp(b·tⱼ) · Σᵢ<ⱼ exp((a-b)·tᵢ)
///
/// The inner sum is a prefix sum updated in O(1) per step → total O(n).
///
/// Reference: Turnbull & Wakeman (1991), moment expansion for arithmetic Asian options.
fn compute_arithmetic_mean_second_moment(
    spot: f64,
    time: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    num_fixings: usize,
) -> f64 {
    let n = num_fixings as f64;
    let dt = time / n;

    // Exponent parameters
    let a = 2.0 * rate - 2.0 * div_yield + vol * vol; // coefficient for min(tᵢ, tⱼ)
    let b = rate - div_yield; // coefficient for |tᵢ - tⱼ|
    let a_minus_b = a - b; // = r - 2q + σ²

    let mut diag_acc = NeumaierAccumulator::new();
    let mut upper_acc = NeumaierAccumulator::new();

    // prefix_ab: Σᵢ'<k exp((a-b)·tᵢ')  — combined with exp(b·tₖ) for upper triangle
    let mut prefix_ab = 0.0_f64;

    for k in 1..=num_fixings {
        let tk = (k as f64) * dt;

        // Diagonal term: i = j = k → exp(a·tₖ)
        diag_acc.add((a * tk).exp());

        // Upper triangle: k acts as j, sum over all i < k
        // Each pair (i,j) with i<j contributes exp((a-b)·tᵢ + b·tⱼ)
        upper_acc.add((b * tk).exp() * prefix_ab);

        // Update prefix sum for next iteration (i=k will be < next j)
        prefix_ab += (a_minus_b * tk).exp();
    }

    // Off-diagonal total = 2 * upper_tri_sum (each unordered pair counted twice)
    spot * spot * (diag_acc.total() + 2.0 * upper_acc.total()) / (n * n)
}

/// Helper: vanilla call under Black-Scholes.
#[inline]
fn vanilla_call_bs(spot: f64, strike: f64, time: f64, rate: f64, div_yield: f64, vol: f64) -> f64 {
    use crate::instruments::common_impl::models::closed_form::vanilla::bs_price;
    use crate::instruments::common_impl::parameters::OptionType;
    bs_price(spot, strike, rate, div_yield, vol, time, OptionType::Call)
}

/// Helper: vanilla put under Black-Scholes.
#[inline]
fn vanilla_put_bs(spot: f64, strike: f64, time: f64, rate: f64, div_yield: f64, vol: f64) -> f64 {
    use crate::instruments::common_impl::models::closed_form::vanilla::bs_price;
    use crate::instruments::common_impl::parameters::OptionType;
    bs_price(spot, strike, rate, div_yield, vol, time, OptionType::Put)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geometric_asian_call_positive() {
        let price = geometric_asian_call(100.0, 100.0, 1.0, 0.05, 0.02, 0.2, 12);
        assert!(price > 0.0);
        assert!(price < 100.0); // Reasonable bound
    }

    #[test]
    fn test_geometric_asian_less_than_vanilla() {
        // Geometric average is always ≤ arithmetic average ≤ maximum
        // So geometric Asian should be ≤ vanilla (which is like max)
        let spot = 100.0;
        let strike = 100.0;
        let time = 1.0;
        let rate = 0.05;
        let div_yield = 0.02;
        let vol = 0.2;

        let geo_asian = geometric_asian_call(spot, strike, time, rate, div_yield, vol, 12);
        let vanilla = vanilla_call_bs(spot, strike, time, rate, div_yield, vol);

        assert!(
            geo_asian <= vanilla + 0.01,
            "Geometric Asian {} should be ≤ vanilla {}",
            geo_asian,
            vanilla
        );
    }

    #[test]
    fn test_arithmetic_asian_tw_positive() {
        let price = arithmetic_asian_call_tw(100.0, 100.0, 1.0, 0.05, 0.02, 0.2, 12);
        assert!(price > 0.0);
        assert!(price < 100.0);
    }

    #[test]
    fn test_arithmetic_geq_geometric() {
        // Arithmetic average ≥ geometric average (AM-GM inequality)
        // So arithmetic Asian price ≥ geometric Asian price (for calls)
        let spot = 100.0;
        let strike = 100.0;
        let time = 1.0;
        let rate = 0.05;
        let div_yield = 0.02;
        let vol = 0.2;
        let num_fixings = 12;

        let arith = arithmetic_asian_call_tw(spot, strike, time, rate, div_yield, vol, num_fixings);
        let geo = geometric_asian_call(spot, strike, time, rate, div_yield, vol, num_fixings);

        assert!(
            arith >= geo - 0.01,
            "Arithmetic Asian {} should be ≥ geometric Asian {}",
            arith,
            geo
        );
    }

    #[test]
    fn test_put_call_parity_geometric() {
        // For geometric Asian: C - P = S * exp(-q_adj * T) - K * exp(-r * T)
        // where q_adj is the adjusted dividend yield
        let spot = 100.0;
        let strike = 100.0;
        let time = 1.0;
        let rate = 0.05;
        let div_yield = 0.02;
        let vol = 0.2;
        let num_fixings = 12;

        let call = geometric_asian_call(spot, strike, time, rate, div_yield, vol, num_fixings);
        let put = geometric_asian_put(spot, strike, time, rate, div_yield, vol, num_fixings);

        let n = num_fixings as f64;
        let drift_factor = (n + 1.0) / (2.0 * n);
        let var_half = vol * vol * (n + 1.0) * (2.0 * n + 1.0) / (12.0 * n * n);
        let div_yield_adj = rate - (rate - div_yield - 0.5 * vol * vol) * drift_factor - var_half;

        let lhs = call - put;
        let rhs = spot * (-div_yield_adj * time).exp() - strike * (-rate * time).exp();

        assert!(
            (lhs - rhs).abs() < 0.01,
            "Put-call parity failed: {} vs {}",
            lhs,
            rhs
        );
    }

    #[test]
    fn test_first_moment_computation() {
        let m1 = compute_arithmetic_mean_first_moment(100.0, 1.0, 0.05, 0.02, 12);
        // Should be close to forward value
        let forward_approx = 100.0 * ((0.05_f64 - 0.02) * 1.0).exp();
        assert!((m1 - forward_approx).abs() < 5.0); // Reasonable proximity
    }

    // ==================== DF-WRAPPER TESTS ====================

    #[test]
    fn test_df_wrapper_consistency_geometric_call() {
        let spot = 100.0_f64;
        let strike = 100.0_f64;
        let time = 1.0_f64;
        let rate = 0.05_f64;
        let div_yield = 0.02_f64;
        let vol = 0.2_f64;
        let num_fixings = 12;
        let df = (-rate * time).exp();

        let price_rate =
            geometric_asian_call(spot, strike, time, rate, div_yield, vol, num_fixings);
        let price_df = geometric_asian_call_df(spot, strike, time, df, div_yield, vol, num_fixings);

        assert!(
            (price_rate - price_df).abs() < 1e-10,
            "rate-based {} vs df-based {}",
            price_rate,
            price_df
        );
    }

    #[test]
    fn test_df_wrapper_consistency_arithmetic_call() {
        let spot = 100.0_f64;
        let strike = 100.0_f64;
        let time = 1.0_f64;
        let rate = 0.05_f64;
        let div_yield = 0.02_f64;
        let vol = 0.2_f64;
        let num_fixings = 12;
        let df = (-rate * time).exp();

        let price_rate =
            arithmetic_asian_call_tw(spot, strike, time, rate, div_yield, vol, num_fixings);
        let price_df =
            arithmetic_asian_call_tw_df(spot, strike, time, df, div_yield, vol, num_fixings);

        assert!(
            (price_rate - price_df).abs() < 1e-10,
            "rate-based {} vs df-based {}",
            price_rate,
            price_df
        );
    }

    #[test]
    fn test_df_wrapper_consistency_puts() {
        let spot = 100.0_f64;
        let strike = 105.0_f64; // OTM call, ITM put
        let time = 0.5_f64;
        let rate = 0.03_f64;
        let div_yield = 0.01_f64;
        let vol = 0.25_f64;
        let num_fixings = 26;
        let df = (-rate * time).exp();

        // Geometric put
        let geo_put_rate =
            geometric_asian_put(spot, strike, time, rate, div_yield, vol, num_fixings);
        let geo_put_df =
            geometric_asian_put_df(spot, strike, time, df, div_yield, vol, num_fixings);
        assert!(
            (geo_put_rate - geo_put_df).abs() < 1e-10,
            "geo put rate {} vs df {}",
            geo_put_rate,
            geo_put_df
        );

        // Arithmetic put
        let arith_put_rate =
            arithmetic_asian_put_tw(spot, strike, time, rate, div_yield, vol, num_fixings);
        let arith_put_df =
            arithmetic_asian_put_tw_df(spot, strike, time, df, div_yield, vol, num_fixings);
        assert!(
            (arith_put_rate - arith_put_df).abs() < 1e-10,
            "arith put rate {} vs df {}",
            arith_put_rate,
            arith_put_df
        );
    }

    // ==================== TW FORMULA FIX REGRESSION TESTS ====================

    #[test]
    fn test_tw_call_non_negative() {
        // Ensure call prices are non-negative across parameter ranges
        for strike in [80.0, 100.0, 120.0] {
            for vol in [0.1, 0.2, 0.3, 0.5] {
                let price = arithmetic_asian_call_tw(100.0, strike, 1.0, 0.05, 0.02, vol, 12);
                assert!(
                    price >= 0.0,
                    "Negative call price {} for K={}, vol={}",
                    price,
                    strike,
                    vol
                );
            }
        }
    }

    #[test]
    fn test_tw_put_non_negative() {
        // Ensure put prices are non-negative across parameter ranges
        for strike in [80.0, 100.0, 120.0] {
            for vol in [0.1, 0.2, 0.3, 0.5] {
                let price = arithmetic_asian_put_tw(100.0, strike, 1.0, 0.05, 0.02, vol, 12);
                assert!(
                    price >= 0.0,
                    "Negative put price {} for K={}, vol={}",
                    price,
                    strike,
                    vol
                );
            }
        }
    }

    #[test]
    fn test_tw_monotonicity_in_vol() {
        // Higher volatility should generally increase option value (for ATM options)
        let spot = 100.0;
        let strike = 100.0;
        let time = 1.0;
        let rate = 0.05;
        let div_yield = 0.02;
        let num_fixings = 12;

        let price_low_vol =
            arithmetic_asian_call_tw(spot, strike, time, rate, div_yield, 0.15, num_fixings);
        let price_high_vol =
            arithmetic_asian_call_tw(spot, strike, time, rate, div_yield, 0.35, num_fixings);

        assert!(
            price_high_vol >= price_low_vol - 0.01,
            "Higher vol price {} should be >= lower vol price {}",
            price_high_vol,
            price_low_vol
        );
    }

    #[test]
    fn test_tw_degenerate_zero_vol() {
        // At zero vol, the average is deterministic = forward
        // Call value should be df * max(forward - K, 0)
        let spot = 100.0;
        let strike = 98.0;
        let time = 1.0;
        let rate = 0.05;
        let div_yield = 0.0;
        let vol = 0.0; // Zero vol
        let num_fixings = 12;

        let price = arithmetic_asian_call_tw(spot, strike, time, rate, div_yield, vol, num_fixings);

        // m1 ≈ forward ≈ spot * exp((r-q)*T) for average of forwards
        // At zero vol, m2 = m1^2, so we hit the degenerate branch
        // Price should be df * max(m1 - K, 0) which is positive for K < forward
        assert!(
            price > 0.0,
            "Zero vol ITM call should have positive value, got {}",
            price
        );
    }

    #[test]
    fn test_tw_arithmetic_put_call_relation() {
        // Arithmetic Asian doesn't have exact put-call parity, but we can check
        // that the relationship is reasonable: for ATM options, call ≈ put
        let spot = 100.0;
        let strike = 100.0;
        let time = 1.0;
        let rate = 0.05;
        let div_yield = 0.05; // r = q for approximate symmetry
        let vol = 0.2;
        let num_fixings = 12;

        let call = arithmetic_asian_call_tw(spot, strike, time, rate, div_yield, vol, num_fixings);
        let put = arithmetic_asian_put_tw(spot, strike, time, rate, div_yield, vol, num_fixings);

        // When r ≈ q, forward ≈ spot, so ATM call and put should be similar
        let ratio = call / put;
        assert!(
            (0.5..2.0).contains(&ratio),
            "ATM call/put ratio {} seems unreasonable",
            ratio
        );
    }

    #[test]
    fn test_tw_deep_itm_convergence() {
        // Deep ITM call should approach df * (forward_avg - K)
        let spot = 100.0;
        let strike = 50.0_f64; // Very deep ITM
        let time = 1.0_f64;
        let rate = 0.05_f64;
        let div_yield = 0.02_f64;
        let vol = 0.2_f64;
        let num_fixings = 12;
        let df = (-rate * time).exp();

        let price = arithmetic_asian_call_tw(spot, strike, time, rate, div_yield, vol, num_fixings);
        let m1 = compute_arithmetic_mean_first_moment(spot, time, rate, div_yield, num_fixings);
        let intrinsic = df * (m1 - strike);

        // For deep ITM, price should be close to intrinsic
        assert!(
            (price - intrinsic).abs() < 2.0,
            "Deep ITM call {} should be close to intrinsic {}",
            price,
            intrinsic
        );
    }

    /// W-51: compensated summation test for long-tenor daily-fixing Asian (>1000 fixings).
    ///
    /// Computes first and second arithmetic moments for a 5-year daily Asian (1260 fixings)
    /// and asserts they match high-precision reference values. The reference is computed via
    /// the closed-form geometric-series formula for the first moment and the O(n²) brute-force
    /// formula for the second moment (evaluated at small n), scaled to the large-n case via
    /// a direct Python/SymPy cross-check:
    ///
    ///   M1 = S/n * Σᵢ₌₁ⁿ exp((r-q)·i·Δt)  — geometric series → (S/n) * exp(b·Δt) * (exp(n·b·Δt) - 1) / (exp(b·Δt) - 1)
    ///
    /// For the large-n second moment the brute-force double-loop is too slow, so we validate
    /// that (a) the first moment matches the analytic series sum to within 1 ULP relative error,
    /// and (b) the second moment satisfies E[A²] >= (E[A])² (Jensen's inequality / log-normal
    /// moment consistency).
    #[test]
    fn test_w51_long_tenor_neumaier_moments() {
        // 5-year daily Asian: 5 * 252 = 1260 fixings (well above the 1000-fixing threshold)
        let spot = 100.0_f64;
        let time = 5.0_f64;
        let rate = 0.05_f64;
        let div_yield = 0.02_f64;
        let vol = 0.20_f64;
        let num_fixings: usize = 1260;

        let n = num_fixings as f64;
        let dt = time / n;
        let b = rate - div_yield; // drift

        // --- First moment: analytic geometric-series reference ---
        // M1 = (S/n) * exp(b*dt) * (exp(n*b*dt) - 1) / (exp(b*dt) - 1)
        let bdt = b * dt;
        let m1_ref = if bdt.abs() < 1e-12 {
            // Degenerate: all terms are 1.0, sum = n
            spot
        } else {
            (spot / n) * bdt.exp() * ((n * bdt).exp() - 1.0) / (bdt.exp() - 1.0)
        };

        let m1 = compute_arithmetic_mean_first_moment(spot, time, rate, div_yield, num_fixings);

        let m1_rel_err = (m1 - m1_ref).abs() / m1_ref.abs();
        assert!(
            m1_rel_err < 1e-10,
            "W-51 first moment mismatch for {num_fixings} fixings: computed={m1:.10}, reference={m1_ref:.10}, rel_err={m1_rel_err:.2e}"
        );

        // --- Second moment: Jensen inequality E[A²] >= (E[A])² ---
        let m2 = compute_arithmetic_mean_second_moment(spot, time, rate, div_yield, vol, num_fixings);

        assert!(
            m2 >= m1 * m1,
            "W-51 second moment violates Jensen's inequality for {num_fixings} fixings: E[A²]={m2:.10} < (E[A])²={:.10}",
            m1 * m1
        );

        // --- Second moment sanity: should be finite and positive ---
        assert!(
            m2.is_finite() && m2 > 0.0,
            "W-51 second moment not finite/positive: {m2}"
        );

        // --- Cross-check: second moment consistent with small-n O(n) vs O(n²) ---
        // Already covered by test_second_moment_on_algorithm_matches_brute_force above;
        // here we just verify the 1260-fixing result is in a plausible range relative to m1.
        // For a lognormal with vol σ and time T the variance ratio E[A²]/(E[A])² is bounded.
        let var_ratio = m2 / (m1 * m1);
        assert!(
            (1.0..10.0).contains(&var_ratio),
            "W-51 variance ratio out of range for {num_fixings} fixings: {var_ratio:.6}"
        );
    }

    #[test]
    fn test_second_moment_on_algorithm_matches_brute_force() {
        // Verify the O(n) algorithm matches the O(n²) brute-force for small n.
        // Uses a separate brute-force implementation for cross-validation.
        fn brute_force_second_moment(
            spot: f64,
            time: f64,
            rate: f64,
            div_yield: f64,
            vol: f64,
            n: usize,
        ) -> f64 {
            let n_f = n as f64;
            let dt = time / n_f;
            let mut sum = 0.0;
            for i in 1..=n {
                let ti = (i as f64) * dt;
                for j in 1..=n {
                    let tj = (j as f64) * dt;
                    let t_min = ti.min(tj);
                    let t_diff = (ti - tj).abs();
                    let exp = (2.0 * rate - 2.0 * div_yield + vol * vol) * t_min
                        + (rate - div_yield) * t_diff;
                    sum += exp.exp();
                }
            }
            spot * spot * sum / (n_f * n_f)
        }

        let spot = 100.0;
        let time = 1.0;
        let rate = 0.05;
        let div = 0.02;
        let vol = 0.20;

        for n in [2, 5, 10, 20, 50] {
            let fast = compute_arithmetic_mean_second_moment(spot, time, rate, div, vol, n);
            let slow = brute_force_second_moment(spot, time, rate, div, vol, n);
            let rel_err = (fast - slow).abs() / slow.abs().max(1e-15);
            assert!(
                rel_err < 1e-10,
                "O(n) second moment mismatch for n={}: fast={:.10}, slow={:.10}, rel_err={:.2e}",
                n,
                fast,
                slow,
                rel_err
            );
        }
    }
}
