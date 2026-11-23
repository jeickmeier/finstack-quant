use super::engine::*;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use time::macros::date;

/// Builds a [`DiscountCurve`] from exact `(t, df)` knots for curve-snapshot
/// tests. The brief's helper omitted `.base_date(..)`; `DiscountCurveBuilder::build`
/// requires a base date (`base: Option<Date>` starts `None` and `build()` errors
/// on `None`), so it is added here. `df(t)` reads knot times as year fractions
/// directly and never consults the base date, so the choice of date is
/// otherwise inert for these tests.
fn curve(id: &'static str, knots: &[(f64, f64)]) -> DiscountCurve {
    DiscountCurve::builder(id)
        .base_date(date!(2024 - 01 - 01))
        .knots(knots.iter().copied())
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("curve")
}

#[test]
fn flat_curve_cell_returns_equal_pure_carry() {
    // 4% flat, Δt = 0.25, width 1.0, max 2.0 => mids 0.5 and 1.5.
    // R = e^{0.04·0.25} − 1 = 0.01005017 for every cell (carry only, no roll differential).
    let knots = [
        (0.0, 1.0),
        (0.25, 0.99004983),
        (0.5, 0.98019867),
        (1.25, 0.95122942),
        (1.5, 0.94176453),
    ];
    let start = curve("UST", &knots);
    let end = curve("UST", &knots);
    let table =
        cell_returns_from_curves(&start, &end, 0.25, 2.0, "UST", &CellConfig { width: 1.0 })
            .unwrap();
    assert_eq!(table.cells.len(), 2);
    for cell in &table.cells {
        assert!(
            (cell.base_return - 0.01005017).abs() < 1e-6,
            "{}",
            cell.base_return
        );
        assert!(cell.observed);
    }
}

#[test]
fn rising_rates_hurt_the_longer_cell_more() {
    // Start 4% flat, end 5% flat: R(0.5) = e^{−0.0125+0.02} − 1, R(1.5) = e^{−0.0625+0.06} − 1.
    //
    // Two corrections to the brief here, both verified with Python
    // `decimal.Decimal.exp` at 50-digit precision:
    //
    // 1. The brief's second end-curve knot literal `(0.25, 0.98757805)` is
    //    wrong: e^{-0.05*0.25} = e^{-0.0125} = 0.9875778004938814..., which
    //    rounds to `0.98757780`, not `0.98757805` — a ~2.5e-7 error, not
    //    just an 8th-decimal rounding artifact. Using the wrong knot as
    //    written makes `df_end(0.25)` (and hence `R(0.5)`) numerically
    //    wrong regardless of interpolation method, since the curve passes
    //    through its own knots exactly. Fixed to `0.98757780` below (the
    //    other three knots in this test were independently verified
    //    correct to 8 decimals).
    // 2. With the corrected knot, R(0.5) = 0.0075281983396..., not the
    //    brief's `0.00752821` (off by ~1.46e-8, outside the brief's own
    //    `1e-8` tolerance). R(1.5) = -0.0024968776025398..., which does
    //    match the brief's `-0.00249688` within `1e-8`.
    let start = curve("UST", &[(0.0, 1.0), (0.5, 0.98019867), (1.5, 0.94176453)]);
    let end = curve("UST", &[(0.0, 1.0), (0.25, 0.98757780), (1.25, 0.93941306)]);
    let table =
        cell_returns_from_curves(&start, &end, 0.25, 2.0, "UST", &CellConfig { width: 1.0 })
            .unwrap();
    assert!((table.cells[0].base_return - 0.0075281983).abs() < 1e-8);
    assert!((table.cells[1].base_return - (-0.00249688)).abs() < 1e-8);
}

#[test]
fn swap_curve_base_is_supported_via_label() {
    let knots = [
        (0.0, 1.0),
        (0.25, 0.99004983),
        (0.5, 0.98019867),
        (1.25, 0.95122942),
        (1.5, 0.94176453),
    ];
    let table = cell_returns_from_curves(
        &curve("USD-SOFR", &knots),
        &curve("USD-SOFR", &knots),
        0.25,
        2.0,
        "USD-SOFR",
        &CellConfig { width: 1.0 },
    )
    .unwrap();
    assert_eq!(table.base_label, "USD-SOFR");
}

