# CALIBRATION-FRAMEWORK-007: CDS accrual, maturity, upfront and protection roles

- Decision status: **Draft (observed implementation; adoption pending)**.
- Implementation status: **Observed for the stated source scope**; discrepancies and limits are stated below.
- Validation status: **Evidence inspected/located, not executed**. No numerical qualification or external compliance certification is implied.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Observed choice and rationale

CDS quote construction separates trade date, coupon accrual anchor, cash settlement and protection-effective behavior. It builds a protection-buyer instrument with the actual quote form and convention rather than collapsing upfront to a flat spread.

## Inputs and stages

Validate quote and resolve currency/documentation-clause convention. Spot settlement is as-of plus convention business days, adjusted by its BDC/calendar. Accrual starts on trade date when it is a quarterly CDS date, otherwise on the preceding quarterly CDS date. For tenor pillars, take the preceding semiannual CDS roll, add tenor unadjusted, then choose the next semiannual CDS maturity. Explicit dates roll to the next quarterly CDS date after `date-1 day`, retaining an already-standard date.

Par-spread quotes use spread bp as premium coupon and no upfront cash. Upfront quotes retain fixed coupon bp and create cash at spot equal to `notional × upfront_pct`; `upfront_pct` is a signed fraction. The instrument is Pay premium/receive protection, uses CdsImm premium roll, registry frequency/stub/BDC/day count/calendar, explicit discount and credit roles, and quoted recovery. Protection settlement delay is the convention settlement-day count. Valuation convention uses the build override or its type default. `protection_effective_date=None` leaves that role to the CDS valuation policy, not to spot or accrual date. The entity is retained as metadata and the documentation clause is explicit.

Prepared CDS quotes cache premium-leg end as pillar and calculate its time from calibration base using the supplied hazard day count. The hazard target separately enforces entity, currency, recovery and recipe compatibility.

## Limits and alternatives

The conventional calendar and CDS pricer are dependencies, so a zero-PV quote replay is internal consistency evidence. Alternative maturity rules include direct contractual dates or bespoke nonstandard schedules; those require separate explicit contracts rather than silently bypassing rolls. A report label naming an ISDA version is not a conformance test or source pin.

## Source and validation evidence

- Implementation: [build/cds.rs](../../../../finstack-quant/calibration/src/build/cds.rs) — quarterly/semiannual date rules, upfront conversion, side and valuation override.
- Implementation: [build/prepared.rs](../../../../finstack-quant/calibration/src/build/prepared.rs) — CDS pillar cache.
- Implementation: [build/context.rs](../../../../finstack-quant/calibration/src/build/context.rs) — valuation-convention override.
- Test evidence: [src/build/tests_credit.rs](../../../../finstack-quant/calibration/src/build/tests_credit.rs) — quarterly roll sweep and distinction among accrual/settlement/protection dates; inspected or located, not executed.
- Test evidence: [src/build/cds.rs](../../../../finstack-quant/calibration/src/build/cds.rs) — tenor and explicit-date roll tests; inspected or located, not executed.
- Test evidence: [src/build/tests_quote_construction.rs](../../../../finstack-quant/calibration/src/build/tests_quote_construction.rs) — par/upfront construction coverage; inspected or located, not executed.
- Reference context: [ISDA CDS standard model](../../../REFERENCES.md#isda-cds-standard-model) — reference family; this audit does not validate any named model release; a catalogue entry is not proof of implementation equivalence.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
