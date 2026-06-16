//! Shared pricing helper for FX Swap calculations.
//!
//! Centralizes the CIP forward rate calculation and PV decomposition logic
//! to ensure consistency across the main pricer and all metric calculators.

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::fx::FxQuery;
use finstack_quant_core::Result;

use super::FxSwap;

/// Minimum threshold for discount factor denominators to avoid division by zero.
/// Values below this trigger an error rather than silent fallback.
const DF_NEAR_ZERO_THRESHOLD: f64 = 1e-12;

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
    /// Model forward rate via CIP (quote per base)
    pub(crate) model_forward: f64,
    /// Contract near rate (explicit or model spot)
    pub(crate) contract_near_rate: f64,
    /// Contract far rate (explicit or model forward)
    pub(crate) contract_far_rate: f64,
    /// Whether near leg should be included (near_date >= as_of)
    pub(crate) include_near: bool,
    /// Whether far leg should be included (far_date >= as_of)
    pub(crate) include_far: bool,
    /// Base notional amount
    pub(crate) base_notional: f64,
}

impl FxSwapPricingContext {
    /// Build pricing context from market data and instrument.
    ///
    /// # Arguments
    /// * `swap` - The FX swap instrument
    /// * `curves` - Market context with discount curves and FX matrix
    /// * `as_of` - Valuation date
    ///
    /// # Errors
    /// Returns error if:
    /// - Required discount curves are missing
    /// - FX matrix is missing
    /// - Discount factors are near-zero (degenerate market data)
    /// - Contract rates are non-positive when explicitly provided
    pub(crate) fn build(swap: &FxSwap, curves: &MarketContext, as_of: Date) -> Result<Self> {
        // Validate explicit contract rates if provided
        if let Some(rate) = swap.near_rate {
            if rate <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "near_rate must be positive, got: {}",
                    rate
                )));
            }
        }
        if let Some(rate) = swap.far_rate {
            if rate <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "far_rate must be positive, got: {}",
                    rate
                )));
            }
        }

        // Get discount curves
        let domestic_disc = curves.get_discount(swap.domestic_discount_curve_id.as_str())?;
        let foreign_disc = curves.get_discount(swap.foreign_discount_curve_id.as_str())?;

        // Settlement checks
        let include_near = swap.near_date >= as_of;
        let include_far = swap.far_date >= as_of;

        // Calculate discount factors
        let (df_dom_near, df_for_near) = if include_near {
            (
                domestic_disc.df_between_dates(as_of, swap.near_date)?,
                foreign_disc.df_between_dates(as_of, swap.near_date)?,
            )
        } else {
            // Near leg has settled - use 1.0 as placeholder since include_near is false
            (1.0, 1.0)
        };
        let df_dom_far = domestic_disc.df_between_dates(as_of, swap.far_date)?;
        let df_for_far = foreign_disc.df_between_dates(as_of, swap.far_date)?;

        // Resolve model spot from FX matrix. Explicit swap contract rates define
        // exchanged cashflows; they are not valid substitutes for market spot.
        let model_spot = if let Some(fx) = curves.fx() {
            (**fx)
                .rate(FxQuery::new(swap.base_currency, swap.quote_currency, as_of))?
                .rate
        } else {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FxSwap {} requires FxMatrix market data for {}/{}; near_rate/far_rate are contract terms and cannot be used as synthetic model spot",
                swap.id, swap.base_currency, swap.quote_currency
            )));
        };

        // Calculate model forward via covered interest rate parity
        let model_forward = Self::calculate_cip_forward(
            model_spot,
            df_dom_near,
            df_dom_far,
            df_for_near,
            df_for_far,
        )?;

        // Contract rates default to model when not explicitly provided
        let contract_near_rate = swap.near_rate.unwrap_or(model_spot);
        let contract_far_rate = swap.far_rate.unwrap_or(model_forward);

        let base_notional = swap.base_notional.amount();

        Ok(Self {
            df_dom_near,
            df_dom_far,
            df_for_near,
            df_for_far,
            model_spot,
            model_forward,
            contract_near_rate,
            contract_far_rate,
            include_near,
            include_far,
            base_notional,
        })
    }

    /// Calculate forward rate via Covered Interest Rate Parity.
    ///
    /// Formula: F = S × (DF_for_far / DF_for_near) / (DF_dom_far / DF_dom_near)
    ///
    /// When r_dom > r_for, forward is at premium (F > S) as required by no-arbitrage.
    ///
    /// # Arguments
    /// * `spot` - Current spot rate (quote per base)
    /// * `df_dom_near` - Domestic DF to near date
    /// * `df_dom_far` - Domestic DF to far date
    /// * `df_for_near` - Foreign DF to near date
    /// * `df_for_far` - Foreign DF to far date
    ///
    /// # Errors
    /// Returns error if near-date discount factors are near-zero.
    pub(crate) fn calculate_cip_forward(
        spot: f64,
        df_dom_near: f64,
        df_dom_far: f64,
        df_for_near: f64,
        df_for_far: f64,
    ) -> Result<f64> {
        // Validate denominators to avoid silent incorrect results
        if df_dom_near.abs() < DF_NEAR_ZERO_THRESHOLD {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Domestic discount factor at near date is near-zero ({}), cannot compute forward",
                df_dom_near
            )));
        }
        if df_for_near.abs() < DF_NEAR_ZERO_THRESHOLD {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Foreign discount factor at near date is near-zero ({}), cannot compute forward",
                df_for_near
            )));
        }

        let dom_ratio = df_dom_far / df_dom_near;
        let for_ratio = df_for_far / df_for_near;
        let forward = spot * for_ratio / dom_ratio;

        Ok(forward)
    }

    /// Calculate PV of the foreign leg in base currency.
    ///
    /// Foreign leg: receive base currency at near, pay base currency at far.
    pub(crate) fn pv_foreign_leg_base(&self) -> f64 {
        let mut pv = 0.0;
        if self.include_near {
            pv += self.base_notional * self.df_for_near;
        }
        if self.include_far {
            pv -= self.base_notional * self.df_for_far;
        }
        pv
    }

    /// Calculate PV of the foreign leg in base currency with custom DFs.
    pub(crate) fn pv_foreign_leg_base_with_dfs(&self, df_for_near: f64, df_for_far: f64) -> f64 {
        let mut pv = 0.0;
        if self.include_near {
            pv += self.base_notional * df_for_near;
        }
        if self.include_far {
            pv -= self.base_notional * df_for_far;
        }
        pv
    }

    /// Calculate PV of the domestic leg in quote currency using contract rates.
    ///
    /// Domestic leg: pay quote currency at near, receive quote currency at far.
    pub(crate) fn pv_domestic_leg(&self) -> f64 {
        let mut pv = 0.0;
        if self.include_near {
            pv -= self.base_notional * self.contract_near_rate * self.df_dom_near;
        }
        if self.include_far {
            pv += self.base_notional * self.contract_far_rate * self.df_dom_far;
        }
        pv
    }

    /// Calculate PV of the domestic leg with custom DFs and rates.
    pub(crate) fn pv_domestic_leg_with_params(
        &self,
        near_rate: f64,
        far_rate: f64,
        df_dom_near: f64,
        df_dom_far: f64,
    ) -> f64 {
        let mut pv = 0.0;
        if self.include_near {
            pv -= self.base_notional * near_rate * df_dom_near;
        }
        if self.include_far {
            pv += self.base_notional * far_rate * df_dom_far;
        }
        pv
    }

    /// Calculate total PV in quote (domestic) currency.
    ///
    /// Converts foreign leg to quote currency at model spot and adds domestic leg.
    pub(crate) fn total_pv(&self) -> f64 {
        let pv_foreign_dom = self.pv_foreign_leg_base() * self.model_spot;
        let pv_dom_leg = self.pv_domestic_leg();
        pv_foreign_dom + pv_dom_leg
    }

    /// Calculate total PV with a specific spot rate for conversion.
    ///
    /// Used for FX sensitivity calculations where we bump the spot rate.
    pub(crate) fn total_pv_with_spot(&self, spot: f64, near_rate: f64, far_rate: f64) -> f64 {
        let pv_foreign_dom = self.pv_foreign_leg_base() * spot;
        let pv_dom_leg = self.pv_domestic_leg_with_params(
            near_rate,
            far_rate,
            self.df_dom_near,
            self.df_dom_far,
        );
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
    fn test_cip_forward_calculation() {
        // Test: r_dom = 5%, r_for = 0.5%, T = 1 year
        // DF_dom = exp(-0.05) ≈ 0.9512, DF_for = exp(-0.005) ≈ 0.995
        // F = S × DF_for / DF_dom = 1.0 × 0.995 / 0.9512 ≈ 1.046
        let spot = 1.0;
        let df_dom_near = 1.0;
        let df_for_near = 1.0;
        let df_dom_far = 0.9512;
        let df_for_far = 0.995;

        let forward = FxSwapPricingContext::calculate_cip_forward(
            spot,
            df_dom_near,
            df_dom_far,
            df_for_near,
            df_for_far,
        )
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
        let result = FxSwapPricingContext::calculate_cip_forward(1.0, 0.0, 0.95, 1.0, 0.99);
        assert!(result.is_err(), "Should reject near-zero domestic DF");

        let result = FxSwapPricingContext::calculate_cip_forward(1.0, 1.0, 0.95, 0.0, 0.99);
        assert!(result.is_err(), "Should reject near-zero foreign DF");
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
        use crate::instruments::common_impl::traits::Attributes;
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::market_data::context::MarketContext;
        use finstack_quant_core::market_data::term_structures::DiscountCurve;
        use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
        use finstack_quant_core::money::Money;
        use finstack_quant_core::types::{CurveId, InstrumentId};
        use std::sync::Arc;
        use time::macros::date;

        let as_of = date!(2025 - 01 - 15);
        let near_date = date!(2025 - 01 - 17);
        let far_date = date!(2025 - 07 - 17);

        let usd = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (0.5, 0.9760), (1.0, 0.9520)])
            .build()
            .expect("usd curve");
        let eur = DiscountCurve::builder("EUR-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (0.5, 0.9900), (1.0, 0.9810)])
            .build()
            .expect("eur curve");
        let provider = SimpleFxProvider::new();
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.10)
            .expect("valid rate");
        let market = MarketContext::new()
            .insert(usd)
            .insert(eur)
            .insert_fx(FxMatrix::new(Arc::new(provider)));

        // Off-market swap: both legs priced away from model spot/forward.
        let swap = FxSwap::builder()
            .id(InstrumentId::new("EURUSD-SWAP-OFFMKT"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .near_date(near_date)
            .far_date(far_date)
            .base_notional(Money::new(10_000_000.0, Currency::EUR))
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
        let n = swap.base_notional.amount();
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
}
