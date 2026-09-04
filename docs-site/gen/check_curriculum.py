"""Validate lesson dependencies, source mappings and publication evidence."""

from __future__ import annotations

import argparse
import ast
from collections import Counter
import importlib
import json
from pathlib import Path
import re
import sys

from common import (
    BUILD,
    ID_PATTERN,
    NOTEBOOKS,
    REPO,
    SITE,
    cell_source,
    contained,
    curriculum_manifest,
    digest,
    fixture_digest,
    frontmatter,
    lab_entries_by_notebook,
    lesson_labs,
    lesson_notebook_names,
    lesson_notebooks,
    notebook_closure,
    notebook_dependencies,
    read_lab_report,
    reference_anchors,
    runtime_identity,
    write_json,
)
from materialize_notebook_blocks import materialize_lesson
from run_lesson_snippets import (
    canonical_execution_blocks,
    canonical_validation_manifest,
    extract_blocks,
    snippet_provenance,
    validate_output_references,
)

_TRACK_FUNCTION_FIXTURES = {
    "build_structured_market": {"waterfall"},
    "clo_deal": {"waterfall"},
    "credit_calibration_envelope": {"waterfall"},
    "structured_index_inputs": {"waterfall"},
    "clo_bbb_holding": {"credit", "waterfall"},
    "complex_bond": {"credit"},
    "convertible_limit": {"credit"},
    "credit_extension": {"credit"},
    "credit_index_inputs": {"credit", "waterfall"},
    "stochastic_revolver": {"credit"},
    "commodity_inputs": {"volatility"},
    "futures_inputs": {"volatility"},
    "ohlc_observations": {"volatility"},
    "variance_inputs": {"volatility"},
    "vol_extension": {"volatility"},
}
_TRACK_STAGE_FUNCTIONS = {"book_spec", "build_book", "build_market"}
_BOOK_STAGE_FIXTURES = {
    "foundations": "foundations",
    "rates": "rates",
    "options": "options",
    "base": "base",
    "common": "common",
}
_BOOK_STAGE_FUNCTIONS = {
    "book_spec",
    "build_book",
    "build_market",
    "calibration_envelope",
    "expected_metrics",
    "instruments",
}
_INSTRUMENT_FIXTURE_REQUIREMENTS = {
    "INSTRUMENT_FACTORIES": {
        "foundations",
        "rates",
        "options",
        "base",
        "common",
        "waterfall",
        "credit",
        "volatility",
    },
    "INSTRUMENT_SCHEMA": {"foundations"},
    "abs_deal": {"waterfall"},
    "acme_bond": {"foundations"},
    "cds": {"base"},
    "cds_index": {"waterfall", "credit"},
    "cds_option": {"waterfall", "credit"},
    "cds_tranche": {"waterfall", "credit"},
    "clo_deal": {"waterfall"},
    "convertible_bond": {"credit"},
    "fixed_bond": {"foundations"},
    "floating_bond": {"foundations"},
    "fx_swap": {"options"},
    "instrument_description": {"foundations"},
    "instrument_envelope": {"foundations"},
    "instrument_envelope_json": {"foundations"},
    "ir_future": {"volatility"},
    "irs": {"rates"},
    "revolver": {"credit"},
    "spot_equity": {"options"},
    "swaption": {"rates"},
    "term_loan": {"common"},
    "variance_swap": {"volatility"},
}
_INLINE_ONLY_INSTRUMENT_NAMES = {"AS_OF", "AS_OF_STR"}
_SHARED_INSTRUMENT_EXPORTS = {"acme_bond", "instrument_envelope", "instrument_envelope_json"}


def _dependency_closure(graph: dict[str, list[str]], requirements: list[str]) -> set[str]:
    """Return all reachable prerequisites, including unknown IDs for diagnostics."""
    pending = list(requirements)
    reached = set()
    while pending:
        identifier = pending.pop()
        if identifier not in reached:
            reached.add(identifier)
            pending.extend(graph.get(identifier, []))
    return reached


def _validate_capstone(capstone: dict, graph: dict[str, list[str]]) -> list[str]:
    """Require the common baseline and the full chosen track without the other track."""
    errors = []
    if capstone.get("requires") != ["4.5"] or set(capstone.get("variants", {})) != {"credit", "volatility"}:
        errors.append("Capstone requires lesson 4.5 plus its chosen track variant")
    variant_labs = capstone.get("variant_labs", {})
    if capstone.get("labs") or set(variant_labs) != set(capstone.get("variants", {})):
        errors.append("Capstone labs must be mapped explicitly to the credit and volatility variants")
    mapped_variant_labs = [lab for labs in variant_labs.values() for lab in labs]
    if any(not labs for labs in variant_labs.values()) or len(mapped_variant_labs) != len(set(mapped_variant_labs)):
        errors.append("Capstone variant labs must be nonempty and exclusive to one variant")
    for variant, own, other in [("credit", "C", "V"), ("volatility", "V", "C")]:
        requirements = capstone.get("variants", {}).get(variant, [])
        predecessors = _dependency_closure(graph, [*capstone.get("requires", []), *requirements])
        required = {identifier for identifier in graph if identifier.startswith(own)}
        missing = required - predecessors
        if missing:
            errors.append(f"Capstone {variant}: missing track prerequisites {sorted(missing)}")
        if any(identifier.startswith(other) for identifier in predecessors):
            errors.append(f"Capstone {variant}: must not require the other specialist track")
    return errors