#[test]
fn cell_maturing_inside_period_is_rejected() {
    let knots = [(0.0, 1.0), (0.25, 0.99004983), (0.5, 0.98019867)];
    // width 0.25 => first mid 0.125 < Δt 0.25 => error naming the cell
    let err = cell_returns_from_curves(
        &curve("UST", &knots),
        &curve("UST", &knots),
        0.25,
        0.5,
        "UST",
        &CellConfig { width: 0.25 },
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("0.0-0.2") || err.to_string().contains("matures"),
        "{err}"
    );
}

/// Exact-boundary pin for the `mid <= horizon_years` guard: a cell
/// midpoint numerically *equal* to `horizon_years` (not just less than
/// it) must still be rejected — the hypothetical zero would mature
/// exactly at the end of the holding period, leaving `end_t = mid -
/// horizon_years = 0.0`, not a forward maturity. Width 1.0, horizon
/// 0.5 => first cell "0.0-1.0" has midpoint exactly 0.5. Weakening the
/// guard's `<=` to `<` would silently accept this cell.
#[test]
fn cell_maturing_exactly_at_horizon_boundary_is_rejected() {
    let knots = [(0.0, 1.0), (0.5, 0.98019867), (1.5, 0.94176453)];
    let err = cell_returns_from_curves(
        &curve("UST", &knots),
        &curve("UST", &knots),
        0.5,
        2.0,
        "UST",
        &CellConfig { width: 1.0 },
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("0.0-1.0") || err.to_string().contains("matures"),
        "{err}"
    );
}

/// Executed regression for the reviewer's Minor-2 finding, second entry
/// point: `max_duration` is a direct caller parameter on
/// `cell_returns_from_curves` (unlike `cell_returns_from_reference`,
/// where it is derived from the reference universe), so a units mistake
/// reaches it even more directly. An astronomically large `max_duration`
/// must fail closed before the `Vec::with_capacity(num_cells)`
/// allocation, not panic.
#[test]
fn astronomically_large_max_duration_fails_closed_not_panics() {
    let knots = [(0.0, 1.0), (0.5, 0.98019867), (1.5, 0.94176453)];
    let err = cell_returns_from_curves(
        &curve("UST", &knots),
        &curve("UST", &knots),
        0.5,
        1e30,
        "UST",
        &CellConfig { width: 1.0 },
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("sanity bound"),
        "error must name the cell-count sanity bound: {err}"
    );
}

#[test]
fn cell_table_interpolates_interior_and_extrapolates_ends() {
    // Observed: cell 0.0-0.5 => 0.01, cell 2.0-2.5 => 0.05. Gaps 0.5-2.0 interpolate
    // linearly by cell index: 0.02, 0.03, 0.04. A trailing empty cell would be flat.
    let reference = vec![
        ReferenceReturn {
            duration: 0.25,
            total_return: 0.01,
        },
        ReferenceReturn {
            duration: 2.25,
            total_return: 0.05,
        },
    ];
    let table = cell_returns_from_reference(&reference, "UST", &CellConfig { width: 0.5 })
        .expect("valid reference");
    let returns: Vec<f64> = table.cells.iter().map(|c| c.base_return).collect();
    let observed: Vec<bool> = table.cells.iter().map(|c| c.observed).collect();
    assert_eq!(table.cells.len(), 5);
    assert_eq!(table.cells[1].label, "0.5-1.0");
    for (got, want) in returns.iter().zip([0.01, 0.02, 0.03, 0.04, 0.05]) {
        assert!((got - want).abs() < 1e-12, "{got} vs {want}");
    }
    assert_eq!(observed, [true, false, false, false, true]);
    assert_eq!(table.base_label, "UST");
}

