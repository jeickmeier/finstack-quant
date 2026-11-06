# STATEMENTS-ANALYTICS-VALUATION-03: LBO entry funding bridge and exit equity multiple

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed in the defined source scope linked below; discrepancies and limits are explicit.
- **Validation status:** Evidence inspected or located, not executed. No numerical qualification or external reconciliation is inferred.
- **Audit date/source revision:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Scope:** [Inventory](README.md); [source coverage](coverage.json).

## Choice, domain and selection

[LBO evaluation](../../../../finstack-quant/statements-analytics/src/analysis/valuation/lbo.rs) is an entry/exit transaction arithmetic layer over evaluated statements. The entry metric is from the **first model period**, not an automatically constructed LTM period. Exit metric and net debt come from the configured exit period. Currency metadata and finite monetary node values in that currency are mandatory.

Entry/exit multiples and transaction fees must be finite, but are not separately constrained positive. Each named funded-debt tranche is finite and nonnegative; tranche names do not select debt amortization or covenant logic. The statement model supplies exit debt and earnings paths.

## Algorithm and conventions

1. Evaluate statements without an additional market/as-of interface.
2. Calculate entry EV `Mentry × entry_metric`; uses are entry EV plus transaction fees. Sum configured debt tranches.
3. Define sponsor equity **as the residual**, `equity_check=uses−debt`. Require it strictly positive. Sources are `debt+equity_check`; `sources_uses_balanced` tests equality with uses at `1e−9 × max(abs(sources),abs(uses),1)`.
4. Calculate exit EV `Mexit × exit_metric`, exit equity proceeds `exit_EV−exit_net_debt`, and MOIC `exit_equity_proceeds/equity_check`. There is no zero floor on exit equity proceeds.
5. If check mappings are supplied, run the preconfigured LBO statement/credit check suite and return its report. A report containing failed checks is returned with the computed LBO result rather than automatically blocking it.

Money construction rejects nonfinite money outputs. The algebraically residual equity means `sources_uses_balanced` primarily checks arithmetic consistency; it is not an independent confirmation that sponsor commitments match uses. MOIC has no time dimension. Interim dividends, sponsor follow-on funding, IRR, fees on exit and negotiated limited-liability payoff rules are not reconstructed.

## Alternatives and evidence

A distinct explicit sponsor-equity source with an independently tested funding gap, dated sponsor cashflow IRR, and debt-schedule-driven transaction model would provide different outputs; none is silently inferred here. Negative multiples/fees or negative exit equity may be useful stress inputs but require a defined permitted domain.

Colocated `evaluate_lbo_reproduces_the_transaction_bridge` supplies a hand-sized arithmetic example; fully funded/negative-tranche/currency tests establish rejected domains. They do not validate sponsor economics or calibrated exits. [Koller](../../../REFERENCES.md#koller-valuation) and [Rosenbaum–Pearl](../../../REFERENCES.md#rosenbaum-pearl-2020) are reference context.

No tests or market/transaction reconciliations were executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-valuation-comparisons-and-templates).
