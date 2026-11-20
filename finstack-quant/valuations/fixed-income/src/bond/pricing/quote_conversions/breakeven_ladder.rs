//! Carry and signed breakeven shifts across bond horizons using the pricing registry.

use crate::bond::Bond;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::table::{TableColumn, TableColumnData, TableEnvelope};
use finstack_quant_core::{Error, Result};
use finstack_quant_valuations_core::instruments::{
    BreakevenConfig, BreakevenMode, BreakevenTarget, Instrument, PricingOptions,
};
use finstack_quant_valuations_core::metrics::MetricId;
use serde::{Deserialize, Serialize};

/// One carry horizon with its sensitivity and solved signed shift.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BondBreakevenRow {
    /// Requested calendar tenor measured from the valuation date.
    pub horizon: Tenor,
    /// Net carry and roll-down in the bond currency over the allowed horizon.
    pub carry: f64,
    /// Signed starting-date price change per basis point of the selected risk factor.
    pub sensitivity: f64,
    /// Signed basis points of movement that offset carry under the selected calculation.
    pub breakeven_bp: f64,
}

/// Safely repriced carry and breakeven across caller-selected bond horizons.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BondBreakevenLadder {
    /// Identifier of the priced bond.
    pub instrument_id: String,
    /// Valuation date anchoring the horizons.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub as_of: Date,
    /// Native currency of carry and sensitivity.
    pub currency: Currency,
    /// Target and solve mode used for each row.
    pub config: BreakevenConfig,
    /// Rows in the requested horizon order.
    pub rows: Vec<BondBreakevenRow>,
}

impl BondBreakevenLadder {
    /// Export the ordered horizon, carry, sensitivity and signed breakeven columns.
    #[must_use]
    pub fn to_table(&self) -> TableEnvelope {
        TableEnvelope {
            row_count: self.rows.len(),
            metadata: Default::default(),
            columns: vec![
                TableColumn::new(
                    "horizon",
                    TableColumnData::String(
                        self.rows
                            .iter()
                            .map(|row| row.horizon.to_string())
                            .collect(),
                    ),
                ),
                TableColumn::new(
                    "carry",
                    TableColumnData::Float64(self.rows.iter().map(|row| row.carry).collect()),
                ),
                TableColumn::new(
                    "sensitivity",
                    TableColumnData::Float64(self.rows.iter().map(|row| row.sensitivity).collect()),
                ),
                TableColumn::new(
                    "breakeven_bp",
                    TableColumnData::Float64(
                        self.rows.iter().map(|row| row.breakeven_bp).collect(),
                    ),
                ),
                TableColumn::new(
                    "currency",
                    TableColumnData::String(vec![self.currency.to_string(); self.rows.len()]),
                ),
            ],
        }
    }
}

/// Compute the registry's signed bond breakeven across multiple carry horizons.
///
/// The bond is cloned once and its existing pricing and metric overrides stay
/// in effect except for the explicitly supplied horizon, target and mode.
/// Registry errors retain their original meaning. An OAS solve that moves the
/// discount curve is excluded because it cannot identify an OAS breakeven.
///
/// # Arguments
/// * `bond` - Bond whose native cashflows, holder conventions and pricing settings determine carry.
/// * `market` - Opening market supplying the bond's pricing and risk inputs.
/// * `as_of` - Valuation date anchoring each requested calendar horizon.
/// * `horizons` - Positive calendar tenors in desired row order.
/// * `config` - Bond yield or spread target and selected linear or iterative convention.
/// * `options` - Standard pricing options, including any selected model and quote-recalibration provider.
///
/// # Errors
/// Rejects invalid horizons, unsupported target/solve combinations, missing
/// prices or sensitivities, and undefined or nonfinite breakeven results.
pub fn bond_breakeven_ladder(
    bond: &Bond,
    market: &MarketContext,
    as_of: Date,
    horizons: &[Tenor],
    config: BreakevenConfig,
    options: PricingOptions,
) -> Result<BondBreakevenLadder> {
    if horizons.is_empty() || horizons.iter().any(|horizon| horizon.count() == 0) {
        return Err(Error::Validation(
            "Breakeven horizons must contain positive tenors".into(),
        ));
    }
    if !matches!(
        config.target,
        BreakevenTarget::ZSpread | BreakevenTarget::Ytm | BreakevenTarget::Oas
    ) || (config.target == BreakevenTarget::Oas && config.mode == BreakevenMode::Iterative)
    {
        return Err(Error::Validation(
            "Unsupported bond breakeven target or mode".into(),
        ));
    }
    if (config.target == BreakevenTarget::Oas && !bond.has_exercise_rights())
        || (config.target == BreakevenTarget::ZSpread && bond.has_exercise_rights())
    {
        return Err(Error::Validation(
            "Use OAS for option-bearing bonds and Z-spread for vanilla bonds".into(),
        ));
    }
    let mut instrument = bond.clone();
    let template = instrument
        .metric_pricing_overrides
        .clone()
        .with_breakeven_config(config);
    let sensitivity = config.target.sensitivity_metric();
    let metrics = [
        MetricId::CarryTotal,
        sensitivity.clone(),
        MetricId::Breakeven,
    ];
    let mut rows = Vec::with_capacity(horizons.len());
    for &horizon in horizons {
        instrument.metric_pricing_overrides = template.clone().with_theta_period(horizon);
        let priced = instrument.price_with_metrics(market, as_of, &metrics, options.clone())?;
        let read = |id: &MetricId| {
            priced
                .metric(id.clone())
                .filter(|value| value.is_finite())
                .ok_or_else(|| {
                    Error::Validation(format!(
                        "Missing finite {id} for breakeven horizon {horizon}"
                    ))
                })
        };
        if sensitivity == MetricId::Cs01
            && priced
                .metric(MetricId::composite(&MetricId::Cs01, &[bond.id()]))
                .is_none()
        {
            return Err(Error::Validation(
                "Bond spread breakeven requires instrument spread CS01".into(),
            ));
        }
        rows.push(BondBreakevenRow {
            horizon,
            carry: read(&MetricId::CarryTotal)?,
            sensitivity: read(&sensitivity)?,
            breakeven_bp: read(&MetricId::Breakeven)?,
        });
    }
    Ok(BondBreakevenLadder {
        instrument_id: bond.id().to_owned(),
        as_of,
        currency: bond.notional.currency(),
        config,
        rows,
    })
}
