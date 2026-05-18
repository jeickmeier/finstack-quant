//! FX01 calculator for NDFs.
//!
//! Computes sensitivity to a 1bp absolute bump in the spot FX rate.
//!
//! # Quote-convention awareness
//!
//! The bumped forward and the resulting settlement amount must follow the
//! NDF's `quote_convention`, mirroring [`Ndf::estimate_forward_rate`] (CIRP
//! direction) and `Ndf::base_value` (settlement formula):
//!
//! - **BasePerSettlement** (spot/forward quoted as base per settlement):
//!   `F = S × DF_settlement / DF_base`,
//!   `Settlement = N_base × (1/F_contract − 1/F)`.
//! - **SettlementPerBase** (spot/forward quoted as settlement per base):
//!   `F = S × DF_base / DF_settlement`,
//!   `Settlement = N_base × (F − F_contract)`.
//!
//! Using the `BasePerSettlement` branch unconditionally (the prior behaviour)
//! produced the wrong magnitude — and an effectively wrong sign — for a
//! `SettlementPerBase` NDF.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fx::ndf::{Ndf, NdfQuoteConvention};
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::money::fx::FxQuery;

/// FX01 calculator for NDFs.
pub(crate) struct Fx01Calculator;

impl MetricCalculator for Fx01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
        let ndf: &Ndf = context.instrument_as()?;
        let curves = context.curves.clone();
        let as_of = context.as_of;

        // Post-fixing NDFs are not sensitive to spot.
        if ndf.fixing_rate.is_some() || as_of >= ndf.fixing_date {
            return Ok(0.0);
        }

        let base_pv = ndf.value(&curves, as_of)?;

        let settlement_disc = curves.get_discount(ndf.domestic_discount_curve_id.as_str())?;
        let df_settlement = settlement_disc.df_between_dates(as_of, ndf.maturity)?;

        // Resolve spot in the same convention as `contract_rate`/`quote_convention`,
        // mirroring `Ndf::estimate_forward_rate`. `FxProvider::rate(from, to)` is
        // quoted as units of `to` per unit of `from`, so base-per-settlement
        // requires querying `(settlement -> base)`.
        let (from_ccy, to_ccy) = match ndf.quote_convention {
            NdfQuoteConvention::BasePerSettlement => (ndf.settlement_currency, ndf.base_currency),
            NdfQuoteConvention::SettlementPerBase => (ndf.base_currency, ndf.settlement_currency),
        };
        let spot = if let Some(rate) = ndf.spot_rate_override {
            rate
        } else if let Some(fx) = curves.fx() {
            match (**fx).rate(FxQuery::new(from_ccy, to_ccy, as_of)) {
                Ok(rate) => rate.rate,
                Err(_) => {
                    let inverse = (**fx).rate(FxQuery::new(to_ccy, from_ccy, as_of))?;
                    1.0 / inverse.rate
                }
            }
        } else {
            return Err(finstack_core::Error::from(
                finstack_core::InputError::NotFound {
                    id: "fx_matrix".to_string(),
                },
            ));
        };

        let bump = 0.0001;
        let bumped_spot = spot + bump;

        // Bumped forward via covered interest rate parity, in the same
        // convention as the spot. Mirrors `Ndf::estimate_forward_rate`:
        //   BasePerSettlement:  F = S × DF_settlement / DF_base
        //   SettlementPerBase:  F = S × DF_base / DF_settlement
        // When no foreign curve is available the forward falls back to spot
        // (the simplified restricted-currency fallback).
        let effective_forward = if let Some(ref foreign_curve_id) = ndf.foreign_discount_curve_id {
            if let Ok(foreign_disc) = curves.get_discount(foreign_curve_id.as_str()) {
                let df_foreign = foreign_disc.df_between_dates(as_of, ndf.maturity)?;
                match ndf.quote_convention {
                    NdfQuoteConvention::BasePerSettlement => {
                        bumped_spot * df_settlement / df_foreign
                    }
                    NdfQuoteConvention::SettlementPerBase => {
                        bumped_spot * df_foreign / df_settlement
                    }
                }
            } else {
                bumped_spot
            }
        } else {
            bumped_spot
        };

        // Settlement amount in settlement currency, by convention. Mirrors
        // `Ndf::base_value`.
        let n_base = ndf.notional.amount();
        let settlement_amount = match ndf.quote_convention {
            NdfQuoteConvention::BasePerSettlement => {
                n_base * (1.0 / ndf.contract_rate - 1.0 / effective_forward)
            }
            NdfQuoteConvention::SettlementPerBase => {
                n_base * (effective_forward - ndf.contract_rate)
            }
        };
        let bumped_pv = settlement_amount * df_settlement;

        Ok(bumped_pv - base_pv.amount())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fx::ndf::NdfQuoteConvention;
    use crate::instruments::Attributes;
    use crate::metrics::MetricContext;
    use finstack_core::currency::Currency;
    use finstack_core::dates::Date;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_core::money::Money;
    use finstack_core::types::{CurveId, InstrumentId};
    use std::sync::Arc;
    use time::Month;

    /// USD (settlement) and CNY (base) flat discount curves.
    fn curves(as_of: Date) -> MarketContext {
        let usd_curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots(vec![(0.0, 1.0), (0.5, 0.9753), (1.0, 0.9512)])
            .build()
            .expect("usd curve");
        let cny_curve = DiscountCurve::builder("CNY-OIS")
            .base_date(as_of)
            .knots(vec![(0.0, 1.0), (0.5, 0.9876), (1.0, 0.9753)])
            .build()
            .expect("cny curve");
        MarketContext::new().insert(usd_curve).insert(cny_curve)
    }

    /// Market with an explicit FX quote. `(from, to, rate)` follows the
    /// `FxProvider` contract (`rate` is `to` per `from`).
    fn market_with_quote(as_of: Date, from: Currency, to: Currency, rate: f64) -> MarketContext {
        let provider = Arc::new(SimpleFxProvider::new());
        provider.set_quote(from, to, rate).expect("valid rate");
        curves(as_of).insert_fx(FxMatrix::new(provider))
    }

    fn fx01_of(ndf: &Ndf, market: &MarketContext, as_of: Date) -> f64 {
        let base_value = ndf.value(market, as_of).expect("base value");
        let instrument: Arc<dyn Instrument> = Arc::new(ndf.clone());
        let mut context = MetricContext::new(
            instrument,
            Arc::new(market.clone()),
            as_of,
            base_value,
            MetricContext::default_config(),
        );
        Fx01Calculator
            .calculate(&mut context)
            .expect("fx01 calculation")
    }

    /// Regression for the audited defect: a `SettlementPerBase` NDF priced its
    /// FX01 with the `BasePerSettlement` settlement formula and CIRP direction,
    /// yielding the wrong magnitude and an effectively wrong sign. The FX01 must
    /// match a finite-difference re-pricing through the *same* market path
    /// (FX-matrix spot), bumped by +1bp.
    #[test]
    fn ndf_fx01_settlement_per_base_matches_finite_difference() {
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("date");
        let fixing = Date::from_calendar_date(2024, Month::April, 13).expect("date");
        let maturity = Date::from_calendar_date(2024, Month::April, 15).expect("date");

        // SettlementPerBase: contract_rate and spot quoted as USD per CNY.
        // `estimate_forward_rate`/Fx01 query `rate(base=CNY -> settlement=USD)`,
        // so store the quote in that direction.
        let spb_spot = 1.0 / 7.25;
        let base_market = market_with_quote(as_of, Currency::CNY, Currency::USD, spb_spot);
        let bumped_market =
            market_with_quote(as_of, Currency::CNY, Currency::USD, spb_spot + 0.0001);

        let ndf = Ndf::builder()
            .id(InstrumentId::new("USDCNY-NDF-SPB"))
            .base_currency(Currency::CNY)
            .settlement_currency(Currency::USD)
            .fixing_date(fixing)
            .maturity(maturity)
            .notional(Money::new(10_000_000.0, Currency::CNY))
            .contract_rate(1.0 / 7.30)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id_opt(Some(CurveId::new("CNY-OIS")))
            .quote_convention(NdfQuoteConvention::SettlementPerBase)
            .attributes(Attributes::new())
            .build()
            .expect("ndf");

        let fx01 = fx01_of(&ndf, &base_market, as_of);

        let pv_base = ndf.value(&base_market, as_of).expect("pv base").amount();
        let pv_bumped = ndf
            .value(&bumped_market, as_of)
            .expect("pv bumped")
            .amount();
        let fd = pv_bumped - pv_base;

        assert!(
            fd > 0.0,
            "SettlementPerBase NDF is long base: a +bump to USD-per-CNY spot \
             must raise PV, got fd={fd}"
        );
        assert!(
            (fx01 - fd).abs() < 1e-6 * fd.abs().max(1.0),
            "FX01 must match the finite-difference bump: fx01={fx01} fd={fd}"
        );
        assert!(
            fx01 > 0.0,
            "FX01 sign must be positive for a long-base SettlementPerBase NDF, got {fx01}"
        );
    }

    /// BasePerSettlement FX01 must also match a finite-difference bump (guards
    /// against regressing the default convention).
    #[test]
    fn ndf_fx01_base_per_settlement_matches_finite_difference() {
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("date");
        let fixing = Date::from_calendar_date(2024, Month::April, 13).expect("date");
        let maturity = Date::from_calendar_date(2024, Month::April, 15).expect("date");

        // BasePerSettlement: contract_rate and spot quoted as CNY per USD.
        // `estimate_forward_rate`/Fx01 query `rate(settlement=USD -> base=CNY)`,
        // so store the quote in that direction.
        let bps_spot = 7.25;
        let base_market = market_with_quote(as_of, Currency::USD, Currency::CNY, bps_spot);
        let bumped_market =
            market_with_quote(as_of, Currency::USD, Currency::CNY, bps_spot + 0.0001);

        let ndf = Ndf::builder()
            .id(InstrumentId::new("USDCNY-NDF-BPS"))
            .base_currency(Currency::CNY)
            .settlement_currency(Currency::USD)
            .fixing_date(fixing)
            .maturity(maturity)
            .notional(Money::new(10_000_000.0, Currency::CNY))
            .contract_rate(7.30)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id_opt(Some(CurveId::new("CNY-OIS")))
            .quote_convention(NdfQuoteConvention::BasePerSettlement)
            .attributes(Attributes::new())
            .build()
            .expect("ndf");

        let fx01 = fx01_of(&ndf, &base_market, as_of);

        let pv_base = ndf.value(&base_market, as_of).expect("pv base").amount();
        let pv_bumped = ndf
            .value(&bumped_market, as_of)
            .expect("pv bumped")
            .amount();
        let fd = pv_bumped - pv_base;

        assert!(
            (fx01 - fd).abs() < 1e-6 * fd.abs().max(1.0),
            "BasePerSettlement FX01 must match the finite-difference bump: \
             fx01={fx01} fd={fd}"
        );
    }
}