def _lesson_graph(records: list[dict], *, include_variants: bool = True) -> dict[str, list[str]]:
    """Build prerequisite adjacency list, optionally including variant requirements."""
    if not include_variants:
        return {record["id"]: list(record.get("requires", [])) for record in records}
    return {
        record["id"]: [
            *record.get("requires", []),
            *(item for requirements in record.get("variants", {}).values() for item in requirements),
        ]
        for record in records
    }


def validate_dependencies(records: list[dict]) -> list[str]:
    """Check dependency closure, independent tracks and complete capstone variants."""
    errors = []
    ids = [record["id"] for record in records]
    if len(ids) != len(set(ids)):
        errors.append("Duplicate lesson IDs")
    graph = _lesson_graph(records)
    active, done = set(), set()

    def visit(identifier: str) -> None:
        if identifier in active:
            errors.append(f"Dependency cycle at {identifier}")
            return
        if identifier in done:
            return
        active.add(identifier)
        for requirement in graph[identifier]:
            if requirement not in graph:
                errors.append(f"{identifier}: unknown prerequisite {requirement}")
            else:
                visit(requirement)
        active.remove(identifier)
        done.add(identifier)

    for identifier in graph:
        visit(identifier)

    for record in records:
        identifier = record["id"]
        predecessors = _dependency_closure(graph, graph[identifier])
        if identifier.startswith("C") and any(item.startswith("V") for item in predecessors):
            errors.append(f"{record['id']}: credit lessons must not require the volatility track")
        if identifier.startswith("V") and any(item.startswith("C") for item in predecessors):
            errors.append(f"{record['id']}: volatility lessons must not require the credit track")
        if identifier[:1] in {"1", "2", "3", "4"} and any(item.startswith(("C", "V")) for item in predecessors):
            errors.append(f"{identifier}: common lessons must not require a specialist track")
    capstone = next((record for record in records if record["id"] == "capstone"), None)
    if capstone:
        errors.extend(_validate_capstone(capstone, graph))
    return errors


def validate_fixture_progression(records: list[dict], fixture_ids: set[str]) -> list[str]:
    """Require every shared fixture to be introduced once before it is consumed."""
    errors = []
    graph = _lesson_graph(records)
    introduced_by: dict[str, str] = {}
    for record in records:
        for fixture in record.get("introduces", []):
            if fixture not in fixture_ids:
                errors.append(f"{record['id']}: introduces unknown fixture {fixture}")
            elif fixture in introduced_by:
                errors.append(f"{fixture}: fixture is introduced by both {introduced_by[fixture]} and {record['id']}")
            else:
                introduced_by[fixture] = record["id"]
            if fixture not in record.get("fixtures", []):
                errors.append(f"{record['id']}: introduced fixture {fixture} is not used by that lesson")
    errors.extend(
        f"{fixture}: fixture has no introduction lesson" for fixture in sorted(fixture_ids - set(introduced_by))
    )
    for record in records:
        available = {record["id"], *_dependency_closure(graph, graph.get(record["id"], []))}
        for fixture in record.get("fixtures", []):
            owner = introduced_by.get(fixture)
            if owner is not None and owner not in available:
                errors.append(f"{record['id']}: fixture {fixture} is consumed before its introduction in {owner}")
    return errors


def validate_fixture_sources(manifest: dict, fixture_ids: set[str]) -> list[str]:
    """Require every fixture family to name existing inspectable source files."""
    errors = []
    sources = manifest.get("fixture_sources", {})
    if set(sources) != fixture_ids:
        missing = sorted(fixture_ids - set(sources))
        unknown = sorted(set(sources) - fixture_ids)
        errors.append(f"Fixture source map differs from fixtures; missing={missing}, unknown={unknown}")
    for fixture, paths in sources.items():
        if not isinstance(paths, list) or not paths:
            errors.append(f"{fixture}: fixture source list must be nonempty")
            continue
        for relative in paths:
            try:
                source = contained(NOTEBOOKS, relative)
                if source.suffix != ".py" or not source.is_file():
                    errors.append(f"{fixture}: missing Python fixture source {relative}")
            except (TypeError, ValueError):
                errors.append(f"{fixture}: invalid fixture source {relative!r}")
    expected_sources = {
        str(path.relative_to(NOTEBOOKS))
        for path in (NOTEBOOKS / "_shared").rglob("*.py")
        if "__pycache__" not in path.parts
    }
    declared_sources = {
        relative
        for paths in sources.values()
        if isinstance(paths, list)
        for relative in paths
        if isinstance(relative, str)
    }
    if declared_sources != expected_sources:
        errors.append(
            "Shared fixture source ownership differs from _shared; "
            f"missing={sorted(expected_sources - declared_sources)}, "
            f"unknown={sorted(declared_sources - expected_sources)}"
        )
    return errors


def _visible_mdx(text: str) -> str:
    """Mask fenced code and comments while preserving source offsets."""

    def mask(value: str) -> str:
        return "".join(character if character in "\r\n" else " " for character in value)

    visible = []
    fence = None
    for line in text.splitlines(keepends=True):
        match = re.match(r"^ {0,3}(`{3,}|~{3,})(.*)$", line.rstrip("\r\n"))
        if fence is None:
            if match:
                fence = match[1]
                visible.append(mask(line))
            else:
                visible.append(line)
        else:
            visible.append(mask(line))
            if match and match[1][0] == fence[0] and len(match[1]) >= len(fence) and not match[2].strip():
                fence = None
    return re.sub(r"\{/\*.*?\*/\}|<!--.*?-->", lambda match: mask(match[0]), "".join(visible), flags=re.S)


