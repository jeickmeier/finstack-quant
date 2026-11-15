# Structured-credit implementation methodologies

**Decision status:** All 22 detailed records are **Accepted**. Implementation and validation retain the statuses in each record.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

The slice inspects all **100 assigned Rust source files** in
`finstack-quant/valuations/structured-credit/src`: **90 algorithm-bearing,
9 support-only, 1 test-only, 0 generated**.
The [coverage ledger](coverage.json) lists every file exactly once and maps every
algorithm-bearing file to a substantive record. Source tests embedded within
runtime files do not make those files test-only; runtime after test items was
included. Classification includes economically material defaults, validation,
configuration and dispatch, not only functions named “price.”

The **22 records** cover collateral normalization and projection, servicing,
funding, waterfall/coverage logic, losses/exercise, stochastic engines, CMO/PAC,
facilities, inverses and risk/reporting. They include the material
[embedded assumption data](../../../../finstack-quant/valuations/structured-credit/data/assumptions/structured_credit_assumptions.v1.json).
Imported algorithms (cashflow model specifications, contractual bond/loan/IRS
schedules, CIR/OU kernels, core solvers and generic curve risk) belong to the
corresponding sibling inventories; these records cover their effective selection,
inputs and financial effects here.

**Validation status:** runtime implementations and selected existing formula,
regression and invariant assertions inspected; tests/builds were not executed.
The original audit checked links and source coverage separately. The accepted
records document independent calculations and pending runtime qualification.

