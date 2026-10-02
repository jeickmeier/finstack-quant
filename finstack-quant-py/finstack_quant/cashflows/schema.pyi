"""
Compiled-in JSON Schemas for the cashflow wire format.

Every cashflow schema (the amortization, coupon, default, fee, prepayment,
recovery and schedule specifications, plus the schedule, build-spec, accrual
and aggregation contracts) is embedded in the extension module, so they always
match the installed version and cannot drift from the wheel that ships them.
These are the documents that resolve the
``https://finstack_quant.dev/schemas/cashflow/1/...`` references appearing
inside instrument schemas.

Examples
--------
>>> import json
>>> from finstack_quant.cashflows import schema
>>> base = "https://finstack_quant.dev/schemas/cashflow/1/"
>>> json.loads(schema.resources()[base + "schedule_params.schema.json"])["title"]
'ScheduleParams'

"""

from __future__ import annotations

__all__ = [
    "get",
    "index",
    "resources",
    "validate",
]

def resources() -> dict[str, str]:
    """
    Return every embedded cashflow schema, keyed by canonical URI.

    The keys are the published ``$id`` URIs used by ``$ref`` targets in
    instrument and cashflow payload schemas, so the mapping can be handed
    directly to a resolver-aware validator.

    Returns
    -------
    dict of str to str
        Mapping from canonical schema URI to pretty-printed JSON Schema text,
        one entry per embedded cashflow schema.

    Raises
    ------
    ValueError
        If a compiled-in schema is malformed or cannot be serialized
        back to JSON text.

    Examples
    --------
    >>> from finstack_quant.cashflows import schema
    >>> sorted(uri.rsplit("/", 1)[-1] for uri in schema.resources())[:2]
    ['accrual_config.schema.json', 'amortization_spec.schema.json']

    """

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
    >>> from finstack_quant.cashflows import schema
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
    >>> from finstack_quant.cashflows import schema
    >>> json.loads(schema.get("schedule_params.schema.json"))["$schema"]
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
    >>> from finstack_quant.cashflows import schema
    >>> json.loads(schema.validate("schedule_params.schema.json", "{}")) != []
    True

    """
