//! Prepared quotes for calibration.
//!
//! Bridges the raw market data schema with the instrument-based calibration solvers.

use crate::instruments::rates::deposit::Deposit;
use crate::instruments::Instrument;
use crate::market::build::prepared::PreparedQuote;
use crate::market::quotes::cds::CdsQuote;
use crate::market::quotes::cds_tranche::CDSTrancheQuote;
use crate::market::quotes::inflation::InflationQuote;
use crate::market::quotes::rates::RateQuote;
use crate::market::quotes::xccy::XccyQuote;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

/// A prepared CDS tranche quote ready for use in calibration.
#[derive(Debug, Clone)]
pub(crate) struct CDSTrancheCalibrationQuote {
    /// Prepared quote with constructed instrument and pillar timing.
    pub(crate) prepared: PreparedQuote<CDSTrancheQuote>,
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
    CDSTranche(CDSTrancheCalibrationQuote),
    /// Inflation quote (ZCIS)
    Inflation(PreparedQuote<InflationQuote>),
    /// Cross-currency basis swap quote (par spread on a fixed-notional or MtM-resetting
    /// XCCY swap). Constructed via `prepare_xccy_quote` in `market::build::prepared`.
    XccyBasis(PreparedQuote<XccyQuote>),
}

impl CalibrationQuote {
    /// Get reference to the underlying instrument.
    pub(crate) fn get_instrument(&self) -> &dyn Instrument {
        match self {
            CalibrationQuote::Rates(q) => q.instrument.as_ref(),
            CalibrationQuote::Cds(q) => q.instrument.as_ref(),
            CalibrationQuote::CDSTranche(q) => q.prepared.instrument.as_ref(),
            CalibrationQuote::Inflation(q) => q.instrument.as_ref(),
            CalibrationQuote::XccyBasis(q) => q.instrument.as_ref(),
        }
    }

    /// Get the pillar time (year fraction from as_of)
    pub(crate) fn pillar_time(&self) -> f64 {
        match self {
            CalibrationQuote::Rates(q) => q.pillar_time,
            CalibrationQuote::Cds(q) => q.pillar_time,
            CalibrationQuote::CDSTranche(q) => q.prepared.pillar_time,
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
