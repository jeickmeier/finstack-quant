"""Typed dict definitions for ``CalibrationEnvelope``.

These are documentation/typing aids for analysts who construct calibration
envelopes as Python dicts. They mirror the Rust `CalibrationEnvelope` schema
in [`finstack-valuations`] and produce JSON that ``calibrate`` and ``dry_run``
accept verbatim.

Coverage:

- Top-level structures: ``CalibrationEnvelope``, ``CalibrationPlan``,
  ``CalibrationStep``.
- All calibration step kinds: ``discount``, ``forward``, ``hazard``,
  ``inflation``, ``vol_surface``, ``swaption_vol``, ``base_correlation``,
  ``student_t``, ``hull_white``, ``cap_floor_hull_white``, ``svi_surface``,
  ``xccy_basis``, ``parametric``.
- Shared building blocks: ``Tenor``, ``Pillar``, ``CdsConventionKey``.
- All ``MarketQuote`` variants: rate (deposit, FRA, futures, swap), CDS (par
  spread, upfront), CDS tranche, FX (forward outright, swap outright, vanilla
  option), inflation (zero-coupon swap, year-on-year), vol (option, swaption,
  cap/floor), cross-currency basis swap, bond (clean price, Z-spread, OAS,
  YTM).
- All ``MarketDatum`` variants (17): the eight quote variants plus FX spot,
  price, dividend schedule, fixing series, inflation fixings, credit index,
  FX vol surface, vol cube, collateral.
- All ``PriorMarketObject`` variants (10): discount, forward, hazard,
  inflation, base correlation, basis spread, parametric, price, volatility
  index curves, and vol surface.

These TypedDicts are documentation only — no runtime validation. Use
``dry_run`` (Phase 4) for structural checks.

Note on serde tagging:

The Rust schema mixes internally tagged and externally tagged enums. Where
an enum carries ``#[serde(tag = "type")]`` (RateQuote, CdsQuote, CDSTrancheQuote,
FxQuote, XccyQuote, BondQuote) the variant fields are flattened. Where it
lacks a tag (InflationQuote, VolQuote) the variant is nested under a
snake_case variant key. The TypedDict shapes below mirror the JSON exactly.

Untyped envelopes still type-check thanks to the ``| dict[str, Any]`` escape
hatch on the public unions.
"""

from __future__ import annotations

from typing import Any, Literal, NotRequired, TypedDict

# =============================================================================
# Shared building blocks
# =============================================================================


class Tenor(TypedDict):
    """A relative tenor like ``5Y`` or ``3M``.

    Maps to ``finstack_core::dates::Tenor``. Serializes as
    ``{"count": 5, "unit": "years"}``.
    """

    count: int
    unit: Literal["days", "weeks", "months", "years"]


class TenorPillar(TypedDict):
    """The ``Tenor`` arm of the ``Pillar`` tagged union.

    Serializes as ``{"tenor": {"count": 5, "unit": "years"}}``.
    """

    tenor: Tenor


class DatePillar(TypedDict):
    """The absolute-date arm of the ``Pillar`` tagged union.

    Serializes as ``{"date": "2027-05-08"}``.
    """

    date: str


# `Pillar` is a snake_case-tagged enum on the Rust side; serde emits one
# variant per dict (`tenor` arm carries a Tenor; `date` arm carries an ISO
# date string).
Pillar = TenorPillar | DatePillar


class CdsConventionKey(TypedDict):
    """Currency + doc-clause pairing identifying CDS market conventions."""

    currency: str
    doc_clause: str


# =============================================================================
# MarketQuote variants
# =============================================================================
#
# `MarketQuote` is `#[serde(tag = "class", rename_all = "snake_case")]`. Each
# inner enum is either internally tagged via `type` (rates, cds, cds_tranche,
# fx, xccy, bond) or externally tagged (inflation, vol). Internally tagged
# variants flatten their fields next to `class`; externally tagged variants
# nest fields under a snake_case variant key.

# --- rates -----------------------------------------------------------------

RateDeposit = TypedDict(
    "RateDeposit",
    {
        "class": Literal["rates"],
        "type": Literal["deposit"],
        "id": str,
        "index": str,
        "pillar": Pillar,
        "rate": float,
    },
)
"""A money-market deposit rate quote."""

RateFra = TypedDict(
    "RateFra",
    {
        "class": Literal["rates"],
        "type": Literal["fra"],
        "id": str,
        "index": str,
        "start": Pillar,
        "end": Pillar,
        "rate": float,
    },
)
"""A forward-rate-agreement (FRA) quote."""

RateFutures = TypedDict(
    "RateFutures",
    {
        "class": Literal["rates"],
        "type": Literal["futures"],
        "id": str,
        "contract": str,
        "expiry": str,
        "price": float,
        "convexity_adjustment": NotRequired[float | None],
        "vol_surface_id": NotRequired[str | None],
    },
)
"""An interest-rate futures price quote."""

