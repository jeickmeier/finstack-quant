# VALUATIONS-FRAMEWORK-09 — Finite-difference conventions and combined, per-curve, and key-rate DV01

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in the source paths cited below.
- Validation status: Evidence inspected/located, not executed; no runtime checks or external accuracy comparison performed.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and controls

Shared risk resolves defaults from `valuations.sensitivities.v1`, then overlays instrument metric bump settings. Defaults are 1bp rate and credit shocks, 1% relative spot and 0.01 absolute volatility. Yield bump is absent unless supplied; product calculators then choose their own default. Bumps must be positive finite. Custom bucket grids must be nonempty, positive finite and strictly increasing. Standard coordinates are 0.25, 0.5, 1, 2, 3, 5, 7, 10, 15, 20 and 30 years. These are calculation controls, not market calibration evidence.

## Algorithms and conventions

The central derivative is `(PV_up−PV_down)/full_width`; finite-width helpers reject nonfinite or absolute width at most `1e−12`. Relative scalar delta uses `S(1±h)` and divides by `2Sh`, requiring positive `Sh`; money scalar replacements preserve their currency. Mixed derivatives use four corners divided by `4hk`. Volatility bumps follow the active quote source: scalar instrument override if present, otherwise the named surface. Surface nodes floor additive downward bumps at zero; callers use the minimum grid vol to detect a one-sided stencil requirement. A bumped scalar vol must remain strictly positive. Correlation01 uses absolute 0.01 as both default shock and reporting unit. Volga is reported per volatility point squared and therefore scales absolute-vol second derivatives by `0.01²`.

Unified DV01 takes declared discount/forward curve roles and selects all, forward-only or discount-only. No declared selected rates curves means zero risk. Every rate-risk reprice first allows the instrument to rebuild itself against the bumped market, then preserves the metric context's selected model dispatch. Three modes are implemented:

- Combined parallel: shock all selected curves together up/down and report `(PV_up−PV_down)/(2×bump_bp)`. Sign is PV change for a positive 1bp rate move; a conventional long fixed-rate bond usually has negative DV01.
- Per-curve parallel: shock each curve separately, store qualified curve results, and return their compensated sum. This need not equal a finite simultaneous shock exactly for nonlinear products.
- Triangular key rate (default configuration): shock each bucket using adjacent coordinates; first and last wings extend flat outward so weights partition unity. Store exact-coordinate labels and sum bucket values and curve totals with Neumaier compensation. A bucket sum can differ from finite parallel risk by truncation/nonlinearity; partitioning weights alone is not a numerical equality proof.

## Failure behavior

Curve collection records missing dependencies but errors only when **all** selected declared curves are missing. A partial missing set is omitted from bumping if at least one curve was found; later repricing may still error if it needs the absent curve. Scratch mutation is used internally and restored; no historical-risk recalibration is implied by a raw rate curve bump.

Display bucket labels round fractional years to months/years and can collide; calculation keys instead use exact year coordinates (`1.4y`) with parser validation. Do not substitute presentation labels in risk storage. Analytic/AAD sensitivities and adaptive error-controlled finite differences are meaningful alternatives; they are not universally selected by this framework.

## Evidence

- [finite_difference.rs](../../../../finstack-quant/valuations/core/src/metrics/finite_difference.rs), [config.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/config.rs), [dv01.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/dv01.rs), and [buckets.rs](../../../../finstack-quant/valuations/core/src/metrics/buckets.rs) contain formulas, precedence and tests. Tests include degenerate-width errors, exact key-rate label round trips and override precedence; this is behavioral/formula evidence, not a cross-vendor accuracy benchmark.
- [Hull](../../../REFERENCES.md#hull-options-futures) provides general sensitivity context.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
