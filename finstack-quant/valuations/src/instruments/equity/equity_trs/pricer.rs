//! Equity TRS pricing - dividend yield forward model.
//!
//! This module implements the total return leg pricing for equity TRS using
//! a cost-of-carry forward model with dividend yield.

use super::types::{EquityTotalReturnSwap, TrsDividendSettlement};
use crate::instruments::common_impl::pricing::time::relative_df_discount_curve;
use crate::instruments::common_impl::pricing::{
    PeriodReturnInputs, TotalReturnLegParams, TrsEngine, TrsReturnModel,
};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::math::neumaier_sum;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

/// Extracts spot price and dividend yield from market data.
///
/// # Errors
///
/// Returns an error if:
/// - The spot price cannot be fetched from market data
/// - A dividend yield ID is provided but the lookup fails (prevents silent configuration errors)
/// - The dividend yield is a Price scalar instead of Unitless
fn extract_underlying_data(
    trs: &EquityTotalReturnSwap,
    context: &MarketContext,
) -> Result<(f64, f64)> {
    let spot = crate::instruments::common_impl::helpers::scalar_price_amount(
        context.get_price(&trs.underlying.spot_id)?,
        trs.underlying.currency,
    )?;

    // When a dividend yield ID is explicitly provided, we require the lookup to succeed
    // and return a unitless scalar. Silent fallback to 0.0 would mask market data
    // configuration errors.
    let div_yield = if let Some(ref div_id) = trs.underlying.div_yield_id {
        let ms = context.get_price(div_id.as_str()).map_err(|e| {
            finstack_quant_core::Error::Validation(format!(
                "Failed to fetch dividend yield '{}': {}",
                div_id, e
            ))
        })?;
        match ms {
            MarketScalar::Unitless(v) => *v,
            MarketScalar::Price(m) => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Dividend yield '{}' should be a unitless scalar, got Price({})",
                    div_id,
                    m.currency()
                )));
            }
        }
    } else {
        0.0
    };

    Ok((spot, div_yield))
}

/// Equity-specific return model using cost-of-carry forward pricing.
///
/// Models the total return as:
/// - **Price return**: Forward price change using F_t = S_0 * e^{(r-q)t}.
///   For the *current* (in-progress) period the return anchors to the level
///   observed at the period start and the forward of the live spot, so the
///   realized spot move enters the PV (equity delta). Fully-future periods
///   are pure carry (the level cancels in the forward ratio).
/// - **Dividend return**: Continuous dividend yield approximation, net of withholding tax
struct EquityReturnModel<'a> {
    trs: &'a EquityTotalReturnSwap,
    spot: f64,
    div_yield: f64,
}

impl EquityReturnModel<'_> {
    /// Observed underlying level at the start of the in-progress period.
    ///
    /// Resolution order: `past_fixings` entry for `period_start`, then
    /// `initial_level` when the period is the first one. Errors otherwise —
    /// without the observed level the realized move (and hence equity delta)
    /// cannot be computed.
    fn period_start_level(&self, period_start: Date) -> Result<f64> {
        if let Some(level) = self.trs.fixing_on(period_start) {
            return Ok(level);
        }
        if period_start <= self.trs.schedule.start {
            if let Some(level) = self.trs.initial_level {
                return Ok(level);
            }
        }
        Err(finstack_quant_core::Error::Validation(format!(
            "EquityTRS '{}': the current return period started {} but no observed level is \
             available (no past_fixings entry and no applicable initial_level); provide the \
             period-start fixing to price this seasoned trade",
            self.trs.id.as_str(),
            period_start
        )))
    }

    fn period_end_level(&self, period_end: Date, as_of: Date) -> Result<f64> {
        if let Some(level) = self.trs.fixing_on(period_end) {
            return Ok(level);
        }
        if period_end == as_of {
            return Ok(self.spot);
        }
        Err(finstack_quant_core::Error::Validation(format!(
            "EquityTRS '{}': return period ended {} before payment but no observed \
             end-level fixing is available; provide the reset fixing to value the \
             fixed-but-unpaid cashflow",
            self.trs.id.as_str(),
            period_end
        )))
    }
}

