//! Typed wire examples for exposure and ordinary regulatory quote risk.
//!
//! The output examples are illustrative signed reprices, not calibrated golden
//! results. Their explicit example fingerprints do not claim market provenance.

use crate::margin::{
    CsrQuoteClassification, CsrQuoteDelta, CsrQuoteRisk, CsrQuoteSource, ExposureCashVm,
    ExposureFactor, ExposureSimulationConfig, GirrQuoteDelta, GirrQuoteRisk, GirrQuoteSource,
    GirrQuoteTarget, PortfolioFrtbConfig, PortfolioFrtbSensitivitiesResult, RegulatoryQuoteReprice,
};
use finstack_quant_calibration::recalibration::CalibrationQuoteCoordinate;
use finstack_quant_core::market_data::term_structures::Seniority;
use finstack_quant_core::{Result, currency::Currency, dates::Date, money::Money, schema::example};
use finstack_quant_margin::{
    regulatory::frtb::FrtbSensitivities,
    types::{CsaSpec, NettingSetId},
};
use finstack_quant_models::monte_carlo::{
    process::gbm::GbmParams,
    simulate::{PathSimulationSpec, ProcessSpec, SchemeSpec, TimeGridSpec},
};
use serde_json::Value;
use time::macros::date;

const AS_OF: Date = date!(2024 - 01 - 02);
const GIRR_VERTICES: [(&str, Date); 10] = [
    ("0.25Y", date!(2024 - 04 - 02)),
    ("0.5Y", date!(2024 - 07 - 02)),
    ("1Y", date!(2025 - 01 - 02)),
    ("2Y", date!(2026 - 01 - 02)),
    ("3Y", date!(2027 - 01 - 02)),
    ("5Y", date!(2029 - 01 - 02)),
    ("10Y", date!(2034 - 01 - 02)),
    ("15Y", date!(2039 - 01 - 02)),
    ("20Y", date!(2044 - 01 - 02)),
    ("30Y", date!(2054 - 01 - 02)),
];
const CSR_VERTICES: [(&str, Date); 5] = [
    ("0.5Y", date!(2024 - 07 - 02)),
    ("1Y", date!(2025 - 01 - 02)),
    ("3Y", date!(2027 - 01 - 02)),
    ("5Y", date!(2029 - 01 - 02)),
    ("10Y", date!(2034 - 01 - 02)),
];

fn cash_vm() -> Result<ExposureCashVm> {
    Ok(ExposureCashVm {
        terms: CsaSpec::regulatory_for_currency(Currency::USD, "EXAMPLE-CSA", "USD-OIS")?,
        opening_settled: Money::from((0_i64, Currency::USD)),
    })
}

pub(super) fn exposure_cash_vm() -> Result<Vec<Value>> {
    example(&cash_vm()?)
}

pub(super) fn exposure_simulation_config() -> Result<Vec<Value>> {
    let dates = vec![AS_OF, date!(2024 - 02 - 02)];
    let times = dates
        .iter()
        .map(|date| (*date - AS_OF).whole_days() as f64 / 365.0)
        .collect();
    example(&ExposureSimulationConfig {
        netting_set: NettingSetId::bilateral("EXAMPLE-COUNTERPARTY", "EXAMPLE-CSA"),
        netting_source_id: "example-netting-record".into(),
        position_ids: vec!["EQUITY_001".into()],
        cash_vm: None,
        factors: vec![ExposureFactor::Equity {
            state: 0,
            price_id: "STOCK-SPOT".into(),
        }],
        dates,
        simulation: PathSimulationSpec {
            process: ProcessSpec::Gbm(GbmParams::new(0.05, 0.0, 0.2)?),
            scheme: SchemeSpec::Default,
            initial_state: vec![100.0],
            time_grid: TimeGridSpec::Times { times },
            num_paths: 1000,
            seed: 42,
            antithetic: true,
            fbm: None,
        },
    })
}

fn classification() -> CsrQuoteClassification {
    CsrQuoteClassification {
        issuer: "EXAMPLE-CORP".into(),
        currency: Currency::USD,
        seniority: Seniority::Senior,
        // Illustrative investment-grade industrial issuer.
        bucket: 4,
        source_id: "example-approved-classification".into(),
        as_of: AS_OF,
    }
}

pub(super) fn csr_quote_classification() -> Result<Vec<Value>> {
    example(&classification())
}

fn csr_source() -> CsrQuoteSource {
    CsrQuoteSource {
        curve_id: "EXAMPLE-CORP-CDS".into(),
        classification: classification(),
        vertices: CSR_VERTICES
            .into_iter()
            .map(|(vertex, pillar_date)| {
                (
                    vertex.into(),
                    CalibrationQuoteCoordinate {
                        quote_id: format!("EXAMPLE-CORP-CDS-{vertex}"),
                        pillar_date,
                    },
                )
            })
            .collect(),
    }
}

