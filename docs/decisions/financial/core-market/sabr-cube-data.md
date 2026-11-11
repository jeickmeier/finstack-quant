# CORE-MARKET-16: Explicit SABR parameter sources and truthful volatility slices

- Decision status: **Accepted — complete SABR source/slice package adopted 2026-11-08.**
- Implementation status: **Observed** in the owning paths below; the accepted source, model-adapter and slice reconciliation is unimplemented.
- Validation status: **Primary/source evidence and independent source/domain, model/output and consumer/host reviews inspected; standalone rational/high-precision/normal-price examples executed 2026-11-08; production qualification pending.** Existing tests were inspected or located, not executed. Examples do not certify production expansions, interpolation, calibration or arbitrage freedom.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Scope and accepted authority

This record owns canonical SABR parameter/forward grid data, source/evaluation
compatibility and the separate fixed-expiry tenor-by-strike quote artifact.
It preserves accepted [ADR0045 volatility clocks](../conventions/0045-financial-clocks-and-date-boundaries.md#accepted-amendment-volatility-clocks-and-fixing-eligibility),
[record 14's native-sigma grid domains](volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks)
and [record 15's explicit source/adapter boundary](fx-delta-quotes.md#accepted-decision-one-explicit-fx-quote-profile-and-qualified-forward-inputs).
A parameter model is different from a grid of volatility observations.

[SABR expansions](../models-analytic/sabr-expansions.md),
[calibration](../models-analytic/sabr-calibration.md),
[off-grid evaluation/materialization](../models-analytic/volatility-source-interpolation.md),
[swaption calibration publication](../calibration-targets/14-swaption-cube.md),
[underlying forwards](../calibration-targets/13-swaption-forwards.md)
and [option pricing](../valuations-rates/07-european-swaptions.md) retain their
separately stated decision statuses. This source contract does not adopt their full
expansion, interpolation, tail, fit, settlement, smile-dynamics or aging policies.

## Foundational findings

Beta is the SABR backbone exponent, not a normal/Black quotation tag. Alpha's
units depend on beta. Current generic dispatch nevertheless infers quote units
from nearest-node beta. Current cube adapters also floor alpha and cap rho,
including at exact source nodes, and replace absent neighboring shifts with zero.
These operations change source interpretation rather than merely validating it.

Finite signed forward/shift storage is useful model data, but neither their
individual signs nor a selected output label proves that the effective-domain
calculation is supported. A positive shift can leave a negative effective rate;
a negative shift can yield a valid positive effective rate.

Actual product selection matters: a caplet resolver passes tenor 0 and requests
clamping, so a same-ID swaption cube can select its first swap tenor. A swaption
also has a live pricing forward distinct from a cube's stored construction
forwards. Neither a shared ID nor numerical tenor compatibility qualifies the
financial underlying or a frozen/rebased-forward smile model.

## Accepted decision: canonical SABR sources and explicit evaluated quote meaning

Accepted on **2026-11-08** after independent source/domain, model/output
and consumer/host reviews. Runtime/caller/host reconciliation and independent
financial qualification remain pending.

1. **Keep one exact immutable parameter-node contract.** Require finite alpha>0,
   beta in [0,1], nu>=0, rho in (-1,1), and optional finite signed displacement.
   Preserve nu=0, exact beta endpoints and signed/zero shifts. `None` records an
   unshifted node; `Some(0)` is an explicit displacement assertion, not missing
   metadata. No alpha floor, rho cap, beta snapping, absolute-shift restriction
   or parameter repair belongs to source admission or exact-node retrieval.
   Reuse canonical admission in restore and model adapters for these source
   nodes rather than contradictory model-only shift/correlation validation.
   Specialized model limit cases do not expand this stored-source domain.

   Beta/rho are dimensionless, nu has units 1/sqrt(source year), and alpha has
   units underlying^(1-beta)/sqrt(source year) for the declared underlying
   representation. Alpha is generally neither ATM Black nor normal sigma.
   Displacement, forwards and strikes share the same underlying units, such as
   decimal swap rates. Named builder/params inputs prevent nu/rho positional
   confusion; the current constructor orders nu before rho.

2. **Retain a strict parameter/forward grid, not a disguised sigma cube.**
   Require a nonempty ID, nonempty finite strictly positive increasing expiry
   and underlying-tenor axes, exact expiry-major parameter/forward arrays and
   checked axis-size multiplication/indexing. Singletons are valid. Admit
   finite signed forward nodes; do not require positive raw forwards merely
   because a later consumer might request Black output. No sorting, filling,
   default forward, shape repair or value-dependent source retagging.

   Make the existing cube interpolation choice mandatory and explicitly name
   parameter-space versus selected-output total-variance modes. Replace the
   misleading shared `Vol` sigma-grid label for parameter interpolation. These
   selectors record the intended model operation; they do not approve its full
   off-grid kernel or accuracy. Do not duplicate derived sigmas, a beta-inferred
   quote tag or a second independently mutable parameter representation in state.

3. **Require explicit output meaning at evaluation.** Keep `VolCube` a parameter
   source without a persisted sigma quote type. Each active evaluation and
   materialization selects Normal, Black or ShiftedBlack output explicitly.
   Generic valuation requests must carry that selection for a cube; Auto or
   `get_convention` without a selected output must error rather than infer
   units from beta. This does not remove explicit metadata from actual sigma
   surfaces/slices. Pure numeric parameter sources need not invent fit records;
   actual fits retain their input quote conventions in accepted construction
   provenance.

   Normal output has underlying units/sqrt(source year); Black/ShiftedBlack
   output is relative annual sigma. Return/check actual output convention and
   effective displacement with the evaluated value through the existing resolved
   quote contract. A shifted relative expansion cannot be silently labelled
   unshifted Black or assigned a caller's unrelated displacement. Normal output
   can come from a displaced SABR backbone; retain that model displacement in
   source/provenance without attaching displaced-Black quote metadata to Normal.
   Distinct normal/lognormal asymptotic formulas are not exact price-equivalent
   conversions; such conversion has its own explicit qualified owner.

4. **Check the selected model and effective output domain.** Finite raw
   parameters/forwards are structural admission only. Validate finite requested
   strike, positive modeled expiry and actual model/output requirements before
   numerical evaluation. Check complete effective F+s/K+s for finiteness and,
   where required, strict positivity. Signed/zero shift itself is not invalid.
   Only an explicitly supported exact beta=0 normal-backbone method can use
   signed rates across zero; a Normal output tag does not make a fractional
   positive-domain backbone admissible there. Do not use a near-zero beta
   threshold to silently change the model or units.

   Propagate domain, unsupported-operation and computation failures through
   checked `Result` operations, including existing clamped/host helpers; remove
   error-to-NaN, zero-vol substitution, arbitrary floors and hidden parameter
   repair. Use stable complete-result arithmetic and the model's independently
   qualified range/error policy. Data bounds do not guarantee an accurate
   expansion at long expiries or extreme wings, or a representable requested
   computation. This record does not select a new expansion formula.

5. **Preserve exact nodes and own off-grid combinations explicitly.** An exact
   cube node resolves its original complete parameters, shift Option and forward
   without neighbor contamination. Off-grid beta/alpha/shift/forward treatment
   belongs to the selected model. Different betas give alpha different units:
   nearest-beta selection plus ordinary alpha interpolation is not an approved
   generic interpolation rule. Require a supported common-beta domain or an
   explicitly qualified dimensionally coherent model; otherwise refuse the
   unsupported request. Core may still store differing node betas.

   Likewise retain per-node shift presence exactly. Mixed presence is valid
   stored data, not permission to substitute zero for absence or infer a shifted
   quote. The actual off-grid model must handle it explicitly or refuse it; no
   global homogeneous-grid restriction is added to core merely to hide an
   evaluator limitation. Combining outputs with different displacements or
   forward bases requires an explicit model interpretation. Interpolation is
   not an independent calibration, convention conversion or arbitrage guarantee.

6. **Preserve source clocks and actual underlying association.** Add/preserve
   `expiry_clock: Option<VolatilityClock>` on cubes and fixed-expiry slices.
   Numeric sources may omit it; dated producers record their actual mapping,
   and active dated use resolves/agrees with that clock under ADR0045. Ordinary
   direct dated use requires source origin=valuation as_of unless an explicit
   owned seasoned-source model applies. Preserve metadata through state, clones
   and materialization. Context roll cloning does not age expiry coordinates.
   The accepted sigma variance bridge does not automatically rescale alpha, nu
   or the full SABR model across time bases; any parameter/model time conversion
   requires its own qualified owner.

   Underlying tenor is a contract coordinate, not elapsed volatility time or a
   replacement for source expiry. Its numeric units and actual date/tenor
   mapping must agree at the financial producer/consumer. At existing owners,
   bind the source and recorded forwards to the actual currency/index/reference
   swap or underlying, dates and conventions; no ID parsing, default synthetic
   swap or borrowed coupon/discount clock supplies missing qualification. Retain
   immutable stored construction forwards separately from live pricing forwards.
   A deliberately chosen frozen-forward or rebased/recalibrated smile is a model
   decision, not an automatic consequence of market-data presence.

7. **Require compatible active source selection.** Select the actual supported
   source kind/reference before evaluating it or resolving its dependencies.
   Reject wrong/ambiguous same-ID associations; do not silently prefer a cube
   over a surface. In particular, a caplet's missing tenor 0 must not clamp into
   a swaption cube's first tenor. Supply a financially qualified coordinate and
   supported source or refuse the request. A numerical coordinate match alone
   does not qualify a swaption smile for caplets/CMS/another underlying.
   Preserve current explicit unsupported-model refusals such as CMS-spread cube
   rejection; this record does not enable a payment-measure/smile mapping.

   Explicit scalar sigma plus model/convention/displacement inputs and inactive
   lifecycle branches do not inspect unused same-ID cubes or inherit their
   shifts/clocks. Actual source-backed requests retain the real source's complete
   compatibility checks. No generic source-selection/tenor sentinel fallback is
   introduced to recover an unsupported product.

8. **Admit fixed-expiry slices as truthful quote grids.** Keep the distinct
   `VolCubeExpirySlice`: one positive option expiry, positive ordered tenor
   rows, ordered finite absolute strikes, exact tenor-major finite nonnegative
   sigma values including zero, and mandatory quote type. Singletons are valid.
   Apply accepted record 14's known-strike domains: Black K>0; ShiftedBlack has
   one explicit finite signed displacement per tenor row and finite positive
   K+s at every stored node; Normal permits signed finite strikes. Non-shifted
   quote types have no displacement rows. Forwards are not stored in this quote
   slice, so active pricing still checks its actual forward domain.

   Preserve the fixed source expiry/clock and actual sigma units. Variance uses
   that option expiry, never the tenor row coordinate. This is a tenor-by-strike
   artifact, not an expiry surface with renamed axes. Its off-grid interpolation
   remains explicitly model-owned rather than adding a core evaluator/default.

9. **Make materialization an explicit derived model operation.** Consume the
   canonical source, selected output and supported requested tenor/expiry/strike
   domain. Ordinary materialization must not silently clamp a requested expiry
   or tenor. Any deliberately selected boundary operation belongs to its actual
   model and reports the coordinates it used truthfully. Fail atomically if any
   required node/model evaluation fails; do not transport NaN to a later generic
   grid error or fill failed rows.

   Fixed-tenor output is an accepted-record-14 expiry-by-strike sigma surface;
   fixed-expiry output is the distinct tenor-by-strike slice above. Carry actual
   quote type, row-oriented displacements and source expiry clock. Explicitly
   choose the output-grid interpolation rather than copying a cube parameter-mode
   tag whose meaning differs. Retain source parameters/forwards and selected
   model/coordinates/output policy in construction provenance. Sampling does not
   replace or preserve the whole SABR function, fit or risk; nodal agreement does
   not prove off-grid/Greek equivalence.

10. **Keep fit, stress and lifecycle claims at their owners.** A parameter cube
    need not have been calibrated; when it was, preserve accepted lossless
    construction history without claiming current fit after market/source changes.
    Nu=0 storage does not require every fitter to reach that boundary, and a
    fitter's positive-nu bounds do not invalidate the stored deterministic limit.
    Generic core curve bumps do not gain a cube/SABR fallback. Parameter stress,
    sigma-grid vega and quote-recalibrated risk are distinct operations. Bounds,
    metadata and successful evaluation do not establish calibration quality,
    identifiability, calendar/strike arbitrage freedom or future smile dynamics.

11. **Reconcile strict state and existing exposed paths together.** Restore must
    reuse canonical parameter/grid/slice validation and preserve explicit modes,
    optional clocks and actual metadata, rejecting missing required/unknown/
    incompatible state without sorting, defaults, aliases, old readers or
    migrations. Extend existing constructors through named params/builders rather
    than an eighth positional argument. Reconcile context/calibration MarketDatum,
    attribution/state/schema transport, actual SABR/model/materializer/valuation
    adapters, Python/WASM classes/helpers, stubs/declarations/parity and examples.
    Rust owns financial decisions and typed failures; no host calculations,
    new generic convention framework or duplicate fit metadata is introduced.

## Observed implementation and discrepancies

Core already validates immutable nodes and validating restore, including nu=0
and finite signed shifts. Cubes validate positive ordered axes, checked shape
and finite forwards but silently select Vol interpolation metadata and carry no
expiry clock. Fixed-expiry slices keep truthful axes/quote types and row shifts,
but lack clocks and known Black/shifted-strike domain admission. Cube header
comments also disagree with the actual mixed-shift evaluator.

Model adapters bilinearly combine several fields, choose nearest beta, floor
alpha to 1e-8 and cap rho at +/-0.9999, even at exact nodes. Any neighboring shift
causes absent shifts to become zero. Model parameter validation separately
rejects nonpositive shifts and allows rho endpoints, unlike core source data.
Generic convention lookup infers units from beta. Explicit output helpers allow
normal/lognormal formulas but call source-shifted output lognormal and reject
an explicit shifted output request. Clamped helpers map failures to NaN.

Materializers silently clamp requested coordinates and copy the cube's mode to
an output sigma surface; an expiry slice records the clamped expiry, rather than
the originally requested horizon. Neither materializer preserves a source clock.
Actual caplet and generic override routes have the source-selection gaps above.
Swaption calibration retains fitted-node forwards but creates interpolated-node
forwards from synthetic reference contracts. These are source/control-flow
findings, not executed production pricing/fit results; their broader financial
model choices remain with the named owners.

## Independent examples and qualification limits

On **2026-11-08**, fourteen standalone rational/200-digit Decimal and standard-
library normal-price examples were evaluated without production imports.
Normal-CDF comparisons are approximate binary64 references, not high-precision
normal-function or production accuracy certification:

- Exact beta=0, nu=0 gives the arithmetic-normal constant-alpha limit, including
  signed forwards/strikes. Separately, beta=1, nu=0 gives the geometric/Black
  constant-alpha limit; rho cannot change either deterministic-volatility law.
- For F=0.03, T=1 and Black sigma=0.2, the price-equivalent ATM normal sigma is
  approximately 0.005990015. Beta=1 therefore does not identify the output units.
  This is a Black/Bachelier price identity, not certification of normal SABR's
  separate asymptotic expansion.
- Flooring alpha=1e-12 to 1e-8 changes it by 10,000 times. Separately, an admitted
  rho=0.99999 differs from the evaluator's 0.9999 cap.
- With two alpha=0.2 nodes at beta=0.5 and 1, rescaling the underlying by 100
  rescales individual alphas to 2 and 0.2. Their midpoint is 1.1, whereas taking
  the original midpoint then rescaling with selected beta=0.5 gives 2.0. This
  checks model-unit dependence, not a chosen replacement interpolator.
- `None` and `Some(0)` are distinct source states. Separately, F=0.02,s=-0.01
  and F=-0.01,s=0.02 both give effective forward 0.01. A third domain case,
  F=-0.03,s=0.02, stays negative despite a positive shift. Finite MAX+MAX
  overflows in a separate complete-effective-input range case.
- Normal slice strike -0.01 is finite; shifted rows s=0.02 and 0.005 respectively
  give positive and negative effective strikes. Row-shift shape alone cannot
  admit the complete shifted slice.
- At fixed option expiry 2 and sigma=0.02, variance=0.0008; division by tenor 1
  or 5 changes recovered sigma. Separately, beta=1/nu=0 node sigmas 0.1 at
  expiry 1 and 0.3 at expiry 3 give midpoint parameter sigma 0.2, versus
  sqrt(0.07), approximately 0.264575131, under linear total-variance interpolation.
  These fourteen cases distinguish source axes and model operations.

Independent production qualification must cover strict source/host/state input,
exact nu/beta/rho/shift domains and numerical units; explicit output/displacement
and selected model-domain/range failures; exact-node fidelity, mixed-beta/shift
requests and supported off-grid behavior; source clocks/tenor/reference-forward
association, active-source selection/overrides and actual product eligibility;
materialized metadata/coordinates/pillars and chosen model/Greek differences.
Reconcile all exposed paths and use independent market/expansion/fit fixtures at
their owners. Source bounds, expression identities, same-kernel round trips and
sampled repricing do not establish those results.

## Evidence and alternatives

Owning code: [canonical parameter validator](../../../../finstack-quant/core/src/market_data/surfaces/sabr_parameter_data.rs),
[cube](../../../../finstack-quant/core/src/market_data/surfaces/vol_cube.rs),
[expiry slice](../../../../finstack-quant/core/src/market_data/surfaces/vol_cube_expiry_slice.rs),
[model dispatch/interpolation/materialization](../../../../finstack-quant/models/src/volatility/source.rs),
[model parameter representation](../../../../finstack-quant/models/src/volatility/sabr/parameters.rs),
[expansions](../../../../finstack-quant/models/src/volatility/sabr/expansion.rs),
[active source resolution](../../../../finstack-quant/valuations/core/src/market/volatility.rs),
[shared quote resolver](../../../../finstack-quant/valuations/core/src/instruments/common_impl/vol_resolution.rs),
[caplet adapter](../../../../finstack-quant/valuations/rates/src/cap_floor/pricing/pricer.rs),
[swaption adapter](../../../../finstack-quant/valuations/rates/src/swaption/types/swaption.rs)
and [calibration publication](../../../../finstack-quant/calibration/src/targets/swaption.rs).
Existing [source/slice tests](../../../../finstack-quant/core/tests/market_data/surfaces/mod.rs),
[cube/model tests](../../../../finstack-quant/models/tests/vol_cube_tests.rs)
and [SABR comparisons](../../../../finstack-quant/models/tests/sabr_golden.rs)
were inspected or located, not executed.

Canonical [Hagan SABR provenance](../../../REFERENCES.md#hagan-2002-sabr) supplies
model context. Primary implementation documentation distinguishes
[explicit normal output and beta=0 versus positive-domain backbones](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/impl/volatility/smile/SabrHaganNormalVolatilityFormula.html);
[QuantLib's SABR implementation](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/termstructures/volatility/sabr.cpp)
separately selects normal/lognormal output, admits nu=0 and checks positive
effective forward/strike for its shifted positive-domain methods. These are
reference domains, not adoption of all external kernels/tolerances. The original
publisher/author-hosted Hagan PDF was located but could not be fetched in this
review; no direct PDF inspection is claimed. The source/output ownership and
strict adapter boundaries are this record's architectural choices.

Price-equivalent convention conversion, alternative beta interpolation, endpoint
models, global arbitrage constraints and dynamic smile models retain explicit
financial owners rather than becoming hidden core defaults or repairs.

## Follow-up tracking

Runtime reconciliation and independent qualification remain pending and are
tracked in the [core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