| Calculation / supported instruments | Implemented method / material variants | Actual selection | Decision record | Implementation / validation |
|---|---|---|---|---|
| cash securitization deal resolution, assumptions and dispatch | One deterministic projected waterfall or explicitly selected pathwise waterfalls | Authoritative collateral, explicit active inputs, opted-in profiles and contractual schedule | [VALUATIONS-STRUCTURED-CREDIT-01](01-dispatch-and-assumptions.md#accepted-decision-for-structured-credit-dispatch-and-assumptions) | Accepted. Independent allocation and assumption checks. Runtime qualification pending |
| collateral normalization, pool statistics and structural bands | One dated pool snapshot, qualified cut-off par bridge, explicit statistic populations and effective obligor count | Sourced WARF profile, original-face bands and explicit loss order | [VALUATIONS-STRUCTURED-CREDIT-02](02-pool-normalization-and-statistics.md#accepted-decision-for-collateral-normalization-and-pool-statistics) | Accepted. Independent balance and weighting checks. Runtime qualification pending |
| statistical collateral cashflow projection | Actual monthly survival or dated target amounts, ordered cash, qualified installments and floating terms | Default-vintage recoveries and shared deterministic, stochastic and OAS cash rules | [VALUATIONS-STRUCTURED-CREDIT-03](03-statistical-collateral-projection.md#accepted-decision-for-statistical-collateral-projection) | Accepted. Independent stub, targets and cash checks. Runtime qualification pending |
| statistical delinquency migration and servicing | Scheduled monthly entry and opening-state transitions, separate modified cohorts and arrears | Explicit advance cap and authorized reimbursement with retained residuals | [VALUATIONS-STRUCTURED-CREDIT-04](04-delinquency-and-advancing.md#accepted-decision-for-delinquency-migration-and-advancing) | Accepted. Independent state and cash checks. Runtime qualification pending |
| statistical NPL resolution and reperformance | Dated deterministic split, checked retired and resumed par, actual receipt with qualified costs | Authorized workout state, separate resumption terms and deterministic dispatch | [VALUATIONS-STRUCTURED-CREDIT-05](05-npl-resolution.md#accepted-decision-for-npl-resolution-and-reperformance) | Accepted. Independent dates, cash and nonlinear allocation checks. Runtime qualification pending |
| statistical commercial-mortgage collateral and DSCR | Deterministic maturity splits, dated workout cash, qualified fee bases and prepayment protection | Explicit premium profiles and complete-population borrower debt service | [VALUATIONS-STRUCTURED-CREDIT-06](06-cmbs-servicing.md#accepted-decision-for-cmbs-balloon-and-servicing) | Accepted. Independent cash, premium and ratio checks. Runtime qualification pending |
| card master-trust collections, investor and seller allocation and metrics | Dated monthly card flows, frozen principal numerator and live finance and default shares | Authorized replenishment, controlled accumulation, dated triggers and labeled ratios | [VALUATIONS-STRUCTURED-CREDIT-07](07-card-master-trust.md#accepted-decision-for-card-master-trust-allocation) | Accepted. Independent cash and metric checks. Runtime qualification pending |
| contractual bond, term-loan and revolving-credit collateral | Dated ledger, exact collection cuts, active credit authority and due-phase liquidity | Qualified terminal claims, separated draw types and active dependency closure | [VALUATIONS-STRUCTURED-CREDIT-08](08-instrument-collateral.md#accepted-decision-for-instrument-collateral-and-liquidity) | Accepted. Independent dates, credit and funding checks. Runtime qualification pending |
| collateral call and put exercise | Exact rights and chronology, actual dated payoff and complete workout yield | Explicit policy per side and qualified incentive inputs | [VALUATIONS-STRUCTURED-CREDIT-09](09-collateral-exercise.md#accepted-decision-for-collateral-call-and-put-exercise) | Accepted. Independent dates, payoff and yield checks. Runtime qualification pending |
| collateral purchases, reserve and funding accounts, hedge cash | Dated eligible purchase cash and explicit cohorts, authorized account movements and hedge ranks | Separate cash and acquired par, exact ledger and qualified releases and netting | [VALUATIONS-STRUCTURED-CREDIT-10](10-reinvestment-reserves-and-hedges.md#accepted-decision-for-reinvestment-reserves-and-hedge-cash) | Accepted. Independent cash and timing checks. Runtime qualification pending |
| standard and custom waterfalls | Dated claims, explicit priority and funding, checked capped allocation | Executable payment units, separate PIK and carryover, dated equity hurdle | [VALUATIONS-STRUCTURED-CREDIT-11](11-waterfall-claims-and-rounding.md#accepted-decision-for-waterfall-claims-and-rounding) | Accepted. Independent claim and allocation checks. Runtime qualification pending |
| OC, IC and borrowing-base coverage | Dated eligible values and claims, explicit breach state and action tiers | Checked executable cures on authorized funding and allocation paths | [VALUATIONS-STRUCTURED-CREDIT-12](12-coverage-and-cures.md#accepted-decision-for-coverage-tests-and-cure-diversion) | Accepted. Independent ratio and cure checks. Runtime qualification pending |
| rule-driven principal allocation and early amortization | Qualified tests, separate principal streams, target OC and controlled accumulation | Explicit rule composition and calendar-month permanent triggers | [VALUATIONS-STRUCTURED-CREDIT-13](13-principal-regimes-and-amortization.md#accepted-decision-for-principal-regimes-and-early-amortization) | Accepted. Independent allocation and trigger checks. Runtime qualification pending |
| note balances, cleanup/forced calls and legal-final resolution | incremental principal effects, eligible settlement funding and dated terminal entitlements | independent loss policies and explicit call, recovery, account and discharge authority | [VALUATIONS-STRUCTURED-CREDIT-14](14-loss-recognition-and-terminal-cash.md#accepted-decision-for-loss-recognition-and-terminal-cash) | Accepted. Independent loss and cash checks. Runtime qualification pending |
| stochastic statistical/instrument pools and full waterfall scenarios | contractual pathwise waterfalls, checked factor and credit laws, independent PV estimators and empirical trajectory risk | explicit route, run count, pairing, seed, dependence roles and applicable granularity | [VALUATIONS-STRUCTURED-CREDIT-15](15-stochastic-waterfall.md#accepted-decision-for-stochastic-waterfall-pricing-and-estimation) | Accepted. Independent loss, sampling and risk arithmetic. Runtime qualification pending |
| revolving instruments inside a structured-credit pool | checked spread and utilization, due-phase funding and matched outstanding draw-spread replay | explicit laws, funding terms and qualified attribution request | [VALUATIONS-STRUCTURED-CREDIT-16](16-revolver-utilization-and-option-cost.md#accepted-decision-for-revolver-utilization-and-draw-spread-attribution) | Accepted. Independent utilization, funding and attribution checks. Runtime qualification pending |
| tranche clean-price inversion with optional rate and credit shocks | fitted dated Hull–White, qualified intensity responses, buyer settlement and checked continuous spread inverse | explicit scenario and seed controls, entitlement, qualified bracket and native-currency residual | [VALUATIONS-STRUCTURED-CREDIT-17](17-oas-scenarios.md#accepted-decision-for-structured-credit-oas-and-settlement-spread-fitting) | Accepted. Independent intensity, settlement and inverse checks. Runtime qualification pending |
| sequential, PAC/support, IO, PO and accrual CMO reference tranches | common qualified MBS cash ledger, authored priorities and immutable dated PAC ending balances | explicit agency collateral, class and settlement terms, PAC support and fixed-strip quote | [VALUATIONS-STRUCTURED-CREDIT-18](18-agency-cmo-and-pac.md#accepted-decision-for-agency-cmo-waterfalls-and-dated-pac-contracts) | Accepted. Independent PAC, accrual and quote checks. Runtime qualification pending |
| eligible collateral, concentration limits, facility advances and lender cashflows | dated base assessment, qualified draw and cure phases, actual fees and signed lender ledger | explicit facility terms, ordered eligible limits, dated state and quote compounding | [VALUATIONS-STRUCTURED-CREDIT-19](19-borrowing-base-facility.md#accepted-decision-for-borrowing-base-facilities-and-lender-cashflows) | Accepted. Independent concentration, cash and return checks. Runtime qualification pending |
| deal/tranche price metrics and inverse fixed-cashflow solvers | signed buyer ledger, original-parent accrued, nominal periodic yield and continuous curve spreads | explicit price, denomination, entitlement, projection and checked solver controls | [VALUATIONS-STRUCTURED-CREDIT-20](20-quotes-and-inverse-pricing.md#accepted-decision-for-structured-credit-settlement-quotes-and-inversion) | Accepted. Independent entitlement and quote checks. Runtime qualification pending |
| rate/spread/credit sensitivities and deterministic scenario summaries | identified quote and live risk, typed loss threshold crossing and ordered grid | qualified active sources, expressed moves, loss threshold and affected exposures | [VALUATIONS-STRUCTURED-CREDIT-21](21-risk-bumps-and-scenarios.md#accepted-decision-for-structured-credit-risk-and-scenarios) | Accepted. Independent risk and stress checks. Runtime qualification pending |
| tranche/deal reporting, equity returns, WAL and pool/deal ratios | signed event ledger, exact diagnostics, retired-par WAL and signed-net equity returns | qualified claim, cash and investment cut, denominators and source coverage | [VALUATIONS-STRUCTURED-CREDIT-22](22-diagnostics-and-reporting.md#accepted-decision-for-structured-credit-diagnostics-and-reporting) | Accepted. Independent ledger, return and ratio checks. Runtime qualification pending |

## Adoption boundaries

- [Accepted dispatch and assumptions](01-dispatch-and-assumptions.md#accepted-decision-for-structured-credit-dispatch-and-assumptions) bind the selected route, authoritative inputs and explicit profile provenance.
- [Accepted stochastic waterfall](15-stochastic-waterfall.md#accepted-decision-for-stochastic-waterfall-pricing-and-estimation) uses authorized dependence roles and deterministic relative discounting. [Accepted OAS scenarios](17-oas-scenarios.md#accepted-decision-for-structured-credit-oas-and-settlement-spread-fitting) fit the valuation-origin buyer claim on the qualified settlement basis.
- [Accepted principal regimes](13-principal-regimes-and-amortization.md#accepted-decision-for-principal-regimes-and-early-amortization) cover qualified allocations and permanent triggers. [Accepted terminal cash](14-loss-recognition-and-terminal-cash.md#accepted-decision-for-loss-recognition-and-terminal-cash) covers loss recognition, funded calls and authorized discharge. [Accepted borrowing base](19-borrowing-base-facility.md#accepted-decision-for-borrowing-base-facilities-and-lender-cashflows) governs live eligibility, draws, cures and lender cash. [Accepted quotes](20-quotes-and-inverse-pricing.md#accepted-decision-for-structured-credit-settlement-quotes-and-inversion) identify the buyer cash, delivered denomination and fixed-path quote law.
- Synthetic CDS tranches belong to [ADR 0031](../models-and-valuations-credit/0031-tranche-loss-model-selection.md) and the credit slice, not this cash-waterfall inventory.

## Month counts at structured-credit sites

[CORE-CONVENTIONS-007 #4](../core-conventions/007-tenor-date-arithmetic.md#accepted-decision-tenor-and-date-arithmetic-conventions)
redefined the core `months_until` as completed months from the original
anchor and made a reversed range an error. Per owner resolution O13, the
structured-credit sites below do not adopt that measure automatically. Each
site's meaning is stated here; every site currently uses the named site-local
`ClampedMonths::clamped_months_until` (`structured_credit/utils/months.rs`),
which keeps the pre-007 count: `(Y₂-Y₁)·12 + (M₂-M₁)`, minus one when the
start day exceeds the end day unless both are month-ends, and zero for a
reversed range. No accepted structured-credit record selects a measure for
any of these meanings, so none has been migrated.

| Site | Meaning | Measure needed (unsettled) |
| --- | --- | --- |
| `metrics/deal_specific/cmbs.rs` (age) | Loan seasoning: age of the loan at valuation | [Accepted regular service from the qualified loan schedule](06-cmbs-servicing.md#accepted-decision-for-cmbs-balloon-and-servicing) |
| `metrics/deal_specific/cmbs.rs` (remaining term) | Remaining months to loan maturity for IO/amortization tests | Contractual remaining payment count from the loan schedule |
| `metrics/risk/oas.rs` (horizon) | Length of the monthly OAS rate path (plus a 12-month buffer) | Simulation grid length (17) |
| `metrics/risk/oas.rs` (index) | Monthly OU-path index of a cashflow date | Simulation month index on the OAS grid (17) |
| `pricing/simulation_engine/orchestration.rs` | Pool seasoning at a redemption date (closing seasoning plus months since closing) | Seasoning convention used for liquidation value (14) |
| `pricing/simulation_engine/pool_flow_source.rs` (seasoning) | Per-asset seasoning at a payment date for prepayment/default ramps | Seasoning convention of the collateral projection (03) |
| `pricing/simulation_engine/pool_flow_source.rs` (index) | Rate-shift path index of a payment date | Simulation month index (15) |
| `pricing/simulation_engine/pool_flows.rs` (age) | Per-asset age at the period start for ramp curves | Seasoning convention of the collateral projection (03) |
| `pricing/simulation_engine/pool_flows.rs` (remaining term) | Remaining amortization months to asset maturity | Contractual remaining payment count (13) |
| `pricing/simulation_engine/simulate_period/mod.rs` | Deal seasoning at a payment date for ramp entry | Seasoning convention of the collateral projection (03) |
| `pricing/simulation_engine/simulate_period/period_waterfall.rs` | Months since closing for shifting-interest lock-out schedules | Contractual lock-out schedule definition (13) |
| `pricing/stochastic/pricer/engine.rs` (test) | Scenario-tree depth in months | Simulation grid length (15) |
| `types/instrument.rs` | Asset original term from origination to maturity | Contractual original term |
| `types/pool.rs` (bound) | Largest representable seasoning from the earliest supported date | A domain bound; either measure suffices |
| `types/pool.rs` (age) | Balance-weighted pool age at a date | [Accepted completed-month pool age](02-pool-normalization-and-statistics.md#accepted-decision-for-collateral-normalization-and-pool-statistics) |
| `types/pricing_methods.rs` | Scenario-tree depth from valuation to maturity | Simulation grid length (15) |

These are the open "state the meaning" items of the core-conventions follow-up
for record 007; each moves to its measure only when the owning record selects
it.

## Follow-up tracking

The [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit) own adoption
questions, implementation reconciliation and missing validation. Records retain
the observed algorithms, evidence and limits on their interpretation.
