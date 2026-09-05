//! Equity option Black–Scholes pricing engine and greeks.
//!
//! Provides deterministic PV and greeks for `EquityOption` using the
//! Black–Scholes model with continuous dividend yield. Volatility is
//! sourced from a surface (clamped) unless overridden. This mirrors the
//! structure used by `fx_option` and keeps pricing logic separate from
//! instrument definitions.

mod black;
mod inputs;

pub use black::EquityOptionGreeks;
pub(crate) use black::{compute_greeks, compute_pv, SimpleEquityOptionBlackPricer};
pub(crate) use inputs::{
    collect_inputs, collect_inputs_extended, has_future_discrete_dividends,
    reject_future_discrete_dividends_for_stochastic_vol, require_european, resolve_lifecycle_value,
};

use crate::instruments::common_impl::parameters::OptionType;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::equity_option::types::EquityOption;
use crate::pricer::expect_inst;
use crate::pricer::PricingError;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_models::closed_form::heston::{
    heston_call_price_fourier, heston_put_price_fourier,
};

/// Equity option Heston semi-analytical pricer (Fourier inversion).
pub(crate) struct EquityOptionHestonFourierPricer;

impl EquityOptionHestonFourierPricer {
    /// Create a new Heston Fourier transform pricer
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for EquityOptionHestonFourierPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::pricer::Pricer for EquityOptionHestonFourierPricer {
    fn key(&self) -> crate::pricer::PricerKey {
        crate::pricer::PricerKey::new(
            crate::pricer::InstrumentType::EquityOption,
            crate::pricer::ModelKey::HestonFourier,
        )
    }

    #[tracing::instrument(
        name = "equity_option.heston_fourier.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(inst_id = %instrument.id(), as_of = %as_of),
        err,
    )]
    fn price_dyn(
        &self,
        instrument: &dyn crate::instruments::common_impl::traits::Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<crate::results::ValuationResult, PricingError> {
        let equity_option =
            expect_inst::<EquityOption>(instrument, crate::pricer::InstrumentType::EquityOption)?;

        if let Some(pv) =
            resolve_lifecycle_value(equity_option, market, as_of).map_err(|error| {
                crate::pricer::PricingError::model_failure_with_context(
                    error.to_string(),
                    crate::pricer::PricingErrorContext::from_instrument(equity_option)
                        .model(crate::pricer::ModelKey::HestonFourier),
                )
            })?
        {
            return Ok(crate::results::ValuationResult::stamped(
                equity_option.id(),
                as_of,
                pv,
            ));
        }
        require_european(equity_option, "Heston Fourier").map_err(|e| {
            crate::pricer::PricingError::model_failure_with_context(
                e.to_string(),
                crate::pricer::PricingErrorContext::from_instrument(equity_option)
                    .model(crate::pricer::ModelKey::HestonFourier),
            )
        })?;

        reject_future_discrete_dividends_for_stochastic_vol(
            equity_option,
            as_of,
            crate::pricer::ModelKey::HestonFourier,
            "Heston Fourier",
        )?;

        let inputs = collect_inputs_extended(equity_option, market, as_of).map_err(|e| {
            crate::pricer::PricingError::model_failure_with_context(
                e.to_string(),
                crate::pricer::PricingErrorContext::from_instrument(equity_option)
                    .model(crate::pricer::ModelKey::HestonFourier),
            )
        })?;
        let (spot, r, q, _sigma, t) = (inputs.spot, inputs.r, inputs.q, inputs.sigma, inputs.t_vol);

        if t <= 0.0 {
            let intrinsic = match equity_option.option_type {
                OptionType::Call => (spot - equity_option.strike).max(0.0),
                OptionType::Put => (equity_option.strike - spot).max(0.0),
            };
            return Ok(crate::results::ValuationResult::stamped(
                equity_option.id(),
                as_of,
                Money::new(
                    intrinsic * equity_option.notional.amount(),
                    equity_option.notional.currency(),
                )
                .map_err(|error| {
                    crate::pricer::PricingError::from_core(
                        error,
                        crate::pricer::PricingErrorContext::from_instrument(equity_option),
                    )
                })?,
            ));
        }

        // Source production Heston parameters from explicit market scalars.
        // Validation is still enforced inside `HestonParams::new`.
        let err_ctx = crate::pricer::PricingErrorContext::from_instrument(equity_option)
            .model(crate::pricer::ModelKey::HestonFourier);
        let params = crate::instruments::equity::equity_option::heston_market::heston_params_from_market_strict(market, r, q)
            .map_err(|e| crate::pricer::PricingError::from_core(e, err_ctx.clone()))?;

        let price = match equity_option.option_type {
            OptionType::Call => {
                heston_call_price_fourier(spot, equity_option.strike, t, &params, None)
            }
            OptionType::Put => {
                heston_put_price_fourier(spot, equity_option.strike, t, &params, None)
            }
        }
        .map_err(|error| crate::pricer::PricingError::from_core(error, err_ctx))?;

        let pv = Money::new(
            price * equity_option.notional.amount(),
            equity_option.notional.currency(),
        )
        .map_err(|error| {
            crate::pricer::PricingError::from_core(
                error,
                crate::pricer::PricingErrorContext::from_instrument(equity_option),
            )
        })?;
        Ok(crate::results::ValuationResult::stamped(
            equity_option.id(),
            as_of,
            pv,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::inputs::adjust_spot_for_discrete_dividends;
    use super::*;
    use crate::instruments::equity::equity_option::types::{
        EquityOption, EquityOptionExercise, ThetaDayBasis,
    };
    use crate::instruments::{Attributes, ExerciseStyle, SettlementType};
    use crate::pricer::{ModelKey, Pricer};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};
    use finstack_quant_models::closed_form::vanilla::bs_greeks_unchecked;
    use time::Month;

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("valid month"), day)
            .expect("valid date")
    }

    fn market(as_of: Date, spot: f64, vol: f64, rate: f64, div_yield: f64) -> MarketContext {
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (10.0, (-rate * 10.0).exp())])
            .build()
            .expect("curve");
        let surface = VolSurface::builder("SPX-VOL")
            .expiries(&[0.25, 0.5, 1.0, 2.0])
            .strikes(&[80.0, 100.0, 120.0, 150.0])
            .row(&[vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol])
            .build()
            .expect("surface");

        MarketContext::new()
            .insert(curve)
            .insert_surface(surface)
            .insert_price("SPX-SPOT", MarketScalar::Unitless(spot))
            .insert_price("SPX-DIV", MarketScalar::Unitless(div_yield))
    }

    fn option(
        expiry: Date,
        option_type: OptionType,
        exercise_style: ExerciseStyle,
    ) -> EquityOption {
        EquityOption::builder()
            .id(InstrumentId::new("EQ-OPT-TEST"))
            .underlying_ticker("SPX".to_string())
            .strike(100.0)
            .option_type(option_type)
            .exercise_style(exercise_style)
            .expiry(expiry)
            .notional(Money::from((100_i64, Currency::USD)))
            .day_count(DayCount::Act365F)
            .settlement(SettlementType::Cash)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(PriceId::new("SPX-DIV")))
            .attributes(Attributes::new())
            .build()
            .expect("equity option")
    }

    #[test]
    fn equity_option_default_equals_black76_for_pv_and_raw() {
        let as_of = date(2025, 1, 1);
        let option = option(date(2026, 1, 1), OptionType::Call, ExerciseStyle::European);
        let market = market(as_of, 100.0, 0.20, 0.03, 0.01);
        let registry = crate::pricer::standard_pricer_registry();

        let default = option
            .price_with_metrics(
                &market,
                as_of,
                &[],
                crate::instruments::PricingOptions::default(),
            )
            .expect("default equity-option price");
        let black76 = registry
            .price_with_metrics(
                &option,
                ModelKey::Black76,
                &market,
                as_of,
                &[],
                crate::instruments::PricingOptions::default(),
            )
            .expect("Black76 equity-option price");
        let default_raw = option
            .value_raw(&market, as_of)
            .expect("default equity-option raw price");
        let black76_raw = registry
            .price_raw(&option, ModelKey::Black76, &market, as_of)
            .expect("Black76 equity-option raw price");

        assert_eq!(default.value, black76.value);
        assert_eq!(default_raw, black76_raw);
    }

    #[test]
    fn expiry_requires_observed_lifecycle_state() {
        let expiry = date(2025, 6, 20);
        let option = option(expiry, OptionType::Call, ExerciseStyle::European);
        let error = compute_pv(&option, &market(expiry, 120.0, 0.2, 0.05, 0.0), expiry)
            .expect_err("expiry without an observation must fail");
        assert!(error.to_string().contains("exercise/expiry observation"));
    }

    #[test]
    fn cash_exercise_remains_until_payment() {
        let expiry = date(2025, 6, 20);
        let settlement = date(2025, 6, 23);
        let mut option = option(expiry, OptionType::Call, ExerciseStyle::European);
        option.exercise = Some(EquityOptionExercise::new(expiry, 120.0, settlement, true));
        let market = market(expiry, 121.0, 0.2, 0.05, 0.0);

        let pv = compute_pv(&option, &market, expiry)
            .expect("fixed cash payoff")
            .amount();
        let df = market
            .get_discount("USD-OIS")
            .expect("discount curve")
            .df_between_dates(expiry, settlement)
            .expect("settlement discount factor");
        assert!((pv - 20.0 * 100.0 * df).abs() < 1e-9);

        let after_settlement = date(2025, 6, 24);
        assert_eq!(
            compute_pv(&option, &market, after_settlement)
                .expect("settled option")
                .amount(),
            0.0
        );
    }

    #[test]
    fn physical_exercise_marks_delivery_obligation() {
        let expiry = date(2025, 6, 20);
        let settlement = date(2025, 6, 23);
        let mut option = option(expiry, OptionType::Call, ExerciseStyle::European);
        option.settlement = SettlementType::Physical;
        option.exercise = Some(EquityOptionExercise::new(expiry, 120.0, settlement, true));
        let market = market(expiry, 121.0, 0.2, 0.05, 0.0);
        let df = market
            .get_discount("USD-OIS")
            .expect("discount curve")
            .df_between_dates(expiry, settlement)
            .expect("settlement discount factor");

        let pv = compute_pv(&option, &market, expiry)
            .expect("physical delivery mark")
            .amount();
        assert!((pv - (121.0 - 100.0 * df) * 100.0).abs() < 1e-9);
        let greeks = compute_greeks(&option, &market, expiry).expect("delivery risk");
        assert!((greeks.delta - 100.0).abs() < 1e-12);
    }

    #[test]
    fn american_discrete_dividend_preserves_pre_exercise_spot() {
        let as_of = date(2025, 1, 2);
        let expiry = date(2026, 1, 2);
        let mut american = option(expiry, OptionType::Call, ExerciseStyle::American);
        american.strike = 50.0;
        american.discrete_dividends = vec![(date(2025, 2, 3), 20.0)];
        american.validate().expect("valid discrete-dividend option");
        let mut european = american.clone();
        european.exercise_style = ExerciseStyle::European;
        let market = market(as_of, 100.0, 0.2, 0.05, 0.0);

        let american_pv = compute_pv(&american, &market, as_of)
            .expect("American discrete-dividend price")
            .amount();
        let european_pv = compute_pv(&european, &market, as_of)
            .expect("European escrowed-dividend price")
            .amount();

        assert!(american_pv >= 50.0 * american.notional.amount());
        assert!(
            american_pv > european_pv,
            "large near-term dividend should create an early-exercise premium"
        );
    }

    #[test]
    fn theta_day_basis_is_explicit_and_configurable() {
        let as_of = date(2025, 1, 2);
        let expiry = date(2026, 1, 2);
        let calendar = option(expiry, OptionType::Call, ExerciseStyle::European);
        let mut trading = calendar.clone();
        trading.theta_day_basis = ThetaDayBasis::Trading252;
        let market = market(as_of, 100.0, 0.2, 0.05, 0.01);

        let calendar_theta = compute_greeks(&calendar, &market, as_of)
            .expect("calendar theta")
            .theta;
        let trading_theta = compute_greeks(&trading, &market, as_of)
            .expect("trading theta")
            .theta;
        assert!((trading_theta / calendar_theta - 365.0 / 252.0).abs() < 1e-12);
    }

    #[test]
    fn test_adjust_spot_for_discrete_dividends_single() {
        // Stock at $100, dividend of $2 in 0.25 years, r = 5%
        let s_adj = adjust_spot_for_discrete_dividends(100.0, 0.05, &[(0.25, 2.0)])
            .expect("valid adjusted spot");
        // PV(div) = 2 × e^{-0.05×0.25} ≈ 1.9751
        assert!((s_adj - 98.0248).abs() < 0.01);
    }

    #[test]
    fn test_adjust_spot_for_discrete_dividends_multiple() {
        let s_adj = adjust_spot_for_discrete_dividends(100.0, 0.05, &[(0.25, 1.5), (0.5, 1.5)])
            .expect("valid adjusted spot");
        let expected = 100.0 - 1.5 * (-0.05 * 0.25_f64).exp() - 1.5 * (-0.05 * 0.5_f64).exp();
        assert!((s_adj - expected).abs() < 1e-10);
    }

    #[test]
    fn test_adjust_spot_for_discrete_dividends_rejects_nonpositive_result() {
        let error = adjust_spot_for_discrete_dividends(1.0, 0.01, &[(0.1, 50.0)])
            .expect_err("dividend PV above spot must fail");
        assert!(error
            .to_string()
            .contains("escrowed-dividend model invalid"));
    }

    #[test]
    fn test_adjust_spot_for_discrete_dividends_empty() {
        let s_adj = adjust_spot_for_discrete_dividends(100.0, 0.05, &[]).expect("unchanged spot");
        assert!((s_adj - 100.0).abs() < 1e-12);
    }

    #[test]
    fn test_adjust_spot_for_discrete_dividends_skips_past() {
        // Dividend at t=0 or negative should be skipped
        let s_adj = adjust_spot_for_discrete_dividends(100.0, 0.05, &[(0.0, 5.0), (-0.1, 3.0)])
            .expect("past dividends ignored");
        assert!((s_adj - 100.0).abs() < 1e-12);
    }

    /// Escrowed-dividend rho must include the `∂S*/∂r` chain-rule term.
    ///
    /// With discrete dividends the BS inputs use `S* = S − Σ D·e^{−r·t}`, which
    /// depends on `r`. The analytic rho from `compute_greeks` must therefore
    /// match a finite-difference rho computed by bumping the discount-curve
    /// rate (which re-derives `S*` at the bumped rate). Before the fix, rho
    /// held `S*` fixed and disagreed with the FD rho by `delta·∂S*/∂r`.
    #[test]
    fn escrowed_dividend_rho_includes_spot_rate_sensitivity() {
        let as_of = date(2025, 1, 1);
        let expiry = date(2026, 1, 1); // ~1y
        let mut opt = option(expiry, OptionType::Call, ExerciseStyle::European);
        // A sizeable dividend mid-life makes ∂S*/∂r materially non-zero.
        opt.discrete_dividends = vec![(date(2025, 7, 1), 8.0)];

        let base_rate = 0.04;
        let analytic = compute_greeks(&opt, &market(as_of, 100.0, 0.20, base_rate, 0.0), as_of)
            .expect("analytic greeks")
            .rho;

        // Central finite-difference rho of the full PV over the curve rate.
        // compute_pv re-derives r (and hence S*) from the curve, so this FD
        // captures the ∂S*/∂r contribution that the analytic rho must match.
        let h = 1e-4; // 1bp in rate space
        let pv_up = compute_pv(&opt, &market(as_of, 100.0, 0.20, base_rate + h, 0.0), as_of)
            .expect("pv up")
            .amount();
        let pv_dn = compute_pv(&opt, &market(as_of, 100.0, 0.20, base_rate - h, 0.0), as_of)
            .expect("pv dn")
            .amount();
        // analytic rho is per 1% (100bp); FD slope per unit-rate * 0.01.
        let fd_rho = (pv_up - pv_dn) / (2.0 * h) * 0.01;

        let denom = analytic.abs().max(fd_rho.abs()).max(1e-9);
        assert!(
            (analytic - fd_rho).abs() / denom < 5e-3,
            "escrowed-dividend rho must match FD rho of the full PV (which \
             re-derives S* at the bumped rate): analytic={analytic} fd={fd_rho}"
        );

        // And it must NOT equal the naive rho that holds S* fixed.
        let inputs =
            collect_inputs_extended(&opt, &market(as_of, 100.0, 0.20, base_rate, 0.0), as_of)
                .expect("inputs");
        let naive = bs_greeks_unchecked(
            inputs.spot,
            opt.strike,
            inputs.r,
            inputs.q,
            inputs.sigma,
            inputs.t_vol,
            opt.option_type,
            opt.theta_day_basis.days_per_year(),
        )
        .rho_r
            * opt.notional.amount();
        assert!(
            (analytic - naive).abs() / denom > 1e-3,
            "the ∂S*/∂r correction must move rho away from the S*-fixed value: \
             analytic={analytic} naive={naive}"
        );
    }

    #[test]
    fn heston_fourier_rejects_future_discrete_dividend() {
        let as_of = date(2025, 1, 1);
        let expiry = date(2026, 1, 1);
        let mut opt = option(expiry, OptionType::Call, ExerciseStyle::European);
        opt.discrete_dividends = vec![(date(2025, 7, 1), 2.0)];

        let err = EquityOptionHestonFourierPricer::new()
            .price_dyn(&opt, &market(as_of, 100.0, 0.20, 0.03, 0.0), as_of)
            .expect_err("Heston Fourier must reject discrete dividends");
        let msg = err.to_string();
        assert!(
            msg.contains("discrete dividends"),
            "unexpected error message: {msg}"
        );
    }

    #[test]
    fn cash_settlement_has_zero_delta_at_expiry() {
        let as_of = date(2025, 1, 1);
        let mut call = option(as_of, OptionType::Call, ExerciseStyle::European);
        let mut put = option(as_of, OptionType::Put, ExerciseStyle::European);
        call.exercise = Some(EquityOptionExercise::new(as_of, 100.0, as_of, true));
        put.exercise = Some(EquityOptionExercise::new(as_of, 100.0, as_of, true));
        let curves = market(as_of, 100.0, 0.20, 0.03, 0.01);

        let call_greeks = compute_greeks(&call, &curves, as_of).expect("call greeks");
        let put_greeks = compute_greeks(&put, &curves, as_of).expect("put greeks");

        assert_eq!(call_greeks, EquityOptionGreeks::default());
        assert_eq!(put_greeks, EquityOptionGreeks::default());
    }

    /// Short-dated tree FD gamma must be well-conditioned.
    ///
    /// An American call on a non-dividend-paying underlying is never optimally
    /// exercised early, so its price (and gamma) equals the European value.
    /// For a short-dated near-ATM option the analytic BS gamma is therefore a
    /// reliable oracle. With the old 1%-of-spot gamma bump the tree second
    /// difference is noise-dominated and gamma drifts well off the analytic
    /// value; the wider `σ√t`-scaled bump keeps it close.
    #[test]
    fn short_dated_tree_gamma_is_well_conditioned() {
        let as_of = date(2025, 1, 1);
        // ~3-week expiry: short enough that a 1%-of-spot bump is noise-prone.
        let expiry = date(2025, 1, 22);
        let mut american = option(expiry, OptionType::Call, ExerciseStyle::American);
        american
            .instrument_pricing_overrides
            .model_config
            .tree_steps = Some(201);
        // Zero dividend yield => American call == European call.
        let curves = market(as_of, 100.0, 0.20, 0.03, 0.0);

        let tree_greeks = compute_greeks(&american, &curves, as_of).expect("tree greeks");

        // Analytic European gamma with the same inputs.
        let inputs = collect_inputs_extended(&american, &curves, as_of).expect("inputs");
        let analytic = bs_greeks_unchecked(
            inputs.spot,
            american.strike,
            inputs.r,
            inputs.q,
            inputs.sigma,
            inputs.t_vol,
            american.option_type,
            american.theta_day_basis.days_per_year(),
        )
        .gamma
            * american.notional.amount();

        assert!(
            analytic > 0.0 && tree_greeks.gamma > 0.0,
            "gamma must be positive: analytic={analytic} tree={}",
            tree_greeks.gamma
        );
        let rel_err = (tree_greeks.gamma - analytic).abs() / analytic;
        assert!(
            rel_err < 0.05,
            "short-dated tree gamma must track analytic gamma within 5%: \
             analytic={analytic} tree={} rel_err={rel_err}",
            tree_greeks.gamma
        );
    }

    #[test]
    fn test_american_call_tree_path_prices_above_european() {
        let as_of = date(2025, 1, 1);
        let expiry = date(2025, 7, 1);
        let mut european = option(expiry, OptionType::Call, ExerciseStyle::European);
        let mut american = option(expiry, OptionType::Call, ExerciseStyle::American);
        european
            .instrument_pricing_overrides
            .model_config
            .tree_steps = Some(51);
        american
            .instrument_pricing_overrides
            .model_config
            .tree_steps = Some(51);
        let curves = market(as_of, 105.0, 0.22, 0.03, 0.01);

        let european_pv = compute_pv(&european, &curves, as_of).expect("european pv");
        let american_pv = compute_pv(&american, &curves, as_of).expect("american pv");

        assert!(american_pv.amount().is_finite());
        assert!(american_pv.amount() >= european_pv.amount());
    }

    #[test]
    fn test_bermudan_schedule_filters_invalid_dates_before_tree_pricing() {
        let as_of = date(2025, 1, 1);
        let expiry = date(2025, 7, 1);
        let mut filtered = option(expiry, OptionType::Put, ExerciseStyle::Bermudan);
        let mut noisy = option(expiry, OptionType::Put, ExerciseStyle::Bermudan);
        filtered
            .instrument_pricing_overrides
            .model_config
            .tree_steps = Some(51);
        noisy.instrument_pricing_overrides.model_config.tree_steps = Some(51);
        filtered.exercise_schedule = Some(vec![date(2025, 3, 1), date(2025, 5, 1)]);
        noisy.exercise_schedule = Some(vec![
            as_of,
            date(2024, 12, 15),
            date(2025, 3, 1),
            date(2025, 5, 1),
            date(2025, 8, 1),
        ]);
        let curves = market(as_of, 95.0, 0.25, 0.03, 0.0);

        let filtered_pv = compute_pv(&filtered, &curves, as_of).expect("filtered bermudan pv");
        let noisy_pv = compute_pv(&noisy, &curves, as_of).expect("noisy bermudan pv");

        assert!((filtered_pv.amount() - noisy_pv.amount()).abs() < 1e-10);
    }

    #[test]
    fn canonical_theta_uses_calendar_365_basis() {
        let as_of = date(2025, 1, 1);
        let expiry = date(2026, 1, 1);
        let curves = market(as_of, 100.0, 0.20, 0.03, 0.0);
        let option = option(expiry, OptionType::Call, ExerciseStyle::European);
        let theta = crate::instruments::common_impl::traits::OptionGreeksProvider::option_theta(
            &option, &curves, as_of,
        )
        .expect("theta")
        .expect("supported");
        let inputs = collect_inputs_extended(&option, &curves, as_of).expect("inputs");
        let expected = bs_greeks_unchecked(
            inputs.spot,
            option.strike,
            inputs.r,
            inputs.q,
            inputs.sigma,
            inputs.t_vol,
            option.option_type,
            365.0,
        )
        .theta
            * option.notional.amount();

        assert!((theta - expected).abs() < 1e-12);
    }

    #[test]
    fn option_rejects_spot_price_in_wrong_currency() {
        let as_of = date(2025, 1, 1);
        let expiry = date(2026, 1, 1);
        let curves = market(as_of, 100.0, 0.20, 0.03, 0.0).insert_price(
            "SPX-SPOT",
            MarketScalar::Price(Money::from((100_i64, Currency::EUR))),
        );
        let option = option(expiry, OptionType::Call, ExerciseStyle::European);

        assert!(matches!(
            compute_pv(&option, &curves, as_of),
            Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: Currency::USD,
                actual: Currency::EUR,
            })
        ));
    }

    #[test]
    fn post_expiry_value_and_greeks_are_zero_without_market_data() {
        let expiry = date(2025, 1, 1);
        let as_of = date(2025, 1, 2);
        let mut option = option(expiry, OptionType::Call, ExerciseStyle::European);
        option.exercise = Some(EquityOptionExercise::new(expiry, 100.0, expiry, false));
        let empty = MarketContext::new();
        let pv = compute_pv(&option, &empty, as_of).expect("post-expiry PV");
        let greeks = compute_greeks(&option, &empty, as_of).expect("post-expiry greeks");
        assert_eq!(pv.amount(), 0.0);
        assert_eq!(greeks.delta, 0.0);
        assert_eq!(greeks.gamma, 0.0);
        assert_eq!(greeks.vega, 0.0);
    }
}
