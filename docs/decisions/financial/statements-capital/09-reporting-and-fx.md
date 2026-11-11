# STATEMENTS-CAPITAL-009: Cashflow reporting, accumulation and FX boundary

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed for the source scope and discrepancies stated below.
**Validation status:** Evidence inspected or located, not executed; numerical qualification remains open.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; [scope and file ledger](README.md).

## Choice and rationale

Native-currency instrument breakdowns, native-currency totals and optional reporting totals are distinct. Components expose already-computed amounts; they never perform implicit FX conversion. Audit principal components explain total principal without creating additional cash. This separates reporting from allocation and preserves currency identity.

## Reporting and accumulation

`CashflowBreakdown` carries positive expense/payment/receipt magnitudes, debt closing and accrued interest. `interest_expense_total = cash expense + PIK`; it does **not** subtract income. The separate net-cash helper returns cash expense minus income. Missing income resolves to a zero in the cash-expense currency. Optional `opening_balance` and four principal components can be absent in manually supplied/wire data; evaluator outputs populate them. Scheduled, mandatory, sweep and voluntary components sum to principal payment and must not be added to it again.

`validate_currency_invariant` checks all required/optional monetary fields against the expense currency. Checked Money additions protect combined values. Optional aggregation initializes a missing total from a present component, adds checked values when both exist, and preserves absence when no component exists.

`get_component` accepts exactly `interest_expense`, `interest_expense_cash`, `interest_expense_pik`, `interest_income`, `principal_payment`, `debt_balance`, `fees`, `accrued_interest`. An exact `total` selector reads reporting totals; otherwise it reads the exact instrument key. Unknown components, absent periods/instruments and unavailable multicurrency totals error. Results are f64 major-currency units with currency metadata available only in the underlying maps.

`set_period` merges supplied entries and overwrites equal keys across instrument, total, native total, equity and available-cash maps. Reporting currency is the first non-None value and is not reconciled to later supplied currencies by this method. It does not remove old rows omitted by the replacement. `period_snapshot` filters all maps to one period and preserves reporting currency.

`periods()` returns total-map insertion order when totals exist; otherwise it returns the sorted unique instrument-period union. Its blanket “ascending order” documentation is stronger than the totals branch. Long-table exports preserve map/flow order, label total rows `__total__`, and include currency on every row. Optional components are emitted only when present; schema/envelope validation is distinct from financial reconciliation.

## FX conversion and established date policy

`convert_to_reporting` is identity for same currency. Otherwise an FX matrix is required; query it with `(source,target,on,policy)`, multiply the native amount by returned rate and build tagged reporting Money. Missing routes/rates, invalid values and amount representation failures propagate. It does not inspect original cashflow dates.

The evaluator owns the aggregate conversion call and supplies the inclusive period-end snapshot after aggregating `cs.*` cash buckets. There is no default policy (resolution O9, 2026-11-11): a cross-currency conversion without `fx_policy` errors, and same-currency totals need none. The established direction is **period-close waterfall/capital-structure aggregation**; preserve that direction. Builder prose claiming `CashflowDate` uses each flow's date conflicts with this aggregated caller. A CashflowDate enum value cannot recover dates already removed by aggregation. This is documentation/API reconciliation, not a new choice between period-close and per-flow accounting.

## Limitations, alternatives and evidence

These DTOs are mutable and do not enforce all cross-map consistency at construction/merge. A dedicated ledger validation boundary could reject mixed reporting currencies, unsorted totals or inconsistent component sums, while current code depends on evaluator-produced data. Sorted outputs and replace-versus-merge semantics are separate API choices. Neither table rows nor totals certify GAAP/IFRS accounting.

- **Implementation:** [reporting types/accessors](../../../../finstack-quant/statements/src/capital_structure/cashflows.rs), `flows`, `get_component`, `periods`, `set_period`, `period_snapshot`, table exporters; [FX helper](../../../../finstack-quant/statements/src/capital_structure/integration.rs); [FX builder prose](../../../../finstack-quant/statements/src/capital_structure/builder.rs); [module date contract](../../../../finstack-quant/statements/src/capital_structure/mod.rs).
- **Cross-boundary:** evaluator orchestration/FX aggregation is documented by the statements-evaluation scope; no duplicate evaluator ownership is claimed.
- **Regression evidence:** inline cashflow currency, absent-income, serde and multicurrency-total cases; [waterfall integration](../../../../finstack-quant/statements/src/capital_structure/tests/waterfall_integration.rs) and [test module](../../../../finstack-quant/statements/src/capital_structure/tests/mod.rs) cover integration expectations. Internal schema/control evidence, not executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-capital-structure).
