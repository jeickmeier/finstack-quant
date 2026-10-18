//! Host-facing authoring constructors for quotes and calibration steps.
//!
//! The Python and WASM bindings expose one named constructor per quote type
//! and per step kind (`RateQuote.deposit`, `CalibrationStep.discount`, ...).
//! Quote constructors call the typed constructors here directly without a JSON
//! round-trip; step constructors marshal their open-ended keyword overrides
//! into [`CalibrationStep::from_wire_fields`].

use crate::api::schema::CalibrationStep;
use crate::quotes::cds::CdsQuote;
use crate::quotes::cds_tranche::CdsTrancheQuote;
use crate::quotes::ids::{Pillar, QuoteId};
use crate::quotes::inflation::InflationQuote;
use crate::quotes::rates::RateQuote;
use crate::quotes::vol::VolQuote;
use crate::quotes::xccy::XccyQuote;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_core::market_data::surfaces::VolQuoteType;
use finstack_quant_core::types::{IndexId, UnderlyingId};
use finstack_quant_core::{Error, Result};
use finstack_quant_valuations_core::instruments::OptionType;
use finstack_quant_valuations_core::market::conventions::ids::{
    CdsConventionKey, CdsDocClause, InflationSwapConventionId, IrFutureContractId,
    SwaptionConventionId, XccyConventionId,
};
use serde_json::{Map, Value};

/// Step kinds whose produced object is identified by `curve_id`.
const CURVE_ID_KINDS: [&str; 6] = [
    "discount",
    "forward",
    "hazard",
    "inflation",
    "xccy_basis",
    "parametric",
];

/// Step kinds whose produced object is identified by `vol_surface_id`.
const VOL_SURFACE_ID_KINDS: [&str; 3] = ["vol_surface", "swaption_vol", "svi_surface"];

/// Set `key` to `value` when the field is absent or `null`.
fn default_field(fields: &mut Map<String, Value>, key: &str, value: Value) {
    match fields.get(key) {
        Some(existing) if !existing.is_null() => {}
        _ => {
            fields.insert(key.to_string(), value);
        }
    }
}

/// Strictly deserialize a wire object, labelling the failure.
fn from_wire<T: serde::de::DeserializeOwned>(value: Value, label: &str) -> Result<T> {
    serde_json::from_value(value)
        .map_err(|error| Error::Validation(format!("invalid {label}: {error}")))
}

fn parse_cds_convention(currency: &str, doc_clause: &str) -> Result<CdsConventionKey> {
    let currency: Currency = currency.parse()?;
    let doc_clause: CdsDocClause = doc_clause.parse().map_err(Error::Validation)?;
    Ok(CdsConventionKey {
        currency,
        doc_clause,
    })
}

impl RateQuote {
    /// Convexity adjustment (decimal rate) of a futures quote authored without one.
    pub const DEFAULT_CONVEXITY_ADJUSTMENT: f64 = 0.0;

    /// Build a validated money-market deposit quote.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique quote identifier used as the residual key in calibration reports.
    /// * `index` - Rate index identifier (for example `"USD-SOFR-OIS"`).
    /// * `pillar` - Maturity pillar as a tenor or calendar date.
    /// * `rate` - Simple deposit rate as a decimal (`0.0525` for 5.25%).
    ///
    /// # Errors
    ///
    /// Returns a validation error if `rate` is not finite.
    pub fn deposit(
        id: impl Into<QuoteId>,
        index: impl Into<IndexId>,
        pillar: Pillar,
        rate: f64,
    ) -> Result<Self> {
        let quote = Self::Deposit {
            id: id.into(),
            index: index.into(),
            pillar,
            rate,
        };
        quote.validate()?;
        Ok(quote)
    }

