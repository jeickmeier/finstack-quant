# STATEMENTS-CAPITAL-001: Instrument construction and supported capital-structure domains

**Decision status:** Accepted for opt-in waterfall prepayments and explicit bond and swap construction (2026-11-15).
**Implementation status:** The accepted prepayment contract is enforced by the default stack, waterfall validation and capital-structure validation. The Statements construction helpers `add_bond`, `add_bond_with_convention`, `add_swap` and `add_swap_with_conventions`, their Python and WASM counterparts and their parameter types and schemas are removed (2026-11-17); every caller builds the canonical Valuations instrument and attaches it through `add_debt` or `addDebt`. The sections below describe the implementation audited before reconciliation.
**Validation status:** Scoped Rust, Python and WASM regression checks passed for opt-in prepayments on 2026-11-15. Attachment and wire replay are qualified: the model holds the canonical bond and swap unchanged, with resolved terms, elected convention and dated schedule equal before and after a round trip, in Rust, Python and WASM. Independent contractual schedule qualification remains open under the capital evidence entry.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; [scope and file ledger](README.md).

## Choice and rationale

Statements uses the typed Valuations instrument registry and its cashflow implementations. Instrument construction and capital-structure admission each have their own validation.

## Accepted decision for opt-in waterfall prepayments

The default waterfall pays fees, interest, scheduled principal and then equity. This stack supports scheduled debt service for admitted bonds, loans and hedged capital structures without an inactive sweep priority triggering prepayment validation.

An excess-cash-flow sweep requires an explicit `Sweep` priority and an `ecf_sweep` sizing specification, including when the sweep percentage is zero. Mandatory and voluntary prepayments each require their own explicit priority and matching sizing expression. Any prepayment priority restricts the entire capital structure to term loans and revolving credit facilities, whose residual schedules the prepayment engine can rebuild.

## Accepted decision for explicit bond and swap construction

Construct bonds and interest-rate swaps through their canonical Valuations APIs using fully authored terms or a deliberately selected qualified dated convention. Keep the elected source, terms and contractual schedule with the canonical instrument for model use and replay. This gives the Statements evaluator the same resolved cashflow contract as other instrument consumers. [Dated convention selection](../valuations-framework/convention-selection.md#accepted-decision-for-checked-convention-selection-and-dated-sources) · [Bond schedule contract](../valuations-bonds/01-schedule-and-holder-cashflows.md#accepted-decision-for-checked-bond-schedules-and-holder-cash) · [Interest-rate swap contract](../valuations-rates/03-interest-rate-swaps.md#accepted-decision-for-checked-interest-rate-swap-cashflows-and-risk).

Attach the instrument through `add_debt` in Rust and Python or `addDebt` in WASM. Rust accepts the typed `FinancialStatementInstrument`. Python accepts instrument objects or canonical instrument envelopes. WASM accepts the canonical instrument envelope. Remove the Statements helpers `add_bond`, `add_bond_with_convention`, `add_swap` and `add_swap_with_conventions` and their WASM counterparts.

## Audited construction and current routing

1. `add_bond` builds a fixed coupon bond using finite annual decimal coupon, tagged principal, issue/maturity dates and discount-curve ID. Its US corporate preset uses semiannual 30/360 and a short front stub. `add_bond_with_convention` delegates the caller's regional preset to the bond constructor.
2. `add_swap` delegates to the custom-convention builder with semiannual 30/360 fixed and quarterly ACT/360 floating legs and Modified Following. Both swap helpers produce a pay-fixed, zero-spread contract with simple floating compounding, no stubs or end-of-month roll, and zero reset and payment lags. They use the currency's standard rates calendar. A currency without a mapped calendar requires an explicitly constructed instrument.
3. `add_debt` appends an already typed `FinancialStatementInstrument`; it does not itself perform the full enclosing-model validation. `build_instrument_from_spec` converts that union to the canonical registry representation and returns an `Arc<dyn Instrument>`, attaching the outer ID to conversion failures.
4. Reporting-currency/FX setters populate optional capital-structure fields; no capital structure is created until needed. The waterfall setter calls `WaterfallSpec::validate` before storing it. Full instrument/target validation belongs to the enclosing model, and duplicate outer IDs are rejected by evaluator initialization.
5. Classification admits borrowing principal for Bond, Convertible, TermLoan and RevolvingCredit; Irs, CapFloor and Swaption are hedge/option categories. Other instrument types error. Placeholder cashflow representations error even for an admitted option type; contingent payouts are not invented.
6. The enclosing `CapitalStructureSpec::validate` rejects hedge and option PIK targets and enforces the instrument scope of the accepted prepayment contract.

## Evidence

- **Implementation:** [builder](../../../../finstack-quant/statements/src/capital_structure/builder.rs), `add_bond`, `add_swap_with_conventions`, `add_debt`, and `waterfall`; [registry adapter](../../../../finstack-quant/statements/src/capital_structure/integration.rs), `build_instrument_from_spec`; [declared-kind and placeholder selection](../../../../finstack-quant/statements/src/capital_structure/period_flows.rs).
- **Prepayment opt-in:** `default_priority_of_payments` defines the scheduled-payment stack, and `WaterfallSpec::validate` requires each prepayment priority and its sizing input together. [Waterfall configuration](../../../../finstack-quant/statements/src/capital_structure/waterfall_spec.rs).
- **Cross-boundary selection** uses `CapitalStructureSpec::validate` for instrument admission. Evaluator initialization belongs to the statements-evaluation scope. [Model types](../../../../finstack-quant/statements/src/types/model.rs).
- **Prepayment regressions** cover missing sweep sizing, missing sweep priority at zero and positive percentages, and valid paired configuration. [Waterfall configuration tests](../../../../finstack-quant/statements/src/capital_structure/waterfall_spec.rs).
- **Executed check scope.** A clean copy of `001a3716cd` with the opt-in change passed 921 Statements Rust tests, 31 Python capital-structure tests and 26 Statements WASM facade tests. `mise run gen-write` refreshed the schemas and declarations in the same copy.
- **Instrument regressions** cover placeholder rejection, PIK target eligibility and fixed-rate evaluation. Model tests reject bonds with prepayment priorities. [Senior-review capital tests](../../../../finstack-quant/statements/src/capital_structure/tests/senior_review_capital.rs) and [model tests](../../../../finstack-quant/statements/src/types/model.rs).

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-capital-structure).
