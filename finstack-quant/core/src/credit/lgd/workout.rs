//! Workout-based LGD model using a collateral-first recovery waterfall.
//!
//! Computes LGD via:
//!
//! ```text
//! gross_recovery = sum(collateral_i.book_value * (1 - haircut_i))
//! net_recovery   = (gross_recovery - costs * EAD) * DF(workout_years)
//! LGD            = 1 - clamp(net_recovery / EAD, 0, 1)
//! ```
//!
//! Both recoveries and workout costs are realized during the workout period
//! and are discounted back to the default date with the same discount factor,
//! per the Basel workout-LGD methodology (costs were previously left
//! undiscounted; fixed per the ).
//!
//! # References
//!
//! - Basel Committee (2005). "Guidance on Paragraph 468 of the Framework
//!   Document" (workout LGD methodology).
//! - Qi, M. & Yang, X. (2009). "Loss Given Default of High Loan-to-Value
//!   Residential Mortgages." Journal of Banking & Finance.

use crate::error::{InputError, NonFiniteKind};
use crate::Result;

fn non_finite_kind(value: f64) -> NonFiniteKind {
    if value.is_nan() {
        NonFiniteKind::NaN
    } else if value.is_sign_positive() {
        NonFiniteKind::PosInfinity
    } else {
        NonFiniteKind::NegInfinity
    }
}

fn validate_finite(value: f64) -> Result<()> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(InputError::NonFiniteValue {
            kind: non_finite_kind(value),
        }
        .into())
    }
}

/// Collateral asset class with associated liquidation haircut.
///
/// Haircuts represent the discount from book value realized in a
/// forced-sale / workout scenario. Values are in \[0, 1\] where 0 means
/// full recovery of book value and 1 means total loss.
#[derive(
    Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub enum CollateralType {
    /// Cash and cash equivalents. Typical haircut: 0-5%.
    Cash,
    /// Government securities. Typical haircut: 2-10%.
    Securities,
    /// Accounts receivable. Typical haircut: 20-40%.
    Receivables,
    /// Inventory (raw materials, finished goods). Typical haircut: 30-60%.
    Inventory,
    /// Equipment and machinery. Typical haircut: 30-50%.
    Equipment,
    /// Commercial real estate. Typical haircut: 20-40%.
    RealEstate,
    /// Intellectual property (patents, trademarks). Typical haircut: 50-90%.
    IntellectualProperty,
    /// Other / custom collateral.
    Other,
}

impl std::str::FromStr for CollateralType {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self> {
        let norm = s.trim().to_ascii_lowercase().replace(['-', ' '], "_");
        match norm.as_str() {
            "cash" => Ok(Self::Cash),
            "securities" => Ok(Self::Securities),
            "receivables" => Ok(Self::Receivables),
            "inventory" => Ok(Self::Inventory),
            "equipment" => Ok(Self::Equipment),
            "real_estate" | "realestate" => Ok(Self::RealEstate),
            "intellectual_property" | "intellectualproperty" | "ip" => {
                Ok(Self::IntellectualProperty)
            }
            "other" => Ok(Self::Other),
            _ => Err(crate::Error::Validation(format!(
                "unknown collateral type: '{s}' (expected cash, securities, receivables, inventory, equipment, real_estate, intellectual_property, other)"
            ))),
        }
    }
}

/// A single piece of collateral in the recovery waterfall.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CollateralPiece {
    /// Collateral asset class.
    pub collateral_type: CollateralType,
    /// Book value (pre-haircut) of the collateral.
    pub book_value: f64,
    /// Liquidation haircut in \[0, 1\]. Applied as: liquidation_value = book_value * (1 - haircut).
    pub haircut: f64,
}