def validate_fixture_builds(record: dict, body: str, fixture_sources: dict) -> list[str]:
    """Require visible fixture introductions to list their canonical sources exactly."""
    identifier = record["id"]
    visible = _visible_mdx(body)
    opening = re.compile(r"<FixtureBuild\b([^>]*)>", re.S)
    attributes = re.compile(r"\b(fixture|sources)\s*=\s*([\"'])(.*?)\2", re.S)
    matches = list(opening.finditer(visible))
    errors = []
    if len(matches) != len(re.findall(r"<FixtureBuild\b", visible)):
        errors.append(f"{identifier}: malformed FixtureBuild component")

    builds = []
    for match in matches:
        fields = attributes.findall(match[1])
        counts = Counter(name for name, _, _ in fields)
        if attributes.sub("", match[1]).strip() or counts != {"fixture": 1, "sources": 1}:
            errors.append(f"{identifier}: FixtureBuild requires literal fixture and sources attributes")
            continue
        values = {name: value for name, _, value in fields}
        builds.append((values["fixture"], values["sources"], match.start()))

    introduced = record.get("introduces", [])
    build_counts = Counter(fixture for fixture, _, _ in builds)
    first_block = body.find("notebook-block")
    for fixture in introduced:
        if build_counts[fixture] != 1:
            errors.append(f"{identifier}: fixture {fixture} needs exactly one visible FixtureBuild")
            continue
        _, displayed, position = next(build for build in builds if build[0] == fixture)
        expected = fixture_sources.get(fixture)
        actual = [source.strip() for source in displayed.split("·")]
        if not isinstance(expected, list) or actual != expected:
            errors.append(
                f"{identifier}: FixtureBuild sources for {fixture} differ from curriculum; "
                f"expected={expected!r}, actual={actual!r}"
            )
        if position > first_block >= 0:
            errors.append(f"{identifier}: fixture {fixture} must be introduced before executable code")

    errors.extend(
        f"{identifier}: visible FixtureBuild for {fixture} is not declared in introduces"
        for fixture in sorted(build_counts.keys() - set(introduced))
    )
    return errors


_notebook_source = cell_source


def _shared_imports(notebook: dict, module: str) -> tuple[set[str], dict[str, str]]:
    """Resolve module and direct aliases for one shared notebook module."""
    modules: set[str] = set()
    direct: dict[str, str] = {}
    for cell in notebook.get("cells", []):
        if cell.get("cell_type") != "code" or not _notebook_source(cell).strip():
            continue
        tree = ast.parse(_notebook_source(cell))
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                for alias in node.names:
                    if alias.name == f"_shared.{module}":
                        modules.add(alias.asname or alias.name)
            elif isinstance(node, ast.ImportFrom) and node.module == "_shared":
                for alias in node.names:
                    if alias.name == module:
                        modules.add(alias.asname or alias.name)
            elif isinstance(node, ast.ImportFrom) and node.module == f"_shared.{module}":
                direct.update({alias.asname or alias.name: alias.name for alias in node.names})
    return modules, direct


def _shared_call(node: ast.Call, modules: set[str], direct: dict[str, str]) -> str | None:
    """Return the canonical shared helper name called by an AST node."""
    if isinstance(node.func, ast.Attribute) and isinstance(node.func.value, ast.Name) and node.func.value.id in modules:
        return node.func.attr
    if isinstance(node.func, ast.Name):
        return direct.get(node.func.id)
    return None


def _shared_export_sources(notebook_root: Path) -> dict[str, str]:
    """Map names re-exported by ``_shared`` to their defining source module."""
    init_path = notebook_root / "_shared" / "__init__.py"
    tree = ast.parse(init_path.read_text(), filename=str(init_path))
    exports = {}
    for node in tree.body:
        if not isinstance(node, ast.ImportFrom) or node.level != 1 or not node.module:
            continue
        source = f"_shared/{node.module.replace('.', '/')}.py"
        for alias in node.names:
            exports[alias.asname or alias.name] = source
    return exports


def _shared_source_imports(notebook: dict, notebook_root: Path) -> set[str]:
    """Return concrete ``_shared`` Python sources imported by one notebook."""
    sources = set()
    init_source = "_shared/__init__.py"
    exports = _shared_export_sources(notebook_root)
    for cell in notebook.get("cells", []):
        source = _notebook_source(cell)
        if cell.get("cell_type") != "code" or not source.strip():
            continue
        tree = ast.parse(source)
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                for alias in node.names:
                    if alias.name == "_shared":
                        sources.add(init_source)
                    elif alias.name.startswith("_shared."):
                        sources.update({init_source, f"{alias.name.replace('.', '/')}.py"})
            elif isinstance(node, ast.ImportFrom) and node.module == "_shared":
                sources.add(init_source)
                for alias in node.names:
                    module_source = f"_shared/{alias.name}.py"
                    if (notebook_root / module_source).is_file():
                        sources.add(module_source)
                    elif alias.name in exports:
                        sources.add(exports[alias.name])
            elif isinstance(node, ast.ImportFrom) and (node.module or "").startswith("_shared."):
                sources.update({init_source, f"{node.module.replace('.', '/')}.py"})
    return sources


