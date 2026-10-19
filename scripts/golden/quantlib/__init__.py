"""Native QuantLib fixture generators.

Two families share this package:

- **pricing** — ``finstack_quant.golden/1`` fixtures under
  ``valuations/tests/golden/data/pricing/quantlib/``
  (``uv run python -m scripts.golden.quantlib.generate --family pricing --product all [--check]``)
- **attribution** — T0/T1 + expected attribution JSON under
  ``valuations/tests/data/quantlib_parity/``
  (``uv run python -m scripts.golden.quantlib.generate --family attribution --product all [--check]``)

Schemas and scenarios differ on purpose; do not merge the fixture sets.
"""
