"""
Type stubs for ``finstack_quant.valuations.credit_derivatives``.

Canonical example payloads for CDS-family instruments (CDS, index, tranche,
option). Each factory returns tagged instrument JSON accepted by
``finstack_quant.valuations.instruments.validate_instrument_json`` and
``price_instrument``.

Examples
--------
>>> import finstack_quant.valuations.credit_derivatives as credit_derivatives
>>> credit_derivatives.__name__
'finstack_quant.valuations.credit_derivatives'
"""

from __future__ import annotations

__all__ = [
    "cds_index_example_json",
    "cds_option_example_json",
    "cds_tranche_example_json",
    "credit_default_swap_example_json",
]

def cds_index_example_json() -> str:
    """Example tagged ``CDSIndex`` instrument JSON.

    Returns
    -------
    str
        Tagged instrument JSON (``{"type": "cds_index", ...}``) accepted by
        ``validate_instrument_json`` and ``price_instrument``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations.credit_derivatives import cds_index_example_json
    >>> json.loads(cds_index_example_json())["type"]
    'cds_index'
    """

def cds_option_example_json() -> str:
    """Example tagged ``CDSOption`` instrument JSON.

    Returns
    -------
    str
        Tagged instrument JSON (``{"type": "cds_option", ...}``) accepted by
        ``validate_instrument_json`` and ``price_instrument``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations.credit_derivatives import cds_option_example_json
    >>> json.loads(cds_option_example_json())["type"]
    'cds_option'
    """

def cds_tranche_example_json() -> str:
    """Example tagged ``CDSTranche`` instrument JSON.

    Returns
    -------
    str
        Tagged instrument JSON (``{"type": "cds_tranche", ...}``) accepted by
        ``validate_instrument_json`` and ``price_instrument``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations.credit_derivatives import cds_tranche_example_json
    >>> json.loads(cds_tranche_example_json())["type"]
    'cds_tranche'
    """

def credit_default_swap_example_json() -> str:
    """Example tagged ``CreditDefaultSwap`` instrument JSON.

    Returns
    -------
    str
        Tagged instrument JSON (``{"type": "credit_default_swap", ...}``)
        accepted by ``validate_instrument_json`` and ``price_instrument``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations.credit_derivatives import (
    ...     credit_default_swap_example_json,
    ... )
    >>> json.loads(credit_default_swap_example_json())["type"]
    'credit_default_swap'
    """
