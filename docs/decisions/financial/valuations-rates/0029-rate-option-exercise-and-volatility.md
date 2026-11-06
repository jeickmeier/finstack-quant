# ADR 0029: Rate-option selection by quotation and exercise

**Decision status:** Draft — observed implementation; adoption and rationale
pending agreement.

**Implementation status:** Observed for vanilla cap/floor and swaption
dispatch, including registered Hull-White and Bermudan alternatives.

**Validation status:** Evidence located, not rerun; independent reference and
convergence coverage remain to be assessed.

**Audit date:** 2026-11-05

**Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`

## Decision awaiting adoption

The proposed rate-option selection boundary preserves declared volatility
quotation and contractual exercise within explicitly scoped engine domains and
acceptance criteria. A registry name alone does not always identify the formula:
cap/floor `Black76` dispatches internally by quotation convention. This record
does not adopt every factor model, calibration objective, settlement annuity or
numerical default behind the selected route.

Scope is European cash-settled caps/floors, European payer/receiver swaptions
with supported settlement methods, and scheduled-exercise Bermudan swaptions.
CMS, inflation options and futures options retain separate economics. Detailed
owners below define exact admissibility, lifecycle and failure branches.

## Observed selection boundary

The [rates family](../../../../finstack-quant/valuations/rates/src/family.rs)
and instrument-owned entry points expose these routes:

| Product | Native/default selection | Explicit alternatives |
| --- | --- | --- |
| Cap/floor | Native optionlet sum; registry `Black76`. `Auto` follows source convention metadata; explicit Normal/Lognormal/ShiftedLognormal preserve quote units. | `HullWhite1F`; no separate registered `Normal` cap/floor key. |
| European swaption | `vol_model=Black` selects `Black76`; `Normal` selects `Normal`, including native `value()`. A mismatched explicit analytic key errors. | `HullWhite1F` tree. |
| Bermudan swaption | Registry default `HullWhite1F` tree. Direct `value()`/`value_raw()` error and require a pricer. | `MonteCarloHullWhite1F` or `LmmMonteCarlo`; no analytic European substitute. |

Selecting a model does not convert an incompatible volatility quote or fit
missing model parameters during valuation. The product records and shared
input resolver own quote precedence, scalar/schedule restrictions and supported
negative-rate domains.

## Canonical methodology owners

| Choice | Authoritative detailed record |
| --- | --- |
| Cap/floor schedules, fixing/premium policy, quote dispatch and HW scalar/schedule approximation | [Cap/floor pricing](05-cap-floor-pricing.md) |
| European forwards, volatility hierarchy, settlement annuities and quote Greeks | [European swaptions](07-european-swaptions.md) |
| European/Bermudan HW cashflows, tree reuse, exercise probabilities and HW LSMC | [Hull–White swaptions](08-hull-white-swaptions.md) |
| LMM future-exercise contract gate, loading/displacement, numeraire and stopping policy | [LMM Bermudans](09-lmm-bermudans.md) |
| Quote units, source metadata and fitted-parameter resolution | [Shared model inputs](../valuations-framework/model-resolution.md) |

Formula owners are [Black/normal/shifted options](../models-analytic/forward-normal-shifted.md),
[HW analytic kernels](../models-analytic/hull-white-analytic-kernels.md) and
[SABR fitting](../models-analytic/sabr-calibration.md). The
[HW numerical tree](../models-numerical/hull-white-event-grid-and-affine-bonds.md),
[LMM dynamics](../models-monte-carlo/10-lmm.md) and
[regression solver/basis policies](../models-monte-carlo/24-regression-policies.md)
own shared numerical components. HW and LMM product stopping rules remain
separate; the generic GBM LSMC implementation is not their algorithm owner.
[Rate-model calibration](../calibration-rate-models/README.md) records upstream
fitting choices without adopting the pricing adapters or their accuracy budgets.

## Rationale and material selection limits

Source-documented rationale is to retain quote units in the shared volatility
resolver and let European/Bermudan swaptions share the HW exercise model. The
proposed adoption rationale is to make quotation and exercise explicit and
reject unsupported combinations. Actual product gates and exceptions must be
assessed before adopting that as a uniform contract.

HW swaption paths are single-curve. Future-exercise LMM simulation has the
narrower physical/co-terminal/matching-schedule gate in its detailed owner;
its exercise-today-only branch can return before that full gate. HW tree and
HW LSMC also differ in their NonCoTerminal handling. These are unresolved
observed differences, not accepted equivalent contract support.

Analytic pricing avoids exercise-grid and regression error but does not solve
Bermudan stopping. HW trees discretize one-factor dynamics; LSMC adds sampling
and fitted-policy uncertainty; LMM introduces different forward dynamics and
prescribed loading/displacement choices. Sparse-ITM exercise behavior differs
between HW and LMM, including the LMM realized-future-payoff comparison. Their
detailed records preserve those branches and the limits of Monte Carlo error
bars. Registration does not establish interchangeable results.

## Evidence limits

Parameter precedence, grid/sample/basis defaults, prepared-model reuse,
exercise-today and risk interpretations are observed implementation choices,
not certified accuracy thresholds. Detailed evidence identifies a cap/floor
Auto prose discrepancy, local-production-formula or broad-range tests labelled
QuantLib parity, and slow/ignored Bermudan acceptance cases. Their names or
existence do not prove independent vendor agreement; no tests or external
benchmarks were executed.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
