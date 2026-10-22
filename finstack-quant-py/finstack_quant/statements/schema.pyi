"""Discover, retrieve, and validate the canonical statements JSON Schemas.

Schema documents are rendered from the same Rust registry as the installed
extension. Functions return JSON text; parse it with ``json.loads``.

Examples
--------
>>> import json
>>> from finstack_quant.statements import schema
>>> json.loads(schema.get("financial_model_spec.schema.json"))["title"]
'FinancialModelSpec'
"""

from __future__ import annotations

__all__ = ["get", "index", "validate"]

def index() -> str:
    """
    List every JSON Schema this crate publishes.

    Returns
    -------
    str
        Pretty-printed JSON with an ``artifacts`` array. Each row carries
        ``path``, ``$id``, ``title``, ``type_name`` (the Rust root type), ``summary``, ``bytes`` and ``kind``
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
