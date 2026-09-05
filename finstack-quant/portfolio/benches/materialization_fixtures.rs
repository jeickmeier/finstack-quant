//! Deterministic shared fixtures and gates for materialization benchmarks.

use finstack_quant_cashflows::builder::AmortizationSpec;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Month, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_portfolio::{
    flatten_dependencies, Entity, InstrumentArtifact, MaterializedPosition, PortfolioHeader,
    PortfolioMaterializationEnvelope, PositionId, PositionUnit,
};
use finstack_quant_valuations::instruments::fixed_income::bond::{Bond, CashflowSpec};
use finstack_quant_valuations::instruments::rates::cms_swap::{CmsSwap, FundingLegSpec};
use finstack_quant_valuations::instruments::{
    IRSConvention, Instrument, InstrumentEnvelope, InstrumentJson, MarketDependencies, PayReceive,
};
use indexmap::IndexMap;
use time::macros::date;

/// Number of artifacts in the unique-instrument acceptance fixture.
pub const FIXTURE_A_ARTIFACTS: usize = 5_000;
/// Number of artifacts in the deduplicated acceptance fixture.
pub const FIXTURE_B_ARTIFACTS: usize = 50;
/// Number of positions in both acceptance fixtures.
pub const FIXTURE_POSITION_COUNT: usize = 5_000;

/// Standard materialization benchmark workload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixtureKind {
    /// One schedule-rich artifact per position.
    UniqueScheduleRich,
    /// Many positions sharing a bounded set of schedule-rich artifacts.
    DeduplicatedScheduleRich,
}

/// Serialized deterministic benchmark fixture and its workload counters.
pub struct BenchmarkFixture {
    /// Stable short fixture label.
    pub name: &'static str,
    /// Complete UTF-8 materialization envelope.
    pub bytes: Vec<u8>,
    /// Number of encoded unique artifacts.
    pub artifact_count: usize,
    /// Number of ordered positions.
    pub position_count: usize,
    /// Sum of normalized market-dependency keys over unique artifacts.
    pub dependency_count: usize,
}

/// Build one deterministic schedule-rich benchmark fixture.
///
/// Artifacts alternate between ten-year semiannual amortizing bonds carrying a
/// complete twenty-step remaining-notional schedule and ten-year CMS swaps
/// whose CMS and funding legs serialize twenty explicit schedule rows each.
///
/// # Arguments
///
/// * `kind` - Unique or deduplicated position-to-artifact distribution.
/// * `artifact_count` - Number of unique instrument artifacts to encode.
/// * `position_count` - Number of ordered positions referencing those artifacts.
///
/// # Panics
///
/// Panics when either count is zero or when an internal benchmark builder or
/// deterministic JSON serialization fails.
#[must_use]
pub fn build_fixture(
    kind: FixtureKind,
    artifact_count: usize,
    position_count: usize,
) -> BenchmarkFixture {
    assert!(artifact_count > 0, "benchmark needs at least one artifact");
    assert!(position_count > 0, "benchmark needs at least one position");
    if kind == FixtureKind::UniqueScheduleRich {
        assert_eq!(
            artifact_count, position_count,
            "unique fixture requires one artifact per position"
        );
    }

    let mut instruments = Vec::with_capacity(artifact_count);
    let mut instrument_ids = Vec::with_capacity(artifact_count);
    let mut dependency_count = 0usize;
    for index in 0..artifact_count {
        let (instrument_id, envelope, dependencies) = if index % 2 == 0 {
            let bond = amortizing_bond(index);
            let dependencies = bond
                .market_dependencies()
                .expect("benchmark bond dependencies");
            (
                bond.id.to_string(),
                InstrumentEnvelope::new(InstrumentJson::Bond(bond)),
                dependencies,
            )
        } else {
            let swap = schedule_rich_swap(index);
            let dependencies = swap
                .market_dependencies()
                .expect("benchmark swap dependencies");
            (
                swap.id.to_string(),
                InstrumentEnvelope::new(InstrumentJson::CmsSwap(swap)),
                dependencies,
            )
        };
        dependency_count += flatten_dependencies(&dependencies).len();
        instrument_ids.push(instrument_id);
        instruments.push(artifact(index, envelope, dependencies));
    }

    let positions = (0..position_count)
        .map(|index| {
            let artifact_index = match kind {
                FixtureKind::UniqueScheduleRich => index,
                FixtureKind::DeduplicatedScheduleRich => index % artifact_count,
            };
            MaterializedPosition {
                id: PositionId::new(format!("position-{index:05}")),
                entity_id: "benchmark-entity".into(),
                instrument_id: instrument_ids[artifact_index].clone(),
                artifact_id: format!("artifact-{artifact_index:05}"),
                quantity: 1.0 + (index % 10) as f64,
                unit: PositionUnit::Units,
                attributes: IndexMap::new(),
                meta: IndexMap::new(),
            }
        })
        .collect();

    let envelope = PortfolioMaterializationEnvelope {
        schema: finstack_quant_portfolio::materialization::PortfolioMaterializationSchema::CURRENT,
        portfolio: PortfolioHeader {
            id: format!("materialization-{}", fixture_name(kind)),
            name: Some("Phase 7 deterministic benchmark".to_string()),
            base_currency: Currency::USD,
            as_of: date!(2025 - 01 - 01),
            entities: IndexMap::from([(
                "benchmark-entity".into(),
                Entity::new("benchmark-entity"),
            )]),
            books: IndexMap::new(),
            tags: IndexMap::from([("fixture".to_string(), fixture_name(kind).to_string())]),
            meta: IndexMap::new(),
        },
        instruments,
        positions,
        materializer: None,
    };
    BenchmarkFixture {
        name: fixture_name(kind),
        bytes: serde_json::to_vec(&envelope).expect("serialize deterministic benchmark fixture"),
        artifact_count,
        position_count,
        dependency_count,
    }
}

