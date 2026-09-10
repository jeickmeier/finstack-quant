//! Reference-swap resolution shared by the CMS instruments.
//!
//! A CMS fixing observes the par rate of a reference swap. [`CmsOption`],
//! [`CmsSwap`] and [`CmsSpreadOption`] all carry the same optional
//! convention overrides; [`CmsReferenceSwap`] resolves them once so the
//! Hagan, static-replication and spread pricers project the same swap.
//!
//! [`CmsOption`]: crate::instruments::rates::cms_option::CmsOption
//! [`CmsSwap`]: crate::instruments::rates::cms_swap::CmsSwap
//! [`CmsSpreadOption`]: crate::instruments::rates::cms_spread_option::CmsSpreadOption
//! [`CmsReferenceSwap`]: crate::instruments::rates::cms_common::CmsReferenceSwap

use crate::instruments::common_impl::parameters::IRSConvention;
use crate::instruments::rates::hw1f::forward_swap_rate::{
    calculate_forward_swap_rate, ForwardSwapRateInputs,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    calendar_by_id, Date, DateExt, DayCount, DayCountContext, StubKind, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;

/// Reference swap of a CMS fixing, with its leg conventions resolved.
///
/// Resolution order for every leg field is explicit override >
/// `swap_convention` > currency market convention. The default USD CMS reference uses the
/// registered USD-LIBOR-3M convention (semi-annual 30/360 fixed versus
/// quarterly ACT/360 floating), distinct from
/// [`IRSConvention::UsdSofr`] (annual/annual OIS).
#[derive(Debug, Clone, Copy)]
pub struct CmsReferenceSwap<'a> {
    /// Instrument label used in error messages (e.g. `CMS option 'ID'`).
    pub label: &'a str,
    /// Notional currency, selecting the market convention when no override is given.
    pub currency: Currency,
    /// Explicit reference-swap convention override.
    pub swap_convention: Option<IRSConvention>,
    /// Explicit fixed-leg payment frequency override.
    pub swap_fixed_frequency: Option<Tenor>,
    /// Explicit floating-leg payment frequency override.
    pub swap_float_frequency: Option<Tenor>,
    /// Explicit fixed-leg accrual day count override.
    pub swap_day_count: Option<DayCount>,
    /// Explicit floating-leg accrual day count override.
    pub swap_float_day_count: Option<DayCount>,
    /// Discount curve used for the reference-swap annuity.
    pub discount_curve_id: &'a CurveId,
    /// Projection curve for the reference-swap floating leg.
    pub forward_curve_id: &'a CurveId,
}

