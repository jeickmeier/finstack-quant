//! Dated source and use reconciliation for a single-currency LBO transaction.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::wire::DecimalWire;
use finstack_quant_statements::checks::CheckReport;
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::types::FinancialModelSpec;

use crate::analysis::checks::{CreditMapping, ThreeStatementMapping, lbo_model_checks};
use finstack_quant_core::money::Money;
use finstack_quant_statements::{Error, Result};
use serde::{Deserialize, Serialize};

/// Economic form of the seller consideration at close.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LboAcquisition {
    /// The agreed cash-free, debt-free purchase uses the entry enterprise value.
    CashFreeDebtFree,
    /// The seller is paid separately from the refinancing and closing reserves.
    SellerAndRefinancing {
        /// Agreed nonnegative purchase consideration paid to the seller.
        seller_consideration: Money,
        /// Cash required at close to refinance resolved obligations.
        refinancing: Money,
        /// Cash contributed to transaction reserves at close.
        closing_reserves: Money,
    },
}

/// Cash proceeds and opening contractual face of one named debt source.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LboDebtFunding {
    /// Unique identity matching the source debt instrument.
    pub name: String,
    /// Actual cash proceeds credited at closing, in transaction currency.
    pub cash_proceeds: Money,
    /// Contractual face outstanding immediately after closing.
    pub opening_face: Money,
}

/// Dated transaction funding assumptions authored on one currency basis.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LboFunding {
    /// Effective investment and refinancing date.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Purchase consideration and refinancing election.
    pub acquisition: LboAcquisition,
    /// Nonnegative adviser, financing, and transaction expenses paid at close.
    pub transaction_fees: Money,
    /// Separate nonnegative credit against otherwise required closing uses.
    pub closing_credits: Money,
    /// Admitted cash debt sources and their separately identified opening face.
    pub debt: Vec<LboDebtFunding>,
    /// Actual sponsor cash committed at entry, or absent for a sizing case.
    pub sponsor_commitment: Option<Money>,
}

/// Reconciled signed funding residual and its separately reported cash and face.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct LboFundingResult {
    /// Total transaction cash uses after separately authored credits.
    pub cash_uses: Money,
    /// Cash proceeds from all admitted debt sources.
    pub debt_cash_proceeds: Money,
    /// Contractual face outstanding at entry, separate from proceeds.
    pub opening_debt_face: Money,
    /// Uses less debt cash proceeds, before applying sponsor cash.
    pub required_sponsor_cash: Money,
    /// Total explicitly committed cash sources when a sponsor commitment is supplied.
    pub committed_cash_sources: Option<Money>,
    /// Uses less committed cash sources. A positive amount is a shortage.
    pub funding_gap: Option<Money>,
}

