//! CMS swap instrument definition.
//!
//! A CMS (Constant Maturity Swap) swap has one leg paying a CMS rate
//! (the par swap rate for a reference tenor, e.g., 10Y) and the other
//! leg paying a fixed or floating rate.
//!
//! The CMS rate requires a convexity adjustment because the forward
//! swap rate is a martingale under the annuity measure, not the payment
//! measure. The adjustment depends on volatility and the rate level.
//!
//! # Reference
//!
//! Hagan, P. S. (2003). "Convexity Conundrums: Pricing CMS Swaps, Caps,
//! and Floors." *Wilmott Magazine*, March, 38-44.

use crate::cashflow::builder::CashFlowSchedule;
use crate::cashflow::primitives::{CFKind, CashFlow};
use crate::impl_instrument_base;
use crate::instruments::common_impl::parameters::IRSConvention;
use crate::instruments::common_impl::traits::{Attributes, Instrument};
use finstack_quant_core::cashflow::CashFlowAccrual;
use finstack_quant_core::dates::{CalendarRegistry, Date, DateExt, DayCount, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};

/// CMS (Constant Maturity Swap) swap instrument.
///
/// One leg pays a CMS rate (the par swap rate for a reference tenor, e.g., 10Y)
/// observed on each fixing date, and the other leg pays a fixed or floating rate.
///
/// The CMS rate requires a convexity adjustment because the CMS rate is not a
/// martingale under the payment measure. The adjustment depends on the correlation
/// between the CMS rate and the numeraire (annuity).
///
/// # Reference
///
/// Hagan, P. S. (2003). "Convexity Conundrums: Pricing CMS Swaps, Caps, and Floors."
/// *Wilmott Magazine*, March, 38-44.
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    finstack_quant_valuations_macros::FocusedPricingOverrides,
)]
#[builder(validate = CmsSwap::validate)]
#[serde(deny_unknown_fields)]
pub struct CmsSwap {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Notional amount.
    pub notional: Money,
    /// Pay direction: `Pay` means pay CMS leg, receive funding leg.
    pub side: crate::instruments::common_impl::parameters::legs::PayReceive,

    // ── CMS Leg ──────────────────────────────────────────────────────────
    /// CMS tenor in years (e.g., 10.0 for 10Y swap rate).
    pub cms_tenor: f64,
    /// Fixing dates for CMS rate observations.
    #[schemars(with = "Vec<String>")]
    pub cms_fixing_dates: Vec<Date>,
    /// Payment dates for the CMS leg.
    #[schemars(with = "Vec<String>")]
    pub cms_payment_dates: Vec<Date>,
    /// Accrual fractions for each CMS period.
    pub cms_accrual_fractions: Vec<f64>,
    /// Day count convention for CMS leg accrual.
    pub cms_day_count: DayCount,
    /// Spread over the CMS rate (decimal, e.g., 0.001 = 10bp).
    #[serde(default)]
    #[builder(default)]
    pub cms_spread: f64,
    /// Optional cap on the CMS rate (decimal).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cms_cap: Option<f64>,
    /// Optional floor on the CMS rate (decimal).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cms_floor: Option<f64>,

    // ── Underlying Swap Conventions ──────────────────────────────────────
    /// IRS convention for the underlying swap (e.g., `USDStandard`).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap_convention: Option<IRSConvention>,
    /// Fixed leg frequency of the underlying swap (overrides convention).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap_fixed_freq: Option<Tenor>,
    /// Floating leg frequency of the underlying swap (overrides convention).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap_float_freq: Option<Tenor>,
    /// Day count of the underlying swap fixed leg (overrides convention).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap_day_count: Option<DayCount>,
    /// Day count of the underlying swap floating leg (overrides convention).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap_float_day_count: Option<DayCount>,

    // ── Funding Leg ──────────────────────────────────────────────────────
    /// Funding leg definition (fixed or floating).
    pub funding_leg: FundingLeg,

    // ── Market References ────────────────────────────────────────────────
    /// Discount curve ID for present value calculations.
    pub discount_curve_id: CurveId,
    /// Forward/projection curve ID for CMS rate projection.
    pub forward_curve_id: CurveId,
    /// Volatility surface ID for CMS convexity adjustment.
    pub vol_surface_id: CurveId,

    /// Pricing overrides (manual price, yield, spread).
    #[serde(default)]
    #[builder(default)]
    /// Instrument-owned pricing inputs.
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[serde(default)]
    #[builder(default)]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[serde(default)]
    #[builder(default)]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and grouping.
    #[serde(default)]
    #[builder(default)]
    pub attributes: Attributes,
}

