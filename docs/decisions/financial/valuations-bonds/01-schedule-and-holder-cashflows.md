# VALUATIONS-BONDS-01: Schedule construction, balance events and holder cashflows

**Decision status:** Accepted — complete checked bond schedules and holder cash package, 2026-11-14.
**Implementation status:** Runtime and caller reconciliation pending. Observed behavior below describes the current implementation.
**Validation status:** Source, published references and independent arithmetic reviewed. Runtime, market and host qualification pending.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked bond schedules and holder cash

1. **Select one complete source of contractual cash.** Require the
   bond identity, exact positive face Money in one native currency,
   explicit issue and contractual maturity, and any separately
   applicable dated/accrual start. Require issue before maturity.
   Select either a generated coupon/principal program or a supplied
   validated schedule as a tagged election at construction and
   restore. Use complete authored terms or a deliberately selected
   qualified dated security convention. Remove the competing
   cashflow spec and custom override, unchecked attachment and
   zero fixed-rate placeholder. Remove implicit currency and index
   convention defaults, automatic end-of-month selection and
   incomplete bare-spec defaults. Full admission applies at every
   entry point. A named weekends-only calendar is usable when it
   is the elected authority.
   [Dated convention selection](../valuations-framework/convention-selection.md)
   and [U.S. Treasury rules for dated and issue dates](https://www.treasurydirect.gov/files/laws-and-regulations/auction-regulations-uoc/auct-reg-gsr-31-cfr-356.pdf).

2. **Compile the exact elected periods and coupon laws.** Retain fixed
   cash coupons, authored step schedules, qualified term/overnight
   floating coupons and explicitly couponless generation. Allow a
   single explicitly selected amortization program where supported.
   Reject nested or incompatible programs and overlapping or
   unresolved coupon terms. Supply nominal and actual accrual
   endpoints, original parent identity, stubs, roll/EOM, exact
   day-count context, benchmark and observation basis where
   applicable, calendar roles and actual payment/settlement terms.
   Select rate, gearing, spread, cap/floor, PIK and rounding terms
   only where the chosen law uses them. Apply the accepted term
   and overnight projection and fixing rules at the actual valuation
   and information cut. A required fixing
   needs exact eligible publication authority. An expressly pending
   observation can use its qualified projection. Use exact signed
   fixed and step rates for cash-paid coupons where the contract
   allows them. Generated PIK stays within the admitted nonnegative
   parent allocation. Cash-only and PIK capability are distinct.
   Resolve principal and payment dates without a synthetic coupon.
   For couponless generation the shared builder uses its literal
   maturity pair. A different terminal economic/payment pair
   requires supported explicit terms or a supplied resolved schedule.
   [Contractual periods](../cashflows/01-schedule-periods.md),
   [program compilation](../cashflows/02-program-compilation.md),
   [coupon allocation](../cashflows/05-coupon-cash-and-pik.md),
   [term projection](../cashflows/06-floating-rate-projection.md)
   and [overnight compounding](../cashflows/07-overnight-compounding.md).

3. **Admit supplied schedules on their actual authority.** Validate
   the complete immutable snapshot and its basis. Require the
   bond's currency, authored lifecycle dates, row identities and
   applicable principal coverage to agree. An authorized partial
   history can have its own opening date and stock. Preserve signed
   amounts, zero and co-dated rows, qualified known/projected
   status, economic dates, actual payments, coupon parents and
   source provenance. A single valid redemption date is enough
   for a principal-only schedule with adequate authored terms
   and coverage. A payment can follow contractual maturity under
   its actual dated terms. Remove issue, maturity and frequency
   inference from cash dates or modal payment gaps. Take any
   required yield, accrued or other quote conventions from their
   own exact terms. An authorized static cash valuation can use a
   supplied schedule on its stated basis. Balance, quote, recovery,
   replay and exercise consumers need the additional context
   their calculation actually uses. A projected snapshot needs
   explicit applicability to the requested information and market
   basis. A responsive reprice requires its qualified original
   construction or replay authority.
   [Immutable schedule and coverage](../cashflows/12-schedule-lifecycle.md)
   and [bond settlement and clocks](02-settlement-and-clocks.md).

4. **Keep principal, coupon and settlement cash distinct.** Declare
   opening principal once, with a dated stock and coverage when
   tracked. Use explicit economic dates and signed deltas for each
   principal event. Cash dates can differ. A cash receipt of 102
   can retire 100 of principal. Generated PIK that accretes this
   bond capitalizes at its actual economic date with zero settlement
   cash. A separate security distribution needs its own claim and
   holder entitlement. An authorized write-down has zero settlement
   cash and a separate principal delta. Recovery is separate cash.
   Retain original coupon
   identity across rate and balance slices and preserve the complete
   schedule's cash and economic events. No inferred principal from
   cash direction, implicit zero opening for an untracked
   snapshot or forced promotion to a tracked one. Require covered
   stock for any calculation depending on it. For generated
   programs retain the accepted event order and exact balance
   closure. Entitlement changes only which cash is selected, with
   no rewrite of principal history.
   [Core cash and principal semantics](../core-conventions/015-cashflow-semantics.md),
   [economic event ordering](../cashflows/03-principal-and-emission-order.md)
   and [amortization](../cashflows/04-amortization.md).

5. **Select holder cash by event role, entitlement and payment cut.**
   Select one authorized perspective and preserve the whole schedule
   separately. Exclude the identified issuance/acquisition funding
   event from the ordinary existing-holder value. Retain any
   authorized signed cash the holder owes or receives, including
   negative coupon or fee amounts. Apply the shared settlement
   cash kinds and exclude noncash principal effects from PV.
   Ordinary holder valuation includes eligible payments after the
   valuation cut. An explicitly entitled same-day view includes
   only known payable cash at factor one. A known detached
   receivable can remain payable to the holder of record.
   A booked position or date alone gives no proof of those rights.
   A buyer uses the actual qualified settlement entitlement.
   Known overdue unpaid cash requires its own authorized treatment.
   Validate the contract before lifecycle shortcuts and skip
   inactive lookups for a proved extinguished claim.
   [Holder and cash perspectives](../conventions/0044-financial-perspectives-and-boundary-conversions.md),
   [checked dated NPV](../core-conventions/016-discounting.md)
   and [cashflow export](../valuations-framework/cashflow-export.md).

6. **Materialize ex-coupon terms per affected claim.** Carry the
   actual source clause, calendar, settlement restrictions and
   applicable cutoff. For every original cash coupon parent,
   elect all-cum or supply its inclusive first-ex eligible
   settlement date `X`. A buyer settling on or after `X`
   loses that coupon. Unknown applicability errors. Map an
   inclusive last-cum/record date to the first ex settlement
   under the actual contract and eligible calendar. Preserve
   distinct entitlements for co-dated parents. An applicable
   principal detachment or final-period settlement restriction
   needs explicit security-specific admission. An unsupported
   settlement fails even if the remaining cash could be summed.
   Keep the decision to admit a trade separate from stripping a
   coupon. Record 02 owns the settlement-date calculation,
   accrued quotation, clean/dirty and carry transforms.
   [Accepted coupon quotation and first-ex dates](../cashflows/13-accrued-interest.md)
   and [UK DMO gilt entitlement and final settlement restrictions](https://www.dmo.gov.uk/responsibilities/gilt-market/about-gilts/).

7. **Keep actual valuation separate from contingent balances and
   hypothetical metrics.** Use the same prepared holder cash and
   original parent entitlement in the static and selected dynamic
   paths, including floating coupons. Exercise or credit routes
   consume the exact schedule, positions, forecasts, balance history
   and contingent terms they require. Their event timing, recovery
   conventions and selected payoff laws stay with the relevant bond
   records. Remove the implicit metrics substitute that turns PIK
   into cash and drops structural model configuration. A hypothetical
   cash-pay measure needs an explicit, separately authorized basis
   with its own provenance. A contractual schedule can be inspected
   with its contingency limits. Selected-model fair-value export
   requires a complete, reconciled decomposition and errors if
   unsupported. A deterministic unexercised schedule is not the
   cash result of an option exercise.
   [Selected-model metric ownership](../valuations-framework/metric-orchestration.md),
   [bond model ownership](03-pv-models.md)
   and [exercise event ownership](04-rates-tree-exercise.md).

8. **Use one validated preparation for valuation and consumers.**
   Determine the scheduled last cash date structurally from actual
   payment terms and the source's admitted cash claims. Keep
   contractual maturity and noncash activity as separate dates.
   Preserve the contractual audit view, eligible holder cash,
   tracked principal and any model contribution through Rust
   valuation, calibration, quote and workout callers. Reconcile
   the tagged bond schema, typed Python and WASM bond builders,
   supplied-schedule constructors and pricing/export routes with
   the exact same source selection and validation. Preserve amount
   and currency, term and fixing authority, row roles, cut and
   capability errors. Qualify
   irregular and one-payment schedules, delayed cash, signed
   coupons, PIK and write-downs, and actual market entitlement
   boundaries independently.
   [Bond schema](../../../../finstack-quant/valuations/schemas/instruments/1/fixed_income/bond.schema.json)
   and [existing Rust bond schedule ingress](../../../../finstack-quant/valuations/src/instruments/fixed_income/bond_json.rs).

## Observed behavior

### Choice, scope and rationale

The bond adapter uses the canonical cashflow builder for fixed, floating, step-up and singly wrapped amortizing specifications, or takes a supplied custom schedule verbatim. Keeping the schedule as the cash/balance source is the source-stated design; accepting its product-specific defaults remains an adoption decision. Projection, fixed/step-up accrual and PIK accrual kernels belong to the cashflows scope, while this record owns their bond selection and filtering.

### Algorithm and controls

1. `full_cashflow_schedule` returns a clone of `custom_cashflows` when supplied. Otherwise it creates issue/maturity principal and applies `fixed_cf`, `floating_cf`, `step_up_cf`, or amortization followed by the base coupon specification, then builds against the market.
2. Holder valuation rows have dates strictly after `as_of`, exclude PIK rows and negative `Notional` exchanges, and drop interest-like rows detached by the ex-coupon rule at the separately supplied entitlement date. Other signed rows, including fees, survive; this is not simply a positive-receipts filter. Same-date rows are included only by the explicit inclusive exercise helper.
3. Accrual configuration defaults to linear, includes PIK, supplies coupon frequency, and optionally supplies an ex-coupon window/calendar. Cashflow export preserves the signed schedule; static export of exercise-contingent bonds is rejected. `metrics_equivalent` makes the coupon cash-pay and removes Merton configuration.
4. The bare `CashflowSpec` constructors use Following, ShortFront, weekends-only, no end-of-month roll, no payment lag and unadjusted accrual dates. Default spec is zero-rate semiannual 30/360. Floating defaults use index-registry calendar/reset lag when found, otherwise weekends-only/two days; gearing is one, spread included in gearing, no caps/floors, no compounding override. Registry lookup failure is silently converted into those constructor defaults.
5. `Bond::fixed` selects US corporate conventions; `with_convention` applies convention frequency/day count/stub/calendar/settlement/ex-coupon and turns on end-of-month only when both endpoints are month ends. Floating currency inference is USD corporate, EUR corporate, GBP gilt or JPY JGB; other currencies require an explicit convention. Zero coupon uses annual ACT/365F without inferred settlement.
6. `from_cashflows` validates the schedule, takes metadata issue/maturity or first/last dates, and infers quote frequency from the modal interval of Fixed/Stub/FloatReset payment dates. Recognized day buckets are annual 355–375, semiannual 175–192, quarterly 85–98, bimonthly 55–68, monthly 25–37 and weekly 5–9; ties favor shorter intervals, fewer than two coupons imply annual. It creates a zero fixed-rate placeholder with the inferred frequency and schedule day count.

### Domain, failures and alternatives

Public pricing validates positive notional, maturity after issue, nonnegative fixed/step-up rates, no nested amortization, option windows/prices, floors and overrides. Deserialization can infer issue from the first custom-schedule date; it does not itself run all economic validation. `with_custom_cashflows` also just installs the schedule. A custom schedule's amounts override coupon-spec amounts, but the coupon spec still supplies frequency, day-count and model capability tests. Explicit conventions/frequency metadata are the available alternative to heuristic construction; a fully schema-owned custom coupon convention would require a new decision.

## Evidence

Owning code: [cashflow specification](../../../../finstack-quant/valuations/fixed-income/src/bond/cashflow_spec.rs), [construction and custom-frequency inference](../../../../finstack-quant/valuations/fixed-income/src/bond/types/construction.rs), [definitions and option validation](../../../../finstack-quant/valuations/fixed-income/src/bond/types/definitions.rs), [holder filter](../../../../finstack-quant/valuations/fixed-income/src/bond/types/pricing.rs), [trait/export behavior](../../../../finstack-quant/valuations/fixed-income/src/bond/types/traits.rs). [Construction tests](../../../../finstack-quant/valuations/fixed-income/src/bond/types/tests.rs) inspect custom schedule preservation; [step-up/serde tests](../../../../finstack-quant/valuations/fixed-income/src/bond/types/tests/stepup.rs) inspect missing-issue handling and floating row signs. These are behavior/invariant tests, not external valuation benchmarks. [ICMA](../../../REFERENCES.md#icma-rule-book) is a convention reference, not proof these defaults fit every security.

### Reconciliation evidence

The current generated path uses the builder without an explicit
as-of projection cut. The provider can relabel a custom schedule
from the unused coupon spec, which also gates some downstream
calculations. The current last-payment lookup reconstructs that
market-dependent schedule. Existing typed and tagged host callers
need the same admission and capability checks.
[Schedule construction](../../../../finstack-quant/valuations/fixed-income/src/bond/types/construction.rs),
[provider and metrics traits](../../../../finstack-quant/valuations/fixed-income/src/bond/types/traits.rs),
[typed Python ingress](../../../../finstack-quant-py/src/bindings/valuations/instruments.rs)
and [typed WASM ingress](../../../../finstack-quant-wasm/src/api/valuations/typed/fixed_income.rs).

### Independent review evidence

- With principal 1,000 at 4% and a contractual quarterly
  accrual of .25, coupon cash is 10. Take contractual maturity
  and the economic principal date as Saturday January 31, 2026,
  and the authorized payment as Monday February 2. Principal
  stock can close at zero on January 31 while principal and
  coupon cash of 1,010 remain payable. Factor .99 gives holder
  PV 999.90 if the holder owns both claims.
- Under a coupon parent with accrual .5, rate 6% and opening
  principal 1,000, 30 of interest splits into 10 cash and 20
  capitalized principal. A same-date amortization of 200 leaves
  820 outstanding. The next equal period earns 24.60. One
  coupon parent keeps its allocation and entitlement even when
  stock history has multiple slices.
- An opening stock of 100 with issue cash −98 and zero
  added principal is followed by zero-cash PIK of +5 and a cash
  repayment of 21 with principal delta −20. Stock becomes 85.
  An authorized zero-cash write-down of 15 leaves 70, a distinct
  recovery receipt of 4 leaves stock at 70, and redemption cash
  72 retires the last 70. With a separate 2 cash coupon, the
  eligible amounts 2, 21, 4, 72 give PV 95.685 at factors
  .99, .985, .975, .96. The initial funding stays in the full
  schedule. This is an event arithmetic check, with modeled default
  and recovery laws left to their owners.
- The DMO's January 31, 2025 example gives January 22 as
  the last cum/record date. In a weekday settlement example
  January 23 is first ex. For illustrative co-dated coupon
  parents of 5 and 7 with first-ex dates January 23 and 24,
  an eligible buyer settling January 22, 23 or 24 gets 12,
  7 or 0 respectively. A holder with both retained entitlements still
  gets 12. A final gilt in the DMO restricted window requires
  refusal of the unsupported purchase.
  [DMO gilt entitlement example](https://www.dmo.gov.uk/responsibilities/gilt-market/about-gilts/).

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
