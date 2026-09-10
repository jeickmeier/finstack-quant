//! SIMM risk classification and sensitivity types.
//!
//! Types for ISDA SIMM categorization and sensitivity inputs,
//! used by the [`Marginable`](crate::traits::Marginable) trait
//! and SIMM calculator.

use super::SimmCurvatureSensitivity;
use core::hash::Hash;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::HashMap;

/// The twelve signed-amount maps of [`SimmSensitivities`], listed once so
/// `is_empty`, `merge`, and `scale_amounts` walk them in one place.
macro_rules! amount_maps {
    (@fields $($args:tt)*) => {
        amount_maps!($($args)*;
            ir_delta, ir_vega,
            credit_qualifying_delta, credit_qualifying_vega,
            credit_non_qualifying_delta, credit_non_qualifying_vega,
            equity_delta, equity_vega,
            fx_delta, fx_vega,
            commodity_delta, commodity_vega)
    };
    (is_empty $s:ident) => { amount_maps!(@fields @is_empty $s) };
    (@is_empty $s:ident; $($f:ident),*) => { true $(&& $s.$f.is_empty())* };
    (merge $s:ident, $other:ident) => { amount_maps!(@fields @merge $s, $other) };
    (@merge $s:ident, $other:ident; $($f:ident),*) => { $( merge_into(&mut $s.$f, &$other.$f); )* };
    (scale $s:ident, $factor:ident) => { amount_maps!(@fields @scale $s, $factor) };
    (@scale $s:ident, $factor:ident; $($f:ident),*) => {
        $( for v in $s.$f.values_mut() { *v *= $factor; } )*
    };
    (non_finite $s:ident) => { amount_maps!(@fields @non_finite $s) };
    (@non_finite $s:ident; $($f:ident),*) => {
        None::<&'static str>
            $( .or_else(|| (!$s.$f.values().all(|v| v.is_finite())).then_some(stringify!($f))) )*
    };
}

/// ISDA SIMM tenor bucket labels, in ascending maturity order.
///
/// Every interest-rate, credit-qualifying and credit-non-qualifying tenor
/// key must be one of these labels; [`SimmSensitivities::validate`] rejects
/// anything else so a mistyped tenor cannot silently price to zero margin.
/// The registry-backed SIMM parameter tables are keyed on exactly this set.
pub const SIMM_TENORS: &[&str] = &[
    "2W", "1M", "3M", "6M", "1Y", "2Y", "3Y", "5Y", "10Y", "15Y", "20Y", "30Y",
];

/// Number of ISDA SIMM commodity buckets (bucket ids `1..=17`).
pub const SIMM_COMMODITY_BUCKET_COUNT: u8 = 17;

/// Resolve a SIMM commodity bucket label to its 1-based bucket id.
///
/// Accepts the numeric id (`"3"`) or the ISDA bucket name in any
/// punctuation/case (`"Light Ends"`, `"light_ends"`); returns `None` for an
/// unknown label.
///
/// # Arguments
///
/// * `bucket` - Commodity bucket label as supplied to
///   [`SimmSensitivities::add_commodity_delta`]; surrounding whitespace is
///   ignored.
#[must_use]
pub fn commodity_bucket_id(bucket: &str) -> Option<u8> {
    let trimmed = bucket.trim();
    if let Ok(value) = trimmed.parse::<u8>() {
        return (1..=SIMM_COMMODITY_BUCKET_COUNT)
            .contains(&value)
            .then_some(value);
    }
    let normalized: String = trimmed
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    match normalized.as_str() {
        "coal" => Some(1),
        "crude" => Some(2),
        "lightends" => Some(3),
        "middledistillates" => Some(4),
        "heavydistillates" => Some(5),
        "northamericannaturalgas" => Some(6),
        "europeannaturalgas" => Some(7),
        "northamericanpowerandcarbon" => Some(8),
        "europeanpowerandcarbon" => Some(9),
        "freight" => Some(10),
        "basemetals" => Some(11),
        "preciousmetals" => Some(12),
        "grainsandoilseed" => Some(13),
        "softsandotheragriculturals" => Some(14),
        "livestockanddairy" => Some(15),
        "other" => Some(16),
        "indexes" | "indices" => Some(17),
        _ => None,
    }
}

fn merge_into<K>(target: &mut HashMap<K, f64>, source: &HashMap<K, f64>)
where
    K: Eq + Hash + Clone,
{
    for (key, &value) in source {
        *target.entry(key.clone()).or_insert(0.0) += value;
    }
}

/// Risk classes for SIMM categorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SimmRiskClass {
    /// Interest rate risk
    InterestRate,
    /// Credit qualifying (corporate, sovereign, and index credit, including high yield)
    CreditQualifying,
    /// Credit non-qualifying (securitizations and designated non-qualifying exposures)
    CreditNonQualifying,
    /// Equity risk
    Equity,
    /// Commodity risk
    Commodity,
    /// Foreign exchange risk
    Fx,
}

/// SIMM credit sector for bucket assignment.
///
/// Maps reference entities to ISDA SIMM credit qualifying buckets.
/// See ISDA SIMM v2.6 Table 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SimmCreditSector {
    /// Bucket 1: IG sovereigns including central banks.
    Sovereign,
    /// Bucket 2: IG financials including government-backed financials.
    Financial,
    /// Bucket 3: IG basic materials, energy, and industrials.
    BasicMaterials,
    /// Bucket 4: IG consumer goods and services.
    ConsumerGoods,
    /// Bucket 5: IG technology, telecommunications.
    TechnologyMedia,
    /// Bucket 6: IG health care, utilities, local government, and government-backed corporates.
    HealthCare,
    /// Bucket 7: HY / non-rated sovereigns including central banks.
    HighYieldSovereign,
    /// Bucket 8: HY / non-rated financials including government-backed financials.
    HighYieldFinancial,
    /// Bucket 9: HY / non-rated basic materials, energy, and industrials.
    HighYieldBasicMaterials,
    /// Bucket 10: HY / non-rated consumer goods and services.
    HighYieldConsumerGoods,
    /// Bucket 11: HY / non-rated technology, telecommunications.
    HighYieldTechnologyMedia,
    /// Bucket 12: HY / non-rated health care, utilities, local government, and government-backed corporates.
    HighYieldHealthCare,
    /// Residual bucket.
    Residual,
}

impl std::fmt::Display for SimmRiskClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimmRiskClass::InterestRate => write!(f, "interest_rate"),
            SimmRiskClass::CreditQualifying => write!(f, "credit_qualifying"),
            SimmRiskClass::CreditNonQualifying => write!(f, "credit_non_qualifying"),
            SimmRiskClass::Equity => write!(f, "equity"),
            SimmRiskClass::Commodity => write!(f, "commodity"),
            SimmRiskClass::Fx => write!(f, "fx"),
        }
    }
}

