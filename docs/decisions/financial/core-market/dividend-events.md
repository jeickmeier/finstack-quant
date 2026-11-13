# CORE-MARKET-13: Explicit cash-dividend events, dates and share basis

- Decision status: **Accepted — complete cash-event/source package adopted 2026-11-08.**
- Implementation status: **Observed** in the owning paths below; the accepted cash-only source and consumer reconciliation are unimplemented.
- Validation status: **Primary/source evidence and independent event/market, consumer/model and state/host reviews inspected; standalone rational/date/high-precision examples executed 2026-11-08; production qualification pending.** Existing tests were inspected or located, not executed. Examples do not qualify dividend pricing, entitlement or corporate-action accuracy.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Scope and accepted authority

This record owns supplied cash-dividend source events, their exact identity,
share/currency basis, dates, admissibility and immutable dated reuse. It preserves
accepted [Money precision](../core-conventions/012-money-decimal-precision.md),
[financial perspective/sign boundaries](../conventions/0044-financial-perspectives-and-boundary-conversions.md),
[date/information phases](../conventions/0045-financial-clocks-and-date-boundaries.md)
and [record12's validated scalar boundary](scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings).

[Equity carry](../valuations-equity-and-composition/02-spot-and-carry.md),
[smile forward inputs](../calibration-targets/09-equity-forward-inputs.md),
[option adapters](../valuations-equity-and-composition/07-black-tree-and-direct-greeks.md),
[lattice dividends](../models-numerical/dividends-and-lattice-exercise.md),
[Monte Carlo jumps](../models-monte-carlo/06-cash-dividend-jumps.md) and
[TRS](../valuations-equity-and-composition/16-equity-trs.md) retain their remaining
Draft pricing/carry/entitlement choices. No stock-price jump, reinvestment,
withholding, escrow, exercise or corporate-action model is adopted here.

## Foundational findings

An ex-date and a payment date have different meanings. The current source has
only an ex-date, while its sole located numerical consumer discounts cash at
that date. This loses payment delay and cannot describe an ex-past/pay-future
receivable. Conversely, special distributions can go ex after payment; source
validation must not invent a universal ordering of those two authoritative dates.

The current enum also mixes a cash magnitude, an annual continuous yield with
no accrual interval, and additional shares/share under one dated event. The
located production reader already rejects both noncash variants. Numerical
Monte Carlo uses `p*S` as the proportional cash drop and `(1-p)*S` as the
remaining stock value on the same share basis. Stock distributions belong
to corporate-action owners.

Optional ticker/currency metadata cannot establish the underlying or units.
Same-ex-date ordinary/special distributions may be distinct, especially when
their payment dates differ. Date sorting alone supplies neither event identity
nor an economic sequencing rule.

## Accepted decision: identified cash-per-share events with explicit dates

Accepted on **2026-11-08** after independent event/market, consumer/model and
state/host review. Runtime/caller/host reconciliation and independent financial
qualification remain pending; observed implementation stays separate below.

1. **Retain a cash-only source.** Keep DividendSchedule and DividendEvent for
   supplied cash distributions. Remove DividendKind and its Cash/Yield/Stock
   wire variants; an event has a direct cash Money amount rather than a one-
   variant enum. Continuous dividend yield remains an explicit scalar/model
   rate with its owned interval/clock; stock dividends, splits and share-count
   changes remain corporate actions at their actual owner. Do not convert
   stock ratios into model proportional-cash fractions or a yield into a
   cash event. No replacement corporate-action registry/framework is introduced.

2. **Require the actual underlying, share basis and currency.** A schedule
   requires its nonempty ID, exact nonempty UnderlyingId and one distribution
   currency; remove the optional display ticker as financial identity. Resolve
   the authoritative security/share-class and quotation/share basis at the
   existing data/market owner and verify agreement with each consuming product.
   Amounts are nonnegative gross cash magnitudes per one entitled share/unit
   on that declared basis, before investor-specific withholding/fees or position
   direction. They are not a position's total cash flow. Each event Money must
   match the schedule currency exactly. Mixed currency, wrong underlying or
   incompatible share basis errors without guessing, ticker/ID matching or
   automatic FX conversion. Actual ADR conversion, taxes and corporate-action
   rebasing require an explicit qualified owner; a currency label is insufficient.
   Empty schedules are valid but still require the same identity/basis.

3. **Make each cash event explicit and immutable.** Require a stable nonempty
   event_id, ex_date, pay_date and admitted nonnegative Money amount. Inspect
   Money's exact Decimal sign/domain, preserving exact input rather than first
   converting to a model float. Zero amounts are valid and do not automatically
   mean cancellation, a declaration or payment. Require unique event IDs across
   the schedule; two revisions of one event are selected upstream, not stored
   as separate payable claims. Validate direct event construction and schedule/
   wire admission through the same checked owners and expose read-only get_*
   access. Canonical event order is (ex_date,event_id), with literal ID ordering
   for ties; reject unordered source/state instead of sorting or repairing it.
   This is deterministic storage order, not financial priority.

4. **Keep both supplied dates without deriving their order.** ex_date is the
   supplied market cum/ex entitlement-transfer boundary; pay_date is the
   scheduled cash payment date. Require valid explicit dates but do not default
   pay_date=ex_date, roll them, infer a record date or require pay_date>=ex_date.
   Due-bill/large-distribution cases make the reverse ordering legitimate.
   The source does not prove that a particular position owns the payment:
   settlement, record-date, beneficial-holder and due-bill rules retain their
   actual contractual/corporate-action owner. A consumer unable to handle the
   supplied regime must error, not rewrite dates or apply ordinary carry blindly.

5. **Preserve distinct same-date events and actual cash timing.** Permit
   multiple unique events on one ex-date, retaining their own payment dates,
   Money and provenance. Do not deduplicate by date, overwrite or automatically
   sum them. A model adapter may explicitly form an equivalent aggregate only
   after validating underlying/share/currency basis, cash timing and its owned
   model assumptions, preserving the contributing event identities. Distinct
   pay dates cannot become one raw cash payment without a timing model.
   Any Money aggregation retains accepted exact-or-error arithmetic.
   In a supported cashflow valuation, discount the entitled cash at its actual
   pay date on the appropriate dated discount/FX basis. Ex-date controls the
   selected entitlement boundary, not automatically the date used to discount
   the cash. Converting a delayed payment to an ex-date model jump requires a
   separately qualified claim-value adapter; no exact stock-price-drop rule is
   supplied by this source.

6. **Keep date phases, carry selection and data knowledge explicit.** Products
   select their before/after ex-date, exercise, settlement and payment phases;
   no universal (as_of,expiry] inclusion is imposed here. Continuous q supplied
   alongside discrete cash must have an explicit total-versus-residual carry
   meaning at the financial owner, without priority inferred from a nonempty
   schedule or a zero q. No double counting or silent suppression of one input.
   Data/construction owners retain source/snapshot provenance, which events are
   announced versus forecast, and the operation's information cutoff. Supplied
   amounts/dates alone do not prove announcement, certainty, entitlement or
   payment. A later revision or forecast cannot acquire historical authority
   merely by appearing in the schedule. Keep that provenance with canonical
   market/construction transport rather than adding a universal announcement
   database or guessed release calendar to core.

7. **Reuse original absolute events as valuation advances.** Retain original
   event IDs, ex/pay dates, underlying/share basis, currency and exact amounts.
   Do not shift dates, delete all ex-past records, settle receivables, mark
   forecasts declared/paid or manufacture replacement events during source/context
   aging. An ex-past/pay-future event may still matter to the actual position's
   receivable owner; preserving it does not grant it to a new ex-dividend buyer.
   Explicit new data or a hypothetical scenario is a new labelled source view
   with selected provenance, not a public generic dividend bump. Broader context
   and instrument lifecycle/corporate-action evolution remain separately owned.

8. **Reconcile actual financial inputs and numerical adapters together.** The
   current calibration reader guesses a schedule from ticker/ID, strips currency
   and discounts at event.date. Require exact configured schedule association
   and complete units/dates, with carry selection explicit at that owner.
   Reconcile independent equity spot/future/option/TRS cash inputs to the same
   checked event meaning; remove obsolete high-level bare(Date,f64) cash-dividend
   paths rather than retain a parallel ex-only source or infer pay=ex. Numerical
   kernels may retain small time/jump inputs as their own model parameters,
   reached through an explicit adapter preserving the rich financial source.
   The adapter must qualify payment delay, share basis, same-date aggregation,
   entitlement and cum/ex ordering, or reject an unrepresentable source case.
   Convert Money to a model value only at the checked numerical boundary after
   currency/unit admission. Do not mechanically map the new events into a
   tree/MC/PDE's existing instantaneous-cash assumption. Model floors, grid
   timing, depleted-stock behavior, escrow and exercise approximations are
   separately reviewed in the pricing records. The accepted lattice contract is in
   [MODELS-NUMERICAL-002](../models-numerical/dividends-and-lattice-exercise.md#accepted-decision-for-checked-escrowed-dividends-and-lattice-exercise).

9. **Keep strict state and existing host transport.** Canonical state contains
   required schedule identity/currency, ordered identified cash events and their
   explicit dates/amounts; retain authoritative share-basis/source provenance at
   the actual owning market/construction layer. Reject missing/unknown fields,
   duplicate IDs, noncanonical order, negative amounts or currency mismatch
   without defaults, aliases, repairs, migrations or old-kind/date readers.
   Reconcile core context/state, attribution snapshots, calibration MarketDatum/
   input schema/context builder, generated declarations/schemas/goldens and
   examples together. The standalone dividend classes are deliberately unbound
   in Python/WASM today; update actual transport/parity exclusions and product
   surfaces, without inventing host dividend classes or a second calculation.

## Observed implementation and discrepancies

The builder has optional underlying/currency metadata and adds cash events at
one ex-date. It sorts by that date, retains all duplicates and checks nonnegative
cash through a model-float Money view. Wire can additionally construct finite
nonnegative Yield/Stock variants. Schedule/event currencies need not agree;
there is no event ID, payment date or share-basis enforcement. Empty schedules
are accepted. These are observed behaviors, not adopted choices.

The sole located numerical reader of core DividendSchedule is the calibration
forward-input utility. It matches by optional ticker or guessed schedule IDs,
filters ex dates on/before base, discounts on the ex date and rejects future
noncash events. Other searched core uses store/transport/snapshot the source.
This is search evidence, not a runtime reachability proof.

Actual equity pricers use independent one-date numeric vectors, with different
duplicate, horizon and continuous-yield precedence rules. Numerical kernels
also have their own dividend jump/exercise assumptions. Core context rolling
clones schedules unchanged. The new source requires coordinated financial
adapters rather than asserting that those independent paths already honor it.

## Independent examples and qualification limits

On **2026-11-08**, ten standalone Fraction/200-digit Decimal/date cases were
executed without production imports. The discount/stock expressions below are
illustrative stated models, not universal dividend pricing or entitlement laws:

- Cash10 with relative factors D(ex)=.98,D(pay)=.95 has payment-date cash PV9.5,
  while substituting the ex date gives9.8. Under the stated deterministic claim
  model its ex-date value is10*.95/.98, approximately9.69387755, not10. These
  were separate payment-PV and ex-claim examples.
- Same-ex-date cash1/2 with payment factors.97/.91 has combined PV2.79.
  Replacing it by cash3 paid wholly on either date gives2.91 or2.73.
- Gross cash.25/share for100 shares is25; a short100-share position has-25
  before owner-specific effects. Applying20% withholding to the long amount
  gives20 without changing the source's gross25.
- An illustrative pure10% share distribution at unchanged capitalization gives
  per-share100/1.1, approximately90.90909; a10% proportional cash jump gives90.
  Cash1 per old share versus per new share after that distribution gives1 versus
  1.1 for an original one-share holder. These were separate share-basis cases.
- A continuous annual q=.04 over half a year implies1-exp(-.02), approximately
  .01980133 of continuous carry in that model, not one dated cash event of.04.
- The SEC's special-distribution example can pay2026-03-17 and go ex2026-03-18;
  pay<ex is not intrinsically invalid. Separately, an illustrative March16 ex/
  March27 pay event at valuation March20 has crossed ex but not payment, so
  filtering ex-past records cannot by itself settle a receivable.
- Exact decimal cash.1+.2=.3 remains exact in the admitted Money input model;
  the corresponding binary64 addition is.30000000000000004. This was the tenth
  independent representation case, not execution of production Money arithmetic.

Independent production qualification must cover exact underlying/share/currency
and gross/position/tax boundaries; event identity, empty/zero/same-date/order and
strict state; announced/forecast/revised knowledge and original-source reuse;
explicit ex/payment/record/settlement phases, due-bill refusal/support and
ex-past/pay-future positions; dated discount/FX timing and same-date aggregation;
continuous/discrete carry selection and actual calibrated/priced source adapters.
Qualify numerical input conversion, grids/jumps/exercise/floors and model error
against independent product references at those owners. Construction counts,
same-pricer fit and serialization round trips do not certify those conclusions.

## Evidence and alternatives

Inspected [core dividend source](../../../../finstack-quant/core/src/market_data/dividends.rs),
[calibration carry reader](../../../../finstack-quant/calibration/src/targets/util.rs),
[spot](../../../../finstack-quant/valuations/equity/src/spot/types.rs),
[equity option inputs](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/inputs.rs),
[TRS](../../../../finstack-quant/valuations/equity/src/equity_trs/pricer.rs),
[tree](../../../../finstack-quant/models/src/trees/binomial_tree.rs),
[Monte Carlo process](../../../../finstack-quant/models/src/monte_carlo/process/gbm_dividends.rs),
[attribution snapshots](../../../../finstack-quant/attribution/src/factors.rs)
and [host parity ledger](../../../../finstack-quant-py/parity_contract.toml).
Existing core construction, equity carry/option, tree and Monte Carlo tests were
inspected or located, not executed; they do not independently qualify this source.

[FINRA Rule11140(a)/(b)(2)](https://www.finra.org/rules-guidance/rulebooks/finra-rules/11140)
and the [SEC ex-dividend explanation](https://www.investor.gov/introduction-investing/investing-basics/glossary/ex-dividend-dates-when-are-you-entitled-stock-and)
distinguish market ex-date designation and special distributions with ex dates
after payable dates. They do not prescribe this source's event IDs, cash-only
scope or canonical order; those are explicit architectural choices. [Hull](../../../REFERENCES.md#hull-options-futures)
provides broader dividend/carry model context, without qualifying the existing
escrow, Monte Carlo or payment-delay approximations.

A continuous yield model, general corporate-action history and automatic
distribution-to-stock-jump conversion are not added as source modes/fallbacks.
The smallest complete source is identified cash-per-share events retaining both
dates, exact amounts and their actual financial association.

## Follow-up tracking

Runtime reconciliation and independent qualification are
tracked in the [core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
