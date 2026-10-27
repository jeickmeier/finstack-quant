"""Stub declarations mirroring ``finstack-quant-py/src/bindings/margin/im_curvature.rs``."""

from __future__ import annotations

class SimmCurvatureSensitivity:
    """Expiry-resolved historical SIMM curvature input, before scaling and netting.

    Equity and non-qualifying credit currently use residual buckets. IR bucket
    is a currency; qualifying credit uses a sector; commodity uses 1..17; FX
    uses ``fx``. The factor is a subcurve, issuer, underlier or ordered FX pair.

    Examples
    --------
    >>> from finstack_quant.margin import SimmCurvatureSensitivity
    >>> c = SimmCurvatureSensitivity("equity", "residual", "ACME", "1Y", 1000.0)
    >>> c.volatility_weighted_vega
    1000.0
    """

    def __init__(
        self,
        risk_class: str,
        bucket: str,
        factor: str,
        expiry_tenor: str,
        volatility_weighted_vega: float,
        risk_tenor: str | None = None,
    ) -> None:
        """Create one factor's volatility-weighted vega at one option expiry.

        Parameters
        ----------
        risk_class : str
            One of interest_rate, credit_qualifying, credit_non_qualifying, equity, commodity or fx.
        bucket : str
            Class-specific currency, credit sector, commodity number, fx or residual bucket.
        factor : str
            Subcurve, issuer or underlier ID; FX uses an ordered pair such as EUR/USD.
        expiry_tenor : str
            SIMM option expiry label (2W through 30Y); used for curvature scaling before netting.
        volatility_weighted_vega : float
            Signed sigma times dPV/dsigma in the container's currency, before SF, HVR, VRW or concentration. Non-IR sigma follows SIMM paragraph 10(b).
        risk_tenor : str | None, default None
            IR underlying maturity (12 SIMM tenors) or credit tenor (1Y, 2Y, 3Y, 5Y, 10Y); absent for other classes.

        Raises
        ------
        ValueError
            If the class, bucket, factor, tenor combination or finite amount is invalid.
        """
        ...

    @property
    def risk_class(self) -> str:
        """Risk class selecting the SIMM correlation and scaling rules.

        Returns
        -------
        str
            Risk class selecting the SIMM correlation and scaling rules.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def bucket(self) -> str:
        """Currency, sector, numbered commodity, fx or residual bucket.

        Returns
        -------
        str
            Currency, sector, numbered commodity, fx or residual bucket.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def factor(self) -> str:
        """Subcurve, issuer, underlier or ordered FX pair identifier.

        Returns
        -------
        str
            Subcurve, issuer, underlier or ordered FX pair identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expiry_tenor(self) -> str:
        """Option expiry used for curvature scaling before netting.

        Returns
        -------
        str
            Option expiry used for curvature scaling before netting.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def risk_tenor(self) -> str | None:
        """Underlying IR or credit risk tenor; None for other risk classes.

        Returns
        -------
        str | None
            Underlying IR or credit risk tenor; None for other risk classes.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def volatility_weighted_vega(self) -> float:
        """Signed sigma times dPV/dsigma in the sensitivity currency before scaling.

        Returns
        -------
        float
            Signed sigma times dPV/dsigma in the sensitivity currency before scaling.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