RateSwap = TypedDict(
    "RateSwap",
    {
        "class": Literal["rates"],
        "type": Literal["swap"],
        "id": str,
        "index": str,
        "pillar": Pillar,
        "rate": float,
        "spread_decimal": NotRequired[float | None],
    },
)
"""A vanilla IRS par-rate quote."""

# --- cds -------------------------------------------------------------------

CdsParSpread = TypedDict(
    "CdsParSpread",
    {
        "class": Literal["cds"],
        "type": Literal["cds_par_spread"],
        "id": str,
        "entity": str,
        "convention": CdsConventionKey,
        "pillar": Pillar,
        "spread_bp": float,
        "recovery_rate": float,
    },
)
"""A CDS par-spread quote."""

CdsUpfront = TypedDict(
    "CdsUpfront",
    {
        "class": Literal["cds"],
        "type": Literal["cds_upfront"],
        "id": str,
        "entity": str,
        "convention": CdsConventionKey,
        "pillar": Pillar,
        "running_spread_bp": float,
        "upfront_pct": float,
        "recovery_rate": float,
    },
)
"""A CDS upfront + running coupon quote."""

# --- cds_tranche -----------------------------------------------------------

CdsTrancheQuote = TypedDict(
    "CdsTrancheQuote",
    {
        "class": Literal["cds_tranche"],
        "type": Literal["cds_tranche"],
        "id": str,
        "index": str,
        "attachment": float,
        "detachment": float,
        "maturity": str,
        "upfront_pct": float,
        "running_spread_bp": float,
        "convention": CdsConventionKey,
    },
)
"""A CDS index tranche quote (attachment/detachment with upfront and running spread)."""

# --- fx --------------------------------------------------------------------

FxForwardOutright = TypedDict(
    "FxForwardOutright",
    {
        "class": Literal["fx"],
        "type": Literal["forward_outright"],
        "id": str,
        "convention": str,
        "pillar": Pillar,
        "forward_rate": float,
    },
)
"""An outright FX forward quote."""

FxSwapOutright = TypedDict(
    "FxSwapOutright",
    {
        "class": Literal["fx"],
        "type": Literal["swap_outright"],
        "id": str,
        "convention": str,
        "far_pillar": Pillar,
        "near_rate": float,
        "far_rate": float,
    },
)
"""A spot-start FX swap quote with explicit near/far outrights."""

FxOptionVanilla = TypedDict(
    "FxOptionVanilla",
    {
        "class": Literal["fx"],
        "type": Literal["option_vanilla"],
        "id": str,
        "convention": str,
        "expiry": str,
        "strike": float,
        "option_type": Literal["call", "put"],
        "vol_surface_id": str,
    },
)
"""A European vanilla FX option quote."""

# --- inflation (externally tagged; payload nested under variant key) ------


class InflationSwapPayload(TypedDict):
    """Zero-coupon inflation swap payload (nested under ``inflation_swap``)."""

    id: str
    maturity: str
    rate: float
    index: str
    convention: str


class YoyInflationSwapPayload(TypedDict):
    """Year-on-year inflation swap payload (nested under ``yo_y_inflation_swap``)."""

    id: str
    maturity: str
    rate: float
    index: str
    frequency: Tenor
    convention: str


InflationSwapQuote = TypedDict(
    "InflationSwapQuote",
    {
        "class": Literal["inflation"],
        "inflation_swap": InflationSwapPayload,
    },
)
"""Zero-coupon inflation swap quote."""

YoyInflationSwapQuote = TypedDict(
    "YoyInflationSwapQuote",
    {
        "class": Literal["inflation"],
        "yo_y_inflation_swap": YoyInflationSwapPayload,
    },
)
"""Year-on-year inflation swap quote.

The serde variant name is ``YoYInflationSwap`` and ``rename_all = "snake_case"``
converts it to ``yo_y_inflation_swap`` (matching how serde lowercases each
character run between case boundaries).
"""

# --- vol (externally tagged) -----------------------------------------------


class OptionVolPayload(TypedDict):
    """Equity/commodity option implied-vol payload."""

    id: str
    underlying: str
    expiry: str
    strike: float
    vol: float
    option_type: Literal["call", "put"]
    convention: str


class SwaptionVolPayload(TypedDict):
    """Swaption implied-vol payload."""

    id: str
    expiry: str
    maturity: str
    strike: float
    vol: float
    quote_type: str
    convention: str


class CapFloorVolPayload(TypedDict):
    """Cap/floor implied-vol payload."""

    id: str
    expiry: str
    strike: float
    vol: float
    quote_type: str
    is_cap: bool
    convention: str


OptionVolQuote = TypedDict(
    "OptionVolQuote",
    {
        "class": Literal["vol"],
        "option_vol": OptionVolPayload,
    },
)
"""Equity/commodity option implied-vol quote."""

