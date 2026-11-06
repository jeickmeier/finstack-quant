# ADR 0045: Financial clocks and date boundaries

**Status:** Accepted

**Decision date:** 2026-11-05

**Category:** Financial conventions

**Implementation status:** Shared date and discount-factor primitives located;
pricer adoption, volatility metadata, and operation-specific defaults remain
partially traced.

**Validation status:** No numerical or host-parity checks executed for this
record.

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
This decision does not choose the metadata representation or omitted-metadata
behavior; missing metadata does not establish an Act/365F or other universal fallback.

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

Metadata choices, caller adoption, fixing-cutoff reconciliation, and outstanding
validation work are owned by the
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

No passing numerical results or model qualification are reported by this
decision; execution and coverage are tracked in the linked register.
