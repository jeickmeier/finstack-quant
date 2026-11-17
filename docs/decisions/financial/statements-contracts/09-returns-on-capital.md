# STATEMENTS-CONTRACTS-009: Bundled period returns on capital

**Decision status:** Accepted for the full bundled period returns contract (2026-11-15).

**Implementation status:** Reconciled (2026-11-17). The four bundled returns are `ratio(earnings, (lag(balance, 1) + balance) / 2)`: same-period, unannualized, over the average of the opening balance (the prior period's close) and the closing balance. ROE and ROA read the shared `fin.net_income`, after statement `tax_expense`, and ROCE and ROIC read the shared `fin.ebit`; no return keeps its own earnings expression or reads `taxes`. ROCE averages total assets less current liabilities at each endpoint. ROIC reads two explicit inputs named by owner resolution on 2026-11-17: `operating_tax` (signed; a credit raises the numerator) and `operating_invested_capital` (period-end operating assets less non-interest-bearing operating liabilities). `ratio` requires the same units on both sides and a positive base: a period without an opening balance, with a missing input or with a zero or negative average is unavailable with a reason, and a loss keeps its sign. Each return declares the `formula` rollup, so a coarser period recomputes it from its own earnings and its own opening and closing balances. The sections below the accepted decision describe the audited implementation.

**Validation status:** Qualified by hand-computed averages for the four returns, a first period without an opening balance, a signed loss, an operating tax credit, a negative average invested capital and a half-year recomputed from its own opening and closing balances.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Scope inventory](README.md).

## Accepted decision for bundled period returns on capital

Report same-period, unannualized decimal returns. Average the comparable opening and closing balances for each denominator. Use the same reporting scope and currency for earnings and the denominator. ROE and ROA use the shared reported-cost net income, including statement `tax_expense`. ROCE and ROIC use the shared reported-cost EBIT. [Reported-cost earnings](07-earnings-and-margins.md#accepted-decision-for-reported-cost-earnings-and-margins).

| Metric | Accepted formula |
|---|---|
| ROE | Net income / average equity in the same reporting scope |
| ROA | Net income / average total assets |
| ROCE | EBIT / average capital employed, calculated as total assets − current liabilities at each endpoint |
| ROIC | (EBIT − explicit operating tax expense or credit before financing effects) / average operating invested capital |

Determine operating invested capital at each endpoint from explicit operating assets less non-interest-bearing operating liabilities. Include required operating cash and exclude excess cash and nonoperating assets. Align the operating capital and tax inputs with the EBIT scope and currency. Operating tax is a separate input from statement `tax_expense`. A signed operating tax credit increases the numerator.

Require a positive denominator. Report unavailable with a reason when a required input is missing or the denominator is nonpositive. Preserve signed losses when the ratio is available. Recompute a return for a coarser reporting period using its earnings and opening and closing balances.

## Audited input domain

Four embedded fin formulas report ROE, ROA, ROIC and ROCE. They use current-period earnings and current period-end capital balances. They do not annualize earnings, average beginning/ending balances or infer a marginal tax rate. Rates are output as decimal fractions.

## Audited algorithm and controls

With EBIT_impl = revenue − cogs − opex − depreciation − amortization and NI_impl = EBIT_impl − interest_expense − taxes:

| Metric | Executed formula and denominator |
|---|---|
| roe | NI_impl / total_equity |
| roa | NI_impl / total_assets |
| roic | (EBIT_impl − taxes) / (total_debt + total_equity) |
| roce | EBIT_impl / (total_assets − current_liabilities) |

The taxes input is a distinct identifier from tax_expense used by fin.net_income; no alias, equality check or reconciliation connects them. Registry requires lists document these literal inputs but do not supply missing observations. The same ex-D&A OPEX assumption as [earnings](07-earnings-and-margins.md) is embedded directly in the return numerators.

ROIC approximates NOPAT by subtracting reported taxes from EBIT, rather than applying an operating marginal tax rate. The data description explicitly acknowledges that an interest tax shield can affect this approximation. Invested capital is debt plus equity without subtracting cash or distinguishing operating from financing assets.

## Limitations and alternatives

Quarterly earnings divided by a period-end balance are a quarterly return measure; they are not automatically comparable with a trailing or annual return. Material within-period financing or acquisitions can make endpoint and average-capital returns differ. Negative/near-zero denominators, tax credits and losses use the generic evaluator's arithmetic rather than a finance-specific policy.

The audited endpoint balances, duplicate earnings expressions and reported-tax approximation are implementation gaps under the accepted return formulas.

## Evidence

[Return catalogue](../../../../finstack-quant/statements/data/metrics/fin_returns.json), [registry tests](../../../../finstack-quant/statements/tests/spec/registry_tests.rs), [registry schema units](../../../../finstack-quant/statements/src/registry/schema.rs) and [corporate valuation reference](../../../REFERENCES.md#koller-valuation) provide implementation and contextual evidence. Tests inspect formula strings and model integration; these do not independently validate NOPAT tax treatment or denominator selection.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
