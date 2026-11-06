# Structured-credit implementation methodologies

**Decision status:** Draft — observed implementation; no financial choice is adopted by this audit.

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
Links and exact source coverage were checked separately. No runtime source,
tests, fixtures or accepted financial choices changed. This is an implementation
inventory, not universal correctness certification or independent model validation.

| Calculation / supported instruments | Implemented method / material variants | Actual selection | Decision record | Implementation / validation |
|---|---|---|---|---|
| cash securitization constructors, validation and deterministic/stochastic entry points | validated normalized deal; deterministic waterfall by ordinary cashflow/PV route; explicit Monte Carlo conveniences | generic builder defaults and family constructor profiles differ; explicit stochastic API and specs | [VALUATIONS-STRUCTURED-CREDIT-01](01-dispatch-and-assumptions.md) | Observed; linked source and existing evidence, not executed |
| asset rows, representative lines, instrument collateral and concentration reporting | exclusive representation normalization; balance-weighted aggregates and inverse obligor HHI | representation supplied by caller; inferred original balance when omitted | [VALUATIONS-STRUCTURED-CREDIT-02](02-pool-normalization-and-statistics.md) | Observed; linked source and existing evidence, not executed |
| performing asset and representative-line pools, amortizing and bullet collateral | monthly survival aggregation; default-first expected flows; level-pay amortization; lagged recoveries | deal model with asset overrides; asset type selects amortizing versus bullet behavior | [VALUATIONS-STRUCTURED-CREDIT-03](03-statistical-collateral-projection.md) | Observed; linked source and existing evidence, not executed |
| statistical pools with delinquency buckets | monthly expected-balance transition recursion with cures, rolls, modifications and arrears | optional DelinquencyModel; incompatible with instrument collateral | [VALUATIONS-STRUCTURED-CREDIT-04](04-delinquency-and-advancing.md) | Observed; linked source and existing evidence, not executed |
| statistical assets carrying explicit liquidation terms | expected split between reperformance and liquidation at a configured resolution date | optional per-asset LiquidationSpec; mutually exclusive with an initial default claim | [VALUATIONS-STRUCTURED-CREDIT-05](05-npl-resolution.md) | Observed; linked source and existing evidence, not executed |
| statistical commercial-mortgage collateral and DSCR metrics | expected maturity split; delayed workouts; coupon-loss penalties; loan-level debt service | optional balloon, prepayment penalty, servicing and NOI inputs | [VALUATIONS-STRUCTURED-CREDIT-06](06-cmbs-servicing.md) | Observed; linked source and existing evidence, not executed |
| statistical credit-card pools with CardPortfolioSpec | portfolio-yield finance charges, charge-offs and monthly payments; revolving versus fixed investor allocation | explicit credit_model.card, with early amortization and controlled accumulation as separate rules | [VALUATIONS-STRUCTURED-CREDIT-07](07-card-master-trust.md) | Observed; linked source and existing evidence, not executed |
| bond, term-loan and revolving-credit collateral schedules | contractual schedule bucketing with survival overlays and liquidity-capped default draws | pool.instruments selects this source; no collateral reinvestment | [VALUATIONS-STRUCTURED-CREDIT-08](08-instrument-collateral.md) | Observed; linked source and existing evidence, not executed |
| callable/putable bond and loan collateral | contractual-only, first opportunity, yield-to-worst and refinance/step-up rules | explicit collateral exercise policy and optional per-instrument overrides | [VALUATIONS-STRUCTURED-CREDIT-09](09-collateral-exercise.md) | Observed; linked source and existing evidence, not executed |
| period cash accounts, collateral purchases and contractual hedge overlays | ordered cash allocation; par purchases from price; targeted reserve replenishment; interval hedge netting | explicit reinvestment/reserve/hedge terms; defaults disable the optional features | [VALUATIONS-STRUCTURED-CREDIT-10](10-reinvestment-reserves-and-hedges.md) | Observed; linked source and existing evidence, not executed |
| standard and custom waterfalls | priority-ordered separate cash accounts; capped sequential/pro-rata allocation; largest-remainder currency rounding | standard sequential template or explicit custom tiers; per-tier funding and allocation overrides | [VALUATIONS-STRUCTURED-CREDIT-11](11-waterfall-claims-and-rounding.md) | Observed; linked source and existing evidence, not executed |
| OC, IC and borrowing-base tests inside the waterfall | haircut-adjusted collateral and senior-claim ratios; ordered cure sizing and diversion | tranche legacy triggers plus explicit CoverageTestSpec placement/action/cap | [VALUATIONS-STRUCTURED-CREDIT-12](12-coverage-and-cures.md) | Observed; linked source and existing evidence, not executed |
| rule-driven changes to principal allocation | date/threshold predicates, current-balance weights, unscheduled-principal shifting and target OC caps | optional WaterfallRules composed each period | [VALUATIONS-STRUCTURED-CREDIT-13](13-principal-regimes-and-amortization.md) | Observed; linked source and existing evidence, not executed |
| note balances, cleanup/forced calls and legal-final resolution | deal-family loss-policy defaults; junior-first capped writedown; conditional cleanup and terminal claim acceleration | explicit overrides or family defaults; call assumptions and cleanup threshold | [VALUATIONS-STRUCTURED-CREDIT-14](14-loss-recognition-and-terminal-cash.md) | Observed; linked source and existing evidence, not executed |
| stochastic statistical/instrument pools and full waterfall scenarios | Philox streams; stationary monthly AR(1) factors; per-name or conditional pool defaults; pathwise waterfalls | ModelKey::StructuredCreditStochastic or explicit stochastic convenience; default 5,000 independent estimators with antithetic pairs | [VALUATIONS-STRUCTURED-CREDIT-15](15-stochastic-waterfall.md) | Observed; linked source and existing evidence, not executed |
| revolving instruments inside a structured-credit pool | CIR-QE spread plus clipped exact-OU utilization; same-shock counterfactual cashflow replay | collateral utilization/spread parameters; only qualifying stochastic revolvers produce draw-option costs | [VALUATIONS-STRUCTURED-CREDIT-16](16-revolver-utilization-and-option-cost.md) | Observed; linked source and existing evidence, not executed |
| tranche clean-price inversion with optional rate and credit shocks | monthly OU/Hull-White rate path plus separate lognormal credit modulation; cached-path Brent root | explicit tranche_oas and OasConfig; defaults 256 paths, credit on, rates off, seed 42 | [VALUATIONS-STRUCTURED-CREDIT-17](17-oas-scenarios.md) | Observed; linked source and existing evidence, not executed |
| sequential, PAC/support, IO, PO and accrual CMO reference tranches | shared MBS collateral projection; priority cash waterfall; persistent dated PAC minimum-collar targets | explicit collateral or assumed-collateral fallback; tranche type and same-priority allocation setting | [VALUATIONS-STRUCTURED-CREDIT-18](18-agency-cmo-and-pac.md) | Observed; linked source and existing evidence, not executed |
| eligible collateral, concentration limits, facility advances and lender cashflows | eligibility/advance-rate weighting and sequential concentration reductions; synthetic senior-plus-residual deal | explicit borrowing-base rules and facility terms | [VALUATIONS-STRUCTURED-CREDIT-19](19-borrowing-base-facility.md) | Observed; linked source and existing evidence, not executed |
| deal/tranche price metrics and inverse fixed-cashflow solvers | current-face clean/dirty settlement basis; XIRR/yield helpers; exponential-spread Brent inversion | explicit settlement or valuation date; metric overrides and standalone helpers have distinct solver units/bounds | [VALUATIONS-STRUCTURED-CREDIT-20](20-quotes-and-inverse-pricing.md) | Observed; linked source and existing evidence, not executed |
| rate/spread/credit sensitivities and deterministic scenario summaries | mixed analytical/fixed-flow and full-reprojection finite differences; bisection and grid enumeration | metric registry or standalone tranche summary; configuration determines different risk meanings | [VALUATIONS-STRUCTURED-CREDIT-21](21-risk-bumps-and-scenarios.md) | Observed; linked source and existing evidence, not executed |
| tranche/deal reporting, equity returns, WAL and pool/deal ratios | tagged cash/noncash accounting; principal-weighted WAL; current-capital return metrics | shared simulation results plus metric registry and advanced summary helpers | [VALUATIONS-STRUCTURED-CREDIT-22](22-diagnostics-and-reporting.md) | Observed; linked source and existing evidence, not executed |

## Adoption boundaries

- [Model dispatch/presets](01-dispatch-and-assumptions.md) distinguish observed defaults from an approved calibration.
- [Main Monte Carlo](15-stochastic-waterfall.md) and [OAS scenarios](17-oas-scenarios.md) are separate models; the main pricer does not consume every named preset correlation dimension.
- [Terminal cash](14-loss-recognition-and-terminal-cash.md), [principal regimes](13-principal-regimes-and-amortization.md), [borrowing base](19-borrowing-base-facility.md) and [quotes](20-quotes-and-inverse-pricing.md) describe observed contracts; they do not establish legal or market approval.
- Synthetic CDS tranches belong to [ADR 0031](../models-and-valuations-credit/0031-tranche-loss-model-selection.md) and the credit slice, not this cash-waterfall inventory.

## Follow-up tracking

The [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit) own adoption
questions, implementation reconciliation and missing validation. Records retain
the observed algorithms, evidence and limits on their interpretation.
