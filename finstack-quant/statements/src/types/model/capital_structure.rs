//! Capital-structure specifications, debt instrument wrappers, and FX policies.

use super::spec::FinancialModelSpec;
use crate::error::{Error, Result};
use crate::types::{ForecastSpec, NodeId, NodeSpec};
use finstack_quant_core::dates::PeriodId;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::fixed_income::convertible::ConvertibleBond;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCredit;
use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
use finstack_quant_valuations::instruments::rates::cap_floor::CapFloor;
use finstack_quant_valuations::instruments::rates::irs::InterestRateSwap;
use finstack_quant_valuations::instruments::rates::swaption::Swaption;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// The model field semantic validation is checking.
#[derive(Clone, Copy)]
pub(super) enum SemanticLocation {
    /// A fixed JSON pointer; empty for the whole document.
    Field(&'static str),
    /// The node at this position of the node map.
    Node(usize),
}

impl SemanticLocation {
    /// The JSON pointer (RFC 6901) of this location in `model`.
    pub(super) fn pointer(self, model: &FinancialModelSpec) -> String {
        match self {
            Self::Field(pointer) => pointer.to_string(),
            Self::Node(index) => match model.nodes.get_index(index) {
                Some((node_id, _)) => format!(
                    "/nodes/{}",
                    node_id.as_str().replace('~', "~0").replace('/', "~1")
                ),
                None => "/nodes".to_string(),
            },
        }
    }
}

/// Validate explicit series units against their optional node declaration.
///
/// # Arguments
///
/// * `node` - Node whose explicit scalar or monetary series must use one type
///   and currency and agree with any declared units. Empty series infer no type.
///
/// # Errors
///
/// Returns a validation error for mixed series units or a declaration mismatch.
pub(crate) fn validated_explicit_value_type(
    node: &NodeSpec,
) -> Result<Option<crate::types::NodeValueType>> {
    let inferred = node
        .values
        .as_ref()
        .map(|values| crate::types::infer_series_value_type(values.values()))
        .transpose()?
        .flatten();
    if let (Some(declared), Some(actual)) = (node.value_type, inferred)
        && declared != actual
    {
        return Err(Error::build(format!(
            "Node '{}' declares {declared:?} but its explicit values have type {actual:?}",
            node.node_id
        )));
    }
    Ok(inferred)
}

/// Capital structure specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CapitalStructureSpec {
    /// Debt instruments (bonds, loans, swaps)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub debt_instruments: Vec<DebtInstrumentSpec>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub meta: IndexMap<String, serde_json::Value>,

    /// Optional reporting currency override for capital structure totals
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reporting_currency: Option<finstack_quant_core::currency::Currency>,

    /// Optional FX conversion policy override.
    ///
    /// Required to consolidate instruments of more than one currency: there
    /// is no default. `period_end` converts each period's native-currency
    /// totals on the inclusive period-close date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fx_policy: Option<CapitalFxPolicy>,

    /// Optional waterfall specification for dynamic cash flow allocation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waterfall: Option<crate::capital_structure::WaterfallSpec>,
}

