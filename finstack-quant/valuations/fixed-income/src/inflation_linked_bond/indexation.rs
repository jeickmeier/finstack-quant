//! Bond reference CPI resolved from explicit fixing snapshots and release facts.

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{
    InflationIndex, InflationLag, SeriesInterpolation,
};
use finstack_quant_core::{Error, Result};

use super::types::{IndexationMethod, InflationLinkedBond};

impl InflationLinkedBond {
    /// Calculate the unfloored contractual index ratio from observed monthly fixings.
    ///
    /// # Arguments
    ///
    /// * `date` - Contractual reference date; the bond applies its lag and monthly
    ///   interpolation once.
    /// * `inflation_index` - Matching immutable fixing snapshot, including actual
    ///   availability and its explicit knowledge cutoff. Every required anchor
    ///   must be observed; this method supplies no projection source.
    ///
    /// # Errors
    ///
    /// Errors on unsupported issue conventions, missing required fixings,
    /// mismatched index identity or an invalid/unrepresentable index ratio.
    pub fn index_ratio(&self, date: Date, inflation_index: &InflationIndex) -> Result<f64> {
        let market = MarketContext::new().insert_inflation_index(inflation_index.clone());
        self.index_ratio_from_market(date, &market, inflation_index.knowledge_date())
    }

    fn reference_interpolation(&self) -> Result<SeriesInterpolation> {
        match self.indexation_method {
            IndexationMethod::Japanese => Err(Error::Validation(
                "Japanese tenth-day indexation requires a qualified issue-specific reference rule"
                    .into(),
            )),
            IndexationMethod::Uk => match self.lag {
                InflationLag::Months(months) if months.get() == 3 => {
                    Ok(SeriesInterpolation::Linear)
                }
                InflationLag::Months(months) if months.get() == 8 => Ok(SeriesInterpolation::Step),
                _ => Err(Error::Validation(
                    "UK gilt indexation requires an explicit three- or eight-month lag".into(),
                )),
            },
            IndexationMethod::Tips | IndexationMethod::Canadian | IndexationMethod::French => {
                Ok(SeriesInterpolation::Linear)
            }
        }
    }

    fn checked_index_ratio(&self, reference: f64) -> Result<f64> {
        let ratio = reference / self.base_cpi;
        if !self.base_cpi.is_finite() || self.base_cpi <= 0.0 || !ratio.is_finite() || ratio <= 0.0
        {
            return Err(Error::Validation(
                "contractual base CPI and resulting index ratio must be finite and positive".into(),
            ));
        }
        Ok(ratio)
    }

    /// Resolve the unfloored ratio from eligible observed or projected monthly anchors.
    ///
    /// # Arguments
    ///
    /// * `date` - Contractual indexation date; the bond applies its lag once.
    /// * `market` - Matching index snapshot and projection source. Missing anchors
    ///   require explicit expected releases strictly after the snapshot cutoff.
    /// * `as_of` - Valuation date; it does not advance the supplied knowledge snapshot.
    ///
    /// # Errors
    ///
    /// Errors for unsupported conventions, unavailable reference inputs, or an
    /// invalid/unrepresentable contractual ratio.
    pub fn index_ratio_from_market(
        &self,
        date: Date,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<f64> {
        self.checked_index_ratio(self.reference_inflation(date, market, as_of)?.value)
    }

    pub(super) fn reference_inflation(
        &self,
        date: Date,
        market: &MarketContext,
        _as_of: Date,
    ) -> Result<
        finstack_quant_valuations_core::instruments::common_impl::helpers::InflationReferenceValue,
    > {
        let interpolation = self.reference_interpolation()?;
        let index = market.get_inflation_index(self.inflation_index_id.as_str())?;
        if index.currency() != self.notional.currency() {
            return Err(Error::Validation(
                "inflation index currency does not match bond currency".into(),
            ));
        }
        finstack_quant_valuations_core::instruments::common_impl::helpers::resolve_reference_inflation(
            market,
            self.inflation_index_id.as_str(),
            date,
            index.knowledge_date(),
            self.lag,
            interpolation,
        )
    }

    /// Expected indexed payment multiplier under its payment-date measure.
    pub(super) fn payment_index_ratio(
        &self,
        date: Date,
        market: &MarketContext,
        as_of: Date,
        protected: bool,
    ) -> Result<f64> {
        let reference = self.reference_inflation(date, market, as_of)?;
        let forward = self.checked_index_ratio(reference.value)?;
        if !protected {
            return Ok(forward);
        }
        if reference.is_known {
            return Ok(forward.max(1.0));
        }
        let id = self.floor_vol_surface_id.as_ref().ok_or_else(|| Error::Validation(
            format!("InflationLinkedBond '{}' requires floor_vol_surface_id for unpublished protected CPI on {date}", self.id)))?;
        let fixing_date = reference.publication_date.ok_or_else(|| {
            Error::Validation("unpublished protected CPI requires an expected release date".into())
        })?;
        let expiry = finstack_quant_core::dates::DayCount::Act365F.year_fraction(
            as_of,
            fixing_date,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if !expiry.is_finite() || expiry <= 0.0 {
            return Err(Error::Validation(
                "unpublished protected CPI requires a positive live publication expiry".into(),
            ));
        }
        let surface = market.get_surface(id.as_str())?;
        surface.require_quote_type(
            finstack_quant_core::market_data::surfaces::VolQuoteType::BlackLognormal,
        )?;
        surface.require_secondary_axis(
            finstack_quant_core::market_data::surfaces::VolSurfaceAxis::Strike,
        )?;
        let sigma = finstack_quant_models::volatility::get_surface_vol(&surface, expiry, 1.0)?;
        let floor = finstack_quant_models::closed_form::black76_price(
            forward,
            1.0,
            1.0,
            sigma,
            expiry,
            finstack_quant_models::types::OptionType::Put,
        )?;
        Ok(forward + floor)
    }
}
