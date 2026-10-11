//! Reference-CPI lookup and index ratios for inflation-linked bonds.
//!
//! Owns the bond-side indexation rules: the lag is applied by the bond (the
//! market index must carry `lag = None`), months-lag linkers with daily
//! interpolation use the official RefCPI formula
//! `RefCPI(d) = CPI(m−L) + (day−1)/D(m) × [CPI(m−L+1) − CPI(m−L)]`, and step
//! linkers read the first-of-month print of the lagged month.

use finstack_quant_core::dates::{Date, DateExt};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{
    InflationIndex, InflationInterpolation, InflationLag,
};
use finstack_quant_core::market_data::term_structures::InflationCurve;
use finstack_quant_core::Result;
use time::Duration;

use super::types::{IndexationMethod, InflationLinkedBond};

impl InflationLinkedBond {
    /// Calculate the raw index ratio for a given date (CPI(date) / CPI(base)).
    ///
    /// Returns the **unfloored** ratio. Deflation protection is applied when
    /// building the canonical cashflow schedule so that the
    /// TIPS-style principal-only floor does not leak into coupon indexation.
    ///
    /// | Method | Lag | Interpolation |
    /// |--------|-----|---------------|
    /// | TIPS/Canadian | 3 months | Linear (daily) |
    /// | UK Gilt (legacy) | 8 months | Step (monthly) |
    /// | UK Gilt (modern) | 3 months | Linear (daily) |
    /// | French OAT€i | 3 months | Linear (daily) |
    /// | Japanese JGBi | 3 months | Linear (daily) |
    ///
    /// # Lag Ownership
    ///
    /// The bond owns the lag: it shifts the lookup date before calling
    /// `InflationIndex::value_on`. The provided index **must** have
    /// `lag == InflationLag::None` to avoid double-lagging.
    pub fn index_ratio(&self, date: Date, inflation_index: &InflationIndex) -> Result<f64> {
        let expected_interp = self.validate_index_conventions(inflation_index)?;

        let current_index = match self.lag {
            // Months-lag with daily interpolation follows the official RefCPI
            // formula: anchor on first-of-month CPI(m−L)/CPI(m−L+1) and weight
            // by (day−1)/D(settlement month). A generic calendar shift +
            // interpolation mis-weights month-end settlements (day clamping).
            InflationLag::Months(m) if expected_interp == InflationInterpolation::Linear => {
                inflation_index.ref_cpi_months_lag(date, m.into())?
            }
            InflationLag::Months(m) => inflation_index.ref_cpi_months_lag(
                date.replace_day(1)
                    .map_err(|_| finstack_quant_core::InputError::InvalidDateRange)?,
                m.into(),
            )?,
            InflationLag::Days(d) => inflation_index.value_on(date - Duration::days(d as i64))?,
            _ => inflation_index.value_on(date)?,
        };

        if self.base_cpi <= 0.0 {
            return Err(finstack_quant_core::InputError::NonPositiveValue.into());
        }
        Ok(current_index / self.base_cpi)
    }