SwaptionVolQuote = TypedDict(
    "SwaptionVolQuote",
    {
        "class": Literal["vol"],
        "swaption_vol": SwaptionVolPayload,
    },
)
"""Interest-rate swaption implied-vol quote."""

CapFloorVolQuote = TypedDict(
    "CapFloorVolQuote",
    {
        "class": Literal["vol"],
        "cap_floor_vol": CapFloorVolPayload,
    },
)
"""Cap/floor implied-vol quote."""

# --- xccy ------------------------------------------------------------------

XccyBasisSwapQuote = TypedDict(
    "XccyBasisSwapQuote",
    {
        "class": Literal["xccy"],
        "type": Literal["basis_swap"],
        "id": str,
        "convention": str,
        "far_pillar": Pillar,
        "basis_spread_bp": float,
        "spot_fx": NotRequired[float | None],
    },
)
"""A cross-currency basis-swap quote."""

# --- bond ------------------------------------------------------------------

BondFixedRateBulletCleanPrice = TypedDict(
    "BondFixedRateBulletCleanPrice",
    {
        "class": Literal["bond"],
        "type": Literal["fixed_rate_bullet_clean_price"],
        "id": str,
        "currency": str,
        "issue_date": str,
        "maturity": str,
        "coupon_rate": float,
        "convention": str,
        "clean_price_pct": float,
    },
)
"""A fixed-rate bullet bond quoted in clean price (% of par)."""

BondFixedRateBulletZSpread = TypedDict(
    "BondFixedRateBulletZSpread",
    {
        "class": Literal["bond"],
        "type": Literal["fixed_rate_bullet_z_spread"],
        "id": str,
        "currency": str,
        "issue_date": str,
        "maturity": str,
        "coupon_rate": float,
        "convention": str,
        "z_spread": float,
    },
)
"""A fixed-rate bullet bond quoted in Z-spread (decimal)."""

BondFixedRateBulletOas = TypedDict(
    "BondFixedRateBulletOas",
    {
        "class": Literal["bond"],
        "type": Literal["fixed_rate_bullet_oas"],
        "id": str,
        "currency": str,
        "issue_date": str,
        "maturity": str,
        "coupon_rate": float,
        "convention": str,
        "oas": float,
    },
)
"""A fixed-rate bullet bond quoted in OAS (decimal)."""

BondFixedRateBulletYtm = TypedDict(
    "BondFixedRateBulletYtm",
    {
        "class": Literal["bond"],
        "type": Literal["fixed_rate_bullet_ytm"],
        "id": str,
        "currency": str,
        "issue_date": str,
        "maturity": str,
        "coupon_rate": float,
        "convention": str,
        "ytm": float,
    },
)
"""A fixed-rate bullet bond quoted in yield-to-maturity (decimal)."""


# Union of typed MarketQuote variants plus an untyped escape hatch.
MarketQuote = (
    RateDeposit
    | RateFra
    | RateFutures
    | RateSwap
    | CdsParSpread
    | CdsUpfront
    | CdsTrancheQuote
    | FxForwardOutright
    | FxSwapOutright
    | FxOptionVanilla
    | InflationSwapQuote
    | YoyInflationSwapQuote
    | OptionVolQuote
    | SwaptionVolQuote
    | CapFloorVolQuote
    | XccyBasisSwapQuote
    | BondFixedRateBulletCleanPrice
    | BondFixedRateBulletZSpread
    | BondFixedRateBulletOas
    | BondFixedRateBulletYtm
    | dict[str, Any]
)

# =============================================================================
# MarketDatum variants
# =============================================================================
#
# `MarketDatum` is `#[serde(tag = "kind", rename_all = "snake_case")]`. Quote
# variants flatten or nest the inner enum exactly as the wrapped `MarketQuote`
# class does (without the outer `class` tag); snapshot variants are tuple
# enums that flatten their wrapped struct fields.

# --- quote-bearing variants ------------------------------------------------


class RateQuoteDepositDatum(TypedDict):
    """Deposit rate quote as a flat ``MarketDatum``."""

    kind: Literal["rate_quote"]
    type: Literal["deposit"]
    id: str
    index: str
    pillar: Pillar
    rate: float


class RateQuoteFraDatum(TypedDict):
    """FRA quote as a flat ``MarketDatum``."""

    kind: Literal["rate_quote"]
    type: Literal["fra"]
    id: str
    index: str
    start: Pillar
    end: Pillar
    rate: float


class RateQuoteFuturesDatum(TypedDict):
    """Interest-rate futures quote as a flat ``MarketDatum``."""

    kind: Literal["rate_quote"]
    type: Literal["futures"]
    id: str
    contract: str
    expiry: str
    price: float
    convexity_adjustment: NotRequired[float | None]
    vol_surface_id: NotRequired[str | None]


