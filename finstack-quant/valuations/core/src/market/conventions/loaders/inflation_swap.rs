//! Loader for inflation swap conventions embedded in JSON registries.

use crate::market::conventions::InflationSwapConventions;
use crate::market::conventions::ids::InflationSwapConventionId;
use finstack_quant_core::Error;
use finstack_quant_core::FxHashMap;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount};

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct InflationSwapConventionRecord {
    calendar_id: String,
    settlement_days: i32,
    business_day_convention: BusinessDayConvention,
    day_count: DayCount,
    inflation_lag: String,
    interpolation: finstack_quant_core::market_data::scalars::SeriesInterpolation,
}

impl InflationSwapConventionRecord {
    fn into_conventions(self) -> Result<InflationSwapConventions, Error> {
        let lag =
            super::json::parse_tenor_field(&self.inflation_lag, "inflation_lag", "Inflation Swap")?;
        Ok(InflationSwapConventions {
            calendar_id: self.calendar_id,
            settlement_days: self.settlement_days,
            business_day_convention: self.business_day_convention,
            day_count: self.day_count,
            inflation_lag: lag,
            interpolation: self.interpolation,
        })
    }
}

/// Load the Inflation Swap conventions from the embedded JSON registry.
pub(crate) fn load_registry()
-> Result<FxHashMap<InflationSwapConventionId, InflationSwapConventions>, Error> {
    let json = include_str!("../../../../data/conventions/inflation_swap_conventions.json");
    super::json::parse_and_rekey(
        json,
        "Inflation Swap",
        InflationSwapConventionId::new,
        InflationSwapConventionRecord::into_conventions,
    )
}
