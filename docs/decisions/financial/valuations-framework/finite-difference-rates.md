# VALUATIONS-FRAMEWORK-09 — Finite-difference conventions and combined, per-curve, and key-rate DV01

- Decision status: **Accepted — complete checked finite differences and rate risk package, 2026-11-13**.
- Implementation status: **Observed, with reconciliation pending** across rate risk, finite-difference consumers, replay and hosts.
- Validation status: Source and primary references inspected. Independent bounded finite-difference, curve-shock and aggregation examples checked. Runtime, calibration-replay and host qualification remain pending.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked finite differences and rate risk

1. **Resolve explicit controls for the selected metric.** Freeze one
   request and resolve its supplied fields over stored instrument metric
   terms, then an explicitly selected workflow profile if any. Require
   the active step and grid controls. An explicitly selected standard
   rate profile may supply a 1 bp calculation step and the documented
   `0.25,0.5,1,2,3,5,7,10,15,20,30` year grid. Omission alone selects
   neither. Keep signed economic shocks, positive calculation steps,
   and reporting increments separate. Preserve explicit units and check
   provided controls, their relevance, and the actual selected operation.
   The registered metric contract fixes its financial meaning. Changing
   a mode or factor role that changes that meaning requires an approved
   identity or a reviewed key change. No implicit adaptive step or
   product-specific fallback is introduced into fixed-step DV01.
   [Financial units and signs](../conventions/0007-units-and-sign-conventions.md),
   [metric identity](0010-metric-identifiers.md).

2. **Validate the represented experiment before differencing.** Require
   finite positive requested calculation steps and checked, distinct,
   ordered coordinates in the admitted domain. For a scalar derivative
   use the actual applied scalar endpoints, including for relative spot
   changes. Preserve the ID, type and currency of a bumped quote. For a
   vector or transformed-source experiment, use its declared common
   scalar amplitude and verify each required signed move and transform.
   The applied direct rate amplitude is the zero-rate parameter, while
   individual forward rates and factors are resulting values. Exact
   identities of the transform, such as a unit factor at its reference
   or a zero-weight bucket coordinate, are valid. Preserve
   requested and represented coordinates and the applied shock direction.
   A collapsed required move, orientation reversal or unresolved result
   errors. Remove absolute width floors, false zero returns and the
   different debug and release behavior of the existing helper.

