# Statement capital-structure methodologies

**Decision status:** The scoped decisions below were accepted on 2026-11-15. Other choices retain the status in their owning records.
**Implementation status:** Observed for the 19 assigned Rust files at `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; specific source/documentation discrepancies are recorded below.
**Validation status:** Source and selected test evidence inspected or located, not executed; no numerical accuracy or accounting-compliance certification.
**Audit date:** 2026-11-05.

This scope covers all **19 assigned files** under `finstack-quant/statements/src/capital_structure/`: **13 algorithm files, one support facade and five dedicated test files**. [The exact source ledger](coverage.json) records each file once with its reviewed SHA-256 and links to nine algorithm records. All production bodies were inspected, including validation, helper algorithms, fallbacks and code around test markers. Inline tests are evidence inside their production owners, not separate source-count entries.

The scope owns instrument construction adapters, period debt-service extraction, principal/PIK state, residual schedules, the corporate debt waterfall and its reporting helpers. It does not duplicate the underlying cashflow/instrument pricing engines or evaluator orchestration: model periods, DSL dependency evaluation, sequential/fixed-point scheduling and aggregate FX calls belong to their respective scopes. The existing **period-close capital-structure/waterfall FX direction remains in force**; conflicting CashflowDate prose is a reconciliation item.

The inventory below describes the audited source. The current default waterfall
contains only fees, interest, scheduled principal and equity. Each prepayment
requires an explicit priority and sizing input. [Accepted prepayment decision](01-instrument-construction.md#accepted-decision-for-opt-in-waterfall-prepayments).

The accepted instrument construction path attaches canonical Valuations bonds
and swaps through `add_debt` or `addDebt`. The Statements helpers are removed
and their callers reconciled. [Accepted instrument construction decision](01-instrument-construction.md#accepted-decision-for-explicit-bond-and-swap-construction).

Coupon settlements net within each instrument and reporting period. The
evaluator aggregates the resulting expense and income separately.
[Accepted coupon netting decision](02-cashflow-classification.md#accepted-decision-for-per-instrument-coupon-netting).

The accepted distress policy reports identified settlements separately from
scheduled debt service. Implementation is pending.
[Accepted distress reporting decision](02-cashflow-classification.md#accepted-decision-for-separate-distress-reporting).

The complete cashflow classification contract was accepted on 2026-11-15. It
covers period admission, native currency and source perspective, dated cash
and principal effects, and contract-based fee and hedge classification.
Period admission, the stated perspective, exhaustive signed classification and
separate dated distress cash are implemented and qualified; fee rebates,
separate funding proceeds and ledger qualification remain.
[Accepted cashflow classification contract](02-cashflow-classification.md#accepted-decision-for-period-admission-and-cashflow-classification).

Opening principal reconciliation requires the dated schedule and stateful debt
ledger to agree before calculating debt service for carried debt. Implementation
is pending. [Accepted opening principal decision](03-funding-and-balances.md#accepted-decision-for-opening-principal-reconciliation).

Contractual accrued interest covers cash interest earned through period close
and due afterward. Separate arrears reporting remains pending.
[Accepted contractual accrual decision](03-funding-and-balances.md#accepted-decision-for-contractual-accrued-cash-interest).

The dated principal contract covers claims, separate economic and settlement
balances, advance reserves, scheduled payment order and exact atomic
reconciliation. Implementation and independent qualification remain pending.
[Accepted dated principal decision](04-dated-principal.md#accepted-decision-for-dated-principal-claims).

The residual coupon contract uses canonical terms to rebuild affected coupon
parents, principal and fees after dated balance changes. Implementation and
independent qualification remain pending.
[Accepted residual coupon decision](05-residual-coupons.md#accepted-decision-for-residual-coupon-rebuilding).

The waterfall contract governs available cash, priority and seniority, payment
rounding, exact conservation and terminal Equity or retained cash.
Implementation and independent qualification remain pending.
[Accepted waterfall allocation decision](06-priority-allocation.md#accepted-decision-for-waterfall-cash-allocation-and-conservation).

The prepayment and ECF sweep contract governs agreement elections, eligible
facilities, dated sizing and payment caps. Implementation and independent
qualification remain pending.
[Accepted prepayment and ECF decision](07-prepayments-and-ecf.md#accepted-decision-for-agreement-defined-prepayments-and-ecf-sweeps).

The PIK contract governs dated coupon elections, coupon-period duration,
capitalization and separate arrears. Implementation and independent
qualification remain pending.
[Accepted PIK decision](08-pik-and-carry-state.md#accepted-decision-for-contractual-pik-elections-and-carried-claims).

The reporting contract covers native and consolidated views, explicit
period-close FX, exact component exports and atomic period replacement.
Implementation and independent qualification remain pending.
[Accepted capital reporting decision](09-reporting-and-fx.md#accepted-decision-for-capital-reporting-and-fx).

| Calculation / supported instruments | Implemented method / material variants | Actual selection | Decision record | Implementation / validation | Scope limits and tracking |
|---|---|---|---|---|---|
| Bond and swap construction | Canonical Valuations instrument, with authored terms or a selected dated convention, attached through `add_debt` / `addDebt` | No Statements construction helper | [001 — construction](01-instrument-construction.md) | Reconciled 2026-11-17; attachment and wire replay of terms, convention and schedule executed in Rust, Python and WASM | Independent schedule qualification is tracked with the capital evidence entry |
| Runtime instrument/schedule domain | Typed registry, declared borrowing versus hedge categories, placeholder rejection | Raw full schedule unless residual schedule supplied | [001](01-instrument-construction.md) | Observed; placeholder and PIK-target fixtures | Admitted option kind does not imply modeled contingent payouts |
| Cash debt service and hedge income | Company cash converted once from the schedule's stated perspective; period-net coupons (fixed, floating, stub, inflation) split into expense `max(−C,0)` and income `max(C,0)` per instrument | Declared instrument kind and stated perspective, not a coupon sign heuristic | [002 — classification](02-cashflow-classification.md) | Reconciled 2026-11-17; negative-rate and zero-leg netting and an inflation coupon executed | A supplied borrowing schedule must be in the holder perspective |
| Principal, PIK, fee and distress classification | Every kind named: repayments and fees the company pays are claims; signed PIK from its principal delta; write-downs move the balance only; recovery, accrual on default and protection payments are dated distress cash outside debt service; margin and unknown kinds rejected | Cashflow kind and company cash sign | [002](02-cashflow-classification.md) | Reconciled 2026-11-17; the record's write-down-with-settlement example, a rebate, a margin row and a PIK correction executed | Fee rebates and refunds are rejected until they are reported as receipts |
| Reporting cash and balance clocks | Cash `[start,end)`; opening strictly before start; inclusive `end−1` balance/accrual | Actual supplied periods | [002](02-cashflow-classification.md) | Reconciled 2026-11-17 for period admission (an empty or reversed period fails before a schedule is read) and the March 31 / April 1 example; boundary fixtures | Same-period hedge netting and uniform invalid-period rejection |
| Funding principal and OID separation | Positive explicit principal deltas; signed cash fallback; issue-period implicit funding only without any funding leg | Economic balance date; zero opening/new issuance gate | [003 — funding/balances](03-funding-and-balances.md) | Observed; issuance/OID/PIK fixtures | Face preservation is not EIR/OID amortization |
| Stateful cash scaling | Piecewise zero/new-funding rules then raw opening/scheduled ratio with warning | Ratio outside `1±1e−6` warned, not clamped | [003](03-funding-and-balances.md) | Observed; stale clamp prose identified | [Follow-ups](../FOLLOW_UPS.md#statements-capital-structure) |
| Closing debt and accrued interest | Economic schedule closing normally; zero-opening/new-funding cash-bucket reconstruction; shared accrued-interest kernel | Branch by debt kind, opening and current funding | [003](03-funding-and-balances.md) | Observed; ordinary issuance tests | [Follow-ups](../FOLLOW_UPS.md#statements-capital-structure) |
| Dated principal allocation | Separate economic/settlement capacities, carried arrears, future-paid reserve and sequential claim caps | Exact current dated claims; aggregate fallback forbids advance state | [004 — dated principal](04-dated-principal.md) | Observed; early/late settlement and no-double-pay fixtures | [Follow-ups](../FOLLOW_UPS.md#statements-capital-structure) |
| Residual coupon projection | Component-rate inference, segment accrual at principal changes, preserve earned interest and cash/PIK shares | Rebuild after closing delta above `1e−9` | [005 — residual coupons](05-residual-coupons.md) | Observed; split-coupon/paydown/rebuild cases | Custom accrual assumptions and irregular calendar qualification |
| Future repayment/PIK reconstruction | Economic event ordering, reserve arrears/advances, cap repayments and final redemption, propagate PIK deltas | Future economic principal and future cash gates differ | [005](05-residual-coupons.md) | Observed; maturity/late-PIK cases | Independent multi-period contractual ledger evidence |
| Waterfall validation and expression inputs | Required categories, unique priorities/classes, exact target checks; finite node/inline expression evaluation; direct substring guard | Optional configured waterfall; empty flow map no-op | [006 — allocation](06-priority-allocation.md) | Observed; nonfinite/currency/transactional fixtures | Cash-pool guard is not transitive dependency validation |
| Cash caps and seniority | Ascending class rank, pro rata within class, negative claim neutralization; state commits on success | Configured stack; empty classes imply one rank | [006](06-priority-allocation.md) | Observed; conservation/ordering checks | Floating residual goes to final staged row; independent rounding bound |
| Shortfall/dust/equity close | `1e−6` material claim threshold; `0.005` balance dust floor; all residual cash to equity | Equity output even without Equity rung | [006](06-priority-allocation.md) | Observed; cash-identity and arrears fixtures | Currency-agnostic floor and retained-cash semantics |
| ECF formula | EBITDA minus optional taxes/capex/ΔWC, explicit or earlier-paid cash interest, earlier-paid fees and scheduled principal | Sweep percentage `[0,1]`; missing optional deductions zero | [007 — prepayments/ECF](07-prepayments-and-ecf.md) | Observed; paid-arrears and stack-order fixtures | Actual agreement definition; earlier mandatory/voluntary not scheduled deduction |
| Mandatory/voluntary/ECF execution | Independent requests, cash and economic-principal caps, class allocation, optional exact ECF target | Each configured rung; no later-rung reservation | [007](07-prepayments-and-ecf.md) | Observed; independent sizing/priority cases | At-par only; no type filter inside no-target helper; contract rights/provenance |
| PIK threshold and hysteresis | Finite metric `< threshold`; explicit IDs; consecutive evaluated-period minimum | Mode before allocation; threshold itself not finite-validated | [008 — PIK/state](08-pik-and-carry-state.md) | Observed; metric/hysteresis tests | Direct typed NaN/infinite threshold gap; period-frequency dependence |
| PIK capitalization and carried claims | Prior interest arrears join coupon before toggle; only toggle PIK added at close; fee/principal buckets retained | Contractual PIK uses dated ledger; state carried across periods | [008](08-pik-and-carry-state.md) | Observed; earned-accrual/dated-PIK cases | Arrears capitalization and post-paydown PIK permission |
| Reporting components and audit amounts | Gross cash+PIK expense, separate income/net helper, optional principal-component breakdown | Eight exact DSL components; native versus reporting-total lookup | [009 — reporting/FX](09-reporting-and-fx.md) | Observed; schema/currency/missing-income fixtures | Mutable data can bypass cross-map consistency |
| Accumulation/order/table projection | Key overwrite merge, first reporting currency retained; totals insertion order; long-table rows with currency | No implicit FX in lookup/export | [009](09-reporting-and-fx.md) | Observed; source discrepancy in ascending-order prose | Sort contract and mixed-currency merge expectations |
| Reporting FX boundary | Same-currency identity; explicit matrix/query date+policy; amount×rate | Aggregate evaluator supplies period-close snapshot; omitted policy PeriodEnd | [009](09-reporting-and-fx.md) | Observed; caller selection traced | [Follow-ups](../FOLLOW_UPS.md#statements-capital-structure) |

The records distinguish confirmed static contracts/discrepancies from numerical impacts that still need reproduction. Tests named here are internal formula, routing, state and regression evidence; they were **not executed**. The [statement-capital follow-ups](../FOLLOW_UPS.md#statements-capital-structure) track scoped findings; [central follow-ups](../../FOLLOW_UPS.md#financial-methodology-audit) coordinate cross-scope decisions; this audit changes no runtime code, tests, fixtures or adopted model policy.
