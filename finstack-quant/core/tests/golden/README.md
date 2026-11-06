# finstack-quant-core golden fixtures

Reference values from external sources, pinned as JSON with their provenance, so
that a change in `finstack-quant-core` which moves a market-standard number fails
loudly.

The envelope format, loader, tolerance semantics, and assertion helpers belong to
[`finstack-quant-test-utils`](../../../test-utils/README.md); read that README
first if you are writing a new suite. This file documents what lives here.

## Layout

```
golden/
├── README.md
├── mod.rs                          included by tests/golden_tests.rs
├── daycount_quantlib_tests.rs
├── variance_tests.rs
└── data/
    ├── daycount_quantlib.json      23 cases
    └── realized_variance.json       4 cases
```

`tests/golden_tests.rs` is the cargo target; it does nothing but
`#[path = "golden/mod.rs"] mod golden;`.

## Suites

### `daycount_quantlib.json` — 23 cases

Day-count year fractions and 30/360 day counts against QuantLib 1.32, driven by
`daycount_quantlib_tests.rs`. Cases by `convention` tag:

| Tag | `DayCount` | Cases |
|-----|------------|-------|
| `act_act_isma` | `ActActIsma` | 7 |
| `30_360` | `Thirty360` | 6 |
| `act_act` | `ActAct` | 3 |
| `act_365l` | `Act365L` | 3 |
| `30e_360` | `ThirtyE360` | 1 |
| `act_360` | `Act360` | 1 |
| `act_365f` | `Act365F` | 1 |
| `comparison` | US vs Euro 30/360 side by side | 1 |

Convention notes that matter when reading a failure:

- 30/360 US (Bond Basis / SIA-NASD PSA) — QuantLib `Thirty360::USA` sets Feb EOM
  `D1 = 30`. This is **not** ISDA 2006 §4.16(f), which leaves February `D1`
  unadjusted.
- 30E/360 (Eurobond) — ISDA 2006 §4.16(g).
- Act/Act ISDA — year-boundary splitting, ISDA 2006 §4.16(b).
- Act/Act ICMA — coupon-period basis, ICMA Rule 251. Every case carries a
  `frequency` field (`1M`/`3M`/`6M`/`1Y`); a case whose `[start, end)` is not an
  integer number of those coupons also carries an explicit `coupon_period`
  `[start, end]` pair. Both feed `DayCountContext`, and `DayCount::ActActIsma`
  returns `MissingCouponPeriodForActActIsma` for an irregular span given
  frequency alone rather than re-anchoring it — so the pair is not optional
  there.
- Act/365L — ICMA Rule 251 leap-year denominator, **not** ACT/ACT AFB.

This suite predates the shared `Expectation` shape: each case carries its own
`expected` object (`year_fraction`, `days_30_360`, a plain `tolerance` float, and
the US/Euro comparison fields) and the test asserts against it directly.

References: QuantLib 1.32; ISDA (2006) 2006 ISDA Definitions §4.16(b), (d)-(g);
ICMA Rule Book Rule 251.

### `realized_variance.json` — 4 cases

OHLC realized-variance estimators, checked against
`finstack_quant_core::math::stats::realized_variance_ohlc`:

| Case | Estimator |
|------|-----------|
| `parkinson_two_day` | Parkinson (1980), high-low range |
| `garman_klass_two_day` | Garman-Klass (1980), OHLC |
| `rogers_satchell_two_day` | Rogers-Satchell (1991) |
| `yang_zhang_four_day` | Yang-Zhang (2000), drift-independent |

This is the one suite here that uses the shared `Expectation` type, so
`GoldenAssert::expected` reads the tolerance out of the fixture.
`variance_tests.rs` also asserts on `suite.meta` (suite id, non-empty reference
source, non-empty `generated.at`/`by`, `status == "certified"`). That checks that
the provenance block is present and labelled; it does not show that the expected
values are independent of the implementation.

Evidence status of the expected values: `meta.generated.by` names the
`finstack_quant_core::math::stats` implementation, so the pinned values are
implementation output at 1e-10, and the per-case "Hand-calculated" notes carry
only three to four significant digits. A separate recomputation from the cited
formulas (plain Python `math`, the Parkinson, Garman-Klass and Rogers-Satchell
definitions above; Yang-Zhang with period count `m = n - 1` for the overnight and
open-close sample variances and the Rogers-Satchell average, and
`k = 0.34 / (1.34 + (m + 1) / (m - 1))`) reproduced all four expected values to
within 1e-12. That recomputation is the independent derivation for these cases;
it is not stored as a generator, and the fixture's `generated` block has not
been changed to reflect it.

References: Parkinson (1980) *J. Business* 53(1) 61-65; Garman & Klass (1980)
*J. Business* 53(1) 67-78; Rogers & Satchell (1991) *Ann. Appl. Probab.* 1(4)
504-512; Yang & Zhang (2000) *J. Business* 73(3) 477-491.

## Running

```bash
cargo nextest run -p finstack-quant-core --test core golden_tests

# one suite
cargo nextest run -p finstack-quant-core --test core variance
```

These are part of `mise run rust-test`. The workspace `mise run goldens-*` tasks
drive the `finstack-quant-valuations` pricing and attribution goldens and do not
touch this directory.

## Adding or regenerating a fixture

1. Add JSON under `data/` with a complete `meta` block: `suite_id`,
   `description`, `reference_source` (name, version, vendor, url), `generated`
   (`at`, `by`, and `command` when a generator exists), and `schema_version: 1`.
2. Set `"status": "provisional"` until the expected values have independent
   evidence: a hand derivation, a separate implementation, or a cited external
   reference, recorded per case with the method, inputs, units and tolerance
   rationale (see [ADR 0014](../../../../docs/decisions/project/0014-financial-reference-evidence.md)
   and [ADR 0048](../../../../docs/decisions/project/0048-reference-provenance-and-version-pinning.md)).
   Promote to `"certified"` only with that evidence and the required
   authorization; a generator that calls the implementation under test does not
   qualify.
3. Add a section to this README covering the cases, conventions, and tolerances.
4. When a vendor version changes: regenerate, bump `reference_source.version` and
   `generated.at`, and record the rationale in the test module docs. Do **not**
   widen a tolerance, skip a case, or change an expected value to make a
   regenerated fixture pass: that changes a protected reference and needs
   evidence and explicit authorization under ADR 0014. A `skip` with an
   explanation does not authorize disabling a protected test; fix the root
   cause.

The day-count and variance suites record their origin in `meta.generated.by`:
`"Market Standards Audit - finstack"` for the day-count suite and
`"finstack_quant_core::math::stats implementation"` for the variance suite.