impl CapitalStructureSpec {
    /// Validate the capital structure on its own, without the enclosing model.
    ///
    /// Runs [`WaterfallSpec::validate`](crate::capital_structure::WaterfallSpec::validate)
    /// on the optional waterfall, rejects swap, cap/floor and swaption PIK
    /// targets, and rejects bond, convertible, swap, cap/floor and swaption
    /// instruments when the waterfall carries a prepayment rung (`Sweep`,
    /// `MandatoryPrepayment` or `VoluntaryPrepayment`). Interest-rate swaps of
    /// either side are accepted. It does not check that
    /// referenced model nodes exist; [`FinancialModelSpec::validate_semantics`]
    /// calls this method as part of whole-model validation.
    ///
    /// # Errors
    ///
    /// Returns a build error for an invalid waterfall, a PIK election naming
    /// anything but a term loan of the structure, or a bond or derivative
    /// instrument alongside a prepayment rung.
    pub fn validate(&self) -> Result<()> {
        if let Some(waterfall) = &self.waterfall {
            waterfall.validate()?;
            // An election is applied through a term loan's own coupon
            // terms, so only a term loan of this structure can be authorized.
            if let Some(pik) = &waterfall.pik_toggle {
                for election in &pik.elections {
                    let Some(debt) = self
                        .debt_instruments
                        .iter()
                        .find(|debt| debt.id == election.instrument_id)
                    else {
                        return Err(Error::build(format!(
                            "WaterfallSpec: `pik_toggle.elections` names '{}', which is not \
                             a debt instrument of the capital structure.",
                            election.instrument_id
                        )));
                    };
                    if !matches!(debt.spec, FinancialStatementInstrument::TermLoan(_)) {
                        return Err(Error::build(format!(
                            "WaterfallSpec: `pik_toggle.elections` names '{}', which is not \
                             a term loan. A PIK election is applied through a term loan's \
                             coupon terms; no other instrument can elect PIK.",
                            election.instrument_id
                        )));
                    }
                }
            }
            let has_prepay = waterfall.priority_of_payments.iter().any(|p| {
                matches!(
                    p,
                    crate::capital_structure::PaymentPriority::Sweep
                        | crate::capital_structure::PaymentPriority::MandatoryPrepayment
                        | crate::capital_structure::PaymentPriority::VoluntaryPrepayment
                )
            });
            if has_prepay {
                for debt in &self.debt_instruments {
                    match &debt.spec {
                        FinancialStatementInstrument::Bond(_)
                        | FinancialStatementInstrument::ConvertibleBond(_) => {
                            return Err(Error::build(format!(
                                "WaterfallSpec: instrument '{}' is a bond; this waterfall \
                                 is a loan/revolver engine and rejects Bond or \
                                 ConvertibleBond targets when a prepayment rung \
                                 (`Sweep`, `MandatoryPrepayment`, or \
                                 `VoluntaryPrepayment`) is present. Bond coupons stay on \
                                 original face.",
                                debt.id
                            )));
                        }
                        FinancialStatementInstrument::InterestRateSwap(_)
                        | FinancialStatementInstrument::CapFloor(_)
                        | FinancialStatementInstrument::Swaption(_) => {
                            return Err(Error::build(format!(
                                "WaterfallSpec: instrument '{}' is not a sweep target; \
                                 swaps and options cannot appear with a prepayment rung.",
                                debt.id
                            )));
                        }
                        FinancialStatementInstrument::TermLoan(_)
                        | FinancialStatementInstrument::RevolvingCredit(_) => {}
                    }
                }
            }
            // Each prepayment names its eligible facilities. A named facility
            // must exist, and a facility whose agreement prices a payoff (a
            // call schedule with a premium or make-whole) cannot be repaid at
            // par here.
            for (_, name, facilities) in waterfall.prepayments() {
                if let crate::capital_structure::FacilitySelection::Named(ids) = facilities {
                    for id in ids {
                        if !self.debt_instruments.iter().any(|debt| &debt.id == id) {
                            return Err(Error::build(format!(
                                "WaterfallSpec: `{name}.facilities` names '{id}', which is \
                                 not a debt instrument of the capital structure."
                            )));
                        }
                    }
                }
                for debt in &self.debt_instruments {
                    if facilities.includes(&debt.id)
                        && matches!(
                            &debt.spec,
                            FinancialStatementInstrument::TermLoan(loan)
                                if loan.call_schedule.is_some()
                        )
                    {
                        return Err(Error::build(format!(
                            "WaterfallSpec: `{name}` would repay '{}' at par, but that loan \
                             has a call schedule with its own payoff terms, which the \
                             waterfall does not price. Exclude it from `{name}.facilities`.",
                            debt.id
                        )));
                    }
                }
            }
        }
        Ok(())
    }
}

/// How capital-structure totals are converted into the reporting currency.
///
/// Period-close conversion is the only supported policy. It must still be
/// chosen explicitly before instruments of more than one currency are
/// consolidated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CapitalFxPolicy {
    /// Convert each period's native-currency totals at the rate of the
    /// inclusive period-close date.
    PeriodEnd,
}

