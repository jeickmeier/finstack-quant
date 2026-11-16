# PORTFOLIO-EXECUTION-012 — Factor units, market shocks and dependency routing

- **Decision status:** Accepted 2026-11-15 — complete typed shocks, explicit bump controls, dependency routing, assignment dispatch and provenance package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent unit, weighted geometry, directional FX, reporting bridge and range examples verified. Runtime routing, engine and host qualification remain pending.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Accepted decision for checked portfolio factor shocks and routing

1. **Admit one explicit shock request across entry points.** Require
   ordered unique factor and holding IDs, valid typed mappings and an
   explicit positive finite bump magnitude per active factor in the
   operation's unit. Retire implicit type defaults and the machine-epsilon
   cutoff. Require each requested nonzero move to survive unit conversion
   and be deliverable on the selected source. Signed stress displacements
   and finite-difference magnitudes have distinct meanings. Validate
   targets, controls and work limits before pricing, including for an
   empty portfolio.

2. **Resolve targets once for shock application and routing.** Bind the
   exact namespace, source family, role, ID and financial operation.
   Reject missing, wrong-kind, unsupported and duplicate targets before
   pricing. Unrelated sources may share an ID in distinct namespaces.
   Apply admitted operations atomically through existing market owners and
   retain the delivered effects. Separately authored actions preserve order.
   [Typed market context](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle).

