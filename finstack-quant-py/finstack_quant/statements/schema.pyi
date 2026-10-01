"""
Compiled-in JSON Schemas for the statements wire format.

The model specification, the evaluated statement result, and the EBITDA
normalization configuration schemas are embedded in the extension module, so
they always match the installed version and cannot drift from the wheel that
ships them.

Each accessor returns JSON *text*; parse it with ``json.loads`` or hand it
straight to a validator such as ``jsonschema.Draft202012Validator``.

Examples
--------
>>> import json
>>> from finstack_quant.statements import schema
>>> json.loads(schema.financial_model_spec_schema())["title"]
'FinancialModelSpec'

"""

from __future__ import annotations

__all__ = [
    "financial_model_spec_schema",
    "get",
    "index",
    "normalization_config_schema",
    "statement_result_schema",
    "validate",
]

def financial_model_spec_schema() -> str:
    """
    Return the JSON Schema for a serialized ``FinancialModelSpec``.

    This is the contract for a statements model definition: periods, nodes,
    forecast specifications, and formulas.

    Returns
    -------
    str
        Pretty-printed JSON Schema text for the financial model specification.

    Raises
    ------
    ValueError
        If the compiled-in schema is malformed or cannot be serialized back to
        JSON text.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.statements import schema
    >>> json.loads(schema.financial_model_spec_schema())["title"]
    'FinancialModelSpec'

    """

def normalization_config_schema() -> str:
    """
    Return the JSON Schema for a serialized ``NormalizationConfig``.

    This is the contract for EBITDA normalization: the adjustment items, their
    sign conventions, and the add-back policy applied during normalization.

    Returns
    -------
    str
        Pretty-printed JSON Schema text for the normalization configuration.

    Raises
    ------
    ValueError
        If the compiled-in schema is malformed or cannot be serialized back to
        JSON text.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.statements import schema
    >>> json.loads(schema.normalization_config_schema())["title"]
    'NormalizationConfig'

    """

def statement_result_schema() -> str:
    """
    Return the JSON Schema for a serialized ``StatementResult``.

    This describes per-period node values and execution statistics: numeric
    mode, node and period counts, optional elapsed time, parallel execution,
    and evaluation warnings. Monetary node maps retain amounts and currencies.

    Returns
    -------
    str
        Pretty-printed JSON Schema text for the statement result envelope.

    Raises
    ------
    ValueError
        If the compiled-in schema is malformed or cannot be serialized back to
        JSON text.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.statements import schema
    >>> json.loads(schema.statement_result_schema())["title"]
    'StatementResult'

    """

def index() -> str:
    """
    List every JSON Schema this crate publishes.

    Returns
    -------
    str
        Pretty-printed JSON with an ``artifacts`` array. Each row carries
        ``path``, ``$id``, ``title``, ``summary``, ``bytes`` and ``kind``
        (``input`` for documents you author, ``output`` for documents the
        library emits, ``component`` for shared definitions).

    Raises
    ------
    ValueError
        If an artifact cannot be rendered.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.statements import schema
    >>> json.loads(schema.index())["schema_index_version"]
    1

    """

def get(selector: str, profile: str = "canonical") -> str:
    """
    Fetch one JSON Schema by path, ``$id``, or filename.

    Parameters
    ----------
    selector : str
        A ``path`` or ``$id`` from :func:`index`, or just the trailing
        filename.
    profile : str, optional
        ``"canonical"`` (default) returns the published contract, which is what
        :func:`validate` checks against. ``"llm"`` returns the projection:
        self-contained, unit enums flattened, Rust prose stripped, and shaped
        for structured-output subsets. The projection is deliberately **not** a
        validator.

    Returns
    -------
    str
        Pretty-printed JSON Schema text.

    Raises
    ------
    KeyError
        If no artifact matches ``selector``.
    ValueError
        If ``profile`` is unknown, or the artifact cannot be rendered.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.statements import schema
    >>> json.loads(schema.get("financial_model_spec.schema.json"))["$schema"]
    'https://json-schema.org/draft/2020-12/schema'

    """

def validate(selector: str, payload: str) -> str:
    """
    Validate a JSON payload against one published schema.

    Parameters
    ----------
    selector : str
        A ``path`` or ``$id`` from :func:`index`, or just the trailing
        filename.
    payload : str
        JSON text to check.

    Returns
    -------
    str
        Pretty-printed JSON array of failures, each with ``pointer`` (a JSON
        Pointer into the payload) and ``message``. An empty array means the
        payload validates.

    Raises
    ------
    KeyError
        If no artifact matches ``selector``.
    ValueError
        If ``payload`` is not valid JSON, or the schema cannot be built.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.statements import schema
    >>> json.loads(schema.validate("financial_model_spec.schema.json", "{}")) != []
    True

    """
