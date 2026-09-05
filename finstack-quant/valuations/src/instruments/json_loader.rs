//! JSON import/export for financial instruments.
//!
//! This module provides a tagged union for all instrument types and helpers
//! for loading instruments from JSON files with strict validation.

use super::*;
use finstack_quant_core::contract::{
    deserialize_json_value, parse_json_value, ContractDescriptor, ContractError, LoadLimits,
    ValidationReport,
};
use finstack_quant_core::Result;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::sync::Arc;

/// Maximum permitted size of a JSON instrument definition, in bytes.
///
/// 16 MiB is far larger than any realistic single-instrument JSON payload
/// (the largest observed real-world instruments are well under 1 MiB), but
/// small enough to prevent unbounded allocations from malicious or
/// accidentally huge inputs.  Reader-based entry points (`from_reader`,
/// `from_path`) enforce this limit before handing bytes to the JSON parser,
/// so a multi-gigabyte file can never cause an OOM allocation.
pub const MAX_JSON_BYTES: usize = 16 * 1024 * 1024; // 16 MiB

/// Persistence contract for [`InstrumentEnvelope`].
pub const INSTRUMENT_CONTRACT: ContractDescriptor =
    ContractDescriptor::new("finstack_quant.instrument");

/// Canonical schema marker for persisted instrument envelopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum InstrumentSchema {
    /// The sole supported instrument contract.
    #[serde(rename = "finstack_quant.instrument/1")]
    Instrument,
}

impl InstrumentSchema {
    /// The exact marker required by every persisted instrument envelope.
    pub const CURRENT: Self = Self::Instrument;

    /// Return the persisted schema marker.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Instrument => "finstack_quant.instrument/1",
        }
    }
}

impl std::fmt::Display for InstrumentSchema {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Versioned envelope for JSON instrument definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct InstrumentEnvelope {
    /// Required v1 instrument contract marker.
    pub schema: InstrumentSchema,
    /// The instrument definition
    pub instrument: InstrumentJson,
}

/// One-line summary per instrument tag for the schema index.
///
/// Every generated single-instrument artifact otherwise shares one templated
/// description, which makes the index unable to tell `cms_option` from
/// `cms_spread_option`. The registry test asserts this covers every tag.
#[cfg(feature = "json-schema")]
pub(crate) fn instrument_summary(tag: &str) -> &'static str {
    match tag {
        "agency_cmo" => "Agency CMO tranche carved from a collateral pool.",
        "agency_mbs_passthrough" => "Agency mortgage pass-through priced off a prepayment model.",
        "agency_tba" => "To-be-announced agency MBS forward trade.",
        "asian_option" => "Option on an average of observed fixings.",
        "autocallable" => "Structured note that redeems early when a barrier is met.",
        "barrier_option" => "Knock-in or knock-out option on a monitored barrier.",
        "basis_swap" => "Floating-versus-floating swap between two indices or tenors.",
        "basket" => "Weighted basket of constituents priced as one position.",
        "bermudan_swaption" => "Bermudan-exercise swaption valued on a short-rate lattice.",
        "bond" => "Fixed- or floating-rate cash bond with a coupon schedule.",
        "bond_future" => "Exchange-traded bond future with a deliverable basket and CTD.",
        "callable_range_accrual" => "Range accrual with issuer call rights.",
        "cap_floor" => "Strip of caplets or floorlets on a floating index.",
        "cds_index" => "Standardised CDS index such as CDX or iTraxx.",
        "cds_option" => "Payer or receiver option on a CDS or CDS index.",
        "cds_tranche" => "Synthetic index tranche priced with a correlation model.",
        "cliquet_option" => "Series of forward-starting options with local and global caps.",
        "cms_option" => "Option on a constant-maturity swap rate.",
        "cms_spread_option" => "Option on the spread between two CMS rates.",
        "cms_swap" => "Swap paying a constant-maturity swap rate.",
        "commodity_asian_option" => "Commodity option on an average settlement price.",
        "commodity_forward" => "Forward on a commodity reference price.",
        "commodity_option" => "Option on a commodity forward or future.",
        "commodity_spread_option" => "Option on the spread between two commodity legs.",
        "commodity_swap" => "Fixed-versus-floating commodity swap on an average price.",
        "commodity_swaption" => "Option to enter a commodity swap.",
        "composite" => "Resolved cross-asset synthetic instrument with immutable leg quantities.",
        "convertible_bond" => "Corporate bond convertible into a fixed number of shares.",
        "credit_default_swap" => "Single-name CDS priced on the ISDA standard model.",
        "deposit" => "Money-market deposit quoted as a simple rate.",
        "discounted_cash_flow" => {
            "Enterprise DCF valuation with an explicit forecast and terminal value."
        }
        "dollar_roll" => "Simultaneous TBA sale and forward repurchase.",
        "equity" => "Cash equity position.",
        "equity_future" => "Listed equity future with optional fixed-currency quanto.",
        "equity_total_return_future" => "Listed equity total-return future and financing spread.",
        "equity_option" => "European or American option on a single equity.",
        "forward_rate_agreement" => "Single-period forward rate agreement.",
        "fx_barrier_option" => "FX option with a knock-in or knock-out barrier.",
        "fx_digital_option" => "FX option paying a fixed amount if in the money.",
        "fx_forward" => "Outright FX forward.",
        "fx_future" => "Exchange-listed future on a deliverable currency pair.",
        "fx_option" => "Vanilla European FX option.",
        "fx_spot" => "Spot foreign-exchange trade.",
        "fx_swap" => "Near- and far-leg FX swap.",
        "fx_touch_option" => "FX option paying on barrier touch or no-touch.",
        "fx_variance_swap" => "Swap on realised FX variance.",
        "inflation_cap_floor" => "Cap or floor on realised inflation.",
        "inflation_linked_bond" => "Bond whose principal and coupons index to a price level.",
        "inflation_swap" => "Zero-coupon inflation swap on a price index.",
        "interest_rate_future" => "Exchange-traded short-rate future.",
        "interest_rate_future_option" => {
            "Listed or bilateral option on an interest-rate futures price."
        }
        "interest_rate_swap" => "Vanilla fixed-versus-floating interest-rate swap.",
        "levered_real_estate_equity" => "Property equity net of its financing stack.",
        "lookback_option" => "Option struck on the path maximum or minimum.",
        "ndf" => "Non-deliverable forward cash-settled against a fixing.",
        "private_markets_fund" => {
            "Closed-end fund with a commitment schedule and distribution waterfall."
        }
        "quanto_option" => {
            "Option on a foreign asset settled in the domestic currency at a fixed rate."
        }
        "commodity_future" => "Linear listed future on a single or average observed price.",
        "commodity_future_option" => "Exchange-listed option on a commodity future.",
        "equity_future_option" => "Exchange-listed option on an equity future.",
        "fx_future_option" => "Exchange-listed option on an FX future.",
        "range_accrual" => "Coupon accruing only on days the index sits inside a range.",
        "real_estate_asset" => "Direct property asset with rent roll and exit assumptions.",
        "repo" => "Repurchase agreement against posted collateral.",
        "revolving_credit" => "Committed revolver with drawn/undrawn balances and commitment fees.",
        "snowball" => "Path-dependent note whose coupon builds on the previous one.",
        "structured_credit" => "ABS/CLO/RMBS/CMBS deal: collateral pool, tranches and waterfall.",
        "swaption" => "European option to enter an interest-rate swap.",
        "tarn" => "Target-redemption note that terminates once cumulative coupons hit a cap.",
        "term_loan" => "Amortising leveraged loan with call protection and covenants.",
        "trs_equity" => "Equity total-return swap versus a funding leg.",
        "trs_fixed_income_index" => {
            "Total-return swap on a fixed-income index versus a funding leg."
        }
        "variance_swap" => "Swap on realised variance against a strike.",
        "volatility_index_future" => "Future on a volatility index such as VIX.",
        "volatility_index_future_option" => "Exchange-listed option on a volatility-index future.",
        "xccy_swap" => "Cross-currency swap with notional exchange and a basis spread.",
        "yoy_inflation_swap" => "Year-on-year inflation swap.",
        _ => "Canonical single-instrument v1 envelope.",
    }
}

