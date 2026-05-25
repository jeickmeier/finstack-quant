//! Convertible bond instrument types and implementation.
//!
//! Data model for `ConvertibleBond` and related enums used by pricing and
//! metrics modules. Pricing logic is intentionally kept out of this file.

use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_core::Error;

use crate::cashflow::builder::specs::{FixedCouponSpec, FloatingCouponSpec};
use crate::cashflow::builder::CashFlowSchedule;
use crate::cashflow::CashflowProvider;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::fixed_income::bond::CallPutSchedule;
use crate::instruments::model_params::ModelParamsSnapshot;

use super::pricer;
use crate::impl_instrument_base;

/// Soft-call trigger condition for convertible bonds.
///
/// A soft call allows the issuer to call the bond only if the underlying stock
/// price has been trading above a threshold (typically 130% of the conversion
/// price) for a sustained period. This protects holders from having their
/// conversion option terminated when the stock is only marginally above parity.
///
/// # Industry Practice
///
/// The standard soft-call trigger is:
/// - **Threshold**: 130% of conversion price (most common)
/// - **Observation period**: 20 of 30 consecutive trading days
///
/// Some issuances use 120% or 150% thresholds.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SoftCallTrigger {
    /// Threshold as a percentage of conversion price (e.g., 130.0 = 130%).
    ///
    /// The issuer can only exercise the call if the stock price exceeds
    /// `threshold_pct / 100 * conversion_price` for the required number of days.
    pub threshold_pct: f64,
    /// Number of trading days in the observation window (e.g., 30).
    pub observation_days: u32,
    /// Minimum number of days within the window that the stock must exceed
    /// the threshold (e.g., 20 out of 30 days).
    pub required_days_above: u32,
}

impl Default for SoftCallTrigger {
    /// Standard market convention: 130% trigger, 20 of 30 days.
    fn default() -> Self {
        Self {
            threshold_pct: 130.0,
            observation_days: 30,
            required_days_above: 20,
        }
    }
}

impl SoftCallTrigger {
    /// Validate soft-call trigger parameters.
    ///
    /// - `threshold_pct` must exceed 100% (otherwise the trigger is trivially satisfied).
    /// - `required_days_above` cannot exceed `observation_days`.
    pub fn validate(&self) -> finstack_core::Result<()> {
        if self.threshold_pct <= 100.0 {
            return Err(finstack_core::Error::Validation(format!(
                "soft-call threshold_pct ({:.1}%) must exceed 100%",
                self.threshold_pct,
            )));
        }
        if self.required_days_above > self.observation_days {
            return Err(finstack_core::Error::Validation(format!(
                "soft-call required_days_above ({}) cannot exceed observation_days ({})",
                self.required_days_above, self.observation_days,
            )));
        }
        Ok(())
    }
}

