//! FRTB Sensitivity-Based Approach types and data structures.
//!
//! Defines the risk class taxonomy, correlation scenarios, sensitivity
//! containers, and result types per BCBS d457.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::HashMap;

// ---------------------------------------------------------------------------
// Risk class enum
// ---------------------------------------------------------------------------

/// FRTB risk classes per BCBS d457.
///
/// These differ from SIMM risk classes: GIRR replaces IR, CSR is split
/// into three sub-types for securitization treatment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum FrtbRiskClass {
    /// General Interest Rate Risk
    Girr,
    /// Credit Spread Risk -- non-securitization
    CsrNonSec,
    /// Credit Spread Risk -- securitization (Correlation Trading Portfolio)
    CsrSecCtp,
    /// Credit Spread Risk -- securitization (non-CTP)
    CsrSecNonCtp,
    /// Equity risk
    Equity,
    /// Commodity risk
    Commodity,
    /// Foreign exchange risk
    Fx,
}

impl FrtbRiskClass {
    /// All risk classes in canonical order.
    pub const ALL: &'static [FrtbRiskClass] = &[
        FrtbRiskClass::Girr,
        FrtbRiskClass::CsrNonSec,
        FrtbRiskClass::CsrSecCtp,
        FrtbRiskClass::CsrSecNonCtp,
        FrtbRiskClass::Equity,
        FrtbRiskClass::Commodity,
        FrtbRiskClass::Fx,
    ];
}

impl std::fmt::Display for FrtbRiskClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Girr => write!(f, "GIRR"),
            Self::CsrNonSec => write!(f, "CSR Non-Sec"),
            Self::CsrSecCtp => write!(f, "CSR Sec CTP"),
            Self::CsrSecNonCtp => write!(f, "CSR Sec Non-CTP"),
            Self::Equity => write!(f, "Equity"),
            Self::Commodity => write!(f, "Commodity"),
            Self::Fx => write!(f, "FX"),
        }
    }
}

// ---------------------------------------------------------------------------
// Correlation scenario
// ---------------------------------------------------------------------------

/// FRTB correlation scenario for capital charge aggregation.
///
/// The final SBA capital charge is max(low, medium, high).
/// Low and high scenarios scale prescribed correlations per BCBS d457,
/// floored/capped by the scenario-specific Basel formulas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CorrelationScenario {
    /// `rho_low = max(2 * rho_medium - 1, 0.75 * rho_medium)`
    Low,
    /// Prescribed correlation (base case).
    Medium,
    /// `rho_high = min(1.25 * rho_medium, cap)`
    High,
}

impl CorrelationScenario {
    /// All scenarios in canonical order.
    pub const ALL: &'static [CorrelationScenario] = &[
        CorrelationScenario::Low,
        CorrelationScenario::Medium,
        CorrelationScenario::High,
    ];

    /// Scale a base (medium) correlation for this scenario.
    ///
    /// Low: `max(2 * rho - 1, 0.75 * rho)`
    /// Medium: `rho` (unchanged)
    /// High: `min(1.25 * rho, 1)`
    #[must_use]
    pub fn scale_correlation(self, rho: f64) -> f64 {
        match self {
            Self::Low => f64::max(2.0 * rho - 1.0, 0.75 * rho),
            Self::Medium => rho,
            Self::High => f64::min(1.25 * rho, 1.0),
        }
    }
}

impl std::fmt::Display for CorrelationScenario {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "Low"),
            Self::Medium => write!(f, "Medium"),
            Self::High => write!(f, "High"),
        }
    }
}

#[cfg(test)]
mod correlation_scenario_tests {
    use super::CorrelationScenario;

    #[test]
    fn low_correlation_uses_basel_floor() {
        assert_eq!(CorrelationScenario::Low.scale_correlation(0.5), 0.375);
        assert_eq!(CorrelationScenario::Low.scale_correlation(0.0), 0.0);
        assert!((CorrelationScenario::Low.scale_correlation(0.8) - 0.6).abs() < 1e-12);
    }
}