impl LboFunding {
    /// Reconcile entry proceeds and uses without rounding away a funding gap.
    ///
    /// # Arguments
    /// * `entry_enterprise_value` - Nonnegative appraised EV in transaction
    ///   currency, used as payment only for a declared cash-free, debt-free sale.
    ///
    /// # Errors
    /// Rejects negative amounts, mismatched currencies, duplicate source identities,
    /// a negative net cash use, or an amount outside the checked Money domain.
    pub fn reconcile(&self, entry_enterprise_value: Money) -> Result<LboFundingResult> {
        let currency = entry_enterprise_value.currency();
        let check = |name, amount| validate_amount(name, amount, currency);
        check("entry enterprise value", entry_enterprise_value)?;
        check("transaction fees", self.transaction_fees)?;
        check("closing credits", self.closing_credits)?;
        let purchase_uses = match self.acquisition {
            LboAcquisition::CashFreeDebtFree => entry_enterprise_value,
            LboAcquisition::SellerAndRefinancing {
                seller_consideration,
                refinancing,
                closing_reserves,
            } => {
                check("seller consideration", seller_consideration)?;
                check("refinancing", refinancing)?;
                check("closing reserves", closing_reserves)?;
                seller_consideration
                    .checked_add(refinancing)?
                    .checked_add(closing_reserves)?
            }
        };
        let cash_uses = purchase_uses
            .checked_add(self.transaction_fees)?
            .checked_sub(self.closing_credits)?;
        check("net cash uses", cash_uses)?;

        let mut names = std::collections::HashSet::new();
        let mut proceeds = Money::zero(currency);
        let mut face = Money::zero(currency);
        for debt in &self.debt {
            if debt.name.trim().is_empty() || !names.insert(debt.name.as_str()) {
                return Err(Error::invalid_input(
                    "LBO debt source identities must be nonempty and unique",
                ));
            }
            check("debt cash proceeds", debt.cash_proceeds)?;
            check("opening debt face", debt.opening_face)?;
            proceeds = proceeds.checked_add(debt.cash_proceeds)?;
            face = face.checked_add(debt.opening_face)?;
        }
        let committed_cash_sources = self
            .sponsor_commitment
            .map(|commitment| -> Result<Money> {
                check("sponsor cash commitment", commitment)?;
                Ok(proceeds.checked_add(commitment)?)
            })
            .transpose()?;
        Ok(LboFundingResult {
            cash_uses,
            debt_cash_proceeds: proceeds,
            opening_debt_face: face,
            required_sponsor_cash: cash_uses.checked_sub(proceeds)?,
            committed_cash_sources,
            funding_gap: committed_cash_sources
                .map(|sources| cash_uses.checked_sub(sources))
                .transpose()?,
        })
    }
}

fn validate_amount(label: &str, amount: Money, currency: Currency) -> Result<()> {
    if amount.currency() != currency || amount.amount_decimal() < rust_decimal::Decimal::ZERO {
        return Err(Error::invalid_input(format!(
            "LBO {label} must be nonnegative in {currency}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn dated_entry_proceeds_and_commitment_do_not_hide_shortfall() {
        let usd = |amount| Money::from((amount, Currency::USD));
        let funding = LboFunding {
            date: date!(2025 - 12 - 31),
            acquisition: LboAcquisition::SellerAndRefinancing {
                seller_consideration: usd(620_i64),
                refinancing: usd(150),
                closing_reserves: usd(20),
            },
            transaction_fees: usd(30),
            closing_credits: usd(0),
            debt: vec![LboDebtFunding {
                name: "term".into(),
                cash_proceeds: usd(500),
                opening_face: usd(520),
            }],
            sponsor_commitment: Some(usd(300)),
        };
        let result = funding.reconcile(usd(800)).unwrap();
        assert_eq!(result.cash_uses, usd(820));
        assert_eq!(result.opening_debt_face, usd(520));
        assert_eq!(result.required_sponsor_cash, usd(320));
        assert_eq!(result.funding_gap, Some(usd(20)));
    }
}

/// Dated and reconciled sponsor economics on the same numeraire as entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LboExitBridge {
    /// Enterprise value at the elected exit date.
    pub enterprise_value: Money,
    /// Debt less admitted cash at exit, which may be negative.
    pub net_debt: Money,
    /// Other signed claims deducted ahead of common equity.
    pub other_claims: Money,
    /// Enterprise value less debt and claims, before the equity option floor.
    pub common_equity_residual: Money,
    /// Common equity distribution floored at zero.
    pub sponsor_proceeds: Money,
    /// Proceeds over implied required cash, when required cash is positive.
    pub sized_moic: Option<f64>,
    /// Proceeds over actual cash only when the commitment reconciles the uses.
    pub funded_moic: Option<f64>,
}

impl LboFundingResult {
    /// Allocate proceeds to common equity using this entry funding bridge.
    ///
    /// # Arguments
    /// * `enterprise_value` - Nonnegative exit EV in entry transaction currency.
    /// * `net_debt` - Signed debt less cash on the same exit date and currency.
    /// * `other_claims` - Signed nondebt claims ahead of common equity on that date.
    ///
    /// # Errors
    /// Rejects mismatched currency, negative EV and unrepresentable results.
    pub fn allocate_exit(
        &self,
        enterprise_value: Money,
        net_debt: Money,
        other_claims: Money,
    ) -> Result<LboExitBridge> {
        let currency = self.cash_uses.currency();
        validate_amount("exit enterprise value", enterprise_value, currency)?;
        let common_equity_residual = enterprise_value
            .checked_sub(net_debt)?
            .checked_sub(other_claims)?;
        let sponsor_proceeds =
            if common_equity_residual.amount_decimal() < rust_decimal::Decimal::ZERO {
                Money::zero(currency)
            } else {
                common_equity_residual
            };
        let moic = |sponsor_cash: Money| -> Result<Option<f64>> {
            if sponsor_cash.amount_decimal() <= rust_decimal::Decimal::ZERO {
                return Ok(None);
            }
            let multiple = sponsor_proceeds.amount() / sponsor_cash.amount();
            if !multiple.is_finite() {
                return Err(Error::invalid_input(
                    "LBO multiple on invested capital cannot be represented",
                ));
            }
            Ok(Some(multiple))
        };
        let sized_moic = moic(self.required_sponsor_cash)?;
        let funded_moic = match (self.funding_gap, self.committed_cash_sources) {
            (Some(gap), Some(sources)) if gap.amount_decimal().is_zero() => {
                moic(sources.checked_sub(self.debt_cash_proceeds)?)?
            }
            _ => None,
        };
        Ok(LboExitBridge {
            enterprise_value,
            net_debt,
            other_claims,
            common_equity_residual,
            sponsor_proceeds,
            sized_moic,
            funded_moic,
        })
    }
}

/// Authored earnings observation and timing for one transaction multiple.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LboEarnings {
    /// Exact node name supplying the elected earnings amount in transaction currency.
    pub node: String,
    /// Existing statement period containing the closing observation.
    pub period: PeriodId,
    /// Price divided by earnings, in positive multiple turns.
    pub multiple: DecimalWire,
    /// Earnings dimension matched to the multiple's convention.
    pub basis: LboEarningsBasis,
}

/// How a matched earnings observation is read.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LboEarningsBasis {
    /// Use one authored observation that already reflects the multiple's basis.
    AsObserved,
    /// Sum twelve contiguous months of additive statement earnings.
    TrailingAdditive,
}