/// Funding leg specification for a CMS swap.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(tag = "type")]
pub enum FundingLeg {
    /// Fixed rate funding leg.
    Fixed {
        /// Fixed coupon rate (decimal, e.g., 0.03 = 3%).
        rate: f64,
        /// Payment dates for each period.
        #[schemars(with = "Vec<String>")]
        payment_dates: Vec<Date>,
        /// Accrual fractions for each period.
        accrual_fractions: Vec<f64>,
        /// Day count convention.
        day_count: DayCount,
    },
    /// Floating rate funding leg.
    ///
    /// # Convention: no payment lag
    ///
    /// This leg models each period by its `payment_dates` only and assumes the
    /// **accrual end equals the payment date** (no payment lag, no
    /// accrual-vs-pay adjustment). The pricer projects the floating forward over
    /// `[previous payment date, payment date]` and discounts to the payment
    /// date. For funding with a genuine payment lag or accrual end ≠ payment
    /// date, model the floating side as a full IRS float leg (which carries
    /// explicit accrual start/end and payment dates) instead of this simplified
    /// funding leg.
    Floating {
        /// Spread over the floating index (decimal, e.g., 0.001 = 10bp).
        spread: f64,
        /// Payment dates for each period. Each is also treated as the period's
        /// accrual-end date (no payment lag — see the variant docs).
        #[schemars(with = "Vec<String>")]
        payment_dates: Vec<Date>,
        /// Accrual fractions for each period.
        accrual_fractions: Vec<f64>,
        /// Day count convention.
        day_count: DayCount,
        /// Forward curve for floating rate projection.
        forward_curve_id: CurveId,
    },
}

