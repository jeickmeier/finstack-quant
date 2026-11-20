//! Host-facing authoring constructors for quotes and calibration steps.
//!
//! The Python and WASM bindings expose one named constructor per quote type
//! and per step kind (`RateQuote.deposit`, `CalibrationStep.discount`, ...).
//! Quote constructors call the typed constructors here directly without a JSON
//! round-trip; step constructors pass the host's wire-field object to
//! [`CalibrationStep::from_wire_fields`]. Quote types whose constructor would
//! only restate the struct (`CdsQuote::CdsUpfront`, [`crate::quotes::CdsTrancheQuote`]) are
//! built as literals (or deserialized) and checked with their `validate()`.

use crate::api::market_datum::MarketDatum;
use crate::api::prior_market::PriorMarketObject;
use crate::api::schema::{CalibrationEnvelope, CalibrationPlan, CalibrationStep, StepWithQuotes};
use crate::config::CalibrationConfig;
use crate::quotes::CdsQuote;
use crate::quotes::InflationQuote;
use crate::quotes::RateQuote;
use crate::quotes::VolQuote;
use crate::quotes::XccyQuote;
use crate::quotes::{Pillar, QuoteId};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_core::market_data::surfaces::VolQuoteType;
use finstack_quant_core::types::{IndexId, UnderlyingId};
use finstack_quant_core::{Error, Result};
use finstack_quant_models::types::OptionType;
use finstack_quant_valuations_core::market::conventions::ids::{
    CdsConventionKey, CdsDocClause, InflationSwapConventionId, IrFutureContractId,
    SwaptionConventionId, XccyConventionId,
};
use indexmap::IndexMap;
use serde_json::{Map, Value};
use std::collections::HashSet;

