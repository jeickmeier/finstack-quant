//! Core enumeration types for structured credit instruments.
//!
//! This module provides all the enumeration types used to classify and categorize
//! various aspects of structured credit instruments including deal types, asset types,
//! credit ratings, and payment modes.

use finstack_quant_core::dates::Date;

use serde::{Deserialize, Serialize};

/// Primary structured credit deal classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum DealType {
    /// Collateralized Loan Obligation
    Clo,
    /// Collateralized Bond Obligation
    Cbo,
    /// Generic Asset-Backed Security
    Abs,
    /// Residential Mortgage-Backed Security
    Rmbs,
    /// Commercial Mortgage-Backed Security
    Cmbs,
    /// Auto Loan ABS
    Auto,
    /// Credit Card ABS
    Card,
}

impl core::fmt::Display for DealType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DealType::Clo => write!(f, "CLO"),
            DealType::Cbo => write!(f, "CBO"),
            DealType::Abs => write!(f, "ABS"),
            DealType::Rmbs => write!(f, "RMBS"),
            DealType::Cmbs => write!(f, "CMBS"),
            DealType::Auto => write!(f, "Auto ABS"),
            DealType::Card => write!(f, "Credit Card ABS"),
        }
    }
}

/// Tranche seniority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum TrancheSeniority {
    /// Most senior debt tranche
    Senior = 0,
    /// Mezzanine debt tranches
    Mezzanine = 1,
    /// Subordinated debt tranches
    Subordinated = 2,
    /// Equity/first loss piece
    Equity = 3,
}

impl core::fmt::Display for TrancheSeniority {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TrancheSeniority::Senior => write!(f, "senior"),
            TrancheSeniority::Mezzanine => write!(f, "mezzanine"),
            TrancheSeniority::Subordinated => write!(f, "subordinated"),
            TrancheSeniority::Equity => write!(f, "equity"),
        }
    }
}

/// Asset type classification for pool composition (flattened hierarchy)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type")]
#[non_exhaustive]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum AssetType {
    /// First lien corporate loan
    FirstLienLoan {
        /// Industry.
        industry: Option<String>,
    },
    /// Second lien corporate loan
    SecondLienLoan {
        /// Industry.
        industry: Option<String>,
    },
    /// Revolving credit facility
    RevolverLoan {
        /// Industry.
        industry: Option<String>,
    },
    /// Bridge loan
    BridgeLoan {
        /// Industry.
        industry: Option<String>,
    },
    /// Mezzanine loan
    MezzanineLoan {
        /// Industry.
        industry: Option<String>,
    },

    /// High yield bond
    HighYieldBond {
        /// Industry.
        industry: Option<String>,
    },
    /// Investment grade bond
    InvestmentGradeBond {
        /// Industry.
        industry: Option<String>,
    },
    /// Distressed bond
    DistressedBond {
        /// Industry.
        industry: Option<String>,
    },
    /// Emerging markets bond
    EmergingMarketsBond {
        /// Industry.
        industry: Option<String>,
    },

    /// Single family residential mortgage
    SingleFamilyMortgage {
        /// Ltv.
        ltv: Option<f64>,
    },
    /// Multifamily residential mortgage
    MultifamilyMortgage {
        /// Ltv.
        ltv: Option<f64>,
    },
    /// Commercial real estate mortgage
    CommercialMortgage {
        /// Ltv.
        ltv: Option<f64>,
    },
    /// Industrial property mortgage
    IndustrialMortgage {
        /// Ltv.
        ltv: Option<f64>,
    },
    /// Retail property mortgage
    RetailMortgage {
        /// Ltv.
        ltv: Option<f64>,
    },
    /// Office property mortgage
    OfficeMortgage {
        /// Ltv.
        ltv: Option<f64>,
    },
    /// Hotel property mortgage
    HotelMortgage {
        /// Ltv.
        ltv: Option<f64>,
    },
    /// Other property type mortgage
    OtherMortgage {
        /// Property type.
        property_type: String,
        /// Ltv.
        ltv: Option<f64>,
    },

    /// New vehicle auto loan
    NewAutoLoan {
        /// Ltv.
        ltv: Option<f64>,
    },
    /// Used vehicle auto loan
    UsedAutoLoan {
        /// Ltv.
        ltv: Option<f64>,
    },
    /// Vehicle lease
    LeaseAutoLoan {
        /// Ltv.
        ltv: Option<f64>,
    },
    /// Fleet vehicle loan
    FleetAutoLoan {
        /// Ltv.
        ltv: Option<f64>,
    },

    /// Prime credit card receivables
    PrimeCreditCard,
    /// Subprime credit card receivables
    SubPrimeCreditCard,
    /// Super prime credit card receivables
    SuperPrimeCreditCard,
    /// Commercial credit card receivables
    CommercialCreditCard,

    /// Federal student loan
    FederalStudentLoan,
    /// Private student loan
    PrivateStudentLoan,
    /// FFELP student loan
    FfelpStudentLoan,
    /// Consolidation student loan
    ConsolidationStudentLoan,

    /// Equipment financing
    Equipment {
        /// Equipment type.
        equipment_type: String,
    },
    /// Generic asset placeholder
    Generic {
        /// Description.
        description: String,
        /// Asset class.
        asset_class: String,
    },
}

