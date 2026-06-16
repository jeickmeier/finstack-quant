//! Commodity spread option instrument definition.
//!
//! A spread option pays max(S1 - S2 - K, 0) for calls, where S1 and S2 are
//! two commodity prices (e.g., crack spread between crude oil and gasoline,
//! or spark spread between gas and electricity).
//!
//! # Pricing
//!
//! Uses Kirk's approximation (1995), which reduces the two-asset spread to a
//! single-asset Black-76 problem with adjusted volatility incorporating the
//! correlation between the two underlying commodities.
//!
//! # References
//!
//! - Kirk, E. (1995). "Correlation in the Energy Markets." Managing Energy
//!   Price Risk, Risk Publications.

use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::{
    Attributes, CurveDependencies, Instrument, InstrumentCurves,
};
use crate::instruments::{OptionType, PricingOverrides};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_core::Result;

/// Commodity spread option: option on the price difference between two commodities.
///
/// Pays max(S1 - S2 - K, 0) for calls, max(K - (S1 - S2), 0) for puts.
///
/// # Kirk's Approximation
///
/// The spread option is priced using Kirk's approximation, which maps the
/// two-asset problem into a single-asset Black-76 framework:
///
/// 1. Forward prices F1, F2 from respective price curves
/// 2. Adjusted strike: K_adj = F2 + K
/// 3. Kirk's volatility:
///    sigma_kirk = sqrt(sigma1^2 - 2*rho*sigma1*sigma2*F2/(F2+K) + (sigma2*F2/(F2+K))^2)
/// 4. Price via Black-76 on F1 vs K_adj with sigma_kirk
///
/// For puts, put-call parity is used: P = C - DF * (F1 - F2 - K)
///
/// # Correlation
///
/// The `correlation` parameter captures the co-movement between the two
/// commodity prices. Higher correlation reduces the effective spread volatility
/// and hence the option price. The correlation must be in [-1, 1].
///
/// # Example
///
/// ```ignore
/// use finstack_quant_valuations::instruments::commodity::CommoditySpreadOption;
/// use finstack_quant_valuations::instruments::OptionType;
/// use finstack_quant_core::types::{CurveId, InstrumentId};
/// use time::macros::date;
///
/// let spread_opt = CommoditySpreadOption::builder()
///     .id(InstrumentId::new("WTI-RBOB-CRACK-SPREAD"))
///     .currency(finstack_quant_core::currency::Currency::USD)
///     .option_type(OptionType::Call)
///     .expiry(date!(2025-06-15))
///     .strike(10.0)           // $10/bbl crack spread strike
///     .notional(1000.0)
///     .leg1_forward_curve_id(CurveId::new("RBOB-FORWARD"))
///     .leg2_forward_curve_id(CurveId::new("WTI-FORWARD"))
///     .leg1_vol_surface_id(CurveId::new("RBOB-VOL"))
///     .leg2_vol_surface_id(CurveId::new("WTI-VOL"))
///     .discount_curve_id(CurveId::new("USD-OIS"))
///     .correlation(0.85)
///     .build()
///     .expect("Valid spread option");
/// ```
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[builder(validate = CommoditySpreadOption::validate)]
pub struct CommoditySpreadOption {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Settlement currency.
    pub currency: Currency,
    /// Option type (call or put on the spread S1 - S2).
    pub option_type: OptionType,
    /// Option expiry date.
    #[schemars(with = "String")]
    pub expiry: Date,
    /// Spread strike price K in the payoff max(S1 - S2 - K, 0).
    pub strike: f64,
    /// Notional quantity (number of units).
    pub notional: f64,
    /// Forward/price curve ID for leg 1 (the "long" commodity).
    pub leg1_forward_curve_id: CurveId,
    /// Forward/price curve ID for leg 2 (the "short" commodity).
    pub leg2_forward_curve_id: CurveId,
    /// Volatility surface ID for leg 1.
    pub leg1_vol_surface_id: CurveId,
    /// Volatility surface ID for leg 2.
    pub leg2_vol_surface_id: CurveId,
    /// Discount curve ID for present value.
    pub discount_curve_id: CurveId,
    /// Correlation between the two commodity prices, in [-1, 1].
    pub correlation: f64,
    /// Day count convention for time to expiry.
    #[serde(default = "crate::serde_defaults::day_count_act365f")]
    #[builder(default = DayCount::Act365F)]
    pub day_count: DayCount,
    /// Pricing overrides (implied vol, etc.).
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: PricingOverrides,
    /// Attributes for scenario selection and tagging.
    #[builder(default)]
    #[serde(default)]
    pub attributes: Attributes,
    /// Rejects unknown JSON fields (restores `deny_unknown_fields` despite the
    /// `#[serde(flatten)]` on `underlying`).
    #[serde(flatten)]
    #[schemars(skip)]
    #[builder(default)]
    pub(crate) unknown_fields: crate::instruments::common_impl::serde_guard::UnknownFieldGuard,
}