impl CalibrationEnvelope {
    /// Author a calibration envelope from ordered steps with inline inputs.
    ///
    /// Each quote appears once in the envelope. A later step can reference an
    /// already registered quote set by supplying the same set identifier and
    /// an empty quote list. The normal calibration validator checks that each
    /// step receives the required market inputs and dependencies.
    ///
    /// # Arguments
    /// * `id` - Nonempty identifier for the new calibration plan.
    /// * `steps` - Ordered native steps with inline market inputs.
    /// * `settings` - Global solver and validation policy for the plan.
    /// * `prior_market` - Previously calibrated objects available as dependencies.
    ///
    /// # Errors
    /// Rejects blank plan, step, set or input identifiers, duplicate steps or
    /// market input identifiers, and duplicate definitions of a quote set.
    pub fn from_steps(
        id: impl Into<String>,
        steps: Vec<StepWithQuotes>,
        settings: CalibrationConfig,
        prior_market: Vec<PriorMarketObject>,
    ) -> Result<Self> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(Error::Validation(
                "Calibration plan id must be nonempty".into(),
            ));
        }
        let mut plan = CalibrationPlan {
            id,
            description: None,
            quote_sets: IndexMap::new(),
            steps: Vec::with_capacity(steps.len()),
            settings,
        };
        let mut step_ids = HashSet::new();
        let mut datum_ids = HashSet::new();
        let mut market_data = Vec::new();
        for entry in steps {
            append_step(
                &mut plan,
                &mut market_data,
                &mut step_ids,
                &mut datum_ids,
                entry,
            )?;
        }
        Ok(Self::new(plan, market_data, prior_market))
    }

    /// Derive an envelope with one more step at the end of the execution order.
    ///
    /// # Arguments
    /// * `entry` - Step and inline market inputs. An empty list references an
    ///   existing quote set when the step names that set.
    ///
    /// # Errors
    /// Rejects blank or duplicate step and quote identifiers, and duplicate
    /// quote-set definitions. The original envelope stays available on error.
    pub fn with_step(&self, entry: StepWithQuotes) -> Result<Self> {
        let mut next = self.clone();
        let mut step_ids = next.plan.steps.iter().map(|step| step.id.clone()).collect();
        let mut datum_ids = next
            .market_data
            .iter()
            .map(|datum| datum.id().to_owned())
            .collect();
        append_step(
            &mut next.plan,
            &mut next.market_data,
            &mut step_ids,
            &mut datum_ids,
            entry,
        )?;
        Ok(next)
    }

    /// Derive an envelope with one quoted observation and all its set references removed.
    ///
    /// # Arguments
    /// * `quote_id` - Stable identifier of a calibration quote, not a market snapshot.
    ///
    /// # Errors
    /// Rejects unknown quote IDs and IDs that refer to non-quote market data.
    pub fn without_quote(&self, quote_id: &str) -> Result<Self> {
        let mut next = self.clone();
        let index = next
            .market_data
            .iter()
            .position(|datum| datum.id() == quote_id && datum.is_quote())
            .ok_or_else(|| Error::Validation(format!("Unknown calibration quote '{quote_id}'")))?;
        next.market_data.remove(index);
        for ids in next.plan.quote_sets.values_mut() {
            ids.retain(|id| id.as_str() != quote_id);
        }
        Ok(next)
    }

    /// Derive an envelope with an additive rate or spread shock on selected quotes.
    ///
    /// # Arguments
    /// * `quote_set` - Optional named set. An omitted set selects every quote once,
    ///   excluding independent observations such as fixings and spot prices.
    /// * `bp` - Finite signed basis points. One basis point adds 0.0001 to decimal
    ///   rates, subtracts 0.01 from rate-futures prices, or adds 1 to spreads
    ///   expressed in basis points.
    ///
    /// # Errors
    /// Rejects unknown sets, unresolved set references, invalid bumped quotes,
    /// and selected quotes without a rate or spread basis, such as volatility,
    /// upfront CDS and tranche quotes.
    pub fn bump_quotes_bp(&self, quote_set: Option<&str>, bp: f64) -> Result<Self> {
        if !bp.is_finite() {
            return Err(Error::Validation(
                "Calibration quote bump must be finite".into(),
            ));
        }
        let mut next = self.clone();
        if let Some(name) = quote_set {
            let ids = self.plan.quote_sets.get(name).ok_or_else(|| {
                Error::Validation(format!("Unknown calibration quote set '{name}'"))
            })?;
            let mut pending: HashSet<_> = ids.iter().map(QuoteId::as_str).collect();
            for datum in &mut next.market_data {
                if pending.remove(datum.id()) {
                    bump_quote(datum, bp)?;
                }
            }
            if !pending.is_empty() {
                return Err(Error::Validation(format!(
                    "Unresolved quote in set '{name}'"
                )));
            }
        } else {
            for datum in &mut next.market_data {
                if datum.is_quote() {
                    bump_quote(datum, bp)?;
                }
            }
        }
        Ok(next)
    }
}

fn bump_quote(datum: &mut MarketDatum, bp: f64) -> Result<()> {
    match datum {
        MarketDatum::RateQuote(quote) => {
            *quote = quote.bump_rate_bp(bp);
            quote.validate()
        }
        MarketDatum::CdsQuote(quote @ CdsQuote::CdsParSpread { .. }) => {
            if let CdsQuote::CdsParSpread { spread_bp, .. } = quote {
                *spread_bp += bp;
            }
            quote.validate()
        }
        MarketDatum::InflationQuote(quote) => {
            match quote {
                InflationQuote::InflationSwap { rate, .. }
                | InflationQuote::YoYInflationSwap { rate, .. } => *rate += bp / 10_000.0,
            }
            quote.validate()
        }
        MarketDatum::XccyQuote(quote) => {
            quote.basis_spread_bp += bp;
            quote.validate()
        }
        _ => Err(Error::Validation(format!(
            "Calibration input '{}' does not have a rate or spread bump basis",
            datum.id()
        ))),
    }
}

