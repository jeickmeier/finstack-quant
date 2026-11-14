# MODELS-AND-VALUATIONS-CREDIT-20: Exchange and liability-management arithmetic

- Decision status: **Accepted — complete checked exchange and liability-management package, 2026-11-13**.
- Implementation status: Accepted target. The observed behavior appears below.
- Validation status: Source and issuer filings inspected. Independent arithmetic examples executed. Runtime and host qualification remain open.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked exchange and liability-management arithmetic

1. **Compare two supplied values for the same holder position.**
   Require one identified old position and common valuation date and
   currency. Compare the supplied holdout PV `H` with the supplied
   consideration for an accepted tender of the same claim quantity.
   Admit nonnegative Money for holdout, replacement claims, separate
   tender cash and equity or warrants, using explicit zero for an
   absent component. The separate cash input is its PV on the common
   date, including any separate eligible consent fee or accrued
   interest. Value note-form premiums as part of replacement claims
   if already included there. Count each entitlement once, and include
   proceeds available when holding out in `H`. The caller resolves
   pricing, proration, eligibility, conditions, timing, FX and the
   cashflows on any untendered balance before supplying the comparison.
   Keep the exchange description as provenance. Par-for-par, discount,
   uptier and downtier labels do not select a valuation or recovery law.
   [Filed exchange terms](https://www.sec.gov/Archives/edgar/data/1776661/000119312526041901/d800674dex101.htm)
   illustrate cash, new-note principal and separate accrued interest.

2. **Report the PV gain and use an explicit decision hurdle.**
   Compute total tender PV `T` as the exact sum of the three admitted
   tender components and signed gain `T−H` in the holder's currency.
   Require a nonnegative minimum gain in the same currency. Report
   whether `T−H > minimum_gain`, using exact Decimal comparison.
   Equality does not clear the hurdle. A literal zero hurdle tests for
   strictly positive gain. Apply the same rule at zero holdout PV.
   Retire the fixed 2% hurdle, the exported constant and the tender
   recommendation label. This result expresses the caller's stated
   rule for the resolved comparison.

3. **Name the break-even PV and any face-value denominator explicitly.**
   The break-even holdout PV is `T`, the value that makes the two
   alternatives equal before applying the caller's hurdle. If the
   caller supplies positive old claim par in the same currency and for
   the same quantity, report the uncapped ratio `T/old_par` as tender
   PV per unit of old par. Without par the ratio is unavailable.
   Reject supplied zero or negative par. Retire the capped
   `breakeven_recovery` and its zero-holdout sentinel. A future cash
   recovery assumption requires its own dates, discount basis and
   claim analysis. Keep those economics with the recovery owner.

4. **Use transaction-specific issuer inputs.**
   Use a strict tagged input and matching result for the modeled
   transaction. For open-market repurchase and cash tender require
   positive target par `N`, accepted principal fraction `a∈[0,1]`
   after proration, and positive cash purchase price fraction `p`
   per unit of accepted par. The selected scenario extinguishes
   accepted debt. Report participating and retired par `Q=N a`,
   remaining target par `N−Q`, purchase consideration `Qp`, and
   signed par less purchase consideration `Q−Qp`. When `Q>0`,
   report the price discount fraction `1−p`. It is negative above
   par. With zero participation, amounts attributable to acceptance
   are zero and the realized discount fraction is unavailable.
   Purchase consideration represents the stated price component,
   with separate accrued interest, external fees and financing
   outside this arithmetic.

   For amend-and-extend require positive target par, participating
   principal fraction in `[0,1]`, and a nonnegative fee fraction
   per unit of participating par. The fee cash is participating
   par times the fee fraction. Debt stays outstanding and purchase
   discount is unavailable. Use the literal fraction units and
   checked range in place of the price ceiling of 1.5 and the
   extension fee ceiling of .10.

   For a transfer-only dropdown require a named asset basis and a
   transfer fraction in `[0,1]`. Report that fraction of the
   stated basis. The branch represents an asset transfer with
   zero modeled cash and debt retirement. No debt participation
   or overloaded price field is admitted. A holder impairment,
   value dilution or combined refinancing requires separate inputs
   and analysis. [J.Crew's annual filing](https://www.sec.gov/Archives/edgar/data/1051251/000156459018006853/jcg-10k_20180203.htm)
   reports a specific IP ownership fraction and separately reports
   related license, repayment and financing transactions.

5. **Compute gross leverage only from an explicit issuer balance.**
   For debt-targeting scenarios, an optional leverage block requires
   pre-transaction gross debt `D` and strictly positive EBITDA `E`,
   with common currency, date basis and issuer reporting perimeter.
   Gross debt includes the target par once and must be at least
   `N`. In this isolated scenario hold EBITDA and other debt
   fixed. With retired par `Q`, report `D−Q`, pre and post
   gross debt/EBITDA `D/E` and `(D−Q)/E`, and reduction
   `Q/E` directly. Amend-and-extend has `Q=0`, so its modeled
   gross leverage is unchanged. Absent leverage inputs and the
   transfer-only dropdown have no leverage result. Invalid supplied
   EBITDA, inconsistent perimeter inputs and unrepresentable
   requested results fail. Do not silently turn them into absence
   or zero. Net debt, liquidity, accounting gains, funding and
   covenant adjustments are outside the isolated calculation.

6. **Use checked currency and Decimal arithmetic.**
   Apply [CORE-CONVENTIONS-012](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity)
   for exact Money admission, currency equality, exact sums and
   differences, checked Decimal-scalar transformations and labelled
   dimensionless ratios. Admit exact decimal fractions for transaction
   terms. Preserve currency, exact zero, signed amounts and the
   distinction between unavailable and zero. For issuer cash
   transformations use the checked participating principal as the
   common basis for purchase or fee consideration, and reconcile
   remaining and retired principal on that same basis. The price
   discount fraction uses the supplied price before Money rounding.
   Fail on a required overflow, underflow or unresolved economic
   boundary. In particular a positive participating fraction below
   one must produce positive participating par below target par.
   Validate the whole scenario, including price and leverage inputs,
   before a zero-participation shortcut. Preserve the source
   arithmetic through wire restore and host inputs. If a positive
   retirement at a price different from par loses the sign of the
   price difference in the monetary outputs, report a precision
   error. Apply any
   required contract rounding explicitly, and keep display
   rounding outside the calculation.

7. **Use the existing Rust and host routes for the resolved arithmetic.**
   Validate direct construction, analysis and strict wire restore
   through one Rust contract. Reject irrelevant or unknown fields
   on the tagged issuer branches. Keep exact monetary values and
   scenario provenance through the existing Python and WASM
   adapters, generated schemas, and UI worker and cards. Reconcile
   dataframe columns, parity, fixtures and restructuring examples.
   Replace the old fixed-hurdle query and overloaded inputs in
   consumers. Keep security pricing, execution, legal eligibility,
   creditor priority, participation strategy and rating/default
   assessment with their owners. The comparison and issuer effects
   are arithmetic for the supplied assumptions.

## Observed behavior

### Choice and rationale

Exchange-offer comparisons use supplied PVs and a fixed 2% tender hurdle; LME scenarios apply explicit balance-sheet arithmetic. These functions do not price the securities or legal priority changes.

### Scope and input domain

Nonnegative supplied exchange PVs/fees/equity sweetener; positive LME notional and acceptance fraction [0,1]. Repurchase price is a fraction of par; its meaning changes by LME type.

### Algorithm

1. Tender total=new PV+consent fee+equity sweetener; delta=tender total−old PV. Breakeven recovery=min(tender total/old PV,1), or one if old PV is zero. Recommend tender only when total exceeds 1.02×old PV.
2. Repurchase/tender retires notional×acceptance and costs retired par×price (price in (0,1.5]). Amend-and-extend retires no par and costs participating notional×fee (fee in [0,.10]). Dropdown records transferred-asset fraction in [0,1] as holder impact but no cost or debt retirement.
3. Discount capture=par retired−cost and fraction divides by retired par, zero if none. Positive optional EBITDA yields pre/post leverage and reduction from debt retirement.

### Selection and controls

ExchangeType is descriptive; no branch changes exchange arithmetic. LmeType chooses the scenario formulas. The 1.02 tender hurdle is a constant, not caller policy input.

### Conventions

PVs/fees and notional/EBITDA require consistent monetary units; price, acceptance and holder impact are fractions despite pct field names. Breakeven recovery is a ratio to old PV, not to claim par.

### Limitations, failure and fallback

Optional EBITDA is filtered only by >0, so positive infinity can produce zero ratios. Uptier/downtier labels do not model priority, dilution or participating/nonparticipating claims. Amend-and-extend discount capture is negative fee cost.

### Alternatives and tradeoffs

Pricing new/old instruments and passing their PVs is supported composition; issuer-strategy optimization and legal restructuring simulations are unimplemented.

## Evidence

Owner: [liability_management.rs](../../../../finstack-quant/models/src/credit/liability_management.rs) and [liability_management.rs tests](../../../../finstack-quant/models/tests/liability_management.rs) with formula/domain cases. No independent rationale for the 2% recommendation hurdle was established.

Existing [Python terms and presentation](../../../../finstack-quant-py/src/bindings/models/credit/liability_management.rs),
[WASM adapter](../../../../finstack-quant-wasm/src/api/models/credit/liability_management.rs),
[schema registry](../../../../finstack-quant/models/src/schema.rs),
and [exchange card](../../../../finstack-quant-ui/registry/models/components/liability-management-card/exchange-offer-card.tsx)
locate the observed public fields and hurdle presentation.

### Independent review evidence

- Holdout PV `45`, replacement PV `80`, tender cash PV `2`,
  and old par `100` give tender PV and break-even holdout PV
  `82`, gain `37`, and tender PV per old par `.82`.
  A tender PV of `103` against the same par gives `1.03`.
  With holdout `100`, tender PV `102` and minimum gain `2`,
  the strict hurdle is not met. With zero holdout, tender PV
  `10` and zero hurdle it is met without dividing by holdout PV.
- With holdout and replacement PV both `10^16`, tender cash
  PV `1`, and zero hurdle, exact arithmetic retains the gain
  of `1`. The current binary64 sum rounds the gain to zero.
- Target par `200`, acceptance `.40` and price `.60`
  retire `80`, leave `120` and pay `48`, for a signed
  par less consideration of `32` and discount fraction `.40`.
  Price `1.60` instead pays `128` and gives `−48` and
  `−.60`. With pre gross debt `500` and EBITDA `100`,
  gross leverage is `5.0x` before and `4.2x` after either
  repurchase. Zero participation retires and pays zero.
- Extending `.75` of target par `500` at fee `.02` costs
  `7.50` and leaves target par `500`. The observed
  positive-only filter admits infinite EBITDA and produces
  zero multiples. The observed calculation can also overflow
  with finite notional and price. These distinguish invalid or
  unresolved results from an explicitly absent leverage request.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
