//! Exposure at Default (EAD) modeling.
//!
//! Provides Credit Conversion Factor (CCF) based EAD computation for
//! facilities with both drawn and undrawn components.
//!
//! # References
//!
//! - Basel Committee (2006). "International Convergence of Capital
//!   Measurement and Capital Standards" (Basel II), paragraphs 310-316.
//! - Jacobs, M. (2010). "An Empirical Study of Exposure at Default."
//!   Journal of Advanced Studies in Finance.

use crate::error::InputError;
use crate::Result;

/// Credit Conversion Factor (CCF) for off-balance-sheet exposures.
///
/// Represents the fraction of undrawn commitments expected to be drawn
/// at the time of default.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CreditConversionFactor {
    /// CCF value in \[0, 1\]. Basel IRB: typically 0.75 for revolvers.
    ccf: f64,
}

impl CreditConversionFactor {
    /// Create a CCF.
    ///
    /// # Errors
    ///
    /// Returns an error if `ccf` is not in \[0, 1\].
    pub fn new(ccf: f64) -> Result<Self> {
        if !(0.0..=1.0).contains(&ccf) {
            return Err(InputError::Invalid.into());
        }
        Ok(Self { ccf })
    }

    /// Basel IRB standardized CCF for revolving facilities (75%).
    pub fn basel_revolver() -> Self {
        Self { ccf: 0.75 }
    }

    /// Full draw assumption (100% CCF).
    pub fn full_draw() -> Self {
        Self { ccf: 1.0 }
    }

    /// CCF value.
    pub fn value(&self) -> f64 {
        self.ccf
    }
}

/// Exposure at Default calculator.
///
/// Computes EAD for facilities with both drawn and undrawn components:
///
/// ```text
/// EAD = drawn + undrawn * CCF
/// ```
///
/// For fully drawn term loans, set `undrawn = 0`.
///
/// Optionally supports Loan Equivalency (LEQ) estimation for revolving
/// facilities where the CCF varies with utilization.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct EadCalculator {
    /// Currently drawn amount.
    drawn: f64,
    /// Undrawn (available) commitment.
    undrawn: f64,
    /// Credit conversion factor for the undrawn portion.
    ccf: CreditConversionFactor,
}

impl EadCalculator {
    /// Create an EAD calculator.
    ///
    /// # Errors
    ///
    /// Returns an error if `drawn < 0` or `undrawn < 0`.
    pub fn new(drawn: f64, undrawn: f64, ccf: CreditConversionFactor) -> Result<Self> {
        if drawn < 0.0 || undrawn < 0.0 {
            return Err(InputError::NegativeValue.into());
        }
        Ok(Self {
            drawn,
            undrawn,
            ccf,
        })
    }

    /// Create for a fully drawn term loan (no undrawn component).
    pub fn term_loan(drawn: f64) -> Result<Self> {
        Self::new(drawn, 0.0, CreditConversionFactor::full_draw())
    }

    /// Create for a revolving facility with Basel standard CCF.
    pub fn revolver(drawn: f64, undrawn: f64) -> Result<Self> {
        Self::new(drawn, undrawn, CreditConversionFactor::basel_revolver())
    }

    /// Compute exposure at default.
    pub fn ead(&self) -> f64 {
        self.drawn + self.undrawn * self.ccf.value()
    }

    /// Compute Loan Equivalency (LEQ) given observed EAD at default.
    ///
    /// LEQ = (EAD_observed - Drawn) / Undrawn
    ///
    /// This is the ex-post CCF estimated from actual default data.
    /// Returns `None` if undrawn is zero (fully drawn, LEQ undefined).
    pub fn leq_from_observed_ead(&self, observed_ead: f64) -> Option<f64> {
        if self.undrawn <= 0.0 {
            return None;
        }
        Some((observed_ead - self.drawn) / self.undrawn)
    }

    /// Current utilization rate = drawn / (drawn + undrawn).
    pub fn utilization(&self) -> f64 {
        let total = self.drawn + self.undrawn;
        if total <= 0.0 {
            0.0
        } else {
            self.drawn / total
        }
    }