/// Build the full 5,000-unique-artifact acceptance fixture.
#[must_use]
pub fn fixture_a() -> BenchmarkFixture {
    build_fixture(
        FixtureKind::UniqueScheduleRich,
        FIXTURE_A_ARTIFACTS,
        FIXTURE_POSITION_COUNT,
    )
}

/// Build the full 5,000-position / 50-artifact deduplication fixture.
#[must_use]
pub fn fixture_b() -> BenchmarkFixture {
    build_fixture(
        FixtureKind::DeduplicatedScheduleRich,
        FIXTURE_B_ARTIFACTS,
        FIXTURE_POSITION_COUNT,
    )
}

fn fixture_name(kind: FixtureKind) -> &'static str {
    match kind {
        FixtureKind::UniqueScheduleRich => "materialization-a-5000-unique",
        FixtureKind::DeduplicatedScheduleRich => "materialization-b-5000-50",
    }
}

fn amortizing_bond(index: usize) -> Bond {
    let notional_amount = 1_000_000.0 + index as f64;
    let maturity = date!(2035 - 01 - 01);
    let schedule = (1..=20)
        .map(|step| {
            let (year, month) = if step % 2 == 1 {
                (2025 + step / 2, Month::July)
            } else {
                (2025 + step / 2, Month::January)
            };
            let schedule_date =
                Date::from_calendar_date(year, month, 1).expect("valid schedule date");
            let remaining = notional_amount * (20 - step) as f64 / 20.0;
            (
                schedule_date,
                Money::new(remaining, Currency::USD).expect("valid money fixture"),
            )
        })
        .collect();
    let coupon = 0.03 + (index % 200) as f64 / 100_000.0;
    let cashflows = CashflowSpec::amortizing(
        CashflowSpec::fixed(coupon, Tenor::semi_annual(), DayCount::Thirty360)
            .expect("finite benchmark coupon"),
        AmortizationSpec::StepRemaining { schedule },
    );
    Bond::builder()
        .id(format!("BOND-{index:05}").into())
        .notional(Money::new(notional_amount, Currency::USD).expect("valid money fixture"))
        .issue_date(date!(2025 - 01 - 01))
        .maturity(maturity)
        .cashflow_spec(cashflows)
        .discount_curve_id("USD-OIS".into())
        .build()
        .expect("benchmark amortizing bond")
}

fn schedule_rich_swap(index: usize) -> CmsSwap {
    let start = date!(2025 - 01 - 01);
    let end = date!(2035 - 01 - 01);
    CmsSwap::from_schedule(
        format!("SWAP-{index:05}"),
        start,
        end,
        Tenor::semi_annual(),
        10.0,
        (index % 50) as f64 / 100_000.0,
        FundingLegSpec::Floating {
            spread: (index % 25) as f64 / 100_000.0,
            day_count: DayCount::Act360,
            forward_curve_id: "USD-SOFR-3M".into(),
        },
        Money::new(2_000_000.0 + index as f64, Currency::USD).expect("valid money fixture"),
        DayCount::Act360,
        IRSConvention::UsdSofr,
        if index % 4 == 1 {
            PayReceive::Pay
        } else {
            PayReceive::Receive
        },
        "USD-OIS",
        "USD-SOFR-3M",
        "USD-SWAPTION-VOL",
    )
    .expect("benchmark schedule-rich swap")
}

fn artifact(
    index: usize,
    envelope: InstrumentEnvelope,
    dependencies: MarketDependencies,
) -> InstrumentArtifact {
    InstrumentArtifact {
        artifact_id: format!("artifact-{index:05}"),
        content_hash: Some(envelope.content_hash().expect("benchmark artifact hash")),
        envelope,
        dependencies: Some(dependencies),
    }
}
