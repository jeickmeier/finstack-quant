# VALUATIONS-STRUCTURED-CREDIT-22: Cashflow classification, summary metrics and diagnostics

- **Decision status:** Accepted 2026-11-15 — complete cash and event reporting, diagnostics, repayment life, equity returns and collateral metrics package.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent ledger, repayment-life, equity-return and ratio calculations checked. Production ledger, reporting and host qualification remain pending.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Accepted decision for structured-credit diagnostics and reporting

1. **Use one authoritative ledger for cash, principal and claims.**
   Book signed settlement cash, signed principal effects and claim
   movements with actual dates, phases and identities. Record each
   funded draw once as a holder outflow with its corresponding
   principal increase. Keep repayment cash, retired par, redemption
   premium or discount, funded PIK, deferral and credit write-down
   distinct. PIK, deferral and write-down carry zero settlement cash.
   Preserve paid interest, arrears payments, fees and delayed or
   terminal receipts. Generate exports and metrics from booked events.
   Remove reconstruction of principal from cash at export.
   [Accepted waterfall claims and rounding](11-waterfall-claims-and-rounding.md#accepted-decision-for-waterfall-claims-and-rounding).

2. **Reconcile diagnostics by event, account and phase.** Reconcile
   opening balances, credits, debits and closing balances for each
   cash account, principal balance and carried claim in checked Money.
   Identify transfer sources and destinations, authorized rounding
   residuals, due and paid amounts and shortfalls. Remove the absolute
   conservation tolerance and small-shortfall floor. Link reserve
   income, draws, cures and triggers by event or period identity and
   phase. Date-only cash views aggregate flows after validation.
   Balances and test snapshots retain their phase. Identify observed
   history and projected results in reports.
   [Distribution and pool performance reporting, Item 1121](https://www.govinfo.gov/content/pkg/CFR-2023-title17-vol3/pdf/CFR-2023-title17-vol3-sec229-1121.pdf).

3. **Weight deterministic WAL by repaid par.** Use positive par retired
   by eligible repayments and ACT/365F to their actual payment dates.
   Include eligible same-day payments at zero time. Exclude funding,
   premiums and discounts, standalone PIK and noncash write-downs.
   Later genuine repayment of capitalized par counts. Remove the
   absolute-value funding fallback. WAL is unavailable without eligible
   positive repayments. Stochastic WAL uses record 15's eligible mean
   path life with its contributing count and fraction. The life of
   expected cash requires its own identified basis.
   [Accepted repayment life](../cashflows/14-aggregation-and-life.md#accepted-decision-exact-nominal-reporting-and-explicit-repayment-life) ·
   [Accepted stochastic results](15-stochastic-waterfall.md#accepted-decision-for-stochastic-waterfall-pricing-and-estimation).

4. **Require an explicit equity claim and investment basis.** Select
   the residual by ID. Use dated purchase consideration on a
   qualified buyer basis or expressly authorized retained capital.
   Remove implicit first-equity selection and par default. With
   positive investment `I`, cash-on-cash is signed net cash by
   date divided by `I` and MOIC is total signed net cash divided
   by `I`. Count later contributions as outflows. Use canonical
   ACT/365F XIRR on the dated entry `−I` and eligible signed cash.
   Require the first surviving date to be the entry with negative
   net cash and apply the one-sign-change gate. Report native NAV
   independently. NAV per 100 requires positive matched face.
   Publish the dated cash-on-cash series and remove its payback
   description.
   [Accepted XIRR](../core-conventions/017-irr.md) ·
   [Accepted facility residual authority](19-borrowing-base-facility.md#accepted-decision-for-borrowing-base-facilities-and-lender-cashflows).

5. **Report pool statistics and model rates on qualified populations.**
   Use record 02's dated snapshot and units for WAC, WAS, WAM, WARF,
   concentration and effective obligor count. Retain coverage and
   distinguish explicit unrated status from missing rating data.
   Label CPR and CDR as annual equivalents of resolved monthly model
   assumptions on a qualified applied month. Preserve source and
   exposure coverage. Aggregate monthly rates only on an admitted
   common basis before annualizing. Realized trailing rates and
   cumulative original-par targets require their own definitions.
   [Accepted pool-statistics rules](02-pool-normalization-and-statistics.md#accepted-decision-for-collateral-normalization-and-pool-statistics).

6. **Bind each collateral ratio to its denominator and cut.** Divide
   ABS gross charge-offs by qualified fixed original or cut-off par.
   Report gross defaults and realized recoveries separately. A requested
   net-loss ratio deducts eligible recoveries on the same cohort and
   cut. Delinquency uses qualified current par and dated buckets.
   Credit enhancement is `100 × (B−D)/B` on positive eligible carrying
   value `B` and an expressly named debt group `D`, including co-ranked
   notes when authorized. Count cash and recovery claims once under
   their eligibility rules. Identify descriptive snapshots separately
   from tested covenants. CMBS DSCR requires the full matched performing
   population, comparable annual NOI and regular contractual debt service.
   Missing required NOI leaves the ratio unavailable with coverage
   disclosed. Known zero NOI participates.
   [Filed ABS loss-ratio example](https://www.sec.gov/Archives/edgar/data/1405332/000119312512344414/d366991dex102.htm) ·
   [Accepted CMBS DSCR](06-cmbs-servicing.md#accepted-decision-for-cmbs-balloon-and-servicing).

7. **Preserve meaning and availability across report paths.** Reuse a
   prepared simulation only with matching claim, model, dates, market,
   history and path basis. Propagate required calculation failures.
   Distinguish genuine zero, unavailable outcomes and failed requests.
   Preserve typed empty tables. Validate units, currencies, shape and
   ledger reconciliation at restoration. Apply record 15's stochastic
   loss, tail confidence, PV uncertainty and repayment-conditioned WAL
   and remove `credit_duration`. The Rust registry, Python, WASM and
   stored results use the same owned values and semantics.
   [Accepted metric orchestration](../valuations-framework/metric-orchestration.md#accepted-decision-for-checked-metric-orchestration-and-reporting).

## Independent calculation checks

- Opening note par 100 and arrears 2, draw 20, redemption cash 12
  retiring 10, credit write-down 4 and funded PIK 1 leave par 107.
  Interest payment 6 comprises 5 current and 1 carried. A new
  deferral of 2 leaves arrears 3. Net holder settlement cash is −2.
- Retiring 40 at year one and 60 at year three gives WAL 2.2 years,
  including when cash at the second redemption is 66 with premium.
- Equity investment 100 followed by cash +120, −60 and +90 gives
  cash-on-cash 1.20, −0.60 and 0.90 and signed-net MOIC 1.50.
  Discount factors 0.95, 0.90 and 0.85 give native NAV 136.50.
  XIRR is unavailable under the canonical sign-change gate.
- Gross charge-offs 50 and qualified recoveries 15 on original
  par 1,000 give 5% gross and 3.5% net. Current par 400 would
  give 12.5% gross. Eligible carrying value 430 and tested debt 350
  give 18.60465% enhancement. Performing NOI 15 and 5 over regular
  debt service 10 and 8 gives DSCR 1.111111. The second asset's
  known zero NOI instead gives 0.833333.

## Observed implementation and scope

The reporting layer distinguishes paid cash, principal, PIK, deferred interest and writedown. That classification drives valuation and risk metrics; totals must not conflate noncash changes with receipts. Diagnostics expose how the waterfall conserved cash and changed credit support.

## Algorithm and conventions

Simulation records payment-date cashflows and separately tagged principal/interest/notional funding. PIK/deferred/writedown events are noncash classifications. `TrancheCashflows::to_table` validates currency, groups each series by date and sums six output columns. Metric seeding reuses the deterministic run's tagged flows and accrual periods to avoid a second simulation; seed failures can leave optional context absent, so downstream calculation must still validate.

WAL uses principal repayment weights and Act/365F time from the measurement date; cash interest does not count. Tagged-flow fallback recognizes principal kinds and can use absolute negative funding when principal labels are missing, making draw treatment material. Stochastic WAL additionally excludes no-principal paths from its average (see stochastic record). Equity summary uses current face × clean-price factor as invested capital, measures cash multiple/payback and Act/365F XIRR of future equity cash; unsuccessful XIRR becomes `None`, not a zero return.

Pool/deal ratios differ by denominator. ABS credit enhancement adds performing collateral, collection/reserve/excess-spread accounts and initial default recovery claims, then compares the most senior class with that collateral. ABS cumulative charge-off divides cumulative defaults by current pool balance. CPR/CDR metrics annualize the model's monthly rate at pool seasoning, not realized trailing collections. WARF/WAS and WAM use their pool-statistic definitions. CMBS DSCR uses only performing assets with supplied NOI.

The main Monte Carlo summary reports expected loss, population unexpected loss, 95% tail loss, mean PV and sampling error; its clean/dirty price identity and PV-weighted “credit duration” are specific conventions. Finite-value checks validate summaries, OAS and scenario cells; they do not certify statistical estimates or accounting completeness.

## Limitations and alternatives

Opening/static ratios are not projected covenant outcomes. Missing NOI, missing ratings, empty statistics and failed IRR have different handling, requiring meaningful null/zero presentation. Detail/report models do not add new empirical credit information. Alternative original-face denominators, total-return exposure weighting or realized trailing rates must be separately named and implemented rather than inferred from these outputs.

## Implementation and validation evidence

- [structured_credit/pricing/simulation_engine/simulate_period/record.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/simulate_period/record.rs)
- [structured_credit/pricing/simulation_engine/state.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/state.rs)
- [structured_credit/types/results.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/results.rs)
- [structured_credit/types/instrument.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/instrument.rs)
- [structured_credit/types/tranche_view.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/tranche_view.rs)
- [structured_credit/metrics/pricing/wal.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/pricing/wal.rs)
- [structured_credit/metrics/summary.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/summary.rs)
- [structured_credit/metrics/finite.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/finite.rs)
- [structured_credit/metrics/pool.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/pool.rs)
- [structured_credit/metrics/deal_specific/abs.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/deal_specific/abs.rs)
- [structured_credit/metrics/deal_specific/cmbs.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/deal_specific/cmbs.rs)

Existing evidence: [structured_credit/unit/diagnostics_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/diagnostics_tests.rs); [structured_credit/unit/metrics/pool_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/metrics/pool_tests.rs); [structured_credit/unit/metrics/deal_specific_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/metrics/deal_specific_tests.rs); [structured_credit/academic_golden.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/academic_golden.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

The diagnostic and hand-formula tests support accounting identities; they are not an independent validation of every economic metric interpretation.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
