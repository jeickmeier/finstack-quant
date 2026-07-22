//! Agency CMO types.
//!
//! Collateralized Mortgage Obligations (CMOs) are structured products that
//! redistribute the cashflows from underlying MBS pools into tranches with
//! different risk/return profiles.

use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::fixed_income::mbs_passthrough::{AgencyMbsPassthrough, AgencyProgram};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, DealId, InstrumentId};

/// CMO tranche type enumeration.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CmoTrancheType {
    /// Sequential pay - receives principal in order
    Sequential,
    /// PAC (Planned Amortization Class) - protected by support
    Pac,
    /// Support/Companion - absorbs prepayment variability
    Support,
    /// Interest-Only strip
    InterestOnly,
    /// Principal-Only strip
    PrincipalOnly,
    /// Accrual (Z) bond - capitalizes interest while senior tranches
    /// are outstanding; the accrual is redirected as accretion-directed
    /// principal to the current-pay tranches
    Accrual,
}

impl std::fmt::Display for CmoTrancheType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CmoTrancheType::Sequential => write!(f, "SEQ"),
            CmoTrancheType::Pac => write!(f, "PAC"),
            CmoTrancheType::Support => write!(f, "SUP"),
            CmoTrancheType::InterestOnly => write!(f, "IO"),
            CmoTrancheType::PrincipalOnly => write!(f, "PO"),
            CmoTrancheType::Accrual => write!(f, "Z"),
        }
    }
}

/// PAC collar boundaries.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct PacCollar {
    /// Lower PSA bound
    pub lower_psa: f64,
    /// Upper PSA bound
    pub upper_psa: f64,
}

impl PacCollar {
    /// Create a standard PAC collar.
    pub fn new(lower_psa: f64, upper_psa: f64) -> Self {
        Self {
            lower_psa,
            upper_psa,
        }
    }

    /// Standard 100-300 PSA collar.
    pub fn standard() -> Self {
        Self::new(1.0, 3.0)
    }
}

/// CMO tranche definition.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CmoTranche {
    /// Tranche identifier (e.g., "A", "B", "IO")
    pub id: String,
    /// Tranche type
    pub tranche_type: CmoTrancheType,
    /// Original face amount
    pub original_face: Money,
    /// Current face amount
    pub current_face: Money,
    /// Coupon rate (0.0 for PO)
    pub coupon: f64,
    /// Payment priority (1 = highest for sequential)
    pub priority: u32,
    /// PAC collar (if PAC tranche)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pac_collar: Option<PacCollar>,
}

impl CmoTranche {
    /// Create a sequential tranche.
    pub fn sequential(id: &str, face: Money, coupon: f64, priority: u32) -> Self {
        Self {
            id: id.to_string(),
            tranche_type: CmoTrancheType::Sequential,
            original_face: face,
            current_face: face,
            coupon,
            priority,
            pac_collar: None,
        }
    }

    /// Create a PAC tranche.
    pub fn pac(id: &str, face: Money, coupon: f64, priority: u32, collar: PacCollar) -> Self {
        Self {
            id: id.to_string(),
            tranche_type: CmoTrancheType::Pac,
            original_face: face,
            current_face: face,
            coupon,
            priority,
            pac_collar: Some(collar),
        }
    }

    /// Create a support tranche.
    pub fn support(id: &str, face: Money, coupon: f64, priority: u32) -> Self {
        Self {
            id: id.to_string(),
            tranche_type: CmoTrancheType::Support,
            original_face: face,
            current_face: face,
            coupon,
            priority,
            pac_collar: None,
        }
    }

    /// Create an accrual (Z) tranche.
    ///
    /// While any other principal-receiving tranche is outstanding, the Z
    /// receives no cash: its coupon accrual is capitalized into its balance
    /// and an equal amount of interest collections is redirected as
    /// accretion-directed principal to the current-pay tranches. Once all
    /// current-pay tranches retire, the Z pays cash interest on its accreted
    /// balance plus principal until retired.
    ///
    /// See Fabozzi, *The Handbook of Mortgage-Backed Securities* (7th ed.),
    /// Ch. 21 "Accrual Bonds" (Z bonds in sequential-pay CMO structures).
    pub fn accrual(id: &str, face: Money, coupon: f64, priority: u32) -> Self {
        Self {
            id: id.to_string(),
            tranche_type: CmoTrancheType::Accrual,
            original_face: face,
            current_face: face,
            coupon,
            priority,
            pac_collar: None,
        }
    }