impl CommoditySpreadOption {
    /// Create a representative WTI-RBOB crack spread call option example.
    ///
    /// $5 strike, 10,000 bbl notional, 0.85 correlation, European-style.
    pub fn example() -> finstack_quant_core::Result<Self> {
        Self::builder()
            .id(InstrumentId::new("WTI-RBOB-CRACK-SPREAD"))
            .currency(Currency::USD)
            .option_type(OptionType::Call)
            .expiry(time::macros::date!(2025 - 09 - 15))
            .strike(5.0)
            .notional(10_000.0)
            .leg1_forward_curve_id(CurveId::new("RBOB-FORWARD"))
            .leg2_forward_curve_id(CurveId::new("WTI-FORWARD"))
            .leg1_vol_surface_id(CurveId::new("RBOB-VOL"))
            .leg2_vol_surface_id(CurveId::new("WTI-VOL"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .correlation(0.85)
            .day_count(DayCount::Act365F)
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
    }

    /// Time to expiry in year fractions.
    pub(crate) fn time_to_expiry(&self, as_of: Date) -> Result<f64> {
        self.day_count
            .year_fraction(as_of, self.expiry, DayCountContext::default())
            .map(|t| t.max(0.0))
    }

    /// Get forward price for leg 1 from the price curve.
    pub(crate) fn leg1_forward(&self, market: &MarketContext) -> Result<f64> {
        let price_curve = market.get_price_curve(self.leg1_forward_curve_id.as_str())?;
        price_curve.price_on_date(self.expiry)
    }

    /// Get forward price for leg 2 from the price curve.
    pub(crate) fn leg2_forward(&self, market: &MarketContext) -> Result<f64> {
        let price_curve = market.get_price_curve(self.leg2_forward_curve_id.as_str())?;
        price_curve.price_on_date(self.expiry)
    }

    /// Validate correlation is finite and within [-1, 1].
    pub(crate) fn validate(&self) -> Result<()> {
        if !self.correlation.is_finite() || !(-1.0..=1.0).contains(&self.correlation) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CommoditySpreadOption correlation must be a finite value in [-1, 1], got {}",
                self.correlation
            )));
        }
        Ok(())
    }
}

impl CurveDependencies for CommoditySpreadOption {
    fn curve_dependencies(&self) -> finstack_quant_core::Result<InstrumentCurves> {
        InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .forward(self.leg1_forward_curve_id.clone())
            .build()
    }
}

impl Instrument for CommoditySpreadOption {
    impl_instrument_base!(crate::pricer::InstrumentType::CommoditySpreadOption);

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::Black76
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps =
            crate::instruments::common_impl::dependencies::MarketDependencies::from_curve_dependencies(
                self,
            )?;
        deps.add_vol_surface_id(self.leg1_vol_surface_id.as_str());
        deps.add_vol_surface_id(self.leg2_vol_surface_id.as_str());
        // Add leg2 forward curve as an additional forward dependency
        let leg2_curves = InstrumentCurves::builder()
            .forward(self.leg2_forward_curve_id.clone())
            .build()?;
        deps.add_curves(leg2_curves);
        Ok(deps)
    }

    fn base_value(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        crate::instruments::commodity::commodity_spread_option::pricer::compute_pv(
            self, market, as_of,
        )
    }

    fn effective_start_date(&self) -> Option<Date> {
        None
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

crate::impl_empty_cashflow_provider!(
    CommoditySpreadOption,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);
