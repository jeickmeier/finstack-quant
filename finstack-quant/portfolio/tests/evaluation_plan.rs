//! Public-boundary acceptance tests for the request-scoped evaluation engine.
//!
//! Exact job-ID and cache-construction details remain crate-private and are
//! covered by the evaluation module's unit tests. These tests exercise the
//! public compatibility adapters that must compile requests into that engine.

use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::types::Entity;
use finstack_quant_portfolio::valuation::{
    value_portfolio, value_portfolio_at, PortfolioValuationOptions, RequestedMetrics,
};
use finstack_quant_portfolio::{Portfolio, PortfolioBuilder};
use finstack_quant_valuations::instruments::{Attributes, Instrument, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::InstrumentType;
use finstack_quant_valuations::results::ValuationResult;
use std::any::Any;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use time::macros::date;

#[derive(Clone, Default)]
struct Probe {
    base_calls: Arc<AtomicUsize>,
    pv_only_calls: Arc<AtomicUsize>,
    metric_calls: Arc<AtomicUsize>,
    observed_dates: Arc<Mutex<Vec<Date>>>,
    observed_metrics: Arc<Mutex<Vec<Vec<MetricId>>>>,
}

#[derive(Clone)]
struct ProbedInstrument {
    id: String,
    attributes: Attributes,
    value: f64,
    fail_base: bool,
    fail_metrics: bool,
    probe: Probe,
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    ProbedInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl Instrument for ProbedInstrument {
    fn id(&self) -> &str {
        &self.id
    }

    fn key(&self) -> InstrumentType {
        InstrumentType::Basket
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    fn attributes_mut(&mut self) -> &mut Attributes {
        &mut self.attributes
    }

    fn clone_box(&self) -> Box<dyn Instrument> {
        Box::new(self.clone())
    }

    fn base_value(
        &self,
        _market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        self.probe.base_calls.fetch_add(1, Ordering::SeqCst);
        self.probe
            .observed_dates
            .lock()
            .expect("date probe lock")
            .push(as_of);
        if self.fail_base {
            return Err(finstack_quant_core::Error::Validation(format!(
                "configured PV failure for {}",
                self.id
            )));
        }
        Ok(Money::new(self.value, Currency::USD))
    }

    fn price_with_metrics(
        &self,
        _market: &MarketContext,
        as_of: Date,
        metrics: &[MetricId],
        options: PricingOptions,
    ) -> finstack_quant_core::Result<ValuationResult> {
        if metrics.is_empty() {
            self.probe.pv_only_calls.fetch_add(1, Ordering::SeqCst);
        } else {
            self.probe.metric_calls.fetch_add(1, Ordering::SeqCst);
        }
        self.probe
            .observed_dates
            .lock()
            .expect("date probe lock")
            .push(as_of);
        self.probe
            .observed_metrics
            .lock()
            .expect("metric probe lock")
            .push(metrics.to_vec());
        if (metrics.is_empty() && self.fail_base) || (!metrics.is_empty() && self.fail_metrics) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "configured {} failure for {}",
                if metrics.is_empty() { "PV" } else { "metric" },
                self.id
            )));
        }
        let config = options.config.as_deref().ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "evaluation executor did not attach its request config".to_string(),
            )
        })?;
        Ok(ValuationResult::stamped_with_config(
            self.id(),
            as_of,
            Money::new(self.value, Currency::USD),
            config,
        ))
    }
}

fn build_probed_portfolio(
    position_count: usize,
    fail_base_indices: &[usize],
    fail_metrics: bool,
) -> (Portfolio, Vec<Probe>) {
    let mut builder = PortfolioBuilder::new(format!("EVALUATION_{position_count}"))
        .base_ccy(Currency::USD)
        .as_of(date!(2024 - 01 - 01))
        .entity(Entity::new("ENTITY"));
    let mut probes = Vec::with_capacity(position_count);

    for index in 0..position_count {
        let probe = Probe::default();
        let instrument_id = format!("INSTRUMENT_{index:04}");
        let instrument: Arc<dyn Instrument> = Arc::new(ProbedInstrument {
            id: instrument_id.clone(),
            attributes: Attributes::new(),
            value: (index + 1) as f64,
            fail_base: fail_base_indices.contains(&index),
            fail_metrics,
            probe: probe.clone(),
        });
        builder = builder.position(
            Position::new(
                format!("POSITION_{index:04}"),
                "ENTITY",
                instrument_id,
                instrument,
                1.0,
                PositionUnit::Units,
            )
            .expect("valid probed position"),
        );
        probes.push(probe);
    }

    (builder.build().expect("valid probed portfolio"), probes)
}

fn pv_only_options() -> PortfolioValuationOptions {
    PortfolioValuationOptions {
        strict_risk: true,
        metrics: RequestedMetrics::Only(Vec::new()),
    }
}