/// Convertible bond instrument with embedded equity conversion option.
///
/// This fixed income instrument combines debt characteristics (coupons, principal)
/// with equity optionality (conversion rights). Uses the `CashFlowBuilder` for
/// robust schedule generation and tree-based pricing for the hybrid valuation.
#[derive(
    Clone,
    Debug,
    finstack_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ConvertibleBond {
    /// Unique identifier for the instrument.
    pub id: InstrumentId,
    /// Principal amount.
    pub notional: Money,
    /// Issue date.
    #[schemars(with = "String")]
    pub issue_date: Date,
    /// Maturity date.
    #[schemars(with = "String")]
    pub maturity: Date,
    /// Discount curve identifier for the debt component (risk-free or funding).
    pub discount_curve_id: CurveId,
    /// Credit curve identifier for risky discounting (bond floor).
    /// If not provided, falls back to discount_curve_id (implies no credit spread).
    #[builder(optional)]
    pub credit_curve_id: Option<CurveId>,
    /// Conversion terms for equity conversion.
    pub conversion: ConversionSpec,
    /// Optional underlying equity identifier (ticker or instrument id).
    #[builder(optional)]
    pub underlying_equity_id: Option<String>,
    /// Optional call/put schedule (issuer/holder redemption before maturity).
    #[builder(optional)]
    pub call_put: Option<CallPutSchedule>,
    /// Optional soft-call trigger condition.
    ///
    /// When set, the issuer can only exercise call provisions if the underlying
    /// stock price satisfies the trigger condition (e.g., above 130% of conversion
    /// price for 20 of 30 trading days).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soft_call_trigger: Option<SoftCallTrigger>,
    /// Number of business days from trade date to settlement date.
    ///
    /// When set, accrued interest and clean price are computed relative to the
    /// settlement date (trade date + settlement_days business days) rather than
    /// the valuation date. Standard values:
    /// - **US corporate convertibles**: 2 (T+2)
    /// - **US Treasury**: 1 (T+1)
    ///
    /// If `None`, settlement is assumed same-day (as_of = settlement date).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement_days: Option<u32>,
    /// Assumed recovery rate on default, as a fraction (e.g., 0.40 = 40%).
    ///
    /// Used in the Tsiveriotis-Zhang credit model to blend risky and risk-free
    /// discounting on the cash component. A recovery rate of 0 (the default)
    /// reduces to the standard zero-recovery TZ model. Typical values:
    /// - **Investment grade**: 0.40 (ISDA standard assumption)
    /// - **High yield**: 0.25-0.35
    /// - **Distressed**: 0.10-0.20
    ///
    /// Only relevant when `credit_curve_id` is set.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_rate: Option<f64>,
    /// Fixed coupon specification (if applicable).
    #[builder(optional)]
    pub fixed_coupon: Option<FixedCouponSpec>,
    /// Floating coupon specification (if applicable).
    #[builder(optional)]
    pub floating_coupon: Option<FloatingCouponSpec>,
    /// Attributes for selection and tagging.
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

/// Greeks for convertible bonds priced with tree models.
///
/// # Units and Conventions
///
/// - **Delta**: Per unit of spot (dPV/dS)
/// - **Gamma**: Per unit of spot squared (d²PV/dS²)
/// - **Vega**: Per 1% absolute volatility move (dPV for +1 vol point)
/// - **Theta**: Per calendar day (P(t+1d) - P(t), typically negative for long positions)
/// - **Rho**: Per 1 basis point parallel rate shift (dPV for +1bp)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ConvertibleGreeks {
    /// Instrument price
    pub price: f64,
    /// Delta (spot sensitivity per unit spot move)
    pub delta: f64,
    /// Gamma (curvature, second derivative w.r.t. spot)
    pub gamma: f64,
    /// Vega (volatility sensitivity per 1% vol move)
    pub vega: f64,
    /// Theta (time decay per day)
    pub theta: f64,
    /// Rho (interest rate sensitivity per 1bp rate move)
    pub rho: f64,
}

impl From<crate::models::TreeGreeks> for ConvertibleGreeks {
    fn from(g: crate::models::TreeGreeks) -> Self {
        Self {
            price: g.price,
            delta: g.delta,
            gamma: g.gamma,
            vega: g.vega,
            theta: g.theta,
            rho: g.rho,
        }
    }
}

/// Defines how and when conversion can occur.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum ConversionPolicy {
    /// Holder may convert at any time (subject to window, if any).
    Voluntary,
    /// Bond will mandatorily convert on the specified date.
    MandatoryOn(#[schemars(with = "String")] Date),
    /// Holder may convert within a window.
    Window {
        /// Start.
        #[schemars(with = "String")]
        start: Date,
        /// End.
        #[schemars(with = "String")]
        end: Date,
    },
    /// Conversion tied to an external event or condition.
    UponEvent(ConversionEvent),
    /// Mandatory conversion with variable delivery ratio (PERCS / DECS / ACES).
    ///
    /// At `conversion_date`, the delivery ratio depends on the stock price:
    /// - If `spot <= lower_conversion_price`: ratio = face / lower_price (max shares, loss)
    /// - If `lower < spot <= upper`: ratio = face / spot (variable, delivers face value)
    /// - If `spot > upper_conversion_price`: ratio = face / upper_price (min shares, capped)
    ///
    /// # Industry Practice
    ///
    /// PERCS (Preference Equity Redemption Cumulative Stock) cap the upside.
    /// DECS (Dividend Enhanced Convertible Stock) have a dead zone between prices.
    /// ACES (Automatically Convertible Equity Securities) are similar to DECS.
    MandatoryVariable {
        /// Date of mandatory conversion.
        #[schemars(with = "String")]
        conversion_date: Date,
        /// Upper conversion price (above this, holder receives min shares).
        upper_conversion_price: f64,
        /// Lower conversion price (below this, holder receives max shares).
        lower_conversion_price: f64,
    },
}

