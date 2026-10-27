"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_structured_credit/coverage_rules.rs``."""

from __future__ import annotations

from typing import Any

class CoverageRules:
    """
    Collateral valuation rules for the OC tests: rating haircuts, the value
    carried for defaulted collateral, the excess-CCC bucket and discount
    obligations (CLO indenture conventions). Percentages are percent values
    (``7.5`` = 7.5%); haircuts are decimal fractions.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CoverageRules
    >>> rules = CoverageRules.clo_standard()
    >>> rules.ccc_bucket["threshold_pct"]
    7.5
    >>> CoverageRules(rating_haircuts={"NR": 0.5}).rating_haircuts
    {'NR': 0.5}
    """

    def __init__(
        self,
        rating_haircuts: dict[str, float] | str | None = None,
        defaulted_valuation: dict[str, Any] | str | None = None,
        ccc_bucket: dict[str, Any] | str | None = None,
        discount_obligation: dict[str, Any] | str | None = None,
    ) -> None:
        """
        Construct rules from their serde parts (every part optional).

        Parameters
        ----------
        rating_haircuts : dict[str, float] | str, optional
            Haircut per rating as a decimal fraction (``{"CCC": 0.7, "NR":
            0.5}``); performing collateral is carried at ``par × (1 − haircut)``.
            Empty when omitted.
        defaulted_valuation : dict[str, Any] | str, optional
            ``DefaultedValuation`` serde value: ``"recovery"`` (modeled recovery,
            the default) or ``{"market_value": {"pct": 60.0}}``.
        ccc_bucket : dict[str, Any] | str, optional
            ``CccBucketRule`` (``threshold_pct``, ``treatment``); ``None``
            disables the bucket.
        discount_obligation : dict[str, Any] | str, optional
            ``DiscountObligationRule`` (``price_threshold_pct``); ``None``
            disables it.

        Raises
        ------
        ValueError
            If a part does not match its serde shape or the rules fail
            validation.
        """
        ...

    @staticmethod
    def clo_standard() -> CoverageRules:
        """
        Standard CLO rules: performing collateral at par (no rating haircuts),
        defaulted collateral at recovery,
        a 7.5% CCC bucket at market value and discount obligations below 80.

        Returns
        -------
        CoverageRules
            The standard rules.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CoverageRules
        >>> CoverageRules.clo_standard().discount_obligation["price_threshold_pct"]
        80.0
        """
        ...

    def validate(self) -> None:
        """
        Validate haircuts, percentages and thresholds.

        Raises
        ------
        ValueError
            If a haircut is outside ``[0, 1]`` or a percent is out of range.
        """
        ...

    @staticmethod
    def from_json(json: str) -> CoverageRules:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``CoverageRules`` (the shape ``to_json`` writes).

        Returns
        -------
        CoverageRules
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed, carries unknown fields, or fails the same
            canonical Rust validation as construction (invalid ranges, dates or rules).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CoverageRules
        >>> CoverageRules.from_json("{}").ccc_bucket is None
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded ``CoverageRules``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(CoverageRules.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    @property
    def rating_haircuts(self) -> dict[str, float]:
        """
        Haircut per rating as decimal fractions.

        Returns
        -------
        dict[str, float]
            Rating string to haircut.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def defaulted_valuation(self) -> Any:
        """
        ``DefaultedValuation`` serde value.

        Returns
        -------
        Any
            ``"recovery"`` or ``{"market_value": {"pct": ...}}``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def ccc_bucket(self) -> dict[str, Any] | None:
        """
        ``CccBucketRule`` serde dict.

        Returns
        -------
        dict[str, Any] | None
            ``None`` when the bucket is disabled.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def discount_obligation(self) -> dict[str, Any] | None:
        """
        ``DiscountObligationRule`` serde dict.

        Returns
        -------
        dict[str, Any] | None
            ``None`` when disabled.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