macro_rules! with_instrument_json_registry {
    ($callback:ident $(, $extra:expr )* $(,)?) => {
        $callback! {
            [$($extra),*]
            plain: Bond(Bond) => "bond" @ "fixed_income" = Bond::example();
            plain: ConvertibleBond(ConvertibleBond) => "convertible_bond" @ "fixed_income" = ConvertibleBond::example();
            plain: InflationLinkedBond(InflationLinkedBond) => "inflation_linked_bond" @ "fixed_income" = infallible_example(InflationLinkedBond::example());
            plain: TermLoan(TermLoan) => "term_loan" @ "fixed_income" = TermLoan::example();
            plain: RevolvingCredit(RevolvingCredit) => "revolving_credit" @ "fixed_income" = RevolvingCredit::example();
            plain: AgencyMbsPassthrough(AgencyMbsPassthrough) => "agency_mbs_passthrough" @ "fixed_income" = AgencyMbsPassthrough::example();
            plain: AgencyTba(AgencyTba) => "agency_tba" @ "fixed_income" = AgencyTba::example();
            plain: AgencyCmo(AgencyCmo) => "agency_cmo" @ "fixed_income" = AgencyCmo::example();
            plain: DollarRoll(DollarRoll) => "dollar_roll" @ "fixed_income" = DollarRoll::example();
            plain: InterestRateSwap(InterestRateSwap) => "interest_rate_swap" @ "rates" = InterestRateSwap::example_standard();
            plain: BasisSwap(BasisSwap) => "basis_swap" @ "rates" = BasisSwap::example();
            plain: XccySwap(XccySwap) => "xccy_swap" @ "rates" = infallible_example(XccySwap::example());
            plain: InflationSwap(InflationSwap) => "inflation_swap" @ "rates" = infallible_example(InflationSwap::example());
            plain: YoYInflationSwap(YoYInflationSwap) => "yoy_inflation_swap" @ "rates" = infallible_example(YoYInflationSwap::example());
            plain: InflationCapFloor(InflationCapFloor) => "inflation_cap_floor" @ "rates" = infallible_example(InflationCapFloor::example());
            plain: ForwardRateAgreement(ForwardRateAgreement) => "forward_rate_agreement" @ "rates" = ForwardRateAgreement::example();
            plain: Swaption(Swaption) => "swaption" @ "rates" = infallible_example(Swaption::example());
            plain: BermudanSwaption(BermudanSwaption) => "bermudan_swaption" @ "rates" = infallible_example(BermudanSwaption::example());
            plain: InterestRateFuture(InterestRateFuture) => "interest_rate_future" @ "rates" = InterestRateFuture::example();
            plain: InterestRateFutureOption(InterestRateFutureOption) => "interest_rate_future_option" @ "rates" = InterestRateFutureOption::example();
            plain: CapFloor(CapFloor) => "cap_floor" @ "rates" = CapFloor::example();
            plain: CmsSwap(CmsSwap) => "cms_swap" @ "rates" = infallible_example(CmsSwap::example());
            plain: CmsOption(CmsOption) => "cms_option" @ "rates" = infallible_example(CmsOption::example());
            plain: Deposit(Deposit) => "deposit" @ "rates" = Deposit::example();
            plain: Repo(Repo) => "repo" @ "rates" = infallible_example(Repo::example());
            plain: CreditDefaultSwap(CreditDefaultSwap) => "credit_default_swap" @ "credit_derivatives" = infallible_example(CreditDefaultSwap::example());
            plain: CDSIndex(CDSIndex) => "cds_index" @ "credit_derivatives" = infallible_example(CDSIndex::example());
            plain: CDSTranche(CDSTranche) => "cds_tranche" @ "credit_derivatives" = infallible_example(CDSTranche::example());
            plain: CDSOption(CDSOption) => "cds_option" @ "credit_derivatives" = CDSOption::example();
            plain: Equity(Equity) => "equity" @ "equity" = infallible_example(Equity::example());
            plain: EquityOption(EquityOption) => "equity_option" @ "equity" = EquityOption::example();
            plain: AsianOption(AsianOption) => "asian_option" @ "exotics" = AsianOption::example();
            plain: BarrierOption(BarrierOption) => "barrier_option" @ "exotics" = BarrierOption::example();
            plain: LookbackOption(LookbackOption) => "lookback_option" @ "exotics" = LookbackOption::example();
            plain: VarianceSwap(VarianceSwap) => "variance_swap" @ "equity" = VarianceSwap::example();
            plain: VolatilityIndexFuture(VolatilityIndexFuture) => "volatility_index_future" @ "equity" = VolatilityIndexFuture::example();
            plain: VolatilityIndexFutureOption(VolatilityIndexFutureOption) => "volatility_index_future_option" @ "equity" = VolatilityIndexFutureOption::example();
            plain: FxSpot(FxSpot) => "fx_spot" @ "fx" = FxSpot::example();
            plain: FxSwap(FxSwap) => "fx_swap" @ "fx" = infallible_example(FxSwap::example());
            plain: FxForward(FxForward) => "fx_forward" @ "fx" = FxForward::example();
            plain: Ndf(Ndf) => "ndf" @ "fx" = infallible_example(Ndf::example());
            plain: FxOption(FxOption) => "fx_option" @ "fx" = FxOption::example();
            plain: FxDigitalOption(FxDigitalOption) => "fx_digital_option" @ "fx" = FxDigitalOption::example();
            plain: FxTouchOption(FxTouchOption) => "fx_touch_option" @ "fx" = FxTouchOption::example();
            plain: FxBarrierOption(FxBarrierOption) => "fx_barrier_option" @ "fx" = infallible_example(FxBarrierOption::example());
            plain: FxVarianceSwap(FxVarianceSwap) => "fx_variance_swap" @ "fx" = infallible_example(FxVarianceSwap::example());
            plain: QuantoOption(QuantoOption) => "quanto_option" @ "fx" = infallible_example(QuantoOption::example());
            plain: CommodityOption(CommodityOption) => "commodity_option" @ "commodity" = infallible_example(CommodityOption::example());
            plain: CommodityAsianOption(CommodityAsianOption) => "commodity_asian_option" @ "commodity" = infallible_example(CommodityAsianOption::example());
            plain: CommodityForward(CommodityForward) => "commodity_forward" @ "commodity" = infallible_example(CommodityForward::example());
            plain: CommoditySwap(CommoditySwap) => "commodity_swap" @ "commodity" = infallible_example(CommoditySwap::example());
            plain: CommoditySwaption(CommoditySwaption) => "commodity_swaption" @ "commodity" = infallible_example(CommoditySwaption::example());
            plain: CommoditySpreadOption(CommoditySpreadOption) => "commodity_spread_option" @ "commodity" = CommoditySpreadOption::example();
            plain: CommodityFuture(CommodityFuture) => "commodity_future" @ "commodity" = CommodityFuture::example();
            plain: CommodityFutureOption(CommodityFutureOption) => "commodity_future_option" @ "commodity" = CommodityFutureOption::example();
            plain: FxFuture(FxFuture) => "fx_future" @ "fx" = FxFuture::example();
            plain: FxFutureOption(FxFutureOption) => "fx_future_option" @ "fx" = FxFutureOption::example();
            plain: EquityFuture(EquityFuture) => "equity_future" @ "equity" = EquityFuture::example();
            plain: EquityFutureOption(EquityFutureOption) => "equity_future_option" @ "equity" = EquityFutureOption::example();
            plain: EquityTotalReturnFuture(EquityTotalReturnFuture) => "equity_total_return_future" @ "equity" = EquityTotalReturnFuture::example();
            plain: Autocallable(Autocallable) => "autocallable" @ "equity" = Autocallable::example();
            plain: CliquetOption(CliquetOption) => "cliquet_option" @ "equity" = CliquetOption::example();
            plain: RangeAccrual(RangeAccrual) => "range_accrual" @ "exotics" = infallible_example(RangeAccrual::example());
            plain: Tarn(Tarn) => "tarn" @ "exotics" = infallible_example(Tarn::example());
            plain: Snowball(Snowball) => "snowball" @ "exotics" = infallible_example(Snowball::example_snowball());
            plain: CmsSpreadOption(CmsSpreadOption) => "cms_spread_option" @ "rates" = infallible_example(CmsSpreadOption::example());
            plain: TrsEquity(EquityTotalReturnSwap) => "trs_equity" @ "equity" = EquityTotalReturnSwap::example();
            plain: TrsFixedIncomeIndex(FIIndexTotalReturnSwap) => "trs_fixed_income_index" @ "fixed_income" = FIIndexTotalReturnSwap::example();
            plain: Basket(Basket) => "basket" @ "exotics" = Basket::example();
            plain: PrivateMarketsFund(PrivateMarketsFund) => "private_markets_fund" @ "equity" = PrivateMarketsFund::example();
            plain: RealEstateAsset(RealEstateAsset) => "real_estate_asset" @ "equity" = RealEstateAsset::example();
            plain: DiscountedCashFlow(DiscountedCashFlow) => "discounted_cash_flow" @ "equity" = DiscountedCashFlow::example();
            boxed: CallableRangeAccrual(CallableRangeAccrual) => "callable_range_accrual" @ "exotics" = infallible_example(CallableRangeAccrual::example());
            boxed: BondFuture(BondFuture) => "bond_future" @ "fixed_income" = BondFuture::example();
            boxed: StructuredCredit(StructuredCredit) => "structured_credit" @ "fixed_income" = infallible_example(StructuredCredit::example());
            boxed: LeveredRealEstateEquity(crate::instruments::equity::real_estate::LeveredRealEstateEquity) => "levered_real_estate_equity" @ "equity" = crate::instruments::equity::real_estate::LeveredRealEstateEquity::example();
            boxed: Composite(CompositeInstrument) => "composite" @ "composite" = CompositeInstrument::example();
        }
    };
}
macro_rules! define_instrument_json {
    (
        []
        $(plain: $variant:ident($ty:ty) => $tag:literal @ $category:literal = $example:expr;)*
        $(boxed: $boxed_variant:ident($boxed_ty:ty) => $boxed_tag:literal @ $boxed_category:literal = $boxed_example:expr;)*
    ) => {
        mod instrument_variant {
            $(
                #[allow(non_snake_case)]
                pub(crate) mod $variant {
                    use super::super::*;

                    #[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
                    #[serde(
                        rename_all = "snake_case",
                        tag = "type",
                        content = "spec",
                        deny_unknown_fields
                    )]
                    #[cfg_attr(feature = "json-schema", schemars(rename = $tag))]
                    pub(crate) enum Wire<'a> {
                        #[serde(rename = $tag)]
                        Instrument(std::borrow::Cow<'a, $ty>),
                    }
                }
            )*
            $(
                #[allow(non_snake_case)]
                pub(crate) mod $boxed_variant {
                    use super::super::*;

                    #[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
                    #[serde(
                        rename_all = "snake_case",
                        tag = "type",
                        content = "spec",
                        deny_unknown_fields
                    )]
                    #[cfg_attr(feature = "json-schema", schemars(rename = $boxed_tag))]
                    pub(crate) enum Wire<'a> {
                        #[serde(rename = $boxed_tag)]
                        Instrument(std::borrow::Cow<'a, $boxed_ty>),
                    }
                }
            )*
        }

        #[derive(Deserialize)]
        #[serde(
            rename_all = "snake_case",
            tag = "type",
            content = "spec",
            deny_unknown_fields
        )]
        enum InstrumentJsonWire {
            $(
                #[serde(rename = $tag)]
                $variant($ty),
            )*
            $(
                #[serde(rename = $boxed_tag)]
                $boxed_variant(Box<$boxed_ty>),
            )*
        }

        /// Canonical tagged union of all supported instrument serde types.
        #[derive(Debug, Clone)]
        #[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
        #[cfg_attr(
            feature = "json-schema",
            serde(tag = "type", content = "spec", deny_unknown_fields)
        )]
        #[non_exhaustive]
        pub enum InstrumentJson {
            $(
                #[doc = concat!("Canonical `", $tag, "` instrument payload.")]
                #[cfg_attr(feature = "json-schema", serde(rename = $tag))]
                $variant($ty),
            )*
            $(
                #[doc = concat!("Canonical `", $boxed_tag, "` instrument payload.")]
                #[cfg_attr(feature = "json-schema", serde(rename = $boxed_tag))]
                $boxed_variant(Box<$boxed_ty>),
            )*
        }

        impl Serialize for InstrumentJson {
            fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                match self {
                    $(Self::$variant(instrument) => {
                        instrument_variant::$variant::Wire::Instrument(
                            std::borrow::Cow::Borrowed(instrument),
                        )
                        .serialize(serializer)
                    })*
                    $(Self::$boxed_variant(instrument) => {
                        instrument_variant::$boxed_variant::Wire::Instrument(
                            std::borrow::Cow::Borrowed(instrument.as_ref()),
                        )
                        .serialize(serializer)
                    })*
                }
            }
        }

        impl<'de> Deserialize<'de> for InstrumentJson {
            fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                match InstrumentJsonWire::deserialize(deserializer)? {
                    $(InstrumentJsonWire::$variant(instrument) => Ok(Self::$variant(instrument)),)*
                    $(InstrumentJsonWire::$boxed_variant(instrument) => Ok(Self::$boxed_variant(instrument)),)*
                }
            }
        }
    };
}

