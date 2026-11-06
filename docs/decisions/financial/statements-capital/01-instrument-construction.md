# STATEMENTS-CAPITAL-001: Instrument construction and supported capital-structure domains

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed for the source scope and discrepancies stated below.
**Validation status:** Evidence inspected or located, not executed; numerical qualification remains open.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; [scope and file ledger](README.md).

## Choice and rationale

Statement instruments are constructed through the typed valuations registry; convenience builders select explicit bond and swap conventions before appending successfully constructed instruments. The observed separation reuses instrument cashflow implementations and avoids partially mutating a builder on constructor failure. It does not establish that every registered instrument supplies usable debt-service cashflows.

## Inputs and algorithm

1. `add_bond` builds a fixed coupon bond using finite annual decimal coupon, tagged principal, issue/maturity dates and discount-curve ID. It selects US corporate conventions: semiannual 30/360 and short-front stub. `add_bond_with_convention` instead delegates the caller's regional preset to the bond constructor.
2. `add_swap` delegates to the custom-convention builder with semiannual 30/360 fixed and quarterly ACT/360 floating legs and Modified Following. Both swap paths remain pay-fixed, zero spread, simple floating compounding; they have no stub, no end-of-month roll, zero reset lag and zero payment lag. They use the currency's standard rates calendar. A currency without one errors and requires an explicitly constructed instrument.
3. `add_debt` appends an already typed `FinancialStatementInstrument`; it does not itself perform the full enclosing-model validation. `build_instrument_from_spec` converts that union to the canonical registry representation and returns an `Arc<dyn Instrument>`, attaching the outer ID to conversion failures.
4. Reporting-currency/FX setters populate optional capital-structure fields; no capital structure is created until needed. The waterfall setter calls `WaterfallSpec::validate` before storing it. Full instrument/target validation belongs to the enclosing model, and duplicate outer IDs are rejected by evaluator initialization.
5. Classification admits borrowing principal for Bond, Convertible, TermLoan and RevolvingCredit; Irs, CapFloor and Swaption are hedge/option categories. Other instrument types error. Placeholder cashflow representations error even for an admitted option type; contingent payouts are not invented.
6. The enclosing `CapitalStructureSpec::validate` rejects hedge/option PIK targets. Any `Sweep`, `MandatoryPrepayment` or `VoluntaryPrepayment` priority rejects Bond/Convertible and derivatives in the capital structure: the implemented residual prepayment engine is for loans/revolvers. This restriction is triggered by the presence of a rung, including the default `Sweep` when no ECF amount is configured.

## Selection, limitations and alternatives

Convenience conventions are actual defaults, not inference from currency or evidence of market suitability. Regional bond construction and fully explicit typed instruments provide alternatives. The default swap is a simple term-index contract; using an overnight curve ID does not make it an overnight-compounded swap. All market/fixing requirements still come from the underlying raw-schedule implementation.

Unsupported prepayment domains are rejected rather than approximated with bond exercise/redemption rules. An instrument-aware prepayment adapter could broaden support but would require call premiums, eligibility and residual economics; that is not implemented or approved here. The default waterfall's `Sweep` means a bond-only scheduled-payment waterfall must explicitly use a stack without prepayment rungs.

## Evidence

- **Implementation:** [builder](../../../../finstack-quant/statements/src/capital_structure/builder.rs), `add_bond`, `add_swap_with_conventions`, `add_debt`, and `waterfall`; [registry adapter](../../../../finstack-quant/statements/src/capital_structure/integration.rs), `build_instrument_from_spec`; [declared-kind and placeholder selection](../../../../finstack-quant/statements/src/capital_structure/period_flows.rs).
- **Cross-boundary selection:** [model types](../../../../finstack-quant/statements/src/types/model.rs), `CapitalStructureSpec::validate`; evaluator initialization belongs to the statements-evaluation scope. These sources establish gates, not numerical accuracy.
- **Regression evidence:** [senior-review capital tests](../../../../finstack-quant/statements/src/capital_structure/tests/senior_review_capital.rs), `placeholder_rate_option_schedule_is_rejected_explicitly`, `pik_toggle_rejects_hedge_targets_and_keeps_explicit_debt_targets`, and fixed-rate/plain-evaluation cases; model tests reject bonds with prepayment rungs. Tests were not run.

No default is adopted by this record.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-capital-structure).
