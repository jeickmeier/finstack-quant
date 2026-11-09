# CORE-CONVENTIONS-020: Shared barrier classification, touch and conditional payment timing

- Decision status: **Accepted — complete barrier classification/timing package, 2026-11-07**.
- Implementation status: **Reconciled 2026-11-08** (run slice A1): no enum defaults; equity/FX `rebate_timing` is required exactly with a rebate; knock-in `AtHit` is rejected by product validation and the analytic helpers; full enum wire/predicate tests. Independent observation/window/settlement qualification at each consumer remains in the follow-up register. Scoped to `types/barrier.rs`; valuation kernels are outside this record.
- Validation status: **Evidence inspected; independent predicate/touch/wire examples and semantic review passed; production qualification pending**. The four-row truth table, 24 equal/adjacent-value boundary assertions, eight nonfinite-error examples and strict wire reference examples passed. Linked Rust/host tests were located or inspected, not run; these checks do not certify current observation, payment or pricing behavior.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

This record owns three small shared classifications and their meaning:
BarrierType, BarrierDirection and PayoutTiming. It does not own a numerical hit
checker, observation schedule, event history, rebate entitlement, settlement
ledger or pricing engine. The complete package below replaces the observed generic
defaults and overbroad documentation while retaining this narrow type boundary.
Consumer sources are sampled to identify reconciliation, not to adopt their
pricing algorithms or remaining product conventions.

## Accepted decision: barrier classification, touch and conditional payment timing

Accepted on **2026-11-07** after whole-record review. Runtime implementation
and independent product/model/host qualification remain pending;
observed source behavior is retained separately below.

1. **Retain the three closed classifications and exhaustive predicates.** Keep
   BarrierType's UpAndOut, UpAndIn, DownAndOut and DownAndIn; BarrierDirection's
   Up and Down; and PayoutTiming's AtHit and AtExpiry. Preserve the truth table
   below for `is_knock_out`, `is_knock_in`, `is_up` and `direction`. In/out
   describes deactivation/activation on a qualifying hit, not payment entitlement
   or current state. Do not add another condition registry, duplicate enum,
   numeric comparison helper or event/payoff framework to these core primitives.

2. **Remove generic economic defaults and require relevant explicit choices.**
   Remove Default for BarrierType and PayoutTiming, including the UpAndOut/AtHit
   selections, and remove enclosing builder/serde/config fallback that silently
   inserts those choices. Required financial selections must be explicit at the
   owning construction/restore boundary. If a product has one fixed contractual
   timing, its owner can encode that structurally rather than retain a redundant
   selector; any retained selector must validate its supported combinations.
   Missing required choices error, with no private replacement default or migration.
   This does not require inventing a timing input for cash that does not exist.

3. **Select inclusive touch explicitly as the shared library meaning.** At a
   contractually eligible observation of finite same-unit value x and barrier H,
   Up means x>=H and Down means x<=H. Equality is a hit, with no epsilon,
   strict-inequality substitution or automatic level shift/continuity correction.
   This is a selected library convention, not universal market or vendor parity.
   Strict-condition contracts need explicit treatment at their owner and cannot
   silently be converted to inclusive touch. No new global strict/inclusive mode
   is added by this package; unsupported conditions error at the consuming boundary.

4. **Describe direction independently of current moneyness.** Up/Down identifies
   the inequality sense, not a promise that H is currently above/below spot.
   Equality and already-beyond observations can legitimately trigger a barrier;
   do not reject or flip direction solely to keep the barrier on an unhit side.
   Current spot alone cannot prove historical absence of a hit, and a comparison
   outside the contractual monitoring window is not an automatic new hit.