with_instrument_json_registry!(define_instrument_json);

macro_rules! instrument_json_registry_tags {
    (
        []
        $(plain: $variant:ident($ty:ty) => $tag:literal @ $category:literal = $example:expr;)*
        $(boxed: $boxed_variant:ident($boxed_ty:ty) => $boxed_tag:literal @ $boxed_category:literal = $boxed_example:expr;)*
    ) => {
        &[$($tag,)* $($boxed_tag,)*]
    };
}

/// Return every canonical instrument discriminator.
///
/// The returned slice is generated from the same registry that emits the
/// serde enum, boxed-instrument construction, and embedded schema lookup.
#[must_use]
pub fn registry_tags() -> &'static [&'static str] {
    with_instrument_json_registry!(instrument_json_registry_tags)
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct SingleInstrumentEnvelope<T> {
    schema: InstrumentSchema,
    instrument: T,
}

/// Complete generated-contract metadata for one instrument discriminator.
pub struct InstrumentRegistryEntry {
    /// Canonical persisted `type` value.
    pub tag: &'static str,
    /// Stable asset-class directory below `schemas/instruments/1`.
    pub category: &'static str,
    /// Schema artifact generated from the concrete serde type.
    #[cfg(feature = "json-schema")]
    pub artifact: finstack_quant_core::schema::SchemaArtifact,
    /// Canonical generated fixture path relative to this crate.
    pub fixture_path: &'static str,
    examples: fn() -> Result<Vec<serde_json::Value>>,
    embedded_schema: fn() -> std::result::Result<serde_json::Value, String>,
}

impl InstrumentRegistryEntry {
    /// Generate deterministic examples from the instrument's Rust provider.
    ///
    /// # Errors
    ///
    /// Returns an error when the Rust example cannot be constructed,
    /// serialized, or deserialized through its single-instrument envelope.
    pub fn examples(&self) -> Result<Vec<serde_json::Value>> {
        (self.examples)()
    }

    pub(crate) fn load_embedded_schema(&self) -> std::result::Result<serde_json::Value, String> {
        (self.embedded_schema)()
    }
}

fn schema_example_error(
    context: &str,
    error: impl std::fmt::Display,
) -> finstack_quant_core::Error {
    finstack_quant_core::Error::Internal(format!("{context}: {error}"))
}

fn infallible_example<T>(value: T) -> Result<T> {
    Ok(value)
}

macro_rules! instrument_registry_entries {
    (
        []
        $(plain: $variant:ident($ty:ty) => $tag:literal @ $category:literal = $example:expr;)*
        $(boxed: $boxed_variant:ident($boxed_ty:ty) => $boxed_tag:literal @ $boxed_category:literal = $boxed_example:expr;)*
    ) => {{
        let mut entries = Vec::new();
        $(
            entries.push({
                fn examples() -> Result<Vec<serde_json::Value>> {
                    let spec: $ty = $example?;
                    let value = serde_json::to_value(InstrumentEnvelope::new(
                        InstrumentJson::$variant(spec),
                    ))
                    .map_err(|error| {
                        schema_example_error(concat!("serialize ", $tag, " example"), error)
                    })?;
                    serde_json::from_value::<SingleInstrumentEnvelope<
                        instrument_variant::$variant::Wire<'static>,
                    >>(value.clone())
                    .map_err(|error| {
                        schema_example_error(concat!("validate ", $tag, " example"), error)
                    })?;
                    Ok(vec![value])
                }

                fn embedded_schema() -> std::result::Result<serde_json::Value, String> {
                    serde_json::from_str(include_str!(concat!(
                        "../../schemas/instruments/1/",
                        $category,
                        "/",
                        $tag,
                        ".schema.json"
                    )))
                    .map_err(|error| format!("invalid embedded schema for {}: {error}", $tag))
                }

                InstrumentRegistryEntry {
                    tag: $tag,
                    category: $category,
                    #[cfg(feature = "json-schema")]
                    artifact: finstack_quant_core::schema::SchemaArtifact::new::<
                        SingleInstrumentEnvelope<instrument_variant::$variant::Wire<'static>>,
                    >(
                        concat!("schemas/instruments/1/", $category, "/", $tag, ".schema.json"),
                        concat!("https://finstack_quant.dev/schemas/instrument/1/", $category, "/", $tag, ".schema.json"),
                        $tag,
                        "Canonical single-instrument v1 envelope generated from its serde type.",
                    )
                    .with_packager(crate::schema::package_valuations_schema)
                    .with_kind(finstack_quant_core::schema::SchemaKind::Input)
                    .with_summary(instrument_summary($tag))
                    .with_examples(examples),
                    fixture_path: concat!("tests/instruments/json_examples/", $tag, ".json"),
                    examples,
                    embedded_schema,
                }
            });
        )*
        $(
            entries.push({
                fn examples() -> Result<Vec<serde_json::Value>> {
                    let spec: $boxed_ty = $boxed_example?;
                    let value = serde_json::to_value(InstrumentEnvelope::new(
                        InstrumentJson::$boxed_variant(Box::new(spec)),
                    ))
                    .map_err(|error| {
                        schema_example_error(
                            concat!("serialize ", $boxed_tag, " example"),
                            error,
                        )
                    })?;
                    serde_json::from_value::<SingleInstrumentEnvelope<
                        instrument_variant::$boxed_variant::Wire<'static>,
                    >>(value.clone())
                    .map_err(|error| {
                        schema_example_error(
                            concat!("validate ", $boxed_tag, " example"),
                            error,
                        )
                    })?;
                    Ok(vec![value])
                }

                fn embedded_schema() -> std::result::Result<serde_json::Value, String> {
                    serde_json::from_str(include_str!(concat!(
                        "../../schemas/instruments/1/",
                        $boxed_category,
                        "/",
                        $boxed_tag,
                        ".schema.json"
                    )))
                    .map_err(|error| {
                        format!("invalid embedded schema for {}: {error}", $boxed_tag)
                    })
                }

                InstrumentRegistryEntry {
                    tag: $boxed_tag,
                    category: $boxed_category,
                    #[cfg(feature = "json-schema")]
                    artifact: finstack_quant_core::schema::SchemaArtifact::new::<
                        SingleInstrumentEnvelope<instrument_variant::$boxed_variant::Wire<'static>>,
                    >(
                        concat!("schemas/instruments/1/", $boxed_category, "/", $boxed_tag, ".schema.json"),
                        concat!("https://finstack_quant.dev/schemas/instrument/1/", $boxed_category, "/", $boxed_tag, ".schema.json"),
                        $boxed_tag,
                        "Canonical single-instrument v1 envelope generated from its serde type.",
                    )
                    .with_packager(crate::schema::package_valuations_schema)
                    .with_kind(finstack_quant_core::schema::SchemaKind::Input)
                    .with_summary(instrument_summary($boxed_tag))
                    .with_examples(examples),
                    fixture_path: concat!("tests/instruments/json_examples/", $boxed_tag, ".json"),
                    examples,
                    embedded_schema,
                }
            });
        )*
        entries
    }};
}

/// Build the canonical instrument registry used by serde, schemas, fixtures,
/// runtime schema lookup, and binding tag exposure.
#[must_use]
pub fn instrument_registry() -> Vec<InstrumentRegistryEntry> {
    with_instrument_json_registry!(instrument_registry_entries)
}

macro_rules! instrument_json_into_boxed_match {
    (
        [$instrument_json:expr]
        $(plain: $variant:ident($ty:ty) => $tag:literal @ $category:literal = $example:expr;)*
        $(boxed: $boxed_variant:ident($boxed_ty:ty) => $boxed_tag:literal @ $boxed_category:literal = $boxed_example:expr;)*
    ) => {
        match $instrument_json {
            $(InstrumentJson::$variant(instrument) => Ok::<Box<dyn Instrument>, finstack_quant_core::Error>(Box::new(instrument) as Box<dyn Instrument>),)*
            $(InstrumentJson::$boxed_variant(instrument) => Ok::<Box<dyn Instrument>, finstack_quant_core::Error>(Box::new(*instrument) as Box<dyn Instrument>),)*
        }
    };
}

macro_rules! instrument_json_as_instrument_match {
    (
        [$instrument_json:expr]
        $(plain: $variant:ident($ty:ty) => $tag:literal @ $category:literal = $example:expr;)*
        $(boxed: $boxed_variant:ident($boxed_ty:ty) => $boxed_tag:literal @ $boxed_category:literal = $boxed_example:expr;)*
    ) => {
        match $instrument_json {
            $(InstrumentJson::$variant(instrument) => instrument as &dyn Instrument,)*
            $(InstrumentJson::$boxed_variant(instrument) => instrument.as_ref() as &dyn Instrument,)*
        }
    };
}

macro_rules! instrument_json_from_any_match {
    (
        [$value:expr]
        $(plain: $variant:ident($ty:ty) => $tag:literal @ $category:literal = $example:expr;)*
        $(boxed: $boxed_variant:ident($boxed_ty:ty) => $boxed_tag:literal @ $boxed_category:literal = $boxed_example:expr;)*
    ) => {{
        $(
            if let Some(instrument) = $value.downcast_ref::<$ty>() {
                return Some(InstrumentJson::$variant(instrument.clone()));
            }
        )*
        $(
            if let Some(instrument) = $value.downcast_ref::<$boxed_ty>() {
                return Some(InstrumentJson::$boxed_variant(Box::new(instrument.clone())));
            }
        )*
        None
    }};
}

pub(crate) fn instrument_json_from_any(value: &dyn std::any::Any) -> Option<InstrumentJson> {
    with_instrument_json_registry!(instrument_json_from_any_match, value)
}

macro_rules! instrument_json_type_tag_match {
    (
        [$value:expr]
        $(plain: $variant:ident($ty:ty) => $tag:literal @ $category:literal = $example:expr;)*
        $(boxed: $boxed_variant:ident($boxed_ty:ty) => $boxed_tag:literal @ $boxed_category:literal = $boxed_example:expr;)*
    ) => {
        match $value {
            $(InstrumentJson::$variant(_) => $tag,)*
            $(InstrumentJson::$boxed_variant(_) => $boxed_tag,)*
        }
    };
}

impl InstrumentJson {
    /// Return this payload's canonical persisted `type` discriminator.
    #[must_use]
    pub const fn type_tag(&self) -> &'static str {
        with_instrument_json_registry!(instrument_json_type_tag_match, self)
    }

    /// Validate this payload without cloning or consuming its concrete instrument.
    pub(crate) fn validate_for_pricing(&self) -> Result<()> {
        let instrument = with_instrument_json_registry!(instrument_json_as_instrument_match, self);
        instrument.validate_for_pricing()
    }

    /// Convert this JSON representation into a boxed instrument trait object.
    ///
    /// For instruments using a Spec pattern (e.g., TermLoan), this performs
    /// the spec-to-runtime conversion. For direct instrument types, it boxes
    /// them immediately.
    ///
    /// # Errors
    ///
    /// Returns an error if spec validation fails during conversion.
    pub fn into_boxed(self) -> Result<Box<dyn Instrument>> {
        self.validate_for_pricing()?;
        self.into_boxed_assuming_validated()
    }

    /// Box a payload that has already passed [`Self::validate_for_pricing`].
    pub(crate) fn into_boxed_assuming_validated(self) -> Result<Box<dyn Instrument>> {
        with_instrument_json_registry!(instrument_json_into_boxed_match, self)
    }

    /// Convert this JSON representation into a shared cashflow provider.
    ///
    /// # Errors
    ///
    /// Returns an error if the instrument fails construction or pricing
    /// validation.
    pub fn into_cashflow_provider(
        self,
    ) -> Result<Arc<dyn finstack_quant_cashflows::CashflowProvider + Send + Sync>> {
        let instrument = self.into_boxed()?;
        let provider: Box<dyn finstack_quant_cashflows::CashflowProvider + Send + Sync> =
            instrument;
        Ok(Arc::from(provider))
    }
}