    /// Create an IO strip.
    pub fn io_strip(id: &str, notional: Money, coupon: f64) -> Self {
        Self {
            id: id.to_string(),
            tranche_type: CmoTrancheType::InterestOnly,
            original_face: notional,
            current_face: notional,
            coupon,
            priority: 0, // IO gets interest before principal allocation
            pac_collar: None,
        }
    }

    /// Create a PO strip.
    pub fn po_strip(id: &str, face: Money) -> Self {
        Self {
            id: id.to_string(),
            tranche_type: CmoTrancheType::PrincipalOnly,
            original_face: face,
            current_face: face,
            coupon: 0.0,
            priority: 0,
            pac_collar: None,
        }
    }

    /// Get current factor.
    ///
    /// Returns `0.0` for a zero-original-face tranche (fully retired or
    /// degenerate placeholder) rather than dividing by zero.
    pub fn factor(&self) -> f64 {
        let original = self.original_face.amount();
        if original == 0.0 {
            return 0.0;
        }
        self.current_face.amount() / original
    }

    /// Check if tranche is interest-bearing.
    ///
    /// An accrual (Z) tranche is interest-bearing: its coupon accrues every
    /// period, although during the accretion phase the accrual is capitalized
    /// into its balance rather than paid in cash.
    pub fn is_interest_bearing(&self) -> bool {
        self.coupon > 0.0 && self.tranche_type != CmoTrancheType::PrincipalOnly
    }

    /// Check if tranche receives principal.
    ///
    /// An accrual (Z) tranche receives principal, but only after every
    /// current-pay (non-accrual) principal tranche has been retired.
    pub fn receives_principal(&self) -> bool {
        self.tranche_type != CmoTrancheType::InterestOnly
    }
}

/// CMO waterfall configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CmoWaterfall {
    /// Tranches in the deal (ordered by priority for sequential)
    pub tranches: Vec<CmoTranche>,
    /// Whether to use pro-rata allocation within same priority
    pub pro_rata_same_priority: bool,
}

impl CmoWaterfall {
    /// Create a new waterfall with tranches.
    pub fn new(tranches: Vec<CmoTranche>) -> Self {
        Self {
            tranches,
            pro_rata_same_priority: false,
        }
    }

    /// Get tranche by ID.
    pub fn get_tranche(&self, id: &str) -> Option<&CmoTranche> {
        self.tranches.iter().find(|t| t.id == id)
    }

    /// Get mutable tranche by ID.
    pub fn get_tranche_mut(&mut self, id: &str) -> Option<&mut CmoTranche> {
        self.tranches.iter_mut().find(|t| t.id == id)
    }

    /// Get total current face across all tranches (excluding IO).
    pub fn total_current_face(&self) -> Money {
        let total: f64 = self
            .tranches
            .iter()
            .filter(|t| t.receives_principal())
            .map(|t| t.current_face.amount())
            .sum();

        let currency = self
            .tranches
            .first()
            .map(|t| t.current_face.currency())
            .unwrap_or(Currency::USD);

        Money::new(total, currency)
    }
}