    pub(super) fn validate_index_conventions(
        &self,
        inflation_index: &InflationIndex,
    ) -> Result<InflationInterpolation> {
        if !matches!(inflation_index.lag(), InflationLag::None) {
            return Err(finstack_quant_core::Error::Validation(
                "InflationIndex must have lag=None when used with InflationLinkedBond \
                 (the bond applies its own lag to avoid double-lagging)"
                    .to_string(),
            ));
        }

        let is_modern_uk = matches!(self.indexation_method, IndexationMethod::Uk)
            && matches!(self.lag, InflationLag::Months(m) if m <= 3);

        if matches!(self.indexation_method, IndexationMethod::Uk) {
            let valid_uk_lag =
                matches!(self.lag, InflationLag::Months(3) | InflationLag::Months(8));
            if !valid_uk_lag {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Non-standard UK gilt lag {:?}: expected Months(3) for modern or \
                     Months(8) for legacy. Index ratio cannot be reliably computed for \
                     non-standard lags.",
                    self.lag
                )));
            }
        }

        let expected_interp = match self.indexation_method {
            IndexationMethod::Tips
            | IndexationMethod::Canadian
            | IndexationMethod::French
            | IndexationMethod::Japanese => InflationInterpolation::Linear,
            IndexationMethod::Uk => {
                if is_modern_uk {
                    InflationInterpolation::Linear
                } else {
                    InflationInterpolation::Step
                }
            }
        };
        if inflation_index.interpolation() != expected_interp {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Inflation index interpolation mismatch for {:?}: expected {:?}, got {:?}",
                self.indexation_method,
                expected_interp,
                inflation_index.interpolation()
            )));
        }

        Ok(expected_interp)
    }

    /// Calculate the raw index ratio using an inflation term structure.
    ///
    /// Uses the curve's own base date and day count for time conversion
    /// (via [`InflationCurve::cpi_on_date`]), avoiding day-count basis
    /// mismatches between the bond and the curve.
    ///
    /// Returns the **unfloored** ratio. Deflation protection is applied when
    /// building the canonical cashflow schedule.
    pub fn index_ratio_from_curve(
        &self,
        date: Date,
        inflation_curve: &InflationCurve,
    ) -> Result<f64> {
        let current_index = match self.lag {
            // Same official RefCPI weighting as `index_ratio`: anchor on
            // first-of-month CPI(m−L)/CPI(m−L+1), weight by (day−1)/D(m).
            InflationLag::Months(m) if self.daily_interpolated_indexation() => {
                let (anchor0, anchor1, weight) = Self::ref_cpi_anchors(date, m.into())?;
                let cpi0 = inflation_curve.cpi_on_date(anchor0)?;
                let cpi1 = inflation_curve.cpi_on_date(anchor1)?;
                cpi0 + weight * (cpi1 - cpi0)
            }
            InflationLag::Months(m) => {
                inflation_curve.cpi_on_date(Self::step_reference_month(date, m.into())?)?
            }
            InflationLag::Days(d) => {
                inflation_curve.cpi_on_date(date - Duration::days(d as i64))?
            }
            _ => inflation_curve.cpi_on_date(date)?,
        };

        if self.base_cpi <= 0.0 {
            return Err(finstack_quant_core::InputError::NonPositiveValue.into());
        }
        Ok(current_index / self.base_cpi)
    }

    /// Whether the indexation method uses daily-interpolated reference CPI
    /// (the official months-lag RefCPI formula) rather than a step lookup.
    ///
    /// Japanese JGBi (issued from March 2004 onward) use the same
    /// daily-interpolated 3-month-lag reference CPI as US TIPS.
    fn daily_interpolated_indexation(&self) -> bool {
        match self.indexation_method {
            IndexationMethod::Tips
            | IndexationMethod::Canadian
            | IndexationMethod::French
            | IndexationMethod::Japanese => true,
            IndexationMethod::Uk => matches!(self.lag, InflationLag::Months(m) if m <= 3),
        }
    }

    /// Reference month for a step (non-interpolated) months lag: the first of
    /// the month `lag_months` before the month containing `date`. A monthly
    /// CPI print applies to its whole month, so a mid-month date must not
    /// interpolate between prints.
    pub(super) fn step_reference_month(date: Date, lag_months: u32) -> Result<Date> {
        let lag_months = i32::try_from(lag_months).map_err(|_| {
            finstack_quant_core::Error::Validation("CPI lag months exceed supported range".into())
        })?;
        date.replace_day(1)
            .map_err(|_| finstack_quant_core::InputError::InvalidDateRange)?
            .add_months(-lag_months)
    }

    /// First-of-month anchor dates and interpolation weight for the official
    /// RefCPI formula: `RefCPI(d) = CPI(m−L) + (day−1)/D(m) × [CPI(m−L+1) − CPI(m−L)]`.
    pub(super) fn ref_cpi_anchors(date: Date, lag_months: u32) -> Result<(Date, Date, f64)> {
        let first_of_month =
            Date::from_calendar_date(date.year(), date.month(), 1).map_err(|_| {
                finstack_quant_core::Error::Input(finstack_quant_core::InputError::InvalidDateRange)
            })?;
        let lag_months = i32::try_from(lag_months).map_err(|_| {
            finstack_quant_core::Error::Validation("CPI lag months exceed supported range".into())
        })?;
        let anchor0 = first_of_month.add_months(-lag_months)?;
        let anchor1 = anchor0.add_months(1)?;
        let weight = (f64::from(date.day()) - 1.0) / f64::from(date.month().length(date.year()));
        Ok((anchor0, anchor1, weight))
    }

    /// Resolve CPI using publication-aware historical fixings and future projections.
    ///
    /// # Arguments
    ///
    /// * `date` - Contractual indexation date; the bond's observation lag and
    ///   reference-month interpolation are applied once.
    /// * `market` - CPI fixings and projection curve under `inflation_index_id`.
    ///   Published anchors require historical observations; unpublished anchors
    ///   use the projection even when a future fixing is already stored.
    /// * `as_of` - Valuation date determining which observations are published.
    pub fn index_ratio_from_market(
        &self,
        date: Date,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<f64> {
        Ok(self.reference_inflation(date, market, as_of)?.value / self.base_cpi)
    }

    pub(super) fn reference_inflation(
        &self,
        date: Date,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<crate::instruments::common_impl::helpers::InflationReferenceValue> {
        let interpolation = if self.daily_interpolated_indexation() {
            InflationInterpolation::Linear
        } else {
            InflationInterpolation::Step
        };
        if let Ok(index) = market.get_inflation_index(self.inflation_index_id.as_str()) {
            self.validate_index_conventions(index.as_ref())?;
        }
        crate::instruments::common_impl::helpers::resolve_reference_inflation(
            market,
            self.inflation_index_id.as_str(),
            date,
            as_of,
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
        let forward = reference.value / self.base_cpi;
        if !protected {
            return Ok(forward);
        }
        if reference.is_known {
            return Ok(forward.max(1.0));
        }
        let id = self.floor_vol_surface_id.as_ref().ok_or_else(|| finstack_quant_core::Error::Validation(
            format!("InflationLinkedBond '{}' requires floor_vol_surface_id for unpublished protected CPI on {date}", self.id)))?;
        let fixing_date = match reference.publication_date {
            Some(date) => date,
            None => match self.lag {
                InflationLag::Months(months) => {
                    let (first, second, weight) = Self::ref_cpi_anchors(date, u32::from(months))?;
                    if self.daily_interpolated_indexation() && weight > 0.0 {
                        second
                    } else {
                        first
                    }
                }
                InflationLag::Days(days) => date
                    .checked_sub(Duration::days(i64::from(days)))
                    .ok_or(finstack_quant_core::InputError::InvalidDateRange)?,
                InflationLag::None => date,
                _ => {
                    return Err(finstack_quant_core::Error::Validation(
                        "unsupported inflation lag".into(),
                    ))
                }
            },
        };
        let expiry = finstack_quant_core::dates::DayCount::Act365F.year_fraction(
            as_of,
            fixing_date,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
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
            expiry,
            sigma,
            finstack_quant_models::OptionType::Put,
        )?;
        Ok(forward + floor)
    }
}
