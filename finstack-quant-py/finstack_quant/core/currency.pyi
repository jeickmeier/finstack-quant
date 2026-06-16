"""ISO-4217 currency bindings from ``finstack-quant-core``.

Provides the :class:`Currency` type for representing ISO-4217 currencies with
numeric codes and minor-unit precision. Module-level constants are provided for
every supported currency (e.g. ``USD``, ``EUR``, ``GBP``).

Example::

    >>> from finstack_quant.core.currency import Currency, USD
    >>> usd = Currency("USD")
    >>> usd.code
    'USD'
    >>> usd.numeric
    840
    >>> usd == USD
    True
"""

from __future__ import annotations

__all__ = [
    "Currency",
    "AED",
    "AFN",
    "ALL",
    "AMD",
    "ANG",
    "AOA",
    "ARS",
    "AUD",
    "AWG",
    "AZN",
    "BAM",
    "BBD",
    "BDT",
    "BGN",
    "BHD",
    "BIF",
    "BMD",
    "BND",
    "BOB",
    "BRL",
    "BSD",
    "BTN",
    "BWP",
    "BYN",
    "BZD",
    "CAD",
    "CDF",
    "CHF",
    "CLP",
    "CNY",
    "COP",
    "CRC",
    "CUP",
    "CVE",
    "CZK",
    "DJF",
    "DKK",
    "DOP",
    "DZD",
    "EGP",
    "ERN",
    "ETB",
    "EUR",
    "FJD",
    "FKP",
    "GBP",
    "GEL",
    "GHS",
    "GIP",
    "GMD",
    "GNF",
    "GTQ",
    "GYD",
    "HKD",
    "HNL",
    "HRK",
    "HTG",
    "HUF",
    "IDR",
    "ILS",
    "INR",
    "IQD",
    "IRR",
    "ISK",
    "JMD",
    "JOD",
    "JPY",
    "KES",
    "KGS",
    "KHR",
    "KMF",
    "KPW",
    "KRW",
    "KWD",
    "KYD",
    "KZT",
    "LAK",
    "LBP",
    "LKR",
    "LRD",
    "LSL",
    "LYD",
    "MAD",
    "MDL",
    "MGA",
    "MKD",
    "MMK",
    "MNT",
    "MOP",
    "MRU",
    "MUR",
    "MVR",
    "MWK",
    "MXN",
    "MYR",
    "MZN",
    "NAD",
    "NGN",
    "NIO",
    "NOK",
    "NPR",
    "NZD",
    "OMR",
    "PAB",
    "PEN",
    "PGK",
    "PHP",
    "PKR",
    "PLN",
    "PYG",
    "QAR",
    "RON",
    "RSD",
    "RUB",
    "RWF",
    "SAR",
    "SBD",
    "SCR",
    "SDG",
    "SEK",
    "SGD",
    "SHP",
    "SLE",
    "SOS",
    "SRD",
    "SSP",
    "STN",
    "SVC",
    "SYP",
    "SZL",
    "THB",
    "TJS",
    "TMT",
    "TND",
    "TOP",
    "TRY",
    "TTD",
    "TWD",
    "TZS",
    "UAH",
    "UGX",
    "USD",
    "UYU",
    "UZS",
    "VES",
    "VND",
    "VUV",
    "WST",
    "XAF",
    "XCD",
    "XOF",
    "XPF",
    "YER",
    "ZAR",
    "ZMW",
    "ZWL",
]

