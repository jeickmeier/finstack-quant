//! SIMM risk classification and sensitivity types.
//!
//! Types for ISDA SIMM categorization and sensitivity inputs,
//! used by the [`Marginable`](crate::traits::Marginable) trait
//! and SIMM calculator.

use core::hash::Hash;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::HashMap;

fn normalize_simm_label(raw: &str) -> String {
    raw.trim()
        .to_ascii_lowercase()
        .replace([' ', '-', '.'], "_")
}

/// Add every `(key, value)` from `source` into `target`, accumulating
/// values for keys that already exist.
///
/// Centralises the per-bucket merge step used by
/// [`SimmSensitivities::merge`] so adding a new sensitivity bucket is a
/// one-line change in `merge` rather than a copy-paste-edit cycle that
/// risks dropping the new field.
fn merge_into<K>(target: &mut HashMap<K, f64>, source: &HashMap<K, f64>)
where
    K: Eq + Hash + Clone,
{
    for (key, &value) in source {
        *target.entry(key.clone()).or_insert(0.0) += value;
    }
}

/// Risk classes for SIMM categorization.
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
#[non_exhaustive]
pub enum SimmRiskClass {
    /// Interest rate risk
    InterestRate,
    /// Credit qualifying (investment grade)
    CreditQualifying,
    /// Credit non-qualifying (high yield, emerging markets)
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
    /// Legacy broad index bucket. Production v2.6 registry parameters map this to Residual.
    Index,
    /// Legacy broad securitized bucket. Production v2.6 registry parameters map this to Residual.
    Securitized,
    /// Residual bucket.
    Residual,
}

impl std::fmt::Display for SimmRiskClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimmRiskClass::InterestRate => write!(f, "InterestRate"),
            SimmRiskClass::CreditQualifying => write!(f, "CreditQualifying"),
            SimmRiskClass::CreditNonQualifying => write!(f, "CreditNonQualifying"),
            SimmRiskClass::Equity => write!(f, "Equity"),
            SimmRiskClass::Commodity => write!(f, "Commodity"),
            SimmRiskClass::Fx => write!(f, "FX"),
        }
    }
}

impl std::str::FromStr for SimmRiskClass {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match normalize_simm_label(raw).as_str() {
            "interest_rate" | "ir" | "rates" => Ok(Self::InterestRate),
            "credit_qualifying" | "credit_qual" | "cq" => Ok(Self::CreditQualifying),
            "credit_non_qualifying" | "credit_nonqual" | "cnq" => Ok(Self::CreditNonQualifying),
            "equity" | "eq" => Ok(Self::Equity),
            "commodity" | "comm" => Ok(Self::Commodity),
            "fx" | "foreign_exchange" => Ok(Self::Fx),
            other => Err(format!("unknown SIMM risk class '{other}'")),
        }
    }
}

impl std::str::FromStr for SimmCreditSector {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match normalize_simm_label(raw).as_str() {
            "sovereign" | "ig_sovereign" => Ok(Self::Sovereign),
            "financial" | "ig_financial" => Ok(Self::Financial),
            "basic_materials" | "energy_industrials" | "ig_basic_materials" => {
                Ok(Self::BasicMaterials)
            }
            "consumer_goods" | "ig_consumer_goods" => Ok(Self::ConsumerGoods),
            "technology_media" | "technology" | "telecom" | "ig_technology_media" => {
                Ok(Self::TechnologyMedia)
            }
            "health_care" | "healthcare" | "utilities" | "ig_health_care" => Ok(Self::HealthCare),
            "high_yield_sovereign" | "hy_sovereign" => Ok(Self::HighYieldSovereign),
            "high_yield_financial" | "hy_financial" => Ok(Self::HighYieldFinancial),
            "high_yield_basic_materials" | "hy_basic_materials" => {
                Ok(Self::HighYieldBasicMaterials)
            }
            "high_yield_consumer_goods" | "hy_consumer_goods" => Ok(Self::HighYieldConsumerGoods),
            "high_yield_technology_media" | "hy_technology_media" => {
                Ok(Self::HighYieldTechnologyMedia)
            }
            "high_yield_health_care" | "hy_health_care" | "hy_healthcare" => {
                Ok(Self::HighYieldHealthCare)
            }
            "index" => Ok(Self::Index),
            "securitized" | "securitised" => Ok(Self::Securitized),
            "residual" | "other" => Ok(Self::Residual),
            other => Err(format!("unknown SIMM credit sector '{other}'")),
        }
    }
}