impl LboEarnings {
    fn value(
        &self,
        model: &FinancialModelSpec,
        results: &StatementResult,
        date: Date,
        currency: Currency,
        entry: bool,
    ) -> Result<Money> {
        if self.multiple.0 <= rust_decimal::Decimal::ZERO {
            return Err(Error::invalid_input(
                "LBO earnings valuation multiples must be positive exact decimals",
            ));
        }
        let period = results
            .get_periods()
            .iter()
            .find(|period| period.id == self.period)
            .ok_or_else(|| {
                Error::invalid_input(format!(
                    "LBO selected earnings period {} is absent",
                    self.period
                ))
            })?;
        if period.end.previous_day() != Some(date) || period.is_actual != entry {
            return Err(Error::invalid_input(
                "LBO earnings period must close on the matching actual entry or projected exit date",
            ));
        }
        let earnings = match self.basis {
            LboEarningsBasis::AsObserved => {
                super::corporate::monetary_node_money(results, &self.node, &self.period, currency)?
            }
            LboEarningsBasis::TrailingAdditive => super::corporate::trailing_year_metric(
                model, results, &self.node, period.end, currency,
            )?,
        };
        if earnings.amount_decimal() < rust_decimal::Decimal::ZERO
            || (entry && earnings.amount_decimal().is_zero())
        {
            return Err(Error::invalid_input(
                "LBO entry earnings must be positive and exit earnings nonnegative",
            ));
        }
        Ok(earnings)
    }
}

/// Node-id mappings from which the LBO model check suite is built.
///
/// The suite runs against the same evaluated statement results used for the
/// transaction arithmetic, so the report and the returns describe one
/// evaluation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LboCheckMappings {
    /// Three-statement node mapping (balance sheet, income statement, cash flow).
    pub three_statement: ThreeStatementMapping,
    /// Credit node mapping used for the leverage and coverage checks.
    pub credit: CreditMapping,
}

