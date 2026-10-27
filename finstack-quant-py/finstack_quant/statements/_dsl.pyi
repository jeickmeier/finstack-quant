"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements/dsl.rs``."""

from __future__ import annotations

def parse_formula(formula: str) -> str:
    """
    Parse a DSL formula and render it back as canonical DSL source text.

    Parentheses are emitted only where operator precedence requires them, so
    the returned text re-parses to an identical expression.

    Parameters
    ----------
    formula : str
        Source expression in the statements DSL.

    Returns
    -------
    str
        Canonical rendering of the parsed expression.

    Raises
    ------
    ValueError
        If ``formula`` is not valid statements DSL.

    Examples
    --------
    >>> from finstack_quant.statements import parse_formula
    >>> parse_formula("(revenue - cogs)/revenue")
    '(revenue - cogs) / revenue'

    """
    ...

def parse_and_compile(formula: str) -> None:
    """
    Validate that ``formula`` parses and compiles successfully.

    Parameters
    ----------
    formula : str
        DSL expression to validate.

    Returns
    -------
    None
        Returns nothing on success. Validation is reported by raising, so
        ``if parse_and_compile(f):`` is not a validity check.

    Raises
    ------
    ValueError
        If parsing or compilation fails.

    Examples
    --------
    >>> from finstack_quant.statements import parse_and_compile
    >>> parse_and_compile("revenue - cogs") is None
    True

    """
    ...