// ---------------------------------------------------------------------------
// DRC types
// ---------------------------------------------------------------------------

/// DRC sector classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum DrcSector {
    /// Sovereign entities.
    Sovereign,
    /// Financial and corporate issuers.
    FinancialsCorporate,
    /// Materials and energy sector.
    MaterialsEnergy,
    /// Consumer goods sector.
    ConsumerGoods,
    /// Technology and media sector.
    TechnologyMedia,
    /// Healthcare and utilities sector.
    HealthCareUtilities,
}

/// DRC seniority for LGD assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum DrcSeniority {
    /// Senior unsecured debt.
    SeniorUnsecured,
    /// Subordinated debt.
    Subordinated,
    /// Equity instruments.
    Equity,
    /// Securitization tranches.
    Securitization,
}

/// DRC asset type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum DrcAssetType {
    /// Corporate bonds and loans.
    Corporate,
    /// Sovereign bonds.
    Sovereign,
    /// Securitization tranches.
    Securitization,
    /// Equity instruments.
    Equity,
}

/// A position subject to the Default Risk Charge.
///
/// Per MAR22.9, gross JTD for a position is:
///
/// ```text
/// long:  gross = max(LGD * notional + P&L, 0)
/// short: gross = min(LGD * notional + P&L, 0)
/// ```
///
/// where the `P&L` term captures any mark-to-market adjustment already
/// reflected in the trading-book valuation (e.g. an underwater long bond
/// has a small negative `P&L` that reduces the exposed JTD). `jtd_amount`
/// represents the signed *notional* (positive = long, negative = short);
/// [`drc_charge`](super::drc::drc_charge) multiplies by [`DrcSeniority`]
/// LGD and then applies the `pnl_adjustment` and the sign-preserving floor.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DrcPosition {
    /// Issuer identifier.
    pub issuer: String,
    /// Signed JTD *notional* (positive = long, negative = short). Does
    /// **not** include the LGD multiplier — [`super::drc::drc_charge`] applies LGD.
    pub jtd_amount: f64,
    /// Credit rating bucket (1-based per FRTB specification).
    pub rating_bucket: u8,
    /// Sector for DRC bucket assignment.
    pub sector: DrcSector,
    /// Seniority for LGD determination.
    pub seniority: DrcSeniority,
    /// Asset sub-type: corporate bond, equity, or securitization.
    pub asset_type: DrcAssetType,
    /// Mark-to-market / P&L adjustment from MAR22.9. Default 0. Add a
    /// negative value for a long position with unrealised loss so the
    /// gross JTD is correctly floored at zero when the mark-down already
    /// exceeds `LGD * notional`. Ignored for securitisations where Basel
    /// treats JTD differently.
    #[serde(default)]
    pub pnl_adjustment: f64,
}

// ---------------------------------------------------------------------------
// RRAO types
// ---------------------------------------------------------------------------

/// A position subject to the Residual Risk Add-On.
///
/// RRAO applies to exotic instruments whose risks are not adequately
/// captured by the delta/vega/curvature framework -- instruments with
/// gap risk, correlation risk, or behavioral risk.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RraoPosition {
    /// Instrument identifier.
    pub instrument_id: String,
    /// Gross notional amount.
    pub notional: f64,
    /// Whether the instrument bears exotic underlying risk (1.0% weight)
    /// or other residual risk (0.1% weight).
    pub is_exotic: bool,
}

// ---------------------------------------------------------------------------
// Sensitivity inputs
// ---------------------------------------------------------------------------