    /// Build a validated forward-rate-agreement quote.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique quote identifier used as the residual key in calibration reports.
    /// * `index` - Rate index identifier of the underlying floating rate.
    /// * `start` - Accrual start pillar as a tenor or calendar date.
    /// * `end` - Accrual end pillar as a tenor or calendar date.
    /// * `rate` - Quoted FRA rate as a decimal (`0.045` for 4.5%).
    ///
    /// # Errors
    ///
    /// Returns a validation error if `rate` is not finite.
    pub fn fra(
        id: impl Into<QuoteId>,
        index: impl Into<IndexId>,
        start: Pillar,
        end: Pillar,
        rate: f64,
    ) -> Result<Self> {
        let quote = Self::Fra {
            id: id.into(),
            index: index.into(),
            start,
            end,
            rate,
        };
        quote.validate()?;
        Ok(quote)
    }

    /// Build a validated interest-rate futures price quote.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique quote identifier used as the residual key in calibration reports.
    /// * `contract` - Futures contract identifier (for example `"CME:SR3"`).
    /// * `expiry` - Last trading date of the contract.
    /// * `price` - Quoted futures price (for example `98.50`; implied rate is `(100 - price) / 100`).
    /// * `convexity_adjustment` - Optional convexity adjustment as a decimal rate subtracted
    ///   from the futures-implied forward (Hull convention); `None` uses
    ///   [`Self::DEFAULT_CONVEXITY_ADJUSTMENT`].
    ///
    /// # Errors
    ///
    /// Returns a validation error if `price` or `convexity_adjustment` is not finite.
    pub fn futures(
        id: impl Into<QuoteId>,
        contract: impl Into<IrFutureContractId>,
        expiry: Date,
        price: f64,
        convexity_adjustment: Option<f64>,
    ) -> Result<Self> {
        let quote = Self::Futures {
            id: id.into(),
            contract: contract.into(),
            expiry,
            price,
            convexity_adjustment: convexity_adjustment
                .unwrap_or(Self::DEFAULT_CONVEXITY_ADJUSTMENT),
        };
        quote.validate()?;
        Ok(quote)
    }

    /// Build a validated par interest-rate swap quote.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique quote identifier used as the residual key in calibration reports.
    /// * `index` - Floating-leg rate index identifier (for example `"USD-SOFR-OIS"`).
    /// * `pillar` - Swap maturity pillar as a tenor or calendar date.
    /// * `rate` - Fixed par swap rate as a decimal (`0.045` for 4.5%).
    /// * `spread_decimal` - Optional floating-leg spread as a decimal (`0.0010` for 10 bp).
    ///
    /// # Errors
    ///
    /// Returns a validation error if `rate` or a supplied `spread_decimal` is not finite.
    pub fn swap(
        id: impl Into<QuoteId>,
        index: impl Into<IndexId>,
        pillar: Pillar,
        rate: f64,
        spread_decimal: Option<f64>,
    ) -> Result<Self> {
        let quote = Self::Swap {
            id: id.into(),
            index: index.into(),
            pillar,
            rate,
            spread_decimal,
        };
        quote.validate()?;
        Ok(quote)
    }
}

impl CdsQuote {
    /// Build a validated running par-spread CDS quote.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique quote identifier.
    /// * `entity` - Reference entity name.
    /// * `currency` - ISO-4217 contract currency code of the CDS convention.
    /// * `doc_clause` - ISDA documentation clause (`"isda_na"`, `"isda_eu"`, `"cr14"`, `"mr14"`, `"mm14"`, `"xr14"`, ...).
    /// * `pillar` - Maturity pillar as a tenor or calendar date.
    /// * `spread_bp` - Quoted par spread in basis points (`80.0` for 80 bp).
    /// * `recovery_rate` - Assumed recovery rate as a decimal in `[0, 1)`.
    ///
    /// # Errors
    ///
    /// Returns a validation error if `currency` or `doc_clause` cannot be parsed or the quote fails [`CdsQuote::validate`].
    pub fn par_spread(
        id: impl Into<QuoteId>,
        entity: impl Into<String>,
        currency: &str,
        doc_clause: &str,
        pillar: Pillar,
        spread_bp: f64,
        recovery_rate: f64,
    ) -> Result<Self> {
        let quote = Self::CdsParSpread {
            id: id.into(),
            entity: entity.into(),
            convention: parse_cds_convention(currency, doc_clause)?,
            pillar,
            spread_bp,
            recovery_rate,
        };
        quote.validate()?;
        Ok(quote)
    }