#[test]
fn public_profile_normalization_preserves_pv_only_and_stable_metric_order() {
    let (portfolio, probes) = build_probed_portfolio(1, &[], false);
    let market = MarketContext::new();
    let config = FinstackConfig::default();

    value_portfolio(&portfolio, &market, &config, &pv_only_options()).expect("PV-only valuation");
    assert_eq!(probes[0].base_calls.load(Ordering::SeqCst), 0);
    assert_eq!(probes[0].pv_only_calls.load(Ordering::SeqCst), 1);
    assert_eq!(probes[0].metric_calls.load(Ordering::SeqCst), 0);

    let metric_options = PortfolioValuationOptions {
        strict_risk: true,
        metrics: RequestedMetrics::Only(vec![MetricId::Dv01, MetricId::Theta, MetricId::Dv01]),
    };
    value_portfolio(&portfolio, &market, &config, &metric_options).expect("exact-metric valuation");

    assert_eq!(probes[0].metric_calls.load(Ordering::SeqCst), 1);
    let observed_metrics = probes[0]
        .observed_metrics
        .lock()
        .expect("metric probe lock");
    assert_eq!(observed_metrics[0], Vec::<MetricId>::new());
    assert_eq!(
        observed_metrics[1],
        vec![MetricId::Dv01, MetricId::Theta],
        "the executor must stably deduplicate an exact metric profile"
    );
}

#[test]
fn strict_and_best_effort_profiles_remain_distinct() {
    let (portfolio, probes) = build_probed_portfolio(1, &[], true);
    let market = MarketContext::new();
    let config = FinstackConfig::default();
    let metrics = RequestedMetrics::Only(vec![MetricId::Dv01]);

    let best_effort = value_portfolio(
        &portfolio,
        &market,
        &config,
        &PortfolioValuationOptions {
            strict_risk: false,
            metrics: metrics.clone(),
        },
    )
    .expect("best-effort valuation should retain PV");
    assert_eq!(best_effort.degraded_positions().len(), 1);
    assert!(!best_effort.position_values["POSITION_0000"].risk_metrics_complete);

    let strict_error = value_portfolio(
        &portfolio,
        &market,
        &config,
        &PortfolioValuationOptions {
            strict_risk: true,
            metrics,
        },
    )
    .expect_err("strict profile must not reuse a degraded result");
    assert!(strict_error
        .to_string()
        .contains("configured metric failure for INSTRUMENT_0000"));
    assert_eq!(probes[0].metric_calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        probes[0].base_calls.load(Ordering::SeqCst),
        1,
        "only best-effort metric pricing may fall back to PV"
    );
}

#[test]
fn explicit_dates_remain_isolated_and_stamp_each_result() {
    let (portfolio, probes) = build_probed_portfolio(1, &[], false);
    let market = MarketContext::new();
    let config = FinstackConfig::default();
    let first_date = portfolio.as_of;
    let second_date = first_date + time::Duration::days(1);
    let options = PortfolioValuationOptions {
        strict_risk: true,
        metrics: RequestedMetrics::Only(vec![MetricId::Theta]),
    };

    let first = value_portfolio_at(&portfolio, &market, &config, &options, first_date)
        .expect("first dated valuation");
    let second = value_portfolio_at(&portfolio, &market, &config, &options, second_date)
        .expect("second dated valuation");

    assert_eq!(first.as_of, first_date);
    assert_eq!(second.as_of, second_date);
    assert_eq!(
        first.position_values["POSITION_0000"]
            .valuation_result
            .as_ref()
            .expect("first valuation stamp")
            .as_of,
        first_date
    );
    assert_eq!(
        second.position_values["POSITION_0000"]
            .valuation_result
            .as_ref()
            .expect("second valuation stamp")
            .as_of,
        second_date
    );
    assert_eq!(
        probes[0]
            .observed_dates
            .lock()
            .expect("date probe lock")
            .as_slice(),
        &[first_date, second_date],
        "a result from one registered date must not satisfy another date"
    );
}

#[test]
fn rayon_threshold_63_and_64_preserves_order_and_totals() {
    let market = MarketContext::new();
    let config = FinstackConfig::default();

    for position_count in [63usize, 64] {
        let (portfolio, probes) = build_probed_portfolio(position_count, &[], false);
        let valuation = value_portfolio(&portfolio, &market, &config, &pv_only_options())
            .expect("threshold valuation");
        let expected_ids = (0..position_count)
            .map(|index| format!("POSITION_{index:04}"))
            .collect::<Vec<_>>();
        let actual_ids = valuation
            .position_values
            .keys()
            .map(ToString::to_string)
            .collect::<Vec<_>>();

        assert_eq!(actual_ids, expected_ids);
        assert_eq!(
            valuation.total_base_ccy.amount(),
            (position_count * (position_count + 1) / 2) as f64
        );
        assert_eq!(
            probes
                .iter()
                .map(|probe| probe.pv_only_calls.load(Ordering::SeqCst))
                .sum::<usize>(),
            position_count
        );
    }
}

#[test]
fn parallel_position_failures_select_the_earliest_logical_position() {
    let (portfolio, _) = build_probed_portfolio(64, &[3, 51], false);
    let market = MarketContext::new();
    let config = FinstackConfig::default();

    for _ in 0..12 {
        let error = value_portfolio(&portfolio, &market, &config, &pv_only_options())
            .expect_err("configured valuation must fail");
        let message = error.to_string();
        assert!(
            message.contains("POSITION_0003"),
            "parallel completion order must not replace the first logical error: {message}"
        );
        assert!(!message.contains("POSITION_0051"));
    }
}
