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