/// Leading flat extrapolation through the public API: the smallest
/// reference duration lands strictly above cell 0, so cells before it
/// have no members and must flat-equal the first observed cell.
///
#[test]
fn cell_table_extrapolates_leading_cells_flat() {
    let reference = vec![
        ReferenceReturn {
            duration: 1.25, // cell 2 of a 0.5-wide grid: [1.0, 1.5)
            total_return: 0.02,
        },
        ReferenceReturn {
            duration: 2.25, // top cell: [2.0, 2.5)
            total_return: 0.05,
        },
    ];
    let table = cell_returns_from_reference(&reference, "UST", &CellConfig { width: 0.5 })
        .expect("valid reference");
    assert_eq!(table.cells.len(), 5);
    // Cells 0 and 1 are leading gaps below the first observed cell (2).
    assert_eq!(
        table.cells[0].base_return, 0.02,
        "cell 0 must flat-extrapolate from the first observed cell"
    );
    assert!(!table.cells[0].observed);
    assert_eq!(
        table.cells[1].base_return, 0.02,
        "cell 1 must flat-extrapolate from the first observed cell"
    );
    assert!(!table.cells[1].observed);
    assert!(table.cells[2].observed);
}
#[test]
fn cell_table_averages_multiple_instruments_per_cell() {
    let reference = vec![
        ReferenceReturn {
            duration: 0.10,
            total_return: 0.010,
        },
        ReferenceReturn {
            duration: 0.40,
            total_return: 0.030,
        }, // same cell 0.0-0.5
    ];
    let table = cell_returns_from_reference(&reference, "UST", &CellConfig { width: 0.5 }).unwrap();
    assert!((table.cells[0].base_return - 0.020).abs() < 1e-15); // simple average
}

/// Four distinct guards, each asserted on message content rather than
/// bare `.is_err()`: the reviewer traced all four and each *can* fail
/// (none is a can't-fail test), but a bare `.is_err()` would still pass
/// if two of these guards were swapped or merged, since it cannot tell
/// *which* validation fired. This departs from the rest of this
/// module's (and this branch's) message-content convention.
#[test]
fn cell_table_rejects_bad_inputs() {
    let ok = vec![ReferenceReturn {
        duration: 1.0,
        total_return: 0.01,
    }];

    let err = cell_returns_from_reference(&[], "UST", &CellConfig { width: 0.5 })
        .expect_err("empty reference universe must be rejected");
    assert!(
        err.to_string().contains("non-empty"),
        "error must name the empty-reference guard: {err}"
    );

    let err = cell_returns_from_reference(&ok, "UST", &CellConfig { width: 0.0 })
        .expect_err("zero width must be rejected");
    assert!(
        err.to_string().contains("width"),
        "error must name the width guard: {err}"
    );

    let neg = vec![ReferenceReturn {
        duration: -0.5,
        total_return: 0.01,
    }];
    let err = cell_returns_from_reference(&neg, "UST", &CellConfig { width: 0.5 })
        .expect_err("negative duration must be rejected");
    assert!(
        err.to_string().contains("duration"),
        "error must name the duration guard: {err}"
    );

    let nan = vec![ReferenceReturn {
        duration: 1.0,
        total_return: f64::NAN,
    }];
    let err = cell_returns_from_reference(&nan, "UST", &CellConfig { width: 0.5 })
        .expect_err("NaN total_return must be rejected");
    assert!(
        err.to_string().contains("total_return"),
        "error must name the total_return guard: {err}"
    );
}

/// Executed regression for the reviewer's Minor-2 finding: an
/// astronomically large reference `duration` (a plausible units mistake,
/// e.g. days instead of years) must fail closed with a clear message
/// *before* the `vec![0.0; num_cells]` allocations, not panic with
/// "capacity overflow" — a Rust panic that would cross the WASM boundary
/// as an unrecoverable module trap, not a catchable JS exception.
#[test]
fn astronomically_large_duration_fails_closed_not_panics() {
    let reference = vec![ReferenceReturn {
        duration: 1e30,
        total_return: 0.01,
    }];
    let err = cell_returns_from_reference(&reference, "UST", &CellConfig { width: 1.0 })
        .expect_err("a 1e30 duration must fail closed, not panic on allocation");
    assert!(
        err.to_string().contains("sanity bound"),
        "error must name the cell-count sanity bound: {err}"
    );
}