impl std::fmt::Display for CapitalFxPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("period_end")
    }
}

impl std::str::FromStr for CapitalFxPolicy {
    type Err = Error;

    fn from_str(label: &str) -> Result<Self> {
        match label {
            "period_end" => Ok(Self::PeriodEnd),
            other => Err(Error::build(format!(
                "unknown capital-structure FX policy '{other}': the only policy is 'period_end'"
            ))),
        }
    }
}

/// Instruments supported by company financial statement capital structures.
///
/// This intentionally smaller union keeps the financial statement schema
/// focused on debt and its common interest-rate hedges. The payloads are
/// converted to the canonical valuations registry only when a model is
/// evaluated.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(
    rename_all = "snake_case",
    tag = "type",
    content = "spec",
    deny_unknown_fields
)]
#[non_exhaustive]
pub enum FinancialStatementInstrument {
    /// Fixed- or floating-rate corporate or government bond.
    Bond(Bond),
    /// Convertible bond with an equity conversion feature.
    ConvertibleBond(ConvertibleBond),
    /// Revolving credit facility.
    RevolvingCredit(RevolvingCredit),
    /// Bilateral or institutional term loan.
    TermLoan(TermLoan),
    /// Plain-vanilla interest-rate swap used to hedge financing exposure.
    InterestRateSwap(InterestRateSwap),
    /// Interest-rate cap, floor, or collar.
    CapFloor(CapFloor),
    /// Option on an interest-rate swap.
    Swaption(Swaption),
}

impl From<FinancialStatementInstrument> for InstrumentJson {
    fn from(instrument: FinancialStatementInstrument) -> Self {
        match instrument {
            FinancialStatementInstrument::Bond(value) => Self::Bond(value),
            FinancialStatementInstrument::ConvertibleBond(value) => Self::ConvertibleBond(value),
            FinancialStatementInstrument::RevolvingCredit(value) => Self::RevolvingCredit(value),
            FinancialStatementInstrument::TermLoan(value) => Self::TermLoan(value),
            FinancialStatementInstrument::InterestRateSwap(value) => Self::InterestRateSwap(value),
            FinancialStatementInstrument::CapFloor(value) => Self::CapFloor(value),
            FinancialStatementInstrument::Swaption(value) => Self::Swaption(value),
        }
    }
}

impl TryFrom<InstrumentJson> for FinancialStatementInstrument {
    type Error = crate::Error;

    fn try_from(instrument: InstrumentJson) -> std::result::Result<Self, Self::Error> {
        match instrument {
            InstrumentJson::Bond(value) => Ok(Self::Bond(value)),
            InstrumentJson::ConvertibleBond(value) => Ok(Self::ConvertibleBond(value)),
            InstrumentJson::RevolvingCredit(value) => Ok(Self::RevolvingCredit(value)),
            InstrumentJson::TermLoan(value) => Ok(Self::TermLoan(value)),
            InstrumentJson::InterestRateSwap(value) => Ok(Self::InterestRateSwap(value)),
            InstrumentJson::CapFloor(value) => Ok(Self::CapFloor(value)),
            InstrumentJson::Swaption(value) => Ok(Self::Swaption(value)),
            unsupported => Err(crate::Error::invalid_input(format!(
                "instrument type '{}' is not supported in a financial statement capital structure",
                unsupported.type_tag()
            ))),
        }
    }
}

/// Debt instrument specification.
///
/// An identifier paired with a supported financial-statement instrument.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DebtInstrumentSpec {
    /// Instrument identifier (key within the capital structure).
    pub id: String,
    /// Tagged instrument payload: `{"type": "...", "spec": {...}}`.
    pub spec: FinancialStatementInstrument,
}