impl CmsReferenceSwap<'_> {
    fn market_convention(
        &self,
    ) -> Result<&'static crate::market::conventions::RateIndexConventions> {
        if let Some(convention) = self.swap_convention {
            return convention.conventions();
        }
        use crate::market::conventions::ConventionRegistry;
        use finstack_quant_core::types::IndexId;
        let id = match self.currency {
            Currency::USD => "USD-LIBOR-3M",
            Currency::EUR => "EUR-ESTR-OIS",
            Currency::GBP => "GBP-SONIA-OIS",
            Currency::JPY => "JPY-TONAR-OIS",
            _ => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{} requires an explicit reference-swap convention for {}",
                    self.label, self.currency
                )))
            }
        };
        ConventionRegistry::try_global()?.require_rate_index(&IndexId::new(id))
    }

    /// Resolve the fixed-leg payment frequency from an explicit override or the canonical registry.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported currencies or invalid registry entries.
    pub fn resolved_fixed_frequency(&self) -> Result<Tenor> {
        match self.swap_fixed_frequency {
            Some(value) => Ok(value),
            None => Ok(self.market_convention()?.default_fixed_leg_frequency),
        }
    }

    /// Resolve the floating-leg payment frequency from an explicit override or the canonical registry.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported currencies or invalid registry entries.
    pub fn resolved_float_frequency(&self) -> Result<Tenor> {
        match self.swap_float_frequency {
            Some(value) => Ok(value),
            None => Ok(self.market_convention()?.default_payment_frequency),
        }
    }

    /// Resolve the fixed-leg accrual day count from an explicit override or the canonical registry.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported currencies or invalid registry entries.
    pub fn resolved_fixed_day_count(&self) -> Result<DayCount> {
        match self.swap_day_count {
            Some(value) => Ok(value),
            None => Ok(self.market_convention()?.default_fixed_leg_day_count),
        }
    }

    /// Resolve the floating-leg accrual day count from an explicit override or the canonical registry.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported currencies or invalid registry entries.
    pub fn resolved_float_day_count(&self) -> Result<DayCount> {
        match self.swap_float_day_count {
            Some(value) => Ok(value),
            None => Ok(self.market_convention()?.day_count),
        }
    }

    /// Fixed-leg payments per year on the resolved reference-swap convention.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed frequency cannot be resolved.
    pub fn payments_per_year(&self) -> Result<f64> {
        Ok(self.resolved_fixed_frequency()?.payments_per_year())
    }

    /// Effective date of the reference swap observed on `fixing_date`: the
    /// fixing shifted by the convention's reset lag on its calendar.
    ///
    /// # Arguments
    ///
    /// * `fixing_date` - CMS fixing (observation) date.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the convention cannot be resolved, has
    /// no reference calendar, or the calendar is not registered.
    pub fn reference_swap_start(&self, fixing_date: Date) -> Result<Date> {
        let convention = self.market_convention()?;
        let calendar_id = &convention.market_calendar_id;
        let calendar = calendar_by_id(calendar_id).ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "{} reference calendar '{}' is not registered",
                self.label, calendar_id
            ))
        })?;
        fixing_date.add_business_days(convention.market_settlement_days, calendar)
    }

    /// Forward par swap rate and market annuity of the reference swap
    /// `[start, end]` on the resolved conventions.
    ///
    /// # Arguments
    ///
    /// * `market` - Market supplying the discount and projection curves.
    /// * `as_of` - Valuation date used for relative discount factors.
    /// * `start` - Effective date of the reference swap.
    /// * `end` - Maturity of the reference swap.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the convention cannot be resolved, a
    /// curve is missing, the annuity is degenerate, or the projection curve
    /// tenor does not match the floating-leg frequency.
    pub fn forward_rate_and_annuity(
        &self,
        market: &MarketContext,
        as_of: Date,
        start: Date,
        end: Date,
    ) -> Result<(f64, f64)> {
        let convention = self.market_convention()?;
        let calendar_id = &convention.market_calendar_id;
        calculate_forward_swap_rate(ForwardSwapRateInputs {
            market,
            discount_curve_id: self.discount_curve_id,
            forward_curve_id: self.forward_curve_id,
            as_of,
            start,
            end,
            fixed_frequency: self.resolved_fixed_frequency()?,
            fixed_day_count: self.resolved_fixed_day_count()?,
            float_frequency: self.resolved_float_frequency()?,
            float_day_count: self.resolved_float_day_count()?,
            calendar_id,
            business_day_convention: convention.market_business_day_convention,
            stub: StubKind::ShortFront,
            end_of_month: start.end_of_month() == start && end.end_of_month() == end,
            payment_lag_days: convention.default_payment_lag_days,
            enforce_forward_tenor: convention.ois_compounding.is_none(),
        })
    }
}

/// CMS instruments whose PV can be re-run with the convexity adjustment
/// scaled (0 = linear, 1 = full).
pub(crate) trait CmsConvexityPricing {
    /// PV with the Hagan convexity adjustment multiplied by `convexity_scale`.
    fn pv_with_convexity_scale(
        &self,
        market: &MarketContext,
        as_of: Date,
        convexity_scale: f64,
    ) -> Result<finstack_quant_core::money::Money>;
}

/// Dollar value of the convexity adjustment: `PV(full) - PV(linear)`.
pub(crate) struct ConvexityAdjustmentRiskCalculator<I>(pub(crate) std::marker::PhantomData<I>);