def _instrument_fixture_bindings(tree: ast.AST) -> tuple[set[str], set[str], set[str]]:
    """Return fixture-module aliases, package aliases and directly imported names."""
    module_aliases: set[str] = set()
    package_aliases: set[str] = set()
    direct: set[str] = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                if alias.name == "_shared.instrument_fixtures":
                    (module_aliases if alias.asname else package_aliases).add(alias.asname or "_shared")
                elif alias.name == "_shared":
                    package_aliases.add(alias.asname or "_shared")
        elif isinstance(node, ast.ImportFrom) and node.module == "_shared.instrument_fixtures":
            direct.update(alias.name for alias in node.names)
        elif isinstance(node, ast.ImportFrom) and node.module == "_shared":
            for alias in node.names:
                if alias.name == "instrument_fixtures":
                    module_aliases.add(alias.asname or alias.name)
                elif alias.name == "*":
                    direct.update(_SHARED_INSTRUMENT_EXPORTS)
                elif alias.name in _INSTRUMENT_FIXTURE_REQUIREMENTS:
                    direct.add(alias.name)
    return module_aliases, package_aliases, direct


def _instrument_fixture_attribute(
    node: ast.AST, module_aliases: set[str], package_aliases: set[str]
) -> tuple[str | None, bool]:
    """Resolve one staged fixture attribute and whether its module was explicit."""
    if not isinstance(node, ast.Attribute):
        return None, False
    if isinstance(node.value, ast.Name):
        if node.value.id in module_aliases:
            return node.attr, True
        if node.value.id in package_aliases:
            return node.attr, False
    if (
        isinstance(node.value, ast.Attribute)
        and isinstance(node.value.value, ast.Name)
        and node.value.value.id in package_aliases
        and node.value.attr == "instrument_fixtures"
    ):
        return node.attr, True
    return None, False


def _instrument_fixture_imports(notebook: dict) -> tuple[set[str], set[str]]:
    """Return classified and unknown imports from the staged instrument fixture module."""
    classified: set[str] = set()
    unknown: set[str] = set()
    module_aliases: set[str] = set()
    package_aliases: set[str] = set()
    trees = [
        ast.parse(_notebook_source(cell))
        for cell in notebook.get("cells", [])
        if cell.get("cell_type") == "code" and _notebook_source(cell).strip()
    ]
    for tree in trees:
        modules, packages, direct = _instrument_fixture_bindings(tree)
        module_aliases.update(modules)
        package_aliases.update(packages)
        if "*" in direct:
            classified.update(_INSTRUMENT_FIXTURE_REQUIREMENTS)
            unknown.update(_INLINE_ONLY_INSTRUMENT_NAMES)
        classified.update(name for name in direct if name in _INSTRUMENT_FIXTURE_REQUIREMENTS)
        unknown.update(name for name in direct if name != "*" and name not in _INSTRUMENT_FIXTURE_REQUIREMENTS)
    for tree in trees:
        for node in ast.walk(tree):
            name, fixture_module_access = _instrument_fixture_attribute(node, module_aliases, package_aliases)
            if name in _INSTRUMENT_FIXTURE_REQUIREMENTS:
                classified.add(name)
            elif fixture_module_access and name:
                unknown.add(name)
    return classified, unknown


def _record_notebook_scopes(
    record: dict,
    graph: dict[str, list[str]],
    by_id: dict[str, dict],
    *,
    include_examples: bool,
) -> list[tuple[str, set[str], set[str], str | None]]:
    """Map each common or variant notebook to its available lessons and fixtures."""

    def scope(
        names: list[str], requirements: list[str], variant: str | None
    ) -> list[tuple[str, set[str], set[str], str | None]]:
        available_lessons = {record["id"], *_dependency_closure(graph, requirements)}
        available_fixtures = {
            fixture for lesson in available_lessons for fixture in by_id.get(lesson, {}).get("fixtures", [])
        }
        return [(name, available_lessons, available_fixtures, variant) for name in names]

    common = [*record.get("labs", []), *(record.get("examples", []) if include_examples else [])]
    scopes = scope(common, record.get("requires", []), None)
    for variant, names in record.get("variant_labs", {}).items():
        requirements = [*record.get("requires", []), *record.get("variants", {}).get(variant, [])]
        scopes.extend(scope(names, requirements, variant))
    return scopes


def validate_instrument_fixture_usage(records: list[dict], notebook_root: Path = NOTEBOOKS) -> list[str]:
    """Reject staged instrument families exposed before their fixture introduction."""
    errors = []
    by_id = {record["id"]: record for record in records}
    graph = {record["id"]: record.get("requires", []) for record in records}
    for record in records:
        lesson_id = record["id"]
        for mapped, _available_lessons, available_fixtures, _variant in _record_notebook_scopes(
            record, graph, by_id, include_examples=True
        ):
            for relative in sorted(notebook_closure([mapped], notebook_root)):
                notebook = json.loads(contained(notebook_root, relative).read_text())
                imported, unknown = _instrument_fixture_imports(notebook)
                for name in sorted(unknown):
                    if name in _INLINE_ONLY_INSTRUMENT_NAMES:
                        errors.append(f"{lesson_id}: {relative} must inline {name} instead of importing it")
                    else:
                        errors.append(f"{lesson_id}: {relative} imports unclassified instrument fixture {name}")
                for name in sorted(imported):
                    missing = _INSTRUMENT_FIXTURE_REQUIREMENTS[name] - available_fixtures
                    if missing:
                        errors.append(
                            f"{lesson_id}: {relative} imports instrument fixture {name} before fixtures "
                            f"{sorted(missing)} are available"
                        )
    return errors


