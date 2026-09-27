//! Prepared quotes for calibration.
//!
//! Bridges the raw market data schema with the instrument-based calibration solvers.

use crate::build::prepared::PreparedQuote;
use crate::quotes::cds::CdsQuote;
use crate::quotes::cds_tranche::CdsTrancheQuote;
use crate::quotes::inflation::InflationQuote;
use crate::quotes::rates::RateQuote;
use crate::quotes::xccy::XccyQuote;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use finstack_quant_valuations::instruments::Instrument;

/// A prepared CDS tranche quote ready for use in calibration.
#[derive(Debug, Clone)]
pub(crate) struct CdsTrancheCalibrationQuote {
    /// Prepared quote with constructed instrument and pillar timing.
    pub(crate) prepared: PreparedQuote<CdsTrancheQuote>,
    /// Optional upfront cashflow from the market quote.
    pub(crate) upfront: Option<Money>,
    /// Detachment point in percentage terms (e.g. 3.0 for 3%).
    pub(crate) detachment_pct: f64,
}

/// A prepared quote ready for use in calibration.
///
/// This wraps a Quote (data), a prepared Instrument (constructed via builder),
/// and the pre-calculated pillar time.
#[derive(Debug, Clone)]
pub(crate) enum CalibrationQuote {
    /// Rates quote (Deposit, FRA, Swap, Future)
    Rates(PreparedQuote<RateQuote>),
    /// Credit quote (CDS Par Spread, Upfront).
    Cds(PreparedQuote<CdsQuote>),
    /// CDS Tranche quote.
    CdsTranche(CdsTrancheCalibrationQuote),
    /// Inflation quote (ZCIS)
    Inflation(PreparedQuote<InflationQuote>),
    /// Cross-currency basis swap quote (par spread on a fixed-notional or MtM-resetting
    /// XCCY swap). Constructed via `prepare_xccy_quote` in `build::prepared`.
    XccyBasis(PreparedQuote<XccyQuote>),
}

impl CalibrationQuote {
    /// Identifier of the underlying market quote as supplied in the envelope.
    pub(crate) fn quote_id(&self) -> &str {
        match self {
            CalibrationQuote::Rates(q) => q.quote.id().as_str(),
            CalibrationQuote::Cds(q) => q.quote.id().as_str(),
            CalibrationQuote::CdsTranche(q) => q.prepared.quote.id.as_str(),
            CalibrationQuote::Inflation(q) => q.quote.id().as_str(),
            CalibrationQuote::XccyBasis(q) => q.quote.id().as_str(),
        }
    }

    /// Get reference to the underlying instrument.
    pub(crate) fn get_instrument(&self) -> &dyn Instrument {
        match self {
            CalibrationQuote::Rates(q) => q.instrument.as_ref(),
            CalibrationQuote::Cds(q) => q.instrument.as_ref(),
            CalibrationQuote::CdsTranche(q) => q.prepared.instrument.as_ref(),
            CalibrationQuote::Inflation(q) => q.instrument.as_ref(),
            CalibrationQuote::XccyBasis(q) => q.instrument.as_ref(),
        }
    }

    /// Get the pillar time (year fraction from as_of)
    pub(crate) fn pillar_time(&self) -> f64 {
        match self {
            CalibrationQuote::Rates(q) => q.pillar_time,
            CalibrationQuote::Cds(q) => q.pillar_time,
            CalibrationQuote::CdsTranche(q) => q.prepared.pillar_time,
            CalibrationQuote::Inflation(q) => q.pillar_time,
            CalibrationQuote::XccyBasis(q) => q.pillar_time,
        }
    }

    /// Price a quote using calibration residual semantics.
    ///
    /// T+0 deposits retain their initial exchange so a par quote produces zero
    /// trade NPV. Other instruments use the canonical raw holder-view value.
    pub(crate) fn calibration_value_raw(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        if let CalibrationQuote::Rates(prepared) = self {
            if matches!(prepared.quote.as_ref(), RateQuote::Deposit { .. }) {
                let deposit = prepared
                    .instrument
                    .as_any()
                    .downcast_ref::<Deposit>()
                    .ok_or_else(|| {
                        finstack_quant_core::Error::Validation(format!(
                            "Prepared deposit quote '{}' does not contain a Deposit instrument",
                            prepared.quote.id()
                        ))
                    })?;
                return deposit.npv_raw(context, as_of);
            }
        }

        self.get_instrument().value_raw(context, as_of)
    }
}