impl<I: CmsConvexityPricing + Send + Sync + 'static> crate::metrics::MetricCalculator
    for ConvexityAdjustmentRiskCalculator<I>
{
    fn calculate(&self, context: &mut crate::metrics::MetricContext) -> Result<f64> {
        let inst: &I = context.instrument_as()?;
        let linear_pv = inst
            .pv_with_convexity_scale(&context.curves, context.as_of, 0.0)?
            .amount();
        Ok(context.base_value.amount() - linear_pv)
    }
}

/// Closed-form par annuity of a bullet fixed-rate swap discounted at its own rate.
///
/// ```text
/// A_par(k) = (1 - (1 + k/m)^(-n·m)) / k      k != 0
/// A_par(0) = n                                (L'Hôpital limit)
/// ```
///
/// # Arguments
///
/// * `rate` - Flat discount/par rate `k` as a decimal.
/// * `tenor_years` - Swap tenor `n` in years.
/// * `m` - Fixed-leg payments per year (see [`Tenor::payments_per_year`]).
pub fn par_annuity(rate: f64, tenor_years: f64, m: f64) -> f64 {
    if rate.abs() < 1e-9 {
        return tenor_years;
    }
    let discount = (1.0 + rate / m).powf(-tenor_years * m);
    (1.0 - discount) / rate
}

/// ACT/365F year fraction from `start` to `end`, signed when payment precedes swap start.
///
/// Hagan's payment delay is measured from reference-swap start to the coupon
/// payment. A floorlet paid on the fixing date can settle *before* a T+2 swap
/// start, so the delay is a small negative number rather than an invalid range.
///
/// # Arguments
///
/// * `start` - Reference-swap effective date.
/// * `end` - Coupon payment date; may precede `start`.
pub(crate) fn signed_act365f_year_fraction(start: Date, end: Date) -> Result<f64> {
    if end >= start {
        DayCount::Act365F.year_fraction(start, end, DayCountContext::default())
    } else {
        Ok(-DayCount::Act365F.year_fraction(end, start, DayCountContext::default())?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn par_annuity_limit_and_formula() {
        assert_eq!(par_annuity(0.0, 10.0, 2.0), 10.0);
        let a = par_annuity(0.05, 10.0, 2.0);
        let expected = (1.0 - (1.025f64).powf(-20.0)) / 0.05;
        assert!((a - expected).abs() < 1e-12);
    }

    #[test]
    fn resolution_prefers_explicit_then_convention_then_currency() {
        let disc = CurveId::new("EUR-OIS");
        let base = CmsReferenceSwap {
            label: "test",
            currency: Currency::EUR,
            swap_convention: None,
            swap_fixed_frequency: None,
            swap_float_frequency: None,
            swap_day_count: None,
            swap_float_day_count: None,
            discount_curve_id: &disc,
            forward_curve_id: &disc,
        };
        assert_eq!(
            base.resolved_fixed_frequency().expect("registry"),
            IRSConvention::EurEstr.fixed_frequency().expect("registry")
        );
        let explicit = CmsReferenceSwap {
            swap_fixed_frequency: Some(Tenor::quarterly()),
            ..base
        };
        assert_eq!(
            explicit.resolved_fixed_frequency().expect("registry"),
            Tenor::quarterly()
        );
        let usd = CmsReferenceSwap {
            currency: Currency::USD,
            ..base
        };
        assert_eq!(
            usd.resolved_fixed_frequency().expect("registry"),
            Tenor::semi_annual()
        );
        assert_eq!(
            usd.resolved_fixed_day_count().expect("registry"),
            DayCount::Thirty360
        );
        assert_eq!(usd.payments_per_year().expect("registry"), 2.0);
    }

    #[test]
    fn signed_act365f_year_fraction_is_negative_when_payment_precedes_start() {
        let start = Date::from_calendar_date(2027, time::Month::May, 20).expect("date");
        let payment = Date::from_calendar_date(2027, time::Month::May, 18).expect("date");
        let delay = signed_act365f_year_fraction(start, payment).expect("delay");
        assert!(delay < 0.0);
        let forward = signed_act365f_year_fraction(payment, start).expect("forward");
        assert!((delay + forward).abs() < 1e-18);
    }
}
