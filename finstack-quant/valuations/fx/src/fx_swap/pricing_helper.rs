//! Shared pricing helper for FX Swap calculations.
//!
//! Centralizes the CIP forward rate calculation and PV decomposition logic
//! to ensure consistency across the main pricer and all metric calculators.

use finstack_quant_core::Result;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;

use super::FxSwap;

/// Resolved market data and computed values for FX swap pricing.
///
/// This struct captures all the intermediate values needed for PV calculation
/// and risk metrics, ensuring consistent computation across the codebase.
#[derive(Debug, Clone)]
pub(crate) struct FxSwapPricingContext {
    /// Domestic discount factor from as_of to near_date
    pub(crate) df_dom_near: f64,
    /// Domestic discount factor from as_of to far_date
    pub(crate) df_dom_far: f64,
    /// Foreign discount factor from as_of to near_date
    pub(crate) df_for_near: f64,
    /// Foreign discount factor from as_of to far_date
    pub(crate) df_for_far: f64,
    /// Model spot rate from FX matrix (quote per base)
    pub(crate) model_spot: f64,
    /// Contract near rate (explicit or as-of-to-near CIP forward)
    pub(crate) contract_near_rate: f64,
    /// Contract far rate (explicit or model forward)
    pub(crate) contract_far_rate: f64,
    /// Whether near leg should be included (near_date >= as_of)
    pub(crate) include_near: bool,
    /// Whether far leg should be included (far_date >= as_of)
    pub(crate) include_far: bool,
    /// Base notional amount
    pub(crate) notional: f64,
}

impl FxSwapPricingContext {
    /// Build pricing context from market data and instrument.
    ///
    /// # Arguments
    /// * `swap` - Validated FX swap with base-currency notional and quote-per-base contract rates.
    /// * `curves` - Market snapshot containing quote/base discount curves and current FX spot.
    /// * `as_of` - Valuation date from which settlement discount factors and omitted outright forwards are resolved.
    ///
    /// # Errors
    /// Returns error if:
    /// - Required discount curves are missing
    /// - FX matrix is missing
    /// - Discount factors are near-zero (degenerate market data)
    pub(crate) fn build(swap: &FxSwap, curves: &MarketContext, as_of: Date) -> Result<Self> {
        let domestic_disc = curves.get_discount(swap.domestic_discount_curve_id.as_str())?;
        let foreign_disc = curves.get_discount(swap.foreign_discount_curve_id.as_str())?;

        // Settlement checks
        let include_near = swap.near_date >= as_of;
        let include_far = swap.far_date >= as_of;

        let (df_dom_near, df_for_near) = if include_near {
            (
                domestic_disc.df_between_dates(as_of, swap.near_date)?,
                foreign_disc.df_between_dates(as_of, swap.near_date)?,
            )
        } else {
            // Near leg has settled - use 1.0 as placeholder since include_near is false
            (1.0, 1.0)
        };
        let (df_dom_far, df_for_far) = if include_far {
            (
                domestic_disc.df_between_dates(as_of, swap.far_date)?,
                foreign_disc.df_between_dates(as_of, swap.far_date)?,
            )
        } else {
            // Far leg has settled - use 1.0 as placeholder since include_far is false
            (1.0, 1.0)
        };

        // Resolve model spot from FX matrix. Explicit swap contract rates define
        // exchanged cashflows; they are not valid substitutes for market spot.
        let missing_msg = format!(
            "FxSwap {} requires FxMatrix market data for {}/{}; near_rate/far_rate are contract terms and cannot be used as synthetic model spot",
            swap.id, swap.base_currency, swap.quote_currency
        );
        let model_spot = crate::market_inputs::resolve_fx_matrix_spot(
            curves,
            swap.base_currency,
            swap.quote_currency,
            as_of,
            None,
            Some(&missing_msg),
        )?;

        let model_near = crate::market_inputs::cip_forward_from_dfs(
            model_spot,
            df_for_near,
            df_dom_near,
            "FxSwap",
        )?;
        let model_forward = crate::market_inputs::cip_forward_from_dfs(
            model_spot, df_for_far, df_dom_far, "FxSwap",
        )?;

        let contract_near_rate = swap.near_rate.unwrap_or(model_near);
        let contract_far_rate = swap.far_rate.unwrap_or(model_forward);

        let notional = swap.notional.amount();

        Ok(Self {
            df_dom_near,
            df_dom_far,
            df_for_near,
            df_for_far,
            model_spot,
            contract_near_rate,
            contract_far_rate,
            include_near,
            include_far,
            notional,
        })
    }