    /// Total commitment = drawn + undrawn.
    pub fn total_commitment(&self) -> f64 {
        self.drawn + self.undrawn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ccf_validation() {
        assert!(CreditConversionFactor::new(-0.1).is_err());
        assert!(CreditConversionFactor::new(1.1).is_err());
        assert!(CreditConversionFactor::new(0.75).is_ok());
        assert!(CreditConversionFactor::new(0.0).is_ok());
        assert!(CreditConversionFactor::new(1.0).is_ok());
    }

    #[test]
    fn ccf_presets() {
        assert!((CreditConversionFactor::basel_revolver().value() - 0.75).abs() < 1e-12);
        assert!((CreditConversionFactor::full_draw().value() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn ead_term_loan() {
        let calc = EadCalculator::term_loan(100.0).expect("valid");
        assert!((calc.ead() - 100.0).abs() < 1e-12);
        assert!((calc.utilization() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn ead_revolver_with_ccf() {
        let calc = EadCalculator::revolver(60.0, 40.0).expect("valid");
        // EAD = 60 + 40 * 0.75 = 60 + 30 = 90
        assert!(
            (calc.ead() - 90.0).abs() < 1e-12,
            "EAD = {}, expected 90.0",
            calc.ead()
        );
    }

    #[test]
    fn ead_utilization() {
        let calc = EadCalculator::revolver(60.0, 40.0).expect("valid");
        assert!(
            (calc.utilization() - 0.60).abs() < 1e-12,
            "utilization = {}, expected 0.60",
            calc.utilization()
        );
    }

    #[test]
    fn ead_total_commitment() {
        let calc = EadCalculator::revolver(60.0, 40.0).expect("valid");
        assert!((calc.total_commitment() - 100.0).abs() < 1e-12);
    }

    #[test]
    fn ead_leq_roundtrip() {
        let calc = EadCalculator::revolver(60.0, 40.0).expect("valid");
        let ead = calc.ead(); // 90
        let leq = calc.leq_from_observed_ead(ead).expect("undrawn > 0");
        // LEQ = (90 - 60) / 40 = 0.75 = CCF
        assert!(
            (leq - 0.75).abs() < 1e-12,
            "LEQ = {}, expected 0.75 (= CCF)",
            leq
        );
    }

    #[test]
    fn ead_leq_returns_raw_realized_values() {
        let calc = EadCalculator::revolver(60.0, 40.0).expect("valid");

        assert_eq!(calc.leq_from_observed_ead(120.0), Some(1.5));
        assert_eq!(calc.leq_from_observed_ead(40.0), Some(-0.5));
    }

    #[test]
    fn ead_leq_fully_drawn() {
        let calc = EadCalculator::term_loan(100.0).expect("valid");
        assert!(calc.leq_from_observed_ead(100.0).is_none());
    }

    #[test]
    fn ead_validation() {
        assert!(EadCalculator::new(-1.0, 40.0, CreditConversionFactor::full_draw()).is_err());
        assert!(EadCalculator::new(60.0, -1.0, CreditConversionFactor::full_draw()).is_err());
        assert!(EadCalculator::term_loan(-1.0).is_err());
    }

    #[test]
    fn ead_custom_ccf() {
        let ccf = CreditConversionFactor::new(0.50).expect("valid");
        let calc = EadCalculator::new(60.0, 40.0, ccf).expect("valid");
        // EAD = 60 + 40 * 0.50 = 80
        assert!((calc.ead() - 80.0).abs() < 1e-12);
    }

    #[test]
    fn ead_zero_commitment() {
        let calc =
            EadCalculator::new(0.0, 0.0, CreditConversionFactor::full_draw()).expect("valid");
        assert!((calc.ead() - 0.0).abs() < 1e-12);
        assert!((calc.utilization() - 0.0).abs() < 1e-12);
    }

    #[test]
    fn ead_serialization_roundtrip() {
        let calc = EadCalculator::revolver(60.0, 40.0).expect("valid");
        let json = serde_json::to_string(&calc).expect("serialize");
        let calc2: EadCalculator = serde_json::from_str(&json).expect("deserialize");
        assert!((calc.ead() - calc2.ead()).abs() < 1e-12);
    }
}