impl FinancialModelSpec {
    /// Static checks of one node's forecast: the method's own requirements,
    /// then the ones that need the model. They apply even when explicit
    /// observations cover every forecast period.
    pub(super) fn validate_forecast(
        &self,
        node_id: &NodeId,
        forecast: &ForecastSpec,
    ) -> Result<()> {
        use crate::types::{DistributionParams, MeanRevertingParams};

        forecast
            .validate()
            .map_err(|error| Error::build(format!("Node '{node_id}': {error}")))?;
        let peer = match forecast {
            // A growth curve covers exactly the model's forecast periods.
            ForecastSpec::CurvePct(params) => {
                let horizon = self.periods.iter().filter(|p| !p.is_actual).count();
                if params.curve.len() != horizon {
                    return Err(Error::build(format!(
                        "Node '{node_id}': the growth curve has {} rates but the model has \
                         {horizon} forecast periods",
                        params.curve.len()
                    )));
                }
                None
            }
            // Override keys are periods of this model's plan.
            ForecastSpec::Override(params) => {
                for key in params.overrides.keys() {
                    let in_plan = key
                        .parse::<PeriodId>()
                        .is_ok_and(|id| self.periods.iter().any(|period| period.id == id));
                    if !in_plan {
                        return Err(Error::build(format!(
                            "Node '{node_id}': forecast override key '{key}' is not a period of \
                             the model"
                        )));
                    }
                }
                None
            }
            ForecastSpec::Normal(DistributionParams {
                correlation_with, ..
            })
            | ForecastSpec::LogNormal(DistributionParams {
                correlation_with, ..
            })
            | ForecastSpec::MeanReverting(MeanRevertingParams {
                correlation_with, ..
            }) => correlation_with.as_deref(),
            _ => None,
        };
        // A correlation peer is another stochastic forecast of the model.
        let Some(peer) = peer else {
            return Ok(());
        };
        let stochastic_peer = peer != node_id.as_str()
            && self
                .get_node(peer)
                .and_then(|node| node.forecast.as_ref())
                .is_some_and(|peer| {
                    matches!(
                        peer,
                        ForecastSpec::Normal(_)
                            | ForecastSpec::LogNormal(_)
                            | ForecastSpec::MeanReverting(_)
                    )
                });
        if !stochastic_peer {
            return Err(Error::build(format!(
                "Node '{node_id}': correlation peer '{peer}' is not another node with a normal, \
                 log_normal or mean_reverting forecast"
            )));
        }
        // An active (non-zero) correlation mixes standardized shocks, so
        // both nodes need positive volatility and the peer chain must end.
        fn active(forecast: &ForecastSpec) -> Option<(&str, f64)> {
            let (peer, rho, std_dev) = match forecast {
                ForecastSpec::Normal(p) | ForecastSpec::LogNormal(p) => {
                    (p.correlation_with.as_deref(), p.correlation, p.std_dev)
                }
                ForecastSpec::MeanReverting(p) => {
                    (p.correlation_with.as_deref(), p.correlation, p.std_dev)
                }
                _ => return None,
            };
            peer.filter(|_| rho.is_some_and(|rho| rho != 0.0))
                .map(|peer| (peer, std_dev))
        }
        let Some((mut next, std_dev)) = active(forecast) else {
            return Ok(());
        };
        if std_dev == 0.0 {
            return Err(Error::build(format!(
                "Node '{node_id}': a non-zero correlation needs a positive 'std_dev'"
            )));
        }
        for _ in 0..self.nodes.len() {
            if next == node_id.as_str() {
                return Err(Error::build(format!(
                    "Node '{node_id}': its correlation peers form a cycle"
                )));
            }
            let peer_forecast = self.get_node(next).and_then(|node| node.forecast.as_ref());
            let std_dev = match peer_forecast {
                Some(ForecastSpec::Normal(p) | ForecastSpec::LogNormal(p)) => p.std_dev,
                Some(ForecastSpec::MeanReverting(p)) => p.std_dev,
                _ => 0.0,
            };
            if std_dev == 0.0 {
                return Err(Error::build(format!(
                    "Node '{node_id}': correlation peer '{next}' needs a positive 'std_dev' to \
                     supply shocks"
                )));
            }
            match peer_forecast.and_then(active) {
                Some((peer, _)) => next = peer,
                None => return Ok(()),
            }
        }
        // A chain longer than the node count has a cycle that does not pass
        // through this node; the nodes on it report it.
        Ok(())
    }
}