impl InstrumentEnvelope {
    /// Current schema version emitted by Finstack instrument envelopes.
    pub const CURRENT_SCHEMA: &'static str = "finstack_quant.instrument/1";

    /// Create the canonical v1 envelope for an instrument payload.
    ///
    /// # Arguments
    ///
    /// * `instrument` - Fully specified tagged instrument payload to wrap with
    ///   the current persistence schema marker.
    #[must_use]
    pub fn new(instrument: InstrumentJson) -> Self {
        Self {
            schema: InstrumentSchema::Instrument,
            instrument,
        }
    }

    /// Consume this validated wire envelope and build its runtime instrument.
    ///
    /// # Errors
    ///
    /// Returns an error when the concrete instrument cannot be constructed or
    /// violates its domain or pricing invariants.
    pub fn into_boxed(self) -> Result<Box<dyn Instrument>> {
        self.instrument.into_boxed()
    }

    /// Compute the versioned SHA-256 hash of this envelope's canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if the envelope contains a non-finite number or cannot
    /// be serialized to the core canonical JSON representation.
    pub fn content_hash(&self) -> Result<String> {
        finstack_quant_core::canonical::content_hash(self)
    }

    /// Load a persisted instrument envelope under strict contract policy.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Complete UTF-8 JSON encoding of an instrument envelope.
    /// * `limits` - Resource policy bounding input size, JSON depth, and
    ///   retained diagnostics.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError`] for malformed JSON, resource-limit failures,
    /// a missing envelope, malformed or unsupported schema markers, invalid
    /// instrument shape, or failed instrument validation.
    pub fn from_slice_strict(
        bytes: &[u8],
        limits: &LoadLimits,
    ) -> std::result::Result<(Box<dyn Instrument>, ValidationReport), ContractError> {
        let value = parse_json_value(bytes, limits)?;
        let envelope: Self = deserialize_json_value(value, limits)?;
        let instrument = envelope.into_boxed()?;
        Ok((instrument, ValidationReport::default()))
    }

