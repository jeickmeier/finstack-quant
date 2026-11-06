# VALUATIONS-FRAMEWORK-10 — Quote-space CS01, direct-hazard and z-spread alternatives, and recovery risk

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in the source paths cited below.
- Validation status: Evidence inspected/located, not executed; no runtime checks or external accuracy comparison performed.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and selection

There are three economically distinct credit-risk methods. Generic CS01 requires a lossless hazard calibration recipe and a recalibration provider. Bond-like `ZSpreadCs01` registrations can explicitly remain z-spread-only or delegate when a credit dependency exists: replayable hazard selects quote-space CS01, while an unreplayable hazard selects the direct-hazard approximation. This selection is observed behavior, not approval to treat the measures as interchangeable.

## Algorithms and conventions

**Quote-space CS01.** Rebuild the hazard curve with every calibration spread quote shifted by ±`b` bp, keep the requested discount curve/conventions and any deal quote override, reprice under the selected dispatch, and report `(PV_up−PV_down)/(2b)`. Parallel `b` must exceed `1e−10` bp and be finite. Bucketed CS01 uses exact source calibration quote indices exposed by the provider, not the generic triangular tenors. Rebuild once per up/down quote, store provider labels, and sum using compensation. Generic curve resolution chooses the **first** declared credit and discount curves; it does not aggregate all credit curves. A missing required curve/recipe/provider fails.

**Direct-hazard approximation.** Set hazard bump `b_h=b/(1−R)` when LGD lies in `(1e−6,1]`, otherwise `b_h=b`. Bump hazard knots in parallel or configured triangular buckets and normalize back to spread bp. This uses the approximation `spread≈hazard×LGD`, with recovery fixed; no calibration quotes are held. If a down-hazard bump fails, use the up/base one-sided derivative. The fallback catches any down-bump error, not only a detected nonnegativity boundary.

**Z-spread CS01.** Cache only cashflows strictly after settlement as `(time, settlement-to-payment DF, amount)`, with time on the discount curve's day count. Apply the core periodic-compounding z-spread discount factor and compensated sum. Anchor priority is an explicit z-spread quote, a spread solved to the supplied dirty quote, then zero. The Brent anchor solve starts at zero with ±0.05 bracket size, tolerance `1e−10`, and verifies reproduced price within `1e−8×max(|target|,1)`; pricing failures inside the objective return `1e12`. Parallel risk uses `z±b/10000` and is keyed by instrument ID. Buckets assign each cashflow to the first tenor at/after its time, with all remaining cash in the final bucket. These are disjoint cashflow bands, **not triangular curve shocks**. Empty future cash yields zero.

**Recovery01.** Shock recovery ±0.01, clamp each to `[0,0.999]`, divide by the actual available width, and report PV change per +0.01. Use an up/base or base/down difference if the opposite distance is at most `1e−6`; return zero if neither side moves. A replayable hazard is recalibrated with changed recovery; any replay failure errors rather than reverting to frozen hazard. An unreplayable curve instead keeps hazard knots fixed and updates its recovery metadata. An explicit trade recovery is shifted alongside curve recovery, preserving their base offset subject to individual clamps.

The separate risk-free-component DV01 interface asks an instrument to shift only its internal risk-free discount component by ±`b/10000`; the framework returns the same signed PV-per-bp central difference. Product implementations define that decomposition.

## Limitations

- The common `sensitivity_central_diff` helper returns zero for `|b|≤1e−10` in release builds and debug-asserts in debug builds, unlike the guarded generic width helper. Some callers only enforce strictly positive `b`.
- Z-spread bucket labels use rounded display labels. Distinct custom fractional-year buckets can therefore collide when stored as qualified metrics, unlike exact DV01 labels.
- Provider replay is an execution dependency; this record does not certify the external calibration algorithm.

## Evidence and alternatives

[cs01.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/cs01.rs), [cs01_direct_hazard.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/cs01_direct_hazard.rs), [cs01_z_spread.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/cs01_z_spread.rs), [recovery01.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/recovery01.rs), [rf_component_dv01.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/rf_component_dv01.rs), and [recalibration.rs](../../../../finstack-quant/valuations/core/src/recalibration.rs) provide the formulas, delegation and replay contract. Inline tests cover behavior and formula reconciliation; none was run. [Duffie–Singleton](../../../REFERENCES.md#duffie-singleton-1999) provides general intensity/recovery context. Independently calibrated quote-bump benchmarks, direct-hazard approximation error, boundary stencils and key-label uniqueness remain adoption evidence gaps.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
