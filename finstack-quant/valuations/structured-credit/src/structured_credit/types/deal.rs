//! Core structured-credit deal structs ([`StructuredCredit`], [`MarketConditions`], [`Metadata`], [`CreditModelConfig`]).

use super::*;

use finstack_quant_models::credit::pool::StochasticPrepaySpec;
use finstack_quant_models::credit::pool::{CorrelationStructure, StochasticDefaultSpec};

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};

use finstack_quant_core::dates::{BusinessDayConvention, Date, Tenor};
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::{CurveId, InstrumentId};
use serde::{Deserialize, Serialize};

/// Market conditions that affect prepayment behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MarketConditions {
    /// Finite annual decimal refinancing rate for Richard-Roll incentives; may be negative.
    pub refi_rate: f64,
}

/// Deal metadata (counterparties and identifiers).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    /// Manager identifier (for CLO).
    pub manager_id: Option<String>,
    /// Servicer identifier (for ABS/RMBS/CMBS).
    pub servicer_id: Option<String>,
    /// Master servicer identifier (for CMBS/RMBS).
    pub master_servicer_id: Option<String>,
    /// Special servicer identifier (for CMBS).
    pub special_servicer_id: Option<String>,
    /// Trustee identifier (for ABS).
    pub trustee_id: Option<String>,
}

/// Configuration for deterministic + optional stochastic credit behavior models.
///
/// This groups the "credit model" knobs that were previously exposed as many
/// top-level fields on [`StructuredCredit`]. The struct is intended to be
/// embedded via `#[serde(flatten)]` to preserve the existing JSON shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CreditModelConfig {
    /// Prepayment model specification.
    #[serde(default = "CreditModelConfig::default_prepayment_spec")]
    pub prepayment_spec: PrepaymentModelSpec,

    /// Default model specification.
    #[serde(default = "CreditModelConfig::default_default_spec")]
    pub default_spec: DefaultModelSpec,

    /// Recovery model specification.
    #[serde(default = "CreditModelConfig::default_recovery_spec")]
    pub recovery_spec: RecoveryModelSpec,

    /// Optional stochastic prepayment model specification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stochastic_prepay_spec: Option<StochasticPrepaySpec>,

    /// Optional stochastic default model specification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stochastic_default_spec: Option<StochasticDefaultSpec>,

    /// Optional stochastic recovery model for `price_stochastic`:
    /// `MarketCorrelated` recoveries move with the systematic factor (and
    /// disperse per name), so a path with heavy defaults also recovers
    /// less. `None` keeps recoveries constant at `recovery_spec.rate`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stochastic_recovery_spec: Option<finstack_quant_models::correlation::RecoverySpec>,

    /// Optional correlation structure for stochastic modeling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_structure: Option<CorrelationStructure>,

    /// Optional roll-rate delinquency model with servicer advancing and loan
    /// modification. Asset and rep-line pools only: the default model then
    /// feeds the first delinquency bucket and only the roll out of the last
    /// bucket charges off. See [`DelinquencyModel`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delinquency: Option<DelinquencyModel>,

    /// Optional credit-card master-trust portfolio model. Asset and rep-line
    /// pools only: the pool is the investor interest in the receivables, the
    /// spec's payment rate, portfolio yield and charge-off rate replace the
    /// prepayment, coupon and default assumptions. See [`CardPortfolioSpec`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<CardPortfolioSpec>,
}

