//! `Marginable` implementation for [`InterestRateSwap`].

use super::InterestRateSwap;
use crate::instruments::common_impl::margin::assign_ir_tenor_bucket;
use crate::instruments::common_impl::margin::netting_set_id_from_spec;
use crate::instruments::common_impl::margin::repriced_bucketed_dv01;
use crate::instruments::common_impl::traits::Instrument;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::units::ONE_BASIS_POINT;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_margin::constants::CALENDAR_DAYS_PER_YEAR;
use finstack_quant_margin::constants::DURATION_APPROXIMATION_FACTOR;
use finstack_quant_margin::Marginable;
use finstack_quant_margin::NettingSetId;
use finstack_quant_margin::OtcMarginSpec;
use finstack_quant_margin::SimmSensitivities;

impl Marginable for InterestRateSwap {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn margin_spec(&self) -> Option<&OtcMarginSpec> {
        self.margin_spec.as_ref()
    }

    fn netting_set_id(&self) -> Option<NettingSetId> {
        self.margin_spec.as_ref().map(netting_set_id_from_spec)
    }

    fn simm_sensitivities(&self, market: &MarketContext, as_of: Date) -> Result<SimmSensitivities> {
        let currency = self.notional.currency();
        let mut sens = SimmSensitivities::new(currency);

        let days_to_maturity = (self.float_leg.end - as_of).whole_days().max(0) as f64;
        let years_to_maturity = days_to_maturity / CALENDAR_DAYS_PER_YEAR;

        if years_to_maturity <= 0.0 {
            return Ok(sens);
        }

        // Preferred path: repriced per-tenor (key-rate) DV01 across every rate
        // curve the swap depends on. This respects the actual discount curve,
        // coupon schedule and curve shape rather than a flat
        // `duration ≈ maturity` proxy. The repriced DV01 sign already reflects
        // the swap's `side` (the pricer prices the actual swap).
        let mut rate_curves: Vec<finstack_quant_core::types::CurveId> = self
            .market_dependencies()
            .map(|deps| {
                deps.curves
                    .discount_curves
                    .iter()
                    .chain(deps.curves.forward_curves.iter())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        rate_curves.sort();
        rate_curves.dedup();

        let mut bucket_totals: std::collections::BTreeMap<&'static str, f64> =
            std::collections::BTreeMap::new();
        let mut any_repriced = false;
        for (tenor, dv01) in repriced_bucketed_dv01(self, market, as_of, &rate_curves) {
            any_repriced = true;
            *bucket_totals
                .entry(assign_ir_tenor_bucket(tenor))
                .or_insert(0.0) += dv01;
        }

        if any_repriced {
            for (bucket, dv01) in bucket_totals {
                sens.add_ir_delta(currency, bucket, dv01);
            }
            return Ok(sens);
        }

        // Fallback (no market data / pricer unavailable): the flat
        // duration-proxy DV01 distributed by maturity-overlap weights.
        tracing::warn!(
            instrument_id = self.id.as_str(),
            "SIMM IR delta falling back to duration ≈ maturity proxy: repriced \
             bucketed DV01 unavailable (missing rate curves?)"
        );
        let total_dv01 = self.notional.amount().abs()
            * years_to_maturity
            * DURATION_APPROXIMATION_FACTOR
            * ONE_BASIS_POINT;

        let sign = match self.side {
            crate::instruments::rates::irs::PayReceive::Pay => 1.0,
            crate::instruments::rates::irs::PayReceive::Receive => -1.0,
        };

        let buckets: &[(&str, f64, f64)] = &[
            ("6M", 0.0, 0.5),
            ("1Y", 0.5, 1.0),
            ("2Y", 1.0, 2.0),
            ("3Y", 2.0, 3.0),
            ("5Y", 3.0, 5.0),
            ("10Y", 5.0, 10.0),
            ("15Y", 10.0, 15.0),
            ("20Y", 15.0, 20.0),
            ("30Y", 20.0, 50.0),
        ];

        let mut total_weight = 0.0f64;
        let mut bucket_weights: Vec<(&str, f64)> = Vec::new();
        for &(name, lo, hi) in buckets {
            if years_to_maturity <= lo {
                break;
            }
            let effective_hi = hi.min(years_to_maturity);
            let weight = effective_hi - lo;
            if weight > 0.0 {
                bucket_weights.push((name, weight));
                total_weight += weight;
            }
        }

        if total_weight > 0.0 {
            for (name, weight) in bucket_weights {
                let fraction = weight / total_weight;
                let bucket_dv01 = sign * total_dv01 * fraction;
                sens.add_ir_delta(currency, name, bucket_dv01);
            }
        }

        Ok(sens)
    }

    fn mtm_for_vm(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        Instrument::value(self, market, as_of)
    }
}

#[cfg(test)]
mod tests {
    #[allow(dead_code, unused_imports)]
    mod rates_support {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/rates.rs"
        ));
    }
    #[allow(dead_code, unused_imports)]
    mod discount_forward_curve_support {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/discount_forward_curves.rs"
        ));
    }
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_margin::ClearingStatus;
    use time::Month;

    fn test_date() -> Date {
        Date::from_calendar_date(2024, Month::June, 15).expect("valid date")
    }

    #[test]
    fn test_irs_marginable() {
        let start = test_date();
        let end = Date::from_calendar_date(2029, Month::June, 15).expect("valid date");

        let swap = rates_support::usd_irs_swap(
            "TEST_IRS",
            Money::from((100_000_000_i64, Currency::USD)),
            0.035,
            start,
            end,
            crate::instruments::rates::irs::PayReceive::Pay,
        )
        .expect("swap creation");

        // Without margin spec
        assert!(swap.margin_spec().is_none());
        assert!(!swap.has_margin());

        // Calculate sensitivities
        let market = MarketContext::new();
        let sens = swap
            .simm_sensitivities(&market, start)
            .expect("sensitivities");

        // Should have IR delta
        assert!(!sens.ir_delta.is_empty());
        assert!(sens.total_ir_delta().abs() > 0.0);
    }

    #[test]
    fn test_irs_multi_tenor_decomposition() {
        let start = test_date();
        let end = Date::from_calendar_date(2034, Month::June, 15).expect("valid date");

        let swap = rates_support::usd_irs_swap(
            "TEST_IRS_10Y",
            Money::from((100_000_000_i64, Currency::USD)),
            0.035,
            start,
            end,
            crate::instruments::rates::irs::PayReceive::Pay,
        )
        .expect("swap creation");

        let market = MarketContext::new();
        let sens = swap
            .simm_sensitivities(&market, start)
            .expect("sensitivities");

        assert!(
            sens.ir_delta.len() > 1,
            "Expected multi-tenor decomposition"
        );
        assert!(
            sens.total_ir_delta() > 0.0,
            "Pay fixed gains when floating rates rise"
        );
    }

    /// Audit item #10: with a real market the SIMM IR delta must come from a
    /// repriced (curve-aware) bucketed DV01, not the flat `duration ≈ maturity`
    /// proxy. We assert the repriced path is taken and that it disagrees with
    /// the proxy (the proxy ignores discounting / coupon / curve shape).
    #[test]
    fn test_irs_simm_uses_repriced_dv01_with_market() {
        let start = test_date();
        let end = Date::from_calendar_date(2034, Month::June, 15).expect("valid date");
        let notional = Money::from((100_000_000_i64, Currency::USD));

        let swap = rates_support::usd_irs_swap(
            "TEST_IRS_REPRICED",
            notional,
            0.035,
            start,
            end,
            crate::instruments::rates::irs::PayReceive::Pay,
        )
        .expect("swap creation");

        // Real market: USD-OIS discount + USD-SOFR-3M forward, both flat at 3%.
        let market = MarketContext::new()
            .insert(discount_forward_curve_support::flat_discount_with_tenor(
                "USD-OIS", start, 0.03, 30.0,
            ))
            .insert(discount_forward_curve_support::flat_forward_with_tenor(
                "USD-SOFR-3M",
                start,
                0.03,
                30.0,
            ));

        let sens = swap
            .simm_sensitivities(&market, start)
            .expect("repriced SIMM sensitivities");

        assert!(!sens.ir_delta.is_empty(), "must produce IR delta buckets");
        let repriced_total = sens.total_ir_delta();
        assert!(
            repriced_total.is_finite() && repriced_total.abs() > 0.0,
            "repriced IR delta must be finite and non-zero, got {repriced_total}"
        );

        // The flat duration ≈ maturity proxy total (the OLD behavior) for a
        // ~10y swap: notional × 10 × 0.0001 (DURATION_APPROXIMATION_FACTOR) ×
        // ONE_BASIS_POINT, signed positive for Pay-fixed. The repriced curve-aware DV01
        // genuinely differs from this crude proxy.
        let proxy_total =
            notional.amount().abs() * 10.0 * DURATION_APPROXIMATION_FACTOR * ONE_BASIS_POINT;
        assert!(
            (repriced_total - proxy_total).abs() > 1e-6,
            "repriced DV01 ({repriced_total}) must differ from the duration proxy ({proxy_total})"
        );
    }

    #[test]
    fn test_netting_set_from_cleared_spec() {
        use finstack_quant_margin::types::CsaSpec;

        let start = test_date();
        let end = Date::from_calendar_date(2029, Month::June, 15).expect("valid date");

        let mut swap = rates_support::usd_irs_swap(
            "TEST_IRS",
            Money::from((100_000_000_i64, Currency::USD)),
            0.035,
            start,
            end,
            crate::instruments::rates::irs::PayReceive::Pay,
        )
        .expect("swap creation");

        // Add cleared margin spec
        swap.margin_spec = Some(OtcMarginSpec {
            csa: CsaSpec::usd_regulatory().expect("registry should load"),
            clearing_status: ClearingStatus::Cleared {
                ccp: "LCH".to_string(),
            },
            simm_credit_classification: None,
        });

        let netting_set = swap.netting_set_id().expect("netting set");
        assert!(netting_set.is_cleared());
        assert_eq!(netting_set.ccp_id(), Some("LCH"));
    }

    #[test]
    fn same_curve_simm_matches_one_bucketed_risk_result() {
        let start = test_date();
        let end = Date::from_calendar_date(2034, Month::June, 15).expect("end");
        let mut swap = rates_support::usd_irs_swap(
            "SHARED",
            Money::from((100_000_000_i64, Currency::USD)),
            0.035,
            start,
            end,
            crate::instruments::rates::irs::PayReceive::Pay,
        )
        .expect("swap");
        swap.float_leg.forward_curve_id = swap.fixed_leg.discount_curve_id.clone();
        swap.float_leg.compounding =
            finstack_quant_cashflows::builder::FloatingLegCompounding::CompoundedInArrears {
                lookback_days: 0,
            };
        let market = MarketContext::new().insert(
            discount_forward_curve_support::flat_discount_with_tenor("USD-OIS", start, 0.03, 30.0),
        );
        let result = swap
            .price_with_metrics(
                &market,
                start,
                &[crate::metrics::MetricId::BucketedDv01],
                crate::instruments::PricingOptions::default(),
            )
            .expect("risk");
        let expected: f64 = crate::metrics::STANDARD_BUCKET_LABELS
            .iter()
            .map(|label| {
                let key = crate::metrics::MetricId::composite(
                    &crate::metrics::MetricId::BucketedDv01,
                    &["USD-OIS", label],
                );
                result.measures[key.as_str()]
            })
            .sum();
        assert!(expected.abs() > 0.0);
        let actual = swap
            .simm_sensitivities(&market, start)
            .expect("SIMM")
            .total_ir_delta();
        assert!((actual - expected).abs() < expected.abs() * 1e-12);
    }
}
