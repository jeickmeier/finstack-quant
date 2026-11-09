# CASHFLOWS-004 — Scheduled principal amortization

- Decision status: **Accepted — complete scheduled-amortization package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, scoped below.
- Validation status: **Production evidence inspected/located, not executed**. Independent bounded exact-arithmetic/date-policy examples and read-only formula/host/consumer reviews are reported below; they do not qualify production execution or contractual amortization schedules.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and input domain

The observed builder prepares one amortization rule from all fixed and floating economic accrual boundaries, including coupon windows and rate groups. It distinguishes contractual amortization from final redemption. Opening and target notionals are same-currency nonnegative amounts; percentage inputs are finite f64 fractions in `[0,1]`. The observed default is no scheduled amortization. No level-payment annuity solver is present in this slice. The accepted rules below replace the specified defaults, fields and formulas; observed behavior is retained separately.

## Accepted decision: explicit amortization laws, live-balance installments and monetary boundaries

Accepted on **2026-11-08** after complete whole-record review. Runtime/caller/host/schema reconciliation and independent qualification remain pending.
Inherit accepted [001's actual periods and original context](01-schedule-periods.md#accepted-decision-contractual-periods-calendars-and-day-count-context),
[002's independent-window operator](02-program-compilation.md#accepted-decision-coupon-and-payment-program-compilation),
[003's dated opening/history, atomic principal batches and phase order](03-principal-and-emission-order.md#accepted-decision-dated-opening-principal-signed-cash-and-atomic-event-phases),
[core012's exact Money and bounded transforms](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity)
and [core015's literal cash/principal effects](../core-conventions/015-cashflow-semantics.md#accepted-decision-cash-settlement-principal-effects-and-accrual-metadata).
The seven rules below are explicit deterministic builder operators. They are not
universal loan, bond, swap, annuity, prepayment-allocation or settlement conventions.
This review changes no runtime or generated artifact.

1. **Require one explicit amortization rule and define its original base.** Keep
   the seven existing AmortizationSpec variants with no automatic mode selection.
   Require an explicit variant, including None, through complete Notional/build
   inputs. Remove AmortizationSpec::Default, implicit builder/serde omission as
   None and Python's omitted-amort fallback; the incomplete builder may wait for
   the choice but build cannot infer it. All notionals/targets/repayment amounts
   use the opening currency and exact Money. Let B0 be003's declared initial
   stock BEFORE opening-date movements, never issue-closing stock, cumulative
   funding, purchase cash or original commitment. Pre-issue activity does not
   rewrite B0. Let B be live stock after003's same-date admitted PIK and explicit
   principal batch, before this amortization phase. Stock remains nonnegative;
   position signs and FX stay with their owners.

2. **Use exact per-boundary Decimal fractions.** Replace the two percentage
   variants' f64 pct fields with the existing canonical Decimal representation
   named fraction, exactly in [0,1]. For example0.05 means five percent per
   selected boundary, not5,5bp or an annual rate. Keep the descriptive variant
   names and rename only that member/parameter without aliases. Use accepted
   exact decimal host/wire admission; do not coerce caller decimal text/integers/
   Python Decimal through binary64 or accept a host float as the contractual
   fraction implicitly. A caller choosing a genuine model-float conversion must
   perform that declared core012 boundary first. Zero and one are valid; reject
   out-of-range, unrepresentable or malformed values. No inferred frequency,
   day-count weighting, annualization or universal fraction tolerance.

3. **Use actual coupon boundaries as the sole generated cadence.** Build D as
   the sorted distinct ACTUAL coupon accrual-end dates across every admitted
   fixed/floating period and002 atomic window. Count each distinct economic
   date once, including stubs/irregular periods with equal per-boundary weight.
   Preserve period identities/context while deriving that cadence. Retained
   program cuts can change D, counts and amounts under the accepted operator;
   rate grouping alone cannot. Fee boundaries, standalone principal/cash queue
   dates, ICMA nominal subdivisions and balance/rate calculation slices do not
   add installments. Each grid-based rule requires nonempty D even for a zero
   fraction, zero B0 or F=B0. Require every used cadence date to be in [issue,E],
   where E is003's actual terminal economic date; reject an unsupported pre-issue
   generated installment rather than silently filtering or shifting its date.

4. **Give dated instructions explicit economic bounds.** StepRemaining and
   CustomPrincipal dates lie in [issue,E], inclusive, and may be issue dates or
   noncoupon dates. Remove the additional raw-maturity upper cap, consistently
   with003 when adjustment extends E. LinearBetween requires issue<=start<end<=E,
   end in D and at least one date in D intersect (start,end]. start need not be
   a coupon boundary and causes no installment/coupon cut itself. opening_date
   may precede issue, but scheduled amortization is not a pre-issue activity in
   this builder; use003's explicit dated events for such actual movements.
   Validate every supplied date, including zero amounts, without clipping,
   snapping, stub/frequency repair or cash-date substitution.

5. **Select one payment-date mapping without changing economic cadence.** For
   d in D use the corresponding coupon's actual adjusted/lagged payment date.
   Require a unique mapping for each economic date; conflicting payment dates
   error, even if the computed repayment would be zero. Different economic dates
   may share a payment date and remain distinct movements/rows. A dated
   StepRemaining/CustomPrincipal instruction on d outside D settles on d itself:
   this is an explicit unadjusted recipe, not inferred BDC/lag. A custom price,
   independent settlement date or premium belongs in003's explicit cash/delta
   event, not a second amortization cash/sign convention. Settlement can precede
   or follow d without changing exposure or the principal activity ceiling.

6. **Keep explicit zero-action programs valid.** None means no scheduled
   reduction; it does not promise constant stock or suppress003 PIK, explicit
   events or separately selected final redemption. Empty StepRemaining and
   CustomPrincipal lists are valid explicit empty instructions. Validate all
   supplied nonempty payloads, including exact zeros, before preparing a rule.
   A computed repayment of exact zero emits no generated amortization row;
   a supplied zero CustomPrincipal amount creates no repayment and no coupon
   boundary. This does not delete003's explicitly supplied zero event rows.
   Tiny nonzero amounts must not be treated as zero by display or tolerance.

7. **Make LinearTo's final target literal and protect it from scheduled paydown.**
   Require same-currency exact F with0<=F<=B0 and n=|D|>0. Compute the nominal
   installment I=(B0-F)/n once using core012's bounded division. At each nonfinal
   d in D pay min(I,max(B-F,0)); on the final date require B>=F and pay the exact
   difference B-F, leaving EXACTLY F after this amortization phase. If B<F there,
   error instead of claiming the target was reached, issuing negative repayment
   or inventing a draw. Earlier external movements may temporarily put B<F;
   scheduled payment is then zero and later funding may restore feasibility.
   F is a target after scheduled amortization, BEFORE003's separately selected
   final redemption, which can repay that residual. Additional PIK/draws can
   increase the final scheduled payment; early repayment does not authorize a
   scheduled payment to consume the reserved target. No hidden target reset.

8. **Make StepRemaining a checked exact live-stock target.** Require strictly
   increasing supplied economic dates, same-currency nonnegative target T and
   no duplicate dates, even for equal targets. At each date require T<=B, pay
   exact B-T and leave T after the amortization phase, before003's separately
   selected terminal redemption can repay that residual. Replace misleading
   remaining-principal-after-date descriptions with this phase-specific meaning.
   Targets need not decrease
   relative to B0 or a previous target if intervening PIK/draws funded the live
   stock. A target above B errors; do not increase principal, cap it or repair
   the target. Final redemption still follows003's independent exchange choice.

9. **Keep original-per-period reduction based on the declared B0.** For
   PercentOfOriginalPerPeriod compute I=B0*fraction once by core012's bounded
   transform, then pay min(I,B) at every date in D, including the last. No
   terminal sweep or target is implied by this variant. Later PIK/draws do not
   increase the base and repayments do not recast it. B0=0 means zero scheduled
   payments even when later funding is positive; a product using cumulative
   funded principal or commitment as its original base needs its real owner
   contract/explicit schedule, not an adapter that silently changes this law.

10. **Keep remaining-per-period reduction based on current stock.** For
    PercentOfRemainingPerPeriod pay core012's bounded B*fraction on each d in D.
    Include that date's admitted PIK and atomic principal batch through B. No
    implicit full sweep at the final date; a fraction below one generally leaves
    stock for003's chosen exchange or residual history. With no other movements,
    the unrounded mathematical stock is B0*(1-fraction)^j after j boundaries;
    actual bounded Decimal transforms can differ by their stated rounding error.
    Reject a computed negative/above-B result rather than silently repairing an
    impossible fraction or arithmetic failure in a common emitter.

11. **Replace LinearBetween's cached/draw-specific adjustments with B/k.** For
    each d in D intersect (start,end], let k count the remaining dates INCLUDING
    d through end. Pay core012's bounded B/k; when k=1 pay B exactly, leaving zero
    after the end-date amortization phase. No further scheduled payments occur
    outside that window; later admitted PIK/draws can create new stock and003's
    exchange/residual policy then applies. start only selects the open-left
    window; there is no start-stock snapshot, installment at start, cached
    installment, later-draw add-on or post-payment recast state. All PIK, draws,
    repayments and signed corrections admitted before this phase affect B
    uniformly; no eligible funding kind, cash-sign detector or gross-positive
    draw total. A same-date draw participates in TODAY'S payment, deliberately
    replacing the old next-boundary rule. Net-zero refinancing cannot inflate
    installments. With unchanged stock apart from this amortization, unrounded
    payments are equal; finite Decimal payments can differ by a computation
    quantum and the exact final sweep conserves admitted stock. This is a
    disclosed live-balance operator, not fixed original principal installments
    under later activity or a level-total-payment annuity. Remove obsolete
    installment initialization/update work from the owner004 slot in003's order;
    the relative order of all economic phases is unchanged.

12. **Keep CustomPrincipal as literal scheduled par reductions.** Require all
    supplied same-currency amounts nonnegative; input dates need not be sorted.
    Group by exact economic date and sum ALL amounts exactly, so duplicates add
    rather than deduplicate and order cannot change economics. Pay that total P
    only if P<=B, otherwise error; no cap to initial principal, funded-only stock
    or a rounded/currently displayed balance. Total lifetime repayments may
    exceed B0 when PIK/draws supplied principal. Same-date entries deliberately
    produce one aggregate scheduled row;003 explicit event rows retain their
    separate identity. Every supplied zero/currency/date is still validated.

13. **Emit exact par cash and complete principal effects without generic repair.**
    The owning rule computes an admitted0<=P<=B. The common emitter checks those
    bounds, emits cash=+P, principal_delta=-P, principal_date=d and the selected
    payment date as CFKind::Amortization, then performs exact B-P. Use stored
    Money/Decimal throughout; remove Money→f64→Decimal and pay.as_f64() cash
    reconstruction. A zero P may omit the generated row; a negative/over-B P
    errors. Remove the common unconditional min(pay,B), max/floor and nonpositive
    suppression that could hide invalid rule results. Capping is allowed only
    where explicitly part of7/9. Cash and par reduction equal here by this
    operator;003 retains independently priced cash events. Inherit003 atomic
    phase publication, nonnegative stock and faithful closing history/replay.

14. **Select no financial payment rounding in this generic operator.** Retain
    sub-minor-unit precision and inherit core012's exact add/subtract/sums and
    once-rounded nearest-even bounded scalar multiply/divide. A required exact
    difference/sum that is unrepresentable errors; bounded transform overflow,
    nonzero-to-zero underflow and invalid divisors error. Do not replace an
    underflowed installment by zero or defer it to the final sweep. No implicit
    cents, currency metadata quantum, configured presentation rounding, default
    financial mode, penny allocation or tolerance. Computational bounded Decimal
    rounding is not contractual settlement rounding. A contract requiring a
    selected payment scale/mode and final adjustment must have its owner
    materialize/qualify the actual rounded CustomPrincipal schedule or explicit
    priced003 events. Do not add a second rounding framework/mode to this slice.

15. **Validate completely and return a complete local result or error.** Apply
    canonical local rule validation before configuration/restoration, then final
    B0/currency/horizon/cadence/payment-map/live-stock checks at compilation and
    execution. Plain raw instructions are not a validated compiled plan. Guard
    construction/deserialization/consumption with the same Rust authority;
    validate all supplied instructions before zero omission or aggregation.
    Check date counts, denominator/count conversion, capacities and exact
    aggregation; prepare/reserve rows and stock before commit. Reject duplicate
    targets, missing cadence, impossible targets/repayments and calculation/
    resource failures with their real cause. Inherit002 fallible atomic builder
    updates and003 complete phase/publication rules; no failed partial schedule,
    silent clamp, compatibility mode or builder mutation on failed build.

16. **Reconcile actual callers, hosts and state with these precise laws.** Apply
    the same explicit rule, exact fraction/member rename, B0/live-stock bases,
    actual-date mapping, zero rules and par cash/deltas to Rust/spec/JSON,
    Python/WASM, schemas/stubs/declarations/examples and the parity ledger.
    Validating constructors are real operations rather than literal exclusions;
    supply their existing plain-value WASM counterpart without adding a class.
    Accepted003's Notional::par removal and opening provenance remain mandatory.
    Remove the term-loan adapter's cumulative-funding reinterpretation of the
    original fraction, precomputed cash-date repayment mapping and funded-only
    floating-point clamps; delegate supported generic rules or explicitly reject
    an unsupported product contract, without inventing a new product method.
    Reconcile bond callers and all economic-balance/replay/accrual/fee/credit/WAL
    consumers, including off-coupon reductions. Fee bases, coupon/PIK eligibility,
    prepayment allocation and lifecycle/reporting policies keep their owners.

17. **Qualify the chosen operators independently.** Cover all seven variants,
    zero/one fractions, empty and malformed lists, duplicate/grouped dates,
    exact target retention/failure, PIK/funding/repayment changes, same-date
    permutations, opening-versus-issue bases, independent/duplicate cash dates,
    irregular/cut-dependent cadence, adjusted spillover, None residuals and
    separate automatic redemption. Qualify bounded recurring decimals, exact
    cash/delta conservation, underflow/overflow/unrepresentable differences,
    complete-error atomicity and cross-host/restoration/consumer agreement.
    Actual contractual rounding/prepayment/annuity schedules need their own
    references. The bounded examples below support accepted definitions, not
    production conformance, universal market adoption or vendor equivalence.

## Observed formula and branch inventory — not the accepted target

Let `B` be live principal after same-date PIK and explicit events, `B0` original notional, and `n` the number of distinct coupon accrual boundaries.

| Rule | Prepared amount and emission |
|---|---|
| None | No scheduled principal reduction. |
| LinearTo(F) | Require `0 ≤ F ≤ B0`; equal installment `(B0-F)/n`. On the last boundary replace it with `max(B-F,0)`, so PIK/draws can change the terminal scheduled amount. |
| StepRemaining | Strictly increasing dated targets. At each supplied date pay `B-target`; reject target above the live balance. A target above original principal can be valid after funding/PIK. |
| PercentOfOriginalPerPeriod(p) | Pay `B0 × p` on each boundary, capped at live balance. |
| PercentOfRemainingPerPeriod(p) | Pay `B × p` on each boundary, including that date's PIK/events in the base. |
| LinearBetween(start,end) | End must be a coupon boundary; start must be on/after issue. Count boundaries in `(start,end]`; capture equal installment from balance after all start-date movements. Pay it on each eligible boundary and sweep all remaining principal on `end`. |
| CustomPrincipal | Sum nonnegative same-date supplied amounts in Money; reject an amount above live balance. Zero entries do not create repayments. |

`LinearBetween` adds later Notional draws to the installment only **after** the current date's scheduled payment, dividing the added draw by remaining boundaries strictly after its funding date and on/before end. Draws at the end enter the final sweep. Early repayments reduce live balance; the fixed installment is capped, and final payoff includes new PIK. The installment update specifically recognizes positive `Notional` events, not every possible positive delta kind.

All variants emit at the matching coupon payment date when the economic date is a coupon boundary, otherwise on the raw supplied date. `principal_date` records the economic date. The common emitter caps computed repayment at outstanding, suppresses nonpositive amounts and subtracts it in Decimal. Explicit CustomPrincipal excess and StepRemaining increases are errors rather than silent caps. Any residual is later redeemed if automatic principal exchange is enabled.

## Observed selection, validation and limitations

Schedule-based rules require at least one coupon boundary; no coupon schedule means an error for LinearTo, either percentage rule or LinearBetween. Ambiguous payment dates for the same accrual boundary error. Dated rules must lie within issue/raw maturity and also not after effective terminal accrual. LinearBetween requires a nonempty ordered window and an aligned end. Fee periods do not create amortization cadence.

The prepared arithmetic uses Decimal; percentages and Money inputs cross through checked f64-to-Decimal conversion. No per-period currency-minor-unit rounding or contractual payment-rounding mode is selected here. Changing the number of coupon windows can change equal installment amounts. A calendar-month loan amortizer or annuity schedule is an unimplemented alternative in this builder.

## Independent reference examples and ownership limits

Read-only source/API/consumer reviews inspected spec validation, compilation,
amortization emission, pipeline timing, existing host factories and term-loan
adapters. Independent bounded Fraction/Decimal/date-policy examples on2026-11-08,
without production helpers, checked these accepted targets:

- LinearTo B0=100,F=40,n=3 gives I=20. After external repayment leaves B=50,
  ordinary scheduled payment is10 and retains40; the observed20 would leave30.
  Final B=39 cannot reach F=40 and errors; no artificial funding. Subsequent
  automatic par redemption can separately repay the retained40.
- Original fraction0.25 on B0=100 pays25 each boundary up to available stock;
  it does not become30 after a draw raises B to120. Remaining fraction0.25 on
  100 with no other movements pays25,18.75,14.0625 and leaves42.1875 after3 dates.
- LinearBetween120 over3 due dates pays40/40/40 absent other changes. A net draw60
  before the first due date makes60/60/60, including that date; same-day +60/-60
  leaves40/40/40. With initial100 over2 dates and PIK20 before the first payment,
  it pays60/60. A repayment before a later due date reduces the live reamortized
  payment instead of preserving a stale installment.
- For B=1 over3 dates, core012 bounded payments are
 0.3333333333333333333333333333,
 0.3333333333333333333333333334,
 0.3333333333333333333333333333, summing exactly1. For B=1e-28 and k=2 the
  nonzero quotient rounds to zero at the storage boundary and errors.
- Custom amounts7+5 at one economic date aggregate to12 independent of input
  order;12 against live11 errors, while live12 is repaid exactly. Duplicate
  StepRemaining target dates, currency mismatch and out-of-range zero instructions
  still error.
  Shared cash dates preserve distinct economic movements, and an actual terminal
  beyond raw maturity admits an aligned installment within the selected bounds.

These are small exact/rational numerical and date-policy checks, not executions
of generated production schedules or qualifications of contractual payment
rounding. [QuantLib's amortizing fixed-rate bond interface](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/instruments/bonds/amortizingfixedratebond.hpp)
accepts an explicit notional vector and separately declares French-amortization
helpers. It is evidence of distinct schedule/model choices, not authority for
these seven laws. [Strata's swap NotionalSchedule](https://strata.opengamma.io/apidocs/com/opengamma/strata/product/swap/NotionalSchedule.html)
limits its notional changes to its payment-period boundaries; that scope must not
be transplanted as a universal cadence into this builder's separate accrual and
settlement clocks. No independent vendor/contract conformance claim is made.

## Evidence

- [amortization spec and validation](../../../../finstack-quant/cashflows/src/builder/specs/amortization.rs), [preparation](../../../../finstack-quant/cashflows/src/builder/orchestrator.rs), [emission](../../../../finstack-quant/cashflows/src/builder/emission/amortization.rs), [later-draw update](../../../../finstack-quant/cashflows/src/builder/pipeline.rs).
- Located hand-computed/identity tests in [amortization.rs](../../../../finstack-quant/cashflows/tests/cashflows/builder/amortization.rs): equal principal, geometric remaining balance, maturity residual and issue-date emission. [compiler_regressions.rs](../../../../finstack-quant/cashflows/tests/cashflows/builder/compiler_regressions.rs) locates `linear_between_installments_use_the_balance_entering_the_window`, `linear_between_later_draw_starts_repayment_after_its_funding_boundary` and adjusted-endpoint rejection.
- Consumer inspection includes [term-loan amortization translation](../../../../finstack-quant/valuations/fixed-income/src/term_loan/cashflows.rs), [bond specification forwarding](../../../../finstack-quant/valuations/fixed-income/src/bond/cashflow_spec.rs), [Python factories](../../../../finstack-quant-py/src/bindings/cashflows/builder/specs/amortization.rs) and the [parity ledger](../../../../finstack-quant-py/parity_contract.toml).
- No independent vendor amortization comparison or contract-specific qualification of the complete selected operators was established. Production tests/builds/generators were not run. Bounded independent reference checks do not establish production conformance. The complete package is accepted; runtime/caller/host/schema reconciliation and independent qualification remain pending.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