def validate_shared_source_usage(records: list[dict], manifest: dict, notebook_root: Path = NOTEBOOKS) -> list[str]:
    """Require every imported shared source to be introduced in the lesson closure."""
    errors = []
    by_id = {record["id"]: record for record in records}
    graph = _lesson_graph(records)
    owners: dict[str, set[str]] = {}
    for fixture, paths in manifest.get("fixture_sources", {}).items():
        for source in paths:
            owners.setdefault(source, set()).add(fixture)
    for record in records:
        lesson_id = record["id"]
        available_lessons = {lesson_id, *_dependency_closure(graph, graph.get(lesson_id, []))}
        available_fixtures = {
            fixture for available in available_lessons for fixture in by_id.get(available, {}).get("fixtures", [])
        }
        for relative in sorted(notebook_closure(lesson_notebook_names(record), notebook_root)):
            notebook = json.loads(contained(notebook_root, relative).read_text())
            for source in sorted(_shared_source_imports(notebook, notebook_root)):
                source_owners = owners.get(source, set())
                if not source_owners:
                    errors.append(f"{lesson_id}: {relative} imports unowned shared source {source}")
                elif not source_owners & available_fixtures:
                    errors.append(
                        f"{lesson_id}: {relative} imports {source} before fixture sources "
                        f"{sorted(source_owners)} are available"
                    )
    return errors


def _track_call_fixtures(name: str, node: ast.Call, variant: str | None = None) -> set[str] | None:
    """Map one track helper call to the fixture families it consumes."""
    if name in _TRACK_FUNCTION_FIXTURES:
        required = set(_TRACK_FUNCTION_FIXTURES[name])
        if name == "build_structured_market":
            required.update(_track_core_fixtures(node, {"base"}))
        return required
    if name not in _TRACK_STAGE_FUNCTIONS:
        return None
    stage_node: ast.expr | None = node.args[0] if node.args else None
    if stage_node is None:
        stage_node = next((keyword.value for keyword in node.keywords if keyword.arg == "stage"), None)
    stage = stage_node.value if isinstance(stage_node, ast.Constant) and isinstance(stage_node.value, str) else None
    if stage is None and variant in {"credit", "volatility"}:
        stage = "credit" if variant == "credit" else "vol"
    if stage == "credit":
        required = {"credit", "waterfall"} if name == "build_market" else {"credit"}
        return required | _track_core_fixtures(node, {"common"})
    if stage == "vol":
        return {"volatility"} | _track_core_fixtures(node, {"base"})
    # Capstone helpers dispatch on a visible track variable and must declare
    # both possible extension families. A dynamic credit market also depends
    # on the structured market introduced with the waterfall fixture. Its
    # stage-dependent core default can select either base or common.
    required = {"credit", "volatility", "waterfall"} if name == "build_market" else {"credit", "volatility"}
    return required | _track_core_fixtures(node, {"base", "common"})


def _track_core_fixtures(node: ast.Call, defaults: set[str]) -> set[str]:
    """Resolve a literal track ``core_stage`` or conservatively require both stages."""
    value = next((keyword.value for keyword in node.keywords if keyword.arg == "core_stage"), None)
    if value is None or (isinstance(value, ast.Constant) and value.value is None):
        return set(defaults)
    if isinstance(value, ast.Constant) and value.value in {"base", "common"}:
        return {str(value.value)}
    return {"base", "common"}


def _book_call_stage(node: ast.Call) -> str | None:
    """Resolve a literal analyst-book stage, including its documented default."""
    value: ast.expr | None = node.args[0] if node.args else None
    if value is None:
        value = next((keyword.value for keyword in node.keywords if keyword.arg == "stage"), None)
    if value is None:
        return "base"
    if isinstance(value, ast.Constant) and isinstance(value.value, str):
        return value.value
    return None


def validate_track_fixture_calls(records: list[dict], notebook_root: Path = NOTEBOOKS) -> list[str]:
    """Require tagged track-helper calls to declare their owning fixtures."""
    errors = []
    by_id = {record["id"]: record for record in records}
    base_graph = _lesson_graph(records, include_variants=False)
    variant_scopes = {
        (record["id"], relative): (available_fixtures, variant)
        for record in records
        for relative, _available_lessons, available_fixtures, variant in _record_notebook_scopes(
            record, base_graph, by_id, include_examples=True
        )
        if variant is not None
    }
    notebooks = {name for record in records for name in lesson_notebook_names(record)}
    for relative in sorted(notebooks):
        notebook = json.loads(contained(notebook_root, relative).read_text())
        modules, direct = _shared_imports(notebook, "analyst_tracks")
        if not modules and not direct:
            continue
        for cell_number, cell in enumerate(notebook.get("cells", []), start=1):
            tag = cell.get("metadata", {}).get("analyst_program", {})
            lesson_id = tag.get("lesson")
            if cell.get("cell_type") != "code" or lesson_id not in by_id:
                continue
            declared, variant = variant_scopes.get(
                (lesson_id, relative),
                (set(by_id[lesson_id].get("fixtures", [])), None),
            )
            tree = ast.parse(_notebook_source(cell), filename=f"{relative}:cell-{cell_number}")
            for node in (item for item in ast.walk(tree) if isinstance(item, ast.Call)):
                name = _shared_call(node, modules, direct)
                if name is None:
                    continue
                required = _track_call_fixtures(name, node, variant)
                if required is None:
                    errors.append(f"{lesson_id}: unclassified analyst_tracks helper {name} in {relative}")
                elif missing := required - declared:
                    errors.append(
                        f"{lesson_id}: analyst_tracks.{name} in {relative} requires fixtures {sorted(missing)}"
                    )
    return errors


