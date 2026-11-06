# STATEMENTS-CONTRACTS-007: Bundled earnings and margin formulas

**Decision status:** Draft (observed implementation; adoption pending).

**Implementation status:** Observed for the source responsibilities stated below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Source baseline and audit date: [slice README](README.md).

## Choice and input domain

The embedded fin namespace contains six income-statement metrics and six revenue-normalized margin/cost ratios. They are ordinary statement DSL formulas evaluated with the model's observations, units, periods and missing-data behavior.

The current catalogue explicitly describes OPEX excluding depreciation and amortization and supplies D&A as separate positive expense amounts. This conflicts with the previously agreed reported-cost convention, under which D&A is already included in COGS/OPEX. The present audit records the implementation and a reconciliation requirement; it does not reopen that agreed primary convention or authorize a formula change.

## Algorithm and controls

Let R be revenue, C cost of goods sold, O operating expenses, D depreciation, A amortization, I interest expense and T tax_expense.

| Metric | Executed formula |
|---|---|
| gross_profit | R − C |
| ebitda | R − C − O |
| operating_income | R − C − O − D − A |
| ebit | ebitda − D − A |
| ebt | R − C − O − D − A − I |
| net_income | R − C − O − D − A − I − T |
| gross_margin | gross_profit / R |
| operating_margin | operating_income / R |
| ebitda_margin | ebitda / R |
| net_margin | net_income / R |
| cogs_as_pct_revenue | C / R |
| opex_as_pct_revenue | O / R |

Inputs use positive expense magnitudes: formulas subtract expenses explicitly. Revenue ratios are decimal fractions, without multiplication by 100. Each formula uses the current period's values and performs no annualization or trailing aggregation. A builder qualifies intra-registry references (for example ebitda to fin.ebitda) and preserves an already supplied node with the same qualified ID.

## Limitations and alternatives

The implemented EBITDA excludes a D&A add-back, while EBIT and downstream earnings deduct D&A separately. If reported COGS/OPEX already include the same D&A, this understates EBITDA and deducts D&A again for EBIT/net income. COGS' D&A content is not separately modelled in the catalogue. The problem affects downstream margins, leverage and returns as well as headline earnings.

There is no built-in sign normalization, zero-revenue policy, tax calculation or accounting reconciliation in these formulas. The evaluator supplies division/missing behavior. Registry percentage labels do not verify dimensions.

An explicit ex-D&A input convention can produce internally consistent arithmetic, but it is not the chosen primary reported-cost convention. The agreed reported-cost convention has no compatibility fallback that guesses whether costs contain D&A.

## Evidence

[Embedded basic formulas](../../../../finstack-quant/statements/data/metrics/fin_basic.json), [margins](../../../../finstack-quant/statements/data/metrics/fin_margins.json), [catalogue source](../../../../finstack-quant/statements/src/registry/builtins.rs) and [registry tests](../../../../finstack-quant/statements/tests/spec/registry_tests.rs) are the evidence. Inspected tests explicitly pin the ex-D&A formula strings and a numerical example R=1000, C=400, O=300, D=50, A=30 yielding EBITDA=300 and EBIT=operating_income=220. That is a formula/regression fixture under the implemented convention, not independent approval of that convention.

No current ADR asserting the prior reported-cost convention was located; no historical adoption record is invented here.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