impl std::str::FromStr for SimmRiskClass {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw {
            "interest_rate" => Ok(Self::InterestRate),
            "credit_qualifying" => Ok(Self::CreditQualifying),
            "credit_non_qualifying" => Ok(Self::CreditNonQualifying),
            "equity" => Ok(Self::Equity),
            "commodity" => Ok(Self::Commodity),
            "fx" => Ok(Self::Fx),
            _ => Err(format!("unknown SIMM risk class '{raw}'")),
        }
    }
}

impl std::str::FromStr for SimmCreditSector {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw {
            "sovereign" => Ok(Self::Sovereign),
            "financial" => Ok(Self::Financial),
            "basic_materials" => Ok(Self::BasicMaterials),
            "consumer_goods" => Ok(Self::ConsumerGoods),
            "technology_media" => Ok(Self::TechnologyMedia),
            "health_care" => Ok(Self::HealthCare),
            "high_yield_sovereign" => Ok(Self::HighYieldSovereign),
            "high_yield_financial" => Ok(Self::HighYieldFinancial),
            "high_yield_basic_materials" => Ok(Self::HighYieldBasicMaterials),
            "high_yield_consumer_goods" => Ok(Self::HighYieldConsumerGoods),
            "high_yield_technology_media" => Ok(Self::HighYieldTechnologyMedia),
            "high_yield_health_care" => Ok(Self::HighYieldHealthCare),
            "residual" => Ok(Self::Residual),
            _ => Err(format!("unknown SIMM credit sector '{raw}'")),
        }
    }
}

/// JSON-friendly representation of [`SimmSensitivities`].
///
/// Tuple-keyed maps cannot be represented directly as JSON object keys, so this
/// DTO stores each bucket as an array of tuples. It is the canonical JSON shape
/// used by language bindings and examples.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SimmSensitivitiesJson {
    /// Base currency for the sensitivities.
    pub base_currency: Currency,
    /// Interest-rate delta buckets as `(currency, tenor, amount)`.
    #[serde(default)]
    pub ir_delta: Vec<(Currency, String, f64)>,
    /// Interest-rate vega buckets as `(currency, tenor, amount)`.
    #[serde(default)]
    pub ir_vega: Vec<(Currency, String, f64)>,
    /// Credit qualifying deltas as `(sector, name, tenor, amount)`.
    #[serde(default)]
    pub credit_qualifying_delta: Vec<(SimmCreditSector, String, String, f64)>,
    /// Credit qualifying vegas as `(sector, name, tenor, amount)`.
    #[serde(default)]
    pub credit_qualifying_vega: Vec<(SimmCreditSector, String, String, f64)>,
    /// Credit non-qualifying delta buckets as `(name, tenor, amount)`.
    #[serde(default)]
    pub credit_non_qualifying_delta: Vec<(String, String, f64)>,
    /// Credit non-qualifying vega buckets as `(name, tenor, amount)`.
    #[serde(default)]
    pub credit_non_qualifying_vega: Vec<(String, String, f64)>,
    /// Equity delta buckets as `(underlier, amount)`.
    #[serde(default)]
    pub equity_delta: Vec<(String, f64)>,
    /// Equity vega buckets as `(underlier, amount)`.
    #[serde(default)]
    pub equity_vega: Vec<(String, f64)>,
    /// FX delta buckets as `(currency, amount)`.
    #[serde(default)]
    pub fx_delta: Vec<(Currency, f64)>,
    /// FX vega buckets as `(ccy1, ccy2, amount)`.
    #[serde(default)]
    pub fx_vega: Vec<(Currency, Currency, f64)>,
    /// Commodity delta buckets as `(bucket, amount)`.
    #[serde(default)]
    pub commodity_delta: Vec<(String, f64)>,
    /// Commodity vega buckets as `(bucket, amount)`.
    #[serde(default)]
    pub commodity_vega: Vec<(String, f64)>,
    /// Expiry-resolved volatility-weighted vega inputs before curvature scaling.
    #[serde(default)]
    pub curvature: Vec<SimmCurvatureSensitivity>,
}

impl From<&SimmSensitivities> for SimmSensitivitiesJson {
    fn from(sens: &SimmSensitivities) -> Self {
        let mut json = Self {
            base_currency: sens.base_currency,
            ir_delta: sens
                .ir_delta
                .iter()
                .map(|((currency, tenor), amount)| (*currency, tenor.clone(), *amount))
                .collect(),
            ir_vega: sens
                .ir_vega
                .iter()
                .map(|((currency, tenor), amount)| (*currency, tenor.clone(), *amount))
                .collect(),
            credit_qualifying_delta: sens
                .credit_qualifying_delta
                .iter()
                .map(|((sector, name, tenor), amount)| {
                    (*sector, name.clone(), tenor.clone(), *amount)
                })
                .collect(),
            credit_qualifying_vega: sens
                .credit_qualifying_vega
                .iter()
                .map(|((sector, name, tenor), amount)| {
                    (*sector, name.clone(), tenor.clone(), *amount)
                })
                .collect(),
            credit_non_qualifying_delta: sens
                .credit_non_qualifying_delta
                .iter()
                .map(|((name, tenor), amount)| (name.clone(), tenor.clone(), *amount))
                .collect(),
            credit_non_qualifying_vega: sens
                .credit_non_qualifying_vega
                .iter()
                .map(|((name, tenor), amount)| (name.clone(), tenor.clone(), *amount))
                .collect(),
            equity_delta: sens
                .equity_delta
                .iter()
                .map(|(underlier, amount)| (underlier.clone(), *amount))
                .collect(),
            equity_vega: sens
                .equity_vega
                .iter()
                .map(|(underlier, amount)| (underlier.clone(), *amount))
                .collect(),
            fx_delta: sens
                .fx_delta
                .iter()
                .map(|(currency, amount)| (*currency, *amount))
                .collect(),
            fx_vega: sens
                .fx_vega
                .iter()
                .map(|((ccy1, ccy2), amount)| (*ccy1, *ccy2, *amount))
                .collect(),
            commodity_delta: sens
                .commodity_delta
                .iter()
                .map(|(bucket, amount)| (bucket.clone(), *amount))
                .collect(),
            commodity_vega: sens
                .commodity_vega
                .iter()
                .map(|(bucket, amount)| (bucket.clone(), *amount))
                .collect(),
            curvature: sens.curvature.clone(),
        };
        for entries in [&mut json.ir_delta, &mut json.ir_vega] {
            entries.sort_by(|left, right| {
                left.0
                    .numeric()
                    .cmp(&right.0.numeric())
                    .then_with(|| left.1.cmp(&right.1))
            });
        }
        for entries in [
            &mut json.credit_non_qualifying_delta,
            &mut json.credit_non_qualifying_vega,
        ] {
            entries.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
        }
        for entries in [&mut json.equity_delta, &mut json.equity_vega] {
            entries.sort_by(|left, right| left.0.cmp(&right.0));
        }
        json.fx_delta.sort_by_key(|entry| entry.0.numeric());
        json.fx_vega.sort_by(|left, right| {
            left.0
                .numeric()
                .cmp(&right.0.numeric())
                .then_with(|| left.1.numeric().cmp(&right.1.numeric()))
        });
        for entries in [&mut json.commodity_delta, &mut json.commodity_vega] {
            entries.sort_by(|left, right| left.0.cmp(&right.0));
        }
        json.curvature.sort_by(|a, b| {
            (
                simm_risk_class_sort_key(a.risk_class),
                &a.bucket,
                &a.factor,
                &a.risk_tenor,
                &a.expiry_tenor,
            )
                .cmp(&(
                    simm_risk_class_sort_key(b.risk_class),
                    &b.bucket,
                    &b.factor,
                    &b.risk_tenor,
                    &b.expiry_tenor,
                ))
                .then_with(|| {
                    a.volatility_weighted_vega
                        .total_cmp(&b.volatility_weighted_vega)
                })
        });
        for entries in [
            &mut json.credit_qualifying_delta,
            &mut json.credit_qualifying_vega,
        ] {
            entries.sort_by(|left, right| {
                simm_credit_sector_sort_key(left.0)
                    .cmp(&simm_credit_sector_sort_key(right.0))
                    .then_with(|| left.1.cmp(&right.1))
                    .then_with(|| left.2.cmp(&right.2))
            });
        }
        json
    }
}

