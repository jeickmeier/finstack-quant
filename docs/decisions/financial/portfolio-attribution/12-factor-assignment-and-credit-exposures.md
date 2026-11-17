# PORTFOLIO-ATTRIBUTION-012: Factor assignment and assignment-driven credit exposures

- ID: PORTFOLIO-ATTRIBUTION-012
- Decision status: Accepted 2026-11-16 for checked factor assignment and assignment-driven credit exposures.
- Implementation status: Observed for the stated owning source scope. Runtime, replay and host reconciliation with the accepted contract are pending.
- Validation status: Independent exact checks covered signed source exposures, issuer-specific loading and the difference between local loadings and finite stress. Production replay and host qualification remain pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked factor assignment and credit exposures

1. Prepare one Rust request for assignment, sensitivity and analysis. Bind the immutable portfolio and market, effective valuation date, selected model, pricer, provider, settings and reporting FX basis. Match the dependencies the selected pricer uses, preserving source namespace, role and exact ID, including ordered FX pairs and reporting-only FX dependencies. For credit indices, preserve aggregate hazard, active named issuers and their bound hazard sources, and any consumed base correlation as distinct roles. Resolve direct issuers from qualified instrument metadata and index constituents from exact bound membership. An aggregate source requires an explicit aggregate loading. Reprice the full product on its own pool weights and effective exposure. [Dependency selection](../portfolio-execution/05-dependencies-and-selective-reuse.md#accepted-decision-for-checked-portfolio-dependencies-and-selective-repricing) · [CDS and index risk](../models-and-valuations-credit/13-cds-and-index-risk.md#accepted-decision-for-checked-cds-and-index-risk).
2. Give each factor one computation route. Market-mapped factors use their declared shock engine. Assignment-driven credit factors use the issuer-loading path and standalone engines reject them. A descriptive match alone carries no numerical weight. Require declared factor IDs and finite signed betas. Retain every qualified path to a shared quote source. Apply its full holding exposure once per holding and factor. Conflicting betas for a shared source and factor fail admission. [Factor routing](../portfolio-execution/12-factor-shocks-and-routing.md#accepted-decision-for-checked-portfolio-factor-shocks-and-routing).
3. Keep Strict as the default. A required unmatched source fails in Strict mode. Warn can return a structured incomplete assignment report. Sensitivity and covariance publication require complete requested coverage. Distinguish out-of-scope sources, proven structural zeros and genuinely unmatched sources. Invalid matcher output, missing required data, failed replay and failed pricing remain errors in either mode. Any proxy loading requires explicit authorization and provenance.
4. Require an explicit positive finite quote bump `h` in bp for each source. Qualify a common par-risk center, replay both signs on the exact bound quote set, and retain original observations and represented moves. Use fallible atomic endpoint construction and rebind affected index views. Hold cash terms and non-target inputs fixed. Reprice with the selected evaluator and convert unscaled raw PV with qualified cash FX. Compute `e = w × (V+ − V−)/(2h)` in reporting currency per bp with checked arithmetic and signed holding scale `w`. Both endpoints must succeed. Reject subresolution moves. Remove the epsilon floor and any clipping, one-sided substitution or native hazard fallback. [Hazard quote replay](../calibration-framework/21-hazard-quote-replay.md#accepted-decision-for-qualified-hazard-quote-replay-and-publication) · [Factor secant arithmetic](../portfolio-execution/13-factor-deltas-and-pnl-profiles.md#accepted-decision-for-checked-portfolio-factor-deltas-and-profiles).
5. Compute the first-order factor loading as `Σ β × e` with checked accumulation in declared factor order. Its unit is reporting currency per bp of factor spread. Deliver each qualified pre-beta source exposure once to [record 013](13-credit-residual-risk-and-reporting.md) for residual risk. [Record 014](14-factor-stress-and-position-what-if.md) owns joint finite stress repricing, whose P&L can differ when valuation is nonlinear.
6. Keep replay and pre-beta exposure caches request-local. Reuse requires the exact admitted source, delivered quote action, market snapshots, selected pricing state, effective date, cash FX basis and any required stochastic coupling. Bind the holding scale when caching a scaled exposure. Publish ordered axes, coverage, currency and units, assignment and endpoint provenance through one checked Rust result and restoration contract. Existing Python and WASM paths consume that contract. [Request-local replay identity](../calibration-framework/23-recalibration-cache.md#accepted-decision-for-exact-request-local-recalibration-caching).

Independent exact arithmetic gave source exposures of `+4` and `+2 USD/bp` and a factor loading of `+5.5 USD/bp` with betas `+1.5` and `−0.25`. A nonlinear price example gave different results for a beta-weighted source secant and a joint finite factor reprice.

## Observed choice and dependency boundary

`FactorModel::from_config` validates the factor crate's configuration, builds its matcher and selects the portfolio DeltaBased sensitivity engine with supplied/default bump sizes. Factor definitions and [covariance alignment](../factor/matrix-invariants.md), [dependency matching](../factor/dependency-matching.md), [credit matching](../factor/credit-matching.md) and risk-measure kernels belong to the [factor scope](../factor/README.md). This record owns portfolio dependency adaptation and the additional credit-hierarchy exposure calculation.

## Ordered stages

1. Convert each instrument's declared discount, forward, credit and inflation curves, scalars, volatility surfaces, FX pairs and series to factor dependencies. Expand credit-index identities through portfolio market dependencies into hazard and base-correlation references, deduplicating those additions. Missing credit-index market data errors.
2. Match dependencies using **instrument attributes**. Preserve every returned `(dependency,factor,beta)` entry. Empty/no match produces an unmatched row; matcher errors propagate. Strict policy fails, Warn logs, and permissive policy retains the unmatched report. The default comes from the validated factor configuration.
3. Run the portfolio sensitivity engine on position rows, factor definitions, market, date and base currency. Assignment-driven credit factors receive a separate overlay because their affected curves depend on issuer matching rather than a static curve list.
4. For each matched credit curve/factor, require a declared factor definition. Missing declared IDs fail under Strict, log and drop under Warn, or drop permissively. Obtain the factor bump in basis points. Require finite position scale and finite nonzero bump (absolute size at least machine epsilon).
5. Require a lossless hazard calibration recipe, recover its discount-curve role, and bump/rebootstrap market spread quotes by `+h` and `-h` basis points. There is no direct hazard-shift fallback. Price raw endpoints in portfolio base currency; require finite endpoints and `e=(PV_up-PV_down)/(2*h)*position_scale` finite.
6. Add `beta*e` to that position/factor delta, checking the weighted result is finite. Its units are base-currency P&L per basis-point factor move, with beta dimensionless.
7. Cache bumped contexts by `(curve ID,bump bits)` and exposures by `(position index,curve ID,bump bits)` inside one analysis call. The base market, date, position scale and instrument are fixed by that call; caches are not reusable public cross-date stores.

## Alternatives and tradeoffs

Analytic CS01, direct hazard shifts and frozen-survival approximations differ from spread-quote recalibration. The selected recipe replay preserves the owning calibration contract at additional pricing cost. Noncredit assignment reports do not by themselves imply beta-weighted sensitivity; the selected engine's factor mapping remains the owner of those calculations.

## Evidence

[Model orchestration and inline tests](../../../../finstack-quant/portfolio/src/factor_model/model.rs), [assignment](../../../../finstack-quant/portfolio/src/factor_model/assignment.rs), [dependency adapter](../../../../finstack-quant/portfolio/src/factor_model/dependencies.rs) and [engine integration tests](../../../../finstack-quant/portfolio/tests/factor_model_engines.rs) establish routing, beta scaling, reporting currency and cache reuse. [Hazard recalibration owner](../../../../finstack-quant/calibration/src/recalibration/mod.rs) is linked as a dependency, not a second audited kernel. Tests were not run.

Instrument-tag matching is distinct from position-tag optimization filters.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution). This record does not
authorize a runtime change or certify the observed method.