/// The check suite a dated LBO runs on its own statement result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LboCheckSuite {
    /// An explicit suite of built-in and formula checks.
    Spec {
        /// Built-in and formula checks to resolve and run.
        spec: finstack_quant_statements::checks::CheckSuiteSpec,
    },
    /// The LBO model suite: the three-statement and credit-underwriting
    /// checks merged by [`lbo_model_checks`], which replaces the credit
    /// mapping's leverage warning range with 0 to 8.
    Lbo {
        /// Node mappings the two merged suites read.
        mappings: Box<LboCheckMappings>,
    },
}

impl LboCheckSuite {
    /// Build the runnable suite.
    fn resolve(&self) -> Result<finstack_quant_statements::checks::CheckSuite> {
        match self {
            Self::Spec { spec } => spec.resolve(),
            Self::Lbo { mappings } => {
                lbo_model_checks(mappings.three_statement.clone(), mappings.credit.clone())
            }
        }
    }
}

/// Optional statement check suite and its effect on the LBO qualification.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LboQualification {
    /// Checks run on the same statement result used for the transaction.
    pub suite: LboCheckSuite,
    /// Require a passing suite before publishing a funded return.
    pub required: bool,
}

/// Point-in-time acquisition, recognized cash sources and projected exit plan.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LboRequest {
    /// Valuation date, reporting currency and entry observation cut.
    pub cut: super::corporate::StatementValuationCut,
    /// Exit date in the verified projected statement.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub exit_valuation_date: Date,
    /// Earnings basis and matching purchase multiple on the entry date.
    pub entry: LboEarnings,
    /// Date, actual closing uses and named admissible cash sources.
    pub funding: LboFunding,
    /// Projected earnings basis and exit multiple at the elected exit date.
    pub exit: LboEarnings,
    /// Exact projected closing debt node, measured as financing face.
    pub exit_debt_node: String,
    /// Exact projected cash node eligible in the exit equity bridge.
    pub exit_cash_node: String,
    /// Exact projected non-debt claims or selling costs payable ahead of equity.
    pub exit_claims_node: Option<String>,
    /// Optional report and decision policy for statement checks.
    pub checks: Option<LboQualification>,
}

/// Dated, qualified reconstruction of closing sources and prospective returns.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LboResult {
    /// Accepted statement run for all financial inputs and checks.
    pub run: finstack_quant_statements::evaluator::RunProvenance,
    /// Effective closing date used for the initial funding bridge.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub entry_date: Date,
    /// Projected exit date used for the common-equity bridge.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub exit_valuation_date: Date,
    /// Resolved reported entry earnings in the transaction currency.
    pub entry_earnings: Money,
    /// Entry enterprise value from the elected purchase multiple.
    pub entry_enterprise_value: Money,
    /// Reconciled initial uses, cash proceeds, opening face and funding gap.
    pub funding: LboFundingResult,
    /// Projected earnings in the same currency.
    pub exit_earnings: Money,
    /// Signed exit residual and applicable sponsor returns.
    pub exit: LboExitBridge,
    /// Verdict and evidence from the elected statement checks.
    pub checks: Option<CheckReport>,
}

