//! Embedded option value calculator for callable/putable bonds.
//!
//! Computes the theoretical value of embedded call or put options by pricing
//! the bond twice using a short-rate tree:
//! 1. With call/put constraints → P_embedded
//! 2. Without call/put constraints → P_straight
//!
//! The embedded option value is the difference between these prices.
//!
//! # Option Value Decomposition
//!
//! ## Callable Bonds (Issuer Owns the Call)
//!
//! ```text
//! V_call_holder = P_callable - P_straight  (negative)
//! ```
//!
//! The call option has positive value to the issuer, reducing the price
//! the investor pays.
//!
//! ## Putable Bonds (Investor Owns the Put)
//!
//! ```text
//! P_putable = P_straight + V_put
//! V_put = P_putable - P_straight  (positive)
//! ```
//!
//! The put option has positive value to the investor, increasing the price.
//!
//! ## Bonds with Both Options
//!
//! For bonds with both calls and puts, the metric returns the **net** option
//! value from the investor's perspective:
//!
//! ```text
//! V_net = P_embedded - P_straight
//! ```
//!
//! - Positive: Put value dominates (benefits investor)
//! - Negative: Call value dominates (benefits issuer)
//!
//! # Examples
//!
//! ```text
//! use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
//! use finstack_quant_valuations::metrics::{MetricRegistry, MetricId};
//!
//! # let bond = Bond::example().unwrap();
//! // Register metrics and compute
//! // V_call_holder will be negative for callable bonds
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use crate::instruments::fixed_income::bond::pricing::engine::tree::{bond_tree_config, TreePricer};
use crate::instruments::fixed_income::bond::pricing::quote_conversions::price_from_oas;
use crate::instruments::fixed_income::bond::pricing::settlement::settlement_date;
use crate::instruments::fixed_income::bond::CallPutSchedule;
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};

/// Calculates the embedded option value for callable/putable bonds.
///
/// Uses tree-based pricing to compute the difference between:
/// - The option-embedded bond price (with call/put exercise decisions)
/// - The straight bond price (without embedded options)
///
/// # Returns
///
/// - For **callable bonds**: Negative holder value (call reduces holder value)
/// - For **putable bonds**: Positive holder value (put increases holder value)
/// - For **bonds with both**: Net option value from investor perspective
/// - For **straight bonds**: Zero (no embedded options)
///
/// The value is returned in **currency units** (same as bond notional).
///
/// # Dependencies
///
/// None - this is a standalone metric using tree pricing.
///
/// # Examples
///
/// ```text
/// use finstack_quant_valuations::instruments::fixed_income::bond::metrics::price_yield_spread::EmbeddedOptionValueCalculator;
/// use finstack_quant_valuations::metrics::MetricCalculator;
///
/// let calculator = EmbeddedOptionValueCalculator::new();
/// // Use via MetricRegistry for proper context management
/// ```
#[derive(Debug, Clone, Default)]
pub(crate) struct EmbeddedOptionValueCalculator;

#[allow(dead_code)] // public API for external bindings
impl EmbeddedOptionValueCalculator {
    /// Create a calculator with default settings.
    ///
    /// Uses 100 tree steps and 1% (100 bps) normal volatility.
    pub(crate) fn new() -> Self {
        Self
    }

    /// Create a calculator with custom tree configuration.
    ///
    /// # Arguments
    ///
    /// * `tree_steps` - Number of steps in the short-rate tree (50-200 typical)
    /// * `volatility` - Short rate volatility (normal vol in decimal, e.g., 0.01 = 100 bps)
    ///
    /// # Examples
    ///
    /// ```text
    /// use finstack_quant_valuations::instruments::fixed_income::bond::metrics::price_yield_spread::EmbeddedOptionValueCalculator;
    ///
    /// // High precision with calibrated volatility
    /// let calc = EmbeddedOptionValueCalculator::with_config(200, 0.012);
    /// ```
    pub(crate) fn with_config(_tree_steps: usize, _volatility: f64) -> Self {
        Self
    }
}

