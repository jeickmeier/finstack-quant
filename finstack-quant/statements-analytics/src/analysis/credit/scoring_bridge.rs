//! Statement observations admitted by the existing statistical credit score models.

use crate::analysis::units::finite_ratio;
use finstack_quant_core::dates::{Date, PeriodId, PeriodKind};
use finstack_quant_core::money::Money;
use finstack_quant_models::credit::scoring::{
    AltmanZDoublePrimeInput, AltmanZPrimeInput, AltmanZScoreInput, ScoringResult, ZmijewskiInput,
    altman_em_score, altman_z_double_prime, altman_z_prime, altman_z_score, zmijewski_score,
};
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::{Error, Result};

/// Reporting model and any observation required beyond the statements.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "model", rename_all = "snake_case", deny_unknown_fields)]
pub enum CreditScoringModel {
    /// Original Altman Z-score for public manufacturing companies.
    AltmanZ {
        /// Dated equity market capitalization in the statement currency.
        market_equity: Money,
        /// Date of market capitalization, matching the selected financial close.
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        as_of: Date,
    },
    /// Altman Z-prime for privately held manufacturing companies.
    AltmanZPrime,
    /// Four-factor Altman Z-double-prime for nonmanufacturing companies.
    AltmanZDoublePrime,
    /// Altman emerging-market score using book equity and four factors.
    AltmanEm,
    /// Zmijewski's accounting-ratio probit score.
    Zmijewski,
}

/// Exact statement node IDs supporting the selected statistical score.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StatementScoringMapping {
    /// Closing total assets, a positive same-currency monetary observation.
    pub total_assets: String,
    /// Closing total liabilities in statement currency.
    pub total_liabilities: String,
    /// Closing net working capital used by the Altman models.
    pub working_capital: Option<String>,
    /// Closing retained earnings used by the Altman models.
    pub retained_earnings: Option<String>,
    /// Full-year EBIT used by the Altman models.
    pub ebit: Option<String>,
    /// Full-year sales used by the manufacturing Altman models.
    pub sales: Option<String>,
    /// Closing book equity for the book-value Altman models.
    pub book_equity: Option<String>,
    /// Closing current assets for Zmijewski.
    pub current_assets: Option<String>,
    /// Closing current liabilities for Zmijewski.
    pub current_liabilities: Option<String>,
    /// Full-year net income for Zmijewski.
    pub net_income: Option<String>,
}

/// Calculate one canonical score using a matched full-year statement period.
///
/// # Arguments
/// * `results` - Evaluated monetary observations and their authoritative period plan.
/// * `period` - Annual period with trailing full-year earnings and closing balance-sheet amounts.
/// * `mapping` - Exact node identities required by the chosen model.
/// * `model` - Elected score, including matched market capitalization for original Altman Z.
///
/// # Errors
/// Rejects missing required inputs, absent or nonannual periods, mixed currency,
/// nonpositive denominators, inappropriate market-equity inputs and an invalid
/// numerical score.
pub fn score_statement(
    results: &StatementResult,
    period: &PeriodId,
    mapping: &StatementScoringMapping,
    model: &CreditScoringModel,
) -> Result<ScoringResult> {
    let period = results
        .get_periods()
        .iter()
        .find(|row| row.id == *period)
        .filter(|row| row.id.kind() == PeriodKind::Annual)
        .ok_or_else(|| {
            Error::invalid_input("A credit score requires an enrolled full-year statement period")
        })?;
    let close = period
        .end
        .previous_day()
        .ok_or_else(|| Error::invalid_input("Invalid score closing date"))?;
    let assets = results
        .get_money(&mapping.total_assets, &period.id)
        .ok_or_else(|| {
            Error::invalid_input("Credit score total assets require a monetary observation")
        })?;
    let currency = assets.currency();
    let amount = |node: &str| -> Result<f64> {
        let money = results.get_money(node, &period.id).ok_or_else(|| {
            Error::invalid_input(format!(
                "Credit score requires monetary node '{node}' on {}",
                period.id
            ))
        })?;
        if money.currency() != currency {
            return Err(Error::CurrencyMismatch(currency, money.currency()));
        }
        Ok(money.amount())
    };
    let required = |node: &Option<String>, role: &str| -> Result<f64> {
        amount(node.as_deref().ok_or_else(|| {
            Error::invalid_input(format!("Selected credit model requires the {role} node"))
        })?)
    };
    let ratio = |num: f64, denom: f64| -> Result<f64> {
        if !denom.is_finite() || denom <= 0.0 {
            return Err(Error::invalid_input(
                "Credit score denominators must be finite and positive",
            ));
        }
        finite_ratio(num, denom)
    };
    let assets = assets.amount();
    let liabilities = amount(&mapping.total_liabilities)?;
    let altman_factors = || -> Result<[f64; 3]> {
        Ok([
            ratio(
                required(&mapping.working_capital, "working capital")?,
                assets,
            )?,
            ratio(
                required(&mapping.retained_earnings, "retained earnings")?,
                assets,
            )?,
            ratio(required(&mapping.ebit, "EBIT")?, assets)?,
        ])
    };
    let result = match model {
        CreditScoringModel::AltmanZ {
            market_equity,
            as_of,
        } => {
            if *as_of != close
                || market_equity.currency() != currency
                || market_equity.amount() < 0.0
            {
                return Err(Error::invalid_input(
                    "Market capitalization must be nonnegative in statement currency at the annual close",
                ));
            }
            let [
                working_capital_to_total_assets,
                retained_earnings_to_total_assets,
                ebit_to_total_assets,
            ] = altman_factors()?;
            altman_z_score(&AltmanZScoreInput {
                working_capital_to_total_assets,
                retained_earnings_to_total_assets,
                ebit_to_total_assets,
                market_equity_to_total_liabilities: ratio(market_equity.amount(), liabilities)?,
                sales_to_total_assets: ratio(required(&mapping.sales, "sales")?, assets)?,
            })
        }
        CreditScoringModel::AltmanZPrime => {
            let [
                working_capital_to_total_assets,
                retained_earnings_to_total_assets,
                ebit_to_total_assets,
            ] = altman_factors()?;
            altman_z_prime(&AltmanZPrimeInput {
                working_capital_to_total_assets,
                retained_earnings_to_total_assets,
                ebit_to_total_assets,
                book_equity_to_total_liabilities: ratio(
                    required(&mapping.book_equity, "book equity")?,
                    liabilities,
                )?,
                sales_to_total_assets: ratio(required(&mapping.sales, "sales")?, assets)?,
            })
        }
        CreditScoringModel::AltmanZDoublePrime | CreditScoringModel::AltmanEm => {
            let [
                working_capital_to_total_assets,
                retained_earnings_to_total_assets,
                ebit_to_total_assets,
            ] = altman_factors()?;
            let input = AltmanZDoublePrimeInput {
                working_capital_to_total_assets,
                retained_earnings_to_total_assets,
                ebit_to_total_assets,
                book_equity_to_total_liabilities: ratio(
                    required(&mapping.book_equity, "book equity")?,
                    liabilities,
                )?,
            };
            if matches!(model, CreditScoringModel::AltmanEm) {
                altman_em_score(&input)
            } else {
                altman_z_double_prime(&input)
            }
        }
        CreditScoringModel::Zmijewski => zmijewski_score(&ZmijewskiInput {
            net_income_to_total_assets: ratio(
                required(&mapping.net_income, "net income")?,
                assets,
            )?,
            total_liabilities_to_total_assets: ratio(liabilities, assets)?,
            current_assets_to_current_liabilities: ratio(
                required(&mapping.current_assets, "current assets")?,
                required(&mapping.current_liabilities, "current liabilities")?,
            )?,
        }),
    };
    result.map_err(|error| Error::invalid_input(error.to_string()))
}

