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
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use std::sync::Arc;
use time::Duration;

use super::types::{IndexationMethod, InflationLinkedBond};

/// Market source of reference CPI for an inflation-linked bond.
///
/// Resolved from the market context under the bond's `inflation_index_id`:
/// published fixings (index), a projected curve, or both (hybrid: published
/// months read from the index, later months from the curve).
#[derive(Clone)]
pub(super) enum InflationSource {
    Index(Arc<InflationIndex>),
    Curve(Arc<InflationCurve>),
    Hybrid {
        index: Arc<InflationIndex>,
        curve: Arc<InflationCurve>,
    },
}

impl InflationSource {
    /// Resolve the inflation source registered under `id`.
    pub(super) fn from_market(curves: &MarketContext, id: &CurveId) -> Result<Self> {
        let index = curves.get_inflation_index(id.as_str()).ok();
        let curve = curves.get_inflation_curve(id.as_str()).ok();
        match (index, curve) {
            (Some(index), Some(curve)) => Ok(Self::Hybrid { index, curve }),
            (Some(index), None) => Ok(Self::Index(index)),
            (None, Some(curve)) => Ok(Self::Curve(curve)),
            (None, None) => Err(finstack_quant_core::InputError::NotFound {
                id: id.as_str().to_string(),
            }
            .into()),
        }
    }

    /// Unfloored index ratio `RefCPI(date) / base_index` for `bond`.
    pub(super) fn ratio(&self, bond: &InflationLinkedBond, date: Date) -> Result<f64> {
        match self {
            Self::Index(index) => bond.index_ratio(date, index.as_ref()),
            Self::Curve(curve) => bond.index_ratio_from_curve(date, curve.as_ref()),
            Self::Hybrid { index, curve } => {
                let expected_interp = bond.validate_index_conventions(index.as_ref())?;
                let (first_published, last_published) = index.date_range()?;
                let cpi_on = |reference_date: Date| -> Result<f64> {
                    let monthly = matches!(bond.lag, InflationLag::Months(_));
                    let before_first = if monthly {
                        (reference_date.year(), reference_date.month())
                            < (first_published.year(), first_published.month())
                    } else {
                        reference_date < first_published
                    };
                    let published = if monthly {
                        (reference_date.year(), reference_date.month())
                            <= (last_published.year(), last_published.month())
                    } else {
                        reference_date <= last_published
                    };
                    if before_first {
                        return Err(finstack_quant_core::InputError::NotFound {
                            id: format!(
                                "inflation index '{}' observation on or before {}",
                                index.id, reference_date
                            ),
                        }
                        .into());
                    }
                    if published {
                        if monthly {
                            index.ref_cpi_months_lag(
                                reference_date.replace_day(1).map_err(|_| {
                                    finstack_quant_core::InputError::InvalidDateRange
                                })?,
                                0,
                            )
                        } else {
                            index.value_on(reference_date)
                        }
                    } else {
                        curve.cpi_on_date(reference_date)
                    }
                };

                let current_index = match bond.lag {
                    InflationLag::Months(months)
                        if expected_interp == InflationInterpolation::Linear =>
                    {
                        let (anchor0, anchor1, weight) =
                            InflationLinkedBond::ref_cpi_anchors(date, months.into())?;
                        let cpi0 = cpi_on(anchor0)?;
                        if weight == 0.0 {
                            cpi0
                        } else {
                            let cpi1 = cpi_on(anchor1)?;
                            cpi0 + weight * (cpi1 - cpi0)
                        }
                    }
                    InflationLag::Months(months) => cpi_on(
                        InflationLinkedBond::step_reference_month(date, months.into())?,
                    )?,
                    InflationLag::Days(days) => cpi_on(date - Duration::days(i64::from(days)))?,
                    _ => cpi_on(date)?,
                };
                if bond.base_index <= 0.0 {
                    return Err(finstack_quant_core::InputError::NonPositiveValue.into());
                }
                Ok(current_index / bond.base_index)
            }
        }
    }
}

impl InflationLinkedBond {
    /// Resolve the bond's inflation source from the market context.
    pub(super) fn inflation_source(&self, curves: &MarketContext) -> Result<InflationSource> {
        InflationSource::from_market(curves, &self.inflation_index_id)
    }

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

        if self.base_index <= 0.0 {
            return Err(finstack_quant_core::InputError::NonPositiveValue.into());
        }
        Ok(current_index / self.base_index)
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

        if self.base_index <= 0.0 {
            return Err(finstack_quant_core::InputError::NonPositiveValue.into());
        }
        Ok(current_index / self.base_index)
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
        Ok(date
            .replace_day(1)
            .map_err(|_| finstack_quant_core::InputError::InvalidDateRange)?
            .add_months(-(lag_months as i32)))
    }

    /// First-of-month anchor dates and interpolation weight for the official
    /// RefCPI formula: `RefCPI(d) = CPI(m−L) + (day−1)/D(m) × [CPI(m−L+1) − CPI(m−L)]`.
    pub(super) fn ref_cpi_anchors(date: Date, lag_months: u32) -> Result<(Date, Date, f64)> {
        let first_of_month =
            Date::from_calendar_date(date.year(), date.month(), 1).map_err(|_| {
                finstack_quant_core::Error::Input(finstack_quant_core::InputError::InvalidDateRange)
            })?;
        let anchor0 = first_of_month.add_months(-(lag_months as i32));
        let anchor1 = anchor0.add_months(1);
        let weight = (f64::from(date.day()) - 1.0) / f64::from(date.month().length(date.year()));
        Ok((anchor0, anchor1, weight))
    }

    /// Calculate index ratio sourcing inflation data from the market context
    pub fn index_ratio_from_market(&self, date: Date, curves: &MarketContext) -> Result<f64> {
        let source = self.inflation_source(curves)?;
        source.ratio(self, date)
    }
}
