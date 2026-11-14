# VALUATIONS-FRAMEWORK-11 — Native-model and common-random-number option Greeks, FX01 and surface buckets

- Decision status: **Accepted — complete checked option and FX sensitivities package, 2026-11-13**.
- Implementation status: **Observed, with reconciliation pending** across shared calculators, selected-model providers, reporting and hosts.
- Validation status: Source and primary references inspected. Independent bounded stencil, quote-scale, FX direction and bucket examples checked. Runtime, financial accuracy and host qualification remain pending.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked option and FX sensitivities

1. **Bind each Greek to the selected valuation and the exact experiment.**
   Use the prepared effective date and information phase, admitted lifecycle
   state, selected raw pricer, native currency and quantity, and actual
   factors, units and held-input policy. Register one authorized producer.
   Use a native analytic or pathwise provider only on the equivalent
   pricing and risk basis. A different selected model needs its own
   provider or an explicitly admitted reprice of that model. After a
   lifecycle transition, reuse a native provider only when the owner
   establishes equivalence for the resolved claim. A missing number or
   failed calculation never selects another provider. Preserve unsupported
   as unavailable and return structural zero only after the product proves
   the exposure absent. A fixed unpaid cash claim can still have rate or
   FX risk, and delivery can leave an underlying position. Require
   authoritative observations and preserve fixed history. Remove generic
   expiry-date zero returns.
   [Dispatch and lifecycle](pricing-lifecycle.md#accepted-decision-for-checked-valuation-dispatch-and-lifecycle),
   [metric ownership](metric-orchestration.md#accepted-decision-for-checked-metric-orchestration-and-reporting).

2. **Keep units, controls and scenario geometry explicit.** Resolve active
   calculation steps under [record 09](finite-difference-rates.md#accepted-decision-for-checked-finite-differences-and-rate-risk).
   Require a finite positive step for every bumped axis. Analytic results
   need no unused bump. The metric contract fixes its reporting increment,
   which can differ from the step. Use actual represented scalar nodes;
   for vector or transformed scenarios retain the common amplitude and
   verify each required move. Precheck the complete stencil, keep admitted
   reprices comparable, and use checked differences and scaled arithmetic
   through the final signed native PV result. Preserve native analytic
   partials in their own units and convert exactly once at the product
   boundary. Keep calculation errors and genuine numerical zero distinct.
   Remove absolute spot floors, volatility floors and implicit step
   shrinking.

3. **Take generic spot risk on one exact active positive quote.** Bind the
   quote's identity, representation, role, currency and units. Hold
   contractual strikes and reference levels, recognized observations and
   other admitted inputs fixed; apply only the declared responsive mappings.
   A forward or futures-price Greek retains its own factor; never select
   the first declared scalar or infer a joint basket move. Use finite
   relative fraction `0<b<1` to request `S0±S0×b` and verify actual
   `0<S−<S0<S+`. For `a=S+−S0` and `c=S0−S−`, delta is
   `(V+−V−)/(a+c)`, and gamma is
   `2[(V+−V0)/a−(V0−V−)/c]/(a+c)`. Report signed holder PV per
   spot unit and spot unit squared. Delta is an endpoint secant;
   its midpoint can differ from the base. Unusable sides fail rather than
   widening the step or silently switching direction.

4. **Verify common random numbers on admitted stochastic routes.** Select
   the route's explicit controls, effective key, independent group IDs,
   exact nodes and normal/payoff draw slots. Align the logical draws and
   antithetic legs at all required points, including a freshly paired
   center wherever used. Price on the same selected model and frozen
   experiment basis. Keep joint stencil contributions and reduce one
   observation per independent group when reporting uncertainty, including
   covariance from shared draws. Reuse
   [the accepted paired spot estimator](../models-monte-carlo/25-finite-difference-greeks.md#accepted-decision-for-checked-paired-monte-carlo-spot-greeks)
   for its supported domain. Other stochastic Greeks need an owner-qualified
   coupling, estimator and uncertainty basis. Refuse an unqualified
   stochastic stencil; equal seed strings alone prove no pairing. Remove
   the private `greeks_crn` replacement and the mutable-override
   requirement for deterministic repricing. Sampling uncertainty excludes
   finite-step, payoff discontinuity, time-grid and model errors.
   [Glasserman, sensitivity estimation](https://doi.org/10.1007/978-0-387-21617-1).

5. **Shock the actual active native volatility inputs.** Use the selected
   scalar implied sigma in its model clock, or the complete exact active
   stored annual-sigma grids in their source clocks under
   [record 02](model-resolution.md#accepted-decision-for-checked-valuation-model-inputs-and-controls)
   and [CORE-MARKET-14](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks).
   True parallel moves all nodes of every selected grid; deduplicate only
   identical typed sources. For source quotes reapply the qualified
   variance-clock bridge `sigma_model² t_model = sigma_source² t_source`
   where supported, in each reprice. Remove
   present-only filtering and the reference-strike footprint heuristic.
   An inactive declaration needs no lookup; missing required inputs or
   an unsupported active binding cannot reduce the requested scope.
   A replacement grid is a direct parameter shock. Keep SABR parameters,
   qualified materialized grids, calibration quotes, FX ATM/RR/BF,
   Heston variance and Hull–White sigma distinct. Owner-approved
   parameter or quote risk needs its own exact basis and qualified replay
   whenever applicable. A fixed materialized strike grid and held raw
   FX delta quotes that respond to a moved forward are different
   spot/volatility experiments. Bind the smile response and delta and
   quote orientation explicitly. Data presence does not authorize a
   conversion, rematerialization or refit.
   [FX quote ownership](../core-market/fx-delta-quotes.md#accepted-decision-one-explicit-fx-quote-profile-and-qualified-forward-inputs),
   [SABR source ownership](../core-market/sabr-cube-data.md#accepted-decision-canonical-sabr-sources-and-explicit-evaluated-quote-meaning),
   [Reiswich and Wystup](https://www.econstor.eu/bitstream/10419/40186/1/613825101.pdf).

6. **Use fixed checked vega, volga and vanna stencils.** Let `q>0` be
   the native sigma increment for one reported volatility unit.
   Black and shifted Black use `q=.01`; Normal requires the explicit
   increment in actual underlying units, for example `q=.0001` on a
   decimal-rate Normal per-bp basis. Only compatible sources share a
   common native sigma shock and reporting basis. Let `a,c>0` be the
   upper and lower widths on that axis; use the verified common
   amplitudes for parallel vectors. Report
   `vega=q(V+−V−)/(a+c)` and
   `volga=2q²[(V+−V0)/a−(V0−V−)/c]/(a+c)`.
   For vanna, report
   `q[(V_pp−V_pm)−(V_mp−V_mm)]/(ΔS Δσ)`, over the actual ordered spot and
   sigma spans of the verified rectangle. The first sign indexes spot.
   Subscripts `p` and `m` mark the upper and lower points.
   The mixed secant is centered at the rectangle midpoint and must not
   be labeled a base-local derivative for unequal widths. For vega
   only, an owner-admitted boundary rule may use the full up move and
   matched base, `q(V+−V0)/a`, if the full down move lies outside the
   admitted volatility domain. An exact zero lower endpoint qualifies
   if admitted by the product. Missing data, failed pricing or collapsed
   moves never trigger one-sided pricing. This shared volga/vanna profile
   requires both full sides. The caller can request another step.
   Analytic model-sigma partials, clock-transformed source partials and
   full grid or recalibrated quote risks retain their separate basis.
   [Strata Black forward price derivatives](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/impl/option/BlackFormulaRepository.html).

7. **Report exact sparse node vega with matched reconciliation.** Select
   exact stored coordinates on qualified Strike or Tenor grids. Require
   an explicit all-native-nodes or source-qualified filtered selection.
   Within a filtered selection an omitted axis selects the full axis and
   an explicit empty filter selects no nodes. Validate the complete
   request, uniqueness, exact membership and actual work before pricing.
   Bump just the selected node and apply the vega rule to that node's
   domain. Store a real computed zero and omit nonexistent cells.
   Adopt the three separate coordinates under [ADR 0010](0010-metric-identifiers.md)
   and use the record 08 identity rules for
   `bucketed_vega::<source>::<expiry>::<secondary>` and preserve the
   source's axis, clock, native units and canonical order. Scalar-only
   risk has no grid bucket series or grid residual. Return the checked
   compensated sum for each compatible selected scope. When at least
   one node is selected, compute a dedicated comparator on that same
   union of selected nodes, shocked simultaneously with the same
   dispatch, active sources, units, base, controls and stochastic
   coupling. A subset comparator must identify its subset. Keep node
   and comparator stencil differences visible. Report
   `bucketed_vega_residual = matched_parallel − raw_bucket_sum`.
   An unrelated native or scalar Vega cannot fill the comparator.
   Preserve coverage, including an intentional empty selection, and
   unavailable required results. If a required comparator is inadmissible,
   propagate the failure on that basis. Drop the implicit 10% warning and
   never rescale buckets. Finite nonlinear effects and different
   admitted boundary stencils can create a residual even with
   complete coverage. [Strata parameter sensitivities](https://strata.opengamma.io/apidocs/com/opengamma/strata/market/param/CurrencyParameterSensitivity.html).

8. **Keep FX01 a fixed directional scenario.** Retain the named joint
   `+1%/−1%` relative experiment on the exact active ordered FX
   factors selected by the owner. Apply the declared direct quote
   override or frozen matrix route for each target, with explicit
   reference date and lookup policy. Hold realized fixings, non-FX
   inputs and authorized frozen reporting conversion fixed. Use one
   coherent base; validate all required routes, signed moves and
   dependent transformations. The reverse orientation divides by
   `1+b` when the target orientation multiplies by it, preserving the
   reverse baseline and the NDF owner's quote mapping. Reject conflicting
   reciprocal directions, double shocks and unresolved overlaps.
   A working override for one factor does not replace other active
   factors or authorize a matrix fallback. Report `(V+−V−)/2` in
   holder currency per one positive relative percentage point of the
   specified joint direction. This is neither spot delta nor pip risk.
   Reconcile any producer using a configurable step under the FX01
   identity, including [quanto](../../../../finstack-quant/valuations/fx/src/quanto_option/metrics/mod.rs).
   Attribute it only to a matching direction; distinct moves need
   separately qualified per-pair results or joint repricing.
   [Frozen directional FX shocks](../core-conventions/013-fx-resolution-and-shocks.md#accepted-decision-fx-resolution-state-and-relative-shocks).

9. **Separate rate and time bases.** Keep standard owner-defined
   domestic and foreign rho per +1 bp on their declared rate factors.
   A full curve DV01 cannot stand in for a local rho partial.
   Preserve FX spot, forward and premium-adjusted hedge bases as
   separately identified measures. The selected Hull–White curve
   delta/gamma route shocks its actual typed projection or discount
   evaluator under record 09 in parallel continuous zero-rate
   coordinates and reports PV per decimal rate and squared decimal
   rate. Retain the qualified grid, market roles and explicitly allowed
   held-parameter or refit policy. Its basis differs from a forward
   price partial, DV01 and Hull–White sigma vega. Preserve listed
   contract and CDS option owners. A local diffusion/time Greek is
   `−∂V/∂T` per model year on the owner's held-input basis; a
   per-day output requires the owner's explicit reporting conversion.
   A local
   fixed-market finite reprice qualifies only on the stated dates and
   lifecycle branch. Distinguish both from elapsed-horizon theta
   under record 13, even at one civil day. Remove synthetic zero
   cash carry, roll-down and `theta_period_days=1` from a native
   derivative. Only an actual qualified horizon run publishes those
   components and its actual period.
   [Analytic spot Greeks](../models-analytic/spot-vanilla-and-greeks.md#accepted-decision-checked-spot-prices-native-derivatives-and-explicit-composite-results),
   [listed options](listed-products.md#accepted-decision-for-checked-listed-futures-and-options),
   [CDS options](../models-and-valuations-credit/16-cds-option-risk-and-inversion.md#accepted-decision-for-checked-cds-option-risk-and-implied-volatility),
   [cap/floor risk](../valuations-rates/06-cap-floor-risk.md),
   [Hull–White swaption ownership](../valuations-rates/08-hull-white-swaptions.md),
   [horizon and cash decisions](carry-theta-breakeven.md).

10. **Reconcile the actual risk and reporting paths together.** Use one
    checked Rust contract across native, registry, portfolio,
    attribution and current Python/WASM paths. Preserve exact identity,
    signed and sized currency result, factor coordinates, scenario and
    lifecycle basis, actual stencil and widths, applicable uncertainty
    and sparse availability through catalog, schema, declarations,
    tables and parity. Keep raw and finalized Money bases explicit.
    Qualify independently with analytic limits, unequal represented
    widths, boundary/domain tests, multi-factor and delivery states,
    covered bucket comparisons, and bump, grid and path convergence.
    Neither a matched seed nor a small residual certifies a Greek.
    Cross-factor market risk and implied-vol breakeven retain records
   [12](cross-factor-risk.md) and [13](carry-theta-breakeven.md). The
   [equity selected-risk record](../valuations-equity-and-composition/08-selected-risk-and-implied-vol.md)
   retains dividend risk, the IV inverse and product-owned higher Greeks.

## Observed behavior

## Choice and actual selection

The option Greek registry can use a native product provider, a selected-model provider, or generic finite differences. A nondefault selected model calls `selected_model_greek`; its default implementation errors, preventing an automatic native-model substitution. A provider may explicitly return `None` when native Greeks remain applicable after lifecycle resolution. Sparse unavailable Greeks stay unavailable, not zero. Non-daily theta requests route to the common horizon roll. Native daily theta stores zero cash carry, the returned theta as roll-down, and one day of horizon.

Active price bindings may differ from the declarations the calculator probes.
For example, fixed selected Heston parameters can leave a declared Black grid
unused, and a selected Hull–White cap/floor pricer can consume stored model
parameters rather than the grid that its Vega calculator bumps.

## Finite-difference algorithm and units

Generic spot delta/gamma use the first declared market scalar, require positive finite spot, and take an absolute shock `h=max(S×relative_bump,1e−8)`, applying exactly the matching relative shock. Delta is `(V+−V−)/(2h)` and gamma `(V+−2V0+V−)/h²`. Clone the instrument with `mc_seed_scenario="greeks_crn"` and price every stencil point with the same dispatch and seed scenario, including recomputing the base for gamma. A missing mutable override container errors for these generic calculators; common random numbers do not guarantee low bias or sufficient paths.

Vega bumps the active scalar vol override or all present declared surfaces by absolute `k`. It reports `(V+−V−)/(2k×100)`, per one volatility point. If relevant minimum vol is at most `k`, switch to first-order `(V+−V0)/(k×100)`. The relevant footprint is the two strike columns bracketing each declared reference strike across all expiries, or the full surface when no reference strike is declared. This optimization assumes that footprint describes the actual pricer lookup. Missing all declared surfaces errors; partially present surfaces are the bumped set.

Volga uses `(V+−2V0+V−)/(100k)²`; vanna uses four spot/vol corners divided by `4h×100k`. Their symmetric volatility step shrinks to at most half the relevant positive minimum vol; zero active vol errors. They return zero on/after expiry. Nonfinite results fail. Selected Hull–White rate delta/gamma use parallel rate shocks but normalize by the decimal-rate width, yielding per-rate-unit or per-rate-unit-squared results rather than DV01.

FX01 applies a fixed ±1% relative move to instrument-provided quote overrides when both directions are supported, otherwise to all declared FX pairs. It reports `(V+−V−)/2`, PV per 1% move. No FX dependencies gives zero. This fixed shock is separate from the configurable generic spot shock.

## Surface bucket algorithm

`KeyRateVega` uses all actual surface nodes by default; requested coordinates must be unique increasing exact source nodes. For each surface/expiry/strike, bump only that node, centrally or one-sided when the surface minimum is below the shock. Combine heterogeneous strike grids in a union-column matrix with zero in absent cells. Return the raw sum; record `BucketedVegaResidual=parallel_vega−raw_sum` and warn if its magnitude exceeds 10% of the parallel amount. No proportional rescaling forces reconciliation. An active scalar override produces parallel quote vega while market-surface node shocks can remain zero; the residual exposes the uncovered quote risk.

## Limits, alternatives and evidence

Analytic/pathwise/AAD Greeks are alternatives only where product providers implement them. Matching a production closed form or reusing a seed is useful but does not independently validate the stencils around exercise, barrier and volatility boundaries.

- [fd_greeks.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/fd_greeks.rs), [option_greeks.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/option_greeks.rs), [provider trait](../../../../finstack-quant/valuations/core/src/instruments/common_impl/traits/option_greeks.rs), [vega.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/vega.rs) and [fx01.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/fx01.rs) contain the inspected implementation and inline behavioral/formula tests.
- [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) provides common-random-number context; [Hull](../../../REFERENCES.md#hull-options-futures) provides Greek conventions. Neither establishes adoption of these particular grids, shock sizes or fallback thresholds.

### Independent review evidence

- With represented spot widths `a=1` and `c=.5` at `S0=2^52`,
  price `V(x)=5+3x+x²` in the spot offset `x` produces secant `3.5`
  and curvature `2`. The local base slope is `3`.
- A discounted Black call with `F=102`, `K=100`, `T=1.4`,
  `sigma=.20` and discount `.97` gives vega `.4575991965`,
  volga `−.0001601009` and forward vanna `.0006567071` on a `.01`
  volatility increment. Centered finite differences with steps `.001`
  in forward and `.0001` in sigma agree within `2e−9` absolute.
  At zero sigma the ATM Normal right vega is about `4.57874e−5` per
  native sigma bp for `F=K=−.01` and the same clock and discount.
- A fixed premium-unadjusted forward 25-delta Black call at annual
  sigma `.20` and expiry `1.4` moves strike from about `1.3511311`
  to `1.3680202` as forward moves from `1.12` to `1.134`.
  In a separate FX01 check, a claim of 100 base units at direct quote
  `1.25` yields `1.25` per positive 1% move in that quote.
  Applying the same economic moves through reciprocal `.8` and
  restoring the direct value gives the same result exactly.
- For `V(x,y)=1000x+2500y+10000x²y` at `(.20,.30)` with step
  and increment both `.01`, isolated node vegas are `22` and `29`.
  Matched simultaneous vega is `51.01`; the signed residual of
  `+.01` survives complete coverage and exact arithmetic.
- For an ATM spot Black–Scholes call at `S=K=100`, `r=.04`,
  dividend yield `.01` and sigma `.20`, one calendar day before
  expiry, local theta per 365-based reporting day is about
  `−.2128985`. The dated price change to an independently specified
  ATM spot at expiry is about `−.4217235`.

These are standalone arithmetic examples, including an analytic-versus-FD
check on a smooth case. They exercise no Rust pricer, calibration, stochastic
execution, lifecycle state, reporting host or production convergence.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