impl TrsReturnModel for EquityReturnModel<'_> {
    fn period_return(&self, inputs: &PeriodReturnInputs, context: &MarketContext) -> Result<f64> {
        let PeriodReturnInputs {
            as_of,
            period_start,
            period_end,
            t_start,
            t_end,
            initial_level: _,
        } = *inputs;
        let disc = context.get_discount(self.trs.financing.discount_curve_id.as_str())?;

        let uses_discrete_dividends = !self.trs.discrete_dividends.is_empty();
        let carry_div_yield = if uses_discrete_dividends {
            0.0
        } else {
            self.div_yield
        };

        let pv_discrete_dividends_to = |target: Date| -> Result<f64> {
            if !uses_discrete_dividends {
                return Ok(0.0);
            }
            let mut values = Vec::new();
            for (div_date, amount) in &self.trs.discrete_dividends {
                if *div_date > as_of && *div_date <= target {
                    let df = relative_df_discount_curve(disc.as_ref(), as_of, *div_date)?;
                    values.push(*amount * df);
                }
            }
            Ok(neumaier_sum(values))
        };

        // Price return component. Completed-but-unpaid periods use both
        // contractual reset fixings; in-progress periods combine their start
        // fixing with the live spot projected to period end; future periods
        // use deterministic carry.
        let (fwd_start, fwd_end) = if t_end <= 0.0 {
            (
                self.period_start_level(period_start)?,
                self.period_end_level(period_end, as_of)?,
            )
        } else if t_start < 0.0 {
            let df_end = relative_df_discount_curve(disc.as_ref(), as_of, period_end)?;
            let start_level = self.period_start_level(period_start)?;
            let ex_div_spot = self.spot - pv_discrete_dividends_to(period_end)?;
            let fwd_spot_end = ex_div_spot * df_end.recip() * (-carry_div_yield * t_end).exp();
            (start_level, fwd_spot_end)
        } else {
            let df_start = relative_df_discount_curve(disc.as_ref(), as_of, period_start)?;
            let df_end = relative_df_discount_curve(disc.as_ref(), as_of, period_end)?;
            let fwd_start = (self.spot - pv_discrete_dividends_to(period_start)?)
                * df_start.recip()
                * (-carry_div_yield * t_start).exp();
            let fwd_end = (self.spot - pv_discrete_dividends_to(period_end)?)
                * df_end.recip()
                * (-carry_div_yield * t_end).exp();
            (fwd_start, fwd_end)
        };
        // Dividend return component, net of withholding tax.
        let dt = self.trs.schedule.params.day_count.year_fraction(
            period_start,
            period_end,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        let tax_rate = self.trs.dividend_tax_rate.clamp(0.0, 1.0);

        if uses_discrete_dividends {
            let payment_date = self.trs.schedule.payment_date_for(period_end)?;
            let payment_df = relative_df_discount_curve(disc.as_ref(), as_of, payment_date)?;
            let mut net_dividends = Vec::new();
            for (dividend_date, amount) in &self.trs.discrete_dividends {
                let in_period = *dividend_date > period_start && *dividend_date <= period_end;
                let still_unpaid = matches!(
                    self.trs.dividend_settlement,
                    TrsDividendSettlement::AtPeriodEnd
                ) || *dividend_date > as_of;
                if !in_period || !still_unpaid || !amount.is_finite() || *amount <= 0.0 {
                    continue;
                }
                let settlement_ratio = match self.trs.dividend_settlement {
                    TrsDividendSettlement::OnDividendDate => {
                        relative_df_discount_curve(disc.as_ref(), as_of, *dividend_date)?
                            / payment_df
                    }
                    TrsDividendSettlement::AtPeriodEnd => 1.0,
                };
                net_dividends.push(*amount * (1.0 - tax_rate) * settlement_ratio);
            }
            Ok(neumaier_sum(
                std::iter::once(fwd_end)
                    .chain(std::iter::once(-fwd_start))
                    .chain(net_dividends),
            ) / fwd_start)
        } else {
            let dividend_return = self.div_yield * dt * (1.0 - tax_rate);
            let price_return = if t_start >= 0.0 {
                // Spot cancels analytically for a fully future proportional
                // return. Evaluate the carry ratio directly to avoid numerical
                // spot sensitivity from dividing two projected spot levels.
                let df_start = relative_df_discount_curve(disc.as_ref(), as_of, period_start)?;
                let df_end = relative_df_discount_curve(disc.as_ref(), as_of, period_end)?;
                df_start / df_end * (-carry_div_yield * (t_end - t_start)).exp() - 1.0
            } else {
                (fwd_end - fwd_start) / fwd_start
            };
            Ok(price_return + dividend_return)
        }
    }
}