/// Payment distribution modes
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum PaymentMode {
    /// Normal pro-rata payments to all tranches
    ProRata,
    /// Sequential payment (turbo) due to trigger breach
    Sequential {
        /// Triggered by.
        triggered_by: String,
        /// Trigger date.
        #[serde(with = "finstack_quant_core::wire::date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        trigger_date: Date,
    },
    /// Hybrid mode with custom rules
    Hybrid {
        /// Description.
        description: String,
    },
}

impl AssetType {
    /// Serde wire name of the variant (`"first_lien_loan"`, `"new_auto_loan"`
    /// ...), the key advance rates and concentration limits are written on.
    pub fn wire_name(&self) -> &'static str {
        match self {
            AssetType::FirstLienLoan { .. } => "first_lien_loan",
            AssetType::SecondLienLoan { .. } => "second_lien_loan",
            AssetType::RevolverLoan { .. } => "revolver_loan",
            AssetType::BridgeLoan { .. } => "bridge_loan",
            AssetType::MezzanineLoan { .. } => "mezzanine_loan",
            AssetType::HighYieldBond { .. } => "high_yield_bond",
            AssetType::InvestmentGradeBond { .. } => "investment_grade_bond",
            AssetType::DistressedBond { .. } => "distressed_bond",
            AssetType::EmergingMarketsBond { .. } => "emerging_markets_bond",
            AssetType::SingleFamilyMortgage { .. } => "single_family_mortgage",
            AssetType::MultifamilyMortgage { .. } => "multifamily_mortgage",
            AssetType::CommercialMortgage { .. } => "commercial_mortgage",
            AssetType::IndustrialMortgage { .. } => "industrial_mortgage",
            AssetType::RetailMortgage { .. } => "retail_mortgage",
            AssetType::OfficeMortgage { .. } => "office_mortgage",
            AssetType::HotelMortgage { .. } => "hotel_mortgage",
            AssetType::OtherMortgage { .. } => "other_mortgage",
            AssetType::NewAutoLoan { .. } => "new_auto_loan",
            AssetType::UsedAutoLoan { .. } => "used_auto_loan",
            AssetType::LeaseAutoLoan { .. } => "lease_auto_loan",
            AssetType::FleetAutoLoan { .. } => "fleet_auto_loan",
            AssetType::PrimeCreditCard => "prime_credit_card",
            AssetType::SubPrimeCreditCard => "sub_prime_credit_card",
            AssetType::SuperPrimeCreditCard => "super_prime_credit_card",
            AssetType::CommercialCreditCard => "commercial_credit_card",
            AssetType::FederalStudentLoan => "federal_student_loan",
            AssetType::PrivateStudentLoan => "private_student_loan",
            AssetType::FfelpStudentLoan => "ffelp_student_loan",
            AssetType::ConsolidationStudentLoan => "consolidation_student_loan",
            AssetType::Equipment { .. } => "equipment",
            AssetType::Generic { .. } => "generic",
        }
    }

    /// Returns `true` for asset types that amortize through level payments
    /// (mortgages, auto loans, student loans, equipment).
    ///
    /// Bullet instruments (corporate loans, bonds, credit cards) return `false`.
    pub fn is_amortizing(&self) -> bool {
        matches!(
            self,
            AssetType::SingleFamilyMortgage { .. }
                | AssetType::MultifamilyMortgage { .. }
                | AssetType::CommercialMortgage { .. }
                | AssetType::IndustrialMortgage { .. }
                | AssetType::RetailMortgage { .. }
                | AssetType::OfficeMortgage { .. }
                | AssetType::HotelMortgage { .. }
                | AssetType::OtherMortgage { .. }
                | AssetType::NewAutoLoan { .. }
                | AssetType::UsedAutoLoan { .. }
                | AssetType::LeaseAutoLoan { .. }
                | AssetType::FleetAutoLoan { .. }
                | AssetType::FederalStudentLoan
                | AssetType::PrivateStudentLoan
                | AssetType::FfelpStudentLoan
                | AssetType::ConsolidationStudentLoan
                | AssetType::Equipment { .. }
        )
    }
}

