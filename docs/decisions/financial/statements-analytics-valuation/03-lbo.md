# STATEMENTS-ANALYTICS-VALUATION-03: LBO entry funding bridge and exit equity multiple

- **Decision status:** Accepted for the full Statements LBO contract (2026-11-15).
- **Implementation status:** Reconciliation pending.
- **Validation status:** Existing evidence mapped. Independent qualification pending.
- **Audit date/source revision:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Scope:** [Inventory](README.md); [source coverage](coverage.json).

## Accepted decision for Statements LBO funding and returns

1. Require explicit entry and exit dates, with exit later than entry. Select named amounts in one currency and match the earnings basis to each multiple. An elected LTM basis requires complete coverage. Verify the entry statement snapshot and cutoff, and identify the projected exit assumptions. Require positive multiples and entry earnings. Zero exit earnings are valid, and negative earnings are outside the method's domain.
2. Calculate entry enterprise value from the entry multiple and matched earnings. Use that enterprise value as purchase consideration for a declared cash-free, debt-free transaction. Otherwise, reconcile seller consideration and itemize applicable refinancing, closing reserves and fees. Fees are nonnegative, and credits are separate. Use dated debt proceeds as a cash source and reconcile opening debt face separately. The residual determines required sponsor equity. Reconcile the supplied sponsor commitment and other sources to uses in checked `Money` and report any funding gap or surplus.
3. Bridge exit enterprise value using dated debt, eligible cash and separately modeled senior claims or sale expenses. Retain the signed equity residual. Ordinary limited-liability common equity proceeds have a zero floor. Entry-to-exit MOIC uses positive sponsor cash invested as its denominator. A sizing case may report implied MOIC, and a funded-case MOIC requires reconciled funding. Interim flows and IRR belong to a separate dated calculation.
4. Use the same verified statement results for elected checks. Report their verdict and coverage. Advisory findings accompany the bridge. A failed or incomplete required check blocks a validated outcome. [Check-suite outcomes](../statements-contracts/14-comparison-and-suite-policy.md#accepted-decision-for-numerical-comparisons-and-check-suites).

For example, USD 820 of uses and USD 500 of debt proceeds require USD 320 of sponsor equity. A USD 300 commitment leaves a USD 20 shortfall. With USD 320 invested, USD 1,080 of exit enterprise value less USD 400 of net debt yields USD 680 of common equity proceeds and a 2.125× MOIC.

## Audited choice, domain and selection

[LBO evaluation](../../../../finstack-quant/statements-analytics/src/analysis/valuation/lbo.rs) is an entry/exit transaction arithmetic layer over evaluated statements. The entry metric is from the **first model period**, not an automatically constructed LTM period. Exit metric and net debt come from the configured exit period. Currency metadata and finite monetary node values in that currency are mandatory.

Entry/exit multiples and transaction fees must be finite, but are not separately constrained positive. Each named funded-debt tranche is finite and nonnegative; tranche names do not select debt amortization or covenant logic. The statement model supplies exit debt and earnings paths.

## Audited algorithm and conventions

1. Evaluate statements without an additional market/as-of interface.
2. Calculate entry EV `Mentry × entry_metric`; uses are entry EV plus transaction fees. Sum configured debt tranches.
3. Define sponsor equity **as the residual**, `equity_check=uses−debt`. Require it strictly positive. Sources are `debt+equity_check`; `sources_uses_balanced` tests equality with uses at `1e−9 × max(abs(sources),abs(uses),1)`.
4. Calculate exit EV `Mexit × exit_metric`, exit equity proceeds `exit_EV−exit_net_debt`, and MOIC `exit_equity_proceeds/equity_check`. There is no zero floor on exit equity proceeds.
5. If check mappings are supplied, run the preconfigured LBO statement/credit check suite and return its report. A report containing failed checks is returned with the computed LBO result rather than automatically blocking it.

Money construction rejects nonfinite money outputs. The algebraically residual equity means `sources_uses_balanced` primarily checks arithmetic consistency; it is not an independent confirmation that sponsor commitments match uses. MOIC has no time dimension. Interim dividends, sponsor follow-on funding, IRR, fees on exit and negotiated limited-liability payoff rules are not reconstructed.

## Audited alternatives and evidence

A distinct explicit sponsor-equity source with an independently tested funding gap, dated sponsor cashflow IRR, and debt-schedule-driven transaction model would provide different outputs; none is silently inferred here. Negative multiples/fees or negative exit equity may be useful stress inputs but require a defined permitted domain.

Colocated `evaluate_lbo_reproduces_the_transaction_bridge` supplies a hand-sized arithmetic example; fully funded/negative-tranche/currency tests establish rejected domains. They do not validate sponsor economics or calibrated exits. [Koller](../../../REFERENCES.md#koller-valuation) and [Rosenbaum–Pearl](../../../REFERENCES.md#rosenbaum-pearl-2020) are reference context.

No tests or market/transaction reconciliations were executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-valuation-comparisons-and-templates).
