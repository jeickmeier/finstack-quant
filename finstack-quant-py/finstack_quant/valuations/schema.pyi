"""
Compiled-in JSON Schemas for the valuations wire format.

The instrument envelope, the per-type instrument schemas, and the valuation
result schema are embedded in the extension module, so they always match the
installed version and cannot drift from the wheel that ships them.

Each accessor returns JSON *text*; parse it with ``json.loads`` or hand it
straight to a validator such as ``jsonschema.Draft202012Validator``.

Examples
--------
>>> import json
>>> from finstack_quant.valuations import schema
>>> json.loads(schema.instrument_envelope_schema())["title"]
'Finstack Quant Instrument'

"""

from __future__ import annotations

__all__ = [
    "get",
    "index",
    "instrument_envelope_schema",
    "instrument_schema",
    "instrument_types",
    "validate",
    "validate_instrument_envelope_json",
    "validate_instrument_type_json",
    "valuation_result_schema",
]

def instrument_envelope_schema() -> str:
    """
    Return the JSON Schema for the canonical instrument envelope.

    The envelope is the ``finstack_quant.instrument/1`` wrapper carrying a
    ``type`` discriminator alongside the matching typed ``spec`` payload. Use
    it to validate, document, or generate forms for any instrument payload
    without knowing its concrete type up front.

    Returns
    -------
    str
        Pretty-printed JSON Schema text for the instrument envelope. The
        document is large (roughly one megabyte); read it once and cache the
        parsed result rather than calling this inside a loop.

    Raises
    ------
    ValueError
        If the compiled-in schema is malformed or cannot be serialized back to
        JSON text.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations import schema
    >>> json.loads(schema.instrument_envelope_schema())["title"]
    'Finstack Quant Instrument'

    """

def instrument_types() -> list[str]:
    """
    Return every canonical instrument discriminator, in registry order.

    The tagged-JSON registry is the single source of truth for decoding and
    schema generation, so this is the authoritative set of values accepted in
    an envelope's ``instrument.type`` field.

    Returns
    -------
    list of str
        Registered instrument type tags, for example ``"bond"`` and
        ``"interest_rate_swap"``. Every entry resolves through
        :func:`instrument_schema`.

    Raises
    ------
    ValueError
        If the compiled-in instrument registry cannot be read.

    Examples
    --------
    >>> from finstack_quant.valuations import schema
    >>> "bond" in schema.instrument_types()
    True

    """

def instrument_schema(instrument_type: str) -> str:
    """
    Return the dedicated JSON Schema for one instrument type.

    Parameters
    ----------
    instrument_type : str
        Canonical registry discriminator such as ``"bond"`` or
        ``"interest_rate_swap"``. Call :func:`instrument_types` for the
        complete set of valid values.

    Returns
    -------
    str
        Pretty-printed JSON Schema text for that instrument type, including at
        least one worked ``examples`` entry.

    Raises
    ------
    KeyError
        If ``instrument_type`` is not a registered discriminator. Call
        :func:`instrument_types` for the valid set.
    ValueError
        If the compiled-in schema for that type is malformed.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations import schema
    >>> json.loads(schema.instrument_schema("bond"))["title"]
    'bond'

    """

def valuation_result_schema() -> str:
    """
    Return the JSON Schema for a serialized ``ValuationResult``.

    This is the shape of the pricing output envelope: present value, currency,
    measures, and policy stamps.

    Returns
    -------
    str
        Pretty-printed JSON Schema text for the valuation result envelope.

    Raises
    ------
    ValueError
        If the compiled-in schema is malformed or cannot be serialized back to
        JSON text.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations import schema
    >>> json.loads(schema.valuation_result_schema())["title"]
    'Valuation Result'

    """

def validate_instrument_envelope_json(instrument_json: str) -> str:
    """
    Validate an instrument envelope payload against the canonical schemas.

    Both the envelope schema and the schema selected by ``instrument.type`` are
    applied, exactly as the Rust loader does when it decodes tagged instrument
    JSON.

    Parameters
    ----------
    instrument_json : str
        JSON text of a ``finstack_quant.instrument/1`` envelope. Pass
        ``json.dumps(payload)`` when starting from a Python dictionary.

    Returns
    -------
    str
        Canonical compact JSON of the validated payload. A failure raises
        rather than returning a falsy value, so the individual schema
        violations are never discarded.

    Raises
    ------
    ValueError
        If ``instrument_json`` is not valid JSON, or if it violates the
        envelope or the selected type schema. The message enumerates every
        violation with its instance path.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations import schema
    >>> example = json.loads(schema.instrument_schema("bond"))["examples"][0]
    >>> json.loads(schema.validate_instrument_envelope_json(json.dumps(example)))["schema"]
    'finstack_quant.instrument/1'

    """

def validate_instrument_type_json(instrument_type: str, instrument_json: str) -> str:
    """
    Validate a payload against one specific instrument type's schema.

    Unlike :func:`validate_instrument_envelope_json`, the type is chosen by the
    caller rather than read from the payload, which is what you want when
    checking that a draft payload conforms to an intended instrument type.

    Parameters
    ----------
    instrument_type : str
        Canonical registry discriminator whose schema is used for validation.
        Call :func:`instrument_types` for the complete set of valid values.
    instrument_json : str
        JSON text to validate against that type schema. Pass
        ``json.dumps(payload)`` when starting from a Python dictionary.

    Returns
    -------
    bool
        ``True`` when the payload validates. A failure raises rather than
        returning ``False``, so the individual schema violations are never
        discarded.

    Raises
    ------
    KeyError
        If ``instrument_type`` is not a registered discriminator.
    ValueError
        If ``instrument_json`` is not valid JSON, or if it violates the
        selected type schema.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations import schema
    >>> example = json.loads(schema.instrument_schema("bond"))["examples"][0]
    >>> json.loads(schema.validate_instrument_type_json("bond", json.dumps(example)))["schema"]
    'finstack_quant.instrument/1'

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
    >>> from finstack_quant.valuations import schema
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
    >>> from finstack_quant.valuations import schema
    >>> json.loads(schema.get("bond.schema.json"))["$schema"]
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
    >>> from finstack_quant.valuations import schema
    >>> json.loads(schema.validate("bond.schema.json", "{}")) != []
    True

    """