5. **Leave observation and historical event state explicit with the owner.**
   Continuous/discrete monitoring, eligible times/windows/endpoints, calendars,
   time zones/fixing source, historical hit evidence, first recognized event and
   any latching/reset/seasoned state belong to the product/model. An enum carries
   none of these. Supply required history or report its absence according to the
   explicit owner contract; do not infer unhit/settled state from a label, current
   spot or a rolled valuation date. AtHit uses the first qualifying recognized
   hit; continuous first passage is one owner-specific monitoring model, not the
   meaning of every discrete AtHit payment. Existing monitoring/history types
   remain distinct rather than being merged into BarrierType.

6. **Make payment timing conditional on independently established entitlement.**
   AtHit schedules an otherwise entitled amount at the first qualifying hit;
   AtExpiry schedules an otherwise entitled amount at the specified product
   expiry. Neither grants an unconditional payment, chooses its amount/currency,
   specifies settlement lag/business-day adjustment or selects a discount curve
   or clock. Remove universal 'market standard' AtHit prose and the generic rule
   that every knock-in rebate must ignore this field and pay at expiry. A no-hit
   failure rebate, its determination window and permissible payment date require
   the owning payoff contract. Unsupported or inapplicable timing/entitlement
   combinations error or are structurally unrepresentable, not silently replaced.
   Retaining both labels does not claim every product/pricer supports both.

