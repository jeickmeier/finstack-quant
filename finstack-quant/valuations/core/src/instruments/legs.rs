//! Shared leg specification types for total-return and multi-family swaps.

use finstack_quant_core::dates::DayCount;
use finstack_quant_core::types::CurveId;
use rust_decimal::Decimal;

use serde::{Deserialize, Serialize};

/// Direction for instrument legs (universal for IRS, CDS, variance swaps, etc.)
///
/// For interest rate swaps: Pay = pay fixed/receive floating, Receive = receive fixed/pay floating
/// For credit default swaps: Pay = buy protection (pay premium), Receive = sell protection (receive premium)
/// For variance swaps: Pay = short variance, Receive = long variance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PayReceive {
    /// Pay the primary leg (fixed rate in IRS, protection premium in CDS, short variance)
    Pay,
    /// Receive the primary leg (fixed rate in IRS, protection premium in CDS, long variance)
    Receive,
}

impl PayReceive {
    /// Returns the sign multiplier (+1.0 for Receive, -1.0 for Pay).
    pub fn sign(&self) -> f64 {
        match self {
            PayReceive::Pay => -1.0,
            PayReceive::Receive => 1.0,
        }
    }
}

impl std::fmt::Display for PayReceive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PayReceive::Pay => write!(f, "pay"),
            PayReceive::Receive => write!(f, "receive"),
        }
    }
}

impl std::str::FromStr for PayReceive {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "pay" => Ok(PayReceive::Pay),
            "receive" => Ok(PayReceive::Receive),
            _ => Err(format!("Unknown pay/receive: '{}'. Valid: pay, receive", s)),
        }
    }
}

/// Specification for TRS financing legs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FinancingLegSpec {
    /// Discount curve identifier for present value calculations
    pub discount_curve_id: CurveId,
    /// Forward curve identifier (e.g., USD-SOFR-3M)
    pub forward_curve_id: CurveId,
    /// Spread in basis points over the floating rate (e.g., 50 = 50bp = 0.5%)
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub spread_bp: Decimal,
    /// Day count convention for accrual calculations
    pub day_count: DayCount,
    /// Rate-compounding convention of the financing rate.
    ///
    /// Defaults to `simple` (one term forward per period, e.g. 3M Term SOFR or
    /// EURIBOR). Use a `compounded_*` variant for SOFR/SONIA/€STR
    /// overnight-funded TRS so the financing rate captures the daily
    /// compounding convexity (typically 12–15 bp of rate on an upward curve).
    /// `simple_average` is rejected.
    #[serde(default)]
    pub compounding: finstack_quant_cashflows::builder::FloatingLegCompounding,
}

impl FinancingLegSpec {
    /// Create a new financing leg specification.
    ///
    /// The compounding convention defaults to `simple` for term indices and
    /// unknown ids. A registered overnight RFR forward id (for example
    /// `USD-SOFR-OIS`) takes its registered OIS convention. Use
    /// [`FinancingLegSpec::with_compounding`] to override.
    ///
    /// # Arguments
    ///
    /// * `discount_curve_id` - Discount curve used to PV financing coupons.
    /// * `forward_curve_id` - Term or overnight financing index / forward curve.
    /// * `spread_bp` - Contractual financing spread in basis points.
    /// * `day_count` - Accrual day-count for the financing coupon.
    pub fn new(
        discount_curve_id: impl Into<CurveId>,
        forward_curve_id: impl Into<CurveId>,
        spread_bp: Decimal,
        day_count: DayCount,
    ) -> Self {
        let forward_curve_id: CurveId = forward_curve_id.into();
        let compounding =
            match crate::instruments::pricing::overnight_conventions::compounding_from_index_id(
                forward_curve_id.as_str(),
            ) {
                Ok(Some(registered)) => registered,
                Ok(None) | Err(_) => {
                    finstack_quant_cashflows::builder::FloatingLegCompounding::Simple
                }
            };
        Self {
            discount_curve_id: discount_curve_id.into(),
            forward_curve_id,
            spread_bp,
            day_count,
            compounding,
        }
    }

    /// Set the rate-compounding convention (consuming builder style).
    ///
    /// # Arguments
    ///
    /// * `compounding` - Term-rate versus overnight-compounded convention applied when
    ///   projecting the TRS funding leg.
    pub fn with_compounding(
        mut self,
        compounding: finstack_quant_cashflows::builder::FloatingLegCompounding,
    ) -> Self {
        self.compounding = compounding;
        self
    }

    /// Validate the curve identifiers required by the financing leg.
    ///
    /// # Arguments
    ///
    /// * `context` - Instrument name included in validation diagnostics.
    pub fn validate(&self, context: &str) -> finstack_quant_core::Result<()> {
        if self.discount_curve_id.as_str().trim().is_empty() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} requires a non-empty financing discount_curve_id"
            )));
        }
        if self.forward_curve_id.as_str().trim().is_empty() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} requires a non-empty financing forward_curve_id"
            )));
        }
        if self.compounding
            == finstack_quant_cashflows::builder::FloatingLegCompounding::SimpleAverage
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} financing_leg.compounding = simple_average is not supported; \
                 use simple or a compounded_* variant"
            )));
        }
        crate::instruments::pricing::overnight_conventions::reject_simple_overnight(
            self.forward_curve_id.as_str(),
            &self.compounding,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pay_receive_sign_display_and_from_str() {
        assert_eq!(PayReceive::Pay.sign(), -1.0);
        assert_eq!(PayReceive::Receive.sign(), 1.0);
        assert_eq!(PayReceive::Pay.to_string(), "pay");
        assert_eq!(PayReceive::Receive.to_string(), "receive");
        assert_eq!("pay".parse::<PayReceive>(), Ok(PayReceive::Pay));
        assert_eq!("receive".parse::<PayReceive>(), Ok(PayReceive::Receive));
        assert!("other".parse::<PayReceive>().is_err());
    }

    #[test]
    fn financing_new_upgrades_overnight_index() {
        let ois = FinancingLegSpec::new("USD-OIS", "USD-SOFR-OIS", Decimal::ZERO, DayCount::Act360);
        assert_eq!(
            ois.compounding,
            finstack_quant_cashflows::builder::FloatingLegCompounding::CompoundedInArrears {
                lookback_days: 0
            }
        );
        let term = FinancingLegSpec::new("USD-OIS", "USD-SOFR-3M", Decimal::ZERO, DayCount::Act360);
        assert_eq!(
            term.compounding,
            finstack_quant_cashflows::builder::FloatingLegCompounding::Simple
        );
    }

    #[test]
    fn financing_validate_rejects_term_rate_on_overnight_index() {
        let spec = FinancingLegSpec {
            discount_curve_id: CurveId::new("USD-OIS"),
            forward_curve_id: CurveId::new("USD-SOFR-OIS"),
            spread_bp: Decimal::ZERO,
            day_count: DayCount::Act360,
            compounding: finstack_quant_cashflows::builder::FloatingLegCompounding::Simple,
        };
        let err = spec
            .validate("TRS")
            .expect_err("Simple on USD-SOFR-OIS must fail");
        assert!(
            format!("{err}").contains("Overnight RFR"),
            "expected overnight/simple rejection, got {err}"
        );
    }
}
