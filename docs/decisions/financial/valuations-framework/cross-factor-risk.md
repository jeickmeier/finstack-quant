# VALUATIONS-FRAMEWORK-12 — Cross-factor finite differences with ordered credit recalibration

- Decision status: **Accepted — complete checked cross-factor risk package, 2026-11-13**.
- Implementation status: **Observed, with reconciliation pending** across cross-factor calculators, calibration replay, attribution and hosts.
- Validation status: Source and primary references inspected. Independent bounded stencil, quote replay, normalization and attribution examples checked. Runtime, calibration-replay, financial accuracy and host qualification remain pending.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked cross-factor risk

1. **Register a specific pair and valuation basis.** Retain the seven
   family identifiers: Rates×Credit, Rates×Vol, Spot×Vol, Spot×Credit,
   FX×Vol, FX×Rates and Credit×Vol. A family name does not establish
   product support. Admit only a registered product route with two
   independent, specified shock axes and an explicitly qualified joint
   experiment. Use one prepared effective date, information phase,
   lifecycle, holder currency and position, and the exact selected raw
   model and controls. Freeze contractual terms, realized history and
   other inputs, allowing only the declared dependent pricing and
   calibration responses. The specialized swap `ir_cross_gamma` and
   owner-specific parameter, quote or key-rate crosses remain separate.
   [Accepted dispatch](pricing-lifecycle.md#accepted-decision-for-checked-valuation-dispatch-and-lifecycle).

2. **Bind every active input, unit and step on both axes.** Require
   each positive finite calculation step explicitly under
   [record 09](finite-difference-rates.md#accepted-decision-for-checked-finite-differences-and-rate-risk).
   Keep the step independent from the reporting unit. A declared but
   inactive source does not require lookup. Do not drop any required
   active source or infer an axis from whichever declarations are present.
   Unregistered or unsupported experiments stay unavailable. Missing,
   ambiguous or invalid required data and failed transforms or reprices
   error. A structural zero requires the owner to establish absence
   of the interaction across the admitted joint scope, including
   whether one shock can activate exposure to the other. Validate
   the request and other required active inputs before returning it.
   Preserve structural zero, computed zero and unavailable separately.
   The shared axes are:

   | Axis | Supported shock coordinate | Reporting increment |
   | --- | --- | --- |
   | Rates | Common additive continuous zero-rate move to the exact selected discount, projection and explicitly included calibration rate sources | +1 bp |
   | Credit | Common additive move to the exact selected par credit quotes, with qualified replay | +1 bp |
   | Spot | Relative move of one exact active positive underlying quote; hold contractual reference levels and history fixed | +1% of the base spot |
   | FX | Common relative move to the exact active ordered quote direction or direction set, with the declared override or frozen matrix source, reference lookup and dependencies | +1% in the specified direction |
   | Vol | Common additive annual sigma move to the admitted active scalar or complete compatible native grid scope, retaining quote representation and source clocks | `.01` for Black or shifted Black; explicit positive native increment for Normal |

   Report signed holder PV per product of the two increments. Deduplicate
   only identical typed bindings within an axis. Do not shock a shared
   primitive twice to manufacture two directions. Reject overlapping or
   conflicting writes unless the existing owner defines independently
   controlled input coordinates and their exact joint transformation.
   A legitimate dependent response, such as the discount effect on a
   credit fit, does not create a second quote shock. Preserve the accepted
   FX quote orientation, native volatility and smile dynamics. The FX
   calculation step here is explicit and independent of fixed FX01.
   [Rate shocks](../core-market/rate-shocks.md#accepted-decision-explicit-zero-rate-shocks-and-dated-conditioning),
   [option and FX sensitivities](option-sensitivities.md#accepted-decision-for-checked-option-and-fx-sensitivities).

3. **Stage each joint corner from the same qualified base.** Prevalidate
   the complete pair, shape and target plan, and bound reprices and replay
   work. Apply the full signed input shocks in the owner-qualified order.
   Stage authorized instrument-quote changes too, and bind only the
   explicitly approved calibration inputs to any dependent fit. Use
   one frozen source market for every corner. Preserve
   the two amplitude coordinates across the other axis even when a
   dependent calibrated parameter varies. A threshold or fallback cannot
   silently change the active route, shift width, fit method or coordinate
   in one row. Do not carry scratch state, modified terms or fitted data
   from one corner into another. Independent source writes must have the
   same economic result under either axis enumeration; noncommuting
   shocks require an explicitly admitted transformation or are unavailable.
   Any required failure aborts the pair with the actual target and corner.

4. **Refit credit after the other shock, from fixed center quotes.** For
   each selected hazard, bind the original lossless observations, exact
   par-risk quote identities, recovery, conventions and actual calibration
   dependencies. The fit's discount source can differ from the deal's.
   Declare which of these sources the noncredit axis moves and which
   remain fixed. Require original-source reproduction with the original
   observations and qualify the par-risk center on the unshifted market
   before using it at all corners. At each corner apply the credit shift
   to that one fixed set of par quotes and fit against the complete,
   already staged target state. Verify every required represented signed
   quote move before fitting. A zero credit offset still uses the same
   par-risk center on that corner's dependencies. Keep deal cash,
   recovery and realized observations fixed. A responsive input enters
   the calibration only through its exact declared binding. Require
   accepted target fits and update all affected valuation references
   before repricing. Independent credit fits read the same staged target
   state; rebuild once for a shared request only when the full bindings
   agree, including policy, numerical seed inputs, target dependencies
   and signed action. Publish related replacements atomically, including
   admitted composite references, without changing unrelated sources.
   Any supported dependent fit uses its existing owner-defined
   dependency order. Reject conflicting outputs or unsupported or cyclic
   fit dependencies. Never re-imply the center from the shifted rate
   market or substitute a rate-frozen fit, frozen hazard, direct hazard
   or z-spread basis. Calibration owns the fits and fit acceptance.
   [Accepted credit risk](credit-sensitivities.md#accepted-decision-for-checked-credit-sensitivities),
   [qualified replay](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification),
   [atomic market updates](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle),
   [White on CDS calibration and par-spread risk](https://quant.opengamma.io/Pricing-and-Risk-Management-of-Credit-Default-Swaps-OpenGamma.pdf).

5. **Use a checked four-corner mixed secant by default.** For each
   scalar use its actual represented endpoints in reporting units; for
   verified vector or transformed directions use the declared shared
   scenario amplitudes. Spot at actual quote `S` has coordinate
   `100(S−S0)/S0`; a stable equivalent may carry the normalization to
   the final result. Require an ordered rectangular stencil in the
   two stated coordinates. For actual spans `Δx,Δy`, use
   `[(V_pp−V_pm)−(V_mp−V_mm)]/(Δx Δy)`; `p` and `m` identify the
   upper and lower nodes. This is a finite rectangle average of the
   mixed derivative when the price is sufficiently smooth. Attribute
   the stencil to the actual midpoint, which can differ from the
   unshifted base when represented widths are unequal. It need not
   equal a point derivative at the midpoint or base. Spot×Vol equals
   `Vanna × S0/100` only for matched corners, sigma scales, spot
   basis and held-input policy; no automatic Vanna fallback.
   [Fornberg on represented-node weights](https://people.sc.fsu.edu/~jburkardt/m_src/differ_arbitrary/fornberg.pdf).

6. **Permit a separate explicit volatility-edge stencil.** This
   shared boundary mode is opt-in and requires owner admission.
   Select it before pricing only if the full down volatility step
   crosses the declared volatility boundary and the complete
   `0,+h,+2h` vol stencil is admitted. The other axis stays
   two-sided. A lower point at exactly zero is eligible for the
   central stencil where the product admits it. With actual volatility
   offsets `0<a<b` in reporting units, form the other axis's secant
   `g` at each vol node using the same two endpoints. Set
   `u=(g(a)−g(0))/a`, `v=(g(b)−g(a))/(b−a)` and report
   `u−(a/b)(v−u)`, the quadratic interpolant derivative at zero.
   Equal spacing gives `[−3 g(0)+4 g(h)−g(2h)]/(2h)`.
   The six prices include both zero-offset vol corners at their
   other-axis shocks. Record the midpoint on the two-sided axis and
   base on the vol axis. Remove minimum-vol shrink and floors.
   Missing data, failed reprices, replay failures and collapsed
   moves cannot select this mode. There is no fallback on the other
   axis. This mode does not alter record 11's shared vanna rules.

7. **Use checked arithmetic and verify stochastic coupling.** Keep
   the admitted raw or authorized Money amount basis comparable across
   corners. Use checked, scaled differences and ordered compensated
   accumulation as appropriate; avoid avoidable overflow or underflow in
   intermediate widths, weights and PVs. Admit only a finite result.
   Stochastic routes need an owner-qualified mixed estimator, common
   logical group, node, leg and draw-slot mapping at every corner,
   and paired contributions and uncertainty under record 11. The same
   random seed or independently simulated PVs do not qualify.
   Conditional sampling uncertainty excludes finite-step, discontinuity,
   calibration, grid and model error. Expose the requested and actual
   steps, stencils, scenario directions, fit evidence and uncertainty.
   [Glasserman, sensitivity estimation](https://doi.org/10.1007/978-0-387-21617-1).

8. **Pair attribution with the exact observed moves.** Qualify the
   opening reference and both complete directions under
   [CORE-MARKET-22](../core-market/market-change-measurement.md#accepted-decision-explicit-comparable-quantities-and-ordered-measurement-rows).
   Only use a scalar move on each axis when the relevant required
   movements are verified to match that exact direction in the opening
   experiment's coordinates and units, under the owner's stated
   measurement rule. On a qualified mixed Taylor basis count
   `cross_gamma × move_x × move_y` once, with no factor of one half.
   A rate twist, unequal quote or FX moves, primary-pair proxy or
   successful partial mean cannot supply the missing coordinate. Keep
   unmatched interaction unresolved, with attribution coverage and
   residual handled by its owner. A full repricing interaction or the
   correction to closing marginal P&L has its own basis, not this
   opening mixed Taylor term. A frozen-hazard DV01 does not provide
   the rate partial of a Taylor function that refits credit at fixed
   par quotes when rates move.
   [Metric attribution](../attribution/10-metric-other-factors.md),
   [parallel attribution](../attribution/05-parallel-and-cross-factor.md).

9. **Keep both identities through aggregation and reporting.** Use the
   canonical pair order and exact admitted axis identities and scope,
   with no duplicate transposed contribution. Where multiple experiments
   share a family, identify them separately under
   [ADR 0010](0010-metric-identifiers.md) and
   [record 08](metric-orchestration.md#accepted-decision-for-checked-metric-orchestration-and-reporting).
   A bare family name does not qualify a portfolio total or an
   attribution match. Require compatible signed quantity, currency,
   two-direction basis, method and reporting units. Keep factor, scheme
   and coverage visible in Rust, portfolio and attribution, and existing
   Python/WASM, catalog and result wire views. Qualify bounded work,
   all-corner geometry, replay and permutation behavior, measured
   movements, stochastic uncertainty and convergence independently.
   The shared result is one mixed directional measure, with no generic
   full Hessian, analytic mixed provider, bucket matrix or guarantee
   that it explains large, nonlinear or lifecycle-crossing P&L.
   [Strata cross-parameter identities](https://strata.opengamma.io/apidocs/com/opengamma/strata/market/param/CrossGammaParameterSensitivity.html).

## Observed behavior

## Choice, scope and controls

Cross-factor calculators combine rates, credit spreads, spot, FX and volatility using common factor bumpers. Rates and credit report per bp; spot/FX report per one percentage-point relative move; volatility reports per one volatility point. Thus spot–vol cross gamma differs from absolute-spot vanna by `S/100`. Bump settings follow the shared sensitivity configuration. Pair identifiers describe these units and support the downstream attribution mapping.

The family catalog includes Rates×Vol; the inspected product registry
has no live product route for it. Metrics-based attribution currently
uses family means, primary FX and first-scalar proxies rather than
proving an exact match to each calibrated direction.

## Algorithm

1. Rates collect unique declared discount/forward curve IDs and shock them together. Credit collects every declared hazard and requires a replayable calibration recipe/provider. Spot chooses the first market scalar. FX shocks every declared FX pair. Vol chooses an active instrument quote override, otherwise present surfaces plus unitless scalar volatility dependencies.
2. Central factors use points `(−1,−1/2),(+1,+1/2)`. Volatility reduces its shock to the positive minimum active vol. Exactly zero minimum chooses the forward first-derivative stencil `(0,−3/2),(1,2),(2,−1/2)`; negative/nonfinite minimum fails. The override/scalar boundary here permits zero, unlike generic positive-vol higher Greeks.
3. At each tensor-product corner apply the noncredit factor first, then rebuild credit **from original calibration quotes against the already bumped target market**. This ordering retains rate–credit interaction regardless of the order of factories. Apply any instrument quote shocks, then reprice through selected dispatch.
4. For two central stencils return `(V++−V+−−V−++V−−)/(4hk)`. For a forward stencil use the compensated weighted sum of all corner PVs divided by `hk` in the reporting units above.

## Limitations, failures and alternatives

An absent factor factory or an applicability check returning false yields **zero** cross risk. Rate and FX applicability need only one matching curve/pair; volatility/credit use their own checks. A missing dependency can therefore be indistinguishable from no exposure until later bump/reprice errors. Declared dependency roles and ordering are material input contracts; the framework cannot infer a basket's economically correct scalar from its first entry.

Direct hazard shocks are not a fallback for credit cross gamma. The replay provider is mandatory. The generic stencil does not explicitly seed common random numbers, unlike generic FD option Greeks, so MC variance depends on the product's normal seed behavior. Analytic mixed derivatives, pathwise estimators and single global Hessians are alternatives not selected here.

## Evidence

[cross_factor.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/cross_factor.rs) and [cross_factor_pair.rs](../../../../finstack-quant/valuations/core/src/metrics/cross_factor_pair.rs) own factors, ordering and mappings; inline tests supply behavioral/formula evidence and were not run. [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) supplies numerical risk context.

- [Provider](../../../../finstack-quant/calibration/src/recalibration/provider.rs)
  and [hazard replay](../../../../finstack-quant/calibration/src/recalibration/hazard.rs)
  show the actual corner-fit interface and source/target resolution.
- [Attribution consumer](../../../../finstack-quant/attribution/src/metrics_based/cross_factor.rs)
  shows the current family-move matching and reduction.
- [Existing cross-factor tests](../../../../finstack-quant/valuations/tests/cross_factor_metrics_tests.rs)
  include a missing-replay case. This does not qualify joint replay.

### Independent review evidence

- For `V(x,y)=11+5x+4y+7xy+2x²y+3xy²`, the exact mixed secant
  over `x=(-.5,1)`, `y=(-2,1)` is `5` while the base mixed
  partial is `7`. With symmetric `x=(-2,2)` and irregular forward
  `y=(0,1,2.5)`, the actual-width stencil gives `7`; applying
  nominal equal-spacing weights instead gives `15/8`.
- For payment model `V=10+3(S−250)σ`, a `.01` volatility increment
  gives absolute-spot vanna `.03` and relative Spot×Vol `.075`
  at `S0=250` with matched corners.
- A toy credit fit uses default mass `p` at year one, protection
  `.60 p exp(−r_fit)`, and surviving premium
  `s(1−p) exp(−2 r_fit)`. A one-million year-two survival claim has
  PV `10^6 (1−p) exp(−2 r_deal)`. At `r_fit=r_deal=.04`, `s=.02`
  and 1 bp steps, moving both rate roles gives a mixed secant of
  about `.04077501112` per bp squared. Moving just deal discount
  gives `.02775759780`; moving just the fit's discount gives
  `.01301741295`. Reusing a fit from unshifted rates mistakenly
  returns the deal-only number when both rate roles should move.
- With two rate sources whose mixed coefficients against vol are
  `2` and `1`, a `+2/−2` bp twist and `+1` vol-point move
  produce interaction `2`. A family mean of zero misses it.
  For `V(x,y)=2x+4y+5xy`, the move from `(0,0)` to `(2,3)`
  gives an opening mixed Taylor term `+30` and a closing
  restoration correction `−30` because the closing marginal
  terms count the interaction twice.

These checks use exact rational arithmetic or independent high-precision
cash arithmetic. They do not exercise Rust pricing, calibration, draws,
reporting hosts or production convergence.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
