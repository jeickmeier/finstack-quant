# CALIBRATION-FRAMEWORK-008: Index tranche notional and schedule normalization

- Decision status: **Draft (observed implementation; adoption pending)**.
- Implementation status: **Observed for the stated source scope**; discrepancies and limits are stated below.
- Validation status: **Evidence inspected/located, not executed**. No numerical qualification or external compliance certification is implied.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Observed choice and rationale

The build context notional is a pool-scale amount. Construction converts it to tranche notional and retains separate quote conventions and schedule overrides. This avoids treating the same upfront fraction as a pool-notional payment.

## Algorithm and conventions

Validate attachment/detachment and quote fields; resolve CDS registry conventions and spot. Set `N_tranche = N_context × (detachment − attachment)`. Inputs attachment/detachment are fractions; `CdsTrancheParams` receives percentages by multiplying both by 100. Upfront, when nonzero, is `N_tranche × upfront_pct` at spot; zero upfront is represented by absence. Coupon is bp, realized loss is initialized to zero, and the side is Pay premium/receive protection.

CdsImm mode anchors effective date at the quarterly CDS accrual start and maturity at the next standard quarterly date (preserving an already-standard date). None mode uses spot effective date and maturity adjusted using the registry calendar/BDC. Other roll rules fail. Schedule frequency/day count/calendar/BDC can override the registry; stub remains registry-supplied, EOM false, payment lag zero and accrual-date adjustment false. Build with explicit discount/credit IDs, then assign effective date and upfront.

`CdsTrancheBuildOverrides::default` selects CdsImm, while envelope base-correlation params use `RollRule::default`, which is **None**; actual caller values therefore select different effective/maturity rules. Target-specific loss-model and base-correlation solving are covered in [calibration targets](../calibration-targets/README.md).

## Limits and alternatives

The initial spot and None-mode maturity use registry dates before schedule overrides, so a custom calendar/BDC does not uniformly govern every contractual date. This is a confirmed precedence distinction; material numerical impact requires a holiday-boundary fixture. Alternatives are to apply the override to all date roles or distinguish settlement and schedule calendars explicitly. Internal width/upfront assertions do not independently validate the loss model or accrued premium.

## Source and validation evidence

- Implementation: [build/cds_tranche.rs](../../../../finstack-quant/calibration/src/build/cds_tranche.rs) — width scaling, fraction/percent conversion and schedule override precedence.
- Implementation: [api/schema.rs](../../../../finstack-quant/calibration/src/api/schema.rs) — base-correlation schedule parameters.
- Test evidence: [src/build/cds_tranche.rs](../../../../finstack-quant/calibration/src/build/cds_tranche.rs) — notional normalization, roll and override regressions located; inspected or located, not executed.
- Reference context: [ISDA 2014 credit definitions](../../../REFERENCES.md#isda-2014-credit-definitions) — contract convention context; a catalogue entry is not proof of implementation equivalence.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