/// FRTB sensitivity inputs organized by risk class.
///
/// Compared to `SimmSensitivities`, this struct adds:
/// - GIRR inflation and cross-currency basis risk factors
/// - CSR securitization sub-type separation
/// - Curvature shock direction (up/down) per risk factor
/// - Bucket assignment metadata required for FRTB aggregation
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FrtbSensitivities {
    /// Base/reporting currency.
    pub base_currency: Currency,

    // -- GIRR --
    /// GIRR delta by (currency, tenor).
    ///
    /// Units: base-currency P&L per **1 percentage-point** (1pp) parallel
    /// shift of the yield curve — i.e. `100 × DV01`. This matches the
    /// FRTB risk-weight convention (`GIRR_DELTA_RISK_WEIGHTS` are stated
    /// in percent, e.g. `1.7` for 1.7%). See the module-level docs on
    /// sensitivity conventions.
    pub girr_delta: HashMap<(Currency, String), f64>,
    /// GIRR inflation delta by currency.
    ///
    /// Units: base-currency P&L per 1pp shift in inflation (same
    /// convention as [`Self::girr_delta`]).
    pub girr_inflation_delta: HashMap<Currency, f64>,
    /// GIRR cross-currency basis delta by currency.
    ///
    /// Units: base-currency P&L per 1pp shift in the cross-currency
    /// basis (same convention as [`Self::girr_delta`]).
    pub girr_xccy_basis_delta: HashMap<Currency, f64>,
    /// GIRR vega by (currency, option_maturity, underlying_tenor).
    pub girr_vega: HashMap<(Currency, String, String), f64>,
    /// GIRR curvature: (currency) -> (cvr_up, cvr_down).
    pub girr_curvature: HashMap<Currency, (f64, f64)>,

    // -- CSR Non-Securitization --
    /// CSR non-sec delta by (issuer, bucket, tenor).
    pub csr_nonsec_delta: HashMap<(String, u8, String), f64>,
    /// CSR non-sec vega by (issuer, bucket, option_maturity).
    pub csr_nonsec_vega: HashMap<(String, u8, String), f64>,
    /// CSR non-sec curvature by (issuer, bucket) -> (cvr_up, cvr_down).
    pub csr_nonsec_curvature: HashMap<(String, u8), (f64, f64)>,

    // -- CSR Securitization CTP --
    /// CSR sec-CTP delta by (tranche, bucket, tenor).
    pub csr_sec_ctp_delta: HashMap<(String, u8, String), f64>,
    /// CSR sec-CTP vega by (tranche, bucket, option_maturity).
    pub csr_sec_ctp_vega: HashMap<(String, u8, String), f64>,
    /// CSR sec-CTP curvature by (tranche, bucket) -> (cvr_up, cvr_down).
    pub csr_sec_ctp_curvature: HashMap<(String, u8), (f64, f64)>,

    // -- CSR Securitization Non-CTP --
    /// CSR sec-non-CTP delta by (tranche, bucket, tenor).
    pub csr_sec_nonctp_delta: HashMap<(String, u8, String), f64>,
    /// CSR sec-non-CTP vega by (tranche, bucket, option_maturity).
    pub csr_sec_nonctp_vega: HashMap<(String, u8, String), f64>,
    /// CSR sec-non-CTP curvature by (tranche, bucket) -> (cvr_up, cvr_down).
    pub csr_sec_nonctp_curvature: HashMap<(String, u8), (f64, f64)>,

    // -- Equity --
    /// Equity delta by (underlier, bucket).
    pub equity_delta: HashMap<(String, u8), f64>,
    /// Equity vega by (underlier, bucket, option_maturity).
    pub equity_vega: HashMap<(String, u8, String), f64>,
    /// Equity curvature by (underlier, bucket) -> (cvr_up, cvr_down).
    pub equity_curvature: HashMap<(String, u8), (f64, f64)>,

    // -- Commodity --
    /// Commodity delta by (commodity_name, bucket, tenor).
    pub commodity_delta: HashMap<(String, u8, String), f64>,
    /// Commodity vega by (commodity_name, bucket, option_maturity).
    pub commodity_vega: HashMap<(String, u8, String), f64>,
    /// Commodity curvature by (commodity_name, bucket) -> (cvr_up, cvr_down).
    pub commodity_curvature: HashMap<(String, u8), (f64, f64)>,

    // -- FX --
    /// FX delta by currency pair.
    pub fx_delta: HashMap<(Currency, Currency), f64>,
    /// FX vega by (currency_pair, option_maturity).
    pub fx_vega: HashMap<(Currency, Currency, String), f64>,
    /// FX curvature by currency pair -> (cvr_up, cvr_down).
    pub fx_curvature: HashMap<(Currency, Currency), (f64, f64)>,

    // -- DRC --
    /// Default risk positions by (issuer, rating, sector, seniority).
    pub drc_positions: Vec<DrcPosition>,

    // -- RRAO --
    /// Notional amounts for exotic instruments subject to RRAO.
    pub rrao_exotic_notionals: Vec<RraoPosition>,
}

