# ADR 0027: Vanilla equity and FX option model selection

**Decision status:** Draft — observed implementation; adoption and rationale
pending agreement.

**Implementation status:** Observed for vanilla `EquityOption` and `FxOption`
entry points and registered alternatives; not a completeness assessment.

**Validation status:** Evidence located and inspected; not rerun in this audit.

**Audit date:** 2026-11-05

**Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`

## Decision awaiting adoption

The proposed decision covers the default model/exercise mapping for vanilla
equity and FX options, its supported input and lifecycle domains, and its
accuracy requirements.
Observed registration and source-documented motivation do not approve the
model, calibration provenance or numerical settings.

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
| FX formulation, expiry behavior, input units and delta conventions | [FX vanilla valuation](../valuations-fx-and-exotics/fx-vanilla-delta.md) |
| PDE domains, numerical grids and allowed exercise/dividends | [PDE adapters](09-equity-pde.md) |
| Explicit Heston inputs, Fourier and QE Monte Carlo settings | [Heston adapters](10-heston-adapters.md) |
| Rough-model inputs, variance strip, transform/simulation and diagnostics | [Rough-volatility adapters](11-rough-volatility-adapters.md) |
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

The proposed adoption rationale is to use closed forms for supported European
lognormal cases, a tree for contractual early exercise, and explicitly selected
richer models where appropriate. Closed forms avoid exercise-grid error; trees
and PDEs introduce discretization; stochastic-volatility methods add parameter,
integration or simulation uncertainty. No agreed comparison selecting LR and
its configured step count over alternatives was identified. These are reasons
to assess, not accepted rankings.

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
