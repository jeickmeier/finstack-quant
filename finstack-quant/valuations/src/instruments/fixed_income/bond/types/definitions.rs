//! Bond type definitions and deserialization.

use crate::cashflow::builder::CashFlowSchedule;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::PricingOverrides;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};

use super::CashflowSpec;

fn is_linear_accrual_method(method: &crate::cashflow::accrual::AccrualMethod) -> bool {
    matches!(method, crate::cashflow::accrual::AccrualMethod::Linear)
}

/// Bond settlement and ex-coupon conventions.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct BondSettlementConvention {
    /// Number of settlement days after trade date (e.g., 2 for T+2).
    #[serde(default)]
    pub settlement_days: u32,
    /// Number of ex-coupon days before coupon date.
    #[serde(default)]
    pub ex_coupon_days: u32,
    /// Calendar identifier for ex-coupon day counting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ex_coupon_calendar_id: Option<String>,
}

/// Bond instrument with fixed, floating, or amortizing cashflows.
///
/// Cashflow sign convention (holder view):
/// - All contractual cashflows **received by a long holder** (coupons,
///   amortization, final redemption) are represented as **positive** amounts.
/// - Cash outflows for the holder (e.g., purchase price, funding, short
///   positions) are represented as **negative** amounts and are handled at
///   trade level rather than in the bond's contractual schedule.
///
/// Supports call/put schedules, quoted prices for yield-to-maturity calculations,
/// and custom cashflow schedule overrides. Uses a clean `CashflowSpec` that wraps
/// the canonical builder coupon specs for maximum flexibility and parity.
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    schemars::JsonSchema,
)]
pub struct Bond {
    /// Unique identifier for the bond.
    pub id: InstrumentId,
    /// Principal amount of the bond.
    pub notional: Money,
    /// Issue date of the bond.
    #[schemars(with = "String")]
    pub issue_date: Date,
    /// Maturity date of the bond.
    #[schemars(with = "String")]
    pub maturity: Date,
    /// Cashflow specification (fixed, floating, or amortizing).
    pub cashflow_spec: CashflowSpec,
    /// Discount curve identifier for pricing.
    pub discount_curve_id: CurveId,
    /// Optional forward curve identifier for projected floating or asset-swap legs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub forward_curve_id: Option<CurveId>,
    /// Optional credit curve identifier (default intensity). When present,
    /// credit-rate pricing is enabled.
    pub credit_curve_id: Option<CurveId>,
    /// Optional funding/repo curve for carry cost computation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub funding_curve_id: Option<CurveId>,
    /// Pricing overrides (including quoted clean price)
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: PricingOverrides,
    /// Optional call/put schedule (dates and redemption prices as % of par amount).
    pub call_put: Option<CallPutSchedule>,
    /// Optional guaranteed minimum-return ("return floor") call protection.
    ///
    /// When present, the bond is treated as prepayable across the protection
    /// window with redemption floored so the investor meets the target MOIC/XIRR.
    /// Lowered into `call_put` at pricing time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub return_floor: Option<super::return_floor::ReturnFloorSpec>,
    /// Optional pre-built cashflow schedule. If provided, this will be used instead of
    /// generating cashflows from the cashflow_spec.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_cashflows: Option<CashFlowSchedule>,
    /// Accrual method for interest calculation between coupon dates.
    ///
    /// Determines how accrued interest is calculated:
    /// - `Linear` (default): Simple interest interpolation (most bonds)
    /// - `Compounded`: Actuarial accrual per ICMA Rule 251 (some European bonds)
    ///
    /// For inflation-linked bonds (TIPS, UK Linkers), use the dedicated
    /// `InflationLinkedBond` instrument which handles index-ratio accrual.
    #[serde(default, skip_serializing_if = "is_linear_accrual_method")]
    #[schemars(extend("default" = "Linear"))]
    #[builder(default)]
    pub accrual_method: crate::cashflow::accrual::AccrualMethod,
    /// Attributes for scenario selection and tagging.
    pub attributes: Attributes,
    /// Settlement and ex-coupon conventions.
    #[serde(flatten, default, skip_serializing_if = "Option::is_none")]
    pub settlement_convention: Option<BondSettlementConvention>,
}

impl<'de> serde::Deserialize<'de> for Bond {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct BondHelper {
            id: InstrumentId,
            notional: Money,
            issue_date: Option<Date>,
            maturity: Date,
            cashflow_spec: CashflowSpec,
            discount_curve_id: CurveId,
            #[serde(default)]
            forward_curve_id: Option<CurveId>,
            credit_curve_id: Option<CurveId>,
            #[serde(default)]
            funding_curve_id: Option<CurveId>,
            #[serde(default)]
            pricing_overrides: PricingOverrides,
            call_put: Option<CallPutSchedule>,
            #[serde(default)]
            return_floor: Option<super::return_floor::ReturnFloorSpec>,
            #[serde(default)]
            custom_cashflows: Option<CashFlowSchedule>,
            #[serde(default)]
            accrual_method: crate::cashflow::accrual::AccrualMethod,
            attributes: Attributes,
            settlement_days: Option<u32>,
            ex_coupon_days: Option<u32>,
            ex_coupon_calendar_id: Option<String>,
        }

        let helper = BondHelper::deserialize(deserializer)?;
        let issue_date = if let Some(issue) = helper.issue_date {
            issue
        } else if let Some(ref sched) = helper.custom_cashflows {
            // Infer issue date from the first date in the custom cashflow schedule.
            let dates = sched.dates();
            if dates.is_empty() {
                return Err(serde::de::Error::custom(
                    "Bond requires `issue_date` (custom_cashflows schedule has no dates to infer from)",
                ));
            }
            dates[0]
        } else {
            return Err(serde::de::Error::custom(
                "Bond requires `issue_date` unless `custom_cashflows` is provided",
            ));
        };

