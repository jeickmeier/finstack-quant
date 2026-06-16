"""Domain runner placeholder for analytics golden fixtures.

No analytics fixtures are committed yet. Executable analytics dispatch must be
wired (canonical return/price inputs mapped to the analytics API) before any
``analytics.*`` fixture can run.
"""

from __future__ import annotations

from tests.golden.schema import GoldenFixture


def run(fixture: GoldenFixture) -> dict[str, float]:
    """Reject analytics fixtures until executable dispatch is wired."""
    msg = (
        f"analytics runner is not wired yet for domain '{fixture.metadata.domain}'; "
        "add canonical return/price inputs and API mapping before enabling this golden"
    )
    raise ValueError(msg)
