"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/corkscrew.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd
from finstack_quant.statements import FinancialModelSpec, StatementResult

# Corkscrew (balance-sheet roll-forward) extension

class AccountType:
    """
    Balance-sheet account classifier: asset / liability / equity.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import AccountType
    >>> AccountType.Asset.value()
    'asset'

    """

    Asset: AccountType
    Liability: AccountType
    Equity: AccountType

    @staticmethod
    def from_str(value: str) -> AccountType:
        """
        Parse a balance-sheet account classification.

        Parameters
        ----------
        value : str
            Case-insensitive ``"asset"``, ``"liability"``, or ``"equity"`` value.

        Returns
        -------
        AccountType
            Enum member corresponding to the exact snake-case account type.

        Raises
        ------
        ValueError
            If value is not asset, liability, or equity.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import AccountType
        >>> AccountType.from_str("liability").value()
        'liability'

        """
        ...
    def value(self) -> str:
        """
        Return the canonical snake-case identifier for this variant.

        Returns
        -------
        str
            Exact JSON identifier: ``"asset"``, ``"liability"``, or ``"equity"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class CorkscrewAccount:
    """
    Map one balance-sheet account to its corkscrew input nodes.

    Identity: ``expected = prev + Σ changes − Σ decreases`` (or
    ``beginning + Σ changes − Σ decreases`` when
    ``beginning_balance_node`` is set). Pair a roll-forward template with
    ``changes`` as increases and ``decreases`` as positive disposals.

    Parameters
    ----------
    node_id : str
        Statement node ID receiving the period-end account balance.
    account_type : AccountType
        Asset, liability, or equity classification controlling change signs.
    changes : list[str]
        Statement node IDs added to the opening balance (increases or signed
        net changes).
    decreases : list[str]
        Statement node IDs subtracted from the opening balance (positive
        repayments, outflows, disposals). Default empty.
    beginning_balance_node : str or None
        Optional node ID supplying the opening balance instead of an inferred
        first-period balance.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import AccountType, CorkscrewAccount
    >>> account = CorkscrewAccount("inventory_end", AccountType.Asset, ["purchases"], ["disposals"])
    >>> (account.node_id, account.changes, account.decreases)
    ('inventory_end', ['purchases'], ['disposals'])

    """

    def __init__(
        self,
        node_id: str,
        account_type: AccountType,
        changes: list[str] = ...,
        decreases: list[str] = ...,
        beginning_balance_node: str | None = None,
    ) -> None:
        """
        Map a balance-sheet account to its roll-forward input nodes.

        Parameters
        ----------
        node_id : str
            Statement node receiving the period-end account balance.
        account_type : AccountType
            Asset, liability, or equity classification controlling change signs.
        changes : list[str]
            Statement node IDs added to the opening balance.
        decreases : list[str]
            Statement node IDs subtracted from the opening balance. Default empty.
        beginning_balance_node : str or None, default None
            Optional explicit opening-balance node; ``None`` uses the account's
            prior-period balance.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    @property
    def node_id(self) -> str:
        """
        Node id of the balance account being rolled forward.

        Returns
        -------
        str
            Node id of the balance account.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def account_type(self) -> AccountType:
        """
        Balance-sheet classifier: asset, liability, or equity.

        Returns
        -------
        AccountType
            Balance-sheet classifier for the account.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def changes(self) -> list[str]:
        """
        Node ids of the period increases (or signed net changes) added to the
        balance.

        Identity: ``expected = prev + Σ changes − Σ decreases``. Prefer
        :attr:`decreases` for positive outflows so roll-forward decrease
        nodes do not need to be negated.

        Returns
        -------
        list[str]
            Node ids of the change series added to the balance.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def decreases(self) -> list[str]:
        """
        Node ids of positive decreases (repayments, outflows, disposals)
        subtracted from the balance.

        Returns
        -------
        list[str]
            Node ids of the decrease series subtracted from the balance.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def beginning_balance_node(self) -> str | None:
        """
        Node id overriding the beginning balance, or ``None`` to use the account's own
        prior-period closing balance.

        Returns
        -------
        str or None
            Beginning-balance override node id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def to_json(self) -> str:
        """
        Serialize this object to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `CorkscrewAccount`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> CorkscrewAccount:
        """
        Deserialize an account mapping from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload identifying the balance node, type, and change nodes.

        Returns
        -------
        CorkscrewAccount
            Validated `CorkscrewAccount` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates this type's serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import AccountType, CorkscrewAccount
        >>> account = CorkscrewAccount("cash", AccountType.Asset, ["cash_change"])
        >>> CorkscrewAccount.from_json(account.to_json()).account_type.value()
        'asset'

        """
        ...