class RateQuoteSwapDatum(TypedDict):
    """IRS par-rate quote as a flat ``MarketDatum``."""

    kind: Literal["rate_quote"]
    type: Literal["swap"]
    id: str
    index: str
    pillar: Pillar
    rate: float
    spread_decimal: NotRequired[float | None]


class CdsParSpreadDatum(TypedDict):
    """CDS par-spread quote as a flat ``MarketDatum``."""

    kind: Literal["cds_quote"]
    type: Literal["cds_par_spread"]
    id: str
    entity: str
    convention: CdsConventionKey
    pillar: Pillar
    spread_bp: float
    recovery_rate: float


class CdsUpfrontDatum(TypedDict):
    """CDS upfront + running quote as a flat ``MarketDatum``."""

    kind: Literal["cds_quote"]
    type: Literal["cds_upfront"]
    id: str
    entity: str
    convention: CdsConventionKey
    pillar: Pillar
    running_spread_bp: float
    upfront_pct: float
    recovery_rate: float


class CdsTrancheDatum(TypedDict):
    """CDS tranche quote as a flat ``MarketDatum``."""

    kind: Literal["cds_tranche_quote"]
    type: Literal["cds_tranche"]
    id: str
    index: str
    attachment: float
    detachment: float
    maturity: str
    upfront_pct: float
    running_spread_bp: float
    convention: CdsConventionKey


class FxForwardOutrightDatum(TypedDict):
    """FX outright-forward quote as a flat ``MarketDatum``."""

    kind: Literal["fx_quote"]
    type: Literal["forward_outright"]
    id: str
    convention: str
    pillar: Pillar
    forward_rate: float


class FxSwapOutrightDatum(TypedDict):
    """FX swap-outright quote as a flat ``MarketDatum``."""

    kind: Literal["fx_quote"]
    type: Literal["swap_outright"]
    id: str
    convention: str
    far_pillar: Pillar
    near_rate: float
    far_rate: float


class FxOptionVanillaDatum(TypedDict):
    """FX vanilla-option quote as a flat ``MarketDatum``."""

    kind: Literal["fx_quote"]
    type: Literal["option_vanilla"]
    id: str
    convention: str
    expiry: str
    strike: float
    option_type: Literal["call", "put"]
    vol_surface_id: str


class InflationSwapDatum(TypedDict):
    """Zero-coupon inflation swap quote as a ``MarketDatum`` (nested payload)."""

    kind: Literal["inflation_quote"]
    inflation_swap: InflationSwapPayload


class YoyInflationSwapDatum(TypedDict):
    """Year-on-year inflation swap quote as a ``MarketDatum`` (nested payload)."""

    kind: Literal["inflation_quote"]
    yo_y_inflation_swap: YoyInflationSwapPayload


class OptionVolDatum(TypedDict):
    """Equity/commodity option vol quote as a ``MarketDatum`` (nested payload)."""

    kind: Literal["vol_quote"]
    option_vol: OptionVolPayload


class SwaptionVolDatum(TypedDict):
    """Swaption vol quote as a ``MarketDatum`` (nested payload)."""

    kind: Literal["vol_quote"]
    swaption_vol: SwaptionVolPayload


class CapFloorVolDatum(TypedDict):
    """Cap/floor vol quote as a ``MarketDatum`` (nested payload)."""

    kind: Literal["vol_quote"]
    cap_floor_vol: CapFloorVolPayload


class XccyBasisSwapDatum(TypedDict):
    """Cross-currency basis-swap quote as a flat ``MarketDatum``."""

    kind: Literal["xccy_quote"]
    type: Literal["basis_swap"]
    id: str
    convention: str
    far_pillar: Pillar
    basis_spread_bp: float
    spot_fx: NotRequired[float | None]


class BondCleanPriceDatum(TypedDict):
    """Fixed-rate bullet bond clean-price quote as a flat ``MarketDatum``."""

    kind: Literal["bond_quote"]
    type: Literal["fixed_rate_bullet_clean_price"]
    id: str
    currency: str
    issue_date: str
    maturity: str
    coupon_rate: float
    convention: str
    clean_price_pct: float


class BondZSpreadDatum(TypedDict):
    """Fixed-rate bullet bond Z-spread quote as a flat ``MarketDatum``."""

    kind: Literal["bond_quote"]
    type: Literal["fixed_rate_bullet_z_spread"]
    id: str
    currency: str
    issue_date: str
    maturity: str
    coupon_rate: float
    convention: str
    z_spread: float


class BondOasDatum(TypedDict):
    """Fixed-rate bullet bond OAS quote as a flat ``MarketDatum``."""

    kind: Literal["bond_quote"]
    type: Literal["fixed_rate_bullet_oas"]
    id: str
    currency: str
    issue_date: str
    maturity: str
    coupon_rate: float
    convention: str
    oas: float


