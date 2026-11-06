# STATEMENTS-CONTRACTS-009: Bundled period returns and reported-tax ROIC approximation

**Decision status:** Draft (observed implementation; adoption pending).

**Implementation status:** Observed for the source responsibilities stated below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice README](README.md)).

## Choice and input domain

Four embedded fin formulas report ROE, ROA, ROIC and ROCE. They use current-period earnings and current period-end capital balances. They do not annualize earnings, average beginning/ending balances or infer a marginal tax rate. Rates are output as decimal fractions.

## Algorithm and controls

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

The previously agreed reported-cost convention remains the primary reconciliation target. No alternative ex-D&A input convention is adopted by this record.

Average invested capital, TTM earnings, operating-tax NOPAT, cash-net invested capital and a shared net-income node would be materially different formulas. They require explicit choices, not silent reinterpretation of existing node names.

## Evidence

[Return catalogue](../../../../finstack-quant/statements/data/metrics/fin_returns.json), [registry tests](../../../../finstack-quant/statements/tests/spec/registry_tests.rs), [registry schema units](../../../../finstack-quant/statements/src/registry/schema.rs) and [corporate valuation reference](../../../REFERENCES.md#koller-valuation) provide implementation and contextual evidence. Tests inspect formula strings and model integration; these do not independently validate NOPAT tax treatment or denominator selection.

The reported-tax approximation remains observed behavior, not an adopted operating-tax model.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