/// `width = 0.03` makes distinct cells (e.g. `[0.06, 0.09)` and
/// `[0.09, 0.12)`) both format to the label `"0.1-0.1"` under
/// `"{lower:.1}-{upper:.1}"`. Since downstream stages look positions up
/// by label, this must fail closed rather than silently colliding.
#[test]
fn cell_table_rejects_colliding_width_labels() {
    let reference = vec![ReferenceReturn {
        duration: 0.1,
        total_return: 0.01,
    }];
    let err = cell_returns_from_reference(&reference, "UST", &CellConfig { width: 0.03 })
        .expect_err("width=0.03 must be rejected: labels collide at one-decimal precision");
    let message = err.to_string();
    assert!(message.contains("0.03"), "must name the width: {message}");
    assert!(
        message.contains("0.1-0.1"),
        "must name the colliding label: {message}"
    );
}

/// Widths that are multiples of `0.1` (the plan-mandated widths used by
/// this task and Task 2) must keep passing the collision check.
#[test]
fn cell_table_accepts_legitimate_widths() {
    let reference = vec![
        ReferenceReturn {
            duration: 0.25,
            total_return: 0.01,
        },
        ReferenceReturn {
            duration: 2.25,
            total_return: 0.05,
        },
    ];
    assert!(cell_returns_from_reference(&reference, "UST", &CellConfig { width: 0.5 }).is_ok());
    assert!(cell_returns_from_reference(&reference, "UST", &CellConfig { width: 0.25 }).is_ok());
}

/// Populated cells of Lehman Figure B-1 (May 1997 Treasury returns by
/// duration), percent/100. Empty cells 7.5-9.0 and 12.5+ are deliberately
/// omitted — interpolation/extrapolation is covered by Task 1's own tests.
fn lehman_b1_table() -> DurationCellTable {
    let cells: &[(f64, f64)] = &[
        (0.25, 0.0054),
        (0.75, 0.0059),
        (1.25, 0.0066),
        (1.75, 0.0071),
        (2.25, 0.0073),
        (2.75, 0.0075),
        (3.25, 0.0078),
        (3.75, 0.0078),
        (4.25, 0.0080),
        (4.75, 0.0092),
        (5.25, 0.0097),
        (5.75, 0.0103),
        (6.25, 0.0111),
        (6.75, 0.0105),
        (7.25, 0.0105),
        (9.25, 0.0110),
        (9.75, 0.0111),
        (10.25, 0.0111),
        (10.75, 0.0115),
        (11.25, 0.0118),
        (11.75, 0.0121),
        (12.25, 0.0116),
    ];
    let reference: Vec<ReferenceReturn> = cells
        .iter()
        .map(|&(duration, total_return)| ReferenceReturn {
            duration,
            total_return,
        })
        .collect();
    cell_returns_from_reference(&reference, "UST", &CellConfig { width: 0.5 }).unwrap()
}

/// Reproduces Dynkin, Hyman & Vankudre (1998), Figure B-2 exactly: five
/// real bonds, five published excess returns. Independently re-derived
/// (see task report) with Python at full precision; the brief's values
/// match the arithmetic exactly, so this asserts at `1e-12`.
#[test]
fn lehman_figure_b2_excess_returns_reproduce_exactly() {
    let table = lehman_b1_table();
    let positions = vec![
        ExcessReturnPosition {
            id: "Colombia".into(),
            weight: 0.2,
            duration: 5.16,
            total_return: 0.0225,
        },
        ExcessReturnPosition {
            id: "RiteAid".into(),
            weight: 0.2,
            duration: 6.71,
            total_return: 0.0172,
        },
        ExcessReturnPosition {
            id: "NewsAM".into(),
            weight: 0.2,
            duration: 9.58,
            total_return: 0.0182,
        },
        ExcessReturnPosition {
            id: "Delta".into(),
            weight: 0.2,
            duration: 9.81,
            total_return: 0.0102,
        },
        ExcessReturnPosition {
            id: "Quebec".into(),
            weight: 0.2,
            duration: 11.08,
            total_return: 0.0185,
        },
    ];
    let result = excess_returns(&positions, &table).unwrap();
    let expected = [0.0128, 0.0067, 0.0071, -0.0009, 0.0067]; // Figure B-2, exact
    for (got, want) in result.positions.iter().zip(expected) {
        assert!(
            (got.excess_return - want).abs() < 1e-12,
            "{}: {} vs {want}",
            got.id,
            got.excess_return
        );
    }
    assert_eq!(result.positions[0].cell, "5.0-5.5");
    assert!((result.portfolio_excess_return - 0.00648).abs() < 1e-12); // 0.2 * Sum(excess)
    assert!((result.portfolio_total_return - 0.01732).abs() < 1e-12);
    assert!((result.portfolio_base_return - 0.01084).abs() < 1e-12);
    // Identity: excess === total - base, computed independently.
    assert!(
        (result.portfolio_excess_return
            - (result.portfolio_total_return - result.portfolio_base_return))
            .abs()
            < 1e-15
    );
}

