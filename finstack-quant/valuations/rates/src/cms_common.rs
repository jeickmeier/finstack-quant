//! Reference-swap resolution and historical fixings shared by the CMS
//! instruments.
//!
//! A CMS fixing observes the par rate of a reference swap. [`CmsOption`],
//! [`CmsSwap`] and [`CmsSpreadOption`] all carry the same optional
//! convention overrides; [`CmsReferenceSwap`] resolves them once so the
//! Hagan, static-replication and spread pricers project the same swap.
//!
//! [`CmsOption`]: crate::cms_option::CmsOption
//! [`CmsSwap`]: crate::cms_swap::CmsSwap
//! [`CmsSpreadOption`]: crate::cms_spread_option::CmsSpreadOption
//! [`CmsReferenceSwap`]: crate::cms_common::CmsReferenceSwap

use crate::forward_swap_rate::{ForwardSwapRateInputs, calculate_forward_swap_rate};
use finstack_quant_core::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    Date, DateExt, DayCount, DayCountContext, StubKind, Tenor, calendar_by_id_strict,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::{CurveId, IndexId};

/// Whole months of a CMS reference-swap tenor.
///
/// # Arguments
///
/// * `tenor` - Tenor of the CMS reference swap (e.g. 10Y); must be month- or
///   year-based and non-zero.
/// * `label` - Wire path of the tenor field, quoted in the error message
///   (e.g. `CmsSwap cms_tenor`).
///
/// # Errors
///
/// Returns a validation error for a day- or week-based or zero tenor.
pub fn cms_tenor_months(tenor: Tenor, label: &str) -> Result<i32> {
    match tenor.months() {
        Some(months) if months > 0 => i32::try_from(months).map_err(|_| {
            finstack_quant_core::Error::Validation(format!("{label} ({tenor}) is too long"))
        }),
        _ => Err(finstack_quant_core::Error::Validation(format!(
            "{label} ({tenor}) must be a positive month- or year-based tenor"
        ))),
    }
}