/// Evaluate dated LBO acquisition and exit economics on one verified statement cut.
///
/// # Arguments
/// * `model` - Identified statement model holding matched actual and projected periods.
/// * `request` - Authoritative closing dates, same-currency funding and exit assumptions.
/// * `market` - Market inputs required by any linked capital-structure observations.
///
/// # Errors
/// Rejects incompatible dates, incomplete statement evidence, wrong units or
/// currency, invalid funding, or a failed or incomplete required check suite.
pub fn evaluate_lbo(
    model: &FinancialModelSpec,
    request: &LboRequest,
    market: Option<&MarketContext>,
) -> Result<LboResult> {
    let entry_date = request.cut.as_of;
    if request.funding.date != entry_date || request.exit_valuation_date <= entry_date {
        return Err(Error::invalid_input(
            "LBO requires matching entry and funding dates before the exit date",
        ));
    }
    let results = request.cut.evaluate(model, market)?;
    let run = request.cut.check_result(model, &results)?.clone();
    let currency = request.cut.currency;
    let entry_earnings = request
        .entry
        .value(model, &results, entry_date, currency, true)?;
    let entry_enterprise_value = entry_earnings.checked_mul(request.entry.multiple.0)?;
    let funding = request.funding.reconcile(entry_enterprise_value)?;

    let exit_earnings = request.exit.value(
        model,
        &results,
        request.exit_valuation_date,
        currency,
        false,
    )?;
    let exit_enterprise_value = exit_earnings.checked_mul(request.exit.multiple.0)?;
    let amount = |node| {
        super::corporate::monetary_node_money(&results, node, &request.exit.period, currency)
    };
    let debt = amount(&request.exit_debt_node)?;
    let cash = amount(&request.exit_cash_node)?;
    if debt.amount_decimal() < rust_decimal::Decimal::ZERO
        || cash.amount_decimal() < rust_decimal::Decimal::ZERO
    {
        return Err(Error::invalid_input(
            "LBO closing debt face and eligible cash must be nonnegative",
        ));
    }
    let other_claims = request
        .exit_claims_node
        .as_deref()
        .map(amount)
        .transpose()?
        .unwrap_or_else(|| Money::zero(currency));
    let exit =
        funding.allocate_exit(exit_enterprise_value, debt.checked_sub(cash)?, other_claims)?;

    let checks = request
        .checks
        .as_ref()
        .map(|check| {
            let suite = check.suite.resolve()?;
            let report = suite.run(model, &results)?;
            if check.required
                && report.outcome != finstack_quant_statements::checks::CheckOutcome::Pass
            {
                return Err(Error::eval(format!(
                    "Required LBO check suite returned {:?}",
                    report.outcome
                )));
            }
            Ok(report)
        })
        .transpose()?;
    Ok(LboResult {
        run,
        entry_date,
        exit_valuation_date: request.exit_valuation_date,
        entry_earnings,
        entry_enterprise_value,
        funding,
        exit_earnings,
        exit,
        checks,
    })
}

#[cfg(test)]
mod funding_tests {
    use super::*;
    #[test]
    fn lbo_distinguishes_sizing_funded_return_and_limited_liability() {
        use finstack_quant_statements::builder::ModelBuilder;
        use time::macros::date;

        let entry = PeriodId::annual(2025).unwrap();
        let exit = PeriodId::annual(2028).unwrap();
        let close = date!(2025 - 12 - 31);
        let usd = |value| Money::from((value, Currency::USD));
        let mut builder = ModelBuilder::new("dated-buyout");
        builder.periods("2025..2028", Some("2025")).unwrap();
        builder.with_meta("currency", serde_json::json!("USD"));
        builder.source_snapshot("actuals-2025-12-31");
        builder.value_money("ebitda", &[(entry, usd(100_i64)), (exit, usd(120))]);
        builder
            .availability_dates("ebitda", &[(entry, close)])
            .unwrap();
        builder.value_money("funded_debt", &[(exit, usd(440))]);
        builder.value_money("eligible_cash", &[(exit, usd(40))]);
        let model = builder.build().unwrap();
        let earnings = |period, multiple| LboEarnings {
            node: "ebitda".into(),
            period,
            multiple: DecimalWire(rust_decimal::Decimal::from(multiple)),
            basis: LboEarningsBasis::AsObserved,
        };
        let mut request = LboRequest {
            cut: super::super::corporate::StatementValuationCut {
                as_of: close,
                observed_through: close,
                currency: Currency::USD,
            },
            exit_valuation_date: date!(2028 - 12 - 31),
            entry: earnings(entry, 8_i64),
            funding: LboFunding {
                date: close,
                acquisition: LboAcquisition::CashFreeDebtFree,
                transaction_fees: usd(20),
                closing_credits: usd(0),
                debt: vec![LboDebtFunding {
                    name: "term".into(),
                    cash_proceeds: usd(500),
                    opening_face: usd(520),
                }],
                sponsor_commitment: Some(usd(300)),
            },
            exit: earnings(exit, 9),
            exit_debt_node: "funded_debt".into(),
            exit_cash_node: "eligible_cash".into(),
            exit_claims_node: None,
            checks: None,
        };
        let shortage = evaluate_lbo(&model, &request, None).unwrap();
        assert_eq!(shortage.funding.funding_gap, Some(usd(20)));
        assert_eq!(shortage.exit.sized_moic, Some(2.125));
        assert_eq!(shortage.exit.funded_moic, None);
        request.funding.sponsor_commitment = Some(usd(320));
        let funded = evaluate_lbo(&model, &request, None).unwrap();
        assert_eq!(funded.exit.funded_moic, Some(2.125));
        request.exit = earnings(exit, 2);
        let insolvency = evaluate_lbo(&model, &request, None).unwrap();
        assert_eq!(insolvency.exit.common_equity_residual, usd(-160));
        assert_eq!(insolvency.exit.sponsor_proceeds, usd(0));
        assert_eq!(insolvency.exit.funded_moic, Some(0.0));
    }