/// Unified structured credit instrument representation.
///
/// This single type handles CLO, ABS, CMBS, and RMBS instruments using
/// composition for deal-specific differences.
#[derive(
    Clone, finstack_quant_valuations_macros::FinancialBuilder, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StructuredCredit {
    /// Unique instrument identifier.
    pub id: InstrumentId,

    /// Deal classification (ABS/CLO/CMBS/RMBS).
    pub deal_type: DealType,

    /// Asset pool definition.
    pub pool: AssetPool,

    /// Tranche structure.
    pub tranches: TrancheStructure,

    /// Key dates.
    /// Deal closing date (issuance).
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub closing_date: Date,
    /// First payment date to tranches.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub first_payment_date: Date,
    /// Legal final maturity date.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub maturity: Date,

    /// Buyer settlement date for clean/dirty price and spread metrics.
    /// `None` uses the valuation date. Cashflows payable on or before settlement
    /// belong to the seller; model PV remains measured on the valuation date.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub quote_settlement_date: Option<Date>,

    /// Payment frequency for the structure.
    pub frequency: Tenor,

    /// Optional payment calendar identifier for schedule adjustments.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar_id: Option<finstack_quant_core::types::CalendarId>,

    /// Business day convention for tranche payments (defaults to
    /// ModifiedFollowing).
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub business_day_convention: Option<BusinessDayConvention>,

    /// Discount curve for valuation.
    pub discount_curve_id: CurveId,

    /// Instrument-owned pricing inputs.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::ScenarioPricingOverrides,

    /// Attributes for scenario selection.
    pub attributes: Attributes,

    /// Credit model configuration (prepayment/default/recovery + optional stochastic specs).
    ///
    /// Serialized keys are flattened for flat JSON layout.
    #[builder(default)]
    #[serde(default, flatten)]
    pub credit_model: CreditModelConfig,

    /// Market conditions impacting behavior.
    #[builder(default)]
    pub market_conditions: MarketConditions,

    /// Deal metadata (counterparties, identifiers).
    #[builder(default)]
    #[serde(default)]
    pub deal_metadata: Metadata,

    /// Interest rate swaps settled through the waterfall: net receipts join
    /// interest proceeds, net payments rank as senior or junior fees. See
    /// [`HedgeSwap`].
    #[builder(default)]
    #[serde(default)]
    pub hedge_swaps: Vec<HedgeSwap>,

    /// Senior transaction fees paid ahead of every note.
    ///
    /// `None` (the default) skips the fee tier. Use [`Self::with_standard_fees`]
    /// to apply the deal-type calibration from `types/constants.rs`.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fees: Option<DealFees>,

    /// Overcollateralization / interest-coverage tests evaluated each period.
    ///
    /// Each test names the tested class, its kind and level, and what a
    /// failure does with the diverted interest. The synthesized template
    /// places each test as a [`PaymentType::CoverageTest`] tier right after
    /// the interest tier of its [`CoverageTestSpec::placement_tranche`]
    /// (CLO/CBO: per-class interest tiers; ABS/RMBS/CMBS: after the single
    /// interest tier, so only residual cash turbos). A custom
    /// [`Self::waterfall`] receives them the same way. Only cash ranked below
    /// the test position can be diverted. Empty (the default) means no
    /// coverage tests run.
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_valuations_structured_credit::structured_credit::CoverageTestSpec;
    ///
    /// // Class B must maintain 120% OC (tested on A + B) and 115% IC.
    /// let tests = vec![
    ///     CoverageTestSpec::oc("CLASS_B", 1.20),
    ///     CoverageTestSpec::ic("CLASS_B", 1.15),
    /// ];
    /// assert_eq!(tests[0].id, "OC_CLASS_B");
    /// ```
    ///
    /// Distinct from the per-tranche [`Tranche::oc_trigger`] /
    /// [`Tranche::ic_trigger`] (`structured_credit::CoverageTrigger`), which
    /// carry breach/cure memory and non-diversion consequences.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coverage_triggers: Vec<CoverageTestSpec>,

    /// Clean-up call pool factor threshold as a decimal fraction of the
    /// original balance (`0.10` = 10%).
    ///
    /// When the pool factor (current balance / original balance) drops below
    /// this threshold, the deal is optionally redeemed and all outstanding
    /// tranche balances are returned. Industry standard: typically 10%.
    ///
    /// Set to `None` to disable clean-up call (default).
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleanup_call_decimal: Option<f64>,

    /// Assumed optional redemption for price-to-call analytics. A deal-scope
    /// call liquidates the collateral at [`Self::liquidation_price_pct`] on
    /// the first payment date at or after the call date and redeems every
    /// note at the call price, ending the projection; a tranche-scope call
    /// leaves the deal's cashflows unchanged and only truncates that class's
    /// `*_to_call` metrics. `TrancheMetrics` always reports the to-maturity
    /// figures (projected without the call) next to the twins.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_assumption: Option<CallAssumption>,

    /// Scheduled lender draws on notes after closing, ascending by date
    /// ([`TrancheDraw`]); empty for a fully funded structure.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tranche_draws: Vec<TrancheDraw>,

    /// Re-advance one note each revolving period up to its commitment and the
    /// borrowing base ([`TrancheReadvance`]); `None` for no re-advances.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tranche_readvance: Option<TrancheReadvance>,

    /// Price at which the collateral is realized when the deal is called or
    /// cleaned up, as a percent of par (`None` = par). The clean-up call is
    /// only exercised when the liquidation proceeds, pending recoveries and
    /// every cash account together cover the notes' claims.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub liquidation_price_pct: Option<f64>,

    /// How collateral losses reach the note balances.
    ///
    /// `None` (the default) selects the market convention for the deal type
    /// via [`LossAllocationPolicy::default_for`]: realized-loss write-downs
    /// for RMBS and CMBS, par-preserving balances for CLO/CBO/ABS/cards. Set
    /// explicitly to override; see [`Self::effective_loss_allocation`].
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loss_allocation: Option<LossAllocationPolicy>,

    /// When a collateral loss is booked: at default or when the defaulted
    /// loan liquidates after the recovery lag.
    ///
    /// `None` (the default) selects the market convention for the deal type
    /// via [`LossRecognition::default_for`]: at liquidation for RMBS and
    /// CMBS, at default for CLO/CBO/ABS/cards. Set explicitly to override;
    /// see [`Self::effective_loss_recognition`].
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loss_recognition: Option<LossRecognition>,

    /// Whether the template waterfall pays senior fees and senior note
    /// interest shortfalls from principal proceeds before any note is
    /// redeemed (the CLO principal-waterfall convention).
    ///
    /// `None` (the default) follows the deal type: `true` for CLO/CBO,
    /// `false` otherwise. Ignored by a custom [`Self::waterfall`], whose tiers
    /// carry their own [`FundingSource`]; see
    /// [`Waterfall::fund_senior_interest_from_principal`] and
    /// [`Self::effective_principal_covers_senior_interest`].
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal_covers_senior_interest: Option<bool>,

    /// Collateral valuation rules for the OC tests: rating haircuts, the
    /// value carried for defaulted collateral, the excess-CCC bucket and
    /// discount obligations. `None` values performing collateral at par with
    /// defaulted collateral at its modeled recovery. Attached to the template
    /// waterfall by [`Self::create_waterfall`], and to a custom
    /// [`Self::waterfall`] that carries no rules of its own.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage_rules: Option<CoverageRules>,

    /// Declarative waterfall rules (available-funds caps, etc.) layered onto the
    /// base waterfall each period by the simulation engine. `None` reproduces the base
    /// waterfall exactly.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waterfall_rules: Option<WaterfallRules>,

    /// Custom payment waterfall used verbatim for pricing.
    ///
    /// `None` (the default) synthesizes the canonical sequential template from
    /// the tranche structure ([`Waterfall::standard_sequential`]) plus any
    /// [`Self::fees`]. When set, this waterfall is authoritative:
    /// [`Self::create_waterfall`] returns it (with deal-level
    /// [`Self::coverage_triggers`] appended) and no template is synthesized.
    /// [`Self::waterfall_rules`] overlays (AFC, step-down, shifting interest,
    /// controlled accumulation) still apply — they rewrite tiers generically by
    /// `payment_type`, so they compose with custom structures.
    ///
    /// The waterfall also **defines each tranche's interest claim** (not just
    /// cash allocation): an uncapped `TrancheInterest` recipient owes the full
    /// coupon accrual, a `CappedTrancheInterest` recipient owes the capped
    /// coupon (the capped-off portion is never owed and never defers), and a
    /// debt tranche with **no** interest recipient owes nothing (a
    /// principal-only class). Equity is exempt: its interest/principal split
    /// is a reporting convention driven by the tranche's metadata coupon.
    ///
    /// Constraints, enforced by `validate_invariants` and at pricing time so
    /// every construction path gets identical errors:
    /// - every tranche referenced by a tier recipient or coverage trigger must
    ///   exist in `tranches`, and tranche-keyed recipients must not target an
    ///   equity tranche (the engine records equity flows under
    ///   [`RecipientType::Equity`], paid via `ResidualCash`);
    /// - at most one interest-type recipient may name a given tranche (the
    ///   claim definition must be unambiguous);
    /// - `fees` must be `None` — senior fees are expressed as *leading*
    ///   [`PaymentType::Fee`] tiers of the custom waterfall, which then feed
    ///   the IC numerator and excess-spread/reserve sizing exactly like
    ///   template fees (fee tiers ranked below note interest are junior fees
    ///   and are deliberately not netted as senior claims);
    /// - the waterfall's `currency` must match the pool currency.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waterfall: Option<Waterfall>,
}