/// Calculates the present value of the total return leg for an equity TRS.
///
/// Uses a dividend yield forward model where the forward price is:
/// ```text
/// F_t = S_0 * e^{(r - q) * t}
/// ```
///
/// Total return = Price return + Dividend return
///
/// # Arguments
/// * `trs` — The equity TRS instrument
/// * `context` — Market context containing curves and market data
/// * `as_of` — Valuation date
///
/// # Returns
/// Present value of the total return leg in the instrument's currency.
///
/// # Errors
/// Returns an error if:
/// - The spot price cannot be fetched from market data
/// - The dividend yield ID is set but lookup fails (prevents silent configuration errors)
/// - The trade is seasoned (`as_of` after the schedule start) with discrete
///   dividends and `initial_level` is absent (the live spot would silently
///   reset accrued performance)
/// - The initial level is non-positive or non-finite
/// - The discount curve is not found
pub(crate) fn pv_total_return_leg(
    trs: &EquityTotalReturnSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    let (spot, div_yield) = extract_underlying_data(trs, context)?;
    // For an unseasoned trade (as_of on or before the schedule start) today's
    // spot IS the inception level. With continuous dividend yield the level
    // cancels out of every future-period forward ratio, so a seasoned trade
    // whose period-start fixing comes from `past_fixings` prices correctly
    // without `initial_level`. With DISCRETE dividends the level enters the
    // forward additively and no longer cancels: substituting spot would
    // silently reset accrued performance to zero, so require it.
    let initial = match trs.initial_level {
        Some(level) => level,
        None if as_of > trs.schedule.start && !trs.discrete_dividends.is_empty() => {
            return Err(finstack_quant_core::Error::Validation(format!(
                "EquityTRS '{}': as_of {} is after the schedule start {} and the \
                 trade carries discrete dividends, so initial_level is required; \
                 substituting the live spot would silently reset accrued \
                 performance to zero",
                trs.id.as_str(),
                as_of,
                trs.schedule.start
            )));
        }
        None => spot,
    };

    if !initial.is_finite() || initial <= 0.0 {
        return Err(finstack_quant_core::InputError::Invalid.into());
    }

    let params = TotalReturnLegParams {
        schedule: &trs.schedule,
        notional: trs.notional,
        discount_curve_id: trs.financing.discount_curve_id.as_str(),
        contract_size: trs.underlying.contract_size,
        initial_level: Some(initial),
    };

    let model = EquityReturnModel {
        trs,
        spot,
        div_yield,
    };
    TrsEngine::pv_total_return_leg_with_model(params, context, as_of, &model)
}

#[cfg(test)]
mod tests {
    use super::{EquityReturnModel, TrsReturnModel};
    use crate::instruments::equity::equity_trs::types::{
        EquityTotalReturnSwap, TrsDividendSettlement,
    };
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::types::CurveId;
    use time::Month;

