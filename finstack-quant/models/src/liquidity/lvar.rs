//! Liquidity-adjusted Value at Risk (LVaR) calculator.
//!
//! Composes with existing VaR numbers (from `factor_model/` or external sources)
//! to produce liquidity-adjusted figures following Bangia et al. (1999).
//!
//! # Sign convention
//!
//! VaR and LVaR are expressed on the P&L axis: **losses are negative**. A valid
//! input `var` is therefore non-positive and all LVaR variants are likewise
//! non-positive. A "more conservative" LVaR is the more negative number.
//!
//! # References
//!
//! - Bangia, A., Diebold, F., Schuermann, T., Stroughair, J. (1999).
//!   "Modeling Liquidity Risk with Implications for Traditional Market
//!   Risk Measurement and Management." *Risk*, 12(1). `docs/REFERENCES.md#bangia-1999-lvar`
//!

use finstack_quant_core::Result;

use super::invalid_input;
use finstack_quant_core::math::special_functions::standard_normal_inv_cdf;
use serde::{Deserialize, Serialize};

fn validate_lvar_confidence(confidence: f64) -> Result<()> {
    if !confidence.is_finite() || confidence <= 0.5 || confidence >= 1.0 {
        return Err(invalid_input(
            "confidence must be finite and in the open interval (0.5, 1)",
        ));
    }
    Ok(())
}

/// Scalar Bangia LVaR outputs for an isolated position where relative spread
/// statistics are already known. Used by bindings that don't carry a full
/// `LiquidityProfile`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct LvarBangiaScalar {
    /// Input VaR (non-positive loss number), echoed back for convenience.
    pub var: f64,
    /// Non-negative magnitude of the Bangia spread-cost add-on.
    pub spread_cost: f64,
    /// Bangia-adjusted LVaR (non-positive loss number, `lvar <= var <= 0`).
    pub lvar: f64,
    /// Ratio `lvar / var`. `NaN` when `var == 0`.
    pub lvar_ratio: f64,
}

/// Bangia, Diebold, Schuermann & Stroughair (1999) LVaR from scalar spread
/// statistics, for an isolated position where the caller already has
/// `spread_mean` and `spread_vol` in relative (fraction-of-mid) terms.
///
/// Formula:
/// ```text
/// spread_cost = (0.5 * spread_mean + 0.5 * z_alpha * spread_vol) * |position_value|
/// lvar = var - spread_cost
/// ```
///
/// # Arguments
///
/// * `var` - Market VaR in the library's loss-negative sign convention.
/// * `spread_mean` - Mean relative bid-ask spread as a non-negative fraction
///   of mid price.
/// * `spread_vol` - Standard deviation of relative bid-ask spread as a
///   non-negative fraction of mid price.
/// * `confidence` - VaR confidence level, strictly inside `(0.5, 1)`. Values
///   at or below the median are rejected: `z_alpha` would be non-positive
///   there, turning the spread add-on into a spurious *reduction* of risk.
/// * `position_value` - Signed market value whose absolute magnitude scales
///   the expected liquidation cost.
///
/// # Errors
///
/// Returns `finstack_quant_core::Error::Validation` if `var` is positive or non-finite, if
/// `spread_mean` or `spread_vol` are negative or non-finite, if `confidence`
/// is outside the open interval `(0.5, 1)`, or if `position_value` is
/// non-finite.
///
/// # References
///
/// - Bangia et al. (1999). `docs/REFERENCES.md#bangia-1999-lvar`
pub fn lvar_bangia_scalar(
    var: f64,
    spread_mean: f64,
    spread_vol: f64,
    confidence: f64,
    position_value: f64,
) -> Result<LvarBangiaScalar> {
    if !var.is_finite() || var > 0.0 {
        return Err(invalid_input(format!(
            "var must be non-positive and finite (loss sign convention), got {var}"
        )));
    }
    if !spread_mean.is_finite() || spread_mean < 0.0 {
        return Err(invalid_input("spread_mean must be non-negative and finite"));
    }
    if !spread_vol.is_finite() || spread_vol < 0.0 {
        return Err(invalid_input("spread_vol must be non-negative and finite"));
    }
    validate_lvar_confidence(confidence)?;
    if !position_value.is_finite() {
        return Err(invalid_input("position_value must be finite"));
    }

    let pv = position_value.abs();
    let z_alpha = standard_normal_inv_cdf(confidence);
    let spread_cost = (0.5 * spread_mean + 0.5 * z_alpha * spread_vol) * pv;
    let lvar = var - spread_cost;
    let lvar_ratio = if var.abs() > 1e-15 {
        lvar / var
    } else {
        f64::NAN
    };

    Ok(LvarBangiaScalar {
        var,
        spread_cost,
        lvar,
        lvar_ratio,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mo12_scalar_rejects_sub_median_confidence() {
        let err = lvar_bangia_scalar(-1_000.0, 0.01, 0.001, 0.49, 100_000.0)
            .expect_err("MO-12: confidence below 0.5 must be rejected");

        assert!(
            err.to_string().contains("confidence"),
            "unexpected error: {err}"
        );
    }
}