impl CmsSwap {
    /// Effective date of the reference swap observed on `fixing_date`.
    pub(crate) fn reference_swap_start(
        &self,
        fixing_date: Date,
    ) -> finstack_quant_core::Result<Date> {
        let convention = match (self.swap_convention, self.notional.currency()) {
            (Some(convention), _) => convention,
            (None, finstack_quant_core::currency::Currency::USD) => IRSConvention::USDStandard,
            (None, finstack_quant_core::currency::Currency::EUR) => IRSConvention::EURStandard,
            (None, finstack_quant_core::currency::Currency::GBP) => IRSConvention::GBPStandard,
            (None, finstack_quant_core::currency::Currency::JPY) => IRSConvention::JPYStandard,
            (None, currency) => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CMS swap '{}' requires swap_convention for currency {}",
                    self.id, currency
                )))
            }
        };
        let calendar_id = convention.calendar_id().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "CMS swap '{}' convention has no reference calendar",
                self.id
            ))
        })?;
        let calendar = CalendarRegistry::global()
            .resolve_str(&calendar_id)
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "CMS swap '{}' reference calendar '{}' is not registered",
                    self.id, calendar_id
                ))
            })?;
        fixing_date.add_business_days(convention.reset_lag_days(), calendar)
    }

    /// Validate CMS and funding leg schedule vectors.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if self.cms_fixing_dates.len() != self.cms_payment_dates.len()
            || self.cms_fixing_dates.len() != self.cms_accrual_fractions.len()
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CMS swap vectors must have equal length: fixing_dates={}, payment_dates={}, accrual_fractions={}",
                self.cms_fixing_dates.len(),
                self.cms_payment_dates.len(),
                self.cms_accrual_fractions.len(),
            )));
        }

        match &self.funding_leg {
            FundingLeg::Fixed {
                payment_dates,
                accrual_fractions,
                ..
            }
            | FundingLeg::Floating {
                payment_dates,
                accrual_fractions,
                ..
            } => {
                if payment_dates.len() != accrual_fractions.len() {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "CMS swap funding leg vectors must have equal length: payment_dates={}, accrual_fractions={}",
                        payment_dates.len(),
                        accrual_fractions.len(),
                    )));
                }
            }
        }

        Ok(())
    }

    /// Resolved fixed leg frequency (explicit > convention > default semi-annual).
    pub fn resolved_swap_fixed_freq(&self) -> Tenor {
        self.swap_fixed_freq
            .or_else(|| self.swap_convention.map(|c| c.fixed_frequency()))
            .unwrap_or_else(Tenor::semi_annual)
    }

    /// Resolved float leg frequency (explicit > convention > default quarterly).
    pub fn resolved_swap_float_freq(&self) -> Tenor {
        self.swap_float_freq
            .or_else(|| self.swap_convention.map(|c| c.float_frequency()))
            .unwrap_or_else(Tenor::quarterly)
    }

    /// Resolved fixed leg day count (explicit > convention > default 30/360).
    pub fn resolved_swap_day_count(&self) -> DayCount {
        self.swap_day_count
            .or_else(|| self.swap_convention.map(|c| c.fixed_day_count()))
            .unwrap_or(DayCount::Thirty360)
    }

    /// Resolved float leg day count (explicit > convention > ACT/360).
    pub fn resolved_swap_float_day_count(&self) -> DayCount {
        self.swap_float_day_count
            .or_else(|| self.swap_convention.map(|c| c.float_day_count()))
            .unwrap_or(DayCount::Act360)
    }

    /// Create a CMS swap from schedule parameters.
    ///
    /// Generates fixing/payment dates for both legs from start, end, and
    /// frequency. Calendar and reset lag come from `swap_convention`.
    #[allow(clippy::too_many_arguments)]
    pub fn from_schedule(
        id: impl Into<InstrumentId>,
        start_date: Date,
        maturity: Date,
        cms_frequency: Tenor,
        cms_tenor: f64,
        cms_spread: f64,
        funding_leg: FundingLegSpec,
        notional: Money,
        cms_day_count: DayCount,
        swap_convention: IRSConvention,
        side: crate::instruments::common_impl::parameters::legs::PayReceive,
        discount_curve_id: impl Into<CurveId>,
        forward_curve_id: impl Into<CurveId>,
        vol_surface_id: impl Into<CurveId>,
    ) -> finstack_quant_core::Result<Self> {
        use crate::cashflow::builder::periods::{build_periods, BuildPeriodsParams};
        use finstack_quant_core::dates::{BusinessDayConvention, StubKind};

        let calendar_id = swap_convention.calendar_id().ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "CMS swap convention has no reference calendar".to_string(),
            )
        })?;
        let reset_lag_days = swap_convention.reset_lag_days();

        let cms_periods = build_periods(BuildPeriodsParams {
            start: start_date,
            end: maturity,
            frequency: cms_frequency,
            stub: StubKind::ShortFront,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: &calendar_id,
            end_of_month: false,
            day_count: cms_day_count,
            payment_lag_days: 0,
            reset_lag_days: Some(reset_lag_days),
            adjust_accrual_dates: false,
        })?;

        if cms_periods.is_empty() {
            return Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::Invalid,
            ));
        }

        let cms_fixing_dates: Vec<Date> = cms_periods
            .iter()
            .map(|p| p.reset_date.unwrap_or(p.accrual_start))
            .collect();
        let cms_payment_dates: Vec<Date> = cms_periods.iter().map(|p| p.payment_date).collect();
        let cms_accrual_fractions: Vec<f64> = cms_periods
            .iter()
            .map(|p| p.accrual_year_fraction)
            .collect();

        let funding_leg = match funding_leg {
            FundingLegSpec::Fixed { rate, day_count } => {
                let fund_periods = build_periods(BuildPeriodsParams {
                    start: start_date,
                    end: maturity,
                    frequency: cms_frequency,
                    stub: StubKind::ShortFront,
                    bdc: BusinessDayConvention::ModifiedFollowing,
                    calendar_id: &calendar_id,
                    end_of_month: false,
                    day_count,
                    payment_lag_days: 0,
                    reset_lag_days: None,
                    adjust_accrual_dates: false,
                })?;
                FundingLeg::Fixed {
                    rate,
                    payment_dates: fund_periods.iter().map(|p| p.payment_date).collect(),
                    accrual_fractions: fund_periods
                        .iter()
                        .map(|p| p.accrual_year_fraction)
                        .collect(),
                    day_count,
                }
            }
            FundingLegSpec::Floating {
                spread,
                day_count,
                forward_curve_id,
            } => {
                let fund_periods = build_periods(BuildPeriodsParams {
                    start: start_date,
                    end: maturity,
                    frequency: cms_frequency,
                    stub: StubKind::ShortFront,
                    bdc: BusinessDayConvention::ModifiedFollowing,
                    calendar_id: &calendar_id,
                    end_of_month: false,
                    day_count,
                    payment_lag_days: 0,
                    reset_lag_days: None,
                    adjust_accrual_dates: false,
                })?;
                FundingLeg::Floating {
                    spread,
                    payment_dates: fund_periods.iter().map(|p| p.payment_date).collect(),
                    accrual_fractions: fund_periods
                        .iter()
                        .map(|p| p.accrual_year_fraction)
                        .collect(),
                    day_count,
                    forward_curve_id,
                }
            }
        };

        CmsSwap::builder()
            .id(id.into())
            .notional(notional)
            .side(side)
            .cms_tenor(cms_tenor)
            .cms_fixing_dates(cms_fixing_dates)
            .cms_payment_dates(cms_payment_dates)
            .cms_accrual_fractions(cms_accrual_fractions)
            .cms_day_count(cms_day_count)
            .cms_spread(cms_spread)
            .swap_convention_opt(Some(swap_convention))
            .funding_leg(funding_leg)
            .discount_curve_id(discount_curve_id.into())
            .forward_curve_id(forward_curve_id.into())
            .vol_surface_id(vol_surface_id.into())
            .build()
            .map_err(|e| finstack_quant_core::Error::Validation(e.to_string()))
    }

    /// Create a canonical example CMS swap (pay 10Y CMS, receive fixed).
    #[allow(clippy::expect_used)]
    pub fn example() -> Self {
        use finstack_quant_core::currency::Currency;
        use time::Month;

        let fixing_dates = vec![
            Date::from_calendar_date(2025, Month::March, 20).expect("valid"),
            Date::from_calendar_date(2025, Month::June, 20).expect("valid"),
            Date::from_calendar_date(2025, Month::September, 22).expect("valid"),
            Date::from_calendar_date(2025, Month::December, 22).expect("valid"),
        ];
        let payment_dates = vec![
            Date::from_calendar_date(2025, Month::June, 20).expect("valid"),
            Date::from_calendar_date(2025, Month::September, 22).expect("valid"),
            Date::from_calendar_date(2025, Month::December, 22).expect("valid"),
            Date::from_calendar_date(2026, Month::March, 20).expect("valid"),
        ];
        let accrual_fractions = vec![0.25, 0.25, 0.25, 0.25];

        CmsSwap::builder()
            .id(InstrumentId::new("CMSSWAP-10Y-USD"))
            .notional(Money::new(10_000_000.0, Currency::USD))
            .side(crate::instruments::common_impl::parameters::legs::PayReceive::Pay)
            .cms_tenor(10.0)
            .cms_fixing_dates(fixing_dates)
            .cms_payment_dates(payment_dates.clone())
            .cms_accrual_fractions(accrual_fractions.clone())
            .cms_day_count(DayCount::Act365F)
            .cms_spread(0.0)
            .swap_convention_opt(Some(IRSConvention::USDStandard))
            .swap_float_day_count_opt(Some(DayCount::Act360))
            .funding_leg(FundingLeg::Fixed {
                rate: 0.03,
                payment_dates,
                accrual_fractions,
                day_count: DayCount::Thirty360,
            })
            .discount_curve_id(CurveId::new("USD-OIS"))
            .forward_curve_id(CurveId::new("USD-LIBOR-3M"))
            .vol_surface_id(CurveId::new("USD-CMS10Y-VOL"))
            .attributes(Attributes::new())
            .build()
            .expect("Example CmsSwap construction should not fail")
    }

    fn cms_leg_flows(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Vec<CashFlow>> {
        let mut flows = Vec::new();
        let mut accrual_start = self.cms_fixing_dates.first().copied().unwrap_or(as_of);

        for (i, &fixing_date) in self.cms_fixing_dates.iter().enumerate() {
            let payment_date = self.cms_payment_dates[i];
            let accrual_fraction = self.cms_accrual_fractions[i];

            if payment_date < as_of {
                accrual_start = payment_date;
                continue;
            }

            let coupon_rate =
                super::pricer::cms_coupon_rate(self, market, as_of, fixing_date, 1.0)?;
            let signed_amount = match self.side {
                crate::instruments::common_impl::parameters::legs::PayReceive::Pay => {
                    -coupon_rate * accrual_fraction * self.notional.amount()
                }
                crate::instruments::common_impl::parameters::legs::PayReceive::Receive => {
                    coupon_rate * accrual_fraction * self.notional.amount()
                }
            };
            flows.push(
                CashFlow::new(
                    payment_date,
                    Some(fixing_date),
                    Money::new(signed_amount, self.notional.currency()),
                    CFKind::FloatReset,
                    accrual_fraction,
                    Some(coupon_rate),
                )
                .with_accrual(CashFlowAccrual {
                    start: accrual_start,
                    end: payment_date,
                    day_count: self.cms_day_count,
                    projected_index_rate: None,
                }),
            );
            accrual_start = payment_date;
        }

        Ok(flows)
    }

    fn funding_leg_flows(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Vec<CashFlow>> {
        use crate::instruments::common_impl::pricing::time::rate_between_on_dates;

        let mut flows = Vec::new();
        match &self.funding_leg {
            FundingLeg::Fixed {
                rate,
                payment_dates,
                accrual_fractions,
                day_count,
            } => {
                let mut accrual_start = self
                    .effective_start_date()
                    .unwrap_or_else(|| payment_dates.first().copied().unwrap_or(as_of));
                for (i, &payment_date) in payment_dates.iter().enumerate() {
                    let accrual = accrual_fractions[i];
                    let unsigned = rate * accrual * self.notional.amount();
                    let signed = match self.side {
                        crate::instruments::common_impl::parameters::legs::PayReceive::Pay => {
                            unsigned
                        }
                        crate::instruments::common_impl::parameters::legs::PayReceive::Receive => {
                            -unsigned
                        }
                    };
                    flows.push(
                        CashFlow::new(
                            payment_date,
                            None,
                            Money::new(signed, self.notional.currency()),
                            CFKind::Fixed,
                            accrual,
                            Some(*rate),
                        )
                        .with_accrual(CashFlowAccrual {
                            start: accrual_start,
                            end: payment_date,
                            day_count: *day_count,
                            projected_index_rate: None,
                        }),
                    );
                    accrual_start = payment_date;
                }
            }
            FundingLeg::Floating {
                spread,
                payment_dates,
                accrual_fractions,
                forward_curve_id,
                day_count,
            } => {
                let fwd_curve = market.get_forward(forward_curve_id.as_ref())?;
                let fixing_series_id = finstack_quant_core::market_data::fixings::fixing_series_id(
                    forward_curve_id.as_str(),
                );
                let fixings = market.get_series(&fixing_series_id).ok();
                let mut prev_date = self
                    .effective_start_date()
                    .unwrap_or_else(|| payment_dates.first().copied().unwrap_or(as_of));
                for (i, &payment_date) in payment_dates.iter().enumerate() {
                    let accrual = accrual_fractions[i];
                    let fwd_rate = if prev_date < as_of {
                        finstack_quant_core::market_data::fixings::require_fixing_value_exact(
                            fixings,
                            forward_curve_id.as_str(),
                            prev_date,
                            as_of,
                        )?
                    } else {
                        rate_between_on_dates(fwd_curve.as_ref(), prev_date, payment_date)?
                    };
                    let unsigned = (fwd_rate + spread) * accrual * self.notional.amount();
                    let signed = match self.side {
                        crate::instruments::common_impl::parameters::legs::PayReceive::Pay => {
                            unsigned
                        }
                        crate::instruments::common_impl::parameters::legs::PayReceive::Receive => {
                            -unsigned
                        }
                    };
                    flows.push(
                        CashFlow::new(
                            payment_date,
                            Some(prev_date),
                            Money::new(signed, self.notional.currency()),
                            CFKind::FloatReset,
                            accrual,
                            Some(fwd_rate + spread),
                        )
                        .with_accrual(CashFlowAccrual {
                            start: prev_date,
                            end: payment_date,
                            day_count: *day_count,
                            projected_index_rate: Some(fwd_rate),
                        }),
                    );
                    prev_date = payment_date;
                }
            }
        }
        Ok(flows)
    }
}

/// Simplified funding leg specification for [`CmsSwap::from_schedule`].
pub enum FundingLegSpec {
    /// Fixed rate funding leg.
    Fixed {
        /// Fixed coupon rate (decimal).
        rate: f64,
        /// Day count convention.
        day_count: DayCount,
    },
    /// Floating rate funding leg.
    Floating {
        /// Spread over the floating index (decimal).
        spread: f64,
        /// Day count convention.
        day_count: DayCount,
        /// Forward curve for floating rate projection.
        forward_curve_id: CurveId,
    },
}

impl crate::instruments::common_impl::traits::Instrument for CmsSwap {
    impl_instrument_base!(crate::pricer::InstrumentType::CmsSwap);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        CmsSwap::validate(self)
    }

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::Black76
    }

    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        crate::instruments::rates::cms_swap::pricer::compute_pv(self, market, as_of)
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        deps.add_forward_curve(self.forward_curve_id.clone());
        deps.add_series_id(
            finstack_quant_core::market_data::fixings::cms_fixing_series_id(
                self.forward_curve_id.as_str(),
                self.cms_tenor,
            ),
        );
        if let FundingLeg::Floating {
            forward_curve_id, ..
        } = &self.funding_leg
        {
            deps.add_forward_curve(forward_curve_id.clone());
            deps.add_series_id(finstack_quant_core::market_data::fixings::fixing_series_id(
                forward_curve_id.as_str(),
            ));
        }
        Ok(deps)
    }

    fn effective_start_date(&self) -> Option<Date> {
        self.cms_fixing_dates.first().copied()
    }

    crate::impl_focused_pricing_overrides!();
}