/// Reference swap of a CMS fixing, with its leg conventions resolved.
///
/// Resolution order for every leg field is explicit override >
/// `index_id` > currency market convention. The USD CMS reference requires an
/// explicitly selected index: use `USD-SOFR-OIS` for SOFR or an explicit
/// legacy index for historical contracts. Omitting a USD index is an error.
#[derive(Debug, Clone, Copy)]
pub struct CmsReferenceSwap<'a> {
    /// Instrument label used in error messages (e.g. `CMS option 'ID'`).
    pub label: &'a str,
    /// Notional currency, selecting the market convention when no override is given.
    pub currency: Currency,
    /// Rate-index convention-registry key of the reference swap (e.g. `USD-SOFR-OIS`).
    pub index_id: Option<&'a IndexId>,
    /// Explicit fixed-leg payment frequency override.
    pub swap_fixed_frequency: Option<Tenor>,
    /// Explicit floating-leg payment frequency override.
    pub swap_float_frequency: Option<Tenor>,
    /// Explicit fixed-leg accrual day count override.
    pub swap_fixed_day_count: Option<DayCount>,
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
    ) -> Result<&'static finstack_quant_valuations_core::market::conventions::RateIndexConventions>
    {
        use finstack_quant_valuations_core::market::conventions::ConventionRegistry;
        if let Some(index_id) = self.index_id {
            return ConventionRegistry::try_global()?.require_rate_index(index_id);
        }
        let id = match self.currency {
            Currency::USD => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{} requires explicit index_id for USD CMS (for example USD-SOFR-OIS or a legacy index)",
                    self.label
                )));
            }
            Currency::EUR => "EUR-ESTR-OIS",
            Currency::GBP => "GBP-SONIA-OIS",
            Currency::JPY => "JPY-TONAR-OIS",
            _ => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{} requires an explicit reference-swap convention for {}",
                    self.label, self.currency
                )));
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
        match self.swap_fixed_day_count {
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
        self.resolved_fixed_frequency()?.payments_per_year()
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
        let calendar = calendar_by_id_strict(calendar_id)?;
        fixing_date.add_business_days(convention.market_settlement_days, calendar)
    }

    /// Effective date and forward par rate of the reference swap fixing on
    /// `fixing_date`.
    ///
    /// The swap starts on [`Self::reference_swap_start`] (fixing date plus the
    /// market settlement lag) and runs `tenor_months` months.
    ///
    /// # Arguments
    ///
    /// * `market` - Market supplying the discount and projection curves.
    /// * `as_of` - Valuation date used for relative discount factors.
    /// * `fixing_date` - CMS fixing (observation) date.
    /// * `tenor_months` - Reference swap tenor in months, as returned by
    ///   [`cms_tenor_months`].
    ///
    /// # Errors
    ///
    /// Returns the errors of [`Self::reference_swap_start`] and
    /// [`Self::forward_rate_and_annuity`], or a date error when the swap end
    /// overflows.
    pub fn forward_at_fixing(
        &self,
        market: &MarketContext,
        as_of: Date,
        fixing_date: Date,
        tenor_months: i32,
    ) -> Result<(Date, f64)> {
        let swap_start = self.reference_swap_start(fixing_date)?;
        let swap_end = swap_start.add_months(tenor_months)?;
        let (forward_rate, _annuity) =
            self.forward_rate_and_annuity(market, as_of, swap_start, swap_end)?;
        Ok((swap_start, forward_rate))
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

impl<I: CmsConvexityPricing + Send + Sync + 'static>
    finstack_quant_valuations_core::metrics::MetricCalculator
    for ConvexityAdjustmentRiskCalculator<I>
{
    fn calculate(
        &self,
        context: &mut finstack_quant_valuations_core::metrics::MetricContext,
    ) -> Result<f64> {
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
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] if any input is
/// non-finite, `m <= 0`, or `1 + rate / m <= 0`.
pub fn par_annuity(rate: f64, tenor_years: f64, m: f64) -> Result<f64> {
    if !rate.is_finite() || !tenor_years.is_finite() || !m.is_finite() || m <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "par_annuity requires finite inputs and positive payment frequency m, got rate={rate}, tenor={tenor_years}, m={m}"
        )));
    }
    let compounding_factor = 1.0 + rate / m;
    if compounding_factor <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "par_annuity requires 1 + rate / m > 0, got rate={rate}, m={m}"
        )));
    }
    if rate.abs() < 1e-9 {
        return Ok(tenor_years);
    }
    let discount = compounding_factor.powf(-tenor_years * m);
    Ok((1.0 - discount) / rate)
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

/// Value and first two rate derivatives of the payment-to-annuity mapping.
///
/// # Arguments
///
/// * `rate` - Decimal par swap rate, with `1 + rate / m` strictly positive.
/// * `tenor` - Positive reference-swap maturity in years.
/// * `m` - Positive fixed-leg payment frequency in payments per year.
/// * `delay` - Signed ACT/365F years from reference-swap start to coupon payment.
pub(crate) fn annuity_weight(rate: f64, tenor: f64, m: f64, delay: f64) -> (f64, f64, f64) {
    let n = tenor * m;
    let x = rate / m;
    // Series avoids cancellation in A, A' and A'' near zero. Keep fourth
    // order so the second derivative remains accurate across the branch.
    let (a, ap, app) = if (n * x).abs() < 1e-3 {
        let c1 = -(n + 1.0) / 2.0;
        let c2 = (n + 1.0) * (n + 2.0) / 6.0;
        let c3 = -(n + 1.0) * (n + 2.0) * (n + 3.0) / 24.0;
        let c4 = (n + 1.0) * (n + 2.0) * (n + 3.0) * (n + 4.0) / 120.0;
        (
            tenor * (1.0 + x * (c1 + x * (c2 + x * (c3 + x * c4)))),
            tenor / m * (c1 + x * (2.0 * c2 + x * (3.0 * c3 + x * 4.0 * c4))),
            tenor / (m * m) * (2.0 * c2 + x * (6.0 * c3 + x * 12.0 * c4)),
        )
    } else {
        let q = (-n * x.ln_1p()).exp();
        let b = -(-n * x.ln_1p()).exp_m1();
        let bp = n * q / (m + rate);
        let bpp = -n * (n + 1.0) * q / (m + rate).powi(2);
        (
            b / rate,
            bp / rate - b / rate.powi(2),
            bpp / rate - 2.0 * bp / rate.powi(2) + 2.0 * b / rate.powi(3),
        )
    };
    let delta = m * delay;
    let weight = (-delta * x.ln_1p()).exp() / a;
    let log_prime = -delta / (m + rate) - ap / a;
    let log_second = delta / (m + rate).powi(2) - app / a + (ap / a).powi(2);
    (
        weight,
        weight * log_prime,
        weight * (log_prime.powi(2) + log_second),
    )
}