/// JSON-friendly representation of [`SimmSensitivities`].
///
/// Tuple-keyed maps cannot be represented directly as JSON object keys, so this
/// DTO stores each bucket as an array of tuples. It is the canonical JSON shape
/// used by language bindings and examples.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SimmSensitivitiesJson {
    /// Base currency for the sensitivities.
    pub base_currency: Currency,
    /// Interest-rate delta buckets as `(currency, tenor, amount)`.
    #[serde(default)]
    pub ir_delta: Vec<(Currency, String, f64)>,
    /// Interest-rate vega buckets as `(currency, tenor, amount)`.
    #[serde(default)]
    pub ir_vega: Vec<(Currency, String, f64)>,
    /// Credit qualifying delta buckets as `(name, tenor, amount)`.
    #[serde(default)]
    pub credit_qualifying_delta: Vec<(String, String, f64)>,
    /// Credit non-qualifying delta buckets as `(name, tenor, amount)`.
    #[serde(default)]
    pub credit_non_qualifying_delta: Vec<(String, String, f64)>,
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
    /// Curvature buckets as `(risk_class, amount)`.
    #[serde(default)]
    pub curvature: Vec<(SimmRiskClass, f64)>,
    /// Bucketed credit qualifying deltas as `(sector, name, tenor, amount)`.
    #[serde(default)]
    pub credit_qualifying_delta_bucketed: Vec<(SimmCreditSector, String, String, f64)>,
}