    /// The funded worked example with one scalar node for formula checks.
    fn checked_example() -> (FinancialModelSpec, LboRequest) {
        use finstack_quant_statements::builder::ModelBuilder;
        use time::macros::date;

        let entry = PeriodId::annual(2025).unwrap();
        let exit = PeriodId::annual(2028).unwrap();
        let close = date!(2025 - 12 - 31);
        let usd = |value| Money::from((value, Currency::USD));
        let mut builder = ModelBuilder::new("dated-buyout");
        builder.periods("2025..2028", Some("2025")).unwrap();
        builder.with_meta("currency", serde_json::json!("USD"));
        builder.source_snapshot("actuals-2025-12-31");
        // Every node holds all four years, so a suite that reads each period is complete.
        let years: Vec<PeriodId> = (2025..=2028)
            .map(|year| PeriodId::annual(year).unwrap())
            .collect();
        let series = |values: [i64; 4]| -> Vec<(PeriodId, Money)> {
            years.iter().copied().zip(values.map(usd)).collect()
        };
        builder.value_money("ebitda", &series([100, 105, 112, 120]));
        builder.value_money("funded_debt", &series([520, 500, 470, 440]));
        builder.value_money("eligible_cash", &series([10, 20, 30, 40]));
        let headroom: Vec<_> = years.iter().map(|year| (*year, 1.5.into())).collect();
        builder.value("covenant_headroom", &headroom);
        for node in [
            "ebitda",
            "funded_debt",
            "eligible_cash",
            "covenant_headroom",
        ] {
            builder.availability_dates(node, &[(entry, close)]).unwrap();
        }
        let model = builder.build().unwrap();
        let earnings = |period, multiple: i64| LboEarnings {
            node: "ebitda".into(),
            period,
            multiple: DecimalWire(rust_decimal::Decimal::from(multiple)),
            basis: LboEarningsBasis::AsObserved,
        };
        let request = LboRequest {
            cut: super::super::corporate::StatementValuationCut {
                as_of: close,
                observed_through: close,
                currency: Currency::USD,
            },
            exit_valuation_date: date!(2028 - 12 - 31),
            entry: earnings(entry, 8),
            funding: LboFunding {
                date: close,
                acquisition: LboAcquisition::CashFreeDebtFree,
                transaction_fees: usd(20),
                closing_credits: usd(0),
                debt: vec![LboDebtFunding {
                    name: "term".into(),
                    cash_proceeds: usd(500),
                    opening_face: usd(520),
                }],
                sponsor_commitment: Some(usd(320)),
            },
            exit: earnings(exit, 9),
            exit_debt_node: "funded_debt".into(),
            exit_cash_node: "eligible_cash".into(),
            exit_claims_node: None,
            checks: None,
        };
        (model, request)
    }

    fn spec_suite(formula: &str) -> LboCheckSuite {
        serde_json::from_value(serde_json::json!({
            "kind": "spec",
            "spec": {
                "name": "lbo",
                "builtin_checks": [],
                "formula_checks": [{
                    "id": "headroom",
                    "name": "Covenant headroom",
                    "category": "internal_consistency",
                    "severity": "error",
                    "mode": "predicate",
                    "scope": "all_periods",
                    "formula": formula,
                    "message_template": "Headroom out of bounds in {period}",
                    "tolerance": null
                }]
            }
        }))
        .unwrap()
    }