    /// Build a validated upfront-plus-running-coupon CDS quote.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique quote identifier.
    /// * `entity` - Reference entity name.
    /// * `currency` - ISO-4217 contract currency code of the CDS convention.
    /// * `doc_clause` - ISDA documentation clause (`"isda_na"`, `"isda_eu"`, `"cr14"`, `"mr14"`, `"mm14"`, `"xr14"`, ...).
    /// * `pillar` - Maturity pillar as a tenor or calendar date.
    /// * `coupon_bp` - Contractual standard running coupon in basis points (`25.0`, `100.0`, `500.0` or `1000.0`).
    /// * `upfront_pct` - Upfront payment as a decimal fraction of notional (`0.01` for 1%).
    /// * `recovery_rate` - Assumed recovery rate as a decimal in `[0, 1)`.
    ///
    /// # Errors
    ///
    /// Returns a validation error if `currency` or `doc_clause` cannot be parsed or the quote fails [`CdsQuote::validate`].
    #[allow(clippy::too_many_arguments)]
    pub fn upfront(
        id: impl Into<QuoteId>,
        entity: impl Into<String>,
        currency: &str,
        doc_clause: &str,
        pillar: Pillar,
        coupon_bp: f64,
        upfront_pct: f64,
        recovery_rate: f64,
    ) -> Result<Self> {
        let quote = Self::CdsUpfront {
            id: id.into(),
            entity: entity.into(),
            convention: parse_cds_convention(currency, doc_clause)?,
            pillar,
            coupon_bp,
            upfront_pct,
            recovery_rate,
        };
        quote.validate()?;
        Ok(quote)
    }
}

impl VolQuote {
    /// Option type label of an option-vol quote authored without one.
    pub const DEFAULT_OPTION_TYPE: &'static str = "call";
    /// Swaption convention identifier of a swaption quote authored without one.
    pub const DEFAULT_SWAPTION_CONVENTION: &'static str = "USD";
    /// Cap (`true`) or floor (`false`) side of a cap/floor quote authored without one.
    pub const DEFAULT_IS_CAP: bool = true;

    /// Build a validated equity or FX option implied-volatility quote.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique quote identifier.
    /// * `underlying` - Underlying identifier (ticker) the surface is keyed by.
    /// * `expiry` - Option expiry date.
    /// * `strike` - Absolute strike in underlying price units (> 0).
    /// * `vol` - Black implied volatility as an annualized decimal (`0.28` for 28%).
    /// * `option_type` - Optional payoff direction (`"call"` or `"put"`); `None` uses [`Self::DEFAULT_OPTION_TYPE`].
    ///
    /// # Errors
    ///
    /// Returns a validation error if `option_type` is unknown or the quote fails [`VolQuote::validate`].
    pub fn option_vol(
        id: impl Into<QuoteId>,
        underlying: impl Into<UnderlyingId>,
        expiry: Date,
        strike: f64,
        vol: f64,
        option_type: Option<&str>,
    ) -> Result<Self> {
        let option_type: OptionType = option_type
            .unwrap_or(Self::DEFAULT_OPTION_TYPE)
            .parse()
            .map_err(Error::Validation)?;
        let quote = Self::OptionVol {
            id: id.into(),
            underlying: underlying.into(),
            expiry,
            strike,
            vol,
            option_type,
        };
        quote.validate()?;
        Ok(quote)
    }