fn append_step(
    plan: &mut CalibrationPlan,
    market_data: &mut Vec<MarketDatum>,
    step_ids: &mut HashSet<String>,
    datum_ids: &mut HashSet<String>,
    entry: StepWithQuotes,
) -> Result<()> {
    let StepWithQuotes { step, quotes } = entry;
    if step.id.trim().is_empty() || step.quote_set.trim().is_empty() {
        return Err(Error::Validation(
            "Calibration steps require nonempty step and quote set ids".into(),
        ));
    }
    if !step_ids.insert(step.id.clone()) {
        return Err(Error::Validation(format!(
            "Duplicate calibration step '{}'",
            step.id
        )));
    }
    if !quotes.is_empty() || !plan.quote_sets.contains_key(&step.quote_set) {
        if plan.quote_sets.contains_key(&step.quote_set) {
            return Err(Error::Validation(format!(
                "Duplicate definition of calibration quote set '{}'",
                step.quote_set
            )));
        }
        let mut ids = Vec::with_capacity(quotes.len());
        for quote in quotes {
            let id = quote.id();
            if id.trim().is_empty() || !datum_ids.insert(id.to_owned()) {
                return Err(Error::Validation(format!(
                    "Invalid or duplicate calibration market input '{id}'"
                )));
            }
            ids.push(QuoteId::from(id));
            market_data.push(quote);
        }
        plan.quote_sets.insert(step.quote_set.clone(), ids);
    }
    plan.steps.push(step);
    Ok(())
}

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

impl CalibrationStep {
    /// Build a calibration step from its native, kind-specific parameters.
    ///
    /// # Arguments
    /// * `id` - Nonempty stable identifier for step selection and fit reporting.
    /// * `params` - Native curve, surface or model parameters including explicit output identifiers.
    /// * `quote_set` - Optional nonempty named quote set. Omit it to use the step ID.
    ///
    /// # Errors
    /// Rejects blank step or quote-set identifiers. Calibration validates
    /// dependencies and financial parameters against its quote universe.
    pub fn new(
        id: &str,
        params: crate::api::schema::StepParams,
        quote_set: Option<&str>,
    ) -> Result<Self> {
        let quote_set = quote_set.unwrap_or(id);
        if id.trim().is_empty() || quote_set.trim().is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "Calibration step and quote-set IDs must be nonempty".into(),
            ));
        }
        Ok(Self {
            id: id.into(),
            params,
            quote_set: quote_set.into(),
        })
    }

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
    use crate::quotes::CdsTrancheQuote;
    use finstack_quant_core::currency::Currency;
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

        let upfront = CdsQuote::CdsUpfront {
            id: "ACME-5Y-U".into(),
            entity: "ACME".into(),
            convention: CdsConventionKey {
                currency: Currency::USD,
                doc_clause: CdsDocClause::IsdaNa,
            },
            pillar: "5Y".parse().expect("tenor"),
            coupon_bp: 100.0,
            upfront_pct: 0.01,
            upfront_basis: crate::quotes::CdsUpfrontBasis::Clean,
            recovery_rate: 0.4,
        };
        upfront.validate().expect("upfront");
        assert_eq!(upfront.coupon_bp(), 100.0);
        assert!(
            CdsQuote::par_spread(
                "ACME-5Y",
                "ACME",
                "USD",
                "bogus_clause",
                "5Y".parse().expect("tenor"),
                80.0,
                0.4,
            )
            .is_err()
        );
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

        assert!(
            VolQuote::swaption_vol(
                "Q",
                date!(2027 - 05 - 08),
                date!(2032 - 05 - 08),
                0.04,
                0.2,
                "bogus",
                None,
            )
            .is_err()
        );
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
        assert!(
            XccyQuote::new(
                "X",
                "EUR-USD",
                "5Y".parse().expect("tenor"),
                1.0,
                Some(-1.0),
            )
            .is_err()
        );

        let tranche = |attachment: f64| CdsTrancheQuote {
            id: "IG-3-7".into(),
            index: "CDX.NA.IG".into(),
            series: 42,
            attachment,
            detachment: 0.07,
            maturity: date!(2031 - 06 - 20),
            upfront_pct: 0.01,
            coupon_bp: 100.0,
            convention: CdsConventionKey {
                currency: Currency::USD,
                doc_clause: CdsDocClause::IsdaNa,
            },
        };
        assert!(tranche(0.03).validate().is_ok());
        assert!(
            tranche(0.08).validate().is_err(),
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