    /// Load an instrument from a JSON value.
    ///
    /// Requires the canonical v1 envelope form:
    ///
    /// ```json
    /// { "schema": "finstack_quant.instrument/1", "instrument": { ... } }
    /// ```
    ///
    /// # Arguments
    ///
    /// * `value` - Parsed canonical instrument envelope JSON.
    pub fn from_value(value: serde_json::Value) -> Result<Box<dyn Instrument>> {
        let envelope: Self = serde_json::from_value(value).map_err(|e| {
            finstack_quant_core::Error::Validation(format!(
                "Failed to parse instrument envelope JSON: {e}"
            ))
        })?;
        envelope.into_boxed()
    }

    /// Load an instrument from a JSON reader.
    ///
    /// Reads up to [`MAX_JSON_BYTES`] from the reader.  If the input exceeds
    /// that limit the call returns a clear validation error rather than
    /// attempting an unbounded allocation.
    ///
    /// # Arguments
    ///
    /// * `reader` - Any reader providing JSON bytes
    ///
    /// # Returns
    ///
    /// A boxed instrument trait object ready for pricing.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Input exceeds [`MAX_JSON_BYTES`]
    /// - JSON is malformed
    /// - Schema version is unsupported
    /// - Required fields are missing
    /// - Unknown fields are present (strict mode)
    /// - Spec validation fails
    pub fn from_reader<R: Read>(reader: R) -> Result<Box<dyn Instrument>> {
        // Read up to MAX_JSON_BYTES + 1 bytes so we can distinguish "exactly
        // at limit" from "over limit" without reading the whole stream first.
        let mut buf = Vec::with_capacity(4096.min(MAX_JSON_BYTES));
        reader
            .take((MAX_JSON_BYTES as u64) + 1)
            .read_to_end(&mut buf)
            .map_err(|e| {
                finstack_quant_core::Error::Validation(format!("Failed to read JSON: {e}"))
            })?;

        if buf.len() > MAX_JSON_BYTES {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Instrument JSON input exceeds the {} MiB size limit ({} bytes read before limit)",
                MAX_JSON_BYTES / (1024 * 1024),
                buf.len(),
            )));
        }

        let value: serde_json::Value = serde_json::from_slice(&buf).map_err(|e| {
            finstack_quant_core::Error::Validation(format!("Failed to parse JSON: {e}"))
        })?;
        Self::from_value(value)
    }

    /// Load an instrument from a JSON string.
    ///
    /// Convenience wrapper around `from_reader`.  The same [`MAX_JSON_BYTES`]
    /// cap applies.
    ///
    /// # Arguments
    ///
    /// * `s` - Complete UTF-8 canonical instrument envelope.
    ///
    /// # Errors
    ///
    /// Returns an error before parsing when `s` exceeds [`MAX_JSON_BYTES`], or
    /// when JSON, schema, instrument construction, or instrument validation
    /// fails.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Box<dyn Instrument>> {
        if s.len() > MAX_JSON_BYTES {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Instrument JSON input is {} bytes, which exceeds the {} MiB size limit",
                s.len(),
                MAX_JSON_BYTES / (1024 * 1024),
            )));
        }
        Self::from_reader(s.as_bytes())
    }

    /// Load an instrument from a JSON file path.
    ///
    /// Checks the file's reported byte length up front (defence in depth) and
    /// also enforces the [`MAX_JSON_BYTES`] cap while reading, so the limit
    /// is respected even on file-systems that misreport metadata.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the JSON file
    ///
    /// # Returns
    ///
    /// A boxed instrument trait object ready for pricing.
    pub fn from_path(path: impl AsRef<std::path::Path>) -> Result<Box<dyn Instrument>> {
        let path = path.as_ref();
        let file = std::fs::File::open(path).map_err(|e| {
            finstack_quant_core::Error::Validation(format!(
                "Failed to open instrument JSON file '{}': {e}",
                path.display()
            ))
        })?;

        // Defence-in-depth: reject obviously oversized files before reading.
        if let Ok(meta) = file.metadata() {
            if meta.len() > MAX_JSON_BYTES as u64 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Instrument JSON file '{}' is {} bytes, which exceeds the {} MiB size limit",
                    path.display(),
                    meta.len(),
                    MAX_JSON_BYTES / (1024 * 1024),
                )));
            }
        }

        Self::from_reader(file)
    }
}

