# ADR 0045: Financial clocks and date boundaries

**Status:** Accepted

**Decision date:** 2026-11-05

**Amended:** 2026-11-07 — complete volatility-mapping, dated-use, variance-
conversion and fixing-eligibility package accepted. Implementation
reconciliation and independent qualification remain open.

**Category:** Financial conventions

**Implementation status:** Shared date and discount-factor primitives located;
pricer adoption, volatility metadata, and operation-specific defaults remain
partially traced.

**Validation status:** No runtime pricer or host-parity checks executed.
Illustrative arithmetic was checked independently on 2026-11-07; it does not
qualify the implementation or a product model.

## Scope and existing authority

[ADR 0007](0007-units-and-sign-conventions.md) protects explicit conventions;
[ADR 0008](0008-financial-defaults-and-cagr.md) protects financial defaults and
accepts Actual/365 Fixed for dated-window CAGR. This decision adopts the
separate clock and date-boundary rules retained in
[INVARIANTS.md section 4](../../../../INVARIANTS.md#4-date-day-count-and-time-conventions).
Their clause-level mapping is in [INVARIANT_MAP.md](../../INVARIANT_MAP.md).

Acceptance establishes the clock and boundary contracts below. It does not
certify implementation coverage, accept an instrument model, choose a universal
option day count, or change a default. The CAGR decision does not redefine frequency-based
annualization, XIRR, or an instrument's model clock.

## Accepted amendment: volatility clocks and fixing eligibility

**Accepted:** 2026-11-07.
**Implementation:** Not reconciled; selected source inspected at `827ebb75b9`.
Volatility artifacts have numeric expiry grids but no complete persisted
calibration clock. Dated callers, calibration/materialization, fixing eligibility
and existing bridge enforcement require reconciliation. No runtime or host
checks were executed.

Keep the accepted clock roles, date-based projection/relative discounting and
finite-positive DF bridge below. This package settles the remaining shared
representation, missing/conflicting mapping and information-set requirements.
It does not adopt every instrument's day-count default, aging model, same-day
cutoff or expiry payoff. Those remain with their existing methodology owners.

### One calibration-clock representation and truthful producers

Add one data-only core `VolatilityClock` reused by existing volatility sources:

```text
VolatilityClock {
    reference_date: Date,
    day_count: DayCount,
    context: DayCountContextState,
}
```

The reference date is the calibration origin for the numeric expiry axis.
The declared basis and required context describe that axis and the time unit
of its annualized volatility. Reuse existing validated date/day-count types,
[DayCountContextState](../../../../finstack-quant/core/src/dates/daycount/context.rs)
and calendar resolution; do not add another calendar or clock framework.
The [QuantLib date-to-time definition](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/termstructure.hpp)
provides primary context for reference-date/day-count mapping; it does not
choose this API's missing-metadata or aging policy.
An empty context is sufficient only for conventions that need no extra context.
Required calendar, reference-period, frequency or other context must be supplied
under the chosen convention, not inferred from coupon, discount or observation
clocks. A calibration mapping outside this representation's supported domain
cannot be silently labelled as another convention.

Use `expiry_clock: Option<VolatilityClock>` on existing `VolSurface`, `VolCube`,
`FxDeltaVolSurface` and fixed-expiry cube artifacts. The clock concerns option
expiry/volatility time, not the underlying swap-tenor axis or contractual coupon
accrual. Pure numeric-time construction/interpolation may omit it: the caller
then supplies coordinates in the source's declared numeric units. Such a source
does not acquire a dated interpretation automatically.

Dated calibration producers record the mapping actually used. Preserve it through
serialization/restoration, clones, quote shocks, calibrated outputs and derived
surfaces/cubes/slices. Do not retag numeric pillars with a different origin or
basis, invent metadata from a convenient default, or treat metadata attachment
as proof that the original dated calibration is consistent. Date-based producer
pipelines must construct coordinates from the recorded mapping. Recording an
existing pipeline's observed mapping does not approve its other Draft defaults
or financial methodology.

### Resolve active source clocks explicitly

Resolve the actually used volatility source first. Its metadata governs its
date-to-expiry lookup. If absent, a complete explicit source-clock contract
supplied by the caller or owning model may supply the mapping. That contract
must identify the actual calibration origin, basis and required context; a
hardcoded model day count alone does not establish them.

When both metadata and an explicit declaration describe the same source,
require agreement on the validated, normalized reference date, convention and
context. Otherwise error; do not silently override either or infer equivalence
from matching a single expiry. Unresolved/missing mappings and invalid required
context are request errors before a dated lookup. No universal Act/365F fallback,
borrowing of a curve/coupon clock or legacy inferred-origin path is selected.

A separately declared **model** clock may differ from the source clock. That
is not a conflicting source declaration; it needs the explicit parameter/time
conversion supported by that operation. A scalar-volatility path with its own
explicit model/source-unit contract does not inspect an unused surface or acquire
its metadata requirements. Do not require volatility data in a lifecycle branch
whose accepted operation contract does not use it.

All dated volatility consumers must use this resolution contract; continuing to
compute an unverified scalar expiry and pass it to a numeric lookup is not a
date-based fallback. A future contractual expiry that maps to zero/nonfinite or
negative required time is an unsupported clock/date calculation, not automatic
expiry or permission to floor time. Actual zero-time/expired branches remain
explicit operation contracts based on their relevant dates and state.

### Calibration origin, valuation date and variance conversion

For the simple direct dated-pricing path, require the effective source calibration
reference date to equal valuation `as_of`. A past or future source origin requires
an explicitly selected and owned rolling/recalibration/model contract. Do not
silently replace the origin with `as_of`, shift the expiry grid, or query an old
surface using remaining time as though it were calibrated at the new date.
This accepted 2026-11-07 admission boundary does not prevent an operation
from explicitly selecting and qualifying a seasoned-volatility model.

Keep source lookup coordinates and model remaining time distinct. Where an
operation explicitly uses a variance representation `sigma^2 * time`, preserve
that quantity over the **same physical interval** `[v, T]` when changing bases:

```text
delta_source = tau_source(T) - tau_source(v)
delta_model = tau_model(T) - tau_model(v)
sigma_model = sigma_source * sqrt(delta_source / delta_model)
sigma_model^2 * delta_model = sigma_source^2 * delta_source
```

Both coordinates use their fixed declared reference mappings. Require finite
positive interval times for this bridge, preserve zero volatility, and evaluate
adequately without unnecessary overflow/underflow in squared intermediates.
This adopts variance preservation for operations supporting that representation;
it does not authorize automatic rescaling of every volatility/model parameter
or assert equivalent stochastic dynamics.

In particular, selecting `W(origin,T) - W(origin,v)` from an older implied-volatility
surface chooses forward-variance/aging and smile assumptions. Metadata and existing
origin-to-expiry total-variance interpolation do not select that model. A dated
caller must use its independently owned aging contract or require a current source.
The [separate forward-variance operations in QuantLib](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/termstructures/volatility/equityfx/blackvoltermstructure.cpp)
are primary context for that distinction, not a selected finstack aging method.
The accepted DF bridge similarly preserves a factor, not full model dynamics.

For one 30-calendar-day interval, source Act/365F volatility `20%` corresponds to
approximately `19.8625%` on an Act/360 model clock under this variance bridge:
`0.20 * sqrt(360/365)`. Both give integrated variance `0.04 * 30/365`.
This is independent arithmetic, not executed pricer/model qualification.

### Publication-aware fixing eligibility without a universal cutoff

Keep the rate's observation/value date as its fixing lookup key. Determine
eligibility using the index publication convention and the operation's explicit
valuation information cutoff/phase, not only `fixing_date < as_of` or presence
in a date-only series. Required published observations missing from the configured
source are errors. An observation not yet eligible must not be consumed; project
only where the operation explicitly permits projection, otherwise error.

For example, [SOFR](https://www.newyorkfed.org/markets/reference-rates/additional-information-about-reference-rates)
and [SONIA](https://www.bankofengland.co.uk/markets/sonia-benchmark/sonia-key-features-and-policies)
publish after their rate value dates. A past value date can therefore still be
unpublished at a valuation cutoff. Their publication schedules illustrate the
distinction; this decision does not choose a universal market hour.

Each operation declares its same-day phase, relevant publication calendar/time
zone where needed, and treatment of revisions. Callers/producers supply a snapshot
consistent with that information set. A latest unversioned date-only series cannot
be described as historical point-in-time publication evidence; data presence does
not authorize use of a later publication/revision. Do not consult wall-clock time
or fabricate publication timestamps from value dates. No universal timestamp,
snapshot store or default same-day policy is introduced by this shared package.
The distinct cashflow, rate-specification, cap/floor and overnight conventions
remain decisions in their existing owners, not implicitly adopted here.

### Existing bridge enforcement, outputs and qualification

Enforce the already accepted finite-positive factor/time contract in
`zero_rate_from_df`: invalid required inputs and unrepresentable required results
are errors. Its current `t <= 0 -> 0` branch and claim of a universally correct
zero-rate limit require reconciliation. A zero-time DF alone does not define a
unique model zero rate; operation lifecycle logic runs under its own contract
instead of using a fabricated rate. Do not turn this enforcement into a new
universal expiry payoff or select additional pricing defaults.

Persist and validate the clock type through existing core and host/state/schema
surfaces. Reconcile dated callers, calibration, transformations, snapshots,
Rust/Python/WASM docs/types/parity and fixtures together when implementation is
authorized. Preserve the existing numerical/reference-evidence contracts; no
compatibility fallback, parallel volatility framework or new result/status
hierarchy is selected.

Independent qualification must cover missing/conflicting active mappings, inactive
sources, actual origin versus `as_of`, context resolution, positive-calendar-span
zero-clock cases, both endpoints of basis conversion, quote/model units, preserved
metadata, same-day and next-day publications/revisions, and strict bridge errors.
Full product models, interpolation/arbitrage policies, calibration defaults and
aging dynamics remain with their existing Draft owners. This review changes
existing records only.

## Decision

### Separate clock roles

A single instrument can require distinct clocks. Do not derive one clock's
convention from another merely because they occur in the same calculation.

| Clock | Accepted contract |
| --- | --- |
| Discount | Obtain the exact relative discount factor using curve dates and the discount curve's own base date and day-count convention. |
| Projection | Obtain projection growth through the curve's date-based APIs using its base date and clock, then express the forward on the contractual index accrual fraction. The curve clock does not replace contractual annualization. |
| Accrual | Compute coupon accrual using the instrument or leg day count. |
| Option/volatility | Use the calibrated date-to-expiry mapping: reference/origin date and time basis, including a calendar where relevant. Surface metadata or an explicit model contract supplies that mapping. |

An accrual convention such as Act/360 does not establish the option clock.
Use the surface-declared or explicitly documented calibration mapping. Each
rate-option, equity, FX, or commodity default needs evidence at its actual
selection path; none becomes a universal clock through this decision.
The accepted 2026-11-07 amendment above chooses `VolatilityClock` and the
active-source metadata/explicit-contract, omission/conflict and origin rules.
Missing metadata does not establish an Act/365F or other universal fallback.

### Preserve discount factors across model clocks

Use the exact curve discount factor directly whenever the model permits.
For a curve based at `b`, valuation at `v`, and payment at `T`, obtain the
relative factor from the same curve's date-based values:

```text
df_relative = D(b, T) / D(b, v)
```

The curve factors and resulting relative factor must be finite and positive.
When a single-time model instead needs a continuously compounded rate for
finite positive model time `t_model`, bridge from that relative factor:

```text
r_model = -ln(df_relative) / t_model
exp(-r_model * t_model) = df_relative
```

This mathematical identity preserves the discount factor while expressing
the rate on the model's time basis. Numerical verification uses justified
floating-point tolerances under
[ADR 0042](../../standards/0042-numerical-accumulation-and-comparisons.md).
The bridge does not make model time equal to the curve's year fraction or
establish equivalence of full model dynamics. Zero-time, expired, and
invalid-input handling remain explicit operation-specific contracts; the
formula itself does not choose their behavior.

### Preserve date boundaries

- Keep valuation `as_of`, curve `base_date`, trade date, spot/settlement date,
  fixing date, and payment date distinct.
- Rebase seasoned curves with relative date-based discount factors.
- Apply the relevant spot lag, calendars, business-day convention, and
  end-of-month rule to contractual settlement.
- Define the valuation/publication cutoff used to distinguish observed from
  projected resets. Required historical fixings come from the configured
  series; missing required observations are errors. Do not silently substitute
  a projected curve value. Same-day fixing selection follows an explicit
  operation-specific contract; this decision does not select one cutoff for every caller.
- Permit different credit-survival and discount day counts, joining them
  through date-based survival probabilities and discount factors.

Do not publish basis-point drift or other numerical error bands without a
reproducible benchmark or golden test specifying setup and provenance, under
[ADR 0014](../../project/0014-financial-reference-evidence.md) and the precise
evidence requirements in accepted
[ADR 0048](../../project/0048-reference-provenance-and-version-pinning.md).

## Rationale and alternatives

Curve construction, contractual accrual, volatility calibration, and credit
survival describe different quantities. Preserving their date-based values
avoids changing a calculation simply by borrowing a convenient year fraction
from another part of the instrument.

| Alternative | Tradeoff |
| --- | --- |
| Force one day count throughout an instrument | Reduces conversion code but can reinterpret curve, coupon, or volatility inputs. |
| Reuse a curve zero rate with a different model time | Avoids an explicit bridge but need not reproduce the curve's discount factor. |
| Silently project missing historical fixings | Produces a value while changing the agreed information set and fixing convention. |

The approach uses existing date-based APIs and conversion helpers;
it does not require a second curve framework.

## Follow-up tracking

Accepted metadata implementation, caller adoption, operation-owned fixing-
cutoff reconciliation and outstanding validation work are owned by the
[clock follow-up](../../FOLLOW_UPS.md#clock-metadata-defaults). The
[rate-option](../valuations-rates/0029-rate-option-exercise-and-volatility.md) and
[curve](../calibration-targets/0032-curve-interpolation-and-calibration.md) methodology
drafts retain their independent decision status.

## Evidence and verification required

The [shared pricing helpers](../../../../finstack-quant/valuations/core/src/instruments/common_impl/helpers.rs)
contain `zero_rate_from_df`; the
[discount curve](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/curve.rs)
provides date-based discounting. Their presence does not prove all callers
use the correct clock or bridge. The catalogue's autocallable regression is
a bounded example, not general clock-independence or cliquet coverage.

The [forward curve's `rate_between_dates`](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/curve.rs)
maps dates on the curve basis and annualizes the resulting growth on the
supplied contractual index accrual fraction. Caller selection of that
fraction is distinct from the projection clock.

Required checks should:

- compare direct curve discounting with the model-rate bridge across distinct
  curve and model day counts, including seasoned curve base dates;
- distinguish projection, accrual, and option clocks with inputs for which
  substituting one for another changes the result, including contractual
  forward annualization and volatility reference-date mapping;
- exercise spot lags, calendar adjustments, end-of-month boundaries, and
  configured historical fixings without silently projecting required past
  resets; cover missing observations and the declared same-day cutoff;
- exercise finite-input validation, zero-time and expired branches under
  their declared contracts, without assuming the shared helper covers them;
- verify survival and discount factors over the same dates when their
  underlying day counts differ;
- document the reproducible inputs and justified tolerance for any claimed
  numerical agreement or error band.

No passing source-estimator/pricer tests or model qualification are reported
by this decision. Its illustrative arithmetic is separate evidence; runtime
execution and coverage remain tracked in the linked register.