/// Setters for single fields of the staged [`CreditModelConfig`].
///
/// `credit_model(...)` replaces the whole configuration; these setters change
/// one field of the staged configuration (starting from
/// [`CreditModelConfig::default`] when none was staged) and leave the rest.
impl StructuredCreditBuilder {
    fn staged_credit_model(&mut self) -> &mut CreditModelConfig {
        self.credit_model
            .get_or_insert_with(CreditModelConfig::default)
    }

    /// Set the deterministic prepayment model of the staged credit model.
    ///
    /// # Arguments
    ///
    /// * `value` - Prepayment curve and speed (CPR / PSA / SMM) applied to the
    ///   collateral pool.
    pub fn prepayment_spec(mut self, value: PrepaymentModelSpec) -> Self {
        self.staged_credit_model().prepayment_spec = value;
        self
    }

    /// Set the deterministic default model of the staged credit model.
    ///
    /// # Arguments
    ///
    /// * `value` - Default curve and rate (CDR / SDA / MDR) applied to the
    ///   collateral pool.
    pub fn default_spec(mut self, value: DefaultModelSpec) -> Self {
        self.staged_credit_model().default_spec = value;
        self
    }

    /// Set the deterministic recovery model of the staged credit model.
    ///
    /// # Arguments
    ///
    /// * `value` - Recovery rate (decimal fraction of defaulted par) and the
    ///   recovery lag in months.
    pub fn recovery_spec(mut self, value: RecoveryModelSpec) -> Self {
        self.staged_credit_model().recovery_spec = value;
        self
    }

