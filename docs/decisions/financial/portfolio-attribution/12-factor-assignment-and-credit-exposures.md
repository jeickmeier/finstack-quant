# Factor assignment and assignment-driven credit exposures

- ID: PORTFOLIO-ATTRIBUTION-012
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the stated owning source scope; discrepancies are named below.
- Validation status: Evidence inspected/located, not executed; independent accuracy qualification remains open.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and dependency boundary

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