    /// Build a validated swaption implied-volatility quote.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique quote identifier.
    /// * `expiry` - Option expiry date.
    /// * `maturity` - Underlying swap maturity date (must be after `expiry`).
    /// * `strike` - Fixed strike rate as a decimal.
    /// * `vol` - Implied volatility in canonical decimal units (absolute rate volatility for `"normal"`, relative volatility for `"black_lognormal"`).
    /// * `quote_type` - Volatility quoting convention (`"normal"`, `"black_lognormal"`, or `"shifted_black_lognormal"`).
    /// * `convention` - Optional swaption market convention identifier; `None` uses [`Self::DEFAULT_SWAPTION_CONVENTION`].
    ///
    /// # Errors
    ///
    /// Returns a validation error if `quote_type` is unknown or the quote fails [`VolQuote::validate`].
    pub fn swaption_vol(
        id: impl Into<QuoteId>,
        expiry: Date,
        maturity: Date,
        strike: f64,
        vol: f64,
        quote_type: &str,
        convention: Option<&str>,
    ) -> Result<Self> {
        let quote_type: VolQuoteType = quote_type.parse()?;
        let convention =
            SwaptionConventionId::new(convention.unwrap_or(Self::DEFAULT_SWAPTION_CONVENTION));
        let quote = Self::SwaptionVol {
            id: id.into(),
            expiry,
            maturity,
            strike,
            vol,
            quote_type,
            convention,
        };
        quote.validate()?;
        Ok(quote)
    }

    /// Build a validated cap or floor implied-volatility quote.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique quote identifier.
    /// * `expiry` - Cap/floor maturity or caplet expiry date.
    /// * `strike` - Strike rate as a decimal.
    /// * `vol` - Implied volatility in canonical decimal units (absolute rate volatility for `"normal"`, relative volatility for `"black_lognormal"`).
    /// * `quote_type` - Volatility quoting convention (`"normal"`, `"black_lognormal"`, or `"shifted_black_lognormal"`).
    /// * `is_cap` - Optional cap (`true`) or floor (`false`) indicator; `None` uses [`Self::DEFAULT_IS_CAP`].
    ///
    /// # Errors
    ///
    /// Returns a validation error if `quote_type` is unknown or the quote fails [`VolQuote::validate`].
    pub fn cap_floor_vol(
        id: impl Into<QuoteId>,
        expiry: Date,
        strike: f64,
        vol: f64,
        quote_type: &str,
        is_cap: Option<bool>,
    ) -> Result<Self> {
        let quote_type: VolQuoteType = quote_type.parse()?;
        let quote = Self::CapFloorVol {
            id: id.into(),
            expiry,
            strike,
            vol,
            quote_type,
            is_cap: is_cap.unwrap_or(Self::DEFAULT_IS_CAP),
        };
        quote.validate()?;
        Ok(quote)
    }
}

impl InflationQuote {
    /// Build a validated zero-coupon inflation swap quote.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique quote identifier.
    /// * `maturity` - Swap maturity date.
    /// * `rate` - Fixed zero-coupon swap rate as a decimal (`0.025` for 2.5%).
    /// * `index` - Inflation index identifier (for example `"USA-CPI-U"`).
    /// * `convention` - Inflation-swap convention identifier (for example `"USD"`).
    ///
    /// # Errors
    ///
    /// Returns a validation error if `rate` is not finite.
    pub fn inflation_swap(
        id: impl Into<QuoteId>,
        maturity: Date,
        rate: f64,
        index: impl Into<String>,
        convention: impl Into<InflationSwapConventionId>,
    ) -> Result<Self> {
        let quote = Self::InflationSwap {
            id: id.into(),
            maturity,
            rate,
            index: index.into(),
            convention: convention.into(),
        };
        quote.validate()?;
        Ok(quote)
    }

