//! Serde wire type for [`super::Performance`]; caches are rebuilt on load.

use super::{Performance, TickerSpan, invalid_return_series};
use finstack_quant_core::dates::{Date, PeriodKind};

#[derive(serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub(super) struct PerformanceState {
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    price_dates: Vec<Date>,
    returns: Vec<Vec<f64>>,
    return_spans: Vec<TickerSpan>,
    ticker_names: Vec<String>,
    benchmark_idx: Option<usize>,
    frequency: Option<PeriodKind>,
    start_idx: usize,
    end_idx: usize,
}

impl TryFrom<PerformanceState> for Performance {
    type Error = finstack_quant_core::Error;

    fn try_from(state: PerformanceState) -> Result<Self, Self::Error> {
        let benchmark = state
            .benchmark_idx
            .map(|index| {
                state.ticker_names.get(index).cloned().ok_or_else(|| {
                    invalid_return_series("<panel>", index, "benchmark index out of range")
                })
            })
            .transpose()?;
        if state.start_idx > state.end_idx
            || state.end_idx > state.price_dates.len().saturating_sub(1)
        {
            return Err(invalid_return_series(
                "<panel>",
                state.start_idx,
                "active window out of range",
            )
            .into());
        }
        let mut perf = Self::assemble(
            state.price_dates,
            state.returns,
            state.return_spans,
            state.ticker_names,
            benchmark.as_deref(),
            state.frequency,
        )?;
        perf.start_idx = state.start_idx;
        perf.end_idx = state.end_idx;
        perf.refresh_active_drawdown_cache()?;
        Ok(perf)
    }
}

/// Raw strict lookback carrier, admitted against resolved calendar boundaries.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LookbackReturnsState {
    ref_date: Date,
    ticker_names: Vec<String>,
    fiscal_config: finstack_quant_core::dates::FiscalConfig,
    #[serde(with = "finstack_quant_core::wire::non_finite_f64_seq")]
    mtd: Vec<f64>,
    #[serde(with = "finstack_quant_core::wire::non_finite_f64_seq")]
    qtd: Vec<f64>,
    #[serde(with = "finstack_quant_core::wire::non_finite_f64_seq")]
    ytd: Vec<f64>,
    #[serde(with = "finstack_quant_core::wire::non_finite_f64_seq")]
    fytd: Vec<f64>,
    mtd_samples: Vec<crate::aggregation::LookbackSample>,
    qtd_samples: Vec<crate::aggregation::LookbackSample>,
    ytd_samples: Vec<crate::aggregation::LookbackSample>,
    fytd_samples: Vec<crate::aggregation::LookbackSample>,
}

impl TryFrom<LookbackReturnsState> for super::LookbackReturns {
    type Error = finstack_quant_core::Error;
    fn try_from(state: LookbackReturnsState) -> Result<Self, Self::Error> {
        let result = Self {
            ref_date: state.ref_date,
            ticker_names: state.ticker_names,
            fiscal_config: state.fiscal_config,
            mtd: state.mtd,
            qtd: state.qtd,
            ytd: state.ytd,
            fytd: state.fytd,
            mtd_samples: state.mtd_samples,
            qtd_samples: state.qtd_samples,
            ytd_samples: state.ytd_samples,
            fytd_samples: state.fytd_samples,
        };
        result.validate()?;
        Ok(result)
    }
}

impl super::LookbackReturns {
    pub(crate) fn validate(&self) -> finstack_quant_core::Result<()> {
        use finstack_quant_core::dates::DateExt;
        let error = |message: &str| finstack_quant_core::Error::Validation(message.to_string());
        let mut names = std::collections::HashSet::new();
        if self.ticker_names.is_empty()
            || self
                .ticker_names
                .iter()
                .any(|name| name.trim().is_empty() || !names.insert(name))
        {
            return Err(error("lookback ticker names must be nonempty and unique"));
        }
        let make_start = |month| {
            Date::from_calendar_date(self.ref_date.year(), month, 1)
                .map_err(|err| finstack_quant_core::Error::Validation(err.to_string()))
        };
        let starts = [
            make_start(self.ref_date.month())?,
            make_start(time::Month::January.nth_next((self.ref_date.quarter() - 1) * 3))?,
            make_start(time::Month::January)?,
            self.fiscal_config
                .year_start(self.ref_date.fiscal_year(self.fiscal_config))?,
        ];
        let horizons = [
            (&self.mtd, &self.mtd_samples),
            (&self.qtd, &self.qtd_samples),
            (&self.ytd, &self.ytd_samples),
            (&self.fytd, &self.fytd_samples),
        ];
        for ((values, samples), start) in horizons.into_iter().zip(starts) {
            if values.len() != self.ticker_names.len() || samples.len() != self.ticker_names.len() {
                return Err(error(
                    "lookback values and sample vectors must match ticker names",
                ));
            }
            for (&value, sample) in values.iter().zip(samples) {
                sample.validate()?;
                if sample.n_obs == 0 {
                    if !value.is_nan() {
                        return Err(error("empty lookback sample must have undefined return"));
                    }
                } else if !value.is_finite()
                    || value < -1.0
                    || sample
                        .holding_end
                        .is_none_or(|end| end < start || end > self.ref_date)
                {
                    return Err(error(
                        "nonempty lookback requires finite return and actual ending bound inside its horizon",
                    ));
                }
            }
        }
        Ok(())
    }
}