const fn simm_risk_class_sort_key(risk_class: SimmRiskClass) -> u8 {
    match risk_class {
        SimmRiskClass::InterestRate => 0,
        SimmRiskClass::CreditQualifying => 1,
        SimmRiskClass::CreditNonQualifying => 2,
        SimmRiskClass::Equity => 3,
        SimmRiskClass::Commodity => 4,
        SimmRiskClass::Fx => 5,
    }
}

const fn simm_credit_sector_sort_key(sector: SimmCreditSector) -> u8 {
    match sector {
        SimmCreditSector::Sovereign => 0,
        SimmCreditSector::Financial => 1,
        SimmCreditSector::BasicMaterials => 2,
        SimmCreditSector::ConsumerGoods => 3,
        SimmCreditSector::TechnologyMedia => 4,
        SimmCreditSector::HealthCare => 5,
        SimmCreditSector::HighYieldSovereign => 6,
        SimmCreditSector::HighYieldFinancial => 7,
        SimmCreditSector::HighYieldBasicMaterials => 8,
        SimmCreditSector::HighYieldConsumerGoods => 9,
        SimmCreditSector::HighYieldTechnologyMedia => 10,
        SimmCreditSector::HighYieldHealthCare => 11,
        SimmCreditSector::Residual => 12,
    }
}

impl From<SimmSensitivitiesJson> for SimmSensitivities {
    fn from(value: SimmSensitivitiesJson) -> Self {
        let mut sens = Self::new(value.base_currency);
        for (currency, tenor, amount) in value.ir_delta {
            sens.add_ir_delta(currency, tenor, amount);
        }
        for (currency, tenor, amount) in value.ir_vega {
            sens.add_ir_vega(currency, tenor, amount);
        }
        for (sector, name, tenor, amount) in value.credit_qualifying_delta {
            sens.add_credit_qualifying_delta(sector, name, tenor, amount);
        }
        for (sector, name, tenor, amount) in value.credit_qualifying_vega {
            sens.add_credit_qualifying_vega(sector, name, tenor, amount);
        }
        for (name, tenor, amount) in value.credit_non_qualifying_delta {
            sens.add_credit_non_qualifying_delta(name, tenor, amount);
        }
        for (name, tenor, amount) in value.credit_non_qualifying_vega {
            sens.add_credit_non_qualifying_vega(name, tenor, amount);
        }
        for (underlier, amount) in value.equity_delta {
            sens.add_equity_delta(underlier, amount);
        }
        for (underlier, amount) in value.equity_vega {
            sens.add_equity_vega(underlier, amount);
        }
        for (currency, amount) in value.fx_delta {
            sens.add_fx_delta(currency, amount);
        }
        for (ccy1, ccy2, amount) in value.fx_vega {
            sens.add_fx_vega(ccy1, ccy2, amount);
        }
        for (bucket, amount) in value.commodity_delta {
            sens.add_commodity_delta(bucket, amount);
        }
        for (bucket, amount) in value.commodity_vega {
            sens.add_commodity_vega(bucket, amount);
        }
        sens.curvature = value.curvature;
        sens
    }
}

