//! Stable identifiers for convention lookups.
//!
//! This module provides type-safe identifiers for all convention types. These identifiers
//! prevent accidental mismatches between different ID types and ensure conventions are looked
//! up correctly.

use finstack_quant_core::currency::Currency;
use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! define_convention_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord, schemars::JsonSchema)]
        pub struct $name(String);

        impl $name {
            /// Create a new identifier from a string.
            pub fn new(s: impl Into<String>) -> Self { Self(s.into()) }

            /// View the inner string representation.
            pub fn as_str(&self) -> &str { &self.0 }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.0) }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self { Self::new(s) }
        }
    };
}

define_convention_id!(
    /// Stable identifier for an Interest Rate Future contract (e.g., "CME:SR3").
    ///
    /// Used to look up [`IrFutureConventions`](crate::market::conventions::defs::IrFutureConventions)
    /// from the convention registry.
    IrFutureContractId
);

define_convention_id!(
    /// Identifier for Option market conventions (Equity/FX/Commodity).
    ///
    /// Used to look up [`OptionConventions`](crate::market::conventions::defs::OptionConventions)
    /// from the convention registry.
    OptionConventionId
);

define_convention_id!(
    /// Identifier for Swaption market conventions (e.g., "USD", "EUR").
    ///
    /// Used to look up [`SwaptionConventions`](crate::market::conventions::defs::SwaptionConventions)
    /// from the convention registry.
    SwaptionConventionId
);

define_convention_id!(
    /// Identifier for cap/floor market conventions (e.g., "USD-SOFR-CAP").
    CapFloorConventionId
);

define_convention_id!(
    /// Identifier for Inflation Swap market conventions (e.g., "USD-CPI", "UK-RPI").
    ///
    /// Used to look up [`InflationSwapConventions`](crate::market::conventions::defs::InflationSwapConventions)
    /// from the convention registry.
    InflationSwapConventionId
);

define_convention_id!(
    /// Identifier for FX pair market conventions (e.g., "EUR/USD", "USD/CAD").
    FxConventionId
);

define_convention_id!(
    /// Identifier for bond market conventions (e.g., "USD-UST", "USD-CORP").
    BondConventionId
);

define_convention_id!(
    /// Identifier for cross-currency swap market conventions (e.g., "EUR/USD-XCCY").
    XccyConventionId
);

define_convention_id!(
    /// Identifier for FX option market conventions (e.g., "EUR/USD-VANILLA").
    FxOptionConventionId
);

/// CDS market standard documentation clauses.
///
/// Represents the ISDA documentation clause used for CDS contracts. Different clauses
/// define different restructuring events and settlement procedures. Used as part of
/// [`CdsConventionKey`] to look up CDS conventions.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::market::conventions::ids::CdsDocClause;
///
/// let clause = CdsDocClause::Cr14; // Cum-Restructuring 2014
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CdsDocClause {
    /// Cum-Restructuring 2014 (CR14)
    Cr14,
    /// Modified-Restructuring 2014 (MR14)
    Mr14,
    /// Modified-Modified-Restructuring 2014 (MM14)
    Mm14,
    /// No-Restructuring 2014 (XR14)
    Xr14,
    /// ISDA North American Corporate (IsdaNa)
    IsdaNa,
    /// ISDA European Corporate (IsdaEu)
    IsdaEu,
    /// ISDA Asia Corporate (IsdaAs)
    IsdaAs,
    /// ISDA Australia Corporate (IsdaAu)
    IsdaAu,
    /// ISDA New Zealand Corporate (IsdaNz)
    IsdaNz,
    /// Custom / Other
    Custom,
}

impl std::str::FromStr for CdsDocClause {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "cr14" | "cr" => Ok(CdsDocClause::Cr14),
            "mr14" | "mr" => Ok(CdsDocClause::Mr14),
            "mm14" | "mm" => Ok(CdsDocClause::Mm14),
            "xr14" | "xr" => Ok(CdsDocClause::Xr14),
            "isdana" | "isda_na" | "na" => Ok(CdsDocClause::IsdaNa),
            "isdaeu" | "isda_eu" | "eu" => Ok(CdsDocClause::IsdaEu),
            "isdaas" | "isda_as" | "as" => Ok(CdsDocClause::IsdaAs),
            "isdaau" | "isda_au" | "au" => Ok(CdsDocClause::IsdaAu),
            "isdanz" | "isda_nz" | "nz" => Ok(CdsDocClause::IsdaNz),
            "custom" => Ok(CdsDocClause::Custom),
            _ => Err(format!("Unknown CDS doc clause: {}", s)),
        }
    }
}

/// Key to look up CDS conventions (Currency + DocClause).
///
/// CDS conventions are identified by both currency and documentation clause, as different
/// clauses have different market conventions. Used to look up [`CdsConventions`](crate::market::conventions::defs::CdsConventions)
/// from the convention registry.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
/// use finstack_quant_core::currency::Currency;
///
/// let key = CdsConventionKey {
///     currency: Currency::USD,
///     doc_clause: CdsDocClause::Cr14,
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CdsConventionKey {
    /// The currency of the CDS.
    pub currency: Currency,
    /// The documentation clause (e.g. CR14, MM14).
    pub doc_clause: CdsDocClause,
}

impl fmt::Display for CdsConventionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.currency, self.doc_clause.as_str())
    }
}

impl CdsDocClause {
    /// Returns the canonical snake_case string representation for registry lookups.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cr14 => "cr14",
            Self::Mr14 => "mr14",
            Self::Mm14 => "mm14",
            Self::Xr14 => "xr14",
            Self::IsdaNa => "isda_na",
            Self::IsdaEu => "isda_eu",
            Self::IsdaAs => "isda_as",
            Self::IsdaAu => "isda_au",
            Self::IsdaNz => "isda_nz",
            Self::Custom => "custom",
        }
    }
}