    /// Build a validated year-on-year inflation swap quote.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique quote identifier.
    /// * `maturity` - Swap maturity date.
    /// * `rate` - Fixed year-on-year swap rate as a decimal (`0.025` for 2.5%).
    /// * `index` - Inflation index identifier (for example `"USA-CPI-U"`).
    /// * `frequency` - Payment frequency tenor string (for example `"1Y"`).
    /// * `convention` - Inflation-swap convention identifier (for example `"USD"`).
    ///
    /// # Errors
    ///
    /// Returns a validation error if `frequency` cannot be parsed as a [`Tenor`] or `rate` is not finite.
    pub fn yoy_inflation_swap(
        id: impl Into<QuoteId>,
        maturity: Date,
        rate: f64,
        index: impl Into<String>,
        frequency: &str,
        convention: impl Into<InflationSwapConventionId>,
    ) -> Result<Self> {
        let frequency: Tenor = frequency.parse()?;
        let quote = Self::YoYInflationSwap {
            id: id.into(),
            maturity,
            rate,
            index: index.into(),
            frequency,
            convention: convention.into(),
        };
        quote.validate()?;
        Ok(quote)
    }
}

impl XccyQuote {
    /// Build a validated cross-currency basis-swap quote.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique quote identifier.
    /// * `convention` - Cross-currency swap convention identifier (for example `"EUR-USD"`).
    /// * `far_pillar` - Far-leg maturity pillar as a tenor or calendar date.
    /// * `basis_spread_bp` - Basis spread in basis points on the base-currency leg.
    /// * `spot_fx` - Optional spot FX rate override (quote currency per 1 unit of base currency, > 0).
    ///
    /// # Errors
    ///
    /// Returns a validation error if `basis_spread_bp` is not finite or `spot_fx` is not positive.
    pub fn new(
        id: impl Into<QuoteId>,
        convention: impl Into<XccyConventionId>,
        far_pillar: Pillar,
        basis_spread_bp: f64,
        spot_fx: Option<f64>,
    ) -> Result<Self> {
        let quote = Self {
            id: id.into(),
            convention: convention.into(),
            far_pillar,
            basis_spread_bp,
            spot_fx,
        };
        quote.validate()?;
        Ok(quote)
    }
}

impl CdsTrancheQuote {
    /// Build a validated CDS index tranche quote.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique quote identifier.
    /// * `index` - Credit index name (for example `"CDX.NA.IG"`).
    /// * `series` - CDS index series number.
    /// * `attachment` - Tranche attachment point as a decimal loss fraction in `[0, 1]`.
    /// * `detachment` - Tranche detachment point as a decimal loss fraction in `[0, 1]`, strictly above `attachment`.
    /// * `maturity` - Tranche maturity date.
    /// * `upfront_pct` - Upfront payment as a decimal fraction of tranche notional (`|upfront_pct| <= 1`).
    /// * `coupon_bp` - Contractual running coupon in basis points (> 0).
    /// * `currency` - ISO-4217 contract currency code of the CDS convention.
    /// * `doc_clause` - ISDA documentation clause (`"isda_na"`, `"cr14"`, ...).
    ///
    /// # Errors
    ///
    /// Returns a validation error if `currency` or `doc_clause` cannot be parsed or the quote fails [`CdsTrancheQuote::validate`].
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<QuoteId>,
        index: impl Into<String>,
        series: u16,
        attachment: f64,
        detachment: f64,
        maturity: Date,
        upfront_pct: f64,
        coupon_bp: f64,
        currency: &str,
        doc_clause: &str,
    ) -> Result<Self> {
        let quote = Self {
            id: id.into(),
            index: index.into(),
            series,
            attachment,
            detachment,
            maturity,
            upfront_pct,
            coupon_bp,
            convention: parse_cds_convention(currency, doc_clause)?,
        };
        quote.validate()?;
        Ok(quote)
    }
}

