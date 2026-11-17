# STATEMENTS-CONTRACTS-007: Bundled earnings and margin formulas

**Decision status:** Accepted for the full bundled earnings and margin contract (2026-11-15).

**Implementation status:** Reconciled (2026-11-17). The bundled catalogue reads reported `cogs` and `opex` with their D&A included and follows the accepted table by reference: gross profit, operating income, EBIT as operating income, EBITDA as EBIT plus the included depreciation and amortization, EBT and net income. The four margins and two cost ratios use the new `ratio(numerator, base)` function, which gives a decimal ratio over a strictly positive base, is unavailable with reason `undefined_base` for a zero or negative base and propagates a missing input; a loss keeps its sign on positive revenue. Ratios roll up by recomputing the formula on aggregated inputs. The bundled return ratios no longer deduct D&A a second time.

**Validation status:** Qualified by a hand-worked income statement that separates the D&A included in COGS and OPEX from the single add-back, a negative expense credit, margins without positive revenue, a missing add-back and recomputation of a ratio from aggregated annual inputs.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Scope inventory](README.md).

## Accepted decision for reported-cost earnings and margins

Use reported COGS and OPEX with their depreciation and amortization included. Use inputs for the same reporting period and currency. Add back only the D&A included in those expense lines, once. Expense charges are positive. Actual credits and reversals retain their negative sign. Use an explicit zero for a known zero. A missing dependency makes the affected metric unavailable. [Formula units and currency](05-currency-dimensions.md#accepted-decision-for-formula-units-and-currency).

| Metric | Accepted formula |
|---|---|
| Gross profit | Revenue − reported COGS |
| Operating income | Gross profit − reported OPEX |
| EBIT | Operating income |
| EBITDA | EBIT + included depreciation + included amortization |
| EBT | EBIT − statement interest expense |
| Net income | EBT − statement tax expense |

The four margins use gross profit, operating income, EBITDA and net income as numerators. The two cost ratios use reported COGS and OPEX. Divide each by same-period revenue and report a decimal ratio. The ratio is unavailable with a reason when revenue is missing, zero or negative. For positive revenue, preserve the signed numerator. For a coarser reporting period, recompute the ratio using the aggregated numerator and revenue.

## Audited input domain

The sections below describe the implementation audited before reconciliation.

The embedded fin namespace contains six income-statement metrics and six revenue-normalized margin/cost ratios. They are ordinary statement DSL formulas evaluated with the model's observations, units, periods and missing-data behavior.

The audited catalogue explicitly describes OPEX excluding depreciation and amortization and supplies D&A as separate positive expense amounts. This differs from the accepted reported-cost convention.

## Audited algorithm and controls

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

The original audit did not locate a standalone ADR for the prior reported-cost direction.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
