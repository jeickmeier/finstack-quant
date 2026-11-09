# ADR 0044: Financial perspectives and boundary conversions

**Status:** Accepted

**Decision date:** 2026-11-05

**Amended:** 2026-11-07 — complete explicit VM/NICA component-input package
accepted. Implementation reconciliation and independent qualification remain open.

**Category:** Financial conventions

**Implementation status:** Selected source paths inspected; cross-boundary
coverage is not established; see [follow-up tracking](#follow-up-tracking).

**Validation status:** No numerical or host-parity checks executed for this
record.

## Scope and existing authority

[ADR 0007](0007-units-and-sign-conventions.md) requires explicit financial
units and sign conventions. This decision adopts the specific perspectives and
conversions retained in [INVARIANTS.md section 3](../../../../INVARIANTS.md#3-sign-and-perspective-conventions).
Their clause-level mapping is in [INVARIANT_MAP.md](../../INVARIANT_MAP.md).

Acceptance establishes the contracts below; it does not certify implementation
coverage. The accepted amendment below settles collateral representation;
full margined/unmargined methodology remains with its margin owners.

## Accepted amendment: explicit collateral components

**Accepted:** 2026-11-07.
**Implementation:** Partial; selected source inspected at `827ebb75b9`.
Current Rust calculations use VM plus NICA, but the public `collateral` name
is ambiguous and constructors do not uniformly require both components.
No runtime, regulatory-conformance or host-parity checks were executed.

The accepted perspectives and seven operation-specific boundary rules below
remain authoritative. This amendment settles the remaining shared methodology choice in this
record: the public collateral representation and its conversion boundary.
The full SA-CCR methodology remains with
[margin record 18](../margin-regulatory-xva/18-saccr-rc-pfe-ead.md); this package
does not adopt its other Draft policies.

### One explicit component representation

Choose a single public component form in `SaCcrNettingSetConfig`:

| Canonical field | Meaning |
| --- | --- |
| `variation_margin` | Recognized, adjusted signed net VM, excluding independent collateral. Positive means held by the bank; negative means posted by the bank. |
| `nica` | Recognized, adjusted signed net independent collateral. It excludes VM and is positive when available to offset the bank's exposure. |

Rename the current ambiguous `collateral` field/arguments to
`variation_margin`; remove the old name from constructors, accessors and wire
schemas without an alias, fallback or migration. Keep `nica`. Require both
components explicitly wherever total collateral is formed, including the
unmargined constructor; deliberate `0` means none. Do not silently infer
missing NICA/VM as zero or accept an additional total-C input mode.

Derive total recognized adjusted collateral once in canonical Rust:

```text
C = variation_margin + nica
```

Every consumer of total `C`, including RC and PFE, uses that same derived
quantity. Do not add NICA again to it or repeat subtraction in a different
order in another consumer. Under the already accepted margined RC equation,
NICA also appears separately in `TH + MTA - NICA`; that floor term remains
required and is not an additional addition to `C`.

No new wrapper per perspective or additional collateral/result hierarchy is
selected. The canonical Rust/Python field and JSON/schema key are
`variation_margin`. WASM's existing schema-generated plain configuration uses
the same wire key; ordinary callable parameter names follow the project's
Rust/Python/WASM naming rules. Update these existing surfaces together.

### Amount preparation, admission and caller responsibility

All component amounts and the calculation's `V`, TH and MTA use one explicit
caller-selected reporting currency and compatible monetary scale. Do not
infer currency/percentage units or perform hidden currency conversion at this
boundary. Retain [ADR 0020's monetary/model conversion rules](0020-numerical-representation-and-rounding.md);
this field choice does not turn an f64 model calculation into Decimal-exact
regulatory reporting.

Both components must be finite signed amounts. Negative VM/NICA is supported;
do not take absolute values, clamp posted amounts to zero or infer the
perspective from signs. Validate supported construction, wire restoration and
calculation entry points before the components are used. A nonfinite component
is a request error. Unrepresentable derived total `C` is a calculation error,
not infinity, zero collateral or missing data. Propagate errors through the
existing Rust/Python/WASM mechanisms without adding a status framework.

The caller supplies the recognized, adjusted components after the applicable
recognition, eligibility, custody, haircut and currency treatment. The names
define non-overlap; numeric values alone cannot establish whether a caller
mislabelled total collateral as VM. Do not claim automatic detection of such
an economic classification error or infer NICA from a raw posted-IM balance.
Preparation/provenance and full margined/unmargined treatment remain with
the existing margin owners, including the pinned recognition rules below.

### Examples, scope and implementation reconciliation

All amounts below share one reporting currency and use the already accepted
margined formula `max(V - C, TH + MTA - NICA, 0)`:

| V | VM | NICA | TH | MTA | Derived C | RC |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 100 | 20 | 10 | 5 | 2 | 30 | 70 |
| 100 | 80 | 10 | 20 | 5 | 90 | 15 |
| 100 | -20 | -5 | 5 | 2 | -25 | 125 |

In the first case, passing total `30` as though it were VM and adding NICA
again would incorrectly use `C = 40` and yield RC `60`. That is a misclassified
input, not something an amount-only API can discover from those numbers.
These examples are independently derived arithmetic, not executed estimator
tests or regulatory qualification.

Reconcile the existing
[config/constructors](../../../../finstack-quant/margin/src/regulatory/sa_ccr/types.rs),
[RC](../../../../finstack-quant/margin/src/regulatory/sa_ccr/replacement_cost.rs),
[PFE](../../../../finstack-quant/margin/src/regulatory/sa_ccr/pfe.rs), engine,
Python/WASM declarations and docs, parity, schemas, invariants, consumers and
fixtures together when implementation is authorized. Qualify held/posted and
zero components, missing/legacy wire keys, invalid/overflowing values, shared
C consistency and both RC branches across interfaces.

This package does not reopen Dietz/XIRR signs, CapEx/dividend magnitude rules,
corkscrew conversions, option sides or long-holder attribution. Their observed
implementation gaps remain reconciliation work. It does not settle collateral
eligibility/haircut algorithms, margin classification, PFE/add-on methodology,
full unmargined classification/recognition or the EAD cap/decomposition. Those remain decisions and
qualification in the margin records. This review changes existing records only.

## Decision

Interfaces name the perspective of each signed amount. Moving an amount
between perspectives requires an explicit conversion. The following are
conceptual labels; this decision does not require a new Rust wrapper for
every amount.

| Perspective | Positive means | Applicable use |
| --- | --- | --- |
| `EconomicCashFlow` | Cash received by the entity or holder being valued | NPV, XIRR, DCF, instrument and Monte Carlo cash flows |
| `PortfolioExternalFlow` | Client contribution into the portfolio | Modified Dietz and portfolio performance |
| `StatementMagnitude` | Non-negative magnitude; the formula supplies direction | CapEx, opex, tax, dividends, increase/decrease buckets |
| `RollForwardActivity` | Signed additive change in `ending = beginning + sum(change)` | Corkscrews and additive reconciliations |
| `RegulatoryCollateral` | Net collateral held by the bank | SA-CCR variation margin and NICA |

`AmountOrScalar` separates currency amounts from unitless scalars; it does not
identify any of these perspectives.

Adopt the following operation-specific rules without inferring a universal
sign convention for other calculations:

| Boundary | Accepted contract |
| --- | --- |
| Dietz to XIRR/MWR | Contributions are positive portfolio external flows and negative economic cash flows: `xirr_amount = -dietz_amount`. |
| Real-estate CapEx to pricing cash flow | The schedule contains non-negative outflow magnitudes. The pricer negates exactly once. Negative, already CFS-signed schedule amounts are invalid and must be rejected by `RealEstateAsset::validate()`. |
| Retained earnings and equity corkscrew | Retained-earnings dividends default to `MagnitudePositive`: a dividend of `+10` contributes `-10`. An explicitly selected `InflowPositive` convention accepts signed additive dividends: a dividend of `-10` contributes `-10`. Never infer the convention from the value or apply `abs`. Equity reductions in a corkscrew are negative additive activity. |
| Increase/decrease buckets to corkscrew | Both buckets are positive magnitudes; conversion contributes `+increase` and `-decrease`. |
| Margined SA-CCR replacement cost | Collateral and NICA are signed net amounts, positive when held by the bank. Preserve `RC = max(V - C, TH + MTA - NICA, 0)`, where `C` is total recognized, adjusted net collateral and already includes recognized NICA. Any component inputs must be explicit and non-overlapping; never add NICA again to a total-collateral input. The accepted 2026-11-07 amendment chooses explicit `variation_margin`/`nica` components; full margined/unmargined methodology qualification remains separately owned. |
| CDS option side | Call means payer protection; Put means receiver protection. |
| Portfolio position and attribution | Positive quantity means long; attribution P&L is positive for a gain to the long holder. Sensitivities retain the signs specified by the applicable `MetricId`. |

Binding documentation states the same polarity as Rust. `PayReceive` uses
`pay`, `receive`, or an explicit protection side; avoid asset-class-ambiguous
`long`/`short` aliases. This naming rule does not change the positive-quantity
meaning of an actual portfolio position.

## Rationale and alternatives

An amount can be valid in currency and scale yet reverse the economic result
when its perspective is lost. Naming the boundary and conversion owner makes
single-negation and additive identities reviewable.

| Alternative | Tradeoff |
| --- | --- |
| One sign convention for every domain | Simplifies labels but conflicts with distinct statement-magnitude, portfolio-contribution, and holder-cash-flow meanings. |
| Infer perspective from the sign or value | Cannot distinguish a valid negative activity from a wrongly signed magnitude. |
| Require a new wrapper for every quantity | Could encode more distinctions but adds API and conversion complexity without demonstrating a need at each boundary. |

Use named helpers or types where they protect a high-risk boundary, following
[ADR 0037](../../standards/0037-simple-modular-implementation-and-reuse.md) and
[ADR 0041](../../standards/0041-type-invariants-and-validation-boundaries.md).

## Reference basis

The accepted sign and total-collateral distinction follows the pinned
[Basel Framework CRE 52.10–52.19](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/52/inforce/2019-12-15/published/2020-06-05):
unmargined replacement cost uses adjusted net collateral; margined collateral
includes net variation margin and recognized independent collateral, while NICA
also appears in the threshold/minimum-transfer floor. Recognition and adjustments
must respect the applicable collateral treatment, including the exclusion of
bank-posted segregated bankruptcy-remote independent collateral from NICA.
This source supports the definitions and formulas; it is not a certification of
current regulatory compliance or of the implementation.

The reference is the deliberately pinned 2019/2020 edition used by this ADR;
the BIS page identifies it as superseded. The current review uses it for the
existing definitions, not as a claim of current jurisdictional applicability.

## Follow-up tracking

Implementation gaps, collateral-representation reconciliation, and outstanding
qualification work are owned by the
[sign-conversion follow-up](../../FOLLOW_UPS.md#sign-conversion-boundaries).
Acceptance settles the public-field representation; it does not establish compliance or close implementation/qualification work.

## Verification required

- Test contributions and withdrawals through Dietz/XIRR conversion, and
  statement/DCF and roll-forward/corkscrew boundaries where exposed.
- Reject negative CapEx magnitudes; verify that a positive scheduled outflow
  reduces holder cash flow exactly once.
- Exercise both explicitly selected dividend conventions, negative-magnitude
  enforcement, equity reductions, and increase/decrease conversion with values
  that expose double negation.
- Check positive and negative held collateral against the accepted formula,
  with total and component representations that expose NICA double counting,
  under the accepted explicit component representation.
- Verify option-side, position, attribution, metric-sign, and host-documentation
  agreement through the affected public interfaces.

Existing source and tests are evidence locations, not a certification that
these checks have passed. The catalogue links the real-estate, statements,
corkscrew, and margin evidence owners.