impl CalibrationStep {
    /// Seniority label of a hazard step authored without one.
    pub const DEFAULT_HAZARD_SENIORITY: &'static str = "senior";
    /// Parametric family label of a `parametric` step authored without one.
    pub const DEFAULT_PARAMETRIC_MODEL: &'static str = "ns";

    /// Build a calibration step from its kind label and serde wire fields.
    ///
    /// # Arguments
    ///
    /// * `kind` - Step kind label (`"discount"`, `"forward"`, `"hazard"`,
    ///   `"inflation"`, `"vol_surface"`, `"swaption_vol"`,
    ///   `"base_correlation"`, `"student_t"`, `"hull_white"`,
    ///   `"cap_floor_hull_white"`, `"svi_surface"`, `"xccy_basis"` or
    ///   `"parametric"`). It becomes the `kind` discriminator, replacing any
    ///   `kind` entry in `fields`.
    /// * `id` - Step identifier. It replaces any `id` entry in `fields` and is
    ///   the fallback for the quote-set name and the produced object's
    ///   identifier.
    /// * `quote_set` - Name of the quote set in the parent plan's
    ///   `quote_sets`; `None` uses `id`.
    /// * `fields` - Kind-specific wire fields of the matching `StepParams`
    ///   variant (dates as ISO-8601 strings, currencies as ISO-4217 codes).
    ///   Omitted or `null` entries take the authoring defaults: `curve_id`
    ///   (curve kinds) or `vol_surface_id` (surface kinds) is `id`, a hazard
    ///   `seniority` is [`Self::DEFAULT_HAZARD_SENIORITY`], a `parametric`
    ///   `model` is [`Self::DEFAULT_PARAMETRIC_MODEL`]. Remaining optional
    ///   fields keep their serde defaults; unknown fields are rejected.
    ///
    /// # Errors
    ///
    /// Returns a validation error for an unknown `kind` or a missing, unknown
    /// or mistyped field.
    pub fn from_wire_fields(
        kind: &str,
        id: &str,
        quote_set: Option<&str>,
        mut fields: Map<String, Value>,
    ) -> Result<Self> {
        if CURVE_ID_KINDS.contains(&kind) {
            default_field(&mut fields, "curve_id", Value::from(id));
        }
        if VOL_SURFACE_ID_KINDS.contains(&kind) {
            default_field(&mut fields, "vol_surface_id", Value::from(id));
        }
        match kind {
            "hazard" => default_field(
                &mut fields,
                "seniority",
                Value::from(Self::DEFAULT_HAZARD_SENIORITY),
            ),
            "parametric" => default_field(
                &mut fields,
                "model",
                Value::from(Self::DEFAULT_PARAMETRIC_MODEL),
            ),
            _ => {}
        }
        fields.insert("kind".to_string(), Value::String(kind.to_string()));
        fields.insert("id".to_string(), Value::String(id.to_string()));
        fields.insert(
            "quote_set".to_string(),
            Value::String(quote_set.unwrap_or(id).to_string()),
        );
        from_wire(Value::Object(fields), &format!("{kind} step"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::schema::StepParams;
    use serde_json::json;
    use time::macros::date;

    fn map(value: Value) -> Map<String, Value> {
        match value {
            Value::Object(map) => map,
            _ => Map::new(),
        }
    }

    #[test]
    fn rate_quote_constructors_build_and_validate() {
        let deposit =
            RateQuote::deposit("D3M", "USD-SOFR-OIS", "3M".parse().expect("tenor"), 0.052)
                .expect("deposit");
        let wire = serde_json::to_value(&deposit).expect("wire");
        assert_eq!(wire["type"], "deposit");
        assert_eq!(
            wire["pillar"],
            serde_json::to_value("3M".parse::<Pillar>().expect("tenor")).expect("pillar")
        );

        let dated = RateQuote::swap(
            "S",
            "USD-SOFR-OIS",
            "2030-06-20".parse().expect("date pillar"),
            0.04,
            None,
        )
        .expect("swap");
        assert_eq!(
            serde_json::to_value(&dated).expect("wire")["pillar"]["date"],
            "2030-06-20"
        );

        let futures =
            RateQuote::futures("F", "CME:SR3", date!(2026 - 09 - 15), 96.5, None).expect("futures");
        assert_eq!(
            serde_json::to_value(&futures).expect("wire")["convexity_adjustment"],
            0.0
        );

        assert!(RateQuote::deposit("D", "X", "3M".parse().expect("tenor"), f64::NAN).is_err());
        assert!(
            RateQuote::swap("S", "X", "3M".parse().expect("tenor"), 0.04, Some(f64::NAN)).is_err()
        );
    }

    #[test]
    fn cds_quote_builds_both_variants() {
        let par = CdsQuote::par_spread(
            "ACME-5Y",
            "ACME",
            "USD",
            "isda_na",
            "5Y".parse().expect("tenor"),
            80.0,
            0.4,
        )
        .expect("par spread");
        assert_eq!(par.coupon_bp(), 80.0);

        let upfront = CdsQuote::upfront(
            "ACME-5Y-U",
            "ACME",
            "USD",
            "isda_na",
            "5Y".parse().expect("tenor"),
            100.0,
            0.01,
            0.4,
        )
        .expect("upfront");
        assert_eq!(upfront.coupon_bp(), 100.0);
        assert!(CdsQuote::par_spread(
            "ACME-5Y",
            "ACME",
            "USD",
            "bogus_clause",
            "5Y".parse().expect("tenor"),
            80.0,
            0.4,
        )
        .is_err());
    }

    #[test]
    fn vol_quote_applies_authoring_defaults() {
        let option = VolQuote::option_vol("O", "AAPL", date!(2027 - 05 - 08), 155.0, 0.28, None)
            .expect("option vol");
        assert_eq!(
            serde_json::to_value(&option).expect("wire")["option_vol"]["option_type"],
            "call"
        );

        let swaption = VolQuote::swaption_vol(
            "S",
            date!(2027 - 05 - 08),
            date!(2032 - 05 - 08),
            0.04,
            0.007,
            "normal",
            None,
        )
        .expect("swaption vol");
        let wire = serde_json::to_value(&swaption).expect("wire");
        assert_eq!(wire["swaption_vol"]["quote_type"], "normal");
        assert_eq!(wire["swaption_vol"]["convention"], "USD");

        let floor = VolQuote::cap_floor_vol(
            "C",
            date!(2027 - 05 - 08),
            0.04,
            0.20,
            "black_lognormal",
            Some(false),
        )
        .expect("floor vol");
        let wire = serde_json::to_value(&floor).expect("wire");
        assert_eq!(wire["cap_floor_vol"]["is_cap"], false);
        assert_eq!(wire["cap_floor_vol"]["quote_type"], "black_lognormal");

        assert!(VolQuote::swaption_vol(
            "Q",
            date!(2027 - 05 - 08),
            date!(2032 - 05 - 08),
            0.04,
            0.2,
            "bogus",
            None,
        )
        .is_err());
    }

    #[test]
    fn inflation_xccy_and_tranche_quotes_build_and_validate() {
        let zc = InflationQuote::inflation_swap(
            "ZC5Y",
            date!(2031 - 05 - 08),
            0.025,
            "USA-CPI-U",
            "USD",
        )
        .expect("zero-coupon inflation swap");
        assert_eq!(zc.id().as_str(), "ZC5Y");

        let yoy = InflationQuote::yoy_inflation_swap(
            "YOY5Y",
            date!(2031 - 05 - 08),
            0.025,
            "USA-CPI-U",
            "1Y",
            "USD",
        )
        .expect("yoy inflation swap");
        assert!(serde_json::to_value(&yoy).expect("wire")["yoy_inflation_swap"].is_object());

        let xccy = XccyQuote::new("X5Y", "EUR-USD", "5Y".parse().expect("tenor"), -12.5, None)
            .expect("xccy quote");
        assert_eq!(
            serde_json::to_value(&xccy.far_pillar).expect("pillar"),
            serde_json::to_value("5Y".parse::<Pillar>().expect("tenor")).expect("pillar")
        );
        assert!(XccyQuote::new(
            "X",
            "EUR-USD",
            "5Y".parse().expect("tenor"),
            1.0,
            Some(-1.0),
        )
        .is_err());

        let tranche = |attachment: f64| {
            CdsTrancheQuote::new(
                "IG-3-7",
                "CDX.NA.IG",
                42,
                attachment,
                0.07,
                date!(2031 - 06 - 20),
                0.01,
                100.0,
                "USD",
                "isda_na",
            )
        };
        assert!(tranche(0.03).is_ok());
        assert!(
            tranche(0.08).is_err(),
            "attachment above detachment must be rejected"
        );
    }

    #[test]
    fn step_defaults_identifiers_from_the_step_id() {
        let step = CalibrationStep::from_wire_fields(
            "discount",
            "USD-OIS",
            None,
            map(json!({"currency": "USD", "base_date": "2026-05-08", "curve_id": null})),
        )
        .expect("discount step");
        assert_eq!(step.id, "USD-OIS");
        assert_eq!(step.quote_set, "USD-OIS");
        let StepParams::Discount(params) = &step.params else {
            panic!("expected a discount step");
        };
        assert_eq!(params.curve_id.as_str(), "USD-OIS");

        let named = CalibrationStep::from_wire_fields(
            "discount",
            "step",
            Some("quotes"),
            map(json!({"currency": "USD", "base_date": "2026-05-08", "curve_id": "USD-OIS"})),
        )
        .expect("named discount step");
        assert_eq!(named.quote_set, "quotes");
        let wire = serde_json::to_value(&named).expect("wire");
        assert_eq!(wire["curve_id"], "USD-OIS");
        assert_eq!(wire["kind"], "discount");
    }

    #[test]
    fn step_defaults_labels_per_kind() {
        let hazard = CalibrationStep::from_wire_fields(
            "hazard",
            "ACME",
            None,
            map(
                json!({"entity": "ACME", "currency": "USD", "base_date": "2026-05-08",
                       "discount_curve_id": "USD-OIS", "recovery_rate": 0.4}),
            ),
        )
        .expect("hazard step");
        assert_eq!(
            serde_json::to_value(&hazard).expect("wire")["seniority"],
            "senior"
        );

        let surface = CalibrationStep::from_wire_fields(
            "vol_surface",
            "AAPL-VOL",
            None,
            map(json!({"base_date": "2026-05-08", "underlying_ticker": "AAPL"})),
        )
        .expect("vol surface step");
        let wire = serde_json::to_value(&surface).expect("wire");
        assert!(wire.get("model").is_none());
        assert_eq!(wire["vol_surface_id"], "AAPL-VOL");

        let parametric = CalibrationStep::from_wire_fields(
            "parametric",
            "USD-NS",
            None,
            map(json!({"base_date": "2026-05-08"})),
        )
        .expect("parametric step");
        let wire = serde_json::to_value(&parametric).expect("wire");
        assert_eq!(wire["model"], "ns");
        assert_eq!(wire["curve_id"], "USD-NS");
    }

    #[test]
    fn step_rejects_unknown_kinds_and_fields() {
        assert!(CalibrationStep::from_wire_fields("bogus", "x", None, Map::new()).is_err());
        let error = CalibrationStep::from_wire_fields(
            "discount",
            "USD-OIS",
            None,
            map(json!({"currency": "USD", "base_date": "2026-05-08", "bogus": 1})),
        )
        .expect_err("unknown field");
        assert!(error.to_string().contains("invalid discount step"));
    }
}
