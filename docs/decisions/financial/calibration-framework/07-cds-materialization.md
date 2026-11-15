# CALIBRATION-FRAMEWORK-007: CDS accrual, maturity, upfront and protection roles

- Decision status: **Accepted 2026-11-15 — complete single-name CDS terms, dated settlement, quote basis, hazard pillar and replay package.** The named contractual date regimes (O7) were accepted on 2026-11-12.
- Implementation status: **Observed.** The live builder implements O7's single-name tenor and literal-date maturity rules. Reconciliation to the complete accepted package is pending.
- Validation status: **Independent calendar, quote conversion and fit-residual checks completed.** Production pricing, calibration, external-grid, replay and host qualification remain pending.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Accepted decision for single-name CDS calibration and quote settlement

1. **Resolve exact single-name terms and sources.** Require the reference
   entity, currency, governing definitions, transaction type and exact
   restructuring election from a qualified dated convention or complete
   bespoke terms. Require positive exact notional in the contract currency,
   the exact typed discount and issuer hazard roles, and coherent
   source identity and currency. Require exact represented recovery
   agreement among quote, contract, target and source, with `0 ≤ R < 1`.
   Use the accepted `HazardRate` pricing path. Remove regional or currency
   fallbacks and vendor-named valuation defaults.
   [Accepted convention selection](../valuations-framework/convention-selection.md#accepted-decision-for-checked-convention-selection-and-dated-sources) ·
   [Accepted CDS legs and quote basis](../models-and-valuations-credit/12-cds-legs.md#accepted-decision-for-checked-cds-legs-and-quote-basis).

2. **Resolve accrual, modeled protection and cash settlement separately.**
   Preserve the [accepted O7 maturity rules](#accepted-decision-named-cds-contractual-date-regimes).
   In the qualified standard issuance profile, modeled step-in is
   calendar `T+1`. The first accrual anchor is the latest
   Following-adjusted quarterly coupon date on or before step-in.
   The first regular coupon is the first such date strictly afterward.
   Use unadjusted maturity `M` as the last covered day, `M+1` exclusive as
   the final civil accrual boundary, and Following from `M` for final
   payment. Keep the valuation-close cut, upfront settlement and
   contingent default cash distinct. Default settlement is explicitly
   contemporaneous or a positive business-day lag from the default day
   on its own calendar. Bespoke terms supply their own schedule,
   day-count context, dates, and settlement and protection elections
   under the accepted CDS model.
   [Standard contract specification](https://www.cdsmodel.com/assets/cds-model/docs/Standard%20Corporate%20CDS%20Contract%20Specification%20-%20May%2012%2C%202009.pdf).

3. **Require a basis for running par.** On the protection-buyer axis,
   let `P` be protection PV per unit notional, `c` be the observed coupon
   in annual decimal units and `A_dirty` include the scheduled and
   accrual-on-default annuities. Fit `N × (P − cA)`. Dirty par uses
   `A = A_dirty`. Clean par uses `A = A_dirty − Bα`, where `B` is the
   factor to the named quote settlement date and `α` is accrued through
   the quote's valuation close under the resolved coupon clock.
   When represented as dated economic cash, zero clean upfront gives
   buyer settlement cash `−Ncα`. Use the rebate once. Require positive
   observed par spreads in this calibration profile. Zero model-implied
   par remains a valid pricing result. A conventional spread requires
   a separately qualified converter.
   [ISDA converter specification](https://www.cdsmodel.com/assets/cds-model/docs/ISDA%20Standard%20CDS%20Contract%20Converter%20Specification%20-%20Sept%204%2C%202009.pdf).

4. **Admit upfront with an explicit representation.** For a fixed
   running coupon in bp, convert it once to decimal `c = bp/10000`.
   Enforce a fixed-coupon restriction only when the selected contract
   requires it. At named settlement, a signed clean upfront fraction
   `u` creates actual buyer cash `C = N × (u − cα)`. A signed
   actual-cash fraction `q` creates `C = N × q` directly. Positive
   cash is paid by the buyer, negative is received, and zero is valid.
   Use one checked Money boundary for settlement. Fit dirty raw PV
   less the eligible dated cash PV `BC` once. Require explicit
   entitlement for same-day settlement, at factor one.
   [ISDA fee settings](https://cdsmodel.com/fee-computations.html).

5. **Prepare the literal hazard pillar and raw fit values.** Use
   contractual maturity `M` on the target's explicit base, clock and
   context. Require a positive finite pillar time, live protection,
   and genuine dependence on admissible trial hazard. Keep rolled
   payment dates and each evaluator's dated clock for cash PV.
   An earned but unpaid coupon may retain economic PV after protection
   expiry. Reuse the common CDS legs for signed native raw PV before
   final PV Money conversion and divide by positive notional for the
   hazard residual. The hazard target owns solver geometry, bounds and
   fit acceptance.
   [Accepted hazard calibration](../calibration-targets/05-hazard-calibration.md#accepted-decision-for-explicit-single-name-hazard-calibration-and-par-risk-qualification).

6. **Freeze each prepared observation through fit and replay.** Preserve
   exact quote identity, original form and units, resolved dates and
   calendars, elections, cut, recovery and typed curve roles. Freeze
   the qualified terms and nontrial inputs across reprices. Bind any
   derived par-risk quote to its exact original observation. Reject
   incomplete or mismatched replay. Rust owns admission, valuation and
   restoration across Rust, Python, WASM and schemas.
   [Accepted calibration provenance](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification).

## Independent calculation checks

- A March 19, 2026 trade on a weekends-only calendar steps in and starts
  accrual March 20. With separately selected lags, a March 20 default
  pays March 23 and T+3 upfront settles March 24. The first regular
  coupon pays June 22. The 1Y contractual maturity is December 20,
  with both final exclusive accrual end and final payment on December 21.
- For a 10 million notional, 500 bp coupon, accrued fraction 0.025,
  dirty annuity 0.16, protection PV 0.00576125 per unit, and settlement
  factor 0.995, clean upfront −0.10% gives signed cash −22,500 and
  zero raw fit residual. Using the clean quote directly as a cash
  fraction yields a residual of −12,437.50.
- With a June 1, 2026 target base, literal June 20 maturity and rolled
  June 23 payment in a fixture with a June 22 holiday, the ACT/365F
  hazard pillar is `19/365` and the payment coordinate is `22/365`.

## Observed choice and rationale at the audit revision

CDS quote construction separates trade date, coupon accrual anchor, cash settlement and protection-effective behavior. It builds a protection-buyer instrument with the actual quote form and convention rather than collapsing upfront to a flat spread.

## Inputs and stages

Validate quote and resolve currency/documentation-clause convention. Spot settlement is as-of plus convention business days, adjusted by its BDC/calendar. Accrual starts on trade date when it is a quarterly CDS date, otherwise on the preceding quarterly CDS date. For tenor pillars, take the preceding semiannual CDS roll, add tenor unadjusted, then choose the next semiannual CDS maturity. Explicit dates roll to the next quarterly CDS date after `date-1 day`, retaining an already-standard date.

Par-spread quotes use spread bp as premium coupon and no upfront cash. Upfront quotes retain fixed coupon bp and create cash at spot equal to `notional × upfront_pct`; `upfront_pct` is a signed fraction. The instrument is Pay premium/receive protection, uses CdsImm premium roll, registry frequency/stub/BDC/day count/calendar, explicit discount and credit roles, and quoted recovery. Protection settlement delay is the convention settlement-day count. Valuation convention uses the build override or its type default. `protection_effective_date=None` leaves that role to the CDS valuation policy, not to spot or accrual date. The entity is retained as metadata and the documentation clause is explicit.

Prepared CDS quotes cache premium-leg end as pillar and calculate its time from calibration base using the supplied hazard day count. The hazard target separately enforces entity, currency, recovery and recipe compatibility.

## Accepted decision: named CDS contractual date regimes

Accepted on **2026-11-12** as owner resolution O7 of the core-conventions
reconciliation (closing the register's CDS product-boundary Decision entry).
Runtime implementation and independent qualification are tracked in the
register.

1. **No generic CDS grid.** The cashflows roll grid `RollRule::CdsImm` and
   its frequency/stub overrides are removed. A CDS product (single name,
   index, tranche, option underlying) and its calibration builder select a
   named contractual date regime explicitly; nothing falls back to a
   regime, a valuation date or a core date helper.
2. **The product resolves five distinct dates.** Premium accrual start,
   protection start (step-in), upfront cash settlement, maturity and
   default payment are separate roles. The selected regime owns how each is
   derived, the stub selection and the endpoint inclusion; one date never
   stands in for another.
3. **Post-2015 single-name quoting is a named regime.** A tenor quote under
   that regime matures on the latest March/September 20 roll on or before
   the trade date, plus three months, plus the tenor. It is qualified
   against the official December 2015 dated table: a 2015-12-22 trade gives
   2016-03-20 for 3M and 2016-09-20 for 9M, and the whole-year points keep
   their June/December 20 maturities. A maturity on or before the trade
   date is never accepted for a positive tenor. Tenors that the regime and
   its fixtures do not determine are errors.
4. **Literal maturities stay literal.** An explicit contractual maturity
   date is used as given, without rolling to a CDS date.
5. **Other regimes need their own selected rules.** A bespoke schedule
   states its own frequency, stub and boundaries; no other regime is
   inferred.

## Evidence limits

Same-kernel zero-PV replay establishes internal consistency. Qualify
the exact convention dates and the clean upfront, accrual and
settlement-cash outputs against dated external fixtures.
[ISDA RFR test grids](https://www.cdsmodel.com/rfr-test-grids.html).

## Source and validation evidence

- Implementation: [build/cds.rs](../../../../finstack-quant/calibration/src/build/cds.rs) — quarterly/semiannual date rules, upfront conversion, side and valuation override.
- Implementation: [build/prepared.rs](../../../../finstack-quant/calibration/src/build/prepared.rs) — CDS pillar cache.
- Implementation: [build/context.rs](../../../../finstack-quant/calibration/src/build/context.rs) — valuation-convention override.
- Test evidence: [src/build/tests/credit.rs](../../../../finstack-quant/calibration/src/build/tests/credit.rs) — quarterly roll sweep and distinction among accrual/settlement/protection dates; inspected or located, not executed.
- Test evidence: [src/build/cds.rs](../../../../finstack-quant/calibration/src/build/cds.rs) — tenor and explicit-date roll tests; inspected or located, not executed.
- Test evidence: [src/build/tests/quote_construction.rs](../../../../finstack-quant/calibration/src/build/tests/quote_construction.rs) — par/upfront construction coverage; inspected or located, not executed.
- Reference context: [ISDA CDS standard model](../../../REFERENCES.md#isda-cds-standard-model) — reference family; this audit does not validate any named model release; a catalogue entry is not proof of implementation equivalence.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