impl CollateralPiece {
    /// Create a new collateral piece.
    ///
    /// # Errors
    ///
    /// Returns an error if inputs are non-finite, `book_value < 0`, or `haircut` is not in \[0, 1\].
    pub fn new(collateral_type: CollateralType, book_value: f64, haircut: f64) -> Result<Self> {
        validate_finite(book_value)?;
        validate_finite(haircut)?;
        if book_value < 0.0 {
            return Err(InputError::NegativeValue.into());
        }
        if !(0.0..=1.0).contains(&haircut) {
            return Err(InputError::Invalid.into());
        }
        Ok(Self {
            collateral_type,
            book_value,
            haircut,
        })
    }

    /// Net liquidation value after haircut.
    pub fn liquidation_value(&self) -> f64 {
        self.book_value * (1.0 - self.haircut)
    }
}

/// Workout and resolution costs.
///
/// These reduce the net recovery available to creditors.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct WorkoutCosts {
    /// Direct costs as fraction of EAD (legal fees, administrative). Typical: 3-8%.
    pub direct_cost_rate: f64,
    /// Indirect costs as fraction of EAD (opportunity cost, management distraction). Typical: 2-5%.
    pub indirect_cost_rate: f64,
}

impl WorkoutCosts {
    /// Create workout costs specification.
    ///
    /// # Errors
    ///
    /// Returns an error if either rate is non-finite or negative.
    pub fn new(direct_cost_rate: f64, indirect_cost_rate: f64) -> Result<Self> {
        validate_finite(direct_cost_rate)?;
        validate_finite(indirect_cost_rate)?;
        if direct_cost_rate < 0.0 || indirect_cost_rate < 0.0 {
            return Err(InputError::NegativeValue.into());
        }
        Ok(Self {
            direct_cost_rate,
            indirect_cost_rate,
        })
    }

    /// Total cost rate (direct + indirect).
    pub fn total_rate(&self) -> f64 {
        self.direct_cost_rate + self.indirect_cost_rate
    }

    /// Zero costs (for testing or when costs are embedded elsewhere).
    pub fn zero() -> Self {
        Self {
            direct_cost_rate: 0.0,
            indirect_cost_rate: 0.0,
        }
    }

    /// Standard workout costs from the credit assumptions registry.
    ///
    /// # Errors
    ///
    /// Propagates registry load/validation errors. There is intentionally no
    /// `Default` impl for this type: a `Default` that swallowed registry
    /// errors silently zeroed the costs (2026-06-09 core quant review minor
    /// finding). Use [`WorkoutCosts::standard`] and handle the `Result`, or
    /// [`WorkoutCosts::zero`] when costs are deliberately zero.
    pub fn standard() -> Result<Self> {
        let defaults = crate::credit::registry::embedded_registry()?.workout_lgd_defaults(
            crate::credit::registry::embedded_registry()?.default_workout_lgd_id(),
        )?;
        Self::new(defaults.direct_cost_rate, defaults.indirect_cost_rate)
    }
}

/// Workout-based LGD model using a collateral-first recovery waterfall.
///
/// Computes LGD via the formula:
///
/// ```text
/// gross_recovery = sum(collateral_i.book_value * (1 - haircut_i))
/// net_recovery   = (gross_recovery - costs * EAD) * DF(workout_years)
/// LGD            = 1 - clamp(net_recovery / EAD, 0, 1)
/// ```
///
/// where `DF` is the discount factor for the expected time-to-resolution
/// and costs include both direct (legal, administrative) and indirect
/// (opportunity cost) components. Costs are discounted to the default date
/// with the same factor as recoveries (Basel workout-LGD methodology).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct WorkoutLgd {
    /// Ordered collateral waterfall (highest priority first).
    collateral: Vec<CollateralPiece>,
    /// Expected workout duration in years. Typical: 1-5 years.
    workout_years: f64,
    /// Discount rate for time-value-of-money during workout.
    discount_rate: f64,
    /// Direct and indirect resolution costs.
    costs: WorkoutCosts,
}

impl WorkoutLgd {
    /// Start building a WorkoutLgd.
    #[must_use]
    pub fn builder() -> WorkoutLgdBuilder {
        WorkoutLgdBuilder::default()
    }

