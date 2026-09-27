//! CDS Tranche specific parameters.

use finstack_quant_core::{dates::Date, money::Money, Error, Result};

/// CDS Tranche specific parameters.
///
/// Groups parameters specific to CDS tranches.
#[derive(Debug, Clone)]
pub struct CdsTrancheParams {
    /// Index name (e.g., "CDX.NA.IG", "iTraxx Europe")
    pub index_name: String,
    /// Index series
    pub series: u16,
    /// Attachment point as percentage
    pub attach_pct: f64,
    /// Detachment point as percentage
    pub detach_pct: f64,
    /// Notional amount
    pub notional: Money,
    /// Maturity date
    pub maturity: Date,
    /// Running coupon in basis points
    pub coupon_bp: f64,
    /// Realized (settled) loss as a decimal fraction of the original portfolio notional, in `[0.0, 1.0]`
    pub realized_loss: f64,
}

impl CdsTrancheParams {
    /// Create new CDS tranche parameters
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        index_name: impl Into<String>,
        series: u16,
        attach_pct: f64,
        detach_pct: f64,
        notional: Money,
        maturity: Date,
        coupon_bp: f64,
    ) -> Self {
        Self {
            index_name: index_name.into(),
            series,
            attach_pct,
            detach_pct,
            notional,
            maturity,
            coupon_bp,
            realized_loss: 0.0,
        }
    }

    /// Set the realized loss with validation.
    ///
    /// # Arguments
    ///
    /// * `loss` - Realized (settled) loss as a decimal fraction of the original portfolio notional.
    ///   Must be in range [0.0, 1.0].
    ///
    /// # Errors
    ///
    /// Returns an error if loss is outside the valid range [0.0, 1.0].
    pub fn with_realized_loss(mut self, loss: f64) -> Result<Self> {
        if !(0.0..=1.0).contains(&loss) {
            return Err(Error::Validation(format!(
                "realized_loss must be in [0.0, 1.0], got {}",
                loss
            )));
        }
        self.realized_loss = loss;
        Ok(self)
    }

    /// Create equity tranche parameters (0-3% typically)
    pub fn equity_tranche(
        index_name: impl Into<String>,
        series: u16,
        notional: Money,
        maturity: Date,
        coupon_bp: f64,
    ) -> Self {
        Self::new(index_name, series, 0.0, 3.0, notional, maturity, coupon_bp)
    }

    /// Create mezzanine tranche parameters (3-7% typically)
    pub fn mezzanine_tranche(
        index_name: impl Into<String>,
        series: u16,
        notional: Money,
        maturity: Date,
        coupon_bp: f64,
    ) -> Self {
        Self::new(index_name, series, 3.0, 7.0, notional, maturity, coupon_bp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use time::macros::date;

    #[test]
    fn test_tranche_helper_units_are_percent_points() {
        let params = CdsTrancheParams::equity_tranche(
            "CDX.NA.IG",
            42,
            Money::from((1_000_000_i64, Currency::USD)),
            date!(2029 - 12 - 20),
            100.0,
        );
        assert!((params.attach_pct - 0.0).abs() < 1e-12);
        assert!((params.detach_pct - 3.0).abs() < 1e-12);

        let mezz = CdsTrancheParams::mezzanine_tranche(
            "CDX.NA.IG",
            42,
            Money::from((1_000_000_i64, Currency::USD)),
            date!(2029 - 12 - 20),
            100.0,
        );
        assert!((mezz.attach_pct - 3.0).abs() < 1e-12);
        assert!((mezz.detach_pct - 7.0).abs() < 1e-12);
    }
}