pub(super) fn csr_quote_source() -> Result<Vec<Value>> {
    example(&csr_source())
}

fn girr_target(vertex: &str, pillar_date: Date) -> GirrQuoteTarget {
    GirrQuoteTarget {
        risk_currency: Currency::USD,
        risk_curve_id: "USD-OIS".into(),
        vertex: vertex.into(),
        source: GirrQuoteSource::Discount {
            curve_id: "USD-OIS".into(),
        },
        quote: CalibrationQuoteCoordinate {
            quote_id: format!("USD-OIS-{vertex}"),
            pillar_date,
        },
    }
}

pub(super) fn girr_quote_target() -> Result<Vec<Value>> {
    example(&girr_target("5Y", date!(2029 - 01 - 02)))
}

pub(super) fn portfolio_frtb_config() -> Result<Vec<Value>> {
    example(&PortfolioFrtbConfig {
        girr_targets: GIRR_VERTICES
            .into_iter()
            .map(|(vertex, date)| girr_target(vertex, date))
            .collect(),
        csr_sources: vec![csr_source()],
    })
}

/// Synthetic USD-only position evidence with an exact signed 1 bp difference.
fn reprice(position: &str, center: i64, difference: i64) -> RegulatoryQuoteReprice {
    RegulatoryQuoteReprice {
        position_id: position.into(),
        entity_id: "TRADING".into(),
        position_scale: 1.0,
        fx_rate: None,
        fx_triangulated: None,
        center_native: Money::from((center, Currency::USD)),
        shocked_native: Money::from((center + difference, Currency::USD)),
        pnl_native_per_bp: Money::from((difference, Currency::USD)),
        pnl_base_per_pp: difference as f64 * 100.0,
    }
}

fn girr_delta() -> Result<GirrQuoteDelta> {
    let mut sensitivities = FrtbSensitivities::new(Currency::USD);
    let mut quote_risk = Vec::new();
    for (vertex, date) in GIRR_VERTICES {
        let positions = vec![
            reprice("BOND_001", 1_000_000, -100),
            reprice("CDS_001", 10_000, -1),
        ];
        for row in &positions {
            sensitivities.add_girr_delta(Currency::USD, "USD-OIS", vertex, row.pnl_base_per_pp)?;
        }
        quote_risk.push(GirrQuoteRisk {
            target: girr_target(vertex, date),
            positions,
        });
    }
    Ok(GirrQuoteDelta {
        as_of: AS_OF,
        portfolio_id: "example-trading-book".into(),
        market_hash: "example-market-fingerprint".into(),
        pricing_hash: "example-pricing-fingerprint".into(),
        sensitivities,
        quote_risk,
    })
}

pub(super) fn girr_quote_delta() -> Result<Vec<Value>> {
    example(&girr_delta()?)
}

fn csr_delta() -> CsrQuoteDelta {
    let source = csr_source();
    let mut sensitivities = FrtbSensitivities::new(Currency::USD);
    let mut quote_risk = Vec::new();
    for (vertex, _) in CSR_VERTICES {
        let row = reprice("CDS_001", 10_000, 50);
        sensitivities.add_csr_nonsec_delta(
            &source.classification.issuer,
            source.classification.bucket,
            vertex,
            source.curve_id.as_str(),
            row.pnl_base_per_pp,
        );
        quote_risk.push(CsrQuoteRisk {
            curve_id: source.curve_id.clone(),
            vertex: vertex.into(),
            positions: vec![row],
        });
    }
    CsrQuoteDelta {
        as_of: AS_OF,
        portfolio_id: "example-trading-book".into(),
        market_hash: "example-market-fingerprint".into(),
        pricing_hash: "example-pricing-fingerprint".into(),
        sources: vec![source],
        quote_risk,
        sensitivities,
    }
}

pub(super) fn csr_quote_delta() -> Result<Vec<Value>> {
    example(&csr_delta())
}

pub(super) fn portfolio_frtb_sensitivities_result() -> Result<Vec<Value>> {
    let girr = girr_delta()?;
    let csr = csr_delta();
    let mut sensitivities = girr.sensitivities.clone();
    sensitivities.csr_nonsec_delta = csr.sensitivities.csr_nonsec_delta.clone();
    example(&PortfolioFrtbSensitivitiesResult {
        sensitivities,
        girr,
        csr,
        positions_mapped: 2,
        positions_unmapped: Vec::new(),
    })
}
