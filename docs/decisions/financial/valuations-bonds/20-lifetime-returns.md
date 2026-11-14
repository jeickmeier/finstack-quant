# VALUATIONS-BONDS-20: Issue-cost MOIC/XIRR and worst-exit lifetime returns

**Decision status:** Accepted — complete checked issue-to-exit return package, including signed-net MOIC and the complete issuer-call minimum, 2026-11-14.
**Implementation status:** Runtime and caller reconciliation pending. Observed behavior is recorded below.
**Validation status:** Source, primary references and independent arithmetic reviewed. Runtime, contractual path, historical coverage and host qualification pending.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked bond issue-to-exit returns

1. **Bind a single explicit original investment.** Resolve the
   whole issue holder claim, original issue funding date and phase,
   native currency, and finite positive invested amount `V0`. Use
   explicit initial-face par, positive percentage of original
   issued face, or positive exact native Money. An authoritative
   observed original issue investment can supply the exact amount.
   Keep initial original face and investment identity separate from
   outstanding, settlement dirty price, market yield and holder
   model PV. Count the identified original funding event exactly
   once. An issue-day fee already represented in `V0` supplies no
   separate payment. Reuse the checked issue-price arithmetic used
   by BONDS-05.
   A contractual floor's cost basis supplies the return metric only
   with an explicit matching investment binding. Reject conflicting
   authorities over the same original investment. Remove implicit
   par and inference from a floor's presence or absence. This is a
   whole-claim, once-funded issue profile. Secondary acquisitions,
   later capital commitments or separately funded positions require
   their own return basis. Expenses and other operations payable on
   the admitted issue claim continue to have their own cash roles.
   [Bond issue basis and floor scope](05-return-floors.md),
   [holder cash and principal](01-schedule-and-holder-cashflows.md)
   and [monetary precision](../core-conventions/012-money-decimal-precision.md).