/// Events that may trigger conversion.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum ConversionEvent {
    /// Qualified Ipo variant.
    QualifiedIpo,
    /// Change Of Control variant.
    ChangeOfControl,
    /// Forced conversion if share price meets threshold for a lookback period.
    PriceTrigger {
        /// Threshold.
        threshold: f64,
        /// Lookback days.
        lookback_days: u32,
    },
}

/// Anti-dilution protection applied to conversion terms.
///
/// When dilutive events occur (stock splits, below-market issuances, special
/// dividends), the conversion ratio is adjusted to protect bondholders from
/// value erosion.
///
/// # Industry Practice
///
/// Most convertible bonds use **Weighted Average** anti-dilution, which is
/// less protective but more issuer-friendly. **Full Ratchet** is mainly seen
/// in private placements and venture-style convertibles.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum AntiDilutionPolicy {
    /// No anti-dilution protection.
    None,
    /// Full ratchet: conversion price is reduced to the new issue price
    /// regardless of how many shares were issued. Most protective for holders.
    ///
    /// Formula: `new_conversion_price = min(current_conversion_price, new_issue_price)`
    FullRatchet,
    /// Broad-based weighted average: conversion price is adjusted based on the
    /// weighted average of the old and new share prices, factoring in the number
    /// of shares. Less dilutive to existing shareholders than full ratchet.
    ///
    /// Formula:
    /// ```text
    /// new_cp = old_cp × (shares_outstanding + new_money / old_cp)
    ///                  / (shares_outstanding + new_shares_issued)
    /// ```
    WeightedAverage,
}

/// How dividends affect conversion terms.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum DividendAdjustment {
    /// No dividend adjustment.
    None,
    /// Adjust conversion price downward by the dividend amount.
    AdjustPrice,
    /// Adjust conversion ratio upward to compensate for dividends.
    AdjustRatio,
}

/// A dilutive event that triggers anti-dilution adjustment.
///
/// Records details of an equity issuance or corporate action that may
/// affect the conversion ratio under the bond's anti-dilution provisions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct DilutionEvent {
    /// Date of the dilutive event.
    #[schemars(with = "String")]
    pub date: Date,
    /// New issue price per share (for below-market issuances).
    pub new_issue_price: f64,
    /// Number of new shares issued.
    pub new_shares_issued: f64,
    /// Number of shares outstanding before the event.
    pub shares_outstanding_before: f64,
}

/// Conversion specification for the instrument.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ConversionSpec {
    /// Conversion ratio (shares per bond). If not provided, derive from price.
    pub ratio: Option<f64>,
    /// Conversion price (price per share). If not provided, derive from ratio.
    pub price: Option<f64>,
    /// Policy governing conversion timing/conditions.
    pub policy: ConversionPolicy,
    /// Anti-dilution protection policy.
    pub anti_dilution: AntiDilutionPolicy,
    /// Dividend adjustment mechanism.
    pub dividend_adjustment: DividendAdjustment,
    /// Historical dilution events that affect the conversion ratio.
    /// Events are applied in chronological order.
    #[serde(default)]
    pub dilution_events: Vec<DilutionEvent>,
}

impl ConvertibleBond {
    /// Base conversion ratio (shares per bond) derived from explicit ratio or price.
    ///
    /// Returns `None` if neither `ratio` nor `price` is set on the conversion spec.
    pub fn conversion_ratio(&self) -> Option<f64> {
        if let Some(ratio) = self.conversion.ratio {
            Some(ratio)
        } else {
            self.conversion.price.map(|p| self.notional.amount() / p)
        }
    }