/// Proves `portfolio_excess_return`'s independent accumulation is
/// load-bearing, not just algebraically equivalent to `total - base`.
///
/// At the small return magnitudes used elsewhere in this test module
/// (~1e-2), `total - base` and the independent weighted sum agree to
/// 1e-15 (see the identity assertion in the Figure B-2 golden above), so
/// no black-box test at that scale can tell the two computations apart.
/// This test instead uses total/base returns of order 1e8 that differ by
/// only ~1e-6 per position — e.g. a large common carry/roll component
/// swamping a tiny credit-specific excess — which is exactly the shape
/// that makes `total_sum - base_sum` lose precision: both sums round to
/// the nearest representable `f64` near 1e8 (ULP ~1.49e-8) independently
/// of each other, so their difference inherits error at that scale even
/// though the true excess is ~1e-6. Summing the per-position excesses
/// directly never forms those large intermediate sums, so it does not
/// inherit that error.
///
/// Every literal below was independently verified in Python using the
/// same IEEE 754 double arithmetic and the crate's exact Neumaier
/// compensated-summation algorithm (transcribed identically), plus
/// `fractions.Fraction` for an exact rational cross-check of the "true"
/// answer implied by the bit-exact `f64` inputs:
/// - Independent accumulation: `2.9981136322021487e-6`, matching the
///   exact rational answer to `0.0` absolute error.
/// - `total_sum - base_sum` shortcut: `2.995133399963379e-6`, off by
///   `~2.98e-9` absolute (~0.1% relative) from the exact answer.
///
/// See the task report for the full derivation. This test was run
/// against a deliberately mutated implementation
/// (`portfolio_excess_return: total_return_sum.total() -
/// base_return_sum.total()`) and confirmed to fail before being
/// finalized against the real implementation.
#[test]
fn portfolio_excess_return_avoids_catastrophic_cancellation() {
    // Single-member cells so each cell's base_return is exactly its one
    // reference literal (no averaging to reason about).
    let base_returns = [
        99_999_999.999_999,
        100_000_136.999_998,
        100_000_273.999_997,
        100_000_410.999_996,
        100_000_547.999_995,
    ];
    let reference: Vec<ReferenceReturn> = base_returns
        .iter()
        .enumerate()
        .map(|(i, &total_return)| ReferenceReturn {
            #[allow(clippy::cast_precision_loss)]
            duration: 0.25 + i as f64 * 0.5,
            total_return,
        })
        .collect();
    let table = cell_returns_from_reference(&reference, "UST", &CellConfig { width: 0.5 }).unwrap();

    let total_returns = [
        100_000_000.0,
        100_000_137.0,
        100_000_274.0,
        100_000_411.0,
        100_000_548.0,
    ];
    let positions: Vec<ExcessReturnPosition> = total_returns
        .iter()
        .enumerate()
        .map(|(i, &total_return)| ExcessReturnPosition {
            id: format!("P{i}"),
            weight: 0.2,
            #[allow(clippy::cast_precision_loss)]
            duration: 0.25 + i as f64 * 0.5,
            total_return,
        })
        .collect();

    let result = excess_returns(&positions, &table).unwrap();

    // Exact rational answer implied by the bit-exact f64 inputs above
    // (Python `fractions.Fraction`), rounded to the nearest f64.
    let exact = 2.998_113_632_202_148_7e-6;

    // The independent accumulation must match the exact answer far
    // tighter than the shortcut's ~2.98e-9 error: this is the assertion
    // the reviewer's mutation fails.
    assert!(
        (result.portfolio_excess_return - exact).abs() < 1e-9,
        "portfolio_excess_return should avoid catastrophic cancellation: \
         got {}, exact {exact}",
        result.portfolio_excess_return
    );

    // Setup sanity check (not the mutation-detecting assertion above):
    // confirm this scenario genuinely produces a large-sum shortcut that
    // disagrees with the exact answer beyond that same tolerance, so a
    // future edit to the literals can't silently make this test vacuous.
    let shortcut = result.portfolio_total_return - result.portfolio_base_return;
    assert!(
        (shortcut - exact).abs() > 1e-9,
        "test setup should produce a measurable cancellation gap: \
         shortcut {shortcut}, exact {exact}"
    );
}

