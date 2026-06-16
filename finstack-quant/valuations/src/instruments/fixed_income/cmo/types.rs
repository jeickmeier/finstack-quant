//! Agency CMO types.
//!
//! Collateralized Mortgage Obligations (CMOs) are structured products that
//! redistribute the cashflows from underlying MBS pools into tranches with
//! different risk/return profiles.

use crate::cashflow::traits::CashflowProvider;
use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::fixed_income::mbs_passthrough::{AgencyMbsPassthrough, AgencyProgram};
use crate::instruments::PricingOverrides;
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
}

impl std::fmt::Display for CmoTrancheType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CmoTrancheType::Sequential => write!(f, "SEQ"),
            CmoTrancheType::Pac => write!(f, "PAC"),
            CmoTrancheType::Support => write!(f, "SUP"),
            CmoTrancheType::InterestOnly => write!(f, "IO"),
            CmoTrancheType::PrincipalOnly => write!(f, "PO"),
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
    pub fn is_interest_bearing(&self) -> bool {
        self.coupon > 0.0 && self.tranche_type != CmoTrancheType::PrincipalOnly
    }

    /// Check if tranche receives principal.
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
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
#[builder(validate = AgencyCmo::validate_interest_coverage)]
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
    pub pricing_overrides: PricingOverrides,
    /// Attributes for tagging and selection.
    #[builder(default)]
    #[serde(default)]
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

impl AgencyCmo {
    /// Create a canonical example CMO for testing.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use time::macros::date;
        // Create sequential structure: A (front), B (middle), Z (last).
        // Coupons average 3.95% on a 4.5% WAC pool (4.0% pass-through after
        // 50bp fees), so the structure is interest-covered.
        let tranches = vec![
            CmoTranche::sequential("A", Money::new(40_000_000.0, Currency::USD), 0.035, 1),
            CmoTranche::sequential("B", Money::new(30_000_000.0, Currency::USD), 0.04, 2),
            CmoTranche::sequential("Z", Money::new(30_000_000.0, Currency::USD), 0.045, 3),
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
    /// collateral interest supply at issue (interest conservation).
    ///
    /// A structure whose tranches demand more coupon than the collateral
    /// pass-through delivers is interest-deficient: every period some tranche
    /// records an interest shortfall. Such deals are rejected at build time.
    ///
    /// # Errors
    ///
    /// Returns a validation error when annual tranche coupon demand exceeds
    /// `pass_through_rate × collateral face`.
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

impl crate::instruments::common_impl::traits::CurveDependencies for AgencyCmo {
    fn curve_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::common_impl::traits::InstrumentCurves>
    {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .build()
    }
}

impl CashflowProvider for AgencyCmo {
    fn notional(&self) -> Option<Money> {
        self.reference_tranche().map(|tranche| tranche.current_face)
    }

    fn cashflow_schedule(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        let _ = curves;
        let mut schedule =
            crate::instruments::fixed_income::cmo::pricer::build_reference_tranche_schedule(
                self, as_of, None,
            )?;
        schedule.meta.representation = crate::cashflow::builder::CashflowRepresentation::Projected;
        Ok(schedule)
    }
}

impl crate::instruments::common_impl::traits::Instrument for AgencyCmo {
    impl_instrument_base!(crate::pricer::InstrumentType::AgencyCmo);

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

    fn pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&mut self.pricing_overrides)
    }

    fn pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&self.pricing_overrides)
    }
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
    /// pass-through interest are interest-deficient and rejected at build.
    /// (This was the pre-fix `example()`: Z at 5% on a 4% pass-through pool.)
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
}