/// Consequences when triggers are breached
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum TriggerConsequence {
    /// Divert Cash Flow variant.
    DivertCashFlow,
    /// Trap Excess Spread variant.
    TrapExcessSpread,
    /// Accelerate Amortization variant.
    AccelerateAmortization,
    /// Stop Reinvestment variant.
    StopReinvestment,
}

/// How collateral losses reach the note balances.
///
/// Realized net loss (`default × (1 − recovery)`) is always tracked for the
/// cumulative-loss triggers; this policy decides whether it also reduces the
/// notes' outstanding principal before legal final.
///
/// | Policy | Market | Note balances | Loss realized |
/// |---|---|---|---|
/// | `WriteDown` | RMBS, CMBS | reduced junior-first at each default | at default |
/// | `ParPreserving` | CLO, CBO, ABS, cards | carried at par; OC tests and the residual absorb losses | unpaid principal at legal final / liquidation |
///
/// Under `ParPreserving` a subordinated note keeps accruing its full coupon
/// ahead of the residual holder; the OC numerator carries each defaulted
/// asset at its modeled recovery value until the recovery cash arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum LossAllocationPolicy {
    /// Allocate realized net loss to the notes junior-first when the
    /// collateral defaults (realized-loss allocation).
    WriteDown,
    /// Keep note balances at par; losses surface through coverage tests and
    /// as a principal shortfall at legal final.
    ParPreserving,
}

impl LossAllocationPolicy {
    /// Market-standard policy for a deal type: `WriteDown` for RMBS and CMBS,
    /// `ParPreserving` for every other family.
    ///
    /// # Arguments
    ///
    /// * `deal_type` - Deal family whose indenture convention selects the policy.
    pub fn default_for(deal_type: DealType) -> Self {
        match deal_type {
            DealType::Rmbs | DealType::Cmbs => Self::WriteDown,
            DealType::Clo | DealType::Cbo | DealType::Abs | DealType::Auto | DealType::Card => {
                Self::ParPreserving
            }
        }
    }
}

/// When a collateral loss is booked: at default (expected net loss, the
/// INTEX/Moody's Analytics convention for corporate collateral) or when the
/// defaulted loan liquidates and its recovery settles (mortgage servicing
/// convention, where the realized loss is known only at liquidation).
///
/// The timing drives every cumulative-loss quantity: note write-downs under
/// [`LossAllocationPolicy::WriteDown`], the step-down and early-amortization
/// loss triggers and the excess-spread trap. The OC tests carry defaulted
/// collateral at its recovery value from the default date under both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum LossRecognition {
    /// Book `defaulted par − expected recovery` on the default date.
    AtDefault,
    /// Book `defaulted par − recovery` when the claim settles after the
    /// recovery lag (or at the simulation's end for claims still pending).
    AtLiquidation,
}

impl LossRecognition {
    /// Market-standard timing for a deal type: `AtLiquidation` for RMBS and
    /// CMBS, `AtDefault` for every other family.
    ///
    /// # Arguments
    ///
    /// * `deal_type` - Deal family whose servicing convention selects the timing.
    pub fn default_for(deal_type: DealType) -> Self {
        match deal_type {
            DealType::Rmbs | DealType::Cmbs => Self::AtLiquidation,
            DealType::Clo | DealType::Cbo | DealType::Abs | DealType::Auto | DealType::Card => {
                Self::AtDefault
            }
        }
    }
}