3. **Name the stencil and its centering.** At represented scalar nodes
   `x−c,x,x+a`, with `a,c>0`, define upward and downward slopes as
   `d+=(V+−V0)/a` and `d−=(V0−V−)/c`. The two-endpoint secant is
   `(V+−V−)/(a+c)`. When unequal moves require a derivative at the
   base from a three-node interpolant, use `(c d+ + a d−)/(a+c)`.
   The three-node curvature is `2(d+−d−)/(a+c)`. A four-corner mixed
   secant divides the signed cross difference by the two actual axis
   spans. Require a rectangle on the stated axes. With unequal endpoint
   spacing its centering is the rectangle midpoint. Build a base-local
   mixed derivative only through an owner-approved stencil. Use checked
   differences and scaled arithmetic, avoiding unnecessary overflow or
   underflow in widths, products and coefficients. Keep genuine finite
   stencil zero distinct from proven structural zero. An eligible
   one-sided stencil requires a declared domain boundary and owner rule
   before pricing. Missing data or a failed reprice does not select it.
   [Fornberg on finite differences on arbitrary grids](https://doi.org/10.1090/S0025-5718-1988-0935077-0),
   [paired Monte Carlo spot Greeks](../models-monte-carlo/25-finite-difference-greeks.md#accepted-decision-for-checked-paired-monte-carlo-spot-greeks).

4. **Keep all reprices comparable.** Use the same effective date and
   information phase, product, dispatch, native currency, quantity, cash
   basis and admitted controls. Start from one qualified baseline and
   use the product's checked rate-responsive rebuild for each scenario
   where required. Rebuild only what the selected experiment permits.
   Keep active projection and discount bindings fixed in identity and
   role, even when a curve is replaced by its shocked view. Use genuine
   raw reprices for raw risk. Any explicitly admitted Money-only route
   keeps checked monetary differences before conversion. Isolate scratch
   market state and publish a complete validated result. Keep any pricing
   error allowance, sampling and finite-step approximation evidence with
   its model or product. [Accepted dispatch](pricing-lifecycle.md#accepted-decision-for-checked-valuation-dispatch-and-lifecycle),
   [accepted metric orchestration](metric-orchestration.md#accepted-decision-for-checked-metric-orchestration-and-reporting).

5. **Shock the exact active rate sources.** Resolve all, discount or
   projection roles from the prepared metric valuation, including its
   actual additional requirements. A declared inactive source needs no
   lookup. Any missing, ambiguous or wrong-kind required active source
   fails the complete experiment. Deduplicate only the same actual typed
   source. If the model explicitly uses one evaluator for both roles,
   its single source shock affects both. A role-isolated measure needs
   an explicitly qualified separate experiment. Equal ID text neither
   proves duplicate source identity nor establishes a single-curve
   model. An empty unsupported selection is unavailable. Return a
   structural zero only when the metric owner establishes it on the
   admitted basis.

6. **Separate direct rate risk and market quote replay.** Direct risk
   uses [CORE-MARKET-03](../core-market/rate-shocks.md#accepted-decision-explicit-zero-rate-shocks-and-dated-conditioning):
   a signed additive continuous zero-rate parameter shock to the complete
   discount or projection evaluator. Use its retained source coordinate
   `x=τ_B(T)−τ_B(reference)` and the selected weight `w(x)` in
   `exp(−b x w(x))`, with `b` in annual decimal rates. Curve anchors
   and valuation information dates retain their separate roles. Quote
   replay uses exact authorized quote targets, their quote-unit mapping,
   qualified original-source center replay and the bound calibration
   provider. Require the shocks and refits for every required target.
   Missing recipes or failed replay propagate on the selected basis.
   Keep product yield DV01, quote-pinned model risk and frozen observed
   mark risk separately defined. Credit owners select any hazard replay
   accompanying a rate shock.
   [Accepted calibration replay](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification),
   [Strata calibrated and quote PV01 measures](https://strata.opengamma.io/apidocs/com/opengamma/strata/measure/Measures.html).

7. **Report each DV01 mode as a separate signed experiment.** For
   represented scenario amplitudes `b−<0<b+` in bp, use
   `(PV+−PV−)/(b+−b−)` in native currency per positive 1 bp.
   A long fixed cash claim generally has negative direct discount DV01.
   Combined parallel moves all selected sources simultaneously.
   Per-source parallel moves each selected source alone and reports
   qualified source results plus their checked compensated sum.
   Triangular key rate moves one source and one bucket per experiment
   and reports checked bucket, source and total sums. The calculation
   width can differ from 1 bp. Publish the requested mode, applied basis,
   actual shock and role scope. Keep finite directional measurements
   distinct from an exact one-sided 1 bp P&L and from a smooth-limit
   derivative. A per-source or bucket total can differ from combined
   parallel for a nonlinear valuation. Report any reconciliation on
   matched inputs and a stated tolerance, without rescaling results.

8. **Use exact triangular bucket coordinates.** Require a positive,
   finite, strictly increasing tenor grid in source years for shared
   DV01. Use exact positive neighbor widths, unit center weights and
   linear ramps under CORE-MARKET-03. Extend the outer wings flat.
   A singleton is a parallel shape. Retain complete source evaluation,
   including native interpolation and tails. Never round coordinates
   before calculating or storing them. Preserve approved standard labels,
   and use round-trippable exact labels for other year coordinates.
   Bound the requested and actual work and reject conflicting result
   keys before publication. Partition of unity describes the shock
   shapes and the corresponding first-order relation under a smooth
   common basis. It does not force equality of finite PV differences.

9. **Use the admitted native domain for other shared shocks.** Bind
   volatility to the active scalar or typed source under record 02 and
   [CORE-MARKET-14](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks).
   Use native annual volatility units. Zero is a valid volatility
   endpoint when the product admits it. Reject negative changed nodes.
   A floor on some nodes changes the direction of a parallel shock.
   Keep step size and reporting unit separate, including correlation
   per `.01` where selected and volga per volatility point squared on
   the stated native scale. Owner records select vega and volga stencils,
   credit quote and direct-hazard risk, and correlation or mixed-factor
   domain and reporting rules. Reuse the same checked arithmetic.
   [Option risk](option-sensitivities.md), [credit risk](credit-sensitivities.md),
   [cross-factor risk](cross-factor-risk.md).

10. **Reconcile shared and owner routes together.** Apply the same
    checked controls, basis and errors to existing native and registry
    metric paths, instrument-level rate risk, portfolio and attribution,
    and Python/WASM pricing and metric surfaces. Expose selected profile
    or explicit controls and actual applied shock provenance through the
    existing wire, metadata and reporting routes. Update schemas,
    declarations and parity together. Validate independent smooth and
    nonlinear examples, near-boundary or unresolved steps, active
    shared-source bindings, and model-specific quote and curve replay.
    Product conventions and accuracy limits retain their named owners.

## Observed behavior

### Choice and controls

Shared risk resolves defaults from `valuations.sensitivities.v1`, then overlays instrument metric bump settings. Defaults are 1bp rate and credit shocks, 1% relative spot and 0.01 absolute volatility. Yield bump is absent unless supplied; product calculators then choose their own default. Bumps must be positive finite. Custom bucket grids must be nonempty, positive finite and strictly increasing. Standard coordinates are 0.25, 0.5, 1, 2, 3, 5, 7, 10, 15, 20 and 30 years. These are calculation controls, not market calibration evidence.

### Algorithms and conventions

The central derivative is `(PV_up−PV_down)/full_width`; finite-width helpers reject nonfinite or absolute width at most `1e−12`. Relative scalar delta uses `S(1±h)` and divides by `2Sh`, requiring positive `Sh`; money scalar replacements preserve their currency. Mixed derivatives use four corners divided by `4hk`. Volatility bumps follow the active quote source: scalar instrument override if present, otherwise the named surface. Surface nodes floor additive downward bumps at zero; callers use the minimum grid vol to detect a one-sided stencil requirement. A bumped scalar vol must remain strictly positive. Correlation01 uses absolute 0.01 as both default shock and reporting unit. Volga is reported per volatility point squared and therefore scales absolute-vol second derivatives by `0.01²`.

Unified DV01 takes declared discount/forward curve roles and selects all, forward-only or discount-only. No declared selected rates curves means zero risk. Every rate-risk reprice first allows the instrument to rebuild itself against the bumped market, then preserves the metric context's selected model dispatch. Three modes are implemented:

- Combined parallel: shock all selected curves together up/down and report `(PV_up−PV_down)/(2×bump_bp)`. Sign is PV change for a positive 1bp rate move; a conventional long fixed-rate bond usually has negative DV01.
- Per-curve parallel: shock each curve separately, store qualified curve results, and return their compensated sum. This need not equal a finite simultaneous shock exactly for nonlinear products.
- Triangular key rate (default configuration): shock each bucket using adjacent coordinates; first and last wings extend flat outward so weights partition unity. Store exact-coordinate labels and sum bucket values and curve totals with Neumaier compensation. A bucket sum can differ from finite parallel risk by truncation/nonlinearity; partitioning weights alone is not a numerical equality proof.

### Failure behavior

Curve collection records missing dependencies but errors only when **all** selected declared curves are missing. A partial missing set is omitted from bumping if at least one curve was found; later repricing may still error if it needs the absent curve. Scratch mutation is used internally and restored; no historical-risk recalibration is implied by a raw rate curve bump.

Display bucket labels round fractional years to months/years and can collide; calculation keys instead use exact year coordinates (`1.4y`) with parser validation. Do not substitute presentation labels in risk storage. Analytic/AAD sensitivities and adaptive error-controlled finite differences are meaningful alternatives; they are not universally selected by this framework.

## Evidence

- [finite_difference.rs](../../../../finstack-quant/valuations/core/src/metrics/finite_difference.rs), [config.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/config.rs), [dv01.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/dv01.rs), and [buckets.rs](../../../../finstack-quant/valuations/core/src/metrics/buckets.rs) contain formulas, precedence and tests. Tests include degenerate-width errors, exact key-rate label round trips and override precedence; this is behavioral/formula evidence, not a cross-vendor accuracy benchmark.
- [Hull](../../../REFERENCES.md#hull-options-futures) provides general sensitivity context. [IRS risk selection](../../../../finstack-quant/valuations/rates/src/irs/metrics/dv01.rs) shows the observed metadata-driven dispatch. [MBS rebuild](../../../../finstack-quant/valuations/fixed-income/src/mbs_passthrough/types.rs) illustrates an instrument with rate-responsive cash.

### Independent review evidence

- For `V(x)=x²` at `−1,0,+2`, the endpoint secant is `1`,
  the three-node derivative at base is `0`, and curvature is `2`.
  At spot `2^52`, adding and subtracting a requested `.625`
  yields actual moves `+1` and `−.5`. The represented formulas
  return delta `1` and curvature `0` for a linear price. A mixed
  numerator of `1e−300` and two half-widths of `1e−200` produce
  `2.5e99` in real arithmetic even though the width product
  underflows in binary64 when formed separately.
- For a one-year claim of `100` with initial factor `1/1.10`,
  a direct continuous-zero shift of `±1 bp` gives DV01 about
  `−.0090909091061`. Bumping and recalibrating its `10%` annual
  simple market quote gives `−.0082644628782` per bp. Both use
  the same starting PV. A six-month simple forward of 5% moves
  about `1.0250256 bp` under a direct +1 bp zero-rate shock.
- With `V(x,y)=−10x−20y+2x²y`, where the axes are in bp,
  a combined `±1 bp` experiment gives `−28` per bp. The
  separate experiments sum to `−30`. For a two-year zero-coupon
  payoff of `100` at 4% continuous, using `±50 bp` gives
  parallel DV01 about `−.0184626346347`. Two equal bucket weights
  at its payment give a finite bucket sum `−.0184624038542`.
  The smooth-limit derivative normalized to one bp is about
  `−.0184623269277`.
- For a parallel down-volatility move `.01` on native nodes
  `.003,.020`, flooring the first node at zero would move them
  by `−.003,−.010`. A uniform down shock is not represented.
  A separately selected boundary stencil must retain its declared
  direction and validate each required node.

Production qualification requires actual bound rate and option sources,
calibration replay including center reproduction, matched native repricing,
checked risk consumers, distinct quote and curve reports, and host evidence.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