3. **Keep rate, CPI and credit operations financially distinct.** Discount
   and projection shocks add continuous-zero rates on the admitted source
   clock. CPI shocks move anchored annual growth at original pillars.
   Direct credit shocks move native annual hazard intensity and preserve
   recovery. CDS quote replay requires its own explicit operation and
   calibration authority. For weighted buckets, require a strictly
   increasing source-coordinate grid and finite signed weights with at
   least one nonzero weight. Preserve authored weights, neighboring support,
   flat outer wings and the singleton parallel case. A coherent equal-weight
   set reproduces its scaled parallel shock in real arithmetic. Separately
   repriced finite bucket P&Ls can differ from the parallel P&L.
   [Rate shock geometry](../core-market/rate-shocks.md#accepted-decision-explicit-zero-rate-shocks-and-dated-conditioning),
   [CPI shocks](../scenarios/05-direct-rate-shocks.md#accepted-decision-for-typed-direct-rate-and-cpi-parameter-shocks)
   and [hazard and CDS quote shocks](../scenarios/07-hazard-quote-shocks.md#accepted-decision-for-native-hazard-shocks-and-exact-cds-scenario-replay).

4. **Use relative spot and FX shocks and explicit native sigma units.**
   Equity targets are exact, price-qualified spot values. Relative FX
   shocks select an ordered pair and explicit point-date policy. Move an
   independently supplied reverse quote from its own baseline. Volatility
   shifts select the exact supported surface and add annual sigma in its
   declared units. A relative volatility move requires a separate operation.
   Commodity, volatility-index, correlation and parametric sources require
   separately supported typed operations.
   [FX and equity shock rules](../scenarios/04-fx-and-equity-shocks.md#accepted-decision-for-frozen-fx-and-checked-equity-scenario-shocks)
   and [volatility shock units](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks).

5. **Route native pricing and reporting FX separately using proven effects.**
   Compare opening and target source selection, replacements and credit-index
   constituents. Reprice when a valid shock's delivered effects or required
   dependencies cannot be proven. Reuse native PV for proven reporting-only
   changes and refresh conversion. Reporting dependencies include the selected
   FX quote route and any bridge curves. Skip a holding only when native
   pricing and reporting conversion are both proven unaffected.
   [Selective repricing](05-dependencies-and-selective-reuse.md#accepted-decision-for-checked-portfolio-dependencies-and-selective-repricing)
   and [cash FX](08-spot-and-forward-fx.md#accepted-decision-for-checked-portfolio-nav-and-cashflow-fx).

6. **Make assignment-driven credit explicit.** Retire the empty parallel
   curve list as a credit-factor marker. Both standalone engines reject
   assignment-driven factors. `FactorModel` computes market-mapped and
   assignment-owned exposures through their respective paths, then restores
   the declared factor order. The assignment owner retains issuer beta and
   quote-rebootstrap CS01. An unavailable assignment has a typed outcome.
   [Assignment-driven credit](../portfolio-attribution/12-factor-assignment-and-credit-exposures.md).

7. **Carry shock and routing provenance through existing outputs.** Store
   ordered factor axes, actual units and magnitudes, resolved sources,
   effective dates, reporting basis and routing evidence. One Rust contract
   validates existing Python, WASM and UI paths. Covariance consumers check
   the factor basis and IDs. Record 013 owns the central difference, profile
   grid, endpoint controls and numerical qualification.
   [Factor deltas and profiles](13-factor-deltas-and-pnl-profiles.md).

## Independent calculation checks

- Bucket centers `[1, 3, 7]` with weights `[1, −1, .5]` at `20 bp` give
  zero shift at source coordinate 2 and `−5 bp` at coordinate 5.
- One absolute Black vol point changes sigma `.20` to `.21`. A relative
  increase of one percent gives `.202`. An admitted percent near zero may
  round away even when its input passes an epsilon threshold.
- Independently supplied rates `2` and `.6` in opposite directions become
  `2.2` and `6/11` after a positive ten percent bump of the first direction.
- With EUR 100 and an outright of `1.20 USD/EUR`, moving the USD bridge
  factor from `.98` to `.97` while holding the EUR factor at `.99` changes
  the USD mark by `−40/33`. The outright is unchanged.

## Observed choice and rationale at the audit revision

A factor definition describes a market transformation; its declared factor type supplies the bump unit while actual market storage determines which instruments can be skipped. These are separate decisions. A rate-like label does not prove that an ID resolves to a discount curve, and dependency optimization must follow the object the generic bump will actually change.

## Bump formulation and controls

Per-factor overrides precede `BumpSizeConfig` type defaults. Rates/credit/inflation/custom factors use basis points; equity/commodity/FX use percent; volatility uses percent interpreted as absolute volatility points. Defaults are one bp or one percent/vol point respectively. The engine rejects non-finite bump size or `|h| < f64::EPSILON`; negative nonzero sizes are accepted.

Mapping is explicit:

- Parallel curve mappings require exact agreement between factor and mapping units, then apply additive parallel bumps to each listed ID.
- Bucketed curves require rate-bp units. For each ordered tenor/weight pair, form a triangular key-rate bump of `h * weight`, taking the immediately preceding/following tenor as support boundaries. The adapter consumes that order; it does not fit a new basis or independently sort the grid.
- Equity spot mappings convert the bump to a standard fraction and apply multiplier `1+fraction` through the generic curve/surface/scalar/series resolver.
- FX mappings convert fraction to percent and bump the specified pair at `as_of`.
- Volatility mappings require unit agreement and apply additive parallel generic-ID bumps. The generic resolver's name-collision precedence remains relevant.

## Exact versus conservative routing

Resolve curve IDs to actual Discount, Forward, Hazard, Inflation or BaseCorrelation storage and create corresponding dependency keys. Price, BasisSpread and Parametric storage, missing curves and unresolvable classifications cause conservative all-position routing. For EquitySpot, only a scalar or series with no same-named curve/surface admits exact routing. VolShift requires an ordinary surface and no same-named curve; other volatility-source representations are not assumed supported by this mapping.

Instrument dependencies are extracted once and credit-index constituents resolved on the supplied market. Extraction/resolution failure includes that instrument for every nonempty mapping. An ambiguous mapping includes all positions. An exact empty mapping produces no affected positions. Any FX mapping intersects every declared FX dependency, including possible triangulation. The engines also reprice/reconvert all rows for FX mappings to capture reporting-currency translation even for instruments without native FX dependencies.

Free portfolio sensitivity/profile functions reject Credit factors with an empty parallel curve list because those mean assignment-driven hierarchy factors. The direct engine APIs do not apply that wrapper check and can return zero columns/profiles for empty mappings. Assignment-based CS01/beta semantics are owned by [the portfolio factor bridge](../portfolio-attribution/12-factor-assignment-and-credit-exposures.md).

## Limitations and alternatives

Exact routing relies on complete instrument declarations and core bump semantics; it is not an independent detection of all state read by a price. Missing market objects still generally fail when the bump is applied; conservative routing does not substitute absent data. Direct engines do not independently validate every factor configuration/axis invariant enforced by a higher-level `FactorModelConfig`.

A live source check found typed market actions for several mappings and a
separate router that still applies the earlier bare-ID precedence. The FX
reconversion guard keys off the FX mapping kind. The accepted contract
requires actual effect evidence, including the bridge discount sources.

A declared-type-only router would be simpler but can skip real exposures under misclassified definitions; it is not the observed algorithm. A universal full-reprice router is an implemented conservative outcome, trading work for correctness. The default bump size is a convention, not convergence evidence.

## Evidence

[traits.rs](../../../../finstack-quant/portfolio/src/sensitivity/traits.rs), [delta mapping](../../../../finstack-quant/portfolio/src/sensitivity/delta_engine.rs), [wrapper dispatch](../../../../finstack-quant/portfolio/src/sensitivity/compute.rs), [factor config](../../../../finstack-quant/factor/src/config.rs) and [factor type units](../../../../finstack-quant/factor/src/primitives/factor_types.rs). Inline tests locate actual-storage routing, missing-storage conservative behavior, FX triangulation and unit mismatch. Wrapper tests locate empty-curve credit rejection. These validate routing contracts and finite-shock construction, not economic calibration of factors.

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution).