#[test]
fn excess_returns_validation() {
    let table = lehman_b1_table();
    // duration beyond table
    let far = vec![ExcessReturnPosition {
        id: "X".into(),
        weight: 1.0,
        duration: 99.0,
        total_return: 0.01,
    }];
    let err = excess_returns(&far, &table).unwrap_err();
    assert!(err.to_string().contains('X'), "{err}");
    // weight tolerance pinned both directions
    let mk = |w: f64| {
        vec![ExcessReturnPosition {
            id: "A".into(),
            weight: w,
            duration: 1.0,
            total_return: 0.01,
        }]
    };
    assert!(excess_returns(&mk(1.0 + 5e-7), &table).is_ok());
    assert!(excess_returns(&mk(1.0 + 2e-6), &table).is_err());
}

fn cell(label: &str, lower: f64, upper: f64, base_return: f64) -> CellReturn {
    CellReturn {
        label: label.into(),
        lower,
        upper,
        base_return,
        observed: true,
    }
}

fn single_position(duration: f64) -> Vec<ExcessReturnPosition> {
    vec![ExcessReturnPosition {
        id: "P".into(),
        weight: 1.0,
        duration,
        total_return: 0.03,
    }]
}

#[test]
fn duration_cell_table_rejects_invalid_cell_bounds_before_lookup() {
    let cases = [
        (vec![cell("negative", -0.5, 0.5, 0.01)], "non-negative"),
        (
            vec![cell("zero-width", 0.5, 0.5, 0.01)],
            "greater than lower",
        ),
        (vec![cell("nan-lower", f64::NAN, 0.5, 0.01)], "finite"),
        (
            vec![cell("infinite-upper", 0.0, f64::INFINITY, 0.01)],
            "finite",
        ),
    ];

    for (cells, expected) in cases {
        let table = DurationCellTable {
            base_label: "CUSTOM".into(),
            cells,
        };
        let err = excess_returns(&single_position(0.25), &table)
            .expect_err("invalid cell bounds must be rejected before lookup");
        let message = err.to_string();
        assert!(message.contains("cell[0]"), "{message}");
        assert!(message.contains(expected), "{message}");
    }
}

#[test]
fn duration_cell_table_rejects_empty_label_before_lookup() {
    let table = DurationCellTable {
        base_label: "CUSTOM".into(),
        cells: vec![cell("", 0.0, 1.0, 0.01)],
    };

    let err = excess_returns(&single_position(0.25), &table)
        .expect_err("empty duration-cell labels must be rejected before lookup");
    assert!(err.to_string().contains("cell[0]"), "{err}");
    assert!(err.to_string().contains("non-empty"), "{err}");
}

