# CALIBRATION-TARGETS-007 — Foreign discount calibration and derived zero-spread curves

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed for the stated source scope; behavior and scope limitations are described below.
- **Validation status:** Source and selected evidence inspected or located; not executed.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Choice and inputs

Bootstrap a foreign discount curve from rate quotes and/or convention-built XCCY basis swaps while holding an existing domestic discount curve fixed. The optional BasisSpreadCurve is a derived **continuous foreign-minus-domestic zero spread**, not the dealer's coupon basis quotes. Only Bootstrap is implemented; GlobalSolve errors.

## Algorithm and selection

1. Require domestic discount and positive schema FX spot. Extract both rate and XCCY quote classes; either nonempty class suffices, despite a source comment suggesting rate anchors are mandatory. Curve clock defaults ACT/365F, interpolation LogLinear and extrapolation FlatForward. Schema FX input is T+0 cash FX in domestic units per foreign unit; the target does not convert a later-settlement screen spot to cash FX.
2. Rate quotes use candidate foreign curve as both discount and projection at `N=1,000,000`, with no step OIS override. XCCY builders receive domestic/foreign discount roles; missing per-quote spot is filled from schema spot, while an explicit positive quote spot takes precedence. Pair conventions select fixed-notional versus MtM resetting legs and schedules; [framework XCCY construction](../calibration-framework/09-xccy-materialization.md) owns those contracts.
3. Merge prepared instruments by pillar time and bootstrap from `(0,1)`. Rate residual uses raw prepared calibration value; XCCY uses instrument raw PV, each divided by 1,000,000. Initial guess extrapolates the previous zero rate `exp(log(DFprev)*t/tprev)` when previous positive time exists; with the supplied origin seed the initial guess is 1. The no-previous-knot fallback queries domestic DF at numeric t. The target supplies no custom scan, so framework geometric scan and exhaustive bracketing apply.
4. Trial curve uses Raw validation allowing nonmonotone DFs without forward floor; accepted knots need `0<DF<=1.5` and finite values. Final build uses the **default DiscountCurve validation**, without the discount target's currency/nonmonotonicity override. The discount-specific success tolerance applies (default `1e-8` PV/N).
5. Optional spread output samples each quote's actual pillar date. Compute both relative DFs from calibration base date to that date, validate finite positivity, and set `b(t)=(-ln DF_foreign + ln DF_domestic)/t` on the foreign curve clock. Prepend `(0,0)`, use Linear interpolation and the requested extrapolation. Domestic curve may have its own base/day-count because date-relative discounting aligns the dates before dividing by the common t.

## Limits and alternatives

The trial/final shape policies differ: a negative-rate foreign curve can be numerically fitted yet fail final default validation. No currency-based relaxation or target rate bounds are used here, and the fixed 1.5 DF cap is separate from discount calibration bounds. Nonlocal requested interpolation receives a single bootstrap sweep, without discount target global polishing; final repricing can expose earlier-quote drift. The result carries no rate replay sidecar from this target.

Different quote spots can imply different notionals while market FX remains a separate context dependency. Curve zero-spread output combines all differences between calibrated foreign and domestic rates; it should not be interpreted as a pure funding basis without a separately agreed decomposition. Alternatives include direct zero-spread parameterization against a fixed foreign reference curve, coupled multi-currency solve, or benchmark-quote replay.

## Evidence

- Owning [xccy_basis.rs](../../../../finstack-quant/calibration/src/targets/xccy_basis.rs) and its quote-builder dependency were inspected.
- [XCCY integration tests](../../../../finstack-quant/calibration/tests/calibration/xccy_basis.rs) locate signed output, clock/base invariance, ordinary swap quotes and GlobalSolve rejection; inline quote acceptance uses the same production builder/pricer. These do not independently establish collateral/FX-reset market conventions.

Neither comments nor output type prove short-end anchors were supplied.

## Follow-up tracking

Adoption, reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