    /// Set the stochastic prepayment model used by stochastic pricing.
    ///
    /// # Arguments
    ///
    /// * `value` - Factor-driven prepayment specification; replaces any
    ///   previously staged one.
    pub fn stochastic_prepay_spec(mut self, value: StochasticPrepaySpec) -> Self {
        self.staged_credit_model().stochastic_prepay_spec = Some(value);
        self
    }

    /// Set the stochastic default model used by stochastic pricing.
    ///
    /// # Arguments
    ///
    /// * `value` - Factor-driven default specification; replaces any
    ///   previously staged one.
    pub fn stochastic_default_spec(mut self, value: StochasticDefaultSpec) -> Self {
        self.staged_credit_model().stochastic_default_spec = Some(value);
        self
    }

    /// Set the stochastic recovery model used by stochastic pricing.
    ///
    /// # Arguments
    ///
    /// * `value` - Recovery specification (constant or market-correlated);
    ///   without one, recoveries stay at `recovery_spec.rate`.
    pub fn stochastic_recovery_spec(
        mut self,
        value: finstack_quant_models::correlation::RecoverySpec,
    ) -> Self {
        self.staged_credit_model().stochastic_recovery_spec = Some(value);
        self
    }

    /// Set the correlation structure used by stochastic pricing.
    ///
    /// # Arguments
    ///
    /// * `value` - Asset and prepayment-default correlation structure of the
    ///   collateral pool.
    pub fn correlation_structure(mut self, value: CorrelationStructure) -> Self {
        self.staged_credit_model().correlation_structure = Some(value);
        self
    }

    /// Set the roll-rate delinquency model (asset and rep-line pools only).
    ///
    /// # Arguments
    ///
    /// * `value` - Delinquency buckets, roll rates, servicer advancing and
    ///   loan-modification assumptions.
    pub fn delinquency(mut self, value: super::DelinquencyModel) -> Self {
        self.staged_credit_model().delinquency = Some(value);
        self
    }

    /// Set the credit-card master-trust portfolio model (asset and rep-line
    /// pools only).
    ///
    /// # Arguments
    ///
    /// * `value` - Payment rate, portfolio yield and charge-off rate that
    ///   replace the prepayment, coupon and default assumptions.
    pub fn card(mut self, value: super::CardPortfolioSpec) -> Self {
        self.staged_credit_model().card = Some(value);
        self
    }
}

#[cfg(test)]
mod builder_credit_model_tests {
    use super::*;

    #[test]
    fn field_setters_edit_the_staged_credit_model() {
        let prepayment = PrepaymentModelSpec::constant_cpr(0.123);
        let default = DefaultModelSpec::constant_cdr(0.045);
        let builder = StructuredCredit::builder()
            .prepayment_spec(prepayment.clone())
            .default_spec(default.clone());
        let staged = builder.credit_model.as_ref().expect("staged credit model");
        assert_eq!(staged.prepayment_spec, prepayment);
        assert_eq!(staged.default_spec, default);
        // The fields that were not set keep the defaults.
        assert_eq!(
            staged.recovery_spec.rate,
            CreditModelConfig::default().recovery_spec.rate
        );

        // `credit_model` replaces the whole staged configuration.
        let replaced = builder.credit_model(CreditModelConfig::default());
        assert_eq!(
            replaced
                .credit_model
                .as_ref()
                .expect("credit model")
                .prepayment_spec,
            CreditModelConfig::default().prepayment_spec
        );
    }
}