#[test]
fn duration_cell_table_rejects_duplicate_labels_before_lookup() {
    let table = DurationCellTable {
        base_label: "CUSTOM".into(),
        cells: vec![
            cell("duplicate", 0.0, 1.0, 0.01),
            cell("duplicate", 1.0, 2.0, 0.02),
        ],
    };

    let err = excess_returns(&single_position(0.25), &table)
        .expect_err("duplicate duration-cell labels must be rejected before lookup");
    assert!(err.to_string().contains("cell[1]"), "{err}");
    assert!(err.to_string().contains("unique"), "{err}");
}

#[test]
fn duration_cell_table_rejects_ordering_overlap_and_non_finite_base_return() {
    let cases = [
        (
            vec![
                cell("later", 1.0, 2.0, 0.02),
                cell("earlier", 0.0, 0.5, 0.01),
            ],
            "ascending",
        ),
        (
            vec![
                cell("first", 0.0, 1.0, 0.01),
                cell("overlap", 0.5, 1.5, 0.02),
            ],
            "overlaps",
        ),
        (vec![cell("nan-base", 0.0, 1.0, f64::NAN)], "base_return"),
    ];

    for (cells, expected) in cases {
        let table = DurationCellTable {
            base_label: "CUSTOM".into(),
            cells,
        };
        let err = excess_returns(&single_position(0.25), &table)
            .expect_err("invalid table structure must be rejected before lookup");
        let message = err.to_string();
        assert!(message.contains(expected), "{message}");
    }
}

#[test]
fn duration_cell_table_gaps_are_valid_but_uncovered_durations_error() {
    let table = DurationCellTable {
        base_label: "CUSTOM".into(),
        cells: vec![cell("short", 0.0, 1.0, 0.01), cell("long", 2.0, 3.0, 0.02)],
    };

    let err = excess_returns(&single_position(1.5), &table)
        .expect_err("a duration in a valid table gap has no matching cell");
    assert!(err.to_string().contains("position 'P'"), "{err}");
}

#[test]
fn duration_cell_table_exact_top_boundary_remains_inclusive() {
    let table = DurationCellTable {
        base_label: "CUSTOM".into(),
        cells: vec![cell("short", 0.0, 1.0, 0.01), cell("long", 1.0, 2.0, 0.02)],
    };

    let result = excess_returns(&single_position(2.0), &table)
        .expect("the final cell's upper boundary is inclusive");
    assert_eq!(result.positions[0].cell, "long");
    assert!((result.positions[0].excess_return - 0.01).abs() < 1e-15);
}

/// Serialize -> deserialize round-trip for [`ExcessReturnResult`].
///
/// Float fields are compared with a tight tolerance rather than
/// `assert_eq!`: this workspace's `serde_json` dependency does not enable
/// the `float_roundtrip` cargo feature, so JSON float parsing is
/// documented as "best-effort precision," not bit-exact — independently
/// confirmed here by round-tripping `0.0225 - 0.0097` (`0.01279999999999999888`)
/// through `serde_json`, which comes back as `0.01280000000000000061`, one
/// ULP off. That is a `serde_json` parser characteristic, not a defect in
/// this module, so the test tolerates it instead of masking it with
/// `float_roundtrip`.
#[test]
fn excess_return_result_serde_round_trip() {
    let table = lehman_b1_table();
    let positions = vec![ExcessReturnPosition {
        id: "Colombia".into(),
        weight: 1.0,
        duration: 5.16,
        total_return: 0.0225,
    }];
    let result = excess_returns(&positions, &table).unwrap();
    let json = serde_json::to_string(&result).unwrap();
    let round_tripped: ExcessReturnResult = serde_json::from_str(&json).unwrap();
    assert_eq!(round_tripped.base_label, result.base_label);
    assert_eq!(round_tripped.positions.len(), result.positions.len());
    assert_eq!(round_tripped.positions[0].id, result.positions[0].id);
    assert_eq!(round_tripped.positions[0].cell, result.positions[0].cell);
    assert!(
        (round_tripped.positions[0].base_return - result.positions[0].base_return).abs() < 1e-12
    );
    assert!(
        (round_tripped.positions[0].excess_return - result.positions[0].excess_return).abs()
            < 1e-12
    );
    assert!((round_tripped.portfolio_total_return - result.portfolio_total_return).abs() < 1e-12);
    assert!((round_tripped.portfolio_base_return - result.portfolio_base_return).abs() < 1e-12);
    assert!((round_tripped.portfolio_excess_return - result.portfolio_excess_return).abs() < 1e-12);
}

