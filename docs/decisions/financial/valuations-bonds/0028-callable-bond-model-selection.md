# ADR 0028: Callable-bond model selection

**Decision status:** Draft — observed implementation; adoption and rationale
await agreement.

**Implementation status:** Observed in bond native and registry dispatch,
rates-only model resolution, and joint rates-credit routing.

**Validation status:** Evidence located and inspected; not rerun. Independent
accuracy and coverage qualification remain incomplete.

**Audit date:** 2026-11-05

## Decision awaiting adoption

The proposed choice covers the public callable-bond model default, supported
rates-only alternatives, and the boundary between rates-only and credit-sensitive
exercise valuation.
This record covers `Bond` call/put schedules, make-whole terms and return-floor
rights. It does not adopt convertible, term-loan or structural Merton methods,
or the separate joint rates-credit model and regression choices.

The detailed owners below define actual dispatch, model inputs, numerical
controls, failure branches and evidence. Their Draft status and this selection
record's Draft status remain independent of implementation availability.

## Observed selection boundary

[Native model selection](03-pv-models.md) treats credit-factor settings, an
attached credit curve and exercise rights as economic inputs when choosing
`RatesCredit`, `HazardRate`, `Tree` or `Discounting`. Explicit registry selection
retains its model semantics; structural `MertonMc` is separate. The detailed
record owns the ordered routing conditions and unsupported combinations.

The public rates-only tree resolver selects Hull–White unless BDT or BK is
explicitly selected. Its default is distinct from the private Ho–Lee
configuration. [Rates-only exercise](04-rates-tree-exercise.md) owns both
configurations, required inputs and supported cashflow domains; a private
configuration is not evidence of public selection or adoption.

A price/yield quote may pin settlement value while the chosen model remains
relevant to risk. OAS is applied within the selected model. As explained by
[PV selection and quote handling](03-pv-models.md), a successful quoted PV does
not establish model calibration or accuracy.

## Canonical methodology owners

| Choice | Authoritative detailed record |
| --- | --- |
| Native/explicit model routing, quote pinning, discounting and hazard recovery | [PV models](03-pv-models.md) |
| Rates-only HW/BDT/BK inputs, clocks, event mapping and exercise | [Rates-only tree exercise](04-rates-tree-exercise.md) |
| Joint deterministic/sample routing and pathwise coupon/principal/recovery replay | [Joint cashflow replay](06-joint-coupon-replay.md) |
| Joint stopping policy, make-whole projection, training/pricing stages and error interpretation | [Bond LSMC](07-lsmc-policy.md) |
| Spread units, quote compounding, OAS inversion and solver failure behavior | [Spread quotes](12-spread-quotes.md) |

Shared numerical owners are the [Hull–White tree](../models-numerical/hull-white-event-grid-and-affine-bonds.md),
[BDT](../models-numerical/bdt-lognormal-curve-fit.md),
[BK](../models-numerical/black-karasinski-trinomial.md) and
[joint marginal calibration](../models-numerical/rates-credit-marginal-calibration.md).
Shared [model-input resolution](../valuations-framework/model-resolution.md)
and [rates-credit configuration](../valuations-framework/rates-credit-grid.md)
remain distinct from the bond's payoff and exercise choices.

## Rationale and alternatives

The source-documented resolver rationale is to share model inputs across PV,
OAS, option decomposition and vega, including the straight comparator leg.
The proposed adoption rationale is to expose rates-only versus credit-sensitive
exercise assumptions and preserve that price/risk consistency. The default,
alternatives and intended uses still require agreement.

HW permits negative normal short rates; BDT/BK use positive lognormal rates,
with mean reversion in BK. A deterministic cashflow calculation omits exercise
choice and is rejected for option-bearing bonds in those routes. Joint
rates-credit valuation adds default/recovery economics and sampling error when
stochastic. These distinctions do not rank or approve the models.

## Failure and evidence boundaries

The public rates-only resolver does not substitute Ho–Lee or a default
volatility for missing required HW inputs. Rates-only calibration and
unsupported-cashflow errors propagate as documented in its owner. The joint
calibration kernel has different observed fallback branches, including a
failed hazard-root solve returning a bracket midpoint; those branches remain
pending qualification in the linked joint calibration record. OAS trial-error
handling is separately documented in the spread owner.

Detailed evidence retains curve-fit and exercise-grid comparisons, BK limiting
cases, model-sensitive credit-risk tests and OAS round trips. These establish
scoped behavior/internal consistency, not arbitrary-window error bounds or
independent callable-credit accuracy. Joint qualification additionally needs
calibration, regression, sampling and convergence evidence. No tests were
rerun for this audit.

The accepted z-spread CS01 direction retains its scope under
[ADR 0011](0011-bond-cs01-without-hazard-curve.md). This selection draft does not
approve the observed option-bearing OAS branch as an exception.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
