"""
Type stubs for ``finstack_quant.valuations.credit_derivatives``.

Canonical example payloads for CDS-family instruments (CDS, index, tranche,
option). Each factory returns a canonical v1 instrument envelope accepted by
``finstack_quant.valuations.instruments.validate_instrument_json`` and
``price_instrument``.

Examples
--------
>>> import json
>>> from finstack_quant.valuations.credit_derivatives import cds_index_example_json
>>> json.loads(cds_index_example_json())["instrument"]["type"]
'cds_index'

"""

from __future__ import annotations

__all__ = [
    "cds_index_example_json",
    "cds_option_example_json",
    "cds_tranche_example_json",
    "credit_default_swap_example_json",
]

def cds_index_example_json() -> str:
    """
    Example ``CdsIndex`` canonical instrument envelope.

    Returns
    -------
    str
        ``finstack_quant.instrument/1`` envelope accepted by
        ``validate_instrument_json`` and ``price_instrument``.

    Raises
    ------
    ValueError
        If the example or catalog cannot be produced.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations.credit_derivatives import cds_index_example_json
    >>> json.loads(cds_index_example_json())["instrument"]["type"]
    'cds_index'
    """

def cds_option_example_json() -> str:
    """
    Example ``CdsOption`` canonical instrument envelope.

    Returns
    -------
    str
        ``finstack_quant.instrument/1`` envelope accepted by
        ``validate_instrument_json`` and ``price_instrument``.

    Raises
    ------
    ValueError
        If the example or catalog cannot be produced.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations.credit_derivatives import cds_option_example_json
    >>> json.loads(cds_option_example_json())["instrument"]["type"]
    'cds_option'
    """

def cds_tranche_example_json() -> str:
    """
    Example ``CdsTranche`` canonical instrument envelope.

    Returns
    -------
    str
        ``finstack_quant.instrument/1`` envelope accepted by
        ``validate_instrument_json`` and ``price_instrument``.

    Raises
    ------
    ValueError
        If the example or catalog cannot be produced.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations.credit_derivatives import cds_tranche_example_json
    >>> json.loads(cds_tranche_example_json())["instrument"]["type"]
    'cds_tranche'
    """

def credit_default_swap_example_json() -> str:
    """
    Example ``CreditDefaultSwap`` canonical instrument envelope.

    Returns
    -------
    str
        ``finstack_quant.instrument/1`` envelope accepted by
        ``validate_instrument_json`` and ``price_instrument``.

    Raises
    ------
    ValueError
        If the example or catalog cannot be produced.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations.credit_derivatives import (
    ...     credit_default_swap_example_json,
    ... )
    >>> json.loads(credit_default_swap_example_json())["instrument"]["type"]
    'credit_default_swap'
    """