// Historical CMS (par swap rate) fixing lookups for seasoned CMS trades.
//
// Mirrors the cap/floor seasoned-fixing pattern: a coupon whose fixing date
// lies before the valuation date must be valued off the recorded fixing, not
// re-projected from the live curve (which produces phantom P&L every time
// the curve moves). CMS fixings are resolved exclusively through the explicitly
// configured benchmark `cms_fixing_index_id`.

/// Validate that a CMS fixing index identifier is well-formed and distinct
/// from the instrument's forward projection curve and underlying rate index.
///
/// # Arguments
///
/// * `cms_fixing_index_id` - Explicit benchmark CMS fixing identifier (without `FIXING:`).
/// * `forward_curve_id` - Projection curve ID used for forward swap estimation.
/// * `index_id` - Optional underlying rate-index convention identifier.
/// * `field_label` - Field name reported in validation errors.
///
/// # Errors
///
/// Returns a validation error when `cms_fixing_index_id` is blank, has leading
/// or trailing whitespace, includes the `"FIXING:"` prefix, or matches
/// `forward_curve_id` or `index_id`.
pub(crate) fn validate_cms_fixing_index_id(
    cms_fixing_index_id: &IndexId,
    forward_curve_id: &CurveId,
    index_id: Option<&IndexId>,
    field_label: &str,
) -> Result<()> {
    let raw = cms_fixing_index_id.as_str();
    if raw.is_empty() || raw.trim() != raw {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{field_label} must be nonempty and without leading or trailing whitespace, got {raw:?}"
        )));
    }
    if raw.starts_with("FIXING:") {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{field_label} must name a CMS benchmark index without the 'FIXING:' prefix, got {raw:?}"
        )));
    }
    if raw == forward_curve_id.as_str() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{field_label} ({raw}) must be distinct from forward_curve_id ({})",
            forward_curve_id.as_str()
        )));
    }
    if let Some(rate_index_id) = index_id
        && raw == rate_index_id.as_str()
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{field_label} ({raw}) must be distinct from index_id ({})",
            rate_index_id.as_str()
        )));
    }
    Ok(())
}

