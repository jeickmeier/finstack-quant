# VALUATIONS-FRAMEWORK-07 — Variance sampling and barrier monitoring grids

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in the source paths cited below.
- Validation status: Evidence inspected/located, not executed; no runtime checks or external accuracy comparison performed.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope, and rationale

The common observation adapters distinguish contractual sample counts from elapsed calendar time and require simulation grids to contain barrier observation dates. Variance payoff estimators and barrier crossing kernels are implemented in core/models, outside these adapters. This record covers the date-to-sample/time mapping that materially changes those results.

## Algorithm and selection

`variance_observation_dates` resolves a strict single calendar or joint base/quote calendar. Daily tenors mean business-day steps: adjust start/end, walk the calendar by tenor count and explicitly append adjusted maturity if the last step missed it. Weekly/monthly/yearly dates come from the core schedule builder with Short Back stub and caller-selected end-of-month, then each is business-day adjusted. Sort and deduplicate all adjusted dates. Zero tenor is rejected. The daily branch does not independently reject reversed start/maturity before appending its endpoint; callers must establish the date range.

Annualization is `12/month_count` for month/year tenors, `52/week_count` for weeks, and `trading_days_per_year/day_count` for days. It is contractual frequency annualization, not measured spacing after holidays. `realized_sample_fraction` counts observations on `as_of` as realized. Close-to-close and Yang–Zhang subtract one anchor from both realized and total observations; Parkinson, Garman–Klass and Rogers–Satchell count bars. Empty schedule gives zero; as-of at/after final observation gives one before checking sample count. Otherwise a zero sample denominator fails. Thus a one-date close-to-close schedule can return one at maturity despite having no return interval.

Barrier `Monitoring` defaults to Continuous. Continuous monitoring adds the optional future start time, or time zero if monitoring already started, as a required simulation knot. Discrete monitoring converts dates on/after as-of to times using the instrument day count. `PathDependentPricerConfig::build_time_grid` inserts required times into the simulation grid; each is found with tolerance `1e−12×max(|t|,1)`, otherwise internal error. The adapter then supplies continuous start-step or explicit discrete observation-step indices to the model payoff. It does not reconstruct prior barrier breaches.

## Limitations and alternatives

Calendar-day daily sampling, actual-spacing annualization and time-weighted seasoned variance are different methods, not selected alternatives here. Barrier continuous monitoring accuracy depends on the chosen process/payoff crossing method and simulation grid; the enum alone is not evidence of exact continuous monitoring.

## Evidence

- [variance_observations.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/variance_observations.rs): algorithms and inspected inline tests `day_tenor_uses_holiday_calendar_and_business_endpoints`, `monthly_schedule_preserves_end_of_month_anchor`, and unknown-calendar rejection.
- [monitoring.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/parameters/monitoring.rs): required-knot and observation-step mapping; core/models own grid construction and crossing probabilities.
- [Demeterfi et al.](../../../REFERENCES.md#demeterfi-1999-volatility-swaps), [Broadie–Glasserman–Kou](../../../REFERENCES.md#broadie-glasserman-kou-1997), and [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) provide canonical conceptual context, not direct acceptance of these calendar rules.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