/// SIMM sensitivity inputs organized by risk class.
///
/// Contains the risk sensitivities needed for ISDA SIMM calculation.
/// Sensitivities are organized by risk class and further bucketed
/// according to SIMM specifications.
///
/// # Units And Conventions
///
/// - Delta and vega entries are stored as currency amounts, not as decimal
///   rates or basis-point quote moves.
/// - For rate and credit buckets, callers should provide DV01/CS01-style
///   amounts in currency per 1bp move before loading them into this struct.
/// - Equity, FX and commodity deltas are currency P&L per 1% relative
///   price move: `price * dPV/dprice * 0.01`, before concentration or risk weights.
/// - All vegas are `sigma * dPV/dsigma` in currency before HVR, VRW and
///   concentration. For non-IR classes sigma uses SIMM paragraph 10(b)'s proxy.
/// - Curvature inputs preserve expiry and factor identity before tenor scaling.
/// - Tenor labels must be one of [`SIMM_TENORS`] (`2W`, `1M`, `3M`, `6M`,
///   `1Y`, `2Y`, `3Y`, `5Y`, `10Y`, `15Y`, `20Y`, `30Y`); [`Self::validate`]
///   rejects anything else and the calculator runs it before pricing.
/// - Signs are preserved on input so netting and offsetting can occur before
///   SIMM applies absolute-value or quadratic aggregation steps.
/// - `base_currency` identifies the currency in which the sensitivity set was
///   produced; the margin result currency is chosen separately by the caller.
///
/// # Example
///
/// ```
/// use finstack_quant_margin::{SimmCreditSector, SimmSensitivities};
/// use finstack_quant_core::currency::Currency;
///
/// let mut sensitivities = SimmSensitivities::new(Currency::USD);
///
/// // Add IR delta sensitivities by tenor
/// sensitivities.add_ir_delta(Currency::USD, "2Y", 15_000.0);
/// sensitivities.add_ir_delta(Currency::USD, "5Y", 45_000.0);
/// sensitivities.add_ir_delta(Currency::USD, "10Y", 25_000.0);
///
/// // Add sector-bucketed credit qualifying delta
/// sensitivities.add_credit_qualifying_delta(
///     SimmCreditSector::Financial,
///     "CDX.NA.IG",
///     "5Y",
///     50_000.0,
/// );
/// ```
///
/// # References
///
/// - ISDA SIMM: `docs/REFERENCES.md#isda-simm`
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct SimmSensitivities {
    /// Base currency for the sensitivities.
    ///
    /// This is the currency context in which the sensitivity set was produced.
    /// It does not force the output currency of the eventual margin result.
    pub base_currency: Currency,

    /// Interest rate delta by (currency, tenor bucket).
    ///
    /// Tenor buckets follow SIMM specification: 2W, 1M, 3M, 6M, 1Y, 2Y, 3Y, 5Y, 10Y, 15Y, 20Y, 30Y
    pub ir_delta: HashMap<(Currency, String), f64>,

    /// Interest rate vega by `(currency, tenor bucket)`.
    ///
    /// Values are sigma times dPV/dsigma in currency before VRW or concentration.
    /// This legacy two-dimensional vega input collapses underlying-maturity detail;
    /// curvature has the separate full expiry-resolved input.
    pub ir_vega: HashMap<(Currency, String), f64>,

    /// Credit qualifying delta by `(sector, issuer/index, tenor bucket)`.
    ///
    /// Sector assignment is mandatory so the calculator can apply ISDA SIMM
    /// intra- and inter-bucket aggregation without a scalar approximation.
    pub credit_qualifying_delta: HashMap<(SimmCreditSector, String, String), f64>,

    /// Credit qualifying vega by `(sector, issuer/index, tenor bucket)`.
    ///
    /// Bucketed exactly like [`credit_qualifying_delta`](Self::credit_qualifying_delta)
    /// so ISDA SIMM applies the same intra- and inter-bucket aggregation to the
    /// vega risk class, weighted by the single credit-qualifying vega risk
    /// weight rather than the per-bucket delta weights.
    pub credit_qualifying_vega: HashMap<(SimmCreditSector, String, String), f64>,

    /// Credit non-qualifying delta by (issuer/index, tenor bucket).
    ///
    /// For securitizations and exposures explicitly classified as non-qualifying.
    pub credit_non_qualifying_delta: HashMap<(String, String), f64>,

    /// Credit non-qualifying vega by `(issuer/index, tenor bucket)`.
    ///
    /// Pooled like [`credit_non_qualifying_delta`](Self::credit_non_qualifying_delta)
    /// and weighted by the credit-non-qualifying vega risk weight.
    pub credit_non_qualifying_vega: HashMap<(String, String), f64>,

    /// Equity delta by underlier.
    ///
    /// Values are signed currency P&L per 1% relative equity-price increase.
    pub equity_delta: HashMap<String, f64>,

    /// Equity vega by underlier.
    pub equity_vega: HashMap<String, f64>,

    /// FX delta by currency.
    ///
    /// Values are signed currency P&L per 1% relative FX-price increase,
    /// before risk weighting or concentration. USD is the calculation currency.
    pub fx_delta: HashMap<Currency, f64>,

    /// FX vega by currency pair.
    pub fx_vega: HashMap<(Currency, Currency), f64>,

    /// Commodity delta P&L per 1% relative price increase by bucket.
    ///
    /// Bucket labels should match the SIMM commodity bucket naming expected by
    /// the calculator's registry-backed lookup table.
    pub commodity_delta: HashMap<String, f64>,

    /// Commodity vega by bucket.
    ///
    /// Bucket labels follow the same SIMM commodity bucket naming as
    /// [`commodity_delta`](Self::commodity_delta); the single commodity vega
    /// risk weight replaces the per-bucket delta weights.
    pub commodity_vega: HashMap<String, f64>,

    /// Expiry-resolved signed `sigma * dPV/dsigma` inputs before SF, HVR,
    /// vega risk weights or concentration. Entries are retained separately
    /// until expiry scaling, so opposite vegas at different expiries do not
    /// incorrectly cancel curvature.
    pub curvature: Vec<SimmCurvatureSensitivity>,
}

impl SimmSensitivities {
    /// Create new empty sensitivities for a base currency.
    ///
    /// # Arguments
    ///
    /// * `base_currency` - Currency context in which the raw sensitivities were computed
    ///
    /// # Returns
    ///
    /// An empty sensitivity container ready for incremental population.
    #[must_use]
    pub fn new(base_currency: Currency) -> Self {
        Self {
            base_currency,
            ir_delta: HashMap::default(),
            ir_vega: HashMap::default(),
            credit_qualifying_delta: HashMap::default(),
            credit_qualifying_vega: HashMap::default(),
            credit_non_qualifying_delta: HashMap::default(),
            credit_non_qualifying_vega: HashMap::default(),
            equity_delta: HashMap::default(),
            equity_vega: HashMap::default(),
            fx_delta: HashMap::default(),
            fx_vega: HashMap::default(),
            commodity_delta: HashMap::default(),
            commodity_vega: HashMap::default(),
            curvature: Vec::new(),
        }
    }

    /// Add an interest-rate delta sensitivity bucket.
    ///
    /// `delta` should be a signed DV01-style currency amount for the given tenor
    /// bucket, typically interpreted as currency per 1bp move.
    pub fn add_ir_delta(&mut self, currency: Currency, tenor: impl Into<String>, delta: f64) {
        let key = (currency, tenor.into());
        *self.ir_delta.entry(key).or_insert(0.0) += delta;
    }

    /// Add an interest-rate vega sensitivity bucket.
    ///
    /// `vega` should be a signed currency amount compatible with the SIMM vega
    /// weighting conventions for the specified tenor bucket.
    pub fn add_ir_vega(&mut self, currency: Currency, tenor: impl Into<String>, vega: f64) {
        let key = (currency, tenor.into());
        *self.ir_vega.entry(key).or_insert(0.0) += vega;
    }

    /// Add a sector-bucketed credit-qualifying delta sensitivity.
    ///
    /// # Arguments
    ///
    /// * `sector` - ISDA SIMM credit-qualifying sector bucket.
    /// * `name` - Issuer or index identifier.
    /// * `tenor` - Tenor bucket such as `"5Y"`.
    /// * `delta` - Signed CS01-style currency amount, typically currency per 1bp move.
    pub fn add_credit_qualifying_delta(
        &mut self,
        sector: SimmCreditSector,
        name: impl Into<String>,
        tenor: impl Into<String>,
        delta: f64,
    ) {
        let key = (sector, name.into(), tenor.into());
        *self.credit_qualifying_delta.entry(key).or_insert(0.0) += delta;
    }

    /// Add a sector-bucketed credit-qualifying vega sensitivity.
    ///
    /// # Arguments
    ///
    /// * `sector` - ISDA SIMM credit-qualifying sector bucket.
    /// * `name` - Issuer or index identifier.
    /// * `tenor` - Tenor bucket such as `"5Y"`.
    /// * `vega` - Signed currency vega amount compatible with the SIMM
    ///   credit-qualifying vega risk weight.
    pub fn add_credit_qualifying_vega(
        &mut self,
        sector: SimmCreditSector,
        name: impl Into<String>,
        tenor: impl Into<String>,
        vega: f64,
    ) {
        let key = (sector, name.into(), tenor.into());
        *self.credit_qualifying_vega.entry(key).or_insert(0.0) += vega;
    }

    /// Add a credit non-qualifying delta sensitivity.
    ///
    /// # Arguments
    ///
    /// * `name` - Securitization or other non-qualifying exposure identifier.
    /// * `tenor` - Tenor bucket such as `"5Y"`.
    /// * `delta` - Signed CS01-style currency amount, typically currency per 1bp move.
    pub fn add_credit_non_qualifying_delta(
        &mut self,
        name: impl Into<String>,
        tenor: impl Into<String>,
        delta: f64,
    ) {
        let key = (name.into(), tenor.into());
        *self.credit_non_qualifying_delta.entry(key).or_insert(0.0) += delta;
    }