class BondYtmDatum(TypedDict):
    """Fixed-rate bullet bond YTM quote as a flat ``MarketDatum``."""

    kind: Literal["bond_quote"]
    type: Literal["fixed_rate_bullet_ytm"]
    id: str
    currency: str
    issue_date: str
    maturity: str
    coupon_rate: float
    convention: str
    ytm: float


# --- snapshot variants -----------------------------------------------------

# `from` is a Python keyword, so this TypedDict must use the functional form
# to keep the key name exactly ``"from"`` as required by the Rust schema.
FxSpotDatum = TypedDict(
    "FxSpotDatum",
    {
        "kind": Literal["fx_spot"],
        "id": str,
        "from": str,
        "to": str,
        "rate": float,
    },
)
"""FX-spot rate snapshot. Uses ``from``/``to`` ISO currency codes."""


class PriceDatum(TypedDict):
    """Single-name spot price (``MarketScalar``-wrapping) snapshot."""

    kind: Literal["price"]
    id: str
    scalar: dict[str, Any]


class DividendScheduleDatum(TypedDict):
    """Dividend schedule snapshot.

    The wrapped ``DividendSchedule`` does not derive ``JsonSchema`` directly
    on the Rust side, so the inner ``schedule`` field is modelled as an
    open dict.
    """

    kind: Literal["dividend_schedule"]
    schedule: dict[str, Any]


class FixingSeriesDatum(TypedDict):
    """Generic scalar time series snapshot (CPI, historical fixings, ...).

    Wraps ``ScalarTimeSeries`` which serializes with internal id/observations
    fields. The exact wire shape is intentionally left as an open dict
    because ``ScalarTimeSeries`` does not derive ``JsonSchema``.
    """

    kind: Literal["fixing_series"]
    id: NotRequired[str]
    observations: NotRequired[list[dict[str, Any]]]


class InflationFixingsDatum(TypedDict):
    """Inflation index fixings snapshot.

    Wraps ``InflationIndex``; the underlying type carries a ``HashMap`` of
    fixings keyed by ``NaiveDate`` that TypedDicts cannot represent cleanly,
    so the bulk of the payload is left as an open dict.
    """

    kind: Literal["inflation_fixings"]
    id: NotRequired[str]


class CreditIndexDatum(TypedDict):
    """Credit-index reference-state snapshot."""

    kind: Literal["credit_index"]
    id: NotRequired[str]


class FxVolSurfaceDatum(TypedDict):
    """FX delta-vol surface snapshot."""

    kind: Literal["fx_vol_surface"]
    id: NotRequired[str]


class VolCubeDatum(TypedDict):
    """Generic vol-cube snapshot."""

    kind: Literal["vol_cube"]
    id: NotRequired[str]


class CollateralDatum(TypedDict):
    """Collateral / CSA mapping entry.

    The Rust struct's ``id`` field is a ``Currency`` enum; the JSON value
    is the ISO currency string (e.g. ``"USD"``).
    """

    kind: Literal["collateral"]
    id: str
    csa_currency: str


# Union of all typed `MarketDatum` variants plus an untyped escape hatch.
MarketDatum = (
    RateQuoteDepositDatum
    | RateQuoteFraDatum
    | RateQuoteFuturesDatum
    | RateQuoteSwapDatum
    | CdsParSpreadDatum
    | CdsUpfrontDatum
    | CdsTrancheDatum
    | FxForwardOutrightDatum
    | FxSwapOutrightDatum
    | FxOptionVanillaDatum
    | InflationSwapDatum
    | YoyInflationSwapDatum
    | OptionVolDatum
    | SwaptionVolDatum
    | CapFloorVolDatum
    | XccyBasisSwapDatum
    | BondCleanPriceDatum
    | BondZSpreadDatum
    | BondOasDatum
    | BondYtmDatum
    | FxSpotDatum
    | PriceDatum
    | DividendScheduleDatum
    | FixingSeriesDatum
    | InflationFixingsDatum
    | CreditIndexDatum
    | FxVolSurfaceDatum
    | VolCubeDatum
    | CollateralDatum
    | dict[str, Any]
)

# =============================================================================
# PriorMarketObject variants
# =============================================================================
#
# `PriorMarketObject` is `#[serde(tag = "kind", rename_all = "snake_case")]`.
# Each variant wraps a curve/surface struct that does not derive
# ``JsonSchema`` directly on the Rust side, so the wrapped payload is
# modelled as an open dict. The TypedDict still pins the ``kind``
# discriminator to a literal so mypy catches typos there.


class DiscountCurvePrior(TypedDict):
    """Pre-built discount-factor curve."""

    kind: Literal["discount_curve"]
    id: NotRequired[str]