    fn date(y: i32, m: u8, d: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).expect("month"), d).expect("date")
    }

    #[test]
    fn m17_future_dividend_tax_uses_projected_live_reset() {
        let as_of = date(2025, 1, 1);
        let period_start = date(2025, 2, 1);
        let period_end = date(2025, 3, 1);
        let curve = DiscountCurve::builder("DISC")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, 1.0)])
            .build()
            .expect("curve");
        let market = MarketContext::new().insert(curve);
        let mut trs = EquityTotalReturnSwap::example().expect("TRS");
        trs.financing.discount_curve_id = CurveId::new("DISC");
        trs.discrete_dividends = vec![(date(2025, 2, 15), 5.0)];
        trs.dividend_tax_rate = 0.3;
        let actual = EquityReturnModel {
            trs: &trs,
            spot: 120.0,
            div_yield: 0.0,
        }
        .period_return(
            &super::PeriodReturnInputs {
                as_of,
                period_start,
                period_end,
                t_start: 31.0 / 365.0,
                t_end: 59.0 / 365.0,
                initial_level: 100.0,
            },
            &market,
        )
        .expect("return");
        assert!((actual - (-5.0 * 0.3 / 120.0)).abs() < 1e-12, "{actual}");
    }

    #[test]
    fn gross_discrete_dividends_offset_the_ex_dividend_price_drop() {
        let period_start = date(2025, 1, 1);
        let period_end = date(2025, 2, 1);

        let disc = DiscountCurve::builder(CurveId::new("DISC"))
            .base_date(period_start)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, 1.0)])
            .build()
            .expect("discount curve");
        let context = MarketContext::new().insert(disc);

        let mut trs = EquityTotalReturnSwap::example().expect("example TRS");
        trs.financing.discount_curve_id = CurveId::new("DISC");
        trs.underlying.div_yield_id = None;
        trs.dividend_tax_rate = 0.0;
        trs.discrete_dividends = vec![
            (date(2025, 1, 10), 1e16),
            (date(2025, 1, 11), 1.0),
            (date(2025, 1, 12), 1.0),
        ];

        let model = EquityReturnModel {
            trs: &trs,
            spot: 100.0,
            div_yield: 0.0,
        };

        let period_return = model
            .period_return(
                &super::PeriodReturnInputs {
                    as_of: period_start,
                    period_start,
                    period_end,
                    t_start: 0.0,
                    t_end: 1.0,
                    initial_level: 100.0,
                },
                &context,
            )
            .expect("period return");

        assert_eq!(period_return, 0.0);
    }

    /// DF≠1 pin of the dividend settlement convention: dividends pass through
    /// at FACE at the period end, so under positive rates a gross (tax = 0)
    /// pass-through under-compensates the ex-div forward drop by exactly the
    /// funding carry `D·(1 − df_d/df_pe)` (negative). The flat-curve test
    /// above shows exact neutrality; this one pins the with-rates economics so
    /// a silent convention change (e.g. switching to ex-date reinvestment)
    /// fails loudly.
    #[test]
    fn gross_dividend_period_end_settlement_bears_funding_carry() {
        let period_start = date(2025, 1, 1);
        let div_date = date(2025, 4, 1);
        let period_end = date(2025, 7, 1);

        let disc = DiscountCurve::builder(CurveId::new("DISC"))
            .base_date(period_start)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, (-0.05_f64).exp())])
            .build()
            .expect("discount curve");
        let context = MarketContext::new().insert(disc.clone());

        let dividend = 5.0;
        let mut with_div = EquityTotalReturnSwap::example().expect("example TRS");
        with_div.financing.discount_curve_id = CurveId::new("DISC");
        with_div.underlying.div_yield_id = None;
        with_div.dividend_tax_rate = 0.0;
        with_div.dividend_settlement = TrsDividendSettlement::AtPeriodEnd;
        with_div.discrete_dividends = vec![(div_date, dividend)];

        let mut no_div = with_div.clone();
        no_div.discrete_dividends = vec![(div_date, 0.0)];

        let inputs = super::PeriodReturnInputs {
            as_of: period_start,
            period_start,
            period_end,
            t_start: 0.0,
            t_end: 0.5,
            initial_level: 100.0,
        };
        let ret_with = EquityReturnModel {
            trs: &with_div,
            spot: 100.0,
            div_yield: 0.0,
        }
        .period_return(&inputs, &context)
        .expect("period return with dividend");
        let ret_without = EquityReturnModel {
            trs: &no_div,
            spot: 100.0,
            div_yield: 0.0,
        }
        .period_return(&inputs, &context)
        .expect("period return without dividend");

        let df_d = crate::instruments::common_impl::pricing::time::relative_df_discount_curve(
            &disc,
            period_start,
            div_date,
        )
        .expect("df to ex-date");
        let df_pe = crate::instruments::common_impl::pricing::time::relative_df_discount_curve(
            &disc,
            period_start,
            period_end,
        )
        .expect("df to period end");

        // Δreturn = [−D·df_d/df_pe + D] / S0 — the funding carry of settling
        // the dividend at period end instead of its ex-date.
        let expected_delta = dividend * (1.0 - df_d / df_pe) / 100.0;
        assert!(expected_delta < 0.0, "positive rates ⇒ carry cost");
        assert!(
            ((ret_with - ret_without) - expected_delta).abs() < 1e-12,
            "period-end dividend settlement must bear exactly the funding \
             carry: got Δ={}, expected {expected_delta}",
            ret_with - ret_without
        );

        let mut on_dividend_date = with_div;
        on_dividend_date.dividend_settlement = TrsDividendSettlement::OnDividendDate;
        let ret_on_dividend_date = EquityReturnModel {
            trs: &on_dividend_date,
            spot: 100.0,
            div_yield: 0.0,
        }
        .period_return(&inputs, &context)
        .expect("dividend-date settlement return");
        assert!(
            (ret_on_dividend_date - ret_without).abs() < 1e-12,
            "dividend-date settlement must exactly offset the ex-dividend price drop"
        );
    }

    fn flat_market(as_of: Date, spot: f64) -> MarketContext {
        use finstack_quant_core::market_data::scalars::MarketScalar;

        let disc = DiscountCurve::builder(CurveId::new("USD-OIS"))
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (2.0, 1.0)])
            .build()
            .expect("discount curve");
        MarketContext::new()
            .insert(disc)
            .insert_price("SPX-SPOT", MarketScalar::Unitless(spot))
    }

    fn mid_period_trs() -> EquityTotalReturnSwap {
        let mut trs = EquityTotalReturnSwap::example().expect("example TRS");
        trs.underlying.div_yield_id = None;
        trs
    }

    /// The total-return leg must carry spot sensitivity for an in-progress
    /// period: with flat zero rates the current-period return is
    /// `spot / start_fixing - 1`, so bumping spot moves the PV one-for-one.
    #[test]
    fn current_period_return_has_spot_sensitivity() {
        let as_of = date(2024, 2, 15); // inside Q1 of the example schedule
        let mut trs = mid_period_trs();
        trs.past_fixings = vec![(date(2024, 1, 1), 100.0)];

        let pv_base = super::pv_total_return_leg(&trs, &flat_market(as_of, 100.0), as_of)
            .expect("pv at spot 100");
        let pv_up = super::pv_total_return_leg(&trs, &flat_market(as_of, 110.0), as_of)
            .expect("pv at spot 110");

        // Flat zero curve, no dividends: future periods carry zero return and
        // the current period contributes notional * (spot/100 - 1).
        let expected_diff = trs.notional.amount() * 0.10;
        assert!(
            (pv_up.amount() - pv_base.amount() - expected_diff).abs() < 1e-6 * expected_diff.abs(),
            "spot bump must move the TR leg PV: base={} up={} expected_diff={expected_diff}",
            pv_base.amount(),
            pv_up.amount()
        );
        // Realized move at base: spot 100 vs fixing 100 => zero return.
        assert!(
            pv_base.amount().abs() < 1e-6,
            "flat market, spot at fixing: TR leg should be ~0, got {}",
            pv_base.amount()
        );
    }

    /// `initial_level` may anchor the first period when no fixing is recorded.
    #[test]
    fn initial_level_anchors_first_period() {
        let as_of = date(2024, 2, 15);
        let mut trs = mid_period_trs();
        trs.initial_level = Some(100.0);

        let pv = super::pv_total_return_leg(&trs, &flat_market(as_of, 105.0), as_of)
            .expect("pv with initial_level anchor");
        let expected = trs.notional.amount() * 0.05;
        assert!(
            (pv.amount() - expected).abs() < 1e-6 * expected,
            "initial_level anchor must yield realized move 5%: got {}",
            pv.amount()
        );
    }

    /// Pricing inside a period without the period-start level must error,
    /// never silently drop the realized move.
    #[test]
    fn missing_period_start_level_errors() {
        let as_of = date(2024, 2, 15);
        let trs = mid_period_trs(); // no initial_level, no past_fixings

        let err = super::pv_total_return_leg(&trs, &flat_market(as_of, 105.0), as_of)
            .expect_err("missing period-start level");
        assert!(
            err.to_string().contains("period-start fixing")
                || err.to_string().contains("no observed level"),
            "expected period-start level error, got: {err}"
        );
    }

    /// A seasoned trade with discrete dividends must not silently substitute
    /// the live spot for the inception level: the dividend term breaks the
    /// forward-ratio cancellation and would zero the accrued performance.
    #[test]
    fn seasoned_discrete_dividend_trade_requires_initial_level() {
        let as_of = date(2024, 2, 15);
        let mut trs = mid_period_trs();
        trs.past_fixings = vec![(date(2024, 1, 1), 100.0)];
        trs.discrete_dividends = vec![(date(2024, 3, 15), 2.0)];

        let err = super::pv_total_return_leg(&trs, &flat_market(as_of, 105.0), as_of)
            .expect_err("seasoned discrete-dividend trade without initial_level");
        assert!(
            err.to_string().contains("initial_level"),
            "expected initial_level requirement error, got: {err}"
        );

        // Supplying the level prices fine.
        trs.initial_level = Some(100.0);
        super::pv_total_return_leg(&trs, &flat_market(as_of, 105.0), as_of)
            .expect("initial_level supplied");
    }

    /// A new trade priced on its start date has no in-progress period: the
    /// pure-carry path applies and no fixings are required.
    #[test]
    fn new_trade_needs_no_fixings() {
        let as_of = date(2024, 1, 1); // schedule start of the example TRS
        let trs = mid_period_trs();

        let pv = super::pv_total_return_leg(&trs, &flat_market(as_of, 100.0), as_of)
            .expect("new trade prices without fixings");
        // Flat zero curve, no dividends: all forward-ratio returns are zero.
        assert!(
            pv.amount().abs() < 1e-9,
            "flat-market new trade TR leg should be ~0, got {}",
            pv.amount()
        );
    }
}