class Currency:
    """An ISO-4217 currency.

    Immutable, hashable value type representing a single ISO-4217 currency.
    Supports comparison with other ``Currency`` instances and with ISO
    alphabetic code strings.

    Parameters
    ----------
    code : str
        Three-letter ISO-4217 alphabetic code (case-insensitive).

    Raises
    ------
    ValueError
        If *code* is not a recognised ISO-4217 currency.

    Examples
    --------
    >>> from finstack_quant.core.currency import Currency
    >>> eur = Currency("EUR")
    >>> eur.code
    'EUR'
    >>> eur.numeric
    978
    >>> eur.decimals
    2
    >>> eur == "EUR"
    True
    """

    def __init__(self, code: str) -> None:
        """Parse an ISO-4217 alphabetic code (case-insensitive).

        Parameters
        ----------
        code : str
            Three-letter ISO-4217 alphabetic code.

        Raises
        ------
        ValueError
            If *code* is not a recognised currency.
        """
        ...

    @classmethod
    def from_numeric(cls, code: int) -> Currency:
        """Construct from an ISO-4217 numeric code.

        Parameters
        ----------
        code : int
            ISO-4217 numeric identifier (e.g. ``840`` for USD).

        Returns
        -------
        Currency
            The matching currency.

        Raises
        ------
        ValueError
            If *code* does not map to a known currency.
        """
        ...

    @property
    def code(self) -> str:
        """Three-letter ISO-4217 alphabetic code (uppercase).

        Returns
        -------
        str
        """
        ...

    @property
    def numeric(self) -> int:
        """ISO-4217 numeric identifier.

        Returns
        -------
        int
        """
        ...

    @property
    def decimals(self) -> int:
        """Typical number of decimal places (minor units) for this currency.

        Returns
        -------
        int
        """
        ...

    def to_json(self) -> str:
        """Serialize this currency to a JSON string.

        Returns
        -------
        str
            JSON representation.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> Currency:
        """Deserialize a currency from a JSON string.

        Parameters
        ----------
        json : str
            JSON payload.

        Returns
        -------
        Currency
            The deserialized currency.

        Raises
        ------
        ValueError
            If *json* is not valid or does not represent a known currency.
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    def __lt__(self, other: object) -> bool: ...
    def __le__(self, other: object) -> bool: ...
    def __gt__(self, other: object) -> bool: ...
    def __ge__(self, other: object) -> bool: ...

# Module-level currency constants for every supported ISO-4217 currency.
AED: Currency
AFN: Currency
ALL: Currency
AMD: Currency
ANG: Currency
AOA: Currency
ARS: Currency
AUD: Currency
AWG: Currency
AZN: Currency
BAM: Currency
BBD: Currency
BDT: Currency
BGN: Currency
BHD: Currency
BIF: Currency
BMD: Currency
BND: Currency
BOB: Currency
BRL: Currency
BSD: Currency
BTN: Currency
BWP: Currency
BYN: Currency
BZD: Currency
CAD: Currency
CDF: Currency
CHF: Currency
CLP: Currency
CNY: Currency
COP: Currency
CRC: Currency
CUP: Currency
CVE: Currency
CZK: Currency
DJF: Currency
DKK: Currency
DOP: Currency
DZD: Currency
EGP: Currency
ERN: Currency
ETB: Currency
EUR: Currency
FJD: Currency
FKP: Currency
GBP: Currency
GEL: Currency
GHS: Currency
GIP: Currency
GMD: Currency
GNF: Currency
GTQ: Currency
GYD: Currency
HKD: Currency
HNL: Currency
HRK: Currency
HTG: Currency
HUF: Currency
IDR: Currency
ILS: Currency
INR: Currency
IQD: Currency
IRR: Currency
ISK: Currency
JMD: Currency
JOD: Currency
JPY: Currency
KES: Currency
KGS: Currency
KHR: Currency
KMF: Currency
KPW: Currency
KRW: Currency
KWD: Currency
KYD: Currency
KZT: Currency
LAK: Currency
LBP: Currency
LKR: Currency
LRD: Currency
LSL: Currency
LYD: Currency
MAD: Currency
MDL: Currency
MGA: Currency
MKD: Currency
MMK: Currency
MNT: Currency
MOP: Currency
MRU: Currency
MUR: Currency
MVR: Currency
MWK: Currency
MXN: Currency
MYR: Currency
MZN: Currency
NAD: Currency
NGN: Currency
NIO: Currency
NOK: Currency
NPR: Currency
NZD: Currency
OMR: Currency
PAB: Currency
PEN: Currency
PGK: Currency
PHP: Currency
PKR: Currency
PLN: Currency
PYG: Currency
QAR: Currency
RON: Currency
RSD: Currency
RUB: Currency
RWF: Currency
SAR: Currency
SBD: Currency
SCR: Currency
SDG: Currency
SEK: Currency
SGD: Currency
SHP: Currency
SLE: Currency
SOS: Currency
SRD: Currency
SSP: Currency
STN: Currency
SVC: Currency
SYP: Currency
SZL: Currency
THB: Currency
TJS: Currency
TMT: Currency
TND: Currency
TOP: Currency
TRY: Currency
TTD: Currency
TWD: Currency
TZS: Currency
UAH: Currency
UGX: Currency
USD: Currency
UYU: Currency
UZS: Currency
VES: Currency
VND: Currency
VUV: Currency
WST: Currency
XAF: Currency
XCD: Currency
XOF: Currency
XPF: Currency
YER: Currency
ZAR: Currency
ZMW: Currency
ZWL: Currency
