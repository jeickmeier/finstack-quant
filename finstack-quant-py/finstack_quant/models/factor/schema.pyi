"""
Compiled-in JSON Schemas for the factor-model wire format.

The generic factor-model configuration and the three credit artifacts
(hierarchy model, calibration configuration, calibration inputs) are embedded
in the extension module, so they always match the installed version and cannot
drift from the wheel that ships them.

Each accessor returns JSON *text*; parse it with ``json.loads`` or hand it
straight to a validator such as ``jsonschema.Draft202012Validator``.

Examples
--------
>>> import json
>>> from finstack_quant.models.factor import schema
>>> json.loads(schema.credit_factor_model_schema())["title"]
'CreditFactorModel'

"""

from __future__ import annotations

__all__ = [
    "credit_calibration_config_schema",
    "credit_calibration_inputs_schema",
    "credit_factor_model_schema",
    "factor_model_config_schema",
    "get",
    "index",
    "validate",
]

def credit_calibration_config_schema() -> str:
    """
    Return the JSON Schema for a serialized ``CreditCalibrationConfig``.

    This is the contract for the deterministic credit factor-model calibrator:
    estimation windows, shrinkage settings, and solver controls.

    Returns
    -------
    str
        Pretty-printed JSON Schema text for the credit calibration
        configuration.

    Raises
    ------
    RuntimeError
        If the compiled-in schema is malformed JSON.
    ValueError
        If the parsed schema cannot be serialized back to JSON text.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.models.factor import schema
    >>> json.loads(schema.credit_calibration_config_schema())["title"]
    'CreditCalibrationConfig'

    """

def credit_calibration_inputs_schema() -> str:
    """
    Return the JSON Schema for a serialized ``CreditCalibrationInputs``.

    This is the contract for one calibration run's inputs: typed issuer
    histories, tags, generic factor series, the anchor date, and any overrides.

    Returns
    -------
    str
        Pretty-printed JSON Schema text for the credit calibration inputs.

    Raises
    ------
    RuntimeError
        If the compiled-in schema is malformed JSON.
    ValueError
        If the parsed schema cannot be serialized back to JSON text.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.models.factor import schema
    >>> json.loads(schema.credit_calibration_inputs_schema())["title"]
    'CreditCalibrationInputs'

    """

def credit_factor_model_schema() -> str:
    """
    Return the JSON Schema for a serialized ``CreditFactorModel``.

    This is the fully self-contained credit factor hierarchy artifact accepted
    by :meth:`finstack_quant.models.factor.credit.CreditFactorModel.from_json`.

    Returns
    -------
    str
        Pretty-printed JSON Schema text for the credit factor hierarchy model.

    Raises
    ------
    RuntimeError
        If the compiled-in schema is malformed JSON.
    ValueError
        If the parsed schema cannot be serialized back to JSON text.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.models.factor import schema
    >>> json.loads(schema.credit_factor_model_schema())["title"]
    'CreditFactorModel'

    """

def factor_model_config_schema() -> str:
    """
    Return the JSON Schema for a versioned factor-model configuration.

    This is the contract for the generic (non-credit) factor-model envelope:
    typed factors, covariance settings, matching rules, pricing, and risk
    configuration.

    Returns
    -------
    str
        Pretty-printed JSON Schema text for the factor-model configuration
        envelope.

    Raises
    ------
    RuntimeError
        If the compiled-in schema is malformed JSON.
    ValueError
        If the parsed schema cannot be serialized back to JSON text.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.models.factor import schema
    >>> json.loads(schema.factor_model_config_schema())["title"]
    'Finstack Quant Factor Model Configuration'

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
    >>> from finstack_quant.models.factor import schema
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
    >>> from finstack_quant.models.factor import schema
    >>> json.loads(schema.get("factor_model_config.schema.json"))["$schema"]
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
    >>> from finstack_quant.models.factor import schema
    >>> json.loads(schema.validate("factor_model_config.schema.json", "{}")) != []
    True

    """