class ForwardCurvePrior(TypedDict):
    """Pre-built forward-rate curve."""

    kind: Literal["forward_curve"]
    id: NotRequired[str]


class HazardCurvePrior(TypedDict):
    """Pre-built default hazard-rate curve."""

    kind: Literal["hazard_curve"]
    id: NotRequired[str]


class InflationCurvePrior(TypedDict):
    """Pre-built inflation (breakeven / index) curve."""

    kind: Literal["inflation_curve"]
    id: NotRequired[str]


class BaseCorrelationCurvePrior(TypedDict):
    """Pre-built CDS-index base-correlation curve."""

    kind: Literal["base_correlation_curve"]
    id: NotRequired[str]


class BasisSpreadCurvePrior(TypedDict):
    """Pre-built tenor-basis spread curve."""

    kind: Literal["basis_spread_curve"]
    id: NotRequired[str]


class ParametricCurvePrior(TypedDict):
    """Pre-built parametric (e.g. Nelson-Siegel) curve."""

    kind: Literal["parametric_curve"]
    id: NotRequired[str]


class PriceCurvePrior(TypedDict):
    """Pre-built spot / forward price curve."""

    kind: Literal["price_curve"]
    id: NotRequired[str]


class VolatilityIndexCurvePrior(TypedDict):
    """Pre-built volatility-index forward curve."""

    kind: Literal["volatility_index_curve"]
    id: NotRequired[str]


class VolSurfacePrior(TypedDict):
    """Pre-built volatility surface (expiry x strike)."""

    kind: Literal["vol_surface"]
    id: NotRequired[str]


PriorMarketObject = (
    DiscountCurvePrior
    | ForwardCurvePrior
    | HazardCurvePrior
    | InflationCurvePrior
    | BaseCorrelationCurvePrior
    | BasisSpreadCurvePrior
    | ParametricCurvePrior
    | PriceCurvePrior
    | VolatilityIndexCurvePrior
    | VolSurfacePrior
    | dict[str, Any]
)

# =============================================================================
# Calibration step variants
# =============================================================================


class DiscountStep(TypedDict):
    """A ``discount`` calibration step.

    Builds a discount factor curve from money-market quotes (deposits + IRS).
    """

    id: str
    quote_set: str
    kind: Literal["discount"]
    curve_id: str
    currency: str
    base_date: str
    method: NotRequired[str]
    interpolation: NotRequired[str]
    extrapolation: NotRequired[str]
    pricing_discount_id: NotRequired[str | None]
    pricing_forward_id: NotRequired[str | None]
    conventions: NotRequired[dict[str, Any]]


class ForwardStep(TypedDict):
    """A ``forward`` calibration step.

    Builds a forward (projection) curve at a given tenor against a
    pre-existing discount curve.
    """

    id: str
    quote_set: str
    kind: Literal["forward"]
    curve_id: str
    currency: str
    base_date: str
    tenor_years: float
    discount_curve_id: str
    method: NotRequired[str]
    interpolation: NotRequired[str]
    conventions: NotRequired[dict[str, Any]]


class HazardStep(TypedDict):
    """A ``hazard`` calibration step.

    Builds a hazard (default-intensity) curve from CDS par-spread or upfront
    quotes against a discount curve.
    """

    id: str
    quote_set: str
    kind: Literal["hazard"]
    curve_id: str
    entity: str
    seniority: str
    currency: str
    base_date: str
    discount_curve_id: str
    recovery_rate: NotRequired[float]
    notional: NotRequired[float]
    method: NotRequired[str]
    interpolation: NotRequired[str]
    par_interp: NotRequired[str]
    doc_clause: NotRequired[str]
    cds_valuation_convention: NotRequired[str | None]


class InflationStep(TypedDict):
    """An ``inflation`` calibration step.

    Builds a zero-coupon inflation curve from ZCIS or YoY quotes against a
    discount curve.
    """

    id: str
    quote_set: str
    kind: Literal["inflation"]
    curve_id: str
    currency: str
    base_date: str
    discount_curve_id: str
    index: str
    observation_lag: str
    base_cpi: float
    notional: NotRequired[float]
    method: NotRequired[str]
    interpolation: NotRequired[str]
    seasonal_factors: NotRequired[dict[str, Any] | None]


class VolSurfaceStep(TypedDict):
    """A ``vol_surface`` calibration step (SABR-only today).

    Builds an equity / index volatility surface.
    """

    id: str
    quote_set: str
    kind: Literal["vol_surface"]
    surface_id: str
    base_date: str
    underlying_ticker: str
    model: str
    discount_curve_id: NotRequired[str | None]
    beta: NotRequired[float]
    target_expiries: NotRequired[list[float]]
    target_strikes: NotRequired[list[float]]
    spot_override: NotRequired[float | None]
    dividend_yield_override: NotRequired[float | None]
    expiry_extrapolation: NotRequired[str]


