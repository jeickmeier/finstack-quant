# STATEMENTS-ANALYTICS-CREDIT-17: Lifetime CECL with forecast-to-history hazard reversion

- Decision status: **Draft — observed implementation; adoption pending**.
- Implementation status: **Observed** for the stated source scope; discrepancies are identified below.
- Validation status: **Evidence inspected or located, not executed**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

The PD/LGD/EAD CECL path uses lifetime survival-weighted conditional default probabilities, with a forecast window followed by immediate or linearly blended hazard reversion to historical annual PD. It also has a credit-impaired current-exposure-less-recovery branch. The implementation's model and defaults are observed choices, not an ASC 326 compliance determination.

## Scope and input domain

Validated exposure; nonempty validated macro-scenario/PD-source pairs; finite bucket width >= .0001 years; nonnegative forecast horizon and recovery delay; historical annual PD in [0,1]. Linear reversion requires positive finite duration. WARM-specific loss rate must be absent in this methodology.

## Algorithm

1. At engine creation validate config, scenario weights and optional LGD overrides. Measurement validates exposure and source lookup at t=0 for every scenario using current rating/NR, even if the horizon is wholly historical or the exposure is impaired.
2. Credit impairment means DPD >= configured threshold or enabled default-evidence flags. Impaired allowance is weighted `EAD0−(1−scenario LGD)*EAD0*DF(recovery_delay)`; scenario LGD overrides exposure LGD. Discount is `(1+eir)^−t` when enabled, otherwise one. Return recovery delay as horizon.
3. Otherwise bucket the entire remaining maturity, at least one bucket. Within forecast horizon use the source conditional marginal PD. A bucket straddling forecast end is split there and its two survival factors multiplied; this split controls default probability, while EAD/discount still use the original whole-bucket midpoint.
4. Historical hazard is `lambda_hist=−ln(1−annual_PD)`, with annual PD=1 represented as infinite hazard. Immediate reversion gives `1−exp(−lambda_hist*dt)`.
5. Linear reversion evaluates blend fraction `(midpoint−forecast_end)/reversion_years`, clipped to [0,1]. If the segment midpoint is at/after reversion end use historical hazard; otherwise blend forecast-boundary and historical hazards and convert that blended hazard to conditional default probability over the segment. It does not split a segment at reversion end. A reverted segment of width <= machine epsilon returns zero default probability.
6. Forecast-boundary hazard is estimated from the source's conditional PD over `[max(0,forecast_end−segment_width),forecast_end]`; with a zero forecast window it uses `[0,segment_width]`. Thus the calibration interval depends on the current segment's width, including a partial final bucket or the post-forecast piece of a split bucket. The source's boundary response is transformed without an independent [0,1] check: values below one use `−ln(1−mpd)/dt`; all other values, including NaN, take the infinite-hazard branch.
7. Starting from survival one, compute unconditional mass = prior survival × final composed conditional PD, then update survival × (1−conditional PD), each floored at zero. The engine checks the final composed conditional PD is in [0,1]; it does not validate every source marginal response before hazard conversion or the straddling-bucket clamp. Accumulate mass × scenario LGD × midpoint EAD × optional midpoint DF, then weight scenarios. Reject a non-finite final allowance.

## Selection and controls

Default forecast horizon is two years, historical PD .02, immediate reversion, .25-year buckets, impairment threshold 90 DPD, default evidence enabled, one-year recovery and discounting enabled. There is no IFRS stage classification or TTC/downturn config in this CECL path.

## Conventions

Historical PD is a one-year default probability converted to continuous hazard; linearity applies to hazard, not PD. Forecast/reversion times are reporting-date years. Every scenario uses its own boundary hazard and shared historical PD/config. Output identifies methodology and horizon but does not retain bucket or scenario decomposition.

## Limitations, failure and fallback

The forecast boundary hazard changes with grid width/partial segments, rather than being a fixed derivative or fixed-window estimate. The midpoint treatment can straddle the end of linear reversion without exact piecewise integration. These are confirmed static approximation choices; their numerical magnitude was not executed. The default trait marginal method validates cumulative endpoints, but a custom override can bypass that check: NaN or >1 boundary responses become infinite hazard, and a composed straddling probability is clamped before final validation. Invalid custom-source probabilities can therefore be transformed into an apparently valid engine probability. Extreme unit PDs use infinite-hazard arithmetic, requiring boundary qualification. Cumulative sources with nonzero initial PD are validated in range but not forced to F(0)=0 by this generic trait path; performing survival starts at one. No per-scenario audit breakdown is returned.

## Alternatives and tradeoffs

A forecast-boundary hazard fixed independently of the grid, with exact piecewise-linear integration split at both regime boundaries, would separate model choice from quadrature. Reversion in cumulative PD or annual loss rates would be different models. Undiscounted losses are supported by config but require a documented accounting methodology choice.

## Evidence

[cecl.rs](../../../../finstack-quant/statements-analytics/src/analysis/ecl/cecl.rs) owns the engine. Inspected local `test_cecl_immediate_reversion` and `test_cecl_linear_reversion` assert only positive ECL, so they are branch smoke evidence, not reversion-accuracy or grid-invariance oracles. Located tests cover lifetime horizon, weight errors, LGD override and comparison to IFRS Stage 2 under aligned assumptions; [analysis_ecl.rs](../../../../finstack-quant/statements-analytics/tests/analysis_ecl.rs) provides additional integration cases. [ASC 326](../../../REFERENCES.md#asc-326-cecl) names the accounting framework, without certifying the numerical policy.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
