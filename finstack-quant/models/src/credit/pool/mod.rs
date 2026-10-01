//! Product-independent stochastic models for structured-credit collateral pools.
//!
//! This module owns default, prepayment, and correlation engines and their
//! serializable specifications. Deal construction, calibration presets,
//! waterfalls, and tranche pricing remain in `finstack-quant-valuations`.

pub mod correlation;
pub mod default;
pub mod prepayment;

pub use correlation::CorrelationStructure;
pub use default::{
    MacroCreditFactors, PerNameCopulaDefault, PoolGranularity, StochasticDefault,
    StochasticDefaultSpec,
};
pub use prepayment::{RichardRollPrepay, StochasticPrepaySpec, StochasticPrepayment};

fn clamped_cdr_to_mdr(cdr: f64) -> f64 {
    finstack_quant_cashflows::builder::cdr_to_mdr(cdr.clamp(0.0, 1.0)).unwrap_or(f64::NAN)
}

fn clamped_cpr_to_smm(cpr: f64) -> f64 {
    finstack_quant_cashflows::builder::cpr_to_smm(cpr.clamp(0.0, 1.0)).unwrap_or(f64::NAN)
}

/// Mean monthly prepayment after a lognormal CPR shock and the annual-rate cap.
///
/// Integrate `P(SMM > s)` in log-SMM space. This retains precision for small
/// base CPRs and avoids the fractional-power cusp where CPR reaches one.
/// Bounds at ten normal standard deviations omit less than `2e-23` probability.
///
/// # Arguments
///
/// * `base_cpr` - Annual CPR before the shock, in decimal form.
/// * `shock_volatility` - Standard deviation of the log-CPR shock (`|loading * vol|`),
///   in `[0, 1]` under the pool models' parameter bounds; the sign is immaterial.
fn expected_shocked_smm(base_cpr: f64, shock_volatility: f64) -> f64 {
    use finstack_quant_core::math::{gauss_legendre_integrate_composite, norm_cdf};

    if base_cpr <= 0.0 {
        return 0.0;
    }
    let sigma = shock_volatility.abs();
    if sigma == 0.0 {
        return clamped_cpr_to_smm(base_cpr);
    }
    if !base_cpr.is_finite() || !sigma.is_finite() {
        return f64::NAN;
    }

    let lower_smm = clamped_cpr_to_smm(base_cpr * (-10.0 * sigma).exp());
    let upper_smm = clamped_cpr_to_smm(base_cpr * (10.0 * sigma).exp());
    if upper_smm == 0.0 || lower_smm >= upper_smm {
        return upper_smm;
    }
    let log_base = base_cpr.ln();
    let lower_log = if lower_smm > 0.0 {
        lower_smm.ln()
    } else {
        // At tiny CPR, SMM = CPR / 12 to machine precision. Keeping this
        // bound in log space avoids an underflowed integration endpoint.
        log_base - 10.0 * sigma - 12.0_f64.ln()
    };
    let integral = gauss_legendre_integrate_composite(
        |log_smm| {
            let smm = log_smm.exp();
            let cpr = -(12.0 * (-smm).ln_1p()).exp_m1();
            let survival = norm_cdf((log_base - cpr.ln()) / sigma);
            survival * smm
        },
        lower_log,
        upper_smm.ln(),
        16,
        4,
    );
    // Finite bounds and fixed supported quadrature settings make failure an
    // internal numerical error; preserve it as NaN, never as a baseline rate.
    lower_smm + integral.unwrap_or(f64::NAN)
}

#[cfg(test)]
mod tests {
    use super::{clamped_cpr_to_smm, expected_shocked_smm};

    #[test]
    fn shocked_smm_expectation_preserves_deterministic_limits() {
        for base_cpr in [0.0, 0.001, 0.20, 1.0] {
            assert!(
                (expected_shocked_smm(base_cpr, 0.0) - clamped_cpr_to_smm(base_cpr)).abs() < 1e-14
            );
        }
        assert_eq!(expected_shocked_smm(0.0, 1.0), 0.0);
    }

    #[test]
    fn shocked_smm_expectation_covers_small_rates_and_the_cpr_cap() {
        for (base_cpr, sigma, mean) in [
            (0.001, 1.0, 0.000137677916434),
            (0.20, 1.0, 0.08176149769578),
            (0.80, 0.4, 0.36629112977692),
            (0.999, 0.001, 0.52684458761249),
        ] {
            assert!((expected_shocked_smm(base_cpr, sigma) - mean).abs() < 1e-8);
            assert!((expected_shocked_smm(base_cpr, -sigma) - mean).abs() < 1e-8);
        }
    }
}