/// Construct a runtime cashflow-providing instrument from a canonical tagged
/// JSON payload, dispatched through the instrument registry.
///
/// Every instrument type registered in the crate's instrument registry is
/// reachable through the required v1 envelope.
///
/// # Errors
///
/// Returns an error when the payload does not match a registered instrument
/// type or fails spec validation.
///
/// # Arguments
///
/// * `value` - Parsed canonical v1 instrument envelope to construct as a
///   runtime cashflow provider.
pub fn cashflow_provider_from_value(
    value: serde_json::Value,
) -> Result<Arc<dyn finstack_quant_cashflows::CashflowProvider + Send + Sync>> {
    let instrument: Box<dyn Instrument> = InstrumentEnvelope::from_value(value)?;
    let provider: Box<dyn finstack_quant_cashflows::CashflowProvider + Send + Sync> = instrument;
    Ok(Arc::from(provider))
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use time::Month;

    #[test]
    fn nested_instrument_input_objects_are_closed() {
        let schema = serde_json::to_value(schemars::schema_for!(InstrumentEnvelope))
            .expect("instrument envelope schema");
        for definition in [
            "BasisSwapLeg",
            "CallPut",
            "CallPutSchedule",
            "CmoTranche",
            "CmoWaterfall",
            "DeliverableBond",
            "DilutionEvent",
            "DilutionSecurity",
            "DrawRepayEvent",
            "EquityBridge",
            "FutureContractSpecs",
            "PacCollar",
            "RevolvingCreditFees",
            "SabrParameters",
            "SoftCallTrigger",
            "TrancheStructure",
            "ValuationDiscounts",
            "VolIndexContractSpecs",
        ] {
            assert_eq!(
                schema["$defs"][definition]["additionalProperties"], false,
                "{definition} must reject unknown fields"
            );
        }
    }

    #[test]
    fn test_bond_json_roundtrip() {
        let bond = Bond::fixed(
            "TEST-BOND",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            Date::from_calendar_date(2024, Month::January, 1).expect("Valid test date"),
            Date::from_calendar_date(2034, Month::January, 1).expect("Valid test date"),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("Bond::fixed should succeed with valid parameters");

        let json = InstrumentJson::Bond(bond.clone());
        let serialized =
            serde_json::to_string(&json).expect("JSON serialization should succeed in test");
        let deserialized: InstrumentJson =
            serde_json::from_str(&serialized).expect("JSON deserialization should succeed in test");

        match deserialized {
            InstrumentJson::Bond(b) => {
                assert_eq!(b.id, bond.id);
                assert_eq!(b.notional, bond.notional);
            }
            _ => panic!("Expected Bond variant"),
        }
    }

    #[test]
    fn test_envelope_roundtrip() {
        let bond = Bond::fixed(
            "TEST-BOND",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            Date::from_calendar_date(2024, Month::January, 1).expect("Valid test date"),
            Date::from_calendar_date(2034, Month::January, 1).expect("Valid test date"),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("Bond::fixed should succeed with valid parameters");

        let envelope = InstrumentEnvelope {
            schema: InstrumentSchema::CURRENT,
            instrument: InstrumentJson::Bond(bond.clone()),
        };

        let serialized = serde_json::to_string_pretty(&envelope)
            .expect("JSON serialization should succeed in test");
        let deserialized: InstrumentEnvelope =
            serde_json::from_str(&serialized).expect("JSON deserialization should succeed in test");

        assert_eq!(deserialized.schema, envelope.schema);
        match deserialized.instrument {
            InstrumentJson::Bond(b) => {
                assert_eq!(b.id, bond.id);
            }
            _ => panic!("Expected Bond variant"),
        }
    }

    #[test]
    fn test_envelope_from_str() {
        // Use Bond which is simpler and fully tested
        let json = r#"{
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "bond",
                "spec": {
                    "id": "BOND-FROM-STR",
                    "notional": { "amount": "1000000", "currency": "USD" },
                    "issue_date": "2024-01-01",
                    "maturity": "2034-01-01",
                    "cashflow_spec": {
                        "fixed": {
                            "coupon_type": "cash",
                            "rate": "0.05",
                            "frequency": { "count": 6, "unit": "months" },
                            "day_count": "30_360",
                            "business_day_convention": "following",
                            "calendar_id": "weekends_only",
                            "stub": "none",
                            "end_of_month": false,
                            "payment_lag_days": 0
                        }
                    },
                    "discount_curve_id": "USD-OIS",
                    "credit_curve_id": null,
                    "call_put": null,
                    "accrual_method": "linear",
                    "attributes": { "tags": [], "meta": {} },
                    "settlement_days": null,
                    "ex_coupon_days": null
                }
            }
        }"#;

        let instrument = InstrumentEnvelope::from_str(json)
            .expect("Instrument envelope parsing should succeed in test");
        assert_eq!(instrument.id(), "BOND-FROM-STR");
    }

    #[test]
    fn test_envelope_from_value_rejects_bare_tagged_instrument() {
        let bond = Bond::fixed(
            "TEST-BARE",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            Date::from_calendar_date(2024, Month::January, 1).expect("Valid test date"),
            Date::from_calendar_date(2034, Month::January, 1).expect("Valid test date"),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("Bond::fixed should succeed with valid parameters");

        let value =
            serde_json::to_value(InstrumentJson::Bond(bond)).expect("serialize tagged instrument");
        let error = InstrumentEnvelope::from_value(value)
            .err()
            .expect("tagged instrument payload must include the v1 envelope");
        assert!(error.to_string().contains("envelope"), "{error}");
    }

    #[test]
    fn strict_instrument_loader_requires_envelope_and_reports_schema_failures() {
        let bond = Bond::fixed(
            "STRICT-BOND",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            Date::from_calendar_date(2024, Month::January, 1).expect("valid date"),
            Date::from_calendar_date(2034, Month::January, 1).expect("valid date"),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("valid bond");
        let tagged = InstrumentJson::Bond(bond);
        let bare = serde_json::to_vec(&tagged).expect("serialize tagged instrument");
        let error = InstrumentEnvelope::from_slice_strict(
            &bare,
            &finstack_quant_core::LoadLimits::default(),
        )
        .err()
        .expect("strict loading rejects bare instrument payloads");
        let finstack_quant_core::ContractError::Report(report) = error else {
            panic!("expected structured report");
        };
        assert_eq!(report.diagnostics[0].code, "contract/structure-invalid");

        for schema in [
            "finstack_quant.instrument/0",
            "finstack_quant.instrument/2",
            "finstack_quant.instrument/not-a-version",
        ] {
            let envelope = serde_json::json!({"schema": schema, "instrument": tagged});
            let error = InstrumentEnvelope::from_slice_strict(
                &serde_json::to_vec(&envelope).expect("serialize envelope"),
                &finstack_quant_core::LoadLimits::default(),
            )
            .err()
            .expect("invalid schema must fail");
            let finstack_quant_core::ContractError::Report(report) = error else {
                panic!("expected structured schema report");
            };
            assert!(
                matches!(
                    report.diagnostics[0].code.as_str(),
                    "contract/version-unsupported"
                        | "contract/schema-malformed"
                        | "contract/structure-invalid"
                ),
                "precise schema diagnostic expected: {:?}",
                report.diagnostics[0]
            );
        }
    }

    #[test]
    fn test_unknown_fields_rejected() {
        // Test with Bond and an extra unknown field
        let json = r#"{
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "bond",
                "spec": {
                    "id": "BOND-001",
                    "notional": { "amount": "1000000", "currency": "USD" },
                    "issue_date": "2024-01-01",
                    "maturity": "2034-01-01",
                    "cashflow_spec": {
                        "fixed": {
                            "coupon_type": "cash",
                            "rate": "0.05",
                            "frequency": { "count": 6, "unit": "months" },
                            "day_count": "30_360",
                            "business_day_convention": "following",
                            "calendar_id": "weekends_only",
                            "stub": "none",
                            "end_of_month": false,
                            "payment_lag_days": 0
                        }
                    },
                    "discount_curve_id": "USD-OIS",
                    "credit_curve_id": null,
                    "call_put": null,
                    "attributes": { "tags": [], "meta": {} },
                    "settlement_days": null,
                    "ex_coupon_days": null,
                    "unknown_field": "should_fail"
                }
            }
        }"#;

        let result = InstrumentEnvelope::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_unsupported_schema_version() {
        let json = r#"{
            "schema": "finstack_quant.instrument/999",
            "instrument": {
                "type": "bond",
                "spec": {
                    "id": "BOND-001",
                    "notional": { "amount": "1000000", "currency": "USD" },
                    "issue_date": "2024-01-01",
                    "maturity": "2034-01-01",
                    "cashflow_spec": {
                        "fixed": {
                            "coupon_type": "cash",
                            "rate": "0.05",
                            "frequency": { "count": 6, "unit": "months" },
                            "day_count": "30_360",
                            "business_day_convention": "following",
                            "calendar_id": "weekends_only",
                            "stub": "none",
                            "end_of_month": false,
                            "payment_lag_days": 0
                        }
                    },
                    "discount_curve_id": "USD-OIS",
                    "credit_curve_id": null,
                    "call_put": null,
                    "attributes": { "tags": [], "meta": {} },
                    "settlement_days": null,
                    "ex_coupon_days": null
                }
            }
        }"#;

        let result = InstrumentEnvelope::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_unknown_type_rejected() {
        let json = r#"{
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "totally_unknown_instrument",
                "spec": {}
            }
        }"#;

        let result = InstrumentEnvelope::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn unknown_variant_error_lists_canonical_dispatch_tags() {
        let error = serde_json::from_value::<InstrumentJson>(serde_json::json!({
            "type": "totally_unknown_instrument",
            "spec": {}
        }))
        .expect_err("unknown type should fail");
        let message = error.to_string();

        for tag in [
            "commodity_swaption",
            "commodity_spread_option",
            "trs_equity",
            "trs_fixed_income_index",
        ] {
            assert!(
                message.contains(tag),
                "unknown variant message should list {tag}, got: {message}"
            );
        }
    }

    #[test]
    fn test_structured_credit_deserializes_into_boxed_variant() {
        let structured_credit = StructuredCredit::example();
        let structured_credit_id = structured_credit.id.clone();
        let json = serde_json::to_value(InstrumentJson::StructuredCredit(Box::new(
            structured_credit,
        )))
        .expect("StructuredCredit JSON serialization should succeed");

        match serde_json::from_value::<InstrumentJson>(json)
            .expect("StructuredCredit JSON deserialization should succeed")
        {
            InstrumentJson::StructuredCredit(instrument) => {
                assert_eq!(instrument.id, structured_credit_id)
            }
            _ => panic!("Expected StructuredCredit variant"),
        }
    }

    // Note: IRS and TermLoan tests skipped - complex builder patterns
    // The serialization/deserialization works but proper construction
    // requires detailed leg specifications beyond scope of simple unit tests

    #[test]
    fn test_cds_roundtrip() {
        let cds = CreditDefaultSwap::example();

        let json = InstrumentJson::CreditDefaultSwap(cds.clone());
        let serialized =
            serde_json::to_string(&json).expect("JSON serialization should succeed in test");
        let deserialized: InstrumentJson =
            serde_json::from_str(&serialized).expect("JSON deserialization should succeed in test");

        match deserialized {
            InstrumentJson::CreditDefaultSwap(i) => {
                assert_eq!(i.id, cds.id);
                assert_eq!(i.notional, cds.notional);
            }
            _ => panic!("Expected CreditDefaultSwap variant"),
        }
    }

    #[test]
    fn test_fx_swap_roundtrip() {
        let fx_swap = FxSwap::builder()
            .id(InstrumentId::new("FXSWAP-TEST"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .near_date(Date::from_calendar_date(2024, Month::January, 3).expect("Valid test date"))
            .far_date(Date::from_calendar_date(2024, Month::July, 3).expect("Valid test date"))
            .base_notional(Money::from((1_000_000_i64, Currency::EUR)))
            .domestic_discount_curve_id("USD-OIS".into())
            .foreign_discount_curve_id("EUR-OIS".into())
            .near_rate_opt(Some(1.10))
            .far_rate_opt(Some(1.12))
            .attributes(Attributes::new())
            .build()
            .expect("FxSwap builder should succeed with valid test data");

        let json = InstrumentJson::FxSwap(fx_swap.clone());
        let serialized =
            serde_json::to_string(&json).expect("JSON serialization should succeed in test");
        let deserialized: InstrumentJson =
            serde_json::from_str(&serialized).expect("JSON deserialization should succeed in test");

        match deserialized {
            InstrumentJson::FxSwap(i) => {
                assert_eq!(i.id, fx_swap.id);
                assert_eq!(i.base_currency, fx_swap.base_currency);
            }
            _ => panic!("Expected FxSwap variant"),
        }
    }

    #[test]
    fn test_basis_swap_roundtrip() {
        use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};

        let start = Date::from_calendar_date(2024, Month::January, 1).expect("Valid test date");
        let end = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");

        let primary_leg = BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start,
            end,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some("USGS".to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::from(5),
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        };

        let reference_leg = BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start,
            end,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some("USGS".to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        };

        let swap = BasisSwap::new(
            "BASIS-TEST",
            Money::from((10_000_000_i64, Currency::USD)),
            primary_leg,
            reference_leg,
        )
        .expect("BasisSwap construction should succeed in test");

        let json = InstrumentJson::BasisSwap(swap.clone());
        let serialized =
            serde_json::to_string(&json).expect("JSON serialization should succeed in test");
        let deserialized: InstrumentJson =
            serde_json::from_str(&serialized).expect("JSON deserialization should succeed in test");

        match deserialized {
            InstrumentJson::BasisSwap(i) => {
                assert_eq!(i.id, swap.id);
                assert_eq!(
                    i.primary_leg.discount_curve_id,
                    swap.primary_leg.discount_curve_id
                );
                assert_eq!(i.primary_leg.calendar_id.as_deref(), Some("USGS"));
            }
            _ => panic!("Expected BasisSwap variant"),
        }
    }

    #[test]
    fn test_fx_spot_roundtrip() {
        let fx_spot = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD)
            .with_notional(Money::from((1_000_000_i64, Currency::EUR)))
            .expect("FxSpot notional should be valid")
            .with_rate(1.10)
            .expect("FxSpot rate should be valid")
            .with_settlement(
                Date::from_calendar_date(2024, Month::January, 15).expect("Valid test date"),
            )
            .with_base_calendar_id("TARGET")
            .with_quote_calendar_id("USNY");

        let json = InstrumentJson::FxSpot(fx_spot.clone());
        let serialized =
            serde_json::to_string(&json).expect("JSON serialization should succeed in test");
        let deserialized: InstrumentJson =
            serde_json::from_str(&serialized).expect("JSON deserialization should succeed in test");

        match deserialized {
            InstrumentJson::FxSpot(i) => {
                assert_eq!(i.id, fx_spot.id);
                assert_eq!(i.base_currency, fx_spot.base_currency);
                assert_eq!(i.quote_currency, fx_spot.quote_currency);
                assert_eq!(i.base_calendar_id.as_deref(), Some("TARGET"));
                assert_eq!(i.quote_calendar_id.as_deref(), Some("USNY"));
            }
            _ => panic!("Expected FxSpot variant"),
        }
    }

    fn remove_spec_key(value: &mut serde_json::Value, key: &str) {
        value
            .get_mut("spec")
            .and_then(serde_json::Value::as_object_mut)
            .expect("InstrumentJson should serialize with an object spec")
            .remove(key);
    }

    #[test]
    fn legacy_instrument_tags_are_rejected() {
        // schema-rejection-test
        for tag in [
            "yo_y_inflation_swap",
            "target_redemption_note",
            "inverse_floater",
            "equity_trs",
            "fi_trs",
            "fixed_income_trs",
        ] {
            let error = serde_json::from_value::<InstrumentJson>(serde_json::json!({
                "type": tag,
                "spec": {}
            }))
            .expect_err("legacy instrument tags must be rejected");
            assert!(
                error.to_string().contains("unknown variant")
                    || error.to_string().contains("did not match any variant"),
                "unexpected error for {tag}: {error}"
            );
        }
    }

    #[test]
    fn test_basis_swap_defaults_when_optional_fields_omitted() {
        use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};

        let start = Date::from_calendar_date(2026, Month::January, 1).expect("Valid test date");
        let end = Date::from_calendar_date(2027, Month::January, 1).expect("Valid test date");

        let primary_leg = BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start,
            end,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::ShortFront,
            spread_bp: Decimal::from(5),
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        };
        let reference_leg = BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start,
            end,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        };
        let swap = BasisSwap::new(
            "BASIS-DEFAULTS",
            Money::from((10_000_000_i64, Currency::USD)),
            primary_leg,
            reference_leg,
        )
        .expect("BasisSwap construction should succeed in test");
        let mut json = serde_json::to_value(InstrumentJson::BasisSwap(swap))
            .expect("BasisSwap JSON serialization should succeed");
        remove_spec_key(&mut json, "allow_calendar_fallback");
        remove_spec_key(&mut json, "allow_same_curve");

        let deserialized: InstrumentJson =
            serde_json::from_value(json).expect("BasisSwap JSON deserialization should succeed");
        match deserialized {
            InstrumentJson::BasisSwap(i) => {
                assert!(!i.allow_calendar_fallback);
                assert!(!i.allow_same_curve);
                assert_eq!(
                    i.primary_leg.business_day_convention,
                    BusinessDayConvention::ModifiedFollowing
                );
                assert_eq!(i.primary_leg.stub, StubKind::ShortFront);
            }
            _ => panic!("Expected BasisSwap variant"),
        }
    }

    #[test]
    fn test_cap_floor_defaults_when_optional_fields_omitted() {
        use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};

        let option = CapFloor::new(
            InstrumentId::new("IROPT-DEFAULTS"),
            RateOptionType::Cap,
            Money::from((1_000_000_i64, Currency::USD)),
            0.03,
            Date::from_calendar_date(2026, Month::January, 1).expect("Valid test date"),
            Date::from_calendar_date(2028, Month::January, 1).expect("Valid test date"),
            Some(Tenor::quarterly()),
            DayCount::Act360,
            CurveId::new("USD-OIS"),
            CurveId::new("USD-SOFR-3M"),
            CurveId::new("USD-CAPFLOOR-VOL"),
        )
        .expect("valid strike");
        let mut json = serde_json::to_value(InstrumentJson::CapFloor(option))
            .expect("CapFloor JSON serialization should succeed");
        assert_eq!(
            json.pointer("/type").and_then(serde_json::Value::as_str),
            Some("cap_floor")
        );
        remove_spec_key(&mut json, "stub");
        remove_spec_key(&mut json, "business_day_convention");

        let deserialized: InstrumentJson = serde_json::from_value(json.clone())
            .expect("CapFloor JSON deserialization should succeed");
        match deserialized {
            InstrumentJson::CapFloor(i) => {
                assert_eq!(i.stub, StubKind::ShortFront);
                assert_eq!(
                    i.business_day_convention,
                    BusinessDayConvention::ModifiedFollowing
                );
            }
            _ => panic!("Expected CapFloor variant"),
        }

        let mut rejected_json = json;
        rejected_json["type"] = serde_json::Value::String("interest_rate_option".into());
        serde_json::from_value::<InstrumentJson>(rejected_json)
            .expect_err("retired interest_rate_option tag must be rejected");
    }

    #[test]
    fn test_repo_default_business_day_convention_when_omitted() {
        use finstack_quant_core::dates::BusinessDayConvention;

        let repo = Repo::term(
            "REPO-DEFAULTS",
            Money::from((10_000_000_i64, Currency::USD)),
            CollateralSpec::new("UST-10Y", 1000.0, "UST_10Y_PRICE"),
            0.0525,
            Date::from_calendar_date(2025, Month::January, 2).expect("Valid test date"),
            Date::from_calendar_date(2025, Month::January, 9).expect("Valid test date"),
            CurveId::new("USD-OIS"),
        )
        .expect("Repo::term should succeed for test setup");
        let mut json = serde_json::to_value(InstrumentJson::Repo(repo))
            .expect("Repo JSON serialization should succeed");
        remove_spec_key(&mut json, "business_day_convention");

        let deserialized: InstrumentJson =
            serde_json::from_value(json).expect("Repo JSON deserialization should succeed");
        match deserialized {
            InstrumentJson::Repo(i) => {
                assert_eq!(
                    i.business_day_convention,
                    BusinessDayConvention::ModifiedFollowing
                );
            }
            _ => panic!("Expected Repo variant"),
        }
    }

    #[test]
    fn test_inflation_linked_bond_defaults_when_optional_fields_omitted() {
        use finstack_quant_core::dates::{BusinessDayConvention, StubKind};

        let bond = InflationLinkedBond::example();
        let mut json = serde_json::to_value(InstrumentJson::InflationLinkedBond(bond))
            .expect("InflationLinkedBond JSON serialization should succeed");
        remove_spec_key(&mut json, "business_day_convention");
        remove_spec_key(&mut json, "stub");

        let deserialized: InstrumentJson = serde_json::from_value(json)
            .expect("InflationLinkedBond JSON deserialization should succeed");
        match deserialized {
            InstrumentJson::InflationLinkedBond(i) => {
                assert_eq!(
                    i.business_day_convention,
                    BusinessDayConvention::ModifiedFollowing
                );
                assert_eq!(i.stub, StubKind::ShortFront);
            }
            _ => panic!("Expected InflationLinkedBond variant"),
        }
    }

    #[test]
    fn test_cds_tranche_default_business_day_convention_when_omitted() {
        use finstack_quant_core::dates::BusinessDayConvention;

        let tranche = CDSTranche::example();
        let mut json = serde_json::to_value(InstrumentJson::CDSTranche(tranche))
            .expect("CDSTranche JSON serialization should succeed");
        remove_spec_key(&mut json, "business_day_convention");

        let deserialized: InstrumentJson =
            serde_json::from_value(json).expect("CDSTranche JSON deserialization should succeed");
        match deserialized {
            InstrumentJson::CDSTranche(i) => {
                assert_eq!(
                    i.business_day_convention,
                    BusinessDayConvention::ModifiedFollowing
                );
            }
            _ => panic!("Expected CDSTranche variant"),
        }
    }

    #[test]
    fn test_fx_spot_default_business_day_convention_when_omitted() {
        use finstack_quant_core::dates::BusinessDayConvention;

        let spot = FxSpot::new(
            InstrumentId::new("EURUSD-DEFAULTS"),
            Currency::EUR,
            Currency::USD,
        );
        let mut json = serde_json::to_value(InstrumentJson::FxSpot(spot))
            .expect("FxSpot JSON serialization should succeed");
        remove_spec_key(&mut json, "business_day_convention");

        let deserialized: InstrumentJson =
            serde_json::from_value(json).expect("FxSpot JSON deserialization should succeed");
        match deserialized {
            InstrumentJson::FxSpot(i) => {
                assert_eq!(
                    i.business_day_convention,
                    BusinessDayConvention::ModifiedFollowing
                );
            }
            _ => panic!("Expected FxSpot variant"),
        }
    }

    #[test]
    fn test_cds_option_defaults_when_optional_fields_omitted() {
        let option = CDSOption::example().expect("CDSOption example is valid");
        let mut json = serde_json::to_value(InstrumentJson::CDSOption(option))
            .expect("CDSOption JSON serialization should succeed");
        remove_spec_key(&mut json, "underlying_is_index");
        remove_spec_key(&mut json, "underlying_cds_coupon");

        let deserialized: InstrumentJson =
            serde_json::from_value(json).expect("CDSOption JSON deserialization should succeed");
        match deserialized {
            InstrumentJson::CDSOption(i) => {
                assert!(!i.underlying_is_index);
                assert!(i.underlying_cds_coupon.is_none());
            }
            _ => panic!("Expected CDSOption variant"),
        }
    }

    #[test]
    fn test_equity_option_default_discrete_dividends_when_omitted() {
        let option = EquityOption::example().expect("EquityOption example is valid");
        let mut json = serde_json::to_value(InstrumentJson::EquityOption(option))
            .expect("EquityOption JSON serialization should succeed");
        remove_spec_key(&mut json, "discrete_dividends");

        let deserialized: InstrumentJson =
            serde_json::from_value(json).expect("EquityOption JSON deserialization should succeed");
        match deserialized {
            InstrumentJson::EquityOption(i) => {
                assert!(i.discrete_dividends.is_empty());
            }
            _ => panic!("Expected EquityOption variant"),
        }
    }

    /// Verifies that every registry example validates against the generated root schema.
    #[test]
    fn test_instrument_schema_union_parity() {
        let entries = instrument_registry();
        assert_eq!(entries.len(), registry_tags().len());
        for entry in entries {
            for envelope in entry.examples().expect("registry example builds") {
                crate::schema::validate_instrument_envelope_json(&envelope).unwrap_or_else(|err| {
                    panic!("{} registry example must validate: {err}", entry.tag)
                });
            }
        }
    }

    #[test]
    fn test_cashflow_provider_from_value_non_legacy_type() {
        // RevolvingCredit is in the registry but was absent from the old statements
        // `Generic` brute-force list (Bond/IRS/TermLoan/Deposit/FRA/Repo). It must
        // build through the canonical registry.
        let rcf = crate::instruments::RevolvingCredit::example()
            .expect("example RevolvingCredit should construct");
        let tagged = serde_json::to_value(InstrumentEnvelope::new(
            InstrumentJson::RevolvingCredit(rcf),
        ))
        .expect("InstrumentJson serialization should succeed in test");

        let provider = cashflow_provider_from_value(tagged);
        assert!(
            provider.is_ok(),
            "revolving_credit must build via the canonical registry"
        );
    }

    #[test]
    fn test_cashflow_provider_from_value_rejects_unknown_type() {
        let provider = cashflow_provider_from_value(serde_json::json!({
            "schema": "finstack_quant.instrument/1",
            "instrument": {"type": "not_a_real_instrument", "spec": {}}
        }));
        assert!(provider.is_err(), "unknown type tag must error");
    }

    // ── W48: input-size cap regression tests ────────────────────────────────

    /// `from_reader` must return a clear `Err` (not panic / OOM) when the
    /// input stream exceeds `MAX_JSON_BYTES`.
    #[test]
    fn test_from_reader_rejects_oversized_input() {
        // Build a reader that is exactly MAX_JSON_BYTES + 1 bytes of spaces.
        // `std::io::repeat` produces infinite bytes; `take` bounds it.
        let oversized = std::io::repeat(b' ').take((MAX_JSON_BYTES as u64) + 1);

        let result = InstrumentEnvelope::from_reader(oversized);
        assert!(
            result.is_err(),
            "from_reader must reject input larger than MAX_JSON_BYTES"
        );

        let err_msg = result.err().unwrap().to_string();
        assert!(
            err_msg.contains("size limit") || err_msg.contains("MiB"),
            "error message should name the size limit, got: {err_msg}"
        );
    }

    /// A valid, normal-sized instrument JSON must still load successfully after
    /// the byte-cap guard is in place.
    #[test]
    fn test_from_reader_accepts_normal_sized_input() {
        let json = r#"{
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "bond",
                "spec": {
                    "id": "BOND-SIZE-OK",
                    "notional": { "amount": "1000000", "currency": "USD" },
                    "issue_date": "2024-01-01",
                    "maturity": "2034-01-01",
                    "cashflow_spec": {
                        "fixed": {
                            "coupon_type": "cash",
                            "rate": "0.05",
                            "frequency": { "count": 6, "unit": "months" },
                            "day_count": "30_360",
                            "business_day_convention": "following",
                            "calendar_id": "weekends_only",
                            "stub": "none",
                            "end_of_month": false,
                            "payment_lag_days": 0
                        }
                    },
                    "discount_curve_id": "USD-OIS",
                    "credit_curve_id": null,
                    "call_put": null,
                    "accrual_method": "linear",
                    "attributes": { "tags": [], "meta": {} },
                    "settlement_days": null,
                    "ex_coupon_days": null
                }
            }
        }"#;

        let result = InstrumentEnvelope::from_reader(json.as_bytes());
        assert!(
            result.is_ok(),
            "from_reader must accept valid normal-sized input"
        );
        assert_eq!(result.unwrap().id(), "BOND-SIZE-OK");
    }

    /// `from_reader` at exactly `MAX_JSON_BYTES` of whitespace (invalid JSON
    /// but within the size limit) must fail with a parse error, not a size
    /// error.  This confirms the boundary condition is `> MAX_JSON_BYTES`,
    /// not `>= MAX_JSON_BYTES`.
    #[test]
    fn test_from_reader_at_exact_limit_gives_parse_error_not_size_error() {
        let exactly_at_limit = std::io::repeat(b' ').take(MAX_JSON_BYTES as u64);
        let result = InstrumentEnvelope::from_reader(exactly_at_limit);
        // Must fail (all spaces is not valid JSON), but the error must NOT
        // mention the size limit — it should be a JSON parse error.
        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(
            !err_msg.contains("size limit"),
            "at exact limit the error should be a parse error, not a size-limit error, got: {err_msg}"
        );
    }

    #[test]
    fn every_registered_instrument_has_a_distinct_index_summary() {
        // The fallback arm keeps `instrument_summary` total, which would let a
        // newly registered instrument slip into the index with generic prose.
        let fallback = instrument_summary("__unregistered__");
        let mut seen: std::collections::BTreeMap<&str, &str> = std::collections::BTreeMap::new();
        for entry in instrument_registry() {
            let summary = instrument_summary(entry.tag);
            assert_ne!(
                summary, fallback,
                "instrument {} has no index summary",
                entry.tag
            );
            if let Some(other) = seen.insert(summary, entry.tag) {
                panic!("{} and {} share an index summary", other, entry.tag);
            }
        }
    }
}