impl From<&SimmSensitivities> for SimmSensitivitiesJson {
    fn from(sens: &SimmSensitivities) -> Self {
        Self {
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
                .map(|((name, tenor), amount)| (name.clone(), tenor.clone(), *amount))
                .collect(),
            credit_non_qualifying_delta: sens
                .credit_non_qualifying_delta
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
            curvature: sens
                .curvature
                .iter()
                .map(|(risk_class, amount)| (*risk_class, *amount))
                .collect(),
            credit_qualifying_delta_bucketed: sens
                .credit_qualifying_delta_bucketed
                .iter()
                .map(|((sector, name, tenor), amount)| {
                    (*sector, name.clone(), tenor.clone(), *amount)
                })
                .collect(),
        }
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
        for (name, tenor, amount) in value.credit_qualifying_delta {
            sens.add_credit_delta(name, true, tenor, amount);
        }
        for (name, tenor, amount) in value.credit_non_qualifying_delta {
            sens.add_credit_delta(name, false, tenor, amount);
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
        for (risk_class, amount) in value.curvature {
            sens.add_curvature(risk_class, amount);
        }
        for (sector, name, tenor, amount) in value.credit_qualifying_delta_bucketed {
            sens.add_credit_delta_bucketed(sector, name, tenor, amount);
        }
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
/// - Tenor labels should match the registry-backed SIMM tenor set used by the
///   calculator, such as `2W`, `1M`, `3M`, `6M`, `1Y`, `2Y`, `3Y`, `5Y`,
///   `10Y`, `15Y`, `20Y`, and `30Y`.
/// - Signs are preserved on input so netting and offsetting can occur before
///   SIMM applies absolute-value or quadratic aggregation steps.
/// - `base_currency` identifies the currency in which the sensitivity set was
///   produced; the margin result currency is chosen separately by the caller.
///
/// # Example
///
/// ```ignore
/// use finstack_quant_margin::SimmSensitivities;
/// use finstack_quant_core::currency::Currency;
///
/// let mut sensitivities = SimmSensitivities::new(Currency::USD);
///
/// // Add IR delta sensitivities by tenor
/// sensitivities.add_ir_delta(Currency::USD, "2Y", 15_000.0);
/// sensitivities.add_ir_delta(Currency::USD, "5Y", 45_000.0);
/// sensitivities.add_ir_delta(Currency::USD, "10Y", 25_000.0);
///
/// // Add credit delta
/// sensitivities.add_credit_delta("CDX.NA.IG", true, "5Y", 50_000.0);
/// ```
///
/// # References
///
/// - ISDA SIMM: `docs/REFERENCES.md#isda-simm`
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
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
    /// Values should already be expressed in currency units compatible with the
    /// SIMM vega weights.
    pub ir_vega: HashMap<(Currency, String), f64>,

    /// Credit qualifying delta by (issuer/index, tenor bucket).
    ///
    /// For single-name CDS and investment-grade indices.
    pub credit_qualifying_delta: HashMap<(String, String), f64>,

    /// Credit non-qualifying delta by (issuer/index, tenor bucket).
    ///
    /// For high-yield, distressed, and emerging market credit.
    pub credit_non_qualifying_delta: HashMap<(String, String), f64>,

    /// Equity delta by underlier.
    ///
    /// Values are signed currency sensitivities, not percentage deltas.
    pub equity_delta: HashMap<String, f64>,

    /// Equity vega by underlier.
    pub equity_vega: HashMap<String, f64>,

    /// FX delta by currency.
    ///
    /// Values are signed currency sensitivities to the reporting FX risk factor
    /// used by the caller's SIMM mapping, not spot levels or percentage moves.
    pub fx_delta: HashMap<Currency, f64>,

    /// FX vega by currency pair.
    pub fx_vega: HashMap<(Currency, Currency), f64>,

    /// Commodity delta by bucket.
    ///
    /// Bucket labels should match the SIMM commodity bucket naming expected by
    /// the calculator's registry-backed lookup table.
    pub commodity_delta: HashMap<String, f64>,

    /// Curvature risk by risk class.
    ///
    /// Values should be the signed curvature contributions in currency units
    /// before the SIMM curvature scale factor is applied.
    pub curvature: HashMap<SimmRiskClass, f64>,

    /// Credit qualifying delta with sector bucket assignment.
    ///
    /// Keyed by `(sector, issuer/index, tenor)`. When populated, the SIMM
    /// calculator uses bucket-level aggregation with intra/inter-bucket
    /// diversification per ISDA SIMM v2.6 instead of the scalar fallback.
    ///
    /// This field is additive: callers that do not assign sectors can leave it
    /// empty and only populate [`credit_qualifying_delta`](Self::credit_qualifying_delta),
    /// which triggers the legacy scalar code path.
    pub credit_qualifying_delta_bucketed: HashMap<(SimmCreditSector, String, String), f64>,
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
            credit_non_qualifying_delta: HashMap::default(),
            equity_delta: HashMap::default(),
            equity_vega: HashMap::default(),
            fx_delta: HashMap::default(),
            fx_vega: HashMap::default(),
            commodity_delta: HashMap::default(),
            curvature: HashMap::default(),
            credit_qualifying_delta_bucketed: HashMap::default(),
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

    /// Add a credit delta sensitivity bucket.
    ///
    /// # Arguments
    ///
    /// * `name` - Issuer or index identifier
    /// * `qualifying` - `true` for qualifying credit, `false` for non-qualifying credit
    /// * `tenor` - Tenor bucket such as `"5Y"`
    /// * `delta` - Signed CS01-style currency amount, typically currency per 1bp move
    pub fn add_credit_delta(
        &mut self,
        name: impl Into<String>,
        qualifying: bool,
        tenor: impl Into<String>,
        delta: f64,
    ) {
        let key = (name.into(), tenor.into());
        if qualifying {
            *self.credit_qualifying_delta.entry(key).or_insert(0.0) += delta;
        } else {
            *self.credit_non_qualifying_delta.entry(key).or_insert(0.0) += delta;
        }
    }

    /// Add a credit delta sensitivity bucket with sector assignment.
    ///
    /// This populates the bucketed credit qualifying delta map used by the
    /// SIMM bucket-level aggregation path. Sensitivities added here are
    /// aggregated with intra/inter-bucket diversification.
    ///
    /// # Arguments
    ///
    /// * `sector` - ISDA SIMM credit qualifying sector bucket
    /// * `name` - Issuer or index identifier
    /// * `tenor` - Tenor bucket such as `"5Y"`
    /// * `delta` - Signed CS01-style currency amount, typically currency per 1bp move
    pub fn add_credit_delta_bucketed(
        &mut self,
        sector: SimmCreditSector,
        name: impl Into<String>,
        tenor: impl Into<String>,
        delta: f64,
    ) {
        let key = (sector, name.into(), tenor.into());
        *self
            .credit_qualifying_delta_bucketed
            .entry(key)
            .or_insert(0.0) += delta;
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
    pub fn add_commodity_delta(&mut self, bucket: impl Into<String>, delta: f64) {
        let key = bucket.into();
        *self.commodity_delta.entry(key).or_insert(0.0) += delta;
    }

    /// Add a curvature contribution for a SIMM risk class.
    pub fn add_curvature(&mut self, risk_class: SimmRiskClass, amount: f64) {
        *self.curvature.entry(risk_class).or_insert(0.0) += amount;
    }

    /// Construct sensitivities from the canonical JSON representation.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the JSON cannot be deserialized.
    pub fn from_json(json: &str) -> finstack_quant_core::Result<Self> {
        serde_json::from_str::<SimmSensitivitiesJson>(json)
            .map(Self::from)
            .map_err(|e| {
                finstack_quant_core::Error::Validation(format!(
                    "invalid SIMM sensitivities JSON: {e}"
                ))
            })
    }

    /// Serialize sensitivities to the canonical JSON representation.
    ///
    /// # Errors
    ///
    /// Returns a validation error if serialization fails.
    pub fn to_json_pretty(&self) -> finstack_quant_core::Result<String> {
        serde_json::to_string_pretty(&SimmSensitivitiesJson::from(self)).map_err(|e| {
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
        self.ir_delta.is_empty()
            && self.ir_vega.is_empty()
            && self.credit_qualifying_delta.is_empty()
            && self.credit_non_qualifying_delta.is_empty()
            && self.equity_delta.is_empty()
            && self.equity_vega.is_empty()
            && self.fx_delta.is_empty()
            && self.fx_vega.is_empty()
            && self.commodity_delta.is_empty()
            && self.curvature.is_empty()
            && self.credit_qualifying_delta_bucketed.is_empty()
    }

    /// Merge another set of sensitivities into this one.
    ///
    /// Sensitivities are added together, enabling risk offsetting within a netting set.
    pub fn merge(&mut self, other: &SimmSensitivities) {
        merge_into(&mut self.ir_delta, &other.ir_delta);
        merge_into(&mut self.ir_vega, &other.ir_vega);
        merge_into(
            &mut self.credit_qualifying_delta,
            &other.credit_qualifying_delta,
        );
        merge_into(
            &mut self.credit_non_qualifying_delta,
            &other.credit_non_qualifying_delta,
        );
        merge_into(&mut self.equity_delta, &other.equity_delta);
        merge_into(&mut self.equity_vega, &other.equity_vega);
        merge_into(&mut self.fx_delta, &other.fx_delta);
        merge_into(&mut self.fx_vega, &other.fx_vega);
        merge_into(&mut self.commodity_delta, &other.commodity_delta);
        merge_into(&mut self.curvature, &other.curvature);
        merge_into(
            &mut self.credit_qualifying_delta_bucketed,
            &other.credit_qualifying_delta_bucketed,
        );
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
    #[must_use]
    pub fn scaled_to_currency(&self, target_currency: Currency, fx_rate: f64) -> Self {
        let mut out = self.clone();
        out.base_currency = target_currency;
        if target_currency == self.base_currency {
            return out;
        }
        for v in out.ir_delta.values_mut() {
            *v *= fx_rate;
        }
        for v in out.ir_vega.values_mut() {
            *v *= fx_rate;
        }
        for v in out.credit_qualifying_delta.values_mut() {
            *v *= fx_rate;
        }
        for v in out.credit_non_qualifying_delta.values_mut() {
            *v *= fx_rate;
        }
        for v in out.equity_delta.values_mut() {
            *v *= fx_rate;
        }
        for v in out.equity_vega.values_mut() {
            *v *= fx_rate;
        }
        for v in out.fx_delta.values_mut() {
            *v *= fx_rate;
        }
        for v in out.fx_vega.values_mut() {
            *v *= fx_rate;
        }
        for v in out.commodity_delta.values_mut() {
            *v *= fx_rate;
        }
        for v in out.curvature.values_mut() {
            *v *= fx_rate;
        }
        for v in out.credit_qualifying_delta_bucketed.values_mut() {
            *v *= fx_rate;
        }
        out
    }

    /// Get total IR delta across all currencies and tenors.
    #[must_use]
    pub fn total_ir_delta(&self) -> f64 {
        self.ir_delta.values().sum()
    }

    /// Get total credit delta (qualifying + non-qualifying).
    #[must_use]
    pub fn total_credit_delta(&self) -> f64 {
        self.credit_qualifying_delta.values().sum::<f64>()
            + self.credit_non_qualifying_delta.values().sum::<f64>()
    }

    /// Get total equity delta.
    #[must_use]
    pub fn total_equity_delta(&self) -> f64 {
        self.equity_delta.values().sum()
    }
}

// ---------------------------------------------------------------------------
// Symmetric-map ordering helpers
// ---------------------------------------------------------------------------

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

    #[test]
    fn test_simm_sensitivities_creation() {
        let mut sens = SimmSensitivities::new(Currency::USD);
        assert!(sens.is_empty());

        sens.add_ir_delta(Currency::USD, "5Y", 100_000.0);
        sens.add_ir_delta(Currency::USD, "10Y", 50_000.0);
        sens.add_credit_delta("ACME_CORP", true, "5Y", 25_000.0);
        sens.add_fx_vega(Currency::EUR, Currency::USD, 1_000.0);
        sens.add_commodity_delta("energy", 2_000.0);
        sens.add_curvature(SimmRiskClass::Equity, 3_000.0);

        assert!(!sens.is_empty());
        assert_eq!(sens.total_ir_delta(), 150_000.0);
        assert_eq!(sens.total_credit_delta(), 25_000.0);
        assert_eq!(sens.fx_vega[&(Currency::EUR, Currency::USD)], 1_000.0);
        assert_eq!(sens.commodity_delta["energy"], 2_000.0);
        assert_eq!(sens.curvature[&SimmRiskClass::Equity], 3_000.0);
    }

    #[test]
    fn simm_enum_parsers_accept_binding_aliases() {
        assert_eq!(
            "rates".parse::<SimmRiskClass>().expect("risk class alias"),
            SimmRiskClass::InterestRate
        );
        assert_eq!(
            "hy_financial"
                .parse::<SimmCreditSector>()
                .expect("credit sector alias"),
            SimmCreditSector::HighYieldFinancial
        );
        assert_eq!(
            "securitised"
                .parse::<SimmCreditSector>()
                .expect("UK spelling alias"),
            SimmCreditSector::Securitized
        );
    }

    #[test]
    fn simm_sensitivities_json_round_trips_canonical_shape() {
        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_ir_delta(Currency::USD, "5Y", 100.0);
        sens.add_fx_vega(Currency::EUR, Currency::USD, 25.0);
        sens.add_commodity_delta("energy", 10.0);
        sens.add_curvature(SimmRiskClass::Equity, 5.0);

        let json = sens.to_json_pretty().expect("serialize sensitivities");
        let round_tripped = SimmSensitivities::from_json(&json).expect("deserialize sensitivities");

        assert_eq!(
            round_tripped.ir_delta[&(Currency::USD, "5Y".to_string())],
            100.0
        );
        assert_eq!(round_tripped.fx_vega[&(Currency::EUR, Currency::USD)], 25.0);
        assert_eq!(round_tripped.commodity_delta["energy"], 10.0);
        assert_eq!(round_tripped.curvature[&SimmRiskClass::Equity], 5.0);
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
        assert_eq!(SimmRiskClass::InterestRate.to_string(), "InterestRate");
        assert_eq!(
            SimmRiskClass::CreditQualifying.to_string(),
            "CreditQualifying"
        );
        assert_eq!(SimmRiskClass::Fx.to_string(), "FX");
    }
}