        let settlement_convention = if helper.settlement_days.is_some()
            || helper.ex_coupon_days.is_some()
            || helper.ex_coupon_calendar_id.is_some()
        {
            Some(BondSettlementConvention {
                settlement_days: helper.settlement_days.unwrap_or(0),
                ex_coupon_days: helper.ex_coupon_days.unwrap_or(0),
                ex_coupon_calendar_id: helper.ex_coupon_calendar_id,
            })
        } else {
            None
        };

        Ok(Bond {
            id: helper.id,
            notional: helper.notional,
            issue_date,
            maturity: helper.maturity,
            cashflow_spec: helper.cashflow_spec,
            discount_curve_id: helper.discount_curve_id,
            forward_curve_id: helper.forward_curve_id,
            credit_curve_id: helper.credit_curve_id,
            funding_curve_id: helper.funding_curve_id,
            pricing_overrides: helper.pricing_overrides,
            call_put: helper.call_put,
            return_floor: helper.return_floor,
            custom_cashflows: helper.custom_cashflows,
            accrual_method: helper.accrual_method,
            attributes: helper.attributes,
            settlement_convention,
        })
    }
}

/// Call or put option on a bond.
///
/// Represents a single call or put option with an exercise period and redemption price.
/// Call options allow the issuer to redeem early; put options allow the holder to redeem early.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::bond::CallPut;
/// use finstack_quant_core::dates::Date;
/// use time::Month;
///
/// // Discrete call option: issuer can redeem at 102% of par on Jan 1, 2027
/// let call = CallPut {
///     start_date: Date::from_calendar_date(2027, Month::January, 1).unwrap(),
///     end_date: Date::from_calendar_date(2027, Month::January, 1).unwrap(),
///     price_pct_of_par: 102.0,
///     make_whole: None,
/// };
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CallPut {
    /// First date when the option can be exercised.
    #[schemars(with = "String")]
    pub start_date: Date,
    /// Last date when the option can be exercised, inclusive.
    ///
    /// Use the same value as `start_date` for one-day/discrete exercise.
    #[schemars(with = "String")]
    pub end_date: Date,
    /// Redemption price as percentage of par amount.
    pub price_pct_of_par: f64,
    /// Optional make-whole call specification.
    ///
    /// When set, the call price is computed as:
    ///   `max(price_pct_of_par, PV of remaining cashflows at reference_rate + spread)`
    ///
    /// This ensures the holder is compensated at treasury + spread for early redemption.
    /// Common in investment-grade corporate and convertible bonds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub make_whole: Option<MakeWholeSpec>,
}

/// Make-whole call specification.
///
/// Defines the reference curve and spread used to compute the make-whole redemption
/// price. The issuer pays the holder the greater of par and the present value of
/// remaining cashflows discounted at the reference rate plus a spread.
///
/// # Industry Practice
///
/// - Investment-grade corporates: typically Treasury + 25-50 bps
/// - High-yield: typically Treasury + 50-100 bps
/// - Convertibles: typically Treasury + 50 bps
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct MakeWholeSpec {
    /// Reference curve identifier (e.g., "USD-TREASURY").
    pub reference_curve_id: CurveId,
    /// Spread over the reference curve in basis points (e.g., 50.0 = T+50bps).
    pub spread_bps: f64,
}

/// Schedule of call and put options for a bond.
///
/// Contains lists of call and put options that can be exercised during the bond's life.
/// Used for pricing callable/putable bonds and calculating yield-to-worst.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::bond::{CallPut, CallPutSchedule};
/// use finstack_quant_core::dates::Date;
/// use time::Month;
///
/// let mut schedule = CallPutSchedule::default();
/// schedule.calls.push(CallPut {
///     start_date: Date::from_calendar_date(2027, Month::January, 1).unwrap(),
///     end_date: Date::from_calendar_date(2027, Month::January, 1).unwrap(),
///     price_pct_of_par: 102.0,
///     make_whole: None,
/// });
/// ```
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CallPutSchedule {
    /// Call options (issuer can redeem early).
    pub calls: Vec<CallPut>,
    /// Put options (holder can redeem early).
    pub puts: Vec<CallPut>,
}

impl CallPutSchedule {
    /// Check if this schedule has any active call or put options.
    ///
    /// # Returns
    ///
    /// `true` if the schedule contains at least one call or put option, `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::fixed_income::bond::CallPutSchedule;
    ///
    /// let schedule = CallPutSchedule::default();
    /// assert!(!schedule.has_options());
    /// ```
    pub fn has_options(&self) -> bool {
        !self.calls.is_empty() || !self.puts.is_empty()
    }
}

#[cfg(test)]
mod return_floor_field_tests {
    use crate::instruments::fixed_income::bond::types::return_floor::ReturnFloorSpec;

    #[test]
    fn bond_roundtrips_return_floor_via_serde() {
        let mut bond = crate::instruments::fixed_income::bond::Bond::example().unwrap();
        bond.return_floor = Some(ReturnFloorSpec::moic(1.25));
        let json = serde_json::to_string(&bond).unwrap();
        assert!(json.contains("return_floor"));
        let back: crate::instruments::fixed_income::bond::Bond =
            serde_json::from_str(&json).unwrap();
        assert_eq!(back.return_floor, Some(ReturnFloorSpec::moic(1.25)));
    }

    #[test]
    fn bond_omits_return_floor_from_json_when_none() {
        let bond = crate::instruments::fixed_income::bond::Bond::example().unwrap();
        assert!(bond.return_floor.is_none());
        let json = serde_json::to_string(&bond).unwrap();
        assert!(!json.contains("return_floor"));
    }
}