    fn lbo_mappings() -> LboCheckMappings {
        serde_json::from_value(serde_json::json!({
            "three_statement": {
                "assets_nodes": ["ebitda"],
                "liabilities_nodes": ["funded_debt"],
                "equity_nodes": ["eligible_cash"],
                "cash_node": "eligible_cash",
                "retained_earnings_node": "eligible_cash",
                "net_income_node": "ebitda"
            },
            "credit": {
                "debt_node": "funded_debt",
                "ebitda_node": "ebitda",
                "interest_expense_node": "ebitda"
            }
        }))
        .unwrap()
    }

    #[test]
    fn a_spec_suite_runs_on_the_dated_result_and_blocks_only_when_required() {
        use finstack_quant_statements::checks::CheckOutcome;

        let (model, mut request) = checked_example();
        request.checks = Some(LboQualification {
            suite: spec_suite("covenant_headroom > 1"),
            required: true,
        });
        let passed = evaluate_lbo(&model, &request, None).unwrap();
        assert_eq!(passed.checks.unwrap().outcome, CheckOutcome::Pass);
        assert_eq!(passed.exit.funded_moic, Some(2.125));

        request.checks = Some(LboQualification {
            suite: spec_suite("covenant_headroom > 10"),
            required: false,
        });
        let reported = evaluate_lbo(&model, &request, None).unwrap();
        assert_eq!(reported.checks.unwrap().outcome, CheckOutcome::Fail);
        assert_eq!(reported.exit.funded_moic, Some(2.125));

        request.checks = Some(LboQualification {
            suite: spec_suite("covenant_headroom > 10"),
            required: true,
        });
        let blocked = evaluate_lbo(&model, &request, None).unwrap_err();
        assert!(
            blocked
                .to_string()
                .contains("Required LBO check suite returned Fail")
        );
    }

    #[test]
    fn the_lbo_suite_is_lbo_model_checks_run_on_the_dated_result() {
        let (model, mut request) = checked_example();
        let mappings = lbo_mappings();
        request.checks = Some(LboQualification {
            suite: LboCheckSuite::Lbo {
                mappings: Box::new(mappings.clone()),
            },
            required: false,
        });

        let report = evaluate_lbo(&model, &request, None)
            .unwrap()
            .checks
            .unwrap();

        let results = request.cut.evaluate(&model, None).unwrap();
        let direct = lbo_model_checks(mappings.three_statement, mappings.credit)
            .unwrap()
            .run(&model, &results)
            .unwrap();
        assert_eq!(
            serde_json::to_value(&report).unwrap(),
            serde_json::to_value(&direct).unwrap()
        );
        assert!(!report.results.is_empty());
    }

    #[test]
    fn the_check_suite_choice_is_tagged_on_the_wire() {
        let spec = serde_json::to_value(spec_suite("covenant_headroom > 1")).unwrap();
        assert_eq!(spec["kind"], "spec");
        assert!(spec["spec"]["formula_checks"].is_array());

        let lbo = serde_json::to_value(LboCheckSuite::Lbo {
            mappings: Box::new(lbo_mappings()),
        })
        .unwrap();
        assert_eq!(lbo["kind"], "lbo");
        assert_eq!(lbo["mappings"]["credit"]["debt_node"], "funded_debt");

        let untagged =
            serde_json::json!({ "name": "lbo", "builtin_checks": [], "formula_checks": [] });
        assert!(serde_json::from_value::<LboCheckSuite>(untagged).is_err());
        let unknown = serde_json::json!({ "kind": "zzz" });
        assert!(serde_json::from_value::<LboCheckSuite>(unknown).is_err());
        let extra = serde_json::json!({ "kind": "lbo", "mappings": lbo["mappings"], "zzz": 1 });
        assert!(serde_json::from_value::<LboCheckSuite>(extra).is_err());
    }
}