    /// Effective conversion ratio after anti-dilution adjustments.
    ///
    /// Applies all recorded [`DilutionEvent`]s in chronological order using
    /// the bond's [`AntiDilutionPolicy`]:
    ///
    /// - **None**: Returns the base conversion ratio unchanged.
    /// - **FullRatchet**: Conversion price is reduced to the lowest new issue
    ///   price across all dilution events. The ratio is then `notional / adjusted_price`.
    /// - **WeightedAverage**: Conversion price is adjusted using the broad-based
    ///   weighted average formula for each event sequentially.
    ///
    /// # Returns
    ///
    /// The adjusted conversion ratio, or `None` if neither ratio nor price is set.
    pub fn effective_conversion_ratio(&self) -> Option<f64> {
        let base_ratio = self.conversion_ratio()?;

        // If no anti-dilution or no events, return base ratio
        if matches!(self.conversion.anti_dilution, AntiDilutionPolicy::None)
            || self.conversion.dilution_events.is_empty()
        {
            return Some(base_ratio);
        }

        // Start with the original conversion price
        let notional = self.notional.amount();
        let mut current_cp = notional / base_ratio;

        // Sort events by date and apply sequentially
        let mut events = self.conversion.dilution_events.clone();
        events.sort_by_key(|e| e.date);

        for event in &events {
            match &self.conversion.anti_dilution {
                AntiDilutionPolicy::None => unreachable!(), // guarded above
                AntiDilutionPolicy::FullRatchet => {
                    // Full ratchet: conversion price drops to the new issue price
                    // if it is below the current conversion price.
                    if event.new_issue_price < current_cp {
                        current_cp = event.new_issue_price;
                    }
                }
                AntiDilutionPolicy::WeightedAverage => {
                    // Broad-based weighted average formula:
                    //   new_cp = old_cp × (O + new_money / old_cp) / (O + N)
                    // where:
                    //   O = shares outstanding before the event
                    //   N = new shares issued
                    //   new_money = N × new_issue_price
                    let o = event.shares_outstanding_before;
                    let n = event.new_shares_issued;
                    let new_money = n * event.new_issue_price;

                    if (o + n) > 0.0 {
                        let numerator = o + new_money / current_cp;
                        let denominator = o + n;
                        current_cp *= numerator / denominator;
                    }
                }
            }
        }

        // Conversion price cannot go below a small epsilon
        if current_cp < 1e-10 {
            return Some(base_ratio);
        }

        Some(notional / current_cp)
    }