impl finstack_quant_cashflows::CashflowScheduleSource for CmsSwap {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn raw_cashflow_schedule(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        self.validate()?;
        let flows = self
            .cms_leg_flows(market, as_of)?
            .into_iter()
            .chain(self.funding_leg_flows(market, as_of)?)
            .collect();
        let schedule = crate::cashflow::traits::schedule_from_classified_flows(
            flows,
            self.cms_day_count,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: Some(self.notional),
                meta: crate::cashflow::builder::CashFlowMeta {
                    representation: crate::cashflow::builder::CashflowRepresentation::Projected,
                    ..Default::default()
                },
            },
        );
        Ok(schedule
            .with_representation(crate::cashflow::builder::CashflowRepresentation::Projected))
    }
}

#[cfg(test)]
mod tests {
    #[allow(dead_code, unused_imports)]
    mod test_utils {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/test_utils.rs"
        ));
    }

    use super::*;
    use crate::cashflow::CashflowProvider;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::fixings::{cms_fixing_series_id, fixing_series_id};
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    use test_utils::{date, flat_discount_with_tenor, flat_forward_with_tenor, flat_vol_surface};

    #[test]
    fn cms_swap_cashflow_provider_emits_signed_modeled_flows() {
        let as_of = date(2025, 1, 1);
        let swap = CmsSwap::example();
        let market = finstack_quant_core::market_data::context::MarketContext::new()
            .insert(flat_discount_with_tenor("USD-OIS", as_of, 0.0, 2.0))
            .insert(flat_forward_with_tenor("USD-LIBOR-3M", as_of, 0.04, 2.0))
            .insert_surface(flat_vol_surface(
                "USD-CMS10Y-VOL",
                &[0.25, 1.0],
                &[0.03, 0.05],
                0.20,
            ));

        let flows = swap
            .dated_cashflows(&market, as_of)
            .expect("cms contractual schedule should build");
        let schedule = swap
            .cashflow_schedule(&market, as_of)
            .expect("classified cms schedule");

        assert_eq!(
            flows.len(),
            swap.cms_payment_dates.len() + swap.cms_payment_dates.len(),
            "cms swap should emit one cms row and one funding row per period"
        );
        assert!(flows.iter().any(|(_, money)| money.amount() > 0.0));
        assert!(flows.iter().any(|(_, money)| money.amount() < 0.0));
        assert_eq!(
            schedule
                .get_flows()
                .iter()
                .filter(|flow| flow.kind == CFKind::FloatReset)
                .count(),
            swap.cms_payment_dates.len()
        );
        assert_eq!(
            schedule
                .get_flows()
                .iter()
                .filter(|flow| flow.kind == CFKind::Fixed)
                .count(),
            swap.cms_payment_dates.len()
        );
        assert!(schedule
            .get_flows()
            .iter()
            .all(|flow| flow.accrual.is_some()));
    }

    /// Build a shared market context for reconciliation tests: flat 3% OIS,
    /// flat 3% forward curve, flat 25% vol surface with enough knots.
    fn recon_market(as_of: Date) -> finstack_quant_core::market_data::context::MarketContext {
        use finstack_quant_core::market_data::surfaces::VolSurface;
        use finstack_quant_core::types::CurveId;

        let strikes = vec![0.005, 0.02, 0.03, 0.04, 0.06, 0.10];
        let expiries = vec![0.25, 1.0, 2.0, 5.0, 15.0];
        let mut builder = VolSurface::builder(CurveId::new("USD-CMS10Y-VOL"))
            .expiries(&expiries)
            .strikes(&strikes);
        for _ in &expiries {
            builder = builder.row(&vec![0.25_f64; strikes.len()]);
        }
        let vol_surface = builder.build().expect("vol surface");

        finstack_quant_core::market_data::context::MarketContext::new()
            .insert(flat_discount_with_tenor("USD-OIS", as_of, 0.03, 15.0))
            .insert(flat_forward_with_tenor("USD-LIBOR-3M", as_of, 0.03, 15.0))
            .insert_surface(vol_surface)
    }

    /// Build a 1-period CMS swap that settles well in the future (1Y fixing,
    /// 1.25Y payment), with a zero-rate funding leg so base_value == pv_cms_leg.
    fn one_period_cms_swap(cap: Option<f64>, floor: Option<f64>) -> CmsSwap {
        let fixing = date(2026, 1, 1);
        let pay = date(2026, 4, 1);
        let mut builder = CmsSwap::builder()
            .id(InstrumentId::new("CMS-RECON"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            // Receive CMS so base_value = pv_cms − pv_funding
            .side(crate::instruments::common_impl::parameters::legs::PayReceive::Receive)
            .cms_tenor(10.0)
            .cms_fixing_dates(vec![fixing])
            .cms_payment_dates(vec![pay])
            .cms_accrual_fractions(vec![0.25])
            .cms_day_count(DayCount::Act365F)
            .cms_spread(0.0)
            .swap_convention_opt(Some(
                crate::instruments::common_impl::parameters::IRSConvention::USDStandard,
            ))
            // Zero fixed rate so pv_funding = 0 and base_value == pv_cms
            .funding_leg(FundingLeg::Fixed {
                rate: 0.0,
                payment_dates: vec![pay],
                accrual_fractions: vec![0.25],
                day_count: DayCount::Act365F,
            })
            .discount_curve_id(CurveId::new("USD-OIS"))
            .forward_curve_id(CurveId::new("USD-LIBOR-3M"))
            .vol_surface_id(CurveId::new("USD-CMS10Y-VOL"));
        if let Some(c) = cap {
            builder = builder.cms_cap_opt(Some(c));
        }
        if let Some(f) = floor {
            builder = builder.cms_floor_opt(Some(f));
        }
        builder.build().expect("CMS swap should build")
    }

    /// C13 regression: discounting cms_leg_flows must reconcile with base_value
    /// for a CMS swap with an OTM cap (the hard-clamp bug makes them diverge).
    ///
    /// For a Receive CMS swap with zero funding leg:
    ///   base_value = pv_cms_leg = sum_i coupon_rate_i * accrual_i * df_i * N
    ///   cms_leg_flows[i] = +coupon_rate_i * accrual_i * N (positive, Receive side)
    ///   => sum_i df_i * cms_leg_flows[i]  must equal  base_value
    ///
    /// We use `relative_df_discount_curve` (same helper the pricer uses) to
    /// discount, so that any curve-interpolation details cancel exactly.
    #[test]
    fn cms_leg_flows_reconcile_with_base_value_capped() {
        use crate::instruments::common_impl::pricing::time::relative_df_discount_curve;
        use crate::instruments::common_impl::traits::Instrument;

        let as_of = date(2025, 1, 1);
        let market = recon_market(as_of);

        // OTM cap: forward CMS rate ~3%; cap at 6% — clamp is a no-op but
        // the embedded caplet carries positive time value.
        let swap = one_period_cms_swap(Some(0.06), None);

        // base_value uses the pricer (embedded-option path) — this is the ground truth.
        let base_pv = swap
            .base_value(&market, as_of)
            .expect("base_value")
            .amount();

        // Discount cms_leg_flows using the same curve + helper the pricer uses.
        let discount_curve = market
            .get_discount(swap.discount_curve_id.as_ref())
            .expect("discount curve");
        let flows = swap.cms_leg_flows(&market, as_of).expect("cms_leg_flows");
        let discounted_sum: f64 = flows
            .iter()
            .map(|flow| {
                let df = relative_df_discount_curve(discount_curve.as_ref(), as_of, flow.date)
                    .expect("df");
                df * flow.amount.amount()
            })
            .sum();

        // Reconciliation: must agree within 1 currency unit on a 1M notional.
        assert!(
            (discounted_sum - base_pv).abs() < 1.0,
            "capped CMS: discounted cms_leg_flows ({discounted_sum:.4}) must match \
             base_value ({base_pv:.4}); gap = {:.4}",
            (discounted_sum - base_pv).abs()
        );
    }

    /// C13 regression (floor variant): same reconciliation for an ITM floor.
    ///
    /// ITM floor: forward ~3%, floor at 4% — the buggy clamp gives only
    /// intrinsic (floor − forward = 0.01), but the embedded floorlet has
    /// substantial additional time value (~hundreds of USD on 1M notional)
    /// that the old hard-clamp would miss.  This ensures the old bug
    /// (clamping the convexity-adjusted mean instead of pricing an embedded
    /// option) causes this test to FAIL.
    #[test]
    fn cms_leg_flows_reconcile_with_base_value_floored() {
        use crate::instruments::common_impl::pricing::time::relative_df_discount_curve;
        use crate::instruments::common_impl::traits::Instrument;

        let as_of = date(2025, 1, 1);
        let market = recon_market(as_of);

        // ITM floor: floor (4%) > forward (~3%), so the old clamp gives only
        // intrinsic but the correct floorlet includes substantial time value.
        let swap = one_period_cms_swap(None, Some(0.04));

        let base_pv = swap
            .base_value(&market, as_of)
            .expect("base_value")
            .amount();

        let discount_curve = market
            .get_discount(swap.discount_curve_id.as_ref())
            .expect("discount curve");
        let flows = swap.cms_leg_flows(&market, as_of).expect("cms_leg_flows");
        let discounted_sum: f64 = flows
            .iter()
            .map(|flow| {
                let df = relative_df_discount_curve(discount_curve.as_ref(), as_of, flow.date)
                    .expect("df");
                df * flow.amount.amount()
            })
            .sum();

        assert!(
            (discounted_sum - base_pv).abs() < 1.0,
            "floored CMS: discounted cms_leg_flows ({discounted_sum:.4}) must match \
             base_value ({base_pv:.4}); gap = {:.4}",
            (discounted_sum - base_pv).abs()
        );
    }

    #[test]
    fn cms_leg_flows_use_recorded_fixing_for_seasoned_coupon() {
        let fixing = date(2024, 12, 1);
        let as_of = date(2025, 1, 1);
        let pay = date(2025, 3, 1);
        let swap = CmsSwap::builder()
            .id(InstrumentId::new("CMS-SEASONED-FLOWS"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .side(crate::instruments::common_impl::parameters::legs::PayReceive::Receive)
            .cms_tenor(10.0)
            .cms_fixing_dates(vec![fixing])
            .cms_payment_dates(vec![pay])
            .cms_accrual_fractions(vec![0.25])
            .cms_day_count(DayCount::Act365F)
            .cms_spread(0.0)
            .swap_convention_opt(Some(
                crate::instruments::common_impl::parameters::IRSConvention::USDStandard,
            ))
            .funding_leg(FundingLeg::Fixed {
                rate: 0.0,
                payment_dates: vec![pay],
                accrual_fractions: vec![0.25],
                day_count: DayCount::Act365F,
            })
            .discount_curve_id(CurveId::new("USD-OIS"))
            .forward_curve_id(CurveId::new("USD-LIBOR-3M"))
            .vol_surface_id(CurveId::new("USD-CMS10Y-VOL"))
            .build()
            .expect("CMS swap should build");
        let market = recon_market(as_of);

        let err = swap
            .cms_leg_flows(&market, as_of)
            .expect_err("seasoned flow without fixing should error");
        assert!(
            err.to_string().contains("FIXING:CMS-10Y:USD-LIBOR-3M"),
            "error must identify missing CMS fixing series: {err}"
        );

        let observed = 0.0412;
        let series = ScalarTimeSeries::new(
            cms_fixing_series_id("USD-LIBOR-3M", 10.0),
            vec![(fixing, observed)],
            None,
        )
        .expect("fixing series");
        let market = market.insert_series(series);

        let flows = swap
            .cms_leg_flows(&market, as_of)
            .expect("seasoned CMS leg flows");

        assert_eq!(flows.len(), 1);
        assert_eq!(flows[0].date, pay);
        let expected = observed * 0.25 * 1_000_000.0;
        assert!(
            (flows[0].amount.amount() - expected).abs() < 0.01,
            "seasoned flow must use recorded fixing: expected {expected}, got {}",
            flows[0].amount.amount()
        );
    }

    #[test]
    fn generated_cms_funding_flows_require_started_reset_fixing() {
        let reset = date(2024, 12, 1);
        let as_of = date(2025, 1, 1);
        let pay = date(2025, 3, 1);
        let swap = CmsSwap::builder()
            .id(InstrumentId::new("CMS-FUNDING-SEASONED"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .side(crate::instruments::common_impl::parameters::legs::PayReceive::Pay)
            .cms_tenor(10.0)
            .cms_fixing_dates(vec![reset])
            .cms_payment_dates(vec![pay])
            .cms_accrual_fractions(vec![0.25])
            .cms_day_count(DayCount::Act365F)
            .funding_leg(FundingLeg::Floating {
                spread: 0.001,
                payment_dates: vec![pay],
                accrual_fractions: vec![0.25],
                day_count: DayCount::Act360,
                forward_curve_id: CurveId::new("USD-LIBOR-3M"),
            })
            .discount_curve_id(CurveId::new("USD-OIS"))
            .forward_curve_id(CurveId::new("USD-LIBOR-3M"))
            .vol_surface_id(CurveId::new("USD-CMS10Y-VOL"))
            .build()
            .expect("CMS swap");
        let market = recon_market(as_of);

        let error = swap
            .funding_leg_flows(&market, as_of)
            .expect_err("started funding reset without fixing must fail");
        assert!(error.to_string().contains("FIXING:USD-LIBOR-3M"));

        let observed = 0.042;
        let series = ScalarTimeSeries::new(
            fixing_series_id("USD-LIBOR-3M"),
            vec![(reset, observed)],
            None,
        )
        .expect("funding fixing");
        let flows = swap
            .funding_leg_flows(&market.insert_series(series), as_of)
            .expect("funding flows with fixing");
        let expected = (observed + 0.001) * 0.25 * 1_000_000.0;
        assert!((flows[0].amount.amount() - expected).abs() < 1e-8);
    }

    #[test]
    fn builder_rejects_misaligned_cms_leg_vectors() {
        let result = CmsSwap::builder()
            .id(InstrumentId::new("CMSSWAP-BAD-CMS"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .side(crate::instruments::common_impl::parameters::legs::PayReceive::Pay)
            .cms_tenor(10.0)
            .cms_fixing_dates(vec![date(2026, 3, 20), date(2026, 6, 20)])
            .cms_payment_dates(vec![date(2026, 6, 20)])
            .cms_accrual_fractions(vec![0.25, 0.25])
            .cms_day_count(DayCount::Act365F)
            .funding_leg(FundingLeg::Fixed {
                rate: 0.03,
                payment_dates: vec![date(2026, 6, 20), date(2026, 9, 20)],
                accrual_fractions: vec![0.25, 0.25],
                day_count: DayCount::Thirty360,
            })
            .discount_curve_id(CurveId::new("USD-OIS"))
            .forward_curve_id(CurveId::new("USD-LIBOR-3M"))
            .vol_surface_id(CurveId::new("USD-CMS10Y-VOL"))
            .build();

        assert!(
            result.is_err(),
            "CMS swap builder must reject CMS leg vector length mismatches"
        );
    }

    #[test]
    fn builder_rejects_misaligned_funding_leg_vectors() {
        let result = CmsSwap::builder()
            .id(InstrumentId::new("CMSSWAP-BAD-FUNDING"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .side(crate::instruments::common_impl::parameters::legs::PayReceive::Pay)
            .cms_tenor(10.0)
            .cms_fixing_dates(vec![date(2026, 3, 20)])
            .cms_payment_dates(vec![date(2026, 6, 20)])
            .cms_accrual_fractions(vec![0.25])
            .cms_day_count(DayCount::Act365F)
            .funding_leg(FundingLeg::Floating {
                spread: 0.001,
                payment_dates: vec![date(2026, 6, 20), date(2026, 9, 20)],
                accrual_fractions: vec![0.25],
                day_count: DayCount::Act360,
                forward_curve_id: CurveId::new("USD-LIBOR-3M"),
            })
            .discount_curve_id(CurveId::new("USD-OIS"))
            .forward_curve_id(CurveId::new("USD-LIBOR-3M"))
            .vol_surface_id(CurveId::new("USD-CMS10Y-VOL"))
            .build();

        assert!(
            result.is_err(),
            "CMS swap builder must reject funding leg vector length mismatches"
        );
    }
}
