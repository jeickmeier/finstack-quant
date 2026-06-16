//! Core enumeration types for structured credit instruments.
//!
//! This module provides all the enumeration types used to classify and categorize
//! various aspects of structured credit instruments including deal types, asset types,
//! credit ratings, and payment modes.

use finstack_quant_core::dates::Date;

use serde::{Deserialize, Serialize};

// ============================================================================
// CORE DEAL TYPES
// ============================================================================

/// Primary structured credit deal classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[non_exhaustive]
pub enum DealType {
    /// Collateralized Loan Obligation
    CLO,
    /// Collateralized Bond Obligation
    CBO,
    /// Generic Asset-Backed Security
    ABS,
    /// Residential Mortgage-Backed Security
    RMBS,
    /// Commercial Mortgage-Backed Security
    CMBS,
    /// Auto Loan ABS
    Auto,
    /// Credit Card ABS
    Card,
}

impl core::fmt::Display for DealType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DealType::CLO => write!(f, "CLO"),
            DealType::CBO => write!(f, "CBO"),
            DealType::ABS => write!(f, "ABS"),
            DealType::RMBS => write!(f, "RMBS"),
            DealType::CMBS => write!(f, "CMBS"),
            DealType::Auto => write!(f, "Auto ABS"),
            DealType::Card => write!(f, "Credit Card ABS"),
        }
    }
}

/// Tranche seniority levels
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[non_exhaustive]
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
            TrancheSeniority::Senior => write!(f, "Senior"),
            TrancheSeniority::Mezzanine => write!(f, "Mezzanine"),
            TrancheSeniority::Subordinated => write!(f, "Subordinated"),
            TrancheSeniority::Equity => write!(f, "Equity"),
        }
    }
}

// ============================================================================
// ASSET CLASSIFICATION
// ============================================================================

/// Asset type classification for pool composition (flattened hierarchy)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum AssetType {
    // ========== LOAN TYPES ==========
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

    // ========== BOND TYPES ==========
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

    // ========== MORTGAGE TYPES ==========
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

    // ========== AUTO LOAN TYPES ==========
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

    // ========== CREDIT CARD TYPES ==========
    /// Prime credit card receivables
    PrimeCreditCard,
    /// Subprime credit card receivables
    SubPrimeCreditCard,
    /// Super prime credit card receivables
    SuperPrimeCreditCard,
    /// Commercial credit card receivables
    CommercialCreditCard,

    // ========== STUDENT LOAN TYPES ==========
    /// Federal student loan
    FederalStudentLoan,
    /// Private student loan
    PrivateStudentLoan,
    /// FFELP student loan
    FFELPStudentLoan,
    /// Consolidation student loan
    ConsolidationStudentLoan,

    // ========== OTHER TYPES ==========
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

// ============================================================================
// PAYMENT & WATERFALL
// ============================================================================

/// Payment distribution modes
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "mode")]
#[non_exhaustive]
pub enum PaymentMode {
    /// Normal pro-rata payments to all tranches
    ProRata,
    /// Sequential payment (turbo) due to trigger breach
    Sequential {
        /// Triggered by.
        triggered_by: String,
        /// Trigger date.
        #[schemars(with = "String")]
        trigger_date: Date,
    },
    /// Hybrid mode with custom rules
    Hybrid {
        /// Description.
        description: String,
    },
}

impl AssetType {
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
                | AssetType::FFELPStudentLoan
                | AssetType::ConsolidationStudentLoan
                | AssetType::Equipment { .. }
        )
    }
}

/// Consequences when triggers are breached
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[non_exhaustive]
pub enum TriggerConsequence {
    /// Divert Cash Flow variant.
    DivertCashFlow,
    /// Trap Excess Spread variant.
    TrapExcessSpread,
    /// Accelerate Amortization variant.
    AccelerateAmortization,
    /// Stop Reinvestment variant.
    StopReinvestment,
    /// Reduce Manager Fee variant.
    ReduceManagerFee,
    /// Custom variant.
    Custom(String),
}
