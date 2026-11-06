# CALIBRATION-FRAMEWORK-009: Cross-currency quote leg construction

- Decision status: **Draft (observed implementation; adoption pending)**.
- Implementation status: **Observed for the stated source scope**; discrepancies and limits are stated below.
- Validation status: **Evidence inspected/located, not executed**. No numerical qualification or external compliance certification is implied.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Observed choice and rationale

XCCY calibration constructs a two-currency swap with FX-equivalent starting notionals. Projection IDs come from convention index IDs, while discount IDs come from explicit build roles. The design preserves market conventions separately from the curve being solved.

## Ordered construction and units

Validate quote and resolve XCCY convention plus both rate-index conventions. A missing quote-level `spot_fx` fails construction; the target may materialize it from its step parameter before calling this builder. Spot is quote currency per one base unit. Calibration context notional is quote-currency amount, so base notional is `N_quote / spot_fx`.

FX spot date uses pair settlement days, both currencies and supplied base/quote calendars. Far tenor is added unadjusted to spot then adjusted on the resolved joint calendar; explicit far dates are adjusted there too. The base leg receives, carries the basis-spread bp, uses base-index projection and foreign-discount role. The quote leg pays, has zero spread, uses quote-index projection and domestic-discount role. Both use convention payment frequency/day count/BDC, ShortFront stub, EOM false, their currency-specific payment/reset lags and compounding resolved from the index. `fixing_calendar_id=None` leaves downstream fixing-calendar selection to valuation.

The swap reports in quote currency and applies the convention's notional-exchange mode, including MTM modes where specified. Prepared quote pillar is leg-one accrual end, with a target-supplied curve-clock year fraction; payment lags can extend cashflows beyond that pillar.

## Failure and alternatives

Unknown convention/calendar, invalid quote FX, missing roles and invalid leg construction fail. No guessed spot or forward-curve fallback exists in this builder. Alternatives include a direct covered-interest-parity/basis formula or independent per-leg contractual schedules; the former loses reset/exchange richness and the latter needs more explicit data.

## Source and validation evidence

- Implementation: [build/xccy.rs](../../../../finstack-quant/calibration/src/build/xccy.rs) — joint calendars, notional conversion, leg roles and exchange selection.
- Implementation: [build/prepared.rs](../../../../finstack-quant/calibration/src/build/prepared.rs) — XCCY accrual-end pillar.
- Implementation: [build/context.rs](../../../../finstack-quant/calibration/src/build/context.rs) — explicit discount-role lookup.
- Test evidence: [src/build/xccy/tests.rs](../../../../finstack-quant/calibration/src/build/xccy/tests.rs) — FX-equivalent notionals and G10 MTM reset exchange assertions; inspected or located, not executed.
- Reference gap: no method-specific independent source or external benchmark is established by this record; source and regression evidence describe implementation behavior.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