impl FrtbSensitivities {
    /// Create a new empty sensitivity container.
    #[must_use]
    pub fn new(base_currency: Currency) -> Self {
        Self {
            base_currency,
            girr_delta: HashMap::default(),
            girr_inflation_delta: HashMap::default(),
            girr_xccy_basis_delta: HashMap::default(),
            girr_vega: HashMap::default(),
            girr_curvature: HashMap::default(),
            csr_nonsec_delta: HashMap::default(),
            csr_nonsec_vega: HashMap::default(),
            csr_nonsec_curvature: HashMap::default(),
            csr_sec_ctp_delta: HashMap::default(),
            csr_sec_ctp_vega: HashMap::default(),
            csr_sec_ctp_curvature: HashMap::default(),
            csr_sec_nonctp_delta: HashMap::default(),
            csr_sec_nonctp_vega: HashMap::default(),
            csr_sec_nonctp_curvature: HashMap::default(),
            equity_delta: HashMap::default(),
            equity_vega: HashMap::default(),
            equity_curvature: HashMap::default(),
            commodity_delta: HashMap::default(),
            commodity_vega: HashMap::default(),
            commodity_curvature: HashMap::default(),
            fx_delta: HashMap::default(),
            fx_vega: HashMap::default(),
            fx_curvature: HashMap::default(),
            drc_positions: Vec::new(),
            rrao_exotic_notionals: Vec::new(),
        }
    }

