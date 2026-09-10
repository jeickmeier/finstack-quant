//! Shared contractual barrier monitoring and simulation observation grid.
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_models::monte_carlo::payoff::barrier::BarrierMonitoring as McBarrierMonitoring;
use finstack_quant_models::monte_carlo::pricer::path_dependent::PathDependentPricerConfig;
use finstack_quant_models::monte_carlo::TimeGrid;

/// Contractual barrier-monitoring convention.
#[derive(PartialEq, Eq, Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Monitoring {
    /// Monitor continuously through the contractual monitoring interval.
    #[default]
    Continuous,
    /// Monitor only at the stated contractual observation dates.
    Discrete {
        /// Strictly increasing dates on which the barrier level is observed.
        #[serde(with = "finstack_quant_core::wire::dates")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
        )]
        observation_dates: Vec<Date>,
    },
}

impl Monitoring {
    pub(crate) fn time_grid(
        &self,
        as_of: Date,
        day_count: DayCount,
        monitoring_start_date: Option<Date>,
        time_to_maturity: f64,
        config: &PathDependentPricerConfig,
    ) -> finstack_quant_core::Result<(TimeGrid, McBarrierMonitoring)> {
        let date_time =
            |date: Date| day_count.year_fraction(as_of, date, DayCountContext::default());

        let required_times = match self {
            Monitoring::Continuous => {
                let start = monitoring_start_date.unwrap_or(as_of);
                if start <= as_of {
                    vec![0.0]
                } else {
                    vec![date_time(start)?]
                }
            }
            Monitoring::Discrete { observation_dates } => observation_dates
                .iter()
                .copied()
                .filter(|date| *date >= as_of)
                .map(date_time)
                .collect::<finstack_quant_core::Result<Vec<_>>>()?,
        };
        let time_grid = config.build_time_grid(time_to_maturity, &required_times)?;
        let step_for_time = |required: f64| {
            let tolerance = 1.0e-12 * required.abs().max(1.0);
            time_grid
            .times()
            .iter()
            .position(|time| (*time - required).abs() <= tolerance)
            .ok_or_else(|| {
                finstack_quant_core::Error::Internal(format!(
                    "Barrier required monitoring time {required} is missing from the simulation grid"
                ))
            })
        };
        let monitoring = match self {
            Monitoring::Continuous => McBarrierMonitoring::Continuous {
                start_step: step_for_time(required_times[0])?,
            },
            Monitoring::Discrete { .. } => McBarrierMonitoring::Discrete {
                observation_steps: required_times
                    .iter()
                    .copied()
                    .map(step_for_time)
                    .collect::<finstack_quant_core::Result<Vec<_>>>()?,
            },
        };
        Ok((time_grid, monitoring))
    }
}