/// Agency CMO instrument.
///
/// Represents a CMO deal backed by agency MBS collateral with multiple
/// tranches that receive cashflows according to waterfall rules.
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    finstack_quant_valuations_macros::FocusedPricingOverrides,
)]
#[serde(deny_unknown_fields)]
#[builder(validate = AgencyCmo::validate)]
pub struct AgencyCmo {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Deal name (e.g., "FNR 2024-1")
    pub deal_name: DealId,
    /// Agency program
    pub agency: AgencyProgram,
    /// Issue date
    #[schemars(with = "String")]
    pub issue_date: Date,
    /// Waterfall configuration with tranches
    pub waterfall: CmoWaterfall,
    /// Reference tranche ID for pricing (which tranche to value)
    pub reference_tranche_id: String,
    /// Collateral pool (optional - for detailed cashflow projection)
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collateral: Option<Box<AgencyMbsPassthrough>>,
    /// Collateral WAC (if no explicit collateral)
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collateral_wac: Option<f64>,
    /// Collateral WAM (if no explicit collateral)
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collateral_wam: Option<u32>,
    /// Discount curve identifier.
    pub discount_curve_id: CurveId,
    /// Pricing overrides.
    #[builder(default)]
    #[serde(default)]
    /// Instrument-owned pricing inputs.
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[serde(default)]
    #[builder(default)]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[serde(default)]
    #[builder(default)]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for tagging and selection.
    #[builder(default)]
    #[serde(default)]
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

impl AgencyCmo {
    /// Validate the tranche waterfall, collateral, and interest conservation.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        let context = format!("Agency CMO '{}'", self.id.as_str());
        if self.deal_name.as_str().trim().is_empty()
            || self.reference_tranche_id.trim().is_empty()
            || self.discount_curve_id.as_str().trim().is_empty()
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} requires non-empty deal, reference-tranche, and discount-curve identifiers"
            )));
        }
        if self.waterfall.tranches.is_empty() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} requires at least one tranche"
            )));
        }
        let currency = self.waterfall.tranches[0].original_face.currency();
        let mut ids = std::collections::HashSet::new();
        for tranche in &self.waterfall.tranches {
            if tranche.id.trim().is_empty() || !ids.insert(tranche.id.as_str()) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} tranche identifiers must be non-empty and unique"
                )));
            }
            let original = tranche.original_face.amount();
            let current = tranche.current_face.amount();
            if tranche.original_face.currency() != currency
                || tranche.current_face.currency() != currency
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} all tranche balances must use the same currency"
                )));
            }
            if !original.is_finite()
                || original <= 0.0
                || !current.is_finite()
                || current < 0.0
                || current > original
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} tranche '{}' requires 0 <= current_face <= positive original_face",
                    tranche.id
                )));
            }
            if !tranche.coupon.is_finite() || !(0.0..=1.0).contains(&tranche.coupon) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} tranche '{}' coupon must be a finite decimal rate in [0, 1]",
                    tranche.id
                )));
            }
            if matches!(
                tranche.tranche_type,
                CmoTrancheType::Sequential
                    | CmoTrancheType::Pac
                    | CmoTrancheType::Support
                    | CmoTrancheType::Accrual
            ) && tranche.priority == 0
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} waterfall tranche '{}' requires positive priority",
                    tranche.id
                )));
            }
            match (&tranche.tranche_type, &tranche.pac_collar) {
                (CmoTrancheType::Pac, Some(collar))
                    if collar.lower_psa.is_finite()
                        && collar.upper_psa.is_finite()
                        && collar.lower_psa >= 0.0
                        && collar.lower_psa <= collar.upper_psa => {}
                (CmoTrancheType::Pac, _) => {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "{context} PAC tranche '{}' requires a finite ordered non-negative collar",
                        tranche.id
                    )));
                }
                (_, Some(_)) => {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "{context} non-PAC tranche '{}' cannot define a PAC collar",
                        tranche.id
                    )));
                }
                _ => {}
            }
            if tranche.tranche_type == CmoTrancheType::PrincipalOnly && tranche.coupon != 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} principal-only tranche '{}' must have zero coupon",
                    tranche.id
                )));
            }
        }
        if self.reference_tranche().is_none() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} reference tranche '{}' is not present in the waterfall",
                self.reference_tranche_id
            )));
        }
        if self
            .collateral_wac
            .is_some_and(|wac| !wac.is_finite() || !(0.0..=1.0).contains(&wac))
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} collateral_wac must be a finite decimal rate in [0, 1]"
            )));
        }
        if self.collateral_wam == Some(0) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} collateral_wam must be positive"
            )));
        }
        if let Some(pool) = &self.collateral {
            crate::instruments::Instrument::validate_for_pricing(pool.as_ref())?;
            if pool.current_face.currency() != currency {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} collateral and tranche currencies must match"
                )));
            }
        }
        Self::validate_interest_coverage(self)
    }

    /// Create a canonical example CMO for testing.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use time::macros::date;
        // Create sequential structure: A (front), B (middle), C (last).
        // Every tranche coupon is at or below the 4.0% net pass-through
        // (4.5% WAC less 50bp of fees), so the structure stays
        // interest-covered for the life of the deal.
        let tranches = vec![
            CmoTranche::sequential("A", Money::new(40_000_000.0, Currency::USD), 0.035, 1),
            CmoTranche::sequential("B", Money::new(30_000_000.0, Currency::USD), 0.04, 2),
            CmoTranche::sequential("C", Money::new(30_000_000.0, Currency::USD), 0.04, 3),
        ];

        Self::builder()
            .id(InstrumentId::new("FNR-2024-1-A"))
            .deal_name("FNR 2024-1".into())
            .agency(AgencyProgram::Fnma)
            .issue_date(date!(2024 - 01 - 01))
            .waterfall(CmoWaterfall::new(tranches))
            .reference_tranche_id("A".to_string())
            .collateral_wac(0.045)
            .collateral_wam(360)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .attributes(
                Attributes::new()
                    .with_tag("cmo")
                    .with_tag("agency")
                    .with_meta("deal", "fnr-2024-1"),
            )
            .build()
    }

    /// Create an example sequential structure with an accrual (Z) tranche.
    ///
    /// A and B are current-pay sequentials; Z accretes at its coupon while
    /// they are outstanding, redirecting the accrual as accretion-directed
    /// principal that retires A and B faster than in `example()`.
    pub fn example_accrual() -> finstack_quant_core::Result<Self> {
        use time::macros::date;
        let tranches = vec![
            CmoTranche::sequential("A", Money::new(40_000_000.0, Currency::USD), 0.035, 1),
            CmoTranche::sequential("B", Money::new(30_000_000.0, Currency::USD), 0.04, 2),
            CmoTranche::accrual("Z", Money::new(30_000_000.0, Currency::USD), 0.04, 3),
        ];

        Self::builder()
            .id(InstrumentId::new("FNR-2024-3-Z"))
            .deal_name("FNR 2024-3".into())
            .agency(AgencyProgram::Fnma)
            .issue_date(date!(2024 - 01 - 01))
            .waterfall(CmoWaterfall::new(tranches))
            .reference_tranche_id("Z".to_string())
            .collateral_wac(0.045)
            .collateral_wam(360)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
    }

    /// Create an example PAC/Support structure.
    pub fn example_pac_support() -> finstack_quant_core::Result<Self> {
        use time::macros::date;
        let tranches = vec![
            CmoTranche::pac(
                "PAC",
                Money::new(50_000_000.0, Currency::USD),
                0.0375,
                1,
                PacCollar::standard(),
            ),
            CmoTranche::support("SUP", Money::new(50_000_000.0, Currency::USD), 0.04, 2),
        ];

        Self::builder()
            .id(InstrumentId::new("FNR-2024-2-PAC"))
            .deal_name("FNR 2024-2".into())
            .agency(AgencyProgram::Fnma)
            .issue_date(date!(2024 - 01 - 01))
            .waterfall(CmoWaterfall::new(tranches))
            .reference_tranche_id("PAC".to_string())
            .collateral_wac(0.045)
            .collateral_wam(360)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
    }

    /// Create an example IO/PO strip structure.
    pub fn example_io_po() -> finstack_quant_core::Result<Self> {
        use time::macros::date;
        // IO strips the full 3.5% pass-through (4.0% WAC less 50bp fees).
        let tranches = vec![
            CmoTranche::io_strip("IO", Money::new(100_000_000.0, Currency::USD), 0.035),
            CmoTranche::po_strip("PO", Money::new(100_000_000.0, Currency::USD)),
        ];

        Self::builder()
            .id(InstrumentId::new("FNS-2024-1-IO"))
            .deal_name("FNS 2024-1".into())
            .agency(AgencyProgram::Fnma)
            .issue_date(date!(2024 - 01 - 01))
            .waterfall(CmoWaterfall::new(tranches))
            .reference_tranche_id("IO".to_string())
            .collateral_wac(0.04)
            .collateral_wam(360)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
    }

    /// Get the reference tranche being valued.
    pub fn reference_tranche(&self) -> Option<&CmoTranche> {
        self.waterfall.get_tranche(&self.reference_tranche_id)
    }

    /// Validate that the deal's tranche coupon demand does not exceed the
    /// collateral interest supply (interest conservation).
    ///
    /// Two checks run, both against the net pass-through coupon (collateral
    /// WAC less servicing and guarantee fees):
    ///
    /// 1. **Structural (per tranche)**: every principal-bearing tranche with a
    ///    fixed coupon (sequential, PAC, support) must have
    ///    `coupon <= net pass-through`. The aggregate weighted-average test
    ///    alone is insufficient: in a sequential structure the surviving
    ///    pool's weighted coupon rises toward the maximum tranche coupon as
    ///    front tranches retire, so a deal that is covered at t=0 can become
    ///    interest-deficient later. IO strips are excluded (their notional
    ///    references the pass-through and total demand is bounded by check 2);
    ///    PO strips carry no coupon. Accrual (Z) tranches are included: during
    ///    the accretion phase the Z's accrual is funded from interest
    ///    collections (as accretion-directed principal), and after seniors
    ///    retire it is paid cash interest — in both phases its funding demand
    ///    is `coupon × balance`, exactly like a current-pay tranche, so the
    ///    same `coupon <= net pass-through` bound applies.
    /// 2. **Aggregate (t=0)**: total annual coupon demand across all
    ///    interest-bearing tranches (including IO strips) must not exceed
    ///    `pass_through_rate × collateral face`.
    ///
    /// # Errors
    ///
    /// Returns a validation error when any principal-bearing tranche coupon
    /// exceeds the net pass-through coupon, or when aggregate annual coupon
    /// demand exceeds `pass_through_rate × collateral face`.
    fn validate_interest_coverage(cmo: &AgencyCmo) -> finstack_quant_core::Result<()> {
        let (pass_through, collateral_face) = match &cmo.collateral {
            Some(pool) => (pool.pass_through_rate, pool.current_face.amount()),
            None => {
                // Mirror the assumed-collateral construction in the pricer.
                let defaults = crate::instruments::fixed_income::structured_credit::assumptions::embedded_registry()?
                    .cmo_collateral_defaults();
                let wac = cmo.collateral_wac.unwrap_or(defaults.wac);
                let pass_through = wac - defaults.servicing_fee_rate - defaults.guarantee_fee_rate;
                (pass_through, cmo.waterfall.total_current_face().amount())
            }
        };

        // Structural check: a fixed-coupon tranche owed interest on its
        // principal balance can never demand more than the net pass-through,
        // otherwise it goes interest-short once higher-priority tranches
        // retire even if the deal is covered in aggregate at t=0.
        for tranche in &cmo.waterfall.tranches {
            if tranche.is_interest_bearing()
                && tranche.receives_principal()
                && tranche.coupon > pass_through * (1.0 + 1e-9)
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Interest-deficient CMO {}: tranche '{}' coupon {:.4} exceeds the net \
                     pass-through coupon {:.4}; a fixed-coupon principal tranche cannot be \
                     covered once senior tranches retire",
                    cmo.id.as_str(),
                    tranche.id,
                    tranche.coupon,
                    pass_through
                )));
            }
        }

        let annual_supply = pass_through * collateral_face;
        let annual_demand: f64 = cmo
            .waterfall
            .tranches
            .iter()
            .filter(|t| t.is_interest_bearing())
            .map(|t| t.current_face.amount() * t.coupon)
            .sum();

        // Relative tolerance so a fully-stripped IO (demand == supply) passes.
        if annual_demand > annual_supply * (1.0 + 1e-9) + 1e-6 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Interest-deficient CMO {}: annual tranche coupon demand {:.2} exceeds \
                 collateral interest supply {:.2} (pass-through {:.4} on face {:.2})",
                cmo.id.as_str(),
                annual_demand,
                annual_supply,
                pass_through,
                collateral_face
            )));
        }
        Ok(())
    }
}