/// Look up the recorded CMS (par swap) rate fixed on `fixing_date`.
///
/// # Arguments
///
/// * `curves` - Market context supplying the historical fixing series.
/// * `cms_fixing_index_id` - Explicit CMS benchmark fixing identifier.
/// * `forward_curve_id` - Projection curve identifier for distinctness checking.
/// * `index_id` - Optional underlying rate-index identifier for distinctness checking.
/// * `fixing_date` - Historical fixing observation date.
///
/// # Errors
///
/// Returns a validation error when `cms_fixing_index_id` is omitted or invalid,
/// or when `FIXING:<cms_fixing_index_id>` is absent from `curves` or lacks an
/// exact observation on `fixing_date`. Seasoned CMS coupons must never silently
/// fall back to live-curve projection.
pub(crate) fn historical_cms_fixing(
    curves: &MarketContext,
    cms_fixing_index_id: Option<&IndexId>,
    forward_curve_id: &CurveId,
    index_id: Option<&IndexId>,
    fixing_date: Date,
) -> Result<f64> {
    let Some(fixing_id) = cms_fixing_index_id else {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Seasoned CMS coupon requires explicit cms_fixing_index_id for fixing date {}. \
             Fixed-but-unpaid CMS coupons must be valued off an explicit benchmark fixing series \
             FIXING:<cms_fixing_index_id>, not the live forward curve.",
            fixing_date
        )));
    };
    validate_cms_fixing_index_id(fixing_id, forward_curve_id, index_id, "cms_fixing_index_id")?;
    let series =
        finstack_quant_core::market_data::fixings::get_fixing_series(curves, fixing_id.as_str())?;
    finstack_quant_core::market_data::fixings::require_fixing_value_exact(
        Some(series),
        fixing_id.as_str(),
        fixing_date,
    )
}

#[cfg(test)]
mod cms_fixings_tests {
    use super::*;
    use finstack_quant_core::market_data::scalars::{
        ScalarTimeSeries, SeriesExtrapolation, SeriesInterpolation,
    };
    use time::macros::date;

    #[test]
    fn missing_fixing_index_id_is_a_hard_error() {
        let ctx = MarketContext::new();
        let err = historical_cms_fixing(
            &ctx,
            None,
            &CurveId::new("USD-SOFR"),
            None,
            date!(2025 - 01 - 02),
        )
        .expect_err("omitted cms_fixing_index_id must error");
        assert!(err.to_string().contains("cms_fixing_index_id"), "{err}");
    }

    #[test]
    fn colliding_or_prefixed_fixing_index_id_errors() {
        let ctx = MarketContext::new();
        let err_fwd = historical_cms_fixing(
            &ctx,
            Some(&IndexId::new("USD-SOFR")),
            &CurveId::new("USD-SOFR"),
            Some(&IndexId::new("USD-SOFR-OIS")),
            date!(2025 - 01 - 02),
        )
        .expect_err("colliding with forward_curve_id must error");
        assert!(
            err_fwd.to_string().contains("forward_curve_id"),
            "{err_fwd}"
        );

        let err_idx = historical_cms_fixing(
            &ctx,
            Some(&IndexId::new("USD-SOFR-OIS")),
            &CurveId::new("USD-SOFR"),
            Some(&IndexId::new("USD-SOFR-OIS")),
            date!(2025 - 01 - 02),
        )
        .expect_err("colliding with index_id must error");
        assert!(err_idx.to_string().contains("index_id"), "{err_idx}");

        let err_prefix = historical_cms_fixing(
            &ctx,
            Some(&IndexId::new("FIXING:USD-SOFR-CMS-10Y")),
            &CurveId::new("USD-SOFR"),
            None,
            date!(2025 - 01 - 02),
        )
        .expect_err("FIXING:-prefixed ID must error");
        assert!(err_prefix.to_string().contains("FIXING:"), "{err_prefix}");
    }

    #[test]
    fn missing_series_is_a_hard_error() {
        let ctx = MarketContext::new();
        let fixing_id = IndexId::new("USD-SOFR-CMS-10Y");
        let err = historical_cms_fixing(
            &ctx,
            Some(&fixing_id),
            &CurveId::new("USD-SOFR"),
            Some(&IndexId::new("USD-SOFR-OIS")),
            date!(2025 - 01 - 02),
        )
        .expect_err("missing series must error");
        let msg = err.to_string();
        assert!(msg.contains("FIXING:USD-SOFR-CMS-10Y"), "{msg}");
    }