    /// Compute LGD for a given exposure at default.
    ///
    /// # Arguments
    ///
    /// * `ead` - Exposure at default (must be > 0).
    ///
    /// # Errors
    ///
    /// Returns an error if `ead` is non-finite or `ead <= 0`.
    pub fn lgd(&self, ead: f64) -> Result<f64> {
        let net_recovery = self.net_recovery(ead)?;

        Ok((1.0 - net_recovery / ead).clamp(0.0, 1.0))
    }

    /// Net recovery amount after collateral cap, discounting, and workout costs.
    ///
    /// # Errors
    ///
    /// Returns an error if `ead` is non-finite or `ead <= 0`.
    pub fn net_recovery(&self, ead: f64) -> Result<f64> {
        validate_finite(ead)?;
        if ead <= 0.0 {
            return Err(InputError::NonPositiveValue.into());
        }

        let gross_recovery: f64 = self
            .collateral
            .iter()
            .map(|c| c.liquidation_value())
            .sum::<f64>()
            .min(ead); // Recovery capped at EAD

        let df = (1.0 + self.discount_rate).powf(-self.workout_years);
        let total_costs = self.costs.total_rate() * ead;
        // Discount both recoveries and workout costs to the default date
        // (Basel workout-LGD;
        // costs were previously left undiscounted).
        let net_recovery = ((gross_recovery - total_costs) * df).max(0.0);

        Ok(net_recovery)
    }

    /// Recovery rate = 1 - LGD.
    pub fn recovery_rate(&self, ead: f64) -> Result<f64> {
        Ok(1.0 - self.lgd(ead)?)
    }

    /// Gross collateral liquidation value (pre-discount, pre-costs).
    pub fn gross_collateral_value(&self) -> f64 {
        self.collateral.iter().map(|c| c.liquidation_value()).sum()
    }

    /// Discount factor implied by workout duration.
    pub fn workout_discount_factor(&self) -> f64 {
        (1.0 + self.discount_rate).powf(-self.workout_years)
    }
}

/// Builder for `WorkoutLgd`.
#[derive(Debug, Clone, Default)]
pub struct WorkoutLgdBuilder {
    collateral: Vec<CollateralPiece>,
    workout_years: Option<f64>,
    discount_rate: Option<f64>,
    costs: Option<WorkoutCosts>,
}

impl WorkoutLgdBuilder {
    /// Add a collateral piece to the waterfall.
    pub fn collateral(mut self, piece: CollateralPiece) -> Self {
        self.collateral.push(piece);
        self
    }

    /// Add multiple collateral pieces.
    pub fn collateral_pieces(mut self, pieces: Vec<CollateralPiece>) -> Self {
        self.collateral.extend(pieces);
        self
    }

    /// Set expected workout duration in years (default: 2.0).
    pub fn workout_years(mut self, years: f64) -> Self {
        self.workout_years = Some(years);
        self
    }

    /// Set discount rate for workout period (default: 0.05).
    pub fn discount_rate(mut self, rate: f64) -> Self {
        self.discount_rate = Some(rate);
        self
    }

    /// Set workout costs (default: 5% direct + 3% indirect).
    pub fn costs(mut self, costs: WorkoutCosts) -> Self {
        self.costs = Some(costs);
        self
    }