def validate_book_stage_calls(records: list[dict], notebook_root: Path = NOTEBOOKS) -> list[str]:
    """Reject cumulative book or market stages used before their owner lesson."""
    errors = []
    by_id = {record["id"]: record for record in records}
    graph = _lesson_graph(records)
    owners = {fixture: record["id"] for record in records for fixture in record.get("introduces", [])}
    notebooks = {name for record in records for name in lesson_notebook_names(record)}
    for relative in sorted(notebooks):
        notebook = json.loads(contained(notebook_root, relative).read_text())
        modules, direct = _shared_imports(notebook, "analyst_book")
        if not modules and not direct:
            continue
        for cell_number, cell in enumerate(notebook.get("cells", []), start=1):
            tag = cell.get("metadata", {}).get("analyst_program", {})
            lesson_id = tag.get("lesson")
            if cell.get("cell_type") != "code" or lesson_id not in by_id:
                continue
            available = {lesson_id, *_dependency_closure(graph, graph.get(lesson_id, []))}
            tree = ast.parse(_notebook_source(cell), filename=f"{relative}:cell-{cell_number}")
            for node in (item for item in ast.walk(tree) if isinstance(item, ast.Call)):
                name = _shared_call(node, modules, direct)
                if name not in _BOOK_STAGE_FUNCTIONS:
                    continue
                stage = _book_call_stage(node)
                if stage is None:
                    errors.append(f"{lesson_id}: analyst_book.{name} in {relative} requires a literal stage")
                    continue
                fixture = _BOOK_STAGE_FIXTURES.get(stage)
                if fixture is None:
                    errors.append(f"{lesson_id}: analyst_book.{name} in {relative} uses unknown stage {stage!r}")
                    continue
                owner = owners.get(fixture)
                if owner is not None and owner not in available:
                    errors.append(
                        f"{lesson_id}: analyst_book.{name}({stage!r}) in {relative} precedes {fixture} introduction in {owner}"
                    )
    return errors


def _lab_notebook_stage_errors(
    lesson_id: str,
    lab_relative: str,
    exposed_relative: str,
    available_lessons: set[str],
    available_fixtures: set[str],
    notebook_root: Path,
    variant: str | None,
) -> list[str]:
    """Validate one lab notebook or recursively exposed dependency."""
    errors = []
    notebook = json.loads(contained(notebook_root, exposed_relative).read_text())
    track_modules, track_direct = _shared_imports(notebook, "analyst_tracks")
    book_modules, book_direct = _shared_imports(notebook, "analyst_book")
    location = (
        f"lab {lab_relative}"
        if exposed_relative == lab_relative
        else f"lab {lab_relative} dependency {exposed_relative}"
    )
    for cell_number, cell in enumerate(notebook.get("cells", []), start=1):
        if cell.get("cell_type") != "code":
            continue
        tagged_lesson = cell.get("metadata", {}).get("analyst_program", {}).get("lesson")
        if tagged_lesson and tagged_lesson not in available_lessons:
            errors.append(f"{lesson_id}: {location} exposes {tagged_lesson} code before that lesson is available")
        source = _notebook_source(cell)
        if not source.strip():
            continue
        tree = ast.parse(source, filename=f"{exposed_relative}:cell-{cell_number}")
        for node in (item for item in ast.walk(tree) if isinstance(item, ast.Call)):
            track_name = _shared_call(node, track_modules, track_direct)
            if track_name is not None:
                required = _track_call_fixtures(track_name, node, variant)
                if required is None:
                    errors.append(f"{lesson_id}: {location} uses unclassified analyst_tracks helper {track_name}")
                elif missing := required - available_fixtures:
                    errors.append(
                        f"{lesson_id}: {location} uses analyst_tracks.{track_name} before fixtures "
                        f"{sorted(missing)} are available"
                    )
            book_name = _shared_call(node, book_modules, book_direct)
            if book_name not in _BOOK_STAGE_FUNCTIONS:
                continue
            stage = _book_call_stage(node)
            if stage is None:
                errors.append(f"{lesson_id}: {location} uses analyst_book.{book_name} with a nonliteral stage")
                continue
            fixture = _BOOK_STAGE_FIXTURES.get(stage)
            if fixture is None:
                errors.append(f"{lesson_id}: {location} uses analyst_book.{book_name} with unknown stage {stage!r}")
            elif fixture not in available_fixtures:
                errors.append(
                    f"{lesson_id}: {location} uses analyst_book.{book_name}({stage!r}) "
                    f"before fixture {fixture} is available"
                )
    return errors


def validate_lab_stage_exposure(records: list[dict], notebook_root: Path = NOTEBOOKS) -> list[str]:
    """Reject full labs that expose code from fixtures or lessons not yet available."""
    errors = []
    by_id = {record["id"]: record for record in records}
    graph = _lesson_graph(records, include_variants=False)
    for record in records:
        lesson_id = record["id"]
        for relative, available_lessons, available_fixtures, variant in _record_notebook_scopes(
            record, graph, by_id, include_examples=False
        ):
            for exposed_relative in sorted(notebook_closure([relative], notebook_root)):
                errors.extend(
                    _lab_notebook_stage_errors(
                        lesson_id,
                        relative,
                        exposed_relative,
                        available_lessons,
                        available_fixtures,
                        notebook_root,
                        variant,
                    )
                )
    return errors


def resolve_api(name: str) -> None:
    """Resolve a declared Python symbol against the installed extension."""
    parts = name.split(".")
    for count in range(len(parts), 0, -1):
        try:
            value = importlib.import_module(".".join(parts[:count]))
        except ModuleNotFoundError as error:
            if error.name != ".".join(parts[:count]) and not ".".join(parts[:count]).startswith(f"{error.name}."):
                raise
            continue
        for part in parts[count:]:
            value = getattr(value, part)
        return
    raise ValueError(f"Cannot resolve API symbol {name}")