/// Regression for the non-dyadic-width edge desynchronisation (report
/// B-1): computing `upper = lower + width` accumulates a different
/// rounding error than cell `i+1`'s `lower = (i+1) * width`, so for
/// widths like 0.1/0.2/0.3/0.4 (not exactly representable in binary)
/// adjacent cells either overlap by one ULP — which
/// [`validate_cell_table`] rejects with an "overlaps" error even though
/// the width guidance in [`check_label_collisions`]'s own error message
/// recommends multiples of 0.1 — or open a genuine one-ULP gap. Both
/// edges must be computed from the grid origin so
/// `cells[i].upper == cells[i+1].lower` bit-for-bit.
#[test]
fn non_dyadic_width_tables_pass_downstream_validation() {
    for width in [0.1, 0.2, 0.3, 0.4] {
        let reference = vec![
            ReferenceReturn {
                duration: 0.5 * width,
                total_return: 0.01,
            },
            ReferenceReturn {
                duration: 15.5 * width, // 16 cells: spans the first ULP mismatch
                total_return: 0.05,
            },
        ];
        let table = cell_returns_from_reference(&reference, "UST", &CellConfig { width })
            .expect("table construction");
        for pair in table.cells.windows(2) {
            assert_eq!(
                pair[0].upper.to_bits(),
                pair[1].lower.to_bits(),
                "width {width}: cell edge {} != next lower {}",
                pair[0].upper,
                pair[1].lower
            );
        }
        let positions = vec![ExcessReturnPosition {
            id: "P".into(),
            weight: 1.0,
            duration: 0.5 * width,
            total_return: 0.02,
        }];
        excess_returns(&positions, &table).unwrap_or_else(|err| {
            panic!("width {width}: generated table must pass its own validator: {err}")
        });
    }
}

/// Regression for M-1: the reference-bucketing index was derived by
/// truncating `duration / width`, while position lookup (`find_cell`)
/// partitions on the materialized lower bounds `i * width`. The two maps
/// disagree at grid edges — e.g. `4.3 / 0.1` truncates to 42 while
/// `43 * 0.1 <= 4.3` in f64, so `find_cell` returns cell 43. A bond
/// sitting at exactly the reference Treasury's duration then matched an
/// *interpolated* cell instead of the observed one. Both maps must use
/// the same predicate.
#[test]
fn reference_bucketing_agrees_with_position_lookup_at_grid_edges() {
    let reference = vec![
        ReferenceReturn {
            duration: 4.3,
            total_return: 0.05,
        },
        ReferenceReturn {
            duration: 8.0,
            total_return: 0.02,
        },
    ];
    let table = cell_returns_from_reference(&reference, "UST", &CellConfig { width: 0.1 })
        .expect("table construction");
    let positions = vec![ExcessReturnPosition {
        id: "AT_REF".into(),
        weight: 1.0,
        duration: 4.3, // exactly the reference Treasury's duration
        total_return: 0.055,
    }];
    let result = excess_returns(&positions, &table).expect("lookup");
    assert_eq!(
        result.positions[0].base_return, 0.05,
        "a position at the reference instrument's exact duration must match \
         the observed cell holding that instrument, not an interpolated one \
         (got cell '{}')",
        result.positions[0].cell
    );
    let cell = table
        .cells
        .iter()
        .find(|c| c.label == result.positions[0].cell)
        .expect("matched cell exists");
    assert!(
        cell.observed,
        "matched cell '{}' must be the observed one",
        cell.label
    );
}

#[test]
fn excess_return_position_denies_unknown_fields() {
    let json = r#"{"id": "A", "weight": 1.0, "duration": 1.0, "total_return": 0.01, "extra": 1}"#;
    let result: std::result::Result<ExcessReturnPosition, _> = serde_json::from_str(json);
    assert!(result.is_err());
}