    /// Build the WorkoutLgd model.
    ///
    /// # Errors
    ///
    /// Returns an error if workout_years or discount_rate is non-finite or negative.
    pub fn build(self) -> Result<WorkoutLgd> {
        let defaults = crate::credit::registry::embedded_registry()?.workout_lgd_defaults(
            crate::credit::registry::embedded_registry()?.default_workout_lgd_id(),
        )?;
        let workout_years = self.workout_years.unwrap_or(defaults.workout_years);
        let discount_rate = self.discount_rate.unwrap_or(defaults.discount_rate);

        validate_finite(workout_years)?;
        validate_finite(discount_rate)?;
        if workout_years < 0.0 {
            return Err(InputError::NegativeValue.into());
        }
        if discount_rate < 0.0 {
            return Err(InputError::NegativeValue.into());
        }

        Ok(WorkoutLgd {
            collateral: self.collateral,
            workout_years,
            discount_rate,
            costs: match self.costs {
                Some(costs) => costs,
                None => WorkoutCosts::new(defaults.direct_cost_rate, defaults.indirect_cost_rate)?,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn collateral_piece_liquidation_value() {
        let piece =
            CollateralPiece::new(CollateralType::RealEstate, 80.0, 0.30).expect("valid collateral");
        assert!((piece.liquidation_value() - 56.0).abs() < 1e-12);
    }

    #[test]
    fn collateral_type_from_str_accepts_aliases_and_rejects_unknown() {
        assert_eq!(
            CollateralType::from_str("real estate").expect("alias"),
            CollateralType::RealEstate
        );
        assert_eq!(
            CollateralType::from_str("IP").expect("alias"),
            CollateralType::IntellectualProperty
        );
        assert_eq!(
            CollateralType::from_str("cash").expect("cash"),
            CollateralType::Cash
        );

        let err = CollateralType::from_str("crypto")
            .expect_err("unknown collateral type should be rejected");
        assert!(
            err.to_string().contains("unknown collateral type"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn collateral_piece_validation() {
        // Negative book value
        assert!(CollateralPiece::new(CollateralType::Cash, -1.0, 0.0).is_err());
        // Haircut out of range
        assert!(CollateralPiece::new(CollateralType::Cash, 100.0, -0.1).is_err());
        assert!(CollateralPiece::new(CollateralType::Cash, 100.0, 1.1).is_err());
        assert!(CollateralPiece::new(CollateralType::Cash, f64::NAN, 0.0).is_err());
    }

    #[test]
    fn workout_costs_default() {
        let costs = WorkoutCosts::standard().expect("registry costs");
        assert!((costs.direct_cost_rate - 0.05).abs() < 1e-12);
        assert!((costs.indirect_cost_rate - 0.03).abs() < 1e-12);
        assert!((costs.total_rate() - 0.08).abs() < 1e-12);
    }

    #[test]
    fn workout_costs_validation() {
        assert!(WorkoutCosts::new(-0.01, 0.03).is_err());
        assert!(WorkoutCosts::new(0.05, -0.01).is_err());
        assert!(WorkoutCosts::new(f64::NAN, 0.03).is_err());
        assert!(WorkoutCosts::new(0.05, f64::INFINITY).is_err());
        assert!(WorkoutCosts::new(0.05, 0.03).is_ok());
    }

    #[test]
    fn workout_lgd_single_collateral() {
        // Single collateral: RE $80M, 30% haircut, EAD $100M, 2yr workout at 5%, 8% total costs
        let piece =
            CollateralPiece::new(CollateralType::RealEstate, 80.0, 0.30).expect("valid collateral");
        let costs = WorkoutCosts::new(0.05, 0.03).expect("valid costs");

        let model = WorkoutLgd::builder()
            .collateral(piece)
            .workout_years(2.0)
            .discount_rate(0.05)
            .costs(costs)
            .build()
            .expect("valid model");

        let ead = 100.0;
        let lgd = model.lgd(ead).expect("valid ead");

        // gross_recovery = 80 * 0.70 = 56.0 (capped at 100)
        // df = (1.05)^-2 = 0.907029...
        // Costs are discounted alongside recoveries (Basel workout-LGD;
        // ):
        // net = (56.0 - 8.0) * 0.907029 = 43.5374
        // LGD = 1 - 43.5374 / 100 = 0.564626
        let expected_gross = 56.0;
        let df = 1.05_f64.powf(-2.0);
        let expected_net = (expected_gross - 8.0) * df;
        let expected_lgd = 1.0 - expected_net / ead;

        assert!(
            (lgd - expected_lgd).abs() < 1e-6,
            "LGD = {}, expected {}",
            lgd,
            expected_lgd
        );
    }

    #[test]
    fn workout_lgd_zero_collateral() {
        let model = WorkoutLgd::builder()
            .workout_years(2.0)
            .discount_rate(0.05)
            .build()
            .expect("valid model");

        let lgd = model.lgd(100.0).expect("valid ead");
        assert!(
            (lgd - 1.0).abs() < 1e-12,
            "LGD with no collateral should be 1.0, got {}",
            lgd
        );
    }

    #[test]
    fn workout_lgd_collateral_exceeds_ead() {
        // Collateral liquidation value > EAD: recovery capped at EAD
        let piece =
            CollateralPiece::new(CollateralType::Cash, 200.0, 0.0).expect("valid collateral");

        let model = WorkoutLgd::builder()
            .collateral(piece)
            .workout_years(0.0)
            .discount_rate(0.0)
            .costs(WorkoutCosts::zero())
            .build()
            .expect("valid model");

        let lgd = model.lgd(100.0).expect("valid ead");
        assert!(
            (lgd - 0.0).abs() < 1e-12,
            "LGD should be 0 when collateral exceeds EAD, got {}",
            lgd
        );
    }

    #[test]
    fn workout_lgd_ead_validation() {
        let model = WorkoutLgd::builder().build().expect("valid model");
        assert!(model.lgd(0.0).is_err());
        assert!(model.lgd(-1.0).is_err());
        assert!(model.lgd(f64::NAN).is_err());
        assert!(model.lgd(f64::INFINITY).is_err());
    }

    #[test]
    fn workout_lgd_builder_validation() {
        // Negative workout years
        assert!(WorkoutLgd::builder().workout_years(-1.0).build().is_err());
        // Negative discount rate
        assert!(WorkoutLgd::builder().discount_rate(-0.01).build().is_err());
        assert!(WorkoutLgd::builder()
            .workout_years(f64::NAN)
            .build()
            .is_err());
        assert!(WorkoutLgd::builder()
            .discount_rate(f64::INFINITY)
            .build()
            .is_err());
    }

    #[test]
    fn workout_lgd_multiple_collateral() {
        let p1 = CollateralPiece::new(CollateralType::Cash, 20.0, 0.0).expect("valid");
        let p2 = CollateralPiece::new(CollateralType::RealEstate, 60.0, 0.25).expect("valid");

        let model = WorkoutLgd::builder()
            .collateral(p1)
            .collateral(p2)
            .workout_years(1.0)
            .discount_rate(0.05)
            .costs(WorkoutCosts::zero())
            .build()
            .expect("valid model");

        let ead = 100.0;
        let lgd = model.lgd(ead).expect("valid ead");

        // gross = 20*1.0 + 60*0.75 = 20 + 45 = 65
        // df = 1.05^-1 = 0.9523809...
        // net = 65 * 0.9523809 = 61.9047...
        // LGD = 1 - 61.9047/100 = 0.380952
        let expected_gross = 65.0;
        let df = 1.05_f64.powf(-1.0);
        let expected_lgd = 1.0 - (expected_gross * df) / ead;
        assert!(
            (lgd - expected_lgd).abs() < 1e-4,
            "LGD = {}, expected {}",
            lgd,
            expected_lgd
        );
    }

    #[test]
    fn workout_lgd_recovery_rate_complement() {
        let piece = CollateralPiece::new(CollateralType::Equipment, 50.0, 0.40).expect("valid");
        let model = WorkoutLgd::builder()
            .collateral(piece)
            .build()
            .expect("valid model");

        let lgd = model.lgd(100.0).expect("valid");
        let rr = model.recovery_rate(100.0).expect("valid");
        assert!((lgd + rr - 1.0).abs() < 1e-12);
    }

    #[test]
    fn workout_lgd_serialization_roundtrip() {
        let piece = CollateralPiece::new(CollateralType::RealEstate, 80.0, 0.30).expect("valid");
        let model = WorkoutLgd::builder()
            .collateral(piece)
            .build()
            .expect("valid model");

        let json = serde_json::to_string(&model).expect("serialize");
        let model2: WorkoutLgd = serde_json::from_str(&json).expect("deserialize");

        assert!((model.lgd(100.0).expect("ok") - model2.lgd(100.0).expect("ok")).abs() < 1e-12);
    }
}
