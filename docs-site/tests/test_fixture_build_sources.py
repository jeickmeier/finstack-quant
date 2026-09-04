"""Fixture introduction cards expose the exact canonical source inventory."""

from pathlib import Path

from check_curriculum import validate_fixture_builds
from common import curriculum_manifest, frontmatter


def test_fixture_build_sources_must_match_manifest_exactly() -> None:
    record = {"id": "1.1", "introduces": ["foundations"]}
    sources = {"foundations": ["_shared/__init__.py", "_shared/market.py"]}
    exact = '<FixtureBuild fixture="foundations" sources="_shared/__init__.py · _shared/market.py">\n'
    assert validate_fixture_builds(record, exact, sources) == []

    missing = '<FixtureBuild fixture="foundations" sources="_shared/market.py">\n'
    errors = validate_fixture_builds(record, missing, sources)
    assert errors == [
        "1.1: FixtureBuild sources for foundations differ from curriculum; "
        "expected=['_shared/__init__.py', '_shared/market.py'], actual=['_shared/market.py']"
    ]

    reordered = '<FixtureBuild fixture="foundations" sources="_shared/market.py · _shared/__init__.py">\n'
    assert any("differ from curriculum" in error for error in validate_fixture_builds(record, reordered, sources))


def test_only_visible_literal_fixture_builds_count() -> None:
    record = {"id": "2.3", "introduces": ["rates"]}
    sources = {"rates": ["_shared/rates.py"]}
    body = """{/* <FixtureBuild fixture=\"rates\" sources=\"_shared/rates.py\"> */}
```mdx
<FixtureBuild fixture="rates" sources="_shared/rates.py">
```
<FixtureBuild sources="_shared/rates.py" fixture="rates">
"""
    assert validate_fixture_builds(record, body, sources) == []

    malformed = '<FixtureBuild fixture="rates">\n'
    errors = validate_fixture_builds(record, malformed, sources)
    assert any("requires literal fixture and sources" in error for error in errors)
    assert any("needs exactly one visible FixtureBuild" in error for error in errors)


def test_unintroduced_fixture_build_is_rejected() -> None:
    body = '<FixtureBuild fixture="rates" sources="_shared/rates.py">\n'
    errors = validate_fixture_builds({"id": "1.2"}, body, {"rates": ["_shared/rates.py"]})
    assert errors == ["1.2: visible FixtureBuild for rates is not declared in introduces"]


def test_every_fixture_owner_lesson_lists_canonical_sources() -> None:
    site = Path(__file__).parents[1]
    manifest = curriculum_manifest(site)
    errors = []
    for record in manifest["lesson"]:
        if not record.get("introduces"):
            continue
        _, body = frontmatter(site / "content" / "learn" / record["path"])
        errors.extend(validate_fixture_builds(record, body, manifest["fixture_sources"]))

    assert errors == []