class CorkscrewConfig:
    """
    Configure corkscrew roll-forward validation across balance accounts.

    Parameters
    ----------
    accounts : list[CorkscrewAccount]
        Account mappings to reconcile; defaults to an empty validation set.
    tolerance : float
        Absolute currency-unit tolerance allowed for reconciliation differences;
        defaults to ``0.01``.
    fail_on_error : bool
        Whether reconciliation errors abort execution instead of being reported.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CorkscrewConfig
    >>> config = CorkscrewConfig(tolerance=0.05)
    >>> (config.accounts, config.tolerance, config.fail_on_error)
    ([], 0.05, False)

    """

    def __init__(
        self,
        accounts: list[CorkscrewAccount] = ...,
        tolerance: float = 0.01,
        fail_on_error: bool = False,
    ) -> None:
        """
        Configure account roll-forwards and reconciliation failure policy.

        Parameters
        ----------
        accounts : list[CorkscrewAccount]
            Balance-sheet accounts and their change-node mappings.
        tolerance : float, default 0.01
            Maximum absolute reconciliation difference in model currency units.
        fail_on_error : bool, default False
            Whether any failed account reconciliation makes execution fail.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    @property
    def accounts(self) -> list[CorkscrewAccount]:
        """
        Balance accounts validated by this configuration, in configured order.

        Returns
        -------
        list[CorkscrewAccount]
            Validated balance accounts in configured order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def tolerance(self) -> float:
        """
        Absolute roll-forward tolerance, in the balance node's own units.

        A period is flagged when
        ``abs(closing - (opening + sum(changes) - sum(decreases))) >
        tolerance``.

        Returns
        -------
        float
            Absolute roll-forward tolerance in the balance node's units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def fail_on_error(self) -> bool:
        """
        When ``True``, any roll-forward violation is fatal (reported as an error) rather
        than a warning.

        Returns
        -------
        bool
            Whether roll-forward violations are treated as fatal.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def to_json(self) -> str:
        """
        Serialize this object to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `CorkscrewConfig`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> CorkscrewConfig:
        """
        Deserialize corkscrew validation settings from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing account mappings and reconciliation policy.

        Returns
        -------
        CorkscrewConfig
            Validated `CorkscrewConfig` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates this type's serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import CorkscrewConfig
        >>> config = CorkscrewConfig(tolerance=0.05)
        >>> CorkscrewConfig.from_json(config.to_json()).tolerance
        0.05

        """
        ...

class CorkscrewReport:
    """
    Report produced by ``CorkscrewExtension.execute``.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CorkscrewReport
    >>> report = CorkscrewReport.from_json('{"status":"success","message":"Balanced"}')
    >>> (report.status, report.message, report.warnings)
    ('success', 'Balanced', [])

    """

    @property
    def status(self) -> str:
        """
        Corkscrew run status: ``success`` when the walk completed, else ``failed``.

        Returns
        -------
        str
            Overall execution status.

        Raises
        ------
        RuntimeError
            If the stored enum cannot be rendered as its canonical Rust wire label.
        """
        ...
    @property
    def message(self) -> str:
        """
        Human-readable summary of the validation run.

        Returns
        -------
        str
            Summary message for the validation run.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def warnings(self) -> list[str]:
        """
        Non-fatal warnings, including roll-forward breaks reported when
        ``fail_on_error`` is ``False``.

        Returns
        -------
        list[str]
            Non-fatal warnings raised during validation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def errors(self) -> list[str]:
        """
        Roll-forward violations treated as fatal (``fail_on_error=True``) plus any
        structural failure. A non-empty list means ``status`` is ``"failed"``.

        Returns
        -------
        list[str]
            Fatal validation failures.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def data(self) -> dict[str, Any]:
        """
        Return the structured report payload as a Python dictionary.

        The dictionary is reconstructed from canonical JSON, so mutations to
        it do not modify the report. An absent payload is an empty dictionary.

        Returns
        -------
        dict[str, Any]
            A copy of the reconciliation payload with per-account validation
            status, periods checked and maximum absolute error. Missing payload
            fields remain absent; JSON null values become ``None``.

        Raises
        ------
        ValueError
            If the payload cannot be converted through canonical JSON.
        """
        ...
    def data_json(self) -> str:
        """
        Serialize report payload fields to JSON.

        Returns
        -------
        str
            JSON serialization of the structured reconciliation payload,
            including account-level balance checks and differences.

        Raises
        ------
        ValueError
            If the payload cannot be serialized to JSON.
        """
        ...
    def to_json(self) -> str:
        """
        Serialize this object to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `CorkscrewReport`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> CorkscrewReport:
        """
        Deserialize a corkscrew validation report from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload returned by a corkscrew extension execution.

        Returns
        -------
        CorkscrewReport
            Validated `CorkscrewReport` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates this type's serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import CorkscrewReport
        >>> CorkscrewReport.from_json('{"status":"success","message":"Balanced"}').status
        'success'

        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the report header as a single-row pandas ``DataFrame``.

        Columns: ``status``, ``message``, ``account_count``,
        ``warning_count``, ``error_count``.

        ``account_count`` is the number of validated accounts. Per-account
        detail lives in ``to_validations_dataframe``.

        Returns
        -------
        pd.DataFrame
            One row describing the validation run.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.

        RuntimeError
            If the stored enum cannot be rendered as its canonical Rust wire label.
        RuntimeError
            If the report status cannot be rendered as its canonical Rust wire label.
        """
        ...

    def to_validations_dataframe(self) -> pd.DataFrame:
        """
        Export the per-account roll-forward validations as a pandas
        ``DataFrame``.

        Columns: ``account``, ``type``, ``periods_validated``, ``max_error``,
        ``is_valid``. One row per validated account, in configured order; a
        report with no validations still carries the full column schema.

        ``type`` is the account classifier (``"asset"``, ``"liability"``,
        ``"equity"``), ``periods_validated`` is a count of model periods, and
        ``max_error`` is the largest absolute roll-forward break across those
        periods, in the balance node's own units. ``is_valid`` is ``False``
        when ``max_error`` exceeded the configured tolerance.

        Returns
        -------
        pd.DataFrame
            One row per validated account.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