/// Calculate canonical scores for explicitly selected annual periods in request order.
///
/// # Arguments
/// * `results` - Evaluated monetary observations and their period plan.
/// * `rows` - Unique periods and their elected scoring models. Market
///   capitalization, when required, belongs to each period's close.
/// * `mapping` - Shared named statement nodes required by the elected methods.
///
/// # Errors
/// Rejects repeated periods or any observation invalid for its requested model.
pub fn score_statement_series(
    results: &StatementResult,
    rows: &[(PeriodId, CreditScoringModel)],
    mapping: &StatementScoringMapping,
) -> Result<Vec<ScoringResult>> {
    let mut periods = std::collections::HashSet::with_capacity(rows.len());
    rows.iter()
        .map(|(period, model)| {
            if !periods.insert(*period) {
                return Err(Error::invalid_input(format!(
                    "Credit score period {period} was selected twice"
                )));
            }
            score_statement(results, period, mapping, model)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_statements::{builder::ModelBuilder, evaluator::Evaluator};

    #[test]
    fn zmijewski_only_reads_its_required_full_year_nodes() {
        let period = PeriodId::annual(2025).unwrap();
        let mut builder = ModelBuilder::new("annual-scoring");
        builder.periods("2025..2025", None).unwrap();
        for (node, amount) in [
            ("assets", 1000_i64),
            ("liabilities", 500),
            ("income", 75),
            ("liquid_assets", 200),
            ("short_debt", 125),
        ] {
            builder.value_money(node, &[(period, Money::from((amount, Currency::USD)))]);
        }
        let results = Evaluator::new()
            .evaluate(&builder.build().unwrap())
            .unwrap();
        let mapping = StatementScoringMapping {
            total_assets: "assets".into(),
            total_liabilities: "liabilities".into(),
            working_capital: None,
            retained_earnings: None,
            ebit: None,
            sales: None,
            book_equity: None,
            current_assets: Some("liquid_assets".into()),
            current_liabilities: Some("short_debt".into()),
            net_income: Some("income".into()),
        };
        let scored =
            score_statement(&results, &period, &mapping, &CreditScoringModel::Zmijewski).unwrap();
        assert!(
            (scored.score - (-4.336 - 4.513 * 0.075 + 5.679 * 0.5 + 0.004 * 1.6)).abs() < 1e-12
        );
        assert!(
            score_statement(
                &results,
                &PeriodId::quarter(2025, 4).unwrap(),
                &mapping,
                &CreditScoringModel::Zmijewski,
            )
            .is_err()
        );
        assert!(
            score_statement_series(
                &results,
                &[
                    (period, CreditScoringModel::Zmijewski),
                    (period, CreditScoringModel::Zmijewski)
                ],
                &mapping,
            )
            .is_err()
        );
    }
}