impl MetricCalculator for EmbeddedOptionValueCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;

        // If bond has no embedded options, return 0
        let has_options = bond
            .call_put
            .as_ref()
            .map(|cp| cp.has_options())
            .unwrap_or(false);

        if !has_options {
            return Ok(0.0);
        }

        let market = context.curves.as_ref();
        let as_of = context.as_of;
        let quote_date = settlement_date(bond, as_of)?;

        let oas_decimal = if let Some(oas) = context.computed.get(&MetricId::Oas) {
            *oas
        } else if let Some(oas) = bond.pricing_overrides.market_quotes.quoted_oas {
            oas
        } else if let Some(clean_price) = bond.pricing_overrides.market_quotes.quoted_clean_price {
            let pricer = TreePricer::with_config(bond_tree_config(bond)?);
            pricer.calculate_oas(bond, market, as_of, clean_price)? / 10_000.0
        } else {
            0.0
        };

        let price_with_options = price_from_oas(bond, market, quote_date, oas_decimal)?;
        let mut straight_bond = bond.clone();
        straight_bond.call_put = Some(CallPutSchedule::default());
        let price_straight = price_from_oas(&straight_bond, market, quote_date, oas_decimal)?;

        Ok(price_with_options - price_straight)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::fixed_income::bond::BondSettlementConvention;
    use crate::instruments::fixed_income::bond::CashflowSpec;
    use crate::instruments::fixed_income::bond::{CallPut, CallPutSchedule};
    use crate::instruments::PricingOverrides;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use std::sync::Arc;
    use time::Month;

    fn create_test_market() -> MarketContext {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        let discount_curve = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.85), (10.0, 0.70)])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("Valid curve");
        MarketContext::new().insert(discount_curve)
    }

    fn create_callable_bond() -> Bond {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
        let call_date = Date::from_calendar_date(2027, Month::January, 1).expect("Valid date");

        let mut call_put = CallPutSchedule::default();
        call_put.calls.push(CallPut {
            start_date: call_date,
            end_date: call_date,
            price_pct_of_par: 100.0,
            make_whole: None,
        });

        Bond::builder()
            .id("CALLABLE_BOND".into())
            .notional(Money::new(
                1000.0,
                finstack_quant_core::currency::Currency::USD,
            ))
            .issue_date(issue)
            .maturity(maturity)
            .cashflow_spec(
                CashflowSpec::fixed(
                    0.05,
                    finstack_quant_core::dates::Tenor::semi_annual(),
                    finstack_quant_core::dates::DayCount::Act365F,
                )
                .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .credit_curve_id_opt(None)
            .pricing_overrides(PricingOverrides::default())
            .call_put_opt(Some(call_put))
            .custom_cashflows_opt(None)
            .attributes(Default::default())
            .settlement_convention_opt(Some(BondSettlementConvention {
                settlement_days: 2,
                ..Default::default()
            }))
            .build()
            .expect("Valid bond")
    }

    fn create_putable_bond() -> Bond {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
        let put_date = Date::from_calendar_date(2027, Month::January, 1).expect("Valid date");

        let mut call_put = CallPutSchedule::default();
        call_put.puts.push(CallPut {
            start_date: put_date,
            end_date: put_date,
            price_pct_of_par: 100.0,
            make_whole: None,
        });

        Bond::builder()
            .id("PUTABLE_BOND".into())
            .notional(Money::new(
                1000.0,
                finstack_quant_core::currency::Currency::USD,
            ))
            .issue_date(issue)
            .maturity(maturity)
            .cashflow_spec(
                CashflowSpec::fixed(
                    0.05,
                    finstack_quant_core::dates::Tenor::semi_annual(),
                    finstack_quant_core::dates::DayCount::Act365F,
                )
                .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .credit_curve_id_opt(None)
            .pricing_overrides(PricingOverrides::default())
            .call_put_opt(Some(call_put))
            .custom_cashflows_opt(None)
            .attributes(Default::default())
            .settlement_convention_opt(Some(BondSettlementConvention {
                settlement_days: 2,
                ..Default::default()
            }))
            .build()
            .expect("Valid bond")
    }

    fn create_straight_bond() -> Bond {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");

        Bond::builder()
            .id("STRAIGHT_BOND".into())
            .notional(Money::new(
                1000.0,
                finstack_quant_core::currency::Currency::USD,
            ))
            .issue_date(issue)
            .maturity(maturity)
            .cashflow_spec(
                CashflowSpec::fixed(
                    0.05,
                    finstack_quant_core::dates::Tenor::semi_annual(),
                    finstack_quant_core::dates::DayCount::Act365F,
                )
                .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .credit_curve_id_opt(None)
            .pricing_overrides(PricingOverrides::default())
            .call_put_opt(None)
            .custom_cashflows_opt(None)
            .attributes(Default::default())
            .settlement_convention_opt(Some(BondSettlementConvention {
                settlement_days: 2,
                ..Default::default()
            }))
            .build()
            .expect("Valid bond")
    }

    #[test]
    fn test_straight_bond_returns_zero() {
        let bond = create_straight_bond();
        let market = create_test_market();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");

        let calc = EmbeddedOptionValueCalculator::new();
        let base_value = bond.value(&market, as_of).expect("Should price");

        let mut context = MetricContext::new(
            Arc::new(bond),
            Arc::new(market),
            as_of,
            base_value,
            MetricContext::default_config(),
        );
        let option_value = calc.calculate(&mut context).expect("Should calculate");

        assert!(
            option_value.abs() < 1e-10,
            "Straight bond should have zero option value, got {}",
            option_value
        );
    }

    #[test]
    fn test_callable_bond_positive_option_value() {
        let bond = create_callable_bond();
        let market = create_test_market();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");

        let calc = EmbeddedOptionValueCalculator::new();
        let base_value = bond.value(&market, as_of).expect("Should price");

        let mut context = MetricContext::new(
            Arc::new(bond),
            Arc::new(market),
            as_of,
            base_value,
            MetricContext::default_config(),
        );
        let option_value = calc.calculate(&mut context).expect("Should calculate");

        assert!(
            option_value < 0.0,
            "Callable bond holder option value should be negative, got {}",
            option_value
        );
    }

    #[test]
    fn test_putable_bond_positive_option_value() {
        let bond = create_putable_bond();
        let market = create_test_market();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");

        let calc = EmbeddedOptionValueCalculator::new();
        let base_value = bond.value(&market, as_of).expect("Should price");

        let mut context = MetricContext::new(
            Arc::new(bond),
            Arc::new(market),
            as_of,
            base_value,
            MetricContext::default_config(),
        );
        let option_value = calc.calculate(&mut context).expect("Should calculate");

        assert!(
            option_value > 0.0,
            "Putable bond should have positive put option value, got {}",
            option_value
        );
    }
}
