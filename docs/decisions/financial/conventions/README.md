# Shared financial conventions

These accepted contracts apply across financial domains within each ADR’s
stated scope. They govern the [methodology catalogues](../README.md#coverage);
acceptance does not certify every implementation or approve the Draft models.

**Review complete:** all six shared convention ADRs are accepted within their
full stated scope as of 2026-11-07. There is no further shared-conventions file
awaiting adoption. Runtime reconciliation and independent qualification remain
open; operation-specific choices remain with their detailed owners.

| Accepted contract owner | Scope | Detailed implementation owners |
| --- | --- | --- |
| [0007](0007-units-and-sign-conventions.md) | Explicit operation-specific units and signs | [Rate and basis-point conversions](../core-conventions/018-rate-units.md), [curve shocks](../core-market/rate-shocks.md), [metric units and identity storage](../valuations-framework/metric-orchestration.md) |
| [0008](0008-financial-defaults-and-cagr.md) | Shared financial defaults and dated-window CAGR | [Analytics wealth and annualization](../analytics/02-wealth-and-annualization.md), [statement dated growth](../statements-evaluation/03-temporal-operators-and-period-windows.md) |
| [0009](0009-missing-and-invalid-data.md) | Invalid, missing and undefined outcomes; concrete performance-panel rules | [Performance panels and wire behavior](../analytics/01-panels-windows-and-wire.md), [distinct scalar-series and fixing APIs](../core-market/scalar-series-and-fixings.md) |
| [0020](0020-numerical-representation-and-rounding.md) | Model numerics and contractual monetary arithmetic | [Money construction, arithmetic, rounding and currency metadata](../core-conventions/012-money-decimal-precision.md) |
| [0044](0044-financial-perspectives-and-boundary-conversions.md) | Specific perspectives and boundary conversions | [Dietz and investor returns](../portfolio-attribution/04-performance-returns.md), [retained earnings](../statements-contracts/12-retained-earnings-reconciliation.md), [property cash flows](../valuations-equity-and-composition/20-property-valuation.md), [SA-CCR collateral](../margin-regulatory-xva/18-saccr-rc-pfe-ead.md) |
| [0045](0045-financial-clocks-and-date-boundaries.md) | Separate clocks, date roles and conversion boundaries | [Relative discounting](../core-market/discount-evaluation.md), [forward projection](../core-market/forward-projection.md), [fixing selection](../cashflows/06-floating-rate-projection.md), [model clocks](../models-analytic/rate-clock-and-projection.md), [bond settlement and clocks](../valuations-bonds/02-settlement-and-clocks.md) |

The detailed records retain their individually stated statuses. All 15
analytics records have accepted methodology; other unadopted choices remain
Draft. Their formulas, selectors, numerical controls and evidence
remain with those source owners; an implementation gap does not reopen an
accepted contract. The related [metric identity contract](../valuations-framework/0010-metric-identifiers.md)
owns attribution, while the [no-hazard bond CS01 contract](../valuations-bonds/0011-bond-cs01-without-hazard-curve.md)
owns the accepted method within its stated scope. The
[bond spread-risk record](../valuations-bonds/16-spread-credit-risk.md) retains
the observed OAS branch and its unresolved scope reconciliation.

ADR 0008 includes the accepted dated-window CAGR default, and ADR 0009
includes concrete performance-panel rules. Their broader default and
missing-data principles remain operation-specific; the
[analytics catalogue](../analytics/README.md) records the relevant implementations.
The CAGR discrepancies in analytics and statements remain implementation work;
that default does not select frequency-based annualization, XIRR or model clocks.
The accepted ADR 0044/0045 amendments settle shared collateral representation
and volatility-clock/information-set requirements. Their implementation and
verification remain with their stated owners; individual fixing phases,
product defaults and aging models are not blanket-approved.

The principles in ADRs 0007, 0008, 0009 and 0020 are settled. The complete
[ADR 0044 collateral-input representation package](0044-financial-perspectives-and-boundary-conversions.md#accepted-amendment-explicit-collateral-components)
was accepted on 2026-11-07: explicit VM/NICA components, one derived total C,
and the existing perspective/sign contracts.
The complete [ADR 0045 volatility-mapping and information-cutoff package](0045-financial-clocks-and-date-boundaries.md#accepted-amendment-volatility-clocks-and-fixing-eligibility)
was accepted on 2026-11-07: clock metadata, active dated admission,
origin/variance and publication-eligibility boundaries alongside the accepted
separate-clock/DF contracts.
Individual instrument defaults, aging models and same-day policies remain
with their existing methodology owners.

The [core conventions catalogue](../core-conventions/README.md) documents
core algorithms and types. It is a source audit, separate from these shared
contracts. The [invariant map](../../INVARIANT_MAP.md) links detailed clauses
and [central follow-ups](../../FOLLOW_UPS.md) track reconciliation.
