"""
Every JSON Schema the workspace publishes, across all ten domains.

The per-domain namespaces (``finstack_quant.valuations.schema`` and friends)
list one crate each. This namespace merges all of them, so a service exposing
these contracts does not have to hard-code the domain list.

Each :func:`index` row carries ``domain`` alongside ``path``, ``$id``,
``title``, ``type_name`` (the Rust root type), ``summary``, ``bytes`` and ``kind``.

Examples
--------
>>> import json
>>> from finstack_quant import schema
>>> index = json.loads(schema.index())
>>> index["schema_index_version"]
1
>>> sorted({row["domain"] for row in index["artifacts"]})[:3]
['attribution', 'calibration', 'cashflows']

"""

from __future__ import annotations

__all__ = [
    "domains",
    "get",
    "index",
    "validate",
]

def index() -> str:
    """
    List every JSON Schema the workspace publishes, across all ten domains.

    Returns
    -------
    str
        Pretty-printed JSON with an ``artifacts`` array. Each row carries
        ``domain`` (the owning crate namespace), ``path``, ``$id``, ``title``,
        ``type_name`` (the Rust root type), ``summary``, ``bytes`` and ``kind`` (``input`` for documents you author,
        ``output`` for documents the library emits, ``component`` for shared
        definitions). Rows are sorted by ``domain`` then ``path``.

    Raises
    ------
    ValueError
        If an artifact cannot be rendered.

    Examples
    --------
    >>> import json
    >>> from finstack_quant import schema
    >>> len(json.loads(schema.index())["artifacts"]) > 100
    True

    """

def get(selector: str, profile: str = "canonical") -> str:
    """
    Fetch one JSON Schema by path, ``$id``, or filename, from any domain.

    Parameters
    ----------
    selector : str
        A ``path`` or ``$id`` from :func:`index`, or just the trailing filename
        such as ``"bond.schema.json"``. Filename matching is anchored to a path
        separator, so ``"bond.schema.json"`` never resolves to
        ``convertible_bond.schema.json``.
    profile : str, optional
        ``"canonical"`` (default) returns the published contract, which is what
        :func:`validate` checks against. ``"llm"`` returns the projection:
        self-contained, unit enums flattened, Rust prose trimmed to its leading
        paragraph, oversized definitions stood down to handles, and unreachable
        definitions pruned. The projection is deliberately **not** a validator.

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
    >>> from finstack_quant import schema
    >>> json.loads(schema.get("bond.schema.json"))["title"]
    'bond'

    """

def validate(selector: str, payload: str) -> str:
    """
    Validate a JSON payload against one published schema, from any domain.

    Union failures are reported at the offending field rather than at the
    enclosing ``oneOf``, and unit-enum mismatches list the accepted spellings.

    Parameters
    ----------
    selector : str
        A ``path`` or ``$id`` from :func:`index`, or just the trailing filename.
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
    >>> from finstack_quant import schema
    >>> json.loads(schema.validate("bond.schema.json", "{}")) != []
    True

    """

def domains() -> list[str]:
    """
    List the domain namespaces that publish schemas.

    Returns
    -------
    list of str
        Sorted domain names, each of which is also a ``domain`` value in
        :func:`index` and a ``finstack_quant.<domain>.schema`` namespace.

    Raises
    ------
    ValueError
        If the registry cannot be read.

    Examples
    --------
    >>> from finstack_quant import schema
    >>> "valuations" in schema.domains()
    True

    """