    /// Calculate PV of the foreign leg in base currency.
    ///
    /// Foreign leg: receive base currency at near, pay base currency at far.
    pub(crate) fn pv_foreign_leg_base(&self) -> f64 {
        let mut pv = 0.0;
        if self.include_near {
            pv += self.notional * self.df_for_near;
        }
        if self.include_far {
            pv -= self.notional * self.df_for_far;
        }
        pv
    }

    /// Calculate total PV in quote (domestic) currency.
    ///
    /// Converts foreign leg to quote currency at model spot and adds domestic leg.
    pub(crate) fn total_pv(&self) -> f64 {
        let pv_foreign_dom = self.pv_foreign_leg_base() * self.model_spot;
        let mut pv_dom_leg = 0.0;
        if self.include_near {
            pv_dom_leg -= self.notional * self.contract_near_rate * self.df_dom_near;
        }
        if self.include_far {
            pv_dom_leg += self.notional * self.contract_far_rate * self.df_dom_far;
        }
        pv_foreign_dom + pv_dom_leg
    }

    /// Calculate forward points (far_rate - near_rate).
    pub(crate) fn forward_points(&self) -> f64 {
        self.contract_far_rate - self.contract_near_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn future_near_defaults_use_date_based_outright_forwards() {
        use finstack_quant_core::types::Attributes;
        use finstack_quant_core::{
            currency::Currency,
            market_data::term_structures::DiscountCurve,
            money::{
                Money,
                fx::{FxMatrix, SimpleFxProvider},
            },
            types::InstrumentId,
        };
        use finstack_quant_valuations_core::instruments::{Instrument, PricingOptions};
        use finstack_quant_valuations_core::metrics::MetricId;
        use std::sync::Arc;
        use time::macros::date;

        let as_of = date!(2025 - 01 - 01);
        let near = date!(2026 - 01 - 01);
        let far = date!(2027 - 01 - 01);
        let usd = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.05_f64).exp()),
                (2.0, (-0.10_f64).exp()),
            ])
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("USD curve");
        let eur = DiscountCurve::builder("EUR-OIS")
            .base_date(as_of)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.02_f64).exp()),
                (2.0, (-0.04_f64).exp()),
            ])
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("EUR curve");
        let near_forward = 1.1 * eur.df_between_dates(as_of, near).expect("near EUR DF")
            / usd.df_between_dates(as_of, near).expect("near USD DF");
        let far_forward = 1.1 * eur.df_between_dates(as_of, far).expect("far EUR DF")
            / usd.df_between_dates(as_of, far).expect("far USD DF");
        let provider = SimpleFxProvider::new();
        provider
            .set_quotes(&[(Currency::EUR, Currency::USD, 1.1)])
            .expect("FX quote");
        let market = MarketContext::new()
            .try_insert(usd)
            .expect("insert market curve")
            .try_insert(eur)
            .expect("insert market curve")
            .insert_fx(FxMatrix::new(Arc::new(provider), None));
        let mut swap = FxSwap::builder()
            .id(InstrumentId::new("FUTURE-NEAR"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .near_date(near)
            .far_date(far)
            .notional(Money::from((1_000_000_i64, Currency::EUR)))
            .domestic_discount_curve_id("USD-OIS".into())
            .foreign_discount_curve_id("EUR-OIS".into())
            .attributes(Attributes::new())
            .build()
            .expect("swap");
        let ctx = FxSwapPricingContext::build(&swap, &market, as_of).expect("pricing context");
        assert!((ctx.contract_near_rate - near_forward).abs() < 1e-12);
        assert!((ctx.contract_far_rate - far_forward).abs() < 1e-12);
        assert!(swap.value_raw(&market, as_of).expect("default PV").abs() < 1e-8);
        let result = swap
            .price_with_metrics(
                &market,
                as_of,
                &[
                    MetricId::Dv01Domestic,
                    MetricId::Dv01Foreign,
                    MetricId::Fx01,
                ],
                PricingOptions::default(),
            )
            .expect("risk");
        for value in result.measures.values() {
            assert!(
                value.abs() < 1e-8,
                "market-default repricing remains par: {value}"
            );
        }
        swap.near_rate = Some(near_forward);
        assert!(swap.value_raw(&market, as_of).expect("fixed near PV").abs() < 1e-8);
        swap.far_rate = Some(far_forward);
        assert!(
            swap.value_raw(&market, as_of)
                .expect("explicit fair PV")
                .abs()
                < 1e-8
        );
    }

    #[test]
    fn test_cip_forward_calculation() {
        // Test: r_dom = 5%, r_for = 0.5%, T = 1 year
        // DF_dom = exp(-0.05) ≈ 0.9512, DF_for = exp(-0.005) ≈ 0.995
        // F = S × DF_for / DF_dom = 1.0 × 0.995 / 0.9512 ≈ 1.046
        let spot = 1.0;
        let df_dom_far = 0.9512;
        let df_for_far = 0.995;

        let forward =
            crate::market_inputs::cip_forward_from_dfs(spot, df_for_far, df_dom_far, "FxSwap")
                .unwrap();

        // Forward should be at premium when r_dom > r_for
        assert!(forward > spot, "Forward should be > spot");
        assert!(
            (forward - 1.046).abs() < 0.001,
            "Forward should be ~1.046, got {}",
            forward
        );
    }

    #[test]
    fn test_cip_forward_rejects_zero_df() {
        let result = crate::market_inputs::cip_forward_from_dfs(1.0, 0.99, 0.0, "FxSwap");
        assert!(result.is_err(), "Should reject zero domestic DF");

        let result = crate::market_inputs::cip_forward_from_dfs(1.0, 0.0, 0.95, "FxSwap");
        assert!(result.is_err(), "Should reject zero foreign DF");
    }

    /// Item 5 verification: `total_pv` must equal a full four-cashflow,
    /// per-currency discounting of an *off-market* FX swap.
    ///
    /// The four cashflows are: `+N` base @near, `−N·near_rate` quote @near,
    /// `−N` base @far, `+N·far_rate` quote @far. Discounting the base flows on
    /// the foreign curve and the quote flows on the domestic curve, then
    /// converting the (already present-valued) base leg to quote at *today's*
    /// spot, is the exact PV — converting a present value never requires a
    /// future spot. This test pins that equivalence so the audit's "approximation"
    /// concern is resolved by construction; no correction was required.
    #[test]
    fn total_pv_equals_four_cashflow_per_currency_discounting() {
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::market_data::context::MarketContext;
        use finstack_quant_core::market_data::term_structures::DiscountCurve;
        use finstack_quant_core::money::Money;
        use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
        use finstack_quant_core::types::Attributes;
        use finstack_quant_core::types::{CurveId, InstrumentId};
        use std::sync::Arc;
        use time::macros::date;

        let as_of = date!(2025 - 01 - 15);
        let near_date = date!(2025 - 01 - 17);
        let far_date = date!(2025 - 07 - 17);

        let usd = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (0.5, 0.9760), (1.0, 0.9520)])
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("usd curve");
        let eur = DiscountCurve::builder("EUR-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (0.5, 0.9900), (1.0, 0.9810)])
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("eur curve");
        let provider = SimpleFxProvider::new();
        provider
            .set_quotes(&[(Currency::EUR, Currency::USD, 1.10)])
            .expect("valid rate");
        let market = MarketContext::new()
            .try_insert(usd)
            .expect("insert market curve")
            .try_insert(eur)
            .expect("insert market curve")
            .insert_fx(FxMatrix::new(Arc::new(provider), None));

        // Off-market swap: both legs priced away from model spot/forward.
        let swap = FxSwap::builder()
            .id(InstrumentId::new("EURUSD-SWAP-OFFMKT"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .near_date(near_date)
            .far_date(far_date)
            .notional(Money::from((10_000_000_i64, Currency::EUR)))
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .near_rate_opt(Some(1.0850))
            .far_rate_opt(Some(1.1300))
            .attributes(Attributes::new())
            .build()
            .expect("off-market swap");

        let ctx = FxSwapPricingContext::build(&swap, &market, as_of).expect("ctx");
        let total_pv = ctx.total_pv();

        // Explicit four-cashflow, per-currency discounting.
        let n = swap.notional.amount();
        let domestic = market.get_discount("USD-OIS").expect("usd");
        let foreign = market.get_discount("EUR-OIS").expect("eur");
        let df_dom_near = domestic
            .df_between_dates(as_of, near_date)
            .expect("df dom near");
        let df_dom_far = domestic
            .df_between_dates(as_of, far_date)
            .expect("df dom far");
        let df_for_near = foreign
            .df_between_dates(as_of, near_date)
            .expect("df for near");
        let df_for_far = foreign
            .df_between_dates(as_of, far_date)
            .expect("df for far");

        // Base-currency leg, present-valued on the foreign curve.
        let pv_base_leg = n * df_for_near - n * df_for_far;
        // Quote-currency leg, present-valued on the domestic curve.
        let pv_quote_leg = -n * 1.0850 * df_dom_near + n * 1.1300 * df_dom_far;
        // Convert the present-valued base leg to quote at today's spot.
        let expected = pv_base_leg * 1.10 + pv_quote_leg;

        assert!(
            (total_pv - expected).abs() < 1e-6,
            "total_pv must equal four-cashflow per-currency discounting: \
             total_pv={total_pv} expected={expected}"
        );
    }

    #[test]
    fn settled_fx_swap_metrics_and_details_succeed() {
        use finstack_quant_core::types::Attributes;
        use finstack_quant_core::{
            currency::Currency,
            market_data::term_structures::DiscountCurve,
            money::{
                Money,
                fx::{FxMatrix, SimpleFxProvider},
            },
            types::InstrumentId,
        };
        use finstack_quant_valuations_core::instruments::{Instrument, PricingOptions};
        use finstack_quant_valuations_core::metrics::MetricId;
        use finstack_quant_valuations_core::results::{FxValuationDetails, ValuationDetails};
        use std::sync::Arc;
        use time::macros::date;

        let near = date!(2025 - 01 - 05);
        let far = date!(2025 - 06 - 05);
        let as_of = date!(2025 - 07 - 01); // Post-settlement
        let usd = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, (-0.04_f64).exp())])
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("usd");
        let eur = DiscountCurve::builder("EUR-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, (-0.02_f64).exp())])
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("eur");
        let provider = SimpleFxProvider::new();
        provider
            .set_quotes(&[(Currency::EUR, Currency::USD, 1.10)])
            .expect("quote");
        let market = MarketContext::new()
            .try_insert(usd)
            .expect("usd")
            .try_insert(eur)
            .expect("eur")
            .insert_fx(FxMatrix::new(Arc::new(provider), None));

        let swap = FxSwap::builder()
            .id(InstrumentId::new("SETTLED-SWAP"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .near_date(near)
            .far_date(far)
            .notional(Money::from((1_000_000_i64, Currency::EUR)))
            .domestic_discount_curve_id("USD-OIS".into())
            .foreign_discount_curve_id("EUR-OIS".into())
            .attributes(Attributes::new())
            .build()
            .expect("swap");

        assert_eq!(swap.fx_exposure(), Some((Currency::EUR, Currency::USD)));
        let result = swap
            .price_with_metrics(
                &market,
                as_of,
                &[
                    MetricId::custom("carry_pv"),
                    MetricId::custom("forward_points"),
                    MetricId::Dv01Domestic,
                    MetricId::Dv01Foreign,
                ],
                PricingOptions::default(),
            )
            .expect("settled FX swap metrics must succeed");
        assert_eq!(result.value.amount(), 0.0);
        for (id, &val) in &result.measures {
            assert_eq!(val, 0.0, "settled swap metric {id:?} must be 0.0");
        }
        assert!(matches!(
            result.details,
            Some(ValuationDetails::Fx(FxValuationDetails {
                fx_triangulated: Some(false)
            }))
        ));
    }
}