    /// Add a credit non-qualifying vega sensitivity.
    ///
    /// # Arguments
    ///
    /// * `name` - Securitization or other non-qualifying exposure identifier.
    /// * `tenor` - Tenor bucket such as `"5Y"`.
    /// * `vega` - Signed currency vega amount compatible with the SIMM
    ///   credit-non-qualifying vega risk weight.
    pub fn add_credit_non_qualifying_vega(
        &mut self,
        name: impl Into<String>,
        tenor: impl Into<String>,
        vega: f64,
    ) {
        let key = (name.into(), tenor.into());
        *self.credit_non_qualifying_vega.entry(key).or_insert(0.0) += vega;
    }

    /// Add an equity delta sensitivity bucket.
    ///
    /// `delta` is a signed currency sensitivity for the named underlier.
    pub fn add_equity_delta(&mut self, underlier: impl Into<String>, delta: f64) {
        let key = underlier.into();
        *self.equity_delta.entry(key).or_insert(0.0) += delta;
    }

    /// Add an equity vega sensitivity bucket.
    pub fn add_equity_vega(&mut self, underlier: impl Into<String>, vega: f64) {
        let key = underlier.into();
        *self.equity_vega.entry(key).or_insert(0.0) += vega;
    }

    /// Add an FX delta sensitivity bucket.
    ///
    /// `delta` is a signed currency sensitivity to the specified FX risk factor.
    pub fn add_fx_delta(&mut self, currency: Currency, delta: f64) {
        *self.fx_delta.entry(currency).or_insert(0.0) += delta;
    }

    /// Add an FX vega sensitivity bucket.
    pub fn add_fx_vega(&mut self, ccy1: Currency, ccy2: Currency, vega: f64) {
        *self.fx_vega.entry((ccy1, ccy2)).or_insert(0.0) += vega;
    }

    /// Add a commodity delta sensitivity bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Commodity bucket label from the SIMM registry; repeated labels accumulate their sensitivities, and validation rejects unknown labels.
    /// * `delta` - Signed currency delta in the calculation's base currency before SIMM risk weighting; no currency conversion is performed here.
    pub fn add_commodity_delta(&mut self, bucket: impl Into<String>, delta: f64) {
        let key = bucket.into();
        *self.commodity_delta.entry(key).or_insert(0.0) += delta;
    }

    /// Add a commodity vega sensitivity bucket.
    pub fn add_commodity_vega(&mut self, bucket: impl Into<String>, vega: f64) {
        let key = bucket.into();
        *self.commodity_vega.entry(key).or_insert(0.0) += vega;
    }

    /// Preserve one option-expiry contribution until curvature scaling is applied.
    ///
    /// # Arguments
    ///
    /// * `sensitivity` - Signed volatility-weighted vega with explicit factor,
    ///   risk tenor and option expiry. Validation occurs in `validate` and the calculator.
    pub fn add_curvature(&mut self, sensitivity: SimmCurvatureSensitivity) {
        self.curvature.push(sensitivity);
    }

    /// Validate tenor labels, commodity buckets, identifiers and amounts.
    ///
    /// The SIMM calculator looks every tenor and commodity bucket up in its
    /// registry tables and silently skips keys it does not know, so an
    /// unvalidated typo (`"7Y"`, `"bucket 18"`) under-states margin without
    /// any signal. [`crate::SimmCalculator::calculate_from_sensitivities`]
    /// calls this before pricing; call it directly to check a container
    /// built from external data.
    ///
    /// # Errors
    ///
    /// Returns a validation error naming the offending map when a tenor is
    /// not in [`SIMM_TENORS`], a commodity bucket is not one of the 17 ISDA
    /// buckets (see [`commodity_bucket_id`]), an issuer/underlier identifier
    /// is empty, or any amount is non-finite.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        fn tenor(field: &str, label: &str) -> finstack_quant_core::Result<()> {
            if SIMM_TENORS.contains(&label) {
                Ok(())
            } else {
                Err(finstack_quant_core::Error::Validation(format!(
                    "SIMM sensitivity {field} has unknown tenor '{label}'; expected one of {}",
                    SIMM_TENORS.join(", ")
                )))
            }
        }
        fn identifier(field: &str, value: &str) -> finstack_quant_core::Result<()> {
            if value.trim().is_empty() {
                Err(finstack_quant_core::Error::Validation(format!(
                    "SIMM sensitivity {field} must not be empty"
                )))
            } else {
                Ok(())
            }
        }

        if let Some(field) = amount_maps!(non_finite self) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "SIMM sensitivity {field} contains a non-finite amount"
            )));
        }
        for sensitivity in &self.curvature {
            sensitivity.validate()?;
        }
        for (currency, label) in self.ir_delta.keys().chain(self.ir_vega.keys()) {
            tenor(&format!("ir ({currency})"), label)?;
        }
        for (_, name, label) in self
            .credit_qualifying_delta
            .keys()
            .chain(self.credit_qualifying_vega.keys())
        {
            identifier("credit_qualifying.name", name)?;
            tenor("credit_qualifying", label)?;
        }
        for (name, label) in self
            .credit_non_qualifying_delta
            .keys()
            .chain(self.credit_non_qualifying_vega.keys())
        {
            identifier("credit_non_qualifying.name", name)?;
            tenor("credit_non_qualifying", label)?;
        }
        for underlier in self.equity_delta.keys().chain(self.equity_vega.keys()) {
            identifier("equity.underlier", underlier)?;
        }
        for bucket in self
            .commodity_delta
            .keys()
            .chain(self.commodity_vega.keys())
        {
            if commodity_bucket_id(bucket).is_none() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "SIMM sensitivity commodity has unknown bucket '{bucket}'; expected a bucket id \
                     1..={SIMM_COMMODITY_BUCKET_COUNT} or an ISDA SIMM commodity bucket name"
                )));
            }
        }
        Ok(())
    }

    /// Construct sensitivities from the canonical JSON representation.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the JSON cannot be deserialized.
    pub fn from_json(json: &str) -> finstack_quant_core::Result<Self> {
        let wire = serde_json::from_str::<SimmSensitivitiesJson>(json).map_err(|e| {
            finstack_quant_core::Error::Validation(format!("invalid SIMM sensitivities JSON: {e}"))
        })?;
        let value = Self::from(wire);
        value.validate()?;
        Ok(value)
    }

    /// Serialize sensitivities to the canonical JSON representation.
    ///
    /// # Errors
    ///
    /// Returns a validation error if serialization fails.
    pub fn to_json(&self) -> finstack_quant_core::Result<String> {
        serde_json::to_string(&SimmSensitivitiesJson::from(self)).map_err(|e| {
            finstack_quant_core::Error::Validation(format!(
                "failed to serialize SIMM sensitivities: {e}"
            ))
        })
    }

    /// Check if sensitivities are empty.
    ///
    /// Returns true if no sensitivity buckets exist across any risk class.
    /// Note: This checks bucket existence, not whether net sensitivities are zero.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        amount_maps!(is_empty self) && self.curvature.is_empty()
    }

    /// Merge another set of sensitivities into this one.
    ///
    /// Sensitivities are added together, enabling risk offsetting within a netting set.
    ///
    /// # Arguments
    ///
    /// * `other` - Second operand used by the binary arithmetic or merge operation
    pub fn merge(&mut self, other: &SimmSensitivities) {
        amount_maps!(merge self, other);
        self.curvature.extend(other.curvature.iter().cloned());
    }

    /// Return a copy of these sensitivities re-expressed in `target_currency`.
    ///
    /// Every entry is a signed **currency amount** denominated in
    /// [`base_currency`](Self::base_currency), so converting the set to another
    /// currency is a uniform multiply by the spot factor `fx_rate` (the value of
    /// one unit of `base_currency` expressed in `target_currency`). Keys — which
    /// merely *name* the risk factor (the FX/IR currency, the equity underlier,
    /// the commodity bucket) — are unchanged; only the amounts are rescaled.
    ///
    /// This must be applied before [`merge`](Self::merge) when combining
    /// sensitivity sets produced in different base currencies: `merge` sums raw
    /// amounts, so mixing currencies without first collapsing them violates
    /// currency safety and produces a wrong IM.
    ///
    /// # Arguments
    ///
    /// * `target_currency` - Currency that every sensitivity amount should be
    ///   expressed in after the conversion.
    /// * `fx_rate` - Spot value of one unit of the current
    ///   [`base_currency`](Self::base_currency) in `target_currency`. Amounts
    ///   are multiplied by this factor; risk-factor keys are unchanged.
    #[must_use]
    pub fn scaled_to_currency(&self, target_currency: Currency, fx_rate: f64) -> Self {
        let mut out = self.clone();
        out.base_currency = target_currency;
        if target_currency == self.base_currency {
            return out;
        }
        out.scale_amounts(fx_rate);
        out
    }

    /// Return a copy of these sensitivities with every amount multiplied by a
    /// signed scalar `factor`, keeping the base currency unchanged.
    ///
    /// This is the position-quantity counterpart of
    /// [`scaled_to_currency`](Self::scaled_to_currency): a trade-level
    /// sensitivity set produced per unit notional is converted to the HELD
    /// sensitivity by multiplying every bucket by the signed position scale
    /// factor before netting-set [`merge`](Self::merge). SIMM sensitivities
    /// are signed (ISDA SIMM nets long/short risk within a netting set), so
    /// `factor` MUST keep its sign — a short position (`factor < 0`) flips
    /// each bucket so it offsets an equal long.
    ///
    /// # Arguments
    ///
    /// * `factor` - Signed multiplier applied uniformly to every sensitivity
    ///   amount (e.g. position quantity for unit-notional trade sensitivities).
    #[must_use]
    pub fn scaled(&self, factor: f64) -> Self {
        let mut out = self.clone();
        out.scale_amounts(factor);
        out
    }

    /// Multiply every sensitivity amount in place by `factor`.
    ///
    /// Keys (risk-factor names) and `base_currency` are untouched; only the
    /// signed amounts are rescaled.
    fn scale_amounts(&mut self, factor: f64) {
        amount_maps!(scale self, factor);
        for input in &mut self.curvature {
            input.volatility_weighted_vega *= factor;
        }
    }

    /// Get total IR delta across all currencies and tenors.
    #[must_use]
    pub fn total_ir_delta(&self) -> f64 {
        self.ir_delta.values().sum()
    }

    /// Get total equity delta.
    #[must_use]
    pub fn total_equity_delta(&self) -> f64 {
        self.equity_delta.values().sum()
    }
}