2. **Prepare one complete signed issue-holder receipt and payment
   ledger per candidate.** Freeze the valuation date, information
   cut and event/payment phase. Use authorized settled cash through
   the cut, or reconstruct it exactly from authoritative settled
   payments, qualified as-known event/fixing history and documented
   coverage. Preserve cash dates, claim identities, paid or unpaid
   state, issue-holder entitlement and native currency. Include
   relevant past cash even when the current secondary holder did
   not receive it. A schedule, current projection curve or lack of
   past rows does not establish settled history. Empty history
   requires coverage. Preserve known unpaid and vested independent
   claims, and label any admitted projected amounts and timing from
   the frozen opening information set. Do not replace paid values
   with new forecasts or discard unresolved required cash. Admitted
   distinct post-funding issue-day cash may enter on that date when
   its legal phase and amount are explicit. Exclude the original
   investment from the post-funding cash and unrelated acquisitions
   by event identity. Count actual signed coupons, contractual fees
   or charges, amortization, and principal or other authorized
   settlement cash on the elected path. Noncash PIK and write-down
   supply no receipt. Capitalized PIK contributes only through
   authorized later cash. Keep common and branch cash associated
   with original claim/payment identities. Do not infer a receipt
   from accrual, coupon eligibility, or nominal balance changes.
   [Information cuts and buyer versus holder cash](02-settlement-and-clocks.md),
   [cash semantics](../core-conventions/015-cashflow-semantics.md)
   and [filed cash-date XIRR terms](https://www.sec.gov/Archives/edgar/data/1500435/000150043526000003/ex103-amendno2farallonagre.htm).

3. **Use signed net receipts for the issue money multiple.** On the
   qualified path report `moic = Σ C_i / V0`, where `C_i` is signed
   post-funding issue-holder cash. Use exact checked native amounts
   and a checked ratio conversion at the result boundary. Later
   charges reduce the numerator and the one original investment
   remains the denominator. Preserve negative and zero net outcomes
   when the complete claim path establishes them. A missing receipt
   is not a zero. Report a multiple, without discounting, annualizing,
   reinvesting, or introducing an undisposed terminal mark. This
   changes the published bond MOIC from positive-only receipts to a
   net cash multiple on one original investment. Apply the same
   law for `moic_to_worst`. BONDS-05 retains the positive contractual
   paid-through credit test for an issuer-call floor. A contractual
   floor can bind even when separately payable signed charges lower
   the reported issue return below its target.

4. **Use canonical dated XIRR on the same signed cash.** Prepend
   the single original investment `(issue_funding_date,−V0)` and
   submit the complete signed dated cash after the accepted checked
   Money conversion. Reuse CORE-CONVENTIONS-017's canonical date
   netting, fixed actual integer days divided by 365, one-sign-change
   eligibility and returned-rate residual check. Preserve past dates,
   leap days, same-date components and the selected native currency.
   The issue-based profile requires the surviving first date to be
   the original issue funding date with negative net cash there.
   An admitted result is an effective annual decimal rate `r>−1`.
   No market yield, floor day count, price compounding, guessed
   root or alternate loss floor. If a fully reconciled path has
   no admissible finite rate, report an unavailable optional result
   or fail an explicit request with the cause. A total wipeout,
   first investment fully offset on its date, a single surviving
   date, unsupported sign changes, arithmetic and solver failures
   each retain their meaning. An independently valid MOIC remains
   reportable. A signed lifetime cash stream and a floor's narrower
   credited cash can differ even on the same ACT/365F clock.
   [Accepted XIRR](../core-conventions/017-irr.md) and
   [Microsoft on 365-day date timing](https://support.microsoft.com/en-us/excel/functions/xirr-function).

5. **Define the scheduled path and the issuer-controlled stress.**
   `moic` and `xirr` are scheduled-path issue returns. Use the
   applicable contract to maturity with no new discretionary exercise,
   or the qualified realized history of that same completed path.
   A binding early termination or earlier completed exit makes
   those maturity-specific results unavailable. `moic_to_worst` and
   `xirr_to_worst` minimize over the scheduled no-new-exercise path
   when available and all still-reachable admitted full issuer-call
   paths on the frozen scenario. The holder policy for the stress
   elects no further voluntary puts. A voluntary put is a holder
   choice. Keep prior irrevocable calls or puts and their claim effects
   in the history. An irrevocably elected or completed redemption
   that establishes the sole terminal path leaves a singleton set
   when its full cash is qualified. A separate elective put scenario
   can be valued on its own admitted cash. Include mandatory dated
   contractual events on the paths where they actually apply.
   Stochastic default, partial or unresolved termination and optimal
   exercise policies require their own admitted scenarios. This
   minimum is a conditional issue-to-exit result and is separate
   from trade YTW, default stress and an expected or guaranteed return.
   [Exact exercise and prior rights](04-rates-tree-exercise.md),
   [workout quote boundary](11-workout-yield.md) and
   [MSRB on holder put rights](https://www.msrb.org/Rules-and-Interpretations/MSRB-Rules/General/Rule-G-12?tab=3).

6. **Qualify every candidate's actual timing and replacement cash.**
   Enumerate the exact authorized dated opportunities or a fully
   qualified complete scenario manifest. Preserve identity of each
   right, its notice and exercise phase, redemption and actual cash
   dates. Omit a right only when adjudicated unavailable or unreachable
   on the elected basis. No arbitrary sample of windows, date-only
   tuple or dependence on the trade YTW candidate set. Share
   qualified rights and cash preparation with BONDS-11 where its
   settlement-buyer and this issue-holder experiment agree.
   Resolve state, stock and legal payment priority at the exact
   phase. Retain vested amounts paid earlier or independently owed,
   including properly admitted same-date or later receipts. Drop
   unearned hold-path cash and terminal principal replaced by the
   exercise. Include the qualified dirty strike once, with payable
   accrued once under the authorized contract. For a protected early
   issuer call apply BONDS-05's checked floor to the exact admitted
   strike and its paid-through receipts. No floor-generated exit,
   synthetic par covenant or automatic protection on a holder put
   or maturity. Future contingent make-whole, accrued, floating or
   floor state needs a qualified scenario before prices are usable.
   Preserve same-date but distinct candidates and distinguish final
   decision date from final settled cash. Admission turns on legal
   reachability under the elected policy. Model exercise thresholds
   and call friction do not determine that reachability or supply
   a settlement payment. Reject unresolved branch state or missing
   path coverage. An unexercised right can be out of reach because
   of a previous binding election.
   [Call-payoff and floor authority](05-return-floors.md) and
   [bond payoff replay](06-joint-coupon-replay.md).

7. **Publish a minimum only over a complete, successful set for
   that measure.** Prepare the complete eligible candidate set under
   explicit qualified work controls. If a required path cannot be
   established, fail that requested minimum. If a required path has
   undefined or failed XIRR, report that failure with the candidate
   identity. Never silently drop it or substitute infinity, zero,
   a guessed rate or the minimum of successfully solved candidates.
   Evaluate minima independently for MOIC and XIRR. Their minimizing
   paths can differ. Preserve exact tied result identities, with
   no tolerance-based merge across different rights or dates. Label
   the admitted endpoint, actual cash span, candidate coverage and
   observed, projected or mixed status of the result. A maturity-only
   scoped result need not enumerate unused issuable call scenarios.

8. **Keep each return measure and its authority through consumers.**
   Keep the four bond keys `moic`, `xirr`, `moic_to_worst` and
   `xirr_to_worst` with the elected semantics, provenance and units.
   Report resolved issue cost authority, currency, historical coverage,
   information/payment phases, scenario and the minimizing path or
   actual terminal identity where applicable. Remove realized-only
   and guarantee-check labels for projected results. Keep holdings
   scaling and cash sign separate from these intensive issue measures.
   Do not sum rate or multiple outputs across positions. A portfolio
   money multiple or XIRR needs its own admitted pooled cash and
   investment basis. Reconcile the selected bond paths, return-floor
   docs and metric metadata, Rust registry and existing Python, WASM,
   wire, UI and reporting paths with checked errors and optional
   availability. Do not extend the bond changes to other producers
   with separate return definitions.
   [Metric identity and result basis](../valuations-framework/metric-orchestration.md)
   and [public metric meanings](../valuations-framework/0010-metric-identifiers.md).

## Observed behavior

### Choice and input basis

These metrics are issue-to-exit contractual returns, not returns on today's quoted purchase price. Cost basis is issue date and return-floor issue-price specification (par by default, otherwise its amount/OID resolution); currency must match and cost must be strictly positive. Lifetime holder cashflows retain past payments, including amounts before `as_of`; floating history therefore depends on available schedule/fixing inputs. Return floors are early-call protection and do not guarantee the held-to-maturity return.

### Algorithms and selection

MOIC sums positive holder amounts strictly after issue and divides by cost. Negative post-issue holder payments are ignored by MOIC. XIRR instead forms `(issue,−cost)` followed by all signed post-issue holder amounts. The live code delegates to the fixed ACT/365F core XIRR without a day-count override. The earlier audit described a return-floor day-count alternative that the live caller no longer supplies.

Worst-exit metrics first lower any floor into effective call rights, build lifetime flows, and enumerate call/put dates from `as_of` onward using the same workout candidates as record 11. For each path retain issue-to-exit flows including same-date distributions and add redemption, including accrued after those distributions. Take the minimum across held-to-maturity and all call/put exits. MOIC again uses only positive amounts. XIRR maturity-path failure propagates, but an early-exit XIRR failure is silently skipped. Therefore the reported worst XIRR is the minimum among successfully solved candidates, not necessarily every contractual candidate.

### Limits, alternatives and evidence

The full schedule is a current contractual projection; retaining past rows does not turn it into a verified realized cash ledger. The maturity path is unfloored; a floor target above natural maturity return can coexist with a lower `to_worst`. Treating voluntary put exits as worst cases follows enumeration semantics and differs from optimal holder action. Skipped candidates limit the coverage of the reported worst return. A current-purchase return, signed-net MOIC, or per-candidate validity result would be different methods.

Owning code: [cost basis, lifetime flows and MOIC](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/return_metrics/moic.rs), [XIRR paths and skip](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/return_metrics/xirr.rs), [exit redemption](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/yield_price.rs), [floor construction](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/return_floor.rs). Inline floor property/mutation tests locate early-exit floor checks, while metric tests in these modules inspect lifetime versus remaining-cash semantics. Core solver validation does not prove economically unique IRRs for arbitrary signed streams. Reference: [Fabozzi](../../../REFERENCES.md#fabozzi-fixed-income-handbook) is general background; exact lifetime/worst-exit convention provenance remains unestablished.

Consumer evidence includes the [static metric definitions](../../../../finstack-quant/valuations/core/src/metrics/ids/instrument.rs),
[metric units](../../../../finstack-quant/valuations/core/src/metrics/ids/unit.rs),
[WASM return-floor tests](../../../../finstack-quant-wasm/tests/facade/return_floor.test.mjs)
and [bond results fixture](../../../../finstack-quant-ui/src/fixtures/results/bond.json).

## Independent arithmetic and qualification

- Fund `100` on 2024-01-01, receive `12` and pay an independent
  charge `2` on 2024-12-31 (actual day `365`), then receive `110`
  on day `730`. Net dated cash is `−100,+10,+110`, so XIRR is
  exactly `10%` and proposed MOIC is `1.20×`. The legacy positive
  filter gives `1.22×`. A single `110` payment on day `366` instead
  gives about `9.971358593414%` with the fixed denominator `365`.
- With cost `100`, maturity cash `5` on day `365` and `105` on
  day `730` gives `1.10×` and XIRR `5%`. An eligible issuer call
  paying `106` on day `365` gives `1.06×` and `6%`. The call
  minimizes MOIC while maturity minimizes XIRR. A voluntary put
  paying `90` on day `365` can be a distinct elected cash path.
  Under the no-new-put policy it does not enter the issuer minimum.
- On a qualified path with `100` originally funded, day-365
  cash `230` and day-730 cash `−132`, the XIRR equation has valid
  mathematical roots at `10%` and `20%`. CORE-CONVENTIONS-017
  rejects the two sign changes. If required, that candidate
  prevents a numeric worst XIRR even when other paths solve.
  Its lifetime net MOIC is `0.98×`.
- With `100` initial par, qualified co-dated senior amortization
  `20` leaves par `80`. A `101` clean-per-100 issuer call with
  payable accrued `4` gives dirty redemption `84.80`. Add the
  co-dated unconditional coupon `3` and amortization `20` once.
  Preserve a separately vested later cash payment `6`: total
  post-funding cash is `113.80` before other amounts. The later
  payment keeps its actual date for XIRR.

Exact rational checks reproduce the signed returns, path minima,
two-switch roots and call cash. A 70-digit calculation confirms
the leap-year rate.

Qualification covers original amount and same-day issue phase,
complete and incomplete historical coverage, signed and same-date
cash, funded-once scope, leap-day annualization and solver failure,
fixed and projected exits, multiple same-date rights, irrevocable
notice, floors and separate charges, PIK and delayed surviving cash,
candidate validity and reporting, signed-loss multiples and host
parity. Independent arithmetic does not qualify the live entitlement
rules, calibration or solver paths.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
