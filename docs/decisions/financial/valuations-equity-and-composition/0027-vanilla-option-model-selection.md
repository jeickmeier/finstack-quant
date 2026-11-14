# ADR 0027: Vanilla equity and FX option model selection

**Decision status:** Equity lifecycle and baseline model selection accepted in
records 06 and 07. Remaining FX and alternative-model choices retain their
own record status.

**Implementation status:** Observed for vanilla `EquityOption` and `FxOption`
entry points and registered alternatives; not a completeness assessment.

**Validation status:** Evidence located and inspected; not rerun in this audit.

**Audit date:** 2026-11-05

**Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`

## Accepted equity baseline and lifecycle

The accepted [equity lifecycle](06-option-lifecycle.md#accepted-decision-for-checked-equity-option-exercise-and-settlement)
resolves authoritative exercise and settlement before live pricing. The
accepted [live equity baseline](07-black-tree-and-direct-greeks.md#accepted-decision-for-checked-live-equity-option-pricing-and-greeks)
uses `black_scholes` for European exercise and `tree` for American and
Bermudan exercise, with explicit Leisen–Reimer or CRR construction on the
tree route. The equity `black76` alias is retired in the accepted contract.
Product default selection is from the admitted exercise style, with required
controls and the qualified dividend, settlement and risk basis in the owners.
Runtime and host reconciliation are pending.

This selection record covers `EquityOption` and European `FxOption`. Futures
options, barriers, digitals, touch, quanto and path-dependent products have
separate economic and selection records. The detailed owners below define
inputs, formulas, defaults, failure branches and evidence.

## Observed selection boundary

- Both default registry keys are `Black76`. Equity dispatches to spot
  Black–Scholes–Merton for European exercise and Leisen–Reimer for
  American/Bermudan exercise. FX uses Garman–Kohlhagen with domestic
  discounting and foreign carry; the shared key does not imply the same
  formula or lifecycle.
- [Native `Instrument::value`][instrument] validates, resolves the effective
  date, calls `base_value`, then applies scenario adjustments. Supplying
  stochastic-volatility inputs does not itself select another engine.
  [Registry JSON pricing][json] resolves `"default"` through `default_model`;
  otherwise model selection is explicit.
- The [equity family][equity-family] unconditionally registers
  `PdeCrankNicolson1D`, `HestonFourier`, `PdeAdi2D`, `MonteCarloHeston`,
  `RoughHestonFourier`, `MonteCarloRoughHeston` and `MonteCarloRoughBergomi`.
  Registration is not gated by a Monte Carlo feature. Their live exercise,
  dividend and parameter restrictions are owned by the adapter records below;
  failure does not select another engine. CRR exists in the shared tree
  library but is not an equity-option selector here.
- The [FX family][fx-family] registers only `Black76` for `FxOption`. Its type
  has no American/Bermudan selector; other FX product registrations do not
  make their models available to vanilla FX options.

## Canonical methodology owners

| Choice | Authoritative detailed record |
| --- | --- |
| Equity observed exercise, expiry, cash/physical delivery and lifecycle Greeks | [Exercise and settlement lifecycle](06-option-lifecycle.md) |
| Equity baseline, dividend inputs, LR caller settings and direct Greeks | [Black/tree pricing](07-black-tree-and-direct-greeks.md) |
| Equity selected-model sensitivity bases and explicitly elected premium inversion | [Selected-model risk and implied volatility](08-selected-risk-and-implied-vol.md#accepted-decision-for-checked-selected-model-equity-risk-and-implied-volatility) |
| FX formulation, expiry behavior, input units and delta conventions | [FX vanilla valuation](../valuations-fx-and-exotics/fx-vanilla-delta.md) |
| Equity PDE model keys, domains, dated payoffs, exercise and risk | [Accepted PDE adapters](09-equity-pde.md#accepted-decision-for-checked-equity-blackscholes-and-heston-pde-pricing) |
| Equity Heston input block, dated payoff, bounded Fourier and explicit QE-M run | [Accepted Heston adapters](10-heston-adapters.md#accepted-decision-for-checked-dated-heston-fourier-and-monte-carlo-pricing) |
| Rough-model inputs, variance strip, transform/simulation and diagnostics | [Accepted rough-volatility adapters](11-rough-volatility-adapters.md#accepted-decision-for-checked-equity-rough-volatility-adapters) |
| Shared quote and parameter resolution | [Model-input resolution](../valuations-framework/model-resolution.md) |

The shared [spot formulas](../models-analytic/spot-vanilla-and-greeks.md),
[LR construction](../models-numerical/equity-binomial-construction.md) and
[escrow-reserve exercise](../models-numerical/dividends-and-lattice-exercise.md)
own their kernels. Adapter defaults and lifecycle restrictions do not become
global kernel defaults. The [analytic](../models-analytic/README.md),
[numerical](../models-numerical/README.md) and
[Monte Carlo](../models-monte-carlo/README.md) catalogues retain other delegated
methods and their distinct controls.

## Rationale and alternatives

Source-documented motivation is to preserve a recombining equity lattice while
restoring remaining dividends at exercise, and to express FX pricing in a
Garman–Kohlhagen form equivalent to Black on a covered-interest-parity forward.
These mechanics and their evidence are retained in the detailed owners.

The equity package selects closed form for supported European lognormal
cases and a caller-configured tree for supported early exercise. Closed
forms avoid exercise-grid error. Tree, PDE and stochastic-volatility routes
have their own numerical and evidence requirements. Qualify each exercise
grid and keep any preference between the admitted tree constructions on its
supported price and sensitivity evidence.

## Adoption and evidence boundary

Equity requires observed exercise/expiry state and values the resulting claim;
FX uses expiry intrinsic and zero after expiry. The linked lifecycle records
preserve this difference; this ADR does not harmonize or endorse it.

The [calibration-target catalogue](../calibration-targets/README.md) records
existing surface-calibration methods separately. Selecting Heston or a rough
model does not calibrate its parameters, establish market fit or show that
another calibration record covers them. Detailed evidence distinguishes
shared-kernel comparisons, ignored slow cases and vendor-labelled fixtures
whose generation was not requalified.

## Follow-up tracking

Pending adoption and numerical qualification are tracked in the
[equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition),
with product-specific FX work in the
[FX follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).

[instrument]: ../../../../finstack-quant/valuations/core/src/instruments/common_impl/traits/instrument.rs
[json]: ../../../../finstack-quant/valuations/src/pricer/json.rs
[equity-family]: ../../../../finstack-quant/valuations/equity/src/family.rs
[fx-family]: ../../../../finstack-quant/valuations/fx/src/family.rs