// Symmetric-map ordering helpers

/// Canonical ordering of a risk-class pair for symmetric correlation lookups.
///
/// SIMM correlation matrices are symmetric, so only `(min, max)` keys are
/// stored in the registry. All callers MUST route pair lookups through this
/// helper to avoid missing entries.
///
/// # Arguments
///
/// * `a` - First SIMM risk class to normalize for a symmetric lookup.
/// * `b` - Second SIMM risk class to normalize for a symmetric lookup.
#[must_use]
pub fn ordered_risk_class_pair(
    a: SimmRiskClass,
    b: SimmRiskClass,
) -> (SimmRiskClass, SimmRiskClass) {
    if (a as u8) <= (b as u8) {
        (a, b)
    } else {
        (b, a)
    }
}

/// Canonical ordering of a tenor-label pair for symmetric correlation lookups.
///
/// # Arguments
///
/// * `a` - First tenor label, compared lexicographically without changing its
///   text in the returned key.
/// * `b` - Second tenor label, compared lexicographically without changing its
///   text in the returned key.
#[must_use]
pub fn ordered_tenor_pair(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

/// Canonical ordering of a credit-sector pair for symmetric correlation lookups.
///
/// # Arguments
///
/// * `a` - First SIMM credit sector to normalize for a symmetric lookup.
/// * `b` - Second SIMM credit sector to normalize for a symmetric lookup.
#[must_use]
pub fn ordered_credit_sector_pair(
    a: SimmCreditSector,
    b: SimmCreditSector,
) -> (SimmCreditSector, SimmCreditSector) {
    if (a as u8) <= (b as u8) {
        (a, b)
    } else {
        (b, a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curvature_input(class: SimmRiskClass, amount: f64) -> SimmCurvatureSensitivity {
        SimmCurvatureSensitivity {
            risk_class: class,
            bucket: if class == SimmRiskClass::InterestRate {
                "USD"
            } else {
                "residual"
            }
            .into(),
            factor: "TEST".into(),
            risk_tenor: (class == SimmRiskClass::InterestRate).then(|| "5Y".into()),
            expiry_tenor: "1Y".into(),
            volatility_weighted_vega: amount,
        }
    }

    fn insert_entries<K>(map: &mut HashMap<K, f64>, mut entries: [(K, f64); 2], reverse: bool)
    where
        K: Eq + Hash,
    {
        if reverse {
            entries.reverse();
        }
        map.extend(entries);
    }

    fn populated_simm_sensitivities(reverse: bool) -> SimmSensitivities {
        let mut sensitivities = SimmSensitivities::new(Currency::USD);
        insert_entries(
            &mut sensitivities.ir_delta,
            [
                ((Currency::USD, "10Y".to_string()), 10.0),
                ((Currency::EUR, "2Y".to_string()), 20.0),
            ],
            reverse,
        );
        insert_entries(
            &mut sensitivities.ir_vega,
            [
                ((Currency::USD, "5Y".to_string()), 30.0),
                ((Currency::EUR, "1Y".to_string()), 40.0),
            ],
            reverse,
        );
        insert_entries(
            &mut sensitivities.credit_non_qualifying_delta,
            [
                (("RMBS_ZETA".to_string(), "5Y".to_string()), 70.0),
                (("RMBS_ALPHA".to_string(), "3Y".to_string()), 80.0),
            ],
            reverse,
        );
        insert_entries(
            &mut sensitivities.equity_delta,
            [("MSFT".to_string(), 90.0), ("AAPL".to_string(), 100.0)],
            reverse,
        );
        insert_entries(
            &mut sensitivities.equity_vega,
            [("MSFT".to_string(), 110.0), ("AAPL".to_string(), 120.0)],
            reverse,
        );
        insert_entries(
            &mut sensitivities.fx_delta,
            [(Currency::USD, 130.0), (Currency::EUR, 140.0)],
            reverse,
        );
        insert_entries(
            &mut sensitivities.fx_vega,
            [
                ((Currency::USD, Currency::JPY), 150.0),
                ((Currency::EUR, Currency::USD), 160.0),
            ],
            reverse,
        );
        insert_entries(
            &mut sensitivities.commodity_delta,
            [
                ("EuropeanPowerAndCarbon".to_string(), 170.0),
                ("Crude".to_string(), 180.0),
            ],
            reverse,
        );
        sensitivities.curvature = vec![
            curvature_input(SimmRiskClass::Equity, 190.0),
            curvature_input(SimmRiskClass::InterestRate, 200.0),
        ];
        if reverse {
            sensitivities.curvature.reverse();
        }

        insert_entries(
            &mut sensitivities.credit_qualifying_delta,
            [
                (
                    (
                        SimmCreditSector::Financial,
                        "BANK".to_string(),
                        "5Y".to_string(),
                    ),
                    210.0,
                ),
                (
                    (
                        SimmCreditSector::Sovereign,
                        "UST".to_string(),
                        "10Y".to_string(),
                    ),
                    220.0,
                ),
            ],
            reverse,
        );
        sensitivities
    }

    #[test]
    fn test_simm_sensitivities_creation() {
        let mut sens = SimmSensitivities::new(Currency::USD);
        assert!(sens.is_empty());

        sens.add_ir_delta(Currency::USD, "5Y", 100_000.0);
        sens.add_ir_delta(Currency::USD, "10Y", 50_000.0);
        sens.add_credit_qualifying_delta(
            SimmCreditSector::BasicMaterials,
            "ACME_CORP",
            "5Y",
            25_000.0,
        );
        sens.add_fx_vega(Currency::EUR, Currency::USD, 1_000.0);
        sens.add_commodity_delta("energy", 2_000.0);
        sens.add_curvature(curvature_input(SimmRiskClass::Equity, 3_000.0));

        assert!(!sens.is_empty());
        assert_eq!(sens.total_ir_delta(), 150_000.0);
        assert_eq!(
            sens.credit_qualifying_delta.values().sum::<f64>()
                + sens.credit_non_qualifying_delta.values().sum::<f64>(),
            25_000.0
        );
        assert_eq!(sens.fx_vega[&(Currency::EUR, Currency::USD)], 1_000.0);
        assert_eq!(sens.commodity_delta["energy"], 2_000.0);
        assert_eq!(sens.curvature[0].volatility_weighted_vega, 3_000.0);
    }

    #[test]
    fn validate_rejects_unknown_tenor_instead_of_pricing_zero() {
        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_ir_delta(Currency::USD, "7Y", 50_000.0);
        let err = sens.validate().expect_err("7Y is not a SIMM tenor");
        assert!(err.to_string().contains("'7Y'"), "{err}");

        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_credit_qualifying_delta(SimmCreditSector::Financial, "BANK", "4Y", 1.0);
        assert!(sens.validate().is_err());
    }

    #[test]
    fn validate_rejects_unknown_commodity_bucket_and_non_finite_amounts() {
        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_commodity_delta("bucket 18", 1_000.0);
        assert!(sens.validate().is_err());

        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_equity_delta("AAPL", f64::NAN);
        let err = sens.validate().expect_err("NaN amount");
        assert!(err.to_string().contains("equity_delta"), "{err}");

        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_equity_delta("", 1.0);
        assert!(sens.validate().is_err());
    }

    #[test]
    fn validate_accepts_every_simm_tenor_and_commodity_bucket_spelling() {
        let mut sens = SimmSensitivities::new(Currency::USD);
        for tenor in SIMM_TENORS {
            sens.add_ir_delta(Currency::USD, *tenor, 1.0);
            sens.add_credit_non_qualifying_delta("RMBS", *tenor, 1.0);
        }
        sens.add_commodity_delta("Crude", 1.0);
        sens.add_commodity_delta("light_ends", 1.0);
        sens.add_commodity_delta("17", 1.0);
        sens.add_curvature(curvature_input(SimmRiskClass::Equity, 1.0));
        sens.validate().expect("canonical labels validate");
        assert_eq!(commodity_bucket_id(" Precious Metals "), Some(12));
        assert_eq!(commodity_bucket_id("0"), None);
    }

    #[test]
    fn simm_enum_parsers_reject_noncanonical_values() {
        assert_eq!(
            "interest_rate"
                .parse::<SimmRiskClass>()
                .expect("canonical risk class"),
            SimmRiskClass::InterestRate
        );
        assert_eq!(
            "high_yield_financial"
                .parse::<SimmCreditSector>()
                .expect("canonical credit sector"),
            SimmCreditSector::HighYieldFinancial
        );
        for noncanonical in ["rates", "IR", "foreign_exchange", " interest_rate"] {
            assert!(noncanonical.parse::<SimmRiskClass>().is_err());
        }
        for noncanonical in ["hy_financial", "securitised", "other", "bucket_8"] {
            assert!(noncanonical.parse::<SimmCreditSector>().is_err());
        }
    }

    #[test]
    fn simm_sensitivities_json_round_trips_canonical_shape() {
        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_ir_delta(Currency::USD, "5Y", 100.0);
        sens.add_fx_vega(Currency::EUR, Currency::USD, 25.0);
        sens.add_commodity_delta("crude", 10.0);
        sens.add_curvature(curvature_input(SimmRiskClass::Equity, 5.0));

        let json = sens.to_json().expect("serialize sensitivities");
        let round_tripped = SimmSensitivities::from_json(&json).expect("deserialize sensitivities");

        assert_eq!(
            round_tripped.ir_delta[&(Currency::USD, "5Y".to_string())],
            100.0
        );
        assert_eq!(round_tripped.fx_vega[&(Currency::EUR, Currency::USD)], 25.0);
        assert_eq!(round_tripped.commodity_delta["crude"], 10.0);
        assert_eq!(round_tripped.curvature[0].volatility_weighted_vega, 5.0);
    }

    #[test]
    fn simm_json_rejects_scalar_credit_qualifying_shape() {
        let payload = serde_json::json!({
            "base_currency": "USD",
            "credit_qualifying_delta": [["CDX.NA.IG", "5Y", 1000.0]]
        });

        SimmSensitivities::from_json(&payload.to_string())
            .expect_err("sector is mandatory for credit qualifying delta");
    }

    #[test]
    fn simm_pretty_json_sorts_every_sensitivity_family() {
        let first = populated_simm_sensitivities(false);
        let second = populated_simm_sensitivities(true);
        let first_json = first.to_json().expect("first order serializes");
        let second_json = second.to_json().expect("reverse order serializes");
        assert_eq!(first_json, second_json);

        let json: SimmSensitivitiesJson =
            serde_json::from_str(&first_json).expect("canonical DTO parses");
        assert_eq!(json.ir_delta[0].0, Currency::USD);
        assert_eq!(json.ir_vega[0].0, Currency::USD);
        assert_eq!(
            json.credit_qualifying_delta[0].0,
            SimmCreditSector::Sovereign
        );
        assert_eq!(json.credit_qualifying_delta[0].1, "UST");
        assert_eq!(json.credit_non_qualifying_delta[0].0, "RMBS_ALPHA");
        assert_eq!(json.equity_delta[0].0, "AAPL");
        assert_eq!(json.equity_vega[0].0, "AAPL");
        assert_eq!(json.fx_delta[0].0, Currency::USD);
        assert_eq!(json.fx_vega[0].0, Currency::USD);
        assert_eq!(json.commodity_delta[0].0, "Crude");
        assert_eq!(json.curvature[0].risk_class, SimmRiskClass::InterestRate);
    }

    #[test]
    fn scaled_to_currency_rescales_all_amount_maps_uniformly() {
        let mut s = SimmSensitivities::new(Currency::EUR);
        s.add_ir_delta(Currency::EUR, "5Y", 100.0);
        s.equity_delta.insert("AAPL".to_string(), 50.0);
        s.fx_delta.insert(Currency::EUR, 25.0);

        // EUR -> USD at 1.10: every amount scales, base_currency re-tagged.
        let usd = s.scaled_to_currency(Currency::USD, 1.10);
        assert_eq!(usd.base_currency, Currency::USD);
        assert!((usd.ir_delta[&(Currency::EUR, "5Y".to_string())] - 110.0).abs() < 1e-9);
        assert!((usd.equity_delta["AAPL"] - 55.0).abs() < 1e-9);
        assert!((usd.fx_delta[&Currency::EUR] - 27.5).abs() < 1e-9);

        // Same-currency conversion is a no-op on amounts.
        let same = s.scaled_to_currency(Currency::EUR, 999.0);
        assert!((same.ir_delta[&(Currency::EUR, "5Y".to_string())] - 100.0).abs() < 1e-9);
    }

    #[test]
    fn scaled_multiplies_all_amount_maps_by_signed_factor() {
        let mut s = SimmSensitivities::new(Currency::USD);
        s.add_ir_delta(Currency::USD, "5Y", 100.0);
        s.equity_delta.insert("AAPL".to_string(), 50.0);
        s.fx_delta.insert(Currency::EUR, 25.0);

        // Positive quantity: uniform multiply, base currency untouched.
        let long = s.scaled(10.0);
        assert_eq!(long.base_currency, Currency::USD);
        assert!((long.ir_delta[&(Currency::USD, "5Y".to_string())] - 1_000.0).abs() < 1e-9);
        assert!((long.equity_delta["AAPL"] - 500.0).abs() < 1e-9);
        assert!((long.fx_delta[&Currency::EUR] - 250.0).abs() < 1e-9);

        // Signed factor: a short flips every bucket so it nets an equal long.
        let mut net = long;
        net.merge(&s.scaled(-10.0));
        assert!(net.ir_delta[&(Currency::USD, "5Y".to_string())].abs() < 1e-9);
        assert!(net.equity_delta["AAPL"].abs() < 1e-9);
        assert!(net.fx_delta[&Currency::EUR].abs() < 1e-9);
    }

    #[test]
    fn test_simm_sensitivities_merge() {
        let mut sens1 = SimmSensitivities::new(Currency::USD);
        sens1.add_ir_delta(Currency::USD, "5Y", 100_000.0);

        let mut sens2 = SimmSensitivities::new(Currency::USD);
        sens2.add_ir_delta(Currency::USD, "5Y", 50_000.0);
        sens2.add_ir_delta(Currency::USD, "10Y", 25_000.0);

        sens1.merge(&sens2);

        assert_eq!(
            sens1.ir_delta.get(&(Currency::USD, "5Y".to_string())),
            Some(&150_000.0)
        );
        assert_eq!(
            sens1.ir_delta.get(&(Currency::USD, "10Y".to_string())),
            Some(&25_000.0)
        );
    }

    #[test]
    fn test_simm_risk_class_display() {
        assert_eq!(SimmRiskClass::InterestRate.to_string(), "interest_rate");
        assert_eq!(
            SimmRiskClass::CreditQualifying.to_string(),
            "credit_qualifying"
        );
        assert_eq!(SimmRiskClass::Fx.to_string(), "fx");
    }

    #[test]
    fn credit_and_commodity_vega_round_trip_through_canonical_json() {
        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_credit_qualifying_vega(SimmCreditSector::Financial, "BANK_A", "5Y", 250.0);
        sens.add_credit_non_qualifying_vega("RMBS_A", "3Y", 125.0);
        sens.add_commodity_vega("Crude", 75.0);

        let json = sens.to_json().expect("serialize sensitivities");
        let round_tripped = SimmSensitivities::from_json(&json).expect("deserialize sensitivities");

        assert_eq!(
            round_tripped.credit_qualifying_vega[&(
                SimmCreditSector::Financial,
                "BANK_A".to_string(),
                "5Y".to_string(),
            )],
            250.0
        );
        assert_eq!(
            round_tripped.credit_non_qualifying_vega[&("RMBS_A".to_string(), "3Y".to_string())],
            125.0
        );
        assert_eq!(round_tripped.commodity_vega["Crude"], 75.0);
        assert_eq!(round_tripped, sens);
    }

    #[test]
    fn credit_and_commodity_vega_merge_and_scale_with_every_other_bucket() {
        let mut left = SimmSensitivities::new(Currency::USD);
        left.add_credit_qualifying_vega(SimmCreditSector::Sovereign, "UST", "10Y", 100.0);
        left.add_credit_non_qualifying_vega("RMBS_A", "5Y", 40.0);
        left.add_commodity_vega("Crude", 20.0);
        assert!(!left.is_empty());

        let mut right = left.clone();
        right.merge(&left);
        assert_eq!(
            right.credit_qualifying_vega[&(
                SimmCreditSector::Sovereign,
                "UST".to_string(),
                "10Y".to_string(),
            )],
            200.0
        );
        assert_eq!(
            right.credit_non_qualifying_vega[&("RMBS_A".to_string(), "5Y".to_string())],
            80.0
        );
        assert_eq!(right.commodity_vega["Crude"], 40.0);

        // A short position flips every new bucket so it nets an equal long.
        let mut net = left.clone();
        net.merge(&left.scaled(-1.0));
        assert!(net.credit_qualifying_vega.values().all(|v| v.abs() < 1e-12));
        assert!(net
            .credit_non_qualifying_vega
            .values()
            .all(|v| v.abs() < 1e-12));
        assert!(net.commodity_vega.values().all(|v| v.abs() < 1e-12));
    }
}