class SwaptionVolStep(TypedDict):
    """A ``swaption_vol`` calibration step.

    Builds a swaption volatility surface (per-bucket SABR) from swaption
    quotes against a discount curve.
    """

    id: str
    quote_set: str
    kind: Literal["swaption_vol"]
    surface_id: str
    base_date: str
    discount_curve_id: str
    currency: str
    forward_id: NotRequired[str | None]
    vol_convention: NotRequired[str | dict[str, Any]]
    atm_convention: NotRequired[str]
    sabr_beta: NotRequired[float]
    target_expiries: NotRequired[list[float]]
    target_tenors: NotRequired[list[float]]
    sabr_interpolation: NotRequired[str]
    calendar_id: NotRequired[str | None]
    fixed_day_count: NotRequired[str | None]
    swap_index: NotRequired[str | None]
    vol_tolerance: NotRequired[float | None]
    sabr_tolerance: NotRequired[float | None]
    sabr_extrapolation: NotRequired[str]
    allow_sabr_missing_bucket_fallback: NotRequired[bool]


class BaseCorrelationStep(TypedDict):
    """A ``base_correlation`` calibration step.

    Builds a base-correlation curve from CDS tranche quotes.
    """

    id: str
    quote_set: str
    kind: Literal["base_correlation"]
    index_id: str
    series: int
    maturity_years: float
    base_date: str
    discount_curve_id: str
    currency: str
    notional: NotRequired[float]
    frequency: NotRequired[Tenor | None]
    day_count: NotRequired[str | None]
    bdc: NotRequired[str | None]
    calendar_id: NotRequired[str | None]
    detachment_points: NotRequired[list[float]]
    use_imm_dates: NotRequired[bool]


class StudentTStep(TypedDict):
    """A ``student_t`` calibration step.

    Calibrates the degrees-of-freedom parameter of a Student-t copula
    against a tranche upfront target.
    """

    id: str
    quote_set: str
    kind: Literal["student_t"]
    tranche_instrument_id: str
    base_correlation_curve_id: str
    discount_curve_id: NotRequired[str | None]
    initial_df: NotRequired[float]
    df_bounds: NotRequired[tuple[float, float] | list[float]]
    correlation: NotRequired[float]


class HullWhiteStep(TypedDict):
    """A ``hull_white`` calibration step.

    Calibrates the 1-factor Hull-White short-rate model to European
    swaption prices.
    """

    id: str
    quote_set: str
    kind: Literal["hull_white"]
    curve_id: str
    currency: str
    base_date: str
    initial_kappa: NotRequired[float | None]
    initial_sigma: NotRequired[float | None]


class CapFloorHullWhiteStep(TypedDict):
    """A ``cap_floor_hull_white`` calibration step.

    Calibrates 1-factor Hull-White to cap/floor volatility quotes.
    """

    id: str
    quote_set: str
    kind: Literal["cap_floor_hull_white"]
    discount_curve_id: str
    forward_curve_id: str
    currency: str
    base_date: str
    fixed_kappa: NotRequired[float | None]
    initial_kappa: NotRequired[float | None]
    initial_sigma: NotRequired[float | None]
    payment_frequency: NotRequired[str]


class SviSurfaceStep(TypedDict):
    """An ``svi_surface`` calibration step.

    Fits a per-expiry SVI parameterization to market-implied vols.
    """

    id: str
    quote_set: str
    kind: Literal["svi_surface"]
    surface_id: str
    base_date: str
    underlying_ticker: str
    discount_curve_id: NotRequired[str | None]
    target_expiries: NotRequired[list[float]]
    target_strikes: NotRequired[list[float]]
    spot_override: NotRequired[float | None]


class XccyBasisStep(TypedDict):
    """An ``xccy_basis`` calibration step.

    Derives a foreign-currency discount curve from a domestic OIS curve,
    FX spot, and cross-currency basis-swap or FX-forward quotes.
    """

    id: str
    quote_set: str
    kind: Literal["xccy_basis"]
    curve_id: str
    currency: str
    base_date: str
    fx_spot: float
    domestic_discount_id: str
    method: NotRequired[str]
    interpolation: NotRequired[str]
    extrapolation: NotRequired[str]
    conventions: NotRequired[dict[str, Any]]
    basis_spread_curve_id: NotRequired[str | None]


class ParametricStep(TypedDict):
    """A ``parametric`` calibration step.

    Fits a Nelson-Siegel or NSS curve via Levenberg-Marquardt.
    """

    id: str
    quote_set: str
    kind: Literal["parametric"]
    curve_id: str
    base_date: str
    model: str
    discount_curve_id: NotRequired[str | None]
    initial_params: NotRequired[dict[str, Any] | None]