    /// Validate regulatory labels, buckets, identifiers, and numeric inputs.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        fn finite(field: &str, value: f64) -> finstack_quant_core::Result<()> {
            if value.is_finite() {
                Ok(())
            } else {
                Err(finstack_quant_core::Error::Validation(format!(
                    "FRTB sensitivity {field} must be finite, got {value}"
                )))
            }
        }
        fn identifier(field: &str, value: &str) -> finstack_quant_core::Result<()> {
            if value.trim().is_empty() {
                Err(finstack_quant_core::Error::Validation(format!(
                    "FRTB sensitivity {field} must not be empty"
                )))
            } else {
                Ok(())
            }
        }
        fn tenor(field: &str, value: &str) -> finstack_quant_core::Result<()> {
            if super::params::girr::tenor_to_years(value).is_some() {
                Ok(())
            } else {
                Err(finstack_quant_core::Error::Validation(format!(
                    "FRTB sensitivity {field} has unknown tenor '{value}'"
                )))
            }
        }
        fn bucket(
            field: &str,
            value: u8,
            allowed: &[(u8, f64)],
        ) -> finstack_quant_core::Result<()> {
            if allowed.iter().any(|(candidate, _)| *candidate == value) {
                Ok(())
            } else {
                Err(finstack_quant_core::Error::Validation(format!(
                    "FRTB sensitivity {field} has unknown bucket {value}"
                )))
            }
        }

        for ((_, label), value) in &self.girr_delta {
            tenor("girr_delta", label)?;
            finite("girr_delta", *value)?;
        }
        for value in self
            .girr_inflation_delta
            .values()
            .chain(self.girr_xccy_basis_delta.values())
            .chain(self.fx_delta.values())
        {
            finite("delta", *value)?;
        }
        for ((_, option_maturity, underlying_tenor), value) in &self.girr_vega {
            tenor("girr_vega.option_maturity", option_maturity)?;
            tenor("girr_vega.underlying_tenor", underlying_tenor)?;
            finite("girr_vega", *value)?;
        }
        for &(up, down) in self.girr_curvature.values() {
            finite("girr_curvature.up", up)?;
            finite("girr_curvature.down", down)?;
        }

        for ((issuer, bucket_id, label), value) in &self.csr_nonsec_delta {
            identifier("csr_nonsec_delta.issuer", issuer)?;
            bucket(
                "csr_nonsec_delta",
                *bucket_id,
                super::params::csr::CSR_NONSEC_RISK_WEIGHTS,
            )?;
            tenor("csr_nonsec_delta.tenor", label)?;
            finite("csr_nonsec_delta", *value)?;
        }
        for ((name, bucket_id, label), value) in &self.csr_sec_ctp_delta {
            identifier("csr_sec_ctp_delta.name", name)?;
            bucket(
                "csr_sec_ctp_delta",
                *bucket_id,
                super::params::csr::CSR_SEC_CTP_RISK_WEIGHTS,
            )?;
            tenor("csr_sec_ctp_delta.tenor", label)?;
            finite("csr_sec_ctp_delta", *value)?;
        }
        for ((name, bucket_id, label), value) in &self.csr_sec_nonctp_delta {
            identifier("csr_sec_nonctp_delta.name", name)?;
            bucket(
                "csr_sec_nonctp_delta",
                *bucket_id,
                super::params::csr::CSR_SEC_NONCTP_RISK_WEIGHTS,
            )?;
            tenor("csr_sec_nonctp_delta.tenor", label)?;
            finite("csr_sec_nonctp_delta", *value)?;
        }

        for ((issuer, bucket_id, maturity), value) in &self.csr_nonsec_vega {
            identifier("csr_nonsec_vega.issuer", issuer)?;
            bucket(
                "csr_nonsec_vega",
                *bucket_id,
                super::params::csr::CSR_NONSEC_RISK_WEIGHTS,
            )?;
            tenor("csr_nonsec_vega.maturity", maturity)?;
            finite("csr_nonsec_vega", *value)?;
        }
        for ((name, bucket_id, maturity), value) in &self.csr_sec_ctp_vega {
            identifier("csr_sec_ctp_vega.name", name)?;
            bucket(
                "csr_sec_ctp_vega",
                *bucket_id,
                super::params::csr::CSR_SEC_CTP_RISK_WEIGHTS,
            )?;
            tenor("csr_sec_ctp_vega.maturity", maturity)?;
            finite("csr_sec_ctp_vega", *value)?;
        }
        for ((name, bucket_id, maturity), value) in &self.csr_sec_nonctp_vega {
            identifier("csr_sec_nonctp_vega.name", name)?;
            bucket(
                "csr_sec_nonctp_vega",
                *bucket_id,
                super::params::csr::CSR_SEC_NONCTP_RISK_WEIGHTS,
            )?;
            tenor("csr_sec_nonctp_vega.maturity", maturity)?;
            finite("csr_sec_nonctp_vega", *value)?;
        }

        for ((underlier, bucket_id), value) in &self.equity_delta {
            identifier("equity_delta.underlier", underlier)?;
            bucket(
                "equity_delta",
                *bucket_id,
                super::params::equity::EQUITY_RISK_WEIGHTS,
            )?;
            finite("equity_delta", *value)?;
        }
        for ((underlier, bucket_id, maturity), value) in &self.equity_vega {
            identifier("equity_vega.underlier", underlier)?;
            bucket(
                "equity_vega",
                *bucket_id,
                super::params::equity::EQUITY_RISK_WEIGHTS,
            )?;
            tenor("equity_vega.maturity", maturity)?;
            finite("equity_vega", *value)?;
        }
        for ((underlier, bucket_id), &(up, down)) in &self.equity_curvature {
            identifier("equity_curvature.underlier", underlier)?;
            bucket(
                "equity_curvature",
                *bucket_id,
                super::params::equity::EQUITY_RISK_WEIGHTS,
            )?;
            finite("equity_curvature.up", up)?;
            finite("equity_curvature.down", down)?;
        }

        for ((name, bucket_id, label), value) in &self.commodity_delta {
            identifier("commodity_delta.name", name)?;
            bucket(
                "commodity_delta",
                *bucket_id,
                super::params::commodity::COMMODITY_RISK_WEIGHTS,
            )?;
            tenor("commodity_delta.tenor", label)?;
            finite("commodity_delta", *value)?;
        }
        for ((name, bucket_id, maturity), value) in &self.commodity_vega {
            identifier("commodity_vega.name", name)?;
            bucket(
                "commodity_vega",
                *bucket_id,
                super::params::commodity::COMMODITY_RISK_WEIGHTS,
            )?;
            tenor("commodity_vega.maturity", maturity)?;
            finite("commodity_vega", *value)?;
        }
        for ((name, bucket_id), &(up, down)) in &self.commodity_curvature {
            identifier("commodity_curvature.name", name)?;
            bucket(
                "commodity_curvature",
                *bucket_id,
                super::params::commodity::COMMODITY_RISK_WEIGHTS,
            )?;
            finite("commodity_curvature.up", up)?;
            finite("commodity_curvature.down", down)?;
        }

        for ((_, _, maturity), value) in &self.fx_vega {
            tenor("fx_vega.maturity", maturity)?;
            finite("fx_vega", *value)?;
        }
        for &(up, down) in self.fx_curvature.values() {
            finite("fx_curvature.up", up)?;
            finite("fx_curvature.down", down)?;
        }

        for (entries, allowed, field) in [
            (
                &self.csr_nonsec_curvature,
                super::params::csr::CSR_NONSEC_RISK_WEIGHTS,
                "csr_nonsec_curvature",
            ),
            (
                &self.csr_sec_ctp_curvature,
                super::params::csr::CSR_SEC_CTP_RISK_WEIGHTS,
                "csr_sec_ctp_curvature",
            ),
            (
                &self.csr_sec_nonctp_curvature,
                super::params::csr::CSR_SEC_NONCTP_RISK_WEIGHTS,
                "csr_sec_nonctp_curvature",
            ),
        ] {
            for ((issuer, bucket_id), &(up, down)) in entries {
                identifier("csr_curvature.name", issuer)?;
                bucket(field, *bucket_id, allowed)?;
                finite("csr_curvature.up", up)?;
                finite("csr_curvature.down", down)?;
            }
        }
        for position in &self.drc_positions {
            identifier("drc.issuer", &position.issuer)?;
            if !(1..=9).contains(&position.rating_bucket) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "FRTB sensitivity drc has unknown rating bucket {}",
                    position.rating_bucket
                )));
            }
            finite("drc.jtd_amount", position.jtd_amount)?;
            finite("drc.pnl_adjustment", position.pnl_adjustment)?;
        }
        for position in &self.rrao_exotic_notionals {
            identifier("rrao.instrument_id", &position.instrument_id)?;
            finite("rrao.notional", position.notional)?;
        }
        Ok(())
    }

    // -- Builder-style adders --

    /// Add a GIRR delta sensitivity.
    pub fn add_girr_delta(&mut self, ccy: Currency, tenor: &str, delta: f64) {
        *self
            .girr_delta
            .entry((ccy, tenor.to_string()))
            .or_insert(0.0) += delta;
    }

    /// Add a CSR non-sec delta sensitivity.
    pub fn add_csr_nonsec_delta(&mut self, issuer: &str, bucket: u8, tenor: &str, delta: f64) {
        *self
            .csr_nonsec_delta
            .entry((issuer.to_string(), bucket, tenor.to_string()))
            .or_insert(0.0) += delta;
    }

    /// Add an equity delta sensitivity.
    pub fn add_equity_delta(&mut self, underlier: &str, bucket: u8, delta: f64) {
        *self
            .equity_delta
            .entry((underlier.to_string(), bucket))
            .or_insert(0.0) += delta;
    }

    /// Add an FX delta sensitivity.
    pub fn add_fx_delta(&mut self, ccy1: Currency, ccy2: Currency, delta: f64) {
        *self.fx_delta.entry((ccy1, ccy2)).or_insert(0.0) += delta;
    }

    /// Add a commodity delta sensitivity.
    pub fn add_commodity_delta(&mut self, name: &str, bucket: u8, tenor: &str, delta: f64) {
        *self
            .commodity_delta
            .entry((name.to_string(), bucket, tenor.to_string()))
            .or_insert(0.0) += delta;
    }

    /// Add a GIRR vega sensitivity.
    pub fn add_girr_vega(
        &mut self,
        ccy: Currency,
        option_maturity: &str,
        underlying_tenor: &str,
        vega: f64,
    ) {
        *self
            .girr_vega
            .entry((
                ccy,
                option_maturity.to_string(),
                underlying_tenor.to_string(),
            ))
            .or_insert(0.0) += vega;
    }

    /// Add an equity vega sensitivity.
    pub fn add_equity_vega(&mut self, underlier: &str, bucket: u8, maturity: &str, vega: f64) {
        *self
            .equity_vega
            .entry((underlier.to_string(), bucket, maturity.to_string()))
            .or_insert(0.0) += vega;
    }

    /// Add an FX vega sensitivity.
    pub fn add_fx_vega(&mut self, ccy1: Currency, ccy2: Currency, maturity: &str, vega: f64) {
        *self
            .fx_vega
            .entry((ccy1, ccy2, maturity.to_string()))
            .or_insert(0.0) += vega;
    }

    /// Add a GIRR curvature sensitivity.
    pub fn add_girr_curvature(&mut self, ccy: Currency, cvr_up: f64, cvr_down: f64) {
        let entry = self.girr_curvature.entry(ccy).or_insert((0.0, 0.0));
        entry.0 += cvr_up;
        entry.1 += cvr_down;
    }

    /// Add an equity curvature sensitivity.
    pub fn add_equity_curvature(
        &mut self,
        underlier: &str,
        bucket: u8,
        cvr_up: f64,
        cvr_down: f64,
    ) {
        let entry = self
            .equity_curvature
            .entry((underlier.to_string(), bucket))
            .or_insert((0.0, 0.0));
        entry.0 += cvr_up;
        entry.1 += cvr_down;
    }

    /// Add an FX curvature sensitivity.
    pub fn add_fx_curvature(&mut self, ccy1: Currency, ccy2: Currency, cvr_up: f64, cvr_down: f64) {
        let entry = self.fx_curvature.entry((ccy1, ccy2)).or_insert((0.0, 0.0));
        entry.0 += cvr_up;
        entry.1 += cvr_down;
    }
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

/// Complete FRTB SBA capital charge result.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FrtbSbaResult {
    /// Total capital charge (sum of all components).
    pub total: f64,
    /// Delta risk charge by risk class.
    pub delta_by_risk_class: HashMap<FrtbRiskClass, f64>,
    /// Vega risk charge by risk class.
    pub vega_by_risk_class: HashMap<FrtbRiskClass, f64>,
    /// Curvature risk charge by risk class.
    pub curvature_by_risk_class: HashMap<FrtbRiskClass, f64>,
    /// Default Risk Charge (credit + equity).
    pub drc: f64,
    /// Residual Risk Add-On.
    pub rrao: f64,
    /// Which correlation scenario produced the binding charge for each component.
    pub binding_scenario: CorrelationScenario,
    /// Delta+Vega+Curvature charge under each scenario (for transparency).
    pub scenario_charges: HashMap<CorrelationScenario, f64>,
}