def check_metadata(
    record: dict, metadata: dict, references: set[str], fixture_ids: set[str], check_api: bool
) -> list[str]:
    """Validate presentation fields, supported symbols and fixture identities."""
    errors = []
    identifier = record["id"]
    required = ("id", "title", "description", "desk_question", "references", "api", "status")
    errors.extend(f"{identifier}: missing frontmatter {field}" for field in required if field not in metadata)
    if metadata.get("id") != identifier:
        errors.append(f"{identifier}: frontmatter ID differs from manifest")
    if metadata.get("status") not in {"draft", "published"}:
        errors.append(f"{identifier}: invalid publication status")
    errors.extend(
        f"{identifier}: unknown fixture {fixture}"
        for fixture in record.get("fixtures", [])
        if fixture not in fixture_ids
    )
    errors.extend(
        f"{identifier}: missing REFERENCES anchor {reference}"
        for reference in metadata.get("references", [])
        if reference.rsplit("#", 1)[-1] not in references
    )
    if check_api:
        for name in metadata.get("api", []):
            try:
                resolve_api(name)
            except (ImportError, AttributeError, ValueError) as error:
                errors.append(f"{identifier}: {name}: {error}")
    return errors


def published_snippet_assets(identifier: str, block_ids: set[str], site: Path) -> tuple[set[str], list[str]]:
    """Inventory published files and reject lesson roots that escape or outlive evidence."""
    lesson_directory = site / "public" / "snippet-assets" / identifier
    if lesson_directory.is_symlink():
        raise ValueError(f"{identifier}: snippet lesson asset directory must not be a symlink")
    if not lesson_directory.is_dir():
        return set(), []
    actual_assets = {
        f"/snippet-assets/{identifier}/{path.relative_to(lesson_directory).as_posix()}"
        for path in lesson_directory.rglob("*")
        if path.is_file() or path.is_symlink()
    }
    unexpected_directories = sorted(
        path.name
        for path in lesson_directory.iterdir()
        if path.name not in block_ids and (path.is_dir() or path.is_symlink())
    )
    return actual_assets, unexpected_directories


def validate_snippet_assets(identifier: str, captured_blocks: list[dict], site: Path) -> list[str]:
    """Require captured assets to remain in their block directory and match their digest."""
    errors = []
    recorded_hashes: dict[str, str] = {}
    block_ids = set()
    for block in captured_blocks:
        block_id = block.get("id")
        label = f"{identifier}:{block_id}"
        if not isinstance(block_id, str) or not re.fullmatch(r"[a-z0-9][a-z0-9_-]*", block_id):
            errors.append(f"{label}: invalid snippet block ID")
            continue
        block_ids.add(block_id)
        assets = block.get("assets", [])
        hashes = block.get("asset_sha256", {})
        if not isinstance(assets, list) or not all(isinstance(asset, str) for asset in assets):
            errors.append(f"{label}: snippet asset paths must be a list of strings")
            continue
        if not isinstance(hashes, dict) or not all(
            isinstance(asset, str) and isinstance(fingerprint, str) for asset, fingerprint in hashes.items()
        ):
            errors.append(f"{label}: asset_sha256 must map asset paths to hashes")
            continue
        if len(assets) != len(set(assets)) or set(assets) != set(hashes):
            errors.append(f"{label}: snippet asset paths and asset_sha256 keys differ")
        recorded_hashes.update(hashes)

        prefix = f"/snippet-assets/{identifier}/{block_id}/"
        directory = contained(site / "public" / "snippet-assets" / identifier, block_id)
        for asset in sorted(set(assets) | set(hashes)):
            relative = asset.removeprefix(prefix) if asset.startswith(prefix) else ""
            if not relative or "\\" in relative or any(part in {"", ".", ".."} for part in relative.split("/")):
                errors.append(f"{label}: snippet asset path must remain inside its block directory: {asset}")
                continue
            try:
                path = contained(directory, relative)
            except (TypeError, ValueError):
                errors.append(f"{label}: snippet asset path must remain inside its block directory: {asset}")
                continue
            if not path.is_file():
                errors.append(f"{label}: missing snippet asset: {asset}")
                continue
            expected = hashes.get(asset)
            if expected is not None and digest(path) != expected:
                errors.append(f"{label}: snippet asset hash mismatch: {asset}")

    try:
        actual_assets, unexpected_directories = published_snippet_assets(identifier, block_ids, site)
    except ValueError as error:
        errors.append(str(error))
        actual_assets, unexpected_directories = set(), []
    missing = sorted(set(recorded_hashes) - actual_assets)
    unexpected = sorted(actual_assets - set(recorded_hashes))
    if missing or unexpected:
        errors.append(
            f"{identifier}: snippet asset evidence differs from published lesson files; "
            f"missing={missing}, unexpected={unexpected}"
        )
    if unexpected_directories:
        errors.append(f"{identifier}: obsolete snippet block asset directories: {unexpected_directories}")
    return errors


