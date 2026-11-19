"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements/dsl.rs``."""

from __future__ import annotations

def parse_formula(formula: str) -> str:
    """
    Parse a DSL formula and render it back as canonical DSL source text.

    Parentheses are emitted only where operator precedence requires them and
    logical negation is always written ``!``, so the returned text re-parses
    to an identical expression. ``!`` binds like unary minus (``!a > b`` is
    ``(!a) > b``); keyword ``not`` binds outside a comparison (``not a > b``
    is ``not (a > b)``); a nested comparison needs explicit parentheses.

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
        If ``formula`` is not one complete expression, an identifier has an
        empty dot-separated segment, or the formula exceeds 64 levels of
        nesting or 256 terms.

    Examples
    --------
    >>> from finstack_quant.statements import parse_formula
    >>> parse_formula("(revenue - cogs)/revenue")
    '(revenue - cogs) / revenue'
    >>> parse_formula("not a > b")
    '!(a > b)'

    """
    ...

def parse_and_compile(formula: str) -> None:
    """
    Validate that ``formula`` compiles to the shared expression AST for the statements evaluator.

    Compilation checks every branch, including one evaluation would skip:
    function names and argument counts (``cumsum``, ``cumprod``, ``cummin``,
    ``cummax``, ``std``, ``var`` and ``median`` take exactly one argument),
    capital-structure components, and that no ``lag``, ``shift``, ``diff``,
    ``pct_change`` or ``growth_rate`` offset is a negative literal.

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
        If the formula does not parse, names an unknown function or
        capital-structure component, passes a wrong number of arguments, or
        reads a future period through a negative literal offset.

    Examples
    --------
    >>> from finstack_quant.statements import parse_and_compile
    >>> parse_and_compile("revenue - cogs") is None
    True

    """
    ...