7. **Keep hit occurrence separate from payment/settlement status.** AtHit and a
   historical hit flag do not establish that cash was processed or discharged.
   A product helper returning only an expiry amount may explicitly exclude an
   earlier contractual payment, but cannot be used as proof that all remaining
   cash/PV is zero without the owner's settlement and cash-flow perimeter.
   Preserve explicit relevant event/payment dates and pending/settled evidence
   under [015](015-cashflow-semantics.md#accepted-decision-cash-settlement-principal-effects-and-accrual-metadata),
   [016](016-discounting.md#accepted-decision-dated-npv-selection-arithmetic-and-continuous-discounting)
   and [0045](../conventions/0045-financial-clocks-and-date-boundaries.md).
   No settlement ledger or historical-payment inference is added to the enums.

8. **Validate numerical observations at their consuming boundary.** These enums
   contain no level/spot/amount/rate and execute no float comparison. Owners must
   validate observations and barriers they use as finite and in compatible
   units/orientation before classification; NaN/infinity cannot quietly become
   no-hit through comparison behavior. Missing fixings/history need explicit
   treatment. Product/model positivity, signed-underlying eligibility, moving
   barriers, market quote conventions and amount/domain restrictions remain
   separate; do not impose universal positive spot/level rules here. Numerical
   method failure is not a valid enum result or evidence that no hit occurred.

9. **Keep exact canonical wire labels without repair.** BarrierType uses
   up_and_out/up_and_in/down_and_out/down_and_in, BarrierDirection up/down and
   PayoutTiming at_hit/at_expiry. Reject unknown/case-altered/whitespace aliases,
   numeric codes, null or missing required values; do not restore defaults or
   infer classifications from level versus current spot. Preserve explicit
   admitted values in snapshots and host product payloads. Any optional field's
   absence has only its separately specified owner meaning. No new permissive
   text parser, compatibility aliases or migration layer is selected.

10. **Reuse existing host surfaces and preserve distinct concepts.** Core enums
    currently remain Rust-only under the parity contract; hosts use canonical
    product payloads or the existing required up/down plus in/out analytic
    dispatcher inputs. Keep those deliberate boundaries with explicit validation
    and mapped host errors; do not add standalone Python/WASM classes solely for
    parity. Structural-credit default barriers, bridge crossing algorithms,
    monitoring modes and one/no-touch payoff types are separate concepts, not
    duplicates to merge. Public state/signature changes still propagate through
    Rust, host docs/schema/facade/parity and callers in the same implementation.

11. **Reconcile the actual consumers without adopting their algorithms.** Remove
    shared defaults and inherited equity/FX rebate timing defaults, correct core
    and product documentation, and audit required enum/dispatch/wire/error paths
    together. Reconcile silent knock-in timing overrides, no-touch restrictions,
    historical-hit cash exclusion and date/discount behavior with the owning
    analytic/Monte Carlo/product records. Their monitoring, bridge/continuity,
    rebate, model-selection, Greek and historical-settlement policies remain
    separately pending decisions/qualification. Acceptance of this package does
    not implement or certify any pricing method, universal timing profile,
    in/out parity, accuracy bound or contractual market convention.

### Accepted predicate truth table

| BarrierType | is_knock_out | is_knock_in | is_up | direction |
| --- | --- | --- | --- | --- |
| UpAndOut | true | false | true | Up |
| UpAndIn | false | true | true | Up |
| DownAndOut | true | false | false | Down |
| DownAndIn | false | true | false | Down |

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Missing required barrier type or applicable timing | Construction/restoration error; no UpAndOut/AtHit default. |
| Up observation x=H / Down observation x=H | Inclusive hit at an eligible observation. |
| Up observation immediately below H / Down immediately above H | No new hit from that observation; do not apply an epsilon. |
| Up with x>H / Down with x<H | Legitimate hit-side observation, not invalid direction or automatic direction reversal. |
| Spot equals or crosses H outside the owner's active window | No new hit is established by this core convention; owner history/window controls. |
| Current spot on the unhit side after an earlier eligible hit | Cannot infer unhit history or restore a knocked-out contract. |
| A strictly-above contract at x=H | Cannot silently use inclusive Up; explicit owner handling or unsupported-condition error. |
| Discrete observation at time t after an unobserved mathematical crossing | Recognized hit/payment event follows the explicit monitoring contract, not an assumed continuous first passage. |
| NaN/infinite required observation or barrier | Owner validation error, not false/no-hit. |
| Negative same-unit observation/level | No rejection from these enums; owner product/model domain decides eligibility. |
| AtExpiry label, but earning condition never satisfied | Timing alone grants no amount. |
| Historical hit plus AtHit | Does not establish that the contractual payment has settled. |
| Expiry-only payoff helper returns zero for earlier AtHit cash | Defines that helper's expiry slice only; not proof of zero pending cash/PV. |
| Knock-in no-hit rebate with inapplicable AtHit selector | Owner rejects/removes the selector; no silently forced expiry calculation. |
| NoTouch with AtHit in the located FX product | Existing owner validation rejects it; not evidence that every other combination is supported. |
| A fixed-timing product or no rebate cash | Encode the actual owner contract; do not add a meaningless choice solely to replace a default. |
| Wire up_and_out / UpAndOut / padded ' at_hit ' / numeric code / null | Canonical accepted; other forms rejected for the required enum. |
| Same enum passed to analytic/Monte Carlo/PDE pricers | Does not establish equal monitoring, history, settlement or numerical results. |
| Hosts require direction and knock strings | Continue existing explicit dispatcher path; no duplicate core wrapper classes. |

## Observed algorithm and conventions

`is_knock_out` matches the two Out variants; `is_knock_in` is its complement; `is_up` matches the two Up variants; `direction()` maps Up to BarrierDirection::Up and the rest to Down. The docs define touching as a hit (`spot >= barrier` for up and `spot <= barrier` for down), but this file does **not** execute spot comparisons or monitor paths.

PayoutTiming has AtHit and AtExpiry and schedules an otherwise entitled payment; it grants no entitlement. The type calculates no first passage, applies no continuity correction and validates nothing itself. Products validate supported combinations: `finstack_quant_models::closed_form::validate_rebate_timing` requires a timing exactly when a rebate exists and rejects knock-in (no-hit) rebates with `AtHit`.

## Observed selection, limitations and alternatives

None of the three enums has a `Default`; every product selects them explicitly, and serde accepts only canonical snake_case names. Continuous/discrete monitoring, observation calendars, already-hit state, level validation and rebate amounts are not encoded. Both payout timings are supported labels, but availability in an actual pricer must be established at that pricer. No claim is made here that all combinations are implemented or financially approved.

## Evidence

[Owning enum/predicates and serde test](../../../../finstack-quant/core/src/types/barrier.rs)
contains only classifications and exact predicate matches; it does not execute
spot/barrier comparisons, numerical validation, monitoring or pricing. Its tests
cover canonical labels and case/whitespace/numeric/null rejection for all three
enums plus the full predicate truth table; they do not cover touch behavior.

[Equity barrier fields](../../../../finstack-quant/valuations/exotics/src/exotics/barrier_option/types.rs)
and [FX barrier fields](../../../../finstack-quant/valuations/fx/src/fx_barrier_option/types.rs)
carry `rebate_timing: Option<PayoutTiming>`, validated with the rebate: a
rebate without a timing, a timing without a rebate, or a knock-in `AtHit`
rebate is a validation error. Barrier type is required.
The [FX touch product](../../../../finstack-quant/valuations/fx/src/fx_touch_option/types.rs)
has explicit direction/timing and rejects NoTouch+AtHit. These are observed product
contracts, not adoption of all their cash/history/model policies.

The [analytic rebate and expired-payoff helpers](../../../../finstack-quant/models/src/closed_form/barrier.rs)
return a NaN sentinel for the unsupported knock-in/`AtHit` combination instead of
repricing it at expiry. The expired payoff takes the rebate and its timing as one
optional pair and returns only the expiry slice, excluding hit-time knock-out cash.
That expiry-only return does not independently prove settlement or complete
remaining cash/PV.
[FX touch pricing](../../../../finstack-quant/valuations/fx/src/fx_touch_option/pricer.rs)
also has hit/history/payment distinctions requiring owner reconciliation.

The [parity contract](../../../../finstack-quant-py/parity_contract.toml)
keeps these core enums Rust-only. The [analytic dispatcher](../../../../finstack-quant/models/src/closed_form/dispatch.rs),
[Python entry points](../../../../finstack-quant-py/src/bindings/models/closed_form.rs)
and [WASM entry points](../../../../finstack-quant-wasm/src/api/models/closed_form.rs)
use explicit direction/knock inputs rather than duplicate enum implementations.

[FpML 5.12 ConditionEnum](https://www.fpml.org/spec/fpml-5-12-4-rec-1/html/reporting/schemaDocumentation/schemas/fpml-enum-5-12_xsd/simpleTypes/ConditionEnum.html)
distinguishes strict and inclusive inequalities. The consulted [QuantLib barrier
engine helper](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/instruments/barrieroption.cpp)
uses strict comparisons. These narrow primary-source observations establish that
inclusive touch must be identified as this library's selected convention, not a
universal vendor-equality or contractual-parity assertion. They were consulted on
2026-11-07, not treated as a complete current market/product qualification.

[Reiner–Rubinstein](../../../REFERENCES.md#reiner-rubinstein-1991) is pricing
background, not evidence that downstream monitoring or cash timing follows these
enum meanings. Separate owning records are [analytic barriers/rebates](../models-analytic/barriers-and-rebates.md),
[Monte Carlo monitoring](../models-monte-carlo/18-barrier-monitoring.md),
[asset barriers](../valuations-fx-and-exotics/asset-barrier.md),
[FX barriers](../valuations-fx-and-exotics/fx-barriers.md) and
[FX touch](../valuations-fx-and-exotics/fx-touch.md). Their complete conventions and
numerical methods are not adopted by this core record.

Independent reference checks passed for all four predicate rows, exact inclusive
touch at equality/adjacent binary64 values including signed zero and negative
levels, nonfinite rejection examples and exact wire-value examples. Independent
semantic reviews of the primitive and consumer packages found no material
inconsistency. These are documentation/reference checks, not execution of the
accepted consumer-validation or enum-serialization contracts. Type documentation is not
numerical validation of consuming pricing algorithms. Rust/host/model numerical
tests were located or inspected, not executed.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
