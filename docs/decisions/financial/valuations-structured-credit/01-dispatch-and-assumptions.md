# VALUATIONS-STRUCTURED-CREDIT-01: Deal resolution, dispatch and assumption presets

- **Decision status:** Accepted 2026-11-15 — complete deal resolution, route selection, assumption authority, scheduling and request package.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent deterministic-versus-stochastic allocation, source-precedence, assumption catalogue and accrual calculations checked. Production pricing, risk and host qualification remain pending.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Accepted decision for structured-credit dispatch and assumptions

1. **Select the deterministic and stochastic routes explicitly.** Ordinary
   `Discounting` runs one contractual waterfall on projected collateral
   cashflows. An explicit `StructuredCreditStochastic` request runs the
   waterfall on each path and averages the resulting values. Direct and
   registry pricing resolve to the same Rust request. The presence of
   stochastic configuration does not switch ordinary dispatch. OAS
   inversion and synthetic CDS tranche pricing keep their own routes.
   Cashflow and metric results identify their calculation basis. Return
   explicit unavailability or an error for a metric unsupported on the
   selected route, with no deterministic substitute for a stochastic result.
   [Accepted OAS scenarios](17-oas-scenarios.md#accepted-decision-for-structured-credit-oas-and-settlement-spread-fitting),
   [Synthetic CDS tranche selection](../models-and-valuations-credit/0031-tranche-loss-model-selection.md).

2. **Resolve one authoritative collateral source.** Admit authored asset
   rows, representative lines or typed instruments. Reject competing inputs
   even when their identities agree. Internally derived rows can support
   reporting while typed instruments retain projection authority. Validate
   collateral-route compatibility, currencies and applicable opening state
   before making market requests. Apply qualified asset-specific inputs
   before deal assumptions. A deal assumption shock affects only exposures
   that actually use it. An empty opening collateral pool does not establish
   that outstanding claims, reserves or commitments are exhausted.
   [Accepted pool normalization](02-pool-normalization-and-statistics.md#accepted-decision-for-collateral-normalization-and-pool-statistics),
   [Terminal cash](14-loss-recognition-and-terminal-cash.md#accepted-decision-for-loss-recognition-and-terminal-cash).

3. **Require explicit active credit assumptions.** Remove the generic 20%
   CPR, 2% CDR, 60% recovery, 12-month recovery lag and embedded 4% refinancing
   defaults. Select the active prepayment, default and recovery assumptions
   explicitly, including deliberate no-event choices. Require a qualified
   refinancing input only on routes consuming it. Richard–Roll needs its
   qualified pool coupon and refinancing input. Missing required inputs
   error, including unavailable pool coupon previously replaced by zero.

4. **Retain deliberate versioned starter profiles.** Keep the CLO, RMBS,
   CMBS and auto ABS profiles as explicit starter choices. Validate family
   and collateral compatibility and retain the frozen expanded values,
   profile identity and version or data provenance, and visible explicit
   overrides. The auto ABS speed profile requires qualifying collateral.
   Card, CBO and other supported deals can be configured explicitly.
   Fee profiles require independent selection with qualified currency and
   fee basis. Invalid selections or packaged data return an error.
   Selection supplies assumptions and performs no empirical calibration.
   [Embedded assumption catalogue](../../../../finstack-quant/valuations/structured-credit/data/assumptions/structured_credit_assumptions.v1.json),
   [Ginnie Mae PSA disclosure](https://www.ginniemae.gov/s3/sites/default/files/assets/pdfs/sf_base_offering_circular_July_2023.pdf).

5. **Bind the contractual schedule and separate its clocks.** Require
   `closing < first_payment <= legal_maturity` and an explicitly qualified
   grid, roll, stub, adjustment and calendar where applicable. Require an
   authorized stub for a shortened period. Keep actual payments, collateral
   and tranche day counts and discount-curve coordinates distinct.
   Use one explicit valuation and same-day entitlement cut across cashflow
   export, PV and metrics. Discount eligible dated cash using relative
   factors from the selected curve.
   [Accepted schedule conventions](../core-conventions/008-schedule-generation.md),
   [Accepted dated discounting](../core-conventions/016-discounting.md).

6. **Resolve the complete stochastic request before drawing.** Each active
   channel explicitly selects its stochastic law or carries forward its
   deterministic specification by election. Resolve one qualified parameter
   source per active dependence role. Reject conflicting inputs and
   correlation structures unsupported by the selected engine. Retire the
   family switch that silently replaces assumptions. Require explicit
   pricing group count, pairing and seed policy. Rust owns the effective
   model, assumptions, validation and actual-work reporting used by the
   existing Python and WASM routes. Detailed path and dependence laws
   belong to the stochastic waterfall record.
   [Accepted valuation request controls](../valuations-framework/builder-validation.md),
   [Stochastic waterfall](15-stochastic-waterfall.md#accepted-decision-for-stochastic-waterfall-pricing-and-estimation).

## Independent calculations

- One payment discounted at 0.98, equally likely collateral collections
  of 60 and 100, a senior claim capped at 70 and a residual claim give
  senior/residual PV of 68.60/9.80 from applying the waterfall once to
  mean collateral cash of 80. Applying the waterfall on each path gives
  mean senior/residual PV of 63.70/14.70. Total PV is 78.40 in both cases.
- For two names of 100 face each, a qualified asset-specific period PD
  and recovery of 10% and 40% produce loss 6 on the first name under both
  scenarios. Shocking the second name's deal PD/recovery from 2%/60% to
  6%/20% raises pool expected loss from 6.80 to 10.80. Only the second name
  uses the changed sources.
- Catalogue checks confirmed four starter constructor families and
  distinguished monthly auto ABS speed from annual CPR. The hardcoded
  CMBS stochastic switch uses 2% CPR and 2% CDR, while its stochastic
  registry row gives 3% CPR and 2.5% CDR. These are observed inputs from
  different paths.

## Observed implementation and scope

The implementation resolves a deal into a normalized collateral representation before simulating its waterfall. Ordinary `Instrument::base_value` and `run_simulation` use projected deterministic cashflows. Stochastic flags describe available specifications. Separate stochastic entry points build scenario configuration and execute Monte Carlo. The accepted contract above governs reconciliation.

## Scope and input domain

`StructuredCredit` covers CLO/CBO, RMBS, CMBS, ABS/auto/card terms. Standard constructors exist for a smaller explicit family table, while the builder supports the complete enum. Resolution requires closing < first payment ≤ legal maturity; recovery/prepayment/default specifications validate before pricing. Normalization, same-currency collateral, reserves, custom waterfalls, calls, draws and specialized collateral terms have further validations. A supplied calendar must resolve; schedule conventions differ by deal family.

## Algorithm, selection and controls

1. Generic `CreditModelConfig` deserialization/builder defaults come from the embedded assumption registry: constant CPR 20%, CDR 2%, recovery 60%, and a 12-month recovery lag.
2. `new_standard` chooses a registry profile by deal type, checks pool/family consistency and sets first payment one profile frequency after closing. Pricing validation checks it against legal maturity. Example profiles include monthly RMBS PSA 100%, CDR 0.6%, recovery 60%, lag 18 months; quarterly CLO CPR 20%, CDR 2%, recovery 60%, lag 12 months; monthly ABS speed 1.5%, CDR 2%, recovery 45%, lag 12 months. Fees remain absent until explicitly set or `with_standard_fees` is called.
3. `resolved_for_pricing` validates dates and credit model, normalizes the pool, and validates initial recovery claims and reinvestment criteria. Instrument lifecycle validation also checks ancillary rules and nested instruments before market access.
4. Aggregate schedule metadata uses the family day-count helper: RMBS/CMBS 30/360, other families Act/360. The common simulation grid uses Act/360, and actual collateral and tranche accrual use their respective inputs. Discounting uses relative factors from the chosen discount curve.
5. Stochastic convenience presets replace prepayment/default specifications and correlation. RMBS uses the Richard Roll specification; CLO/CBO use factor-correlated prepayment and Gaussian-copula defaults; CMBS uses deterministic CPR 2% and Gaussian defaults CDR 2%, correlation 20%; ABS/auto/card require a constant deterministic default model for the convenience preset.

## Limitations, failure and alternatives

The assumption registry validates shape/ranges, not empirical calibration. Its constructor, stochastic, CMO and fee profiles are different data sets; no common economic meaning should be inferred from the word “standard.” Embedded required-profile lookup can panic if packaged data are invalid. Explicit deal inputs and custom waterfall rules are supported alternatives. A model calibrated from observed collateral or market prices is a possible alternative but no such calibration fit is performed by these preset helpers. Generic builder versus standard-constructor behavior and missing-calendar behavior do not establish approved intended uses.

## Implementation and validation evidence

- [structured_credit/assumptions.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/assumptions.rs)
- [structured_credit/types/constructors.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/constructors.rs)
- [structured_credit/types/resolved.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/resolved.rs)
- [structured_credit/types/instrument.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/instrument.rs)
- [structured_credit/types/pricing_methods.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/pricing_methods.rs)
- [structured_credit/types/stochastic.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/stochastic.rs)
- [structured_credit/pricing/stochastic/calibrations.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/stochastic/calibrations.rs)

Existing evidence: [structured_credit/unit/constructors_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/constructors_tests.rs); [structured_credit/unit/validation_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/validation_tests.rs); [structured_credit/unit/stochastic_pricing_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/stochastic_pricing_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

No empirical calibration or approval was established. The embedded JSON data are material methodology dependencies even though the source manifest counts Rust files only.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