def check_evidence(
    record: dict,
    path: Path,
    blocks: list,
    site: Path,
    lab_evidence: dict,
    fixture_hash: str,
    runtime: dict | None = None,
) -> list[str]:
    """Reject missing or stale execution output for snippets and companion labs."""
    identifier = record["id"]
    current_runtime = runtime_identity() if runtime is None else runtime
    errors = []
    for notebook in lesson_labs(record):
        lab = lab_evidence.get(notebook, {})
        original = contained(NOTEBOOKS, notebook)
        if original.is_file() and (
            lab.get("source_sha256") != digest(original)
            or lab.get("fixtures_sha256") != fixture_hash
            or lab.get("runtime") != current_runtime
            or lab.get("dependencies_sha256", {}) != notebook_dependencies(notebook)
        ):
            errors.append(f"{identifier}: absent or stale executed lab evidence: {notebook}")
    output = site / ".build" / "snippets" / f"{identifier}.json"
    if not output.is_file():
        return [*errors, f"{identifier}: no executed snippet evidence"]
    captured = json.loads(output.read_text())
    evidence_stale = (
        captured.get("status") != "passed"
        or captured.get("source_sha256") != digest(path)
        or captured.get("fixtures_sha256") != fixture_hash
        or captured.get("notebooks_sha256") != lesson_notebooks(record)
        or captured.get("runtime") != current_runtime
        or any(captured.get(key) != value for key, value in snippet_provenance().items())
    )
    if evidence_stale:
        errors.append(f"{identifier}: missing, failed or stale snippet evidence")
    if {b["id"] for b in captured.get("blocks", [])} != {b.id for b in blocks}:
        errors.append(f"{identifier}: output coverage differs from executable blocks")
    errors.extend(validate_snippet_assets(identifier, captured.get("blocks", []), site))
    if not evidence_stale:
        canonical = canonical_execution_blocks(record, blocks, NOTEBOOKS)
        expected_validation = {"status": "passed", **canonical_validation_manifest(canonical)}
        if captured.get("canonical_validation") != expected_validation:
            errors.append(f"{identifier}: canonical assertion evidence is missing or stale")
    return errors


def check(site: Path = SITE, check_api: bool = False, require_evidence: bool = False) -> tuple[list[str], list[dict]]:
    """Validate the authored curriculum and return its frontend projection."""
    manifest = curriculum_manifest(site)
    records = manifest.get("lesson", [])
    fixture_ids = set(manifest.get("fixtures", []))
    errors = [
        *validate_dependencies(records),
        *validate_fixture_progression(records, fixture_ids),
        *validate_fixture_sources(manifest, fixture_ids),
        *validate_shared_source_usage(records, manifest),
        *validate_instrument_fixture_usage(records),
        *validate_track_fixture_calls(records),
        *validate_book_stage_calls(records),
        *validate_lab_stage_exposure(records),
    ]
    projected = []
    lab_report = read_lab_report(site / ".build")
    lab_evidence = lab_entries_by_notebook(lab_report)
    if require_evidence:
        from build_labs import lab_report_errors

        errors.extend(lab_report_errors(lab_report))
    references = reference_anchors((REPO / "docs" / "REFERENCES.md").read_text())
    fixture_hash = fixture_digest()
    runtime = runtime_identity()
    expected_ids = (
        {f"1.{n}" for n in range(1, 6)}
        | {f"2.{n}" for n in range(1, 9)}
        | {f"3.{n}" for n in range(1, 7)}
        | {f"4.{n}" for n in range(1, 6)}
        | {f"C{n}" for n in range(1, 6)}
        | {f"V{n}" for n in range(1, 4)}
        | {"capstone"}
    )
    if {r["id"] for r in records} != expected_ids:
        errors.append("The curriculum must contain exactly the agreed 33 stable lesson IDs")
    for record in records:
        identifier = record["id"]
        try:
            if not ID_PATTERN.fullmatch(identifier):
                raise ValueError(f"Invalid lesson ID {identifier}")
            path = contained(site / "content" / "learn", record["path"])
            materialize_lesson(record, content_root=site / "content" / "learn", notebook_root=NOTEBOOKS)
            metadata, body = frontmatter(path)
            errors.extend(check_metadata(record, metadata, references, fixture_ids, check_api))
            errors.extend(validate_fixture_builds(record, body, manifest.get("fixture_sources", {})))
            errors.extend(
                f"{identifier}: missing notebook {notebook}"
                for notebook in lesson_notebook_names(record)
                if not contained(NOTEBOOKS, notebook).is_file()
            )
            text = path.read_text()
            blocks = extract_blocks(text, str(path))
            validate_output_references(text, identifier, blocks, str(path))
            if metadata.get("status") == "published":
                if not lesson_labs(record):
                    errors.append(f"{identifier}: published lesson has no lab")
                sections = ["DeskContext", "Standard", "BuildIt", "LabCard", "Exercise", "CheckYourself"]
                positions = [body.find(f"<{section}") for section in sections]
                if min(positions) < 0 or positions != sorted(positions):
                    errors.append(f"{identifier}: missing or out-of-order required lesson sections")
            if metadata.get("status") == "published" or require_evidence:
                errors.extend(check_evidence(record, path, blocks, site, lab_evidence, fixture_hash, runtime))
            projected.append({**record, **metadata})
        except (ValueError, TypeError, OSError, KeyError, SyntaxError) as error:
            errors.append(f"{identifier}: {error}")
    return errors, projected


def main() -> int:
    """Validate the manifest and emit the site reader's generated JSON."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check-api", action="store_true")
    parser.add_argument("--require-evidence", action="store_true")
    args = parser.parse_args()
    errors, records = check(check_api=args.check_api, require_evidence=args.require_evidence)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    write_json(BUILD / "curriculum.json", {"lessons": records})
    referenced = {notebook for record in records for notebook in lesson_notebook_names(record)}
    unused = sorted(
        str(p.relative_to(NOTEBOOKS))
        for p in NOTEBOOKS.rglob("*.ipynb")
        if str(p.relative_to(NOTEBOOKS)) not in referenced and ".ipynb_checkpoints" not in p.parts
    )
    print(f"Curriculum valid: {len(records)} lessons; {len(unused)} unused notebooks (informational)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
