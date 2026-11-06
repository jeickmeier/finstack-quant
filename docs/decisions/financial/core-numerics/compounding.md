# CORE-NUMERICS-01: Rate conversion and periodic z-spread application

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`core/src/math/compounding.rs`: decimal annual rates and spreads, discount factors, finite time fractions supplied by the caller, and nonzero integer periodic frequency. No schedule, day-count conversion, credit adjustment or instrument settlement is performed here.

## Choice and rationale

Continuous, periodic and simple conventions are explicit variants; continuous is the enum default. Stable `ln_1p`/`exp_m1` operations implement periodic conversion near zero. The rationale for these operations is numerical cancellation avoidance; choosing a compounding basis for an instrument remains a caller decision.

## Algorithm

1. For rate `r`, time `t`, frequency `m`, compute continuous `D=exp(-rt)`, periodic `D=exp[-mt log1p(r/m)]`, or simple `D=1/(1+rt)`.
2. Invert positive finite `D`: continuous `r=-log(D)/t`; periodic `r=m expm1[-log(D)/(mt)]`; simple `r=(1/D-1)/t`.
3. At exactly `t=0`, rate-to-DF returns 1 before validating rate; DF-to-rate validates DF then returns 0. Cross-convention conversion at zero uses instantaneous limits: periodic `c=m log1p(r/m)` and inverse `r=m expm1(c/m)`; simple and continuous share the local limit `c=r`.
4. `continuous_spread_shift(c,s)` returns `s` for continuous/simple, or `m log1p((m expm1(c/m)+s)/m)-c` for periodic. This is a zero-tenor rate conversion, not a finite-tenor simple spread conversion.
5. `z_spread_discount_factor(D,t,z,m_raw)` validates finite inputs and positive DF; for `t<=0` returns the base DF. Otherwise `m=max(m_raw,1) as u32`, guarded into `NonZeroU32`, converts the base DF to periodic yield, adds `z`, validates `1+(r+z)/m>0`, and converts back.

## Selection and controls

Public methods select the variant directly; named constants are semiannual/quarterly/monthly. `z_spread_discount_factor` always selects periodic discounting and normalizes its floating frequency argument by flooring through the integer cast after a minimum of one. There is no solver or market-data lookup.

## Conventions

`0.01` is a 1% rate or 100 bp spread. `t` must use the caller's annual day-count basis. Negative rates are permitted where the compounding denominator stays positive; the helper does not assume DF≤1.

## Limitations, failure and fallback

Periodic and simple invalid denominators return NaN in scalar methods; the z-spread wrapper returns validation errors. Scalar methods do not uniformly reject nonfinite time or arithmetic overflow. Negative finite `t` is mathematically evaluated by scalar converters, but z-spread returns the unmodified DF for nonpositive time. Nonintegral or negative finite z-spread frequencies are normalized rather than rejected.

## Alternatives and tradeoffs

All three conversions are implemented. Applying a continuously compounded additive spread directly to a periodic-quoted spread is a different economic convention; it is not an interchangeable shortcut. Rejecting nonintegral frequency would change the current public contract and needs agreement.

## Evidence

- [Owning implementation](../../../../finstack-quant/core/src/math/compounding.rs), `df_from_rate`, `rate_from_df`, `convert_rate`, `continuous_spread_shift`, `z_spread_discount_factor`.
- Inline tests `test_known_value_continuous_5pct_1y`, `test_known_value_semiannual_5pct_1y`, and `test_known_value_simple_5pct_half_year`: analytic single-cashflow values; `test_convert_rate_t_zero_returns_limit` and roundtrip cases: boundary and inverse consistency evidence.
- [Hull reference](../../../REFERENCES.md#hull-options-futures) covers discounting conventions; it does not authorize one basis for every product.

Overflow and nonfinite scalar behavior remains a caller-validation responsibility, not a demonstrated global guarantee.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
