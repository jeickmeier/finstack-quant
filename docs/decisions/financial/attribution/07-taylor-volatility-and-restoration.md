# ATTRIBUTION-007: Taylor volatility, restored families and cash theta

- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the source scope listed below.
- Validation status: Source evidence inspected; named test evidence located, not executed. No numerical accuracy certification.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and algorithm

The Taylor route is a hybrid: rates/credit/ordinary vol surfaces use sensitivity expansions at opening state; scalar volatility, FX, inflation, correlation, ordinary market scalars and model parameters use closing-state restore-and-reprice differences. Therefore it is not one uniform multivariate Taylor series.

For each declared volatility dependency that resolves to an ordinary surface in either snapshot, bump all surface volatility absolutely by ±h, default h=0.01 (one vol point), permitted finite range `[0.0001,0.20]`. Vega per vol point is `[Vup-Vdown]/(2*h*100)`. Observed move is measured at opening-date Act/365 expiry and dependency reference strike when both exist; otherwise use the surface-average helper and emit a note. Add vega × move and, when gamma enabled, `0.5*[Vup-2V0+Vdown]/(100h)² * move_points²`.

Scalar-vol quotes are separately restored to opening values in an otherwise closing volatility-family snapshot. This keeps closing surfaces/cubes in that restore, and measures scalar-vol pricing effect exactly. Ordinary surface expansion does not cover non-scalar SABR cubes, FX-delta surfaces or volatility-index curves merely because the restoration layer recognizes them; there is no general restored-volatility remainder here.

FX restores the opening matrix whenever either snapshot has a matrix, including a closing-only introduction (with a note). Inflation, correlations and market scalars restore full opening families whenever either snapshot contains family data. Each contribution is closing PV minus closing-date PV with that family restored. Model parameters similarly restore the opening snapshot, when a non-None snapshot exists. These full-family effects already include own-family nonlinearities; no additional gamma is applied.

Theta uses opening parameters/market at closing date and adds shared period economic cash. It is then routed through total-return carry with income cash but no accrued, flat-yield or funding detail; the remainder is roll-down. Carry collection failure sets invalid, leaves the successful theta absent, and notes that total excludes period cash receipts.

## Controls, limitations and failure

Most factor errors set invalid and add notes; the explicit FX catch adds a note but does not set invalid, a real method-specific asymmetry. Nonfinite aggregate factor amounts become zero sentinels with invalid status; sensitivity working can be omitted when component values are nonfinite. Default residual thresholds are 10 currency units and 5 percent. No mixed-factor cross terms are calculated. Repeated dependencies can generate repeated surface factors; dependency uniqueness is not enforced in this loop.

## Rationale and alternatives

The hybrid supplies full-family attribution where a scalar sensitivity contract is absent, but observation point and interaction conventions differ across factors. Parallel/waterfall are supported alternatives. Located tests cover reference-point/average selection, all surface dependencies, failed-factor flags and restored FX/scalar/inflation/correlation/model factors. They are implementation and synthetic response checks, not a general accuracy certification.

## Source and evidence

- [taylor.rs](../../../../finstack-quant/attribution/src/taylor.rs) — owning implementation.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). This record does not
authorize a runtime change or certify the observed method.