class CorkscrewExtension:
    """
    Corkscrew extension for balance-sheet roll-forward validation.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CorkscrewConfig, CorkscrewExtension
    >>> CorkscrewExtension(CorkscrewConfig()).config().tolerance
    0.01

    """

    def __init__(self, config: CorkscrewConfig) -> None:
        """
        Create an extension driven by ``config``.

        Parameters
        ----------
        config : CorkscrewConfig
            Accounts, tolerance, and error policy used during reconciliation.

        Returns
        -------
        None

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...
    def config(self) -> CorkscrewConfig:
        """
        Return the configuration this extension runs with.

        Returns
        -------
        CorkscrewConfig
            The configuration supplied at construction.

        Notes
        -----
        This method does not raise.
        """
        ...
    def execute(self, model: FinancialModelSpec | str, results: StatementResult | str) -> CorkscrewReport:
        """
        Validate account roll-forwards against evaluated statement results.

        Parameters
        ----------
        model : FinancialModelSpec or str
            Model specification object or JSON used to resolve configured nodes.
        results : StatementResult or str
            Evaluated statement results object or JSON to reconcile.

        Returns
        -------
        CorkscrewReport
            Reconciliation status, diagnostics, and account-level roll-forward
            results for the supplied model evaluation.

        Raises
        ------
        ValueError
            If the corkscrew configuration, model, or statement results are invalid.

        """
        ...