    #[test]
    fn exact_date_lookup_returns_recorded_rate() {
        let series = ScalarTimeSeries::new(
            "FIXING:USD-SOFR-CMS-10Y",
            vec![(date!(2025 - 01 - 02), 0.0412)],
            None,
            SeriesInterpolation::Step,
            SeriesExtrapolation::Error,
        )
        .expect("series");
        let ctx = MarketContext::new().insert_series(series);
        let fixing_id = IndexId::new("USD-SOFR-CMS-10Y");
        let rate = historical_cms_fixing(
            &ctx,
            Some(&fixing_id),
            &CurveId::new("USD-SOFR"),
            Some(&IndexId::new("USD-SOFR-OIS")),
            date!(2025 - 01 - 02),
        )
        .expect("fixing");
        assert!((rate - 0.0412).abs() < 1e-12);

        let err = historical_cms_fixing(
            &ctx,
            Some(&fixing_id),
            &CurveId::new("USD-SOFR"),
            Some(&IndexId::new("USD-SOFR-OIS")),
            date!(2025 - 01 - 03),
        )
        .expect_err("unobserved date must error");
        assert!(err.to_string().contains("2025-01-03"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn par_annuity_limit_and_formula() {
        assert_eq!(par_annuity(0.0, 10.0, 2.0).expect("zero rate"), 10.0);
        let a = par_annuity(0.05, 10.0, 2.0).expect("5% rate");
        let expected = (1.0 - (1.025f64).powf(-20.0)) / 0.05;
        assert!((a - expected).abs() < 1e-12);
        assert!(par_annuity(-2.0, 10.0, 2.0).is_err());
    }

    fn registry_index(
        id: &str,
    ) -> &'static finstack_quant_valuations_core::market::conventions::RateIndexConventions {
        finstack_quant_valuations_core::market::conventions::ConventionRegistry::try_global()
            .expect("registry")
            .require_rate_index(&IndexId::new(id))
            .expect("registered index")
    }

    #[test]
    fn resolution_prefers_explicit_then_convention_then_currency() {
        let disc = CurveId::new("EUR-OIS");
        let base = CmsReferenceSwap {
            label: "test",
            currency: Currency::EUR,
            index_id: None,
            swap_fixed_frequency: None,
            swap_float_frequency: None,
            swap_fixed_day_count: None,
            swap_float_day_count: None,
            discount_curve_id: &disc,
            forward_curve_id: &disc,
        };
        assert_eq!(
            base.resolved_fixed_frequency().expect("registry"),
            registry_index("EUR-ESTR-OIS").default_fixed_leg_frequency
        );
        // An explicit registry index selects conventions the currency default
        // cannot reach (CHF had no CMS reference swap before `index_id`).
        let saron = IndexId::new("CHF-SARON-OIS");
        let chf = CmsReferenceSwap {
            currency: Currency::CHF,
            index_id: Some(&saron),
            ..base
        };
        assert_eq!(
            chf.resolved_fixed_day_count().expect("registry"),
            registry_index("CHF-SARON-OIS").default_fixed_leg_day_count
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
        assert!(usd.resolved_fixed_frequency().is_err());
        let sofr = IndexId::new("USD-SOFR-OIS");
        let explicit_sofr = CmsReferenceSwap {
            index_id: Some(&sofr),
            ..usd
        };
        assert_eq!(
            explicit_sofr.resolved_fixed_frequency().expect("SOFR"),
            Tenor::annual()
        );
        let libor = IndexId::new("USD-LIBOR-3M");
        let explicit_legacy = CmsReferenceSwap {
            index_id: Some(&libor),
            ..usd
        };
        assert_eq!(
            explicit_legacy.resolved_fixed_frequency().expect("legacy"),
            Tenor::semi_annual()
        );
        assert_eq!(
            explicit_legacy.resolved_fixed_day_count().expect("legacy"),
            DayCount::Thirty360
        );
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
