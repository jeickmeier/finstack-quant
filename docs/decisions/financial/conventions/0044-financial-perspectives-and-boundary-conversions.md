# ADR 0044: Financial perspectives and boundary conversions

**Status:** Accepted

**Decision date:** 2026-11-05

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
coverage or resolve the remaining collateral-representation and methodology
questions.

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
| Margined SA-CCR replacement cost | Collateral and NICA are signed net amounts, positive when held by the bank. Preserve `RC = max(V - C, TH + MTA - NICA, 0)`, where `C` is total recognized, adjusted net collateral and already includes recognized NICA. Any component inputs must be explicit and non-overlapping; never add NICA again to a total-collateral input. This decision does not choose the exact public-field representation or qualify the full margined/unmargined methodology. |
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

## Follow-up tracking

Implementation gaps, collateral-representation reconciliation, and outstanding
qualification work are owned by the
[sign-conversion follow-up](../../FOLLOW_UPS.md#sign-conversion-boundaries).
Acceptance does not establish compliance or settle the public-field representation.

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
  once the public-field interpretation is resolved.
- Verify option-side, position, attribution, metric-sign, and host-documentation
  agreement through the affected public interfaces.

Existing source and tests are evidence locations, not a certification that
these checks have passed. The catalogue links the real-estate, statements,
corkscrew, and margin evidence owners.
