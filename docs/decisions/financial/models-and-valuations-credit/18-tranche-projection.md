# MODELS-AND-VALUATIONS-CREDIT-18: Tranche premium, protection, recovery writedown and settlement

- Decision status: **Accepted — complete checked tranche cashflows and settlement package, 2026-11-13**.
- Implementation status: Accepted target. The observed behavior appears below.
- Validation status: Source inspected and independent arithmetic examples executed. Production valuation, dated market comparisons and host parity remain open.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked tranche cashflows and settlement

1. **Require explicit accrual, coverage and settlement terms.**
   Resolve positive original tranche notional and currency, holder
   side, finite signed running coupon in bp, first accrual anchor,
   first covered day and unadjusted maturity `M`. Select either
   the quarterly twentieth ACT/360 profile with a supplied
   Following payment calendar and the accrual geometry of
   [CREDIT-12](12-cds-legs.md#accepted-decision-for-checked-cds-legs-and-quote-basis),
   or fully authored bespoke slices and payment dates with complete
   coupon day-count context. Validate contiguous positive accrual
   slices, ordered payments and payment on or after the last
   accrual day. In the quarterly profile, intermediate accrual
   boundaries meet on adjusted coupon dates and the final payment
   rolls Following from `M`. Both profiles end final accrual at
   `M+1` exclusive and cover `M` through its close. Retain the
   original accrual origin for stubs and elapsed accrual.
   The first covered day is authored in the range from first
   accrual day through `M`. Require a modeled protection settlement
   rule, a separately dated actual upfront when present, and
   a quote settlement date when the quote needs it. Use explicit
   inputs in place of index-prefix recognition, automatic start
   and settlement inference, and calendar fallbacks.

2. **Use one ordered daily series of checked tranche losses.**
   At valuation close `v`, request the required civil closes from
   [CREDIT-17](17-tranche-loss-distribution.md#accepted-decision-for-checked-finite-pool-tranche-losses)
   using one admitted pool state, loss law, source and numerical
   profile. For an active tranche, covered days are from the later
   of `v+1` and the first covered day through `M`, inclusive.
   The preceding close supplies the opening state. If the contract
   excludes pre-start defaults and the resolved valuation-close
   model cannot represent that exposure under CREDIT-17, return
   an unsupported-domain error. Carry distinct cumulative future
   loss and recovery erosion as fractions of original tranche
   notional. Use their checked adjacent differences
   `ΔL_d=L(d)−L(d−1)` and `ΔW_d=W(d)−W(d−1)`.
   Only loss generates protection cash. Both reductions update
   outstanding at close of `d) for subsequent coupon accrual.
   Use the validated outstanding and capacity checks directly.
   Exclude historical erosion from modeled cash and propagate
   model inconsistency and numerical indeterminacy.
   [Brigo, Pallavicini and Torresetti, §5.1](https://arxiv.org/html/0912.5427v3#S5.SS1)
   provides background for valuing tranche legs from expected loss.

3. **Pay modeled protection on the selected dated rule.**
   Project `N ΔL_d` on the buyer axis, where `N` is original
   tranche notional. In the zero-lag profile use the unadjusted
   civil day `d` as the payment and discount date. For a positive
   whole business-day lag, use the named settlement calendar to
   count strictly after `d` and use the resulting date for both
   payment and discount. The modeled erosion day remains `d`,
   even when cash settles after maturity or several days map to
   the same payment date. Keep explicit civil-day timing and
   document its within-day approximation. Remove the coupon-period
   midpoint and the auxiliary index-hazard mean-time proxy.
   An index hazard remains active when it supplies homogeneous
   marginal PDs. A named pool needs no index hazard for this
   timing calculation. The profile models expected protection
   cash and leaves auction and calculation dates for actual events
   to the contractual entitlement state.

4. **Accrue scheduled premium on daily opening outstanding.**
   For a coupon slice `[a,b)`, take the day's year fraction
   `δτ_d` as the difference of cumulative accrual fractions
   from `a` through the closes of `d` and `d−1`, using the
   complete original day-count context and zero before the
   first accrual day. Check that the increments are nonnegative
   and reconcile to the full fraction. If
   `O(d−1)` is opening outstanding as a fraction of original
   tranche notional, the future unit-coupon amount for the day
   is `N O(d−1) δτ_d`. Erosion dated `d` still accrues on
   `d` and reduces premium starting the next day. For a partly
   elapsed period add its authoritative elapsed unit-coupon
   accrual `H` through `v` once. Compute `H` from complete
   dated effective balances and the original clock, or accept
   its resolved equivalent. Current outstanding times elapsed
   time is valid only with established constant exposure.
   Multiply the accrued base by `c=c_bp/10000` and pay it at
   the scheduled coupon date. Accrual through modeled default
   day is already part of that coupon. Separate actual event
   accrual or rebate cash uses its own confirmed due date and
   must reconcile against the coupon basis. Unsupported
   contractual adjustments require resolved cash before pricing.
   [ISDA Global Tranched Transactions STS, §3](https://www.isda.org/a/MMtEE/Part-3-%E2%80%93-Global-Tranched-Transactions-Standard-Terms-Supplement.pdf#page=3)
   describes daily outstanding and contractual event adjustments.

5. **Retain earned, event and upfront cash independently of the live layer.**
   Bind authoritative accrued and earned amounts to coupon periods,
   effective balances and payment state. For a completed coupon,
   require the confirmed amount or its complete resolved earned
   basis and unpaid status. Premium earned through a wipeout can
   still be payable in an unfinished period. Preserve its resolved
   accrued amount and due date. Each confirmed claim carries
   a unique obligation or component ID, the relevant event or
   period, occurrence and effective dates where applicable,
   due date, exact signed amount, currency and settlement status.
   A confirmed net amount must identify and replace its components
   in valuation. For partial payment use the authoritative unpaid
   balance reconciled to booked settlements. Pay positive actual
   upfront from buyer to seller, at its authored date and original
   signed Money amount. Recognized prior loss and recovered
   principal determine exposure. Cash for those events comes only
   from an eligible confirmed obligation. Require trade-specific
   accrual and entitlements to agree with the CREDIT-17 live pool
   at valuation. Refuse unresolved effective changes, overlapping
   accrual or incompatible pending events in the live profile.
   Actual cross-period rebates and separate protection receipts
   preserve their own terms. [ISDA CDX Legacy Tranche STS, §§3–4](https://www.isda.org/a/JgtEE/Part-18-%E2%80%93-CDX-Legacy-Tranche-STS.pdf).

6. **Use one eligibility and relative discount contract.**
   Apply an explicit same-day inclusion choice to all eligible
   unpaid cash. Validate all supplied terms and cash before
   selecting rows. At maturity close or with zero residual
   width, retain every eligible earned coupon, protection claim,
   rebate and upfront with its actual payment state. A rolled
   payment remains separate from the coverage end. Settled cash
   is history. Require explicit resolution of an overdue unpaid
   claim into a dated entitlement before pricing it. Discount
   selected same-day cash at one and each required future row
   by the checked relative factor from `v` to its payment date
   on the admitted discount source, under
   [CORE-CONVENTIONS-016](../core-conventions/016-discounting.md)
   and the [discount evaluation contract](../core-market/discount-evaluation.md).
   The model values expected native cash against a deterministic
   discount curve. Check currency, dates, source admission and finite positive
   factors. Validate the request before returning typed zero
   when no eligible nonzero cash remains. Query only the active
   loss and discount inputs. Missing required state, calendar or
   market data propagates as an error.

7. **Define economic PV and residual quote basis on the same legs.**
   Let `P` be modeled protection PV and `A_dirty` be the
   PV of the unit-coupon basis for the uncompleted live coupon
   stream, including its applicable elapsed `H`. Both are
   on the full trade scale. The buyer's economic PV is
   `P−c A_dirty` plus eligible fixed buyer-signed cash
   outside those legs, including any earned-only coupon, and
   less the PV of actual buyer-paid upfront. Reverse the sign
   once for the seller. Quote the live residual without booked
   upfront, completed earned amounts or confirmed event claims.
   At explicit clean settlement `s>=v`, let `B=D(v,s)`
   and use the elapsed unit-coupon amount `H` belonging to
   that same dirty quote stream. Then `A_clean=A_dirty−B H`.
   Use `P−c A_clean` for the clean mark,
   `(P−c A_clean)/B` for clean upfront cash and
   `(P−c A_dirty)/B` for actual break-even settlement cash.
   Report upfront amounts and their fraction of original
   tranche notional. Report running par as `10000 P/A_selected`
   on the explicit clean or dirty basis with a resolved
   positive annuity and a live covered residual. Exact zero
   protection with a positive annuity gives zero par. Refuse
   an unresolved denominator without an arbitrary floor or
   a spread cap. Compute clean annuity and quote numerators
   with checked differences and scale their allowances before
   division. The unit-coupon PV divided by 10,000 supplies
   the coefficient per coupon bp. [CREDIT-19](19-tranche-risk.md)
   owns risk experiments and their metric conventions.

8. **Bound work and carry the loss allowances into price.**
   Limit the active projection span to 100,000 civil days
   and apply CREDIT-17's supplied aggregate factor and
   conditional-work limits to all required closes. Count
   actual work and check dates and arithmetic before a
   complete result is returned. A budget failure propagates
   without switching to a coarser date grid. For a leg of
   the form `Σ w_d [L(d)−L(d−1)]`, propagate shared loss-node
   allowances using the net coefficients of each node, with
   `w_d−w_(d+1)` at an interior close. Include the end
   coefficients. For premium use the opening-outstanding
   coefficients from its day-count increments and payment
   factors. Scale to the requested native cash, annuity or
   PV units, and account for arithmetic and Money finalization
   separately. Preserve asymmetric allowances where applicable.
   Report the selected timing profile and operational
   qualification. The calculation gives discrete-day expected
   cash under the admitted loss profile. Within-day timing and
   contractual event processing require independent qualification.

9. **Make one native projection authoritative for pricing and export.**
   Produce one validated Rust result for direct, prepared,
   registered and host routes. Retain each signed cash row,
   currency, payment and discount date, event or period identity,
   modeled erosion day when applicable, cash kind and confirmed
   or expected status. Use `ProtectionPayment` for protection
   cash. Keep loss and recovery principal effects as separate
   state or explicitly marked zero-cash events under
   [CORE-CONVENTIONS-015](../core-conventions/015-cashflow-semantics.md).
   Store exact Money for confirmed cash. Apply checked raw model
   products and ordered compensated accumulation, with reporting
   Money finalized at the owner boundary. Expose the row discount
   factor and native raw PV from the same calculation, so dated
   native rows reconcile on that arithmetic basis before display
   rounding and separately applied reporting FX or price shocks.
   Preserve a dated balance and accrual trace for changing
   notionals. Route prospective ladders through these expected
   cash rows, and realized cash and attribution through
   authenticated settlements. Generic hazard cashflow export
   must decline this preweighted projection. Use the product
   trace for pricing explanations and disclose the actual loss
   law, timing, calendar, cash cutoff and sources. Reconcile
   calibration, CREDIT-19 risk, Python, WASM, schemas, declarations,
   parity, docs and fixtures with this Rust path.

## Observed behavior

### Choice and rationale

Tranche PV discounts a common internal projected-row representation, using loss/recovery erosion for premium notionals and an estimated within-period default time for protection. Public cashflow rows omit that internal discount timing.

### Scope and input domain

Running-coupon tranches with optional signed upfront, configurable coupon schedule, realized pool losses and index data. An explicit `start_date` makes settlement `as_of`, bypassing synthetic settlement lag. Standard indices are recognized by case-sensitive CDX/iTraxx/ITRAXX name prefixes.

### Algorithm

1. Standard construction uses quarterly ACT/360, CDS-IMM roll, short-front stub, Following payment adjustment and unadjusted accrual boundaries. Missing explicit start uses the preceding CDS date for CDS-IMM, otherwise settlement; default settlement is one business day for named standard indices and three for bespoke.
2. Generate coupon periods and retain payment dates on/after as-of. Already accrued periods use historical loss/writedown; future accrual ends use the loss curve.
3. Premium is coupon times `[full_accrual × prior_outstanding − (1−f) × remaining_accrual × incremental_erosion]`, floored at zero. Erosion includes both loss and recovery writedown.
4. Protection is incremental expected tranche loss, with opposite premium sign. From interval index-survival endpoints infer constant hazard λ; default-time fraction is `[1/λ − Δ exp(−λΔ)/(1−exp(−λΔ))]/Δ`, clipped to [0,.5]. Invalid, flat or very small hazard intervals use .5.
5. Premium/upfront discount at payment date. Protection discounts at the rounded-day interval fraction, clamped to as-of/end, although its public row date is coupon payment date. A fully wiped tranche retains only an unpaid upfront.

### Selection and controls

Index-name prefixes choose synthetic lag; a supplied calendar must resolve and selects business days, while an omitted calendar uses weekdays only. These name-based rules and explicit-start overrides require adoption alongside the loss model. Coupon sign is PayReceive.sign and protection opposite. Tiny premiums are omitted except during unit-coupon par/spread calculations.

### Conventions

Coupon is basis points divided by 10,000; discount factors are ratios from as-of; upfront must share notional currency. Protection date is an expected timing approximation, not an explicit default-event simulation.

### Limitations, failure and fallback

Public projected schedules discard `DiscountAt::WithinPeriod`, so discounting returned cashflows by their dates does not reproduce PV. Public schedule returns empty when as-of >= maturity, while the pricing path may retain rolled unpaid coupons/upfront. Index-level hazard sets within-period timing even in heterogeneous pricing.

### Alternatives and tradeoffs

Exact survival-weighted protection integration would remove the timing approximation; preserving an explicit expected default date in public rows would make schedule discounting consistent. Neither change is selected by this draft.

## Evidence

Owners: [engine.rs](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/engine.rs), [types.rs](../../../../finstack-quant/valuations/credit/src/cds_tranche/types.rs), [parameters.rs](../../../../finstack-quant/valuations/credit/src/cds_tranche/parameters.rs). [Internal tests](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/tests.rs) and [integration tests](../../../../finstack-quant/valuations/credit/tests/cds_tranche) locate schedule/seasoning evidence; no tests were executed.

[Prepared projection](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/prepared.rs)
locates the private discount date and wipeout shortcut.
[Tranche sensitivities](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/sensitivities.rs)
locate the existing par calculation including upfront.
[Period cash](../../../../finstack-quant/valuations/core/src/metrics/period_cash.rs)
and [generic export](../../../../finstack-quant/valuations/core/src/instruments/cashflow_export.rs)
locate consumers that need confirmed versus expected provenance and the native
tranche discount trace.

### Independent review evidence

- On original tranche notional `1,000,000` and coupon `500bp`,
  take a final ACT/360 slice from January 1 through January 5, 2026,
  inclusive, paying January 7. At January 3 close, opening outstanding
  for the next day is `.80`, established constant over the three
  elapsed days. Future daily loss and recovery increments are
  `(.10,.05)` and `(.05,.10)`. The coupon basis uses `.80`
  on January 4 and `.65` on January 5, for coupon cash
  `534.722222…`. A one-business-day lag on an explicit
  weekdays-only calendar pays the two protection amounts on
  January 5 and 6. Relative factors `.99,.98,.97` on January 5,
  6 and 7 give protection PV `148,000`, premium PV
  `518.680555…` and buyer PV `147,481.319444…`.
- In a heterogeneous full-pool example, use 90 civil days,
  named weights `(.1,.9)`, cash recovery `.4`, annual
  constant member hazards `(20,.01)` and a 10% continuous
  discount rate on an ACT/365F clock. Daily expected loss
  totals `.060896929280` of original pool and discounts to
  `.060586438447`. The loss-weighted payment time is about
  18.70 days. Using an unrelated index hazard of `.02` and
  the rounded day 45 proxy gives `.060150754380`, about
  4.36bp of pool notional lower.
- At maturity close on June 20, 2026, a fully eroded tranche
  with `1,000,000` original notional and coupon `360bp`
  still owes `600` on the rolled June 22 payment date for
  six earned ACT/360 days. It is due confirmed protection of
  `130,000` on June 23 after a settled `50,000`, and a
  confirmed `2,000` rebate on June 24. With relative factors
  `.99,.98,.97` for those three dates and a same-day
  buyer-paid upfront of `20,000`, exact buyer PV is
  `108,746` with same-day inclusion and `128,746`
  with exclusion.
- For a live quote with `N=1,000,000`, elapsed `H=100,000`,
  opening daily outstanding fractions `.80,.70` on the two
  remaining ACT/360 days, coupon payment factor `.98`,
  protection increments `.06,.02` discounted at `.991,.989`,
  and quote factor `.995`, protection PV is `79,240`.
  At `500bp`, clean upfront is approximately `79,508.38`
  and actual break-even cash is `74,508.38`. Their exact
  difference is the `5,000` elapsed accrued premium.
- Production qualification still needs dated contract and
  market comparisons, work and loss-allowance cases,
  maturity and partial-settlement tests, native row/PV
  reconciliation, and host and attribution checks.

## Relationship to the accepted credit-pool boundary

[CORE-MARKET-19's complete accepted package](../core-market/credit-index-composition.md#accepted-decision-explicit-credit-pool-composition-and-exact-issuer-contracts)
separates future pool/member cash recovery, hazard construction metadata and
historical default/recovery state. Resolve observed state or an explicitly
retained historical approximation at its owner. Named pricing currently
consumes index hazard for protection and accrued timing even when issuer
hazards supply expected loss. The accepted daily loss timing replaces
that observed auxiliary role.
Record 19 was accepted on **2026-11-08**, with reconciliation and
qualification pending. The accepted tranche package above owns timing,
settlement and projection.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