CalibrationStep = (
    DiscountStep
    | ForwardStep
    | HazardStep
    | InflationStep
    | VolSurfaceStep
    | SwaptionVolStep
    | BaseCorrelationStep
    | StudentTStep
    | HullWhiteStep
    | CapFloorHullWhiteStep
    | SviSurfaceStep
    | XccyBasisStep
    | ParametricStep
    | dict[str, Any]
)

# =============================================================================
# Top-level
# =============================================================================


class CalibrationPlan(TypedDict):
    """The plan inside a `CalibrationEnvelope`."""

    id: str
    description: NotRequired[str | None]
    # Named ID lists; each ID must resolve to a quote-kind entry in `market_data`.
    quote_sets: dict[str, list[str]]
    steps: list[CalibrationStep]
    settings: NotRequired[dict[str, Any]]


CalibrationEnvelope = TypedDict(
    "CalibrationEnvelope",
    {
        "$schema": NotRequired[str],
        "schema": Literal["finstack.calibration"],
        "plan": CalibrationPlan,
        "market_data": NotRequired[list[MarketDatum]],
        "prior_market": NotRequired[list[PriorMarketObject]],
    },
)
"""Top-level envelope accepted by ``calibrate`` / ``dry_run``.

Construct with::

    envelope: CalibrationEnvelope = {
        "schema": "finstack.calibration",
        "plan": {
            "id": "usd_curves",
            "quote_sets": {"usd_quotes": ["USD-SOFR-DEP-1M", "USD-OIS-SWAP-1Y"]},
            "steps": [...],
            "settings": {},
        },
        "market_data": [
            {"kind": "rate_quote", "type": "deposit", "id": "USD-SOFR-DEP-1M", ...},
            {"kind": "rate_quote", "type": "swap",    "id": "USD-OIS-SWAP-1Y", ...},
        ],
    }

``market_data`` carries the flat list of inputs (quotes + snapshot data).
``prior_market`` carries pre-built calibrated objects from a previous run.
Both fields are optional and default to empty.
"""


__all__ = [
    "BaseCorrelationCurvePrior",
    "BaseCorrelationStep",
    "BasisSpreadCurvePrior",
    "BondCleanPriceDatum",
    "BondFixedRateBulletCleanPrice",
    "BondFixedRateBulletOas",
    "BondFixedRateBulletYtm",
    "BondFixedRateBulletZSpread",
    "BondOasDatum",
    "BondYtmDatum",
    "BondZSpreadDatum",
    "CalibrationEnvelope",
    "CalibrationPlan",
    "CalibrationStep",
    "CapFloorHullWhiteStep",
    "CapFloorVolDatum",
    "CapFloorVolPayload",
    "CapFloorVolQuote",
    "CdsConventionKey",
    "CdsParSpread",
    "CdsParSpreadDatum",
    "CdsTrancheDatum",
    "CdsTrancheQuote",
    "CdsUpfront",
    "CdsUpfrontDatum",
    "CollateralDatum",
    "CreditIndexDatum",
    "DatePillar",
    "DiscountCurvePrior",
    "DiscountStep",
    "DividendScheduleDatum",
    "FixingSeriesDatum",
    "ForwardCurvePrior",
    "ForwardStep",
    "FxForwardOutright",
    "FxForwardOutrightDatum",
    "FxOptionVanilla",
    "FxOptionVanillaDatum",
    "FxSpotDatum",
    "FxSwapOutright",
    "FxSwapOutrightDatum",
    "FxVolSurfaceDatum",
    "HazardCurvePrior",
    "HazardStep",
    "HullWhiteStep",
    "InflationCurvePrior",
    "InflationFixingsDatum",
    "InflationStep",
    "InflationSwapDatum",
    "InflationSwapPayload",
    "InflationSwapQuote",
    "MarketDatum",
    "MarketQuote",
    "OptionVolDatum",
    "OptionVolPayload",
    "OptionVolQuote",
    "ParametricCurvePrior",
    "ParametricStep",
    "Pillar",
    "PriceCurvePrior",
    "PriceDatum",
    "PriorMarketObject",
    "RateDeposit",
    "RateFra",
    "RateFutures",
    "RateQuoteDepositDatum",
    "RateQuoteFraDatum",
    "RateQuoteFuturesDatum",
    "RateQuoteSwapDatum",
    "RateSwap",
    "StudentTStep",
    "SviSurfaceStep",
    "SwaptionVolDatum",
    "SwaptionVolPayload",
    "SwaptionVolQuote",
    "SwaptionVolStep",
    "Tenor",
    "TenorPillar",
    "VolCubeDatum",
    "VolSurfacePrior",
    "VolSurfaceStep",
    "VolatilityIndexCurvePrior",
    "XccyBasisStep",
    "XccyBasisSwapDatum",
    "XccyBasisSwapQuote",
    "YoyInflationSwapDatum",
    "YoyInflationSwapPayload",
    "YoyInflationSwapQuote",
]