impl finstack_quant_cashflows::CashflowScheduleSource for AgencyCmo {
    fn notional(&self) -> Option<Money> {
        self.reference_tranche().map(|tranche| tranche.current_face)
    }

    fn raw_cashflow_schedule(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        let _ = curves;
        let schedule =
            crate::instruments::fixed_income::cmo::pricer::build_reference_tranche_schedule(
                self, as_of, None,
            )?;
        Ok(schedule
            .with_representation(crate::cashflow::builder::CashflowRepresentation::Projected))
    }
}

impl crate::instruments::common_impl::traits::Instrument for AgencyCmo {
    impl_instrument_base!(crate::pricer::InstrumentType::AgencyCmo);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        Ok(deps)
    }

    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        crate::instruments::fixed_income::cmo::pricer::price_cmo(self, market, as_of)
    }

    fn effective_start_date(&self) -> Option<Date> {
        Some(self.issue_date)
    }

    crate::impl_focused_pricing_overrides!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmo_example() {
        let cmo = AgencyCmo::example().expect("AgencyCmo example is valid");
        assert_eq!(cmo.agency, AgencyProgram::Fnma);
        assert_eq!(cmo.waterfall.tranches.len(), 3);
    }

    #[test]
    fn test_tranche_types() {
        let cmo = AgencyCmo::example().expect("AgencyCmo example is valid");

        for tranche in &cmo.waterfall.tranches {
            assert_eq!(tranche.tranche_type, CmoTrancheType::Sequential);
        }
    }

    #[test]
    fn test_pac_support_structure() {
        let cmo = AgencyCmo::example_pac_support().expect("AgencyCmo PAC/support example is valid");

        let pac = cmo.waterfall.get_tranche("PAC").expect("PAC exists");
        assert_eq!(pac.tranche_type, CmoTrancheType::Pac);
        assert!(pac.pac_collar.is_some());

        let sup = cmo.waterfall.get_tranche("SUP").expect("SUP exists");
        assert_eq!(sup.tranche_type, CmoTrancheType::Support);
    }

    #[test]
    fn test_io_po_structure() {
        let cmo = AgencyCmo::example_io_po().expect("AgencyCmo IO/PO example is valid");

        let io = cmo.waterfall.get_tranche("IO").expect("IO exists");
        assert_eq!(io.tranche_type, CmoTrancheType::InterestOnly);
        assert!(io.is_interest_bearing());
        assert!(!io.receives_principal());

        let po = cmo.waterfall.get_tranche("PO").expect("PO exists");
        assert_eq!(po.tranche_type, CmoTrancheType::PrincipalOnly);
        assert!(!po.is_interest_bearing());
        assert!(po.receives_principal());
    }

    #[test]
    fn test_total_face() {
        let cmo = AgencyCmo::example().expect("AgencyCmo example is valid");
        let total = cmo.waterfall.total_current_face();

        // 40M + 30M + 30M = 100M
        assert!((total.amount() - 100_000_000.0).abs() < 1.0);
    }

    #[test]
    fn test_reference_tranche() {
        let cmo = AgencyCmo::example().expect("AgencyCmo example is valid");
        let ref_tranche = cmo.reference_tranche().expect("ref exists");

        assert_eq!(ref_tranche.id, "A");
    }

    /// Finding 17: deals whose tranche coupon demand exceeds the collateral
    /// pass-through interest are interest-deficient and rejected at build
    /// (here B/Z coupons exceed the 4% pass-through and aggregate demand
    /// is 4.45% of face against a 4% supply).
    #[test]
    fn interest_deficient_deal_rejected_at_build() {
        use time::macros::date;
        let tranches = vec![
            CmoTranche::sequential("A", Money::new(40_000_000.0, Currency::USD), 0.04, 1),
            CmoTranche::sequential("B", Money::new(30_000_000.0, Currency::USD), 0.045, 2),
            CmoTranche::sequential("Z", Money::new(30_000_000.0, Currency::USD), 0.05, 3),
        ];

        let result = AgencyCmo::builder()
            .id(InstrumentId::new("FNR-DEFICIENT"))
            .deal_name("FNR DEFICIENT".into())
            .agency(AgencyProgram::Fnma)
            .issue_date(date!(2024 - 01 - 01))
            .waterfall(CmoWaterfall::new(tranches))
            .reference_tranche_id("A".to_string())
            .collateral_wac(0.045) // pass-through 4.0% < 4.45% demand
            .collateral_wam(360)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build();

        let err = result.expect_err("interest-deficient deal must be rejected");
        assert!(
            err.to_string().contains("Interest-deficient"),
            "unexpected error: {err}"
        );
    }

    /// Structural per-tranche coverage: a deal can satisfy the aggregate t=0
    /// weighted-average test yet still carry a tranche coupon above the net
    /// pass-through. In sequential pay that tranche goes interest-short once
    /// senior tranches retire, so validation must reject it.
    ///
    /// This is the pre-fix `example()` structure: A 3.5%/40M, B 4.0%/30M,
    /// Z 4.5%/30M on a 4.0% net pass-through — aggregate demand $3.95M is
    /// under the $4.0M supply, but the 4.5% tranche exceeds the pass-through.
    #[test]
    fn tranche_coupon_above_pass_through_rejected_even_if_aggregate_covered() {
        use time::macros::date;
        let tranches = vec![
            CmoTranche::sequential("A", Money::new(40_000_000.0, Currency::USD), 0.035, 1),
            CmoTranche::sequential("B", Money::new(30_000_000.0, Currency::USD), 0.04, 2),
            CmoTranche::sequential("Z", Money::new(30_000_000.0, Currency::USD), 0.045, 3),
        ];

        let result = AgencyCmo::builder()
            .id(InstrumentId::new("FNR-STRUCTURAL"))
            .deal_name("FNR STRUCTURAL".into())
            .agency(AgencyProgram::Fnma)
            .issue_date(date!(2024 - 01 - 01))
            .waterfall(CmoWaterfall::new(tranches))
            .reference_tranche_id("A".to_string())
            .collateral_wac(0.045) // net pass-through 4.0% < Z's 4.5% coupon
            .collateral_wam(360)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build();

        let err = result.expect_err("tranche coupon above pass-through must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("tranche 'Z'") && msg.contains("pass-through"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_accrual_example_structure() {
        let cmo = AgencyCmo::example_accrual().expect("accrual example is valid");
        let z = cmo.waterfall.get_tranche("Z").expect("Z exists");
        assert_eq!(z.tranche_type, CmoTrancheType::Accrual);
        assert!(z.is_interest_bearing());
        assert!(z.receives_principal());
        assert_eq!(cmo.reference_tranche_id, "Z");
    }

    /// An accrual (Z) tranche's coupon must not exceed the net pass-through:
    /// its accrual is funded from interest collections (accretion-directed
    /// principal during accretion, cash interest after seniors retire), so
    /// the same structural bound as current-pay tranches applies.
    #[test]
    fn accrual_coupon_above_pass_through_rejected() {
        use time::macros::date;
        let tranches = vec![
            CmoTranche::sequential("A", Money::new(40_000_000.0, Currency::USD), 0.035, 1),
            CmoTranche::accrual("Z", Money::new(30_000_000.0, Currency::USD), 0.045, 2),
        ];

        let result = AgencyCmo::builder()
            .id(InstrumentId::new("FNR-Z-DEFICIENT"))
            .deal_name("FNR Z DEFICIENT".into())
            .agency(AgencyProgram::Fnma)
            .issue_date(date!(2024 - 01 - 01))
            .waterfall(CmoWaterfall::new(tranches))
            .reference_tranche_id("Z".to_string())
            .collateral_wac(0.045) // net pass-through 4.0% < Z's 4.5% coupon
            .collateral_wam(360)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build();

        let err = result.expect_err("accrual coupon above pass-through must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("tranche 'Z'") && msg.contains("pass-through"),
            "unexpected error: {err}"
        );
    }

    /// Accrual tranches participate in the waterfall principal order and so
    /// require a positive priority, like other principal-paying classes.
    #[test]
    fn accrual_zero_priority_rejected() {
        use time::macros::date;
        let tranches = vec![
            CmoTranche::sequential("A", Money::new(40_000_000.0, Currency::USD), 0.035, 1),
            CmoTranche::accrual("Z", Money::new(30_000_000.0, Currency::USD), 0.04, 0),
        ];

        let result = AgencyCmo::builder()
            .id(InstrumentId::new("FNR-Z-PRIORITY"))
            .deal_name("FNR Z PRIORITY".into())
            .agency(AgencyProgram::Fnma)
            .issue_date(date!(2024 - 01 - 01))
            .waterfall(CmoWaterfall::new(tranches))
            .reference_tranche_id("Z".to_string())
            .collateral_wac(0.045)
            .collateral_wam(360)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build();

        let err = result.expect_err("zero-priority accrual tranche must be rejected");
        assert!(
            err.to_string().contains("positive priority"),
            "unexpected error: {err}"
        );
    }
}