    /// Create a canonical example convertible bond for testing and documentation.
    ///
    /// Returns a 5-year convertible with fixed coupon and voluntary conversion.
    pub fn example() -> finstack_core::Result<Self> {
        use crate::cashflow::builder::specs::FixedCouponSpec;
        use crate::cashflow::builder::CouponType;
        use finstack_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
        use time::macros::date;

        let coupon_rate = finstack_core::decimal::f64_to_decimal(0.02)?;

        ConvertibleBond::builder()
            .id(InstrumentId::new("CB-TECH-5Y"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .issue_date(date!(2024 - 01 - 15))
            .maturity(date!(2029 - 01 - 15))
            .discount_curve_id(CurveId::new("USD-IG"))
            .credit_curve_id_opt(Some(CurveId::new("USD-CREDIT-BBB")))
            .conversion(ConversionSpec {
                ratio: Some(25.0),
                price: None,
                policy: ConversionPolicy::Voluntary,
                anti_dilution: AntiDilutionPolicy::None,
                dividend_adjustment: DividendAdjustment::None,
                dilution_events: Vec::new(),
            })
            .underlying_equity_id_opt(Some("TECH".to_string()))
            .call_put_opt(None)
            .fixed_coupon_opt(Some(FixedCouponSpec {
                coupon_type: CouponType::Cash,
                rate: coupon_rate,
                freq: Tenor::semi_annual(),
                dc: DayCount::Thirty360,
                bdc: BusinessDayConvention::Following,
                calendar_id: "weekends_only".to_string(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: 0,
            }))
            .floating_coupon_opt(None)
            .attributes(Attributes::new())
            .build()
    }

    /// Create a mandatory convertible bond example (PERCS/DECS style).
    ///
    /// Returns a 3-year mandatory convertible with variable delivery ratio,
    /// soft-call trigger, and a call/put schedule:
    /// - **Notional:** $1M USD
    /// - **Coupon:** 5.0% semi-annual (higher coupon compensates for capped upside)
    /// - **Conversion:** Mandatory variable at maturity (DECS-style)
    ///   - Upper conversion price: $60 (above this, min shares delivered)
    ///   - Lower conversion price: $40 (below this, max shares delivered)
    /// - **Soft call:** 130% trigger, 20 of 30 trading days
    /// - **Call schedule:** Issuer can call at 101% after year 2
    /// - **Put schedule:** Holder can put at 100% after year 1
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example_mandatory() -> finstack_core::Result<Self> {
        use crate::cashflow::builder::specs::FixedCouponSpec;
        use crate::cashflow::builder::CouponType;
        use crate::instruments::fixed_income::bond::CallPut;
        use finstack_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
        use time::macros::date;

        let issue = date!(2024 - 03 - 15);
        let maturity = date!(2027 - 03 - 15);
        let coupon_rate = finstack_core::decimal::f64_to_decimal(0.05)?;

        ConvertibleBond::builder()
            .id(InstrumentId::new("CB-MAND-DECS-3Y"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .issue_date(issue)
            .maturity(maturity)
            .discount_curve_id(CurveId::new("USD-IG"))
            .credit_curve_id_opt(Some(CurveId::new("USD-CREDIT-BBB")))
            .conversion(ConversionSpec {
                ratio: None,
                price: Some(50.0), // reference price for base conversion ratio
                policy: ConversionPolicy::MandatoryVariable {
                    conversion_date: maturity,
                    upper_conversion_price: 60.0,
                    lower_conversion_price: 40.0,
                },
                anti_dilution: AntiDilutionPolicy::WeightedAverage,
                dividend_adjustment: DividendAdjustment::AdjustRatio,
                dilution_events: Vec::new(),
            })
            .underlying_equity_id_opt(Some("INDU".to_string()))
            .call_put_opt(Some(CallPutSchedule {
                calls: vec![CallPut {
                    start_date: date!(2026 - 03 - 15),
                    end_date: maturity,
                    price_pct_of_par: 101.0,
                    make_whole: None,
                }],
                puts: vec![CallPut {
                    start_date: date!(2025 - 03 - 15),
                    end_date: date!(2025 - 03 - 15),
                    price_pct_of_par: 100.0,
                    make_whole: None,
                }],
            }))
            .soft_call_trigger_opt(Some(SoftCallTrigger {
                threshold_pct: 130.0,
                observation_days: 30,
                required_days_above: 20,
            }))
            .recovery_rate_opt(Some(0.35))
            .fixed_coupon_opt(Some(FixedCouponSpec {
                coupon_type: CouponType::Cash,
                rate: coupon_rate,
                freq: Tenor::semi_annual(),
                dc: DayCount::Thirty360,
                bdc: BusinessDayConvention::Following,
                calendar_id: "weekends_only".to_string(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: 0,
            }))
            .floating_coupon_opt(None)
            .attributes(Attributes::new())
            .build()
    }

    /// Calculate parity ratio of this convertible bond
    pub fn parity(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
    ) -> finstack_core::Result<f64> {
        let underlying_id = self.underlying_equity_id.as_ref().ok_or_else(|| {
            finstack_core::Error::internal("convertible parity requires underlying_equity_id")
        })?;

        let spot_price = curves.get_price(underlying_id)?;
        let spot = match spot_price {
            finstack_core::market_data::scalars::MarketScalar::Price(money) => money.amount(),
            finstack_core::market_data::scalars::MarketScalar::Unitless(value) => *value,
        };

        Ok(pricer::calculate_parity(self, spot))
    }

    /// Calculate conversion premium of this convertible bond
    pub fn conversion_premium(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
        bond_price: f64,
    ) -> finstack_core::Result<f64> {
        let underlying_id = self.underlying_equity_id.as_ref().ok_or_else(|| {
            finstack_core::Error::internal(
                "convertible conversion premium requires underlying_equity_id",
            )
        })?;

        let spot_price = curves.get_price(underlying_id)?;
        let spot = match spot_price {
            finstack_core::market_data::scalars::MarketScalar::Price(money) => money.amount(),
            finstack_core::market_data::scalars::MarketScalar::Unitless(value) => *value,
        };

        // Use effective conversion ratio (includes anti-dilution adjustments)
        let conversion_ratio = self.effective_conversion_ratio().ok_or_else(|| {
            finstack_core::Error::internal(
                "convertible conversion premium requires effective conversion ratio",
            )
        })?;

        Ok(pricer::calculate_conversion_premium(
            bond_price,
            spot,
            conversion_ratio,
        ))
    }

    /// Calculate Greeks for this convertible bond
    pub fn greeks(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
        tree_type: Option<pricer::ConvertibleTreeType>,
        bump_size: Option<f64>,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<ConvertibleGreeks> {
        let greeks = pricer::calculate_convertible_greeks(
            self,
            curves,
            tree_type.unwrap_or_default(),
            bump_size,
            as_of,
        )?;
        Ok(greeks.into())
    }

    /// Calculate delta of this convertible bond
    pub fn delta(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<f64> {
        let greeks = self.greeks(curves, None, None, as_of)?;
        Ok(greeks.delta)
    }

    /// Calculate gamma of this convertible bond
    pub fn gamma(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<f64> {
        let greeks = self.greeks(curves, None, None, as_of)?;
        Ok(greeks.gamma)
    }

    /// Calculate vega of this convertible bond
    pub fn vega(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<f64> {
        let greeks = self.greeks(curves, None, None, as_of)?;
        Ok(greeks.vega)
    }

    /// Calculate rho of this convertible bond
    pub fn rho(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<f64> {
        let greeks = self.greeks(curves, None, None, as_of)?;
        Ok(greeks.rho)
    }

    /// Calculate theta of this convertible bond
    pub fn theta(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<f64> {
        let greeks = self.greeks(curves, None, None, as_of)?;
        Ok(greeks.theta)
    }

    fn vol_surface_dependency_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        if let Some(id) = self.attributes.get_meta("vol_surface_id") {
            ids.push(id.to_string());
        }
        if let Some(underlying_id) = &self.underlying_equity_id {
            ids.push(format!("{underlying_id}-VOL"));
            if let Some(stripped) = underlying_id.strip_suffix("-SPOT") {
                ids.push(format!("{stripped}-VOL"));
            }
        }
        ids
    }
}

impl crate::instruments::common_impl::traits::Instrument for ConvertibleBond {
    impl_instrument_base!(crate::pricer::InstrumentType::Convertible);

    fn market_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::dependencies::MarketDependencies>
    {
        let mut deps =
            crate::instruments::common_impl::dependencies::MarketDependencies::from_curve_dependencies(
                self,
            )?;
        if let Some(underlying_id) = &self.underlying_equity_id {
            deps.add_spot_id(underlying_id.as_str());
            for vol_surface_id in self.vol_surface_dependency_ids() {
                deps.add_vol_surface_id(vol_surface_id);
            }
        }
        Ok(deps)
    }

    fn base_value(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<finstack_core::money::Money> {
        if let Some(ref trigger) = self.soft_call_trigger {
            trigger.validate()?;
        }
        pricer::price_convertible_bond(self, curves, pricer::ConvertibleTreeType::default(), as_of)
    }

    fn effective_start_date(&self) -> Option<Date> {
        Some(self.issue_date)
    }

    fn model_params_snapshot(&self) -> ModelParamsSnapshot {
        ModelParamsSnapshot::Convertible {
            conversion_spec: self.conversion.clone(),
        }
    }

    fn with_model_params(
        &self,
        params: &ModelParamsSnapshot,
    ) -> finstack_core::Result<Box<dyn crate::instruments::common_impl::traits::Instrument>> {
        match params {
            ModelParamsSnapshot::Convertible { conversion_spec } => {
                let mut modified = self.clone();
                modified.conversion = conversion_spec.clone();
                Ok(Box::new(modified))
            }
            ModelParamsSnapshot::None => Ok(self.clone_box()),
            ModelParamsSnapshot::StructuredCredit { .. } => Err(Error::Validation(
                "Instrument type mismatch: expected ConvertibleBond model parameters".to_string(),
            )),
        }
    }

    fn pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&mut self.pricing_overrides)
    }

    fn pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&self.pricing_overrides)
    }
}

impl CashflowProvider for ConvertibleBond {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn cashflow_schedule(
        &self,
        _curves: &finstack_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<CashFlowSchedule> {
        let schedule = pricer::build_convertible_schedule(self)?;
        Ok(schedule.normalize_public(
            as_of,
            crate::cashflow::builder::CashflowRepresentation::Contractual,
        ))
    }
}

// Implement CurveDependencies for DV01 calculator
impl crate::instruments::common_impl::traits::CurveDependencies for ConvertibleBond {
    fn curve_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::traits::InstrumentCurves> {
        let builder = crate::instruments::common_impl::traits::InstrumentCurves::builder();
        let builder = builder.discount(self.discount_curve_id.clone());
        let builder = if let Some(credit_curve) = &self.credit_curve_id {
            builder.credit(credit_curve.clone())
        } else {
            builder
        };
        builder.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::CashflowProvider;

    #[test]
    fn test_cashflow_provider_matches_convertible_schedule_builder() {
        let bond = ConvertibleBond::example().expect("example should build");
        let market = finstack_core::market_data::context::MarketContext::new();
        let expected =
            super::pricer::build_convertible_schedule(&bond).expect("schedule should build");
        let actual = bond
            .cashflow_schedule(&market, bond.issue_date)
            .expect("provider schedule should build");

        assert_eq!(actual.flows, expected.flows);
        assert_eq!(actual.notional.initial, expected.notional.initial);
        assert_eq!(actual.day_count, expected.day_count);
    }
}
