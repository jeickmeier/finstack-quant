"""Enforce failure-transparent API usage in example notebooks."""

from __future__ import annotations

import ast
from collections.abc import Callable
from dataclasses import dataclass, field
import hashlib
from pathlib import Path
from typing import Final

import nbformat
from nbformat import NotebookNode
import pytest

NOTEBOOKS_DIR: Final = Path(__file__).resolve().parents[1] / "examples" / "notebooks"
INTENTIONAL_NEGATIVE_TAG: Final = "intentional-negative"


@dataclass(frozen=True, order=True)
class Finding:
    """A forbidden notebook construct."""

    path: str
    fingerprint: str
    line: int
    code: str
    message: str

    def render(self) -> str:
        """Return a stable, actionable description."""
        return f"{self.path} [{self.fingerprint}] line {self.line}: {self.code}: {self.message}"


# Entries must be ((repository-relative notebook path, cell fingerprint), reason).
# Broad catches and expected-success soft-fails are not eligible. An entry is only
# acceptable for an intentionally failing teaching cell tagged ``intentional-negative``.
ALLOWLIST: Final[dict[tuple[str, str], str]] = {}
TRACKED_BUILTINS: Final = frozenset({
    "BaseException",
    "Exception",
    "ImportError",
    "ModuleNotFoundError",
    "getattr",
    "hasattr",
})


def cell_fingerprint(source: str) -> str:
    """Hash normalized cell source independently of mutable cell ordering."""
    normalized = "\n".join(line.rstrip() for line in source.strip().splitlines())
    return hashlib.sha256(normalized.encode()).hexdigest()[:12]


@dataclass
class SymbolTable:
    """Module bindings carried across cells in one notebook."""

    bindings: dict[str, frozenset[str]] = field(default_factory=dict)


FIRST_PARTY: Final = "<first-party>"
BUILTINS_MODULE: Final = "<builtins-module>"
PASSTHROUGH_CONTEXT: Final = "<passthrough-context>"
UNKNOWN: Final = "<unknown>"
UNKNOWN_BINDING: Final = frozenset({UNKNOWN})


@dataclass
class Scope:
    """A lexical scope with explicit shadowing information."""

    bindings: dict[str, frozenset[str]]
    parent: Scope | None = None
    global_names: set[str] = field(default_factory=set)
    nonlocal_names: set[str] = field(default_factory=set)
    is_class: bool = False

    def _module(self) -> Scope:
        scope = self
        while scope.parent is not None:
            scope = scope.parent
        return scope

    def lookup(self, name: str) -> frozenset[str]:
        if name in self.global_names:
            return self._module().lookup(name)
        if name in self.bindings:
            return self.bindings[name]
        if name in self.nonlocal_names and self.parent is not None:
            return self.parent.lookup(name)
        if self.parent is not None:
            return self.parent.lookup(name)
        return frozenset({name}) if name in TRACKED_BUILTINS else UNKNOWN_BINDING

    def bind(self, name: str, binding: frozenset[str]) -> None:
        if name in self.global_names:
            self._module().bindings[name] = binding
            return
        if name in self.nonlocal_names and self.parent is not None:
            scope = self.parent
            while scope.parent is not None and name not in scope.bindings:
                scope = scope.parent
            scope.bindings[name] = binding
            return
        self.bindings[name] = binding


def _binding_for_expr(node: ast.expr, scope: Scope) -> frozenset[str]:
    if isinstance(node, ast.Name):
        return scope.lookup(node.id)
    if isinstance(node, ast.NamedExpr):
        return _binding_for_expr(node.value, scope)
    if isinstance(node, ast.Attribute):
        owners = _binding_for_expr(node.value, scope)
        bindings: set[str] = set()
        if BUILTINS_MODULE in owners and node.attr in TRACKED_BUILTINS:
            bindings.add(node.attr)
        if FIRST_PARTY in owners:
            bindings.add(FIRST_PARTY)
        if owners - {BUILTINS_MODULE, FIRST_PARTY}:
            bindings.add(UNKNOWN)
        return frozenset(bindings) or UNKNOWN_BINDING
    elif isinstance(node, (ast.Call, ast.Subscript)):
        value = node.func if isinstance(node, ast.Call) else node.value
        bindings = _binding_for_expr(value, scope)
        if FIRST_PARTY in bindings:
            return frozenset({FIRST_PARTY, *(bindings - {FIRST_PARTY})})
    return UNKNOWN_BINDING


def _exception_names(handler: ast.ExceptHandler, scope: Scope) -> set[str]:
    if handler.type is None:
        return {"<bare>"}
    if isinstance(handler.type, ast.Tuple):
        return {name for element in handler.type.elts for name in _binding_for_expr(element, scope) if name != UNKNOWN}
    return set(_binding_for_expr(handler.type, scope)) - {UNKNOWN}


def _is_finstack_import(node: ast.Import | ast.ImportFrom) -> bool:
    if isinstance(node, ast.Import):
        return any(alias.name == "finstack_quant" or alias.name.startswith("finstack_quant.") for alias in node.names)
    return node.module == "finstack_quant" or (node.module is not None and node.module.startswith("finstack_quant."))


def _pattern_names(pattern: ast.pattern) -> set[str]:
    names: set[str] = set()
    if isinstance(pattern, ast.MatchAs):
        if pattern.pattern is not None:
            names.update(_pattern_names(pattern.pattern))
        if pattern.name is not None:
            names.add(pattern.name)
    elif isinstance(pattern, ast.MatchStar):
        if pattern.name is not None:
            names.add(pattern.name)
    elif isinstance(pattern, ast.MatchMapping):
        for subpattern in pattern.patterns:
            names.update(_pattern_names(subpattern))
        if pattern.rest is not None:
            names.add(pattern.rest)
    elif isinstance(pattern, ast.MatchSequence):
        for subpattern in pattern.patterns:
            names.update(_pattern_names(subpattern))
    elif isinstance(pattern, ast.MatchClass):
        for subpattern in [*pattern.patterns, *pattern.kwd_patterns]:
            names.update(_pattern_names(subpattern))
    elif isinstance(pattern, ast.MatchOr):
        for subpattern in pattern.patterns:
            names.update(_pattern_names(subpattern))
    return names


class _LocalBindingCollector(ast.NodeVisitor):
    """Collect names that Python treats as local throughout a function."""

    def __init__(self) -> None:
        self.names: set[str] = set()
        self.global_names: set[str] = set()
        self.nonlocal_names: set[str] = set()

    def visit_Name(self, node: ast.Name) -> None:
        if isinstance(node.ctx, ast.Store):
            self.names.add(node.id)

    def visit_Import(self, node: ast.Import) -> None:
        self.names.update(alias.asname or alias.name.split(".", maxsplit=1)[0] for alias in node.names)

    def visit_ImportFrom(self, node: ast.ImportFrom) -> None:
        self.names.update(alias.asname or alias.name for alias in node.names if alias.name != "*")

    def visit_ExceptHandler(self, node: ast.ExceptHandler) -> None:
        if node.name is not None:
            self.names.add(node.name)
        self.generic_visit(node)

    def visit_Global(self, node: ast.Global) -> None:
        self.global_names.update(node.names)

    def visit_Nonlocal(self, node: ast.Nonlocal) -> None:
        self.nonlocal_names.update(node.names)

    def visit_FunctionDef(self, node: ast.FunctionDef) -> None:
        self.names.add(node.name)

    def visit_AsyncFunctionDef(self, node: ast.AsyncFunctionDef) -> None:
        self.names.add(node.name)

    def visit_ClassDef(self, node: ast.ClassDef) -> None:
        self.names.add(node.name)

    def visit_Lambda(self, node: ast.Lambda) -> None:
        del node

    def _visit_comprehension(
        self,
        node: ast.ListComp | ast.SetComp | ast.GeneratorExp | ast.DictComp,
    ) -> None:
        for generator in node.generators:
            self.visit(generator.iter)
            for condition in generator.ifs:
                self.visit(condition)
        if isinstance(node, ast.DictComp):
            self.visit(node.key)
            self.visit(node.value)
        else:
            self.visit(node.elt)

    def visit_ListComp(self, node: ast.ListComp) -> None:
        self._visit_comprehension(node)

    def visit_SetComp(self, node: ast.SetComp) -> None:
        self._visit_comprehension(node)

    def visit_GeneratorExp(self, node: ast.GeneratorExp) -> None:
        self._visit_comprehension(node)

    def visit_DictComp(self, node: ast.DictComp) -> None:
        self._visit_comprehension(node)

    def visit_Match(self, node: ast.Match) -> None:
        self.visit(node.subject)
        for case in node.cases:
            self.names.update(_pattern_names(case.pattern))
            if case.guard is not None:
                self.visit(case.guard)
            for statement in case.body:
                self.visit(statement)


class _HygieneScanner(ast.NodeVisitor):
    """Scan one cell while respecting sequential and lexical name binding."""

    def __init__(
        self,
        *,
        path: str,
        fingerprint: str,
        symbols: SymbolTable,
    ) -> None:
        self.path = path
        self.fingerprint = fingerprint
        self.findings: list[Finding] = []
        self.scope = Scope(symbols.bindings)
        self._try_import_collectors: list[tuple[int, list[tuple[dict[str, frozenset[str]], ...]]]] = []
        self._deferred_flow_depth = 0
        self._comprehension_contexts: list[tuple[Scope, bool]] = []

    def _finding(
        self,
        node: ast.stmt | ast.expr | ast.ExceptHandler,
        code: str,
        message: str,
    ) -> None:
        finding = Finding(
            self.path,
            self.fingerprint,
            node.lineno,
            code,
            message,
        )
        if finding not in self.findings:
            self.findings.append(finding)

    def _visit_block(self, statements: list[ast.stmt]) -> None:
        for statement in statements:
            self.visit(statement)

    def _scope_chain(self) -> list[Scope]:
        scopes = [self.scope]
        while scopes[-1].parent is not None:
            scopes.append(scopes[-1].parent)
        return scopes

    def _snapshot(self) -> tuple[dict[str, frozenset[str]], ...]:
        return tuple(scope.bindings.copy() for scope in self._scope_chain())

    def _restore(self, snapshot: tuple[dict[str, frozenset[str]], ...]) -> None:
        for scope, bindings in zip(self._scope_chain(), snapshot, strict=True):
            scope.bindings.clear()
            scope.bindings.update(bindings)

    @staticmethod
    def _lookup_snapshot(
        scopes: list[Scope],
        snapshot: tuple[dict[str, frozenset[str]], ...],
        level: int,
        name: str,
    ) -> frozenset[str]:
        scope = scopes[level]
        if name in scope.global_names:
            return _HygieneScanner._lookup_snapshot(scopes, snapshot, len(scopes) - 1, name)
        if name in snapshot[level]:
            return snapshot[level][name]
        if level + 1 < len(scopes):
            return _HygieneScanner._lookup_snapshot(scopes, snapshot, level + 1, name)
        return frozenset({name}) if name in TRACKED_BUILTINS else UNKNOWN_BINDING

    def _merge_snapshots(
        self,
        baseline: tuple[dict[str, frozenset[str]], ...],
        branches: list[tuple[dict[str, frozenset[str]], ...]],
    ) -> None:
        scopes = self._scope_chain()
        merged = [bindings.copy() for bindings in baseline]
        for level in range(len(scopes)):
            names = set().union(
                baseline[level],
                *(branch[level] for branch in branches),
            )
            for name in names:
                possibilities = set()
                for branch in branches:
                    possibilities.update(self._lookup_snapshot(scopes, branch, level, name))
                merged[level][name] = frozenset(possibilities)
        self._restore(tuple(merged))

    def _branch(
        self,
        baseline: tuple[dict[str, frozenset[str]], ...],
        statements: list[ast.stmt],
        setup: Callable[[], None] | None = None,
    ) -> tuple[dict[str, frozenset[str]], ...]:
        self._restore(baseline)
        if setup is not None:
            setup()
        self._visit_block(statements)
        return self._snapshot()

    def _bind_target(
        self,
        target: ast.expr,
        binding: frozenset[str] = UNKNOWN_BINDING,
    ) -> None:
        if isinstance(target, ast.Name):
            self.scope.bind(target.id, binding)
        elif isinstance(target, (ast.List, ast.Tuple)):
            for element in target.elts:
                self._bind_target(element)
        elif isinstance(target, ast.Starred):
            self._bind_target(target.value)

    def _bind_assignment_target(self, target: ast.expr, value: ast.expr) -> None:
        if isinstance(target, ast.Name):
            self._bind_target(target, _binding_for_expr(value, self.scope))
            return
        if isinstance(target, ast.Starred):
            self._bind_target(target.value)
            return
        if isinstance(target, (ast.List, ast.Tuple)):
            if isinstance(value, (ast.List, ast.Tuple)) and len(target.elts) == len(value.elts):
                for target_element, value_element in zip(target.elts, value.elts, strict=True):
                    self._bind_assignment_target(target_element, value_element)
            else:
                self._bind_target(target)

    def _bind_iteration_target(self, target: ast.expr, iterable: ast.expr) -> None:
        if not isinstance(iterable, (ast.List, ast.Set, ast.Tuple)):
            self._bind_target(target)
            return
        items = iterable.elts
        if isinstance(target, (ast.List, ast.Tuple)):
            for index, target_element in enumerate(target.elts):
                possibilities: set[str] = set()
                for item in items:
                    if isinstance(item, (ast.List, ast.Tuple)) and index < len(item.elts):
                        possibilities.update(_binding_for_expr(item.elts[index], self.scope))
                    else:
                        possibilities.add(UNKNOWN)
                self._bind_target(
                    target_element,
                    frozenset(possibilities) or UNKNOWN_BINDING,
                )
            return
        possibilities = set()
        for item in items:
            possibilities.update(_binding_for_expr(item, self.scope))
        self._bind_target(target, frozenset(possibilities) or UNKNOWN_BINDING)

    def visit_Module(self, node: ast.Module) -> None:
        self._visit_block(node.body)

    def _record_try_import(self, node: ast.Import | ast.ImportFrom) -> None:
        if self._try_import_collectors and _is_finstack_import(node):
            snapshot = self._snapshot()
            for depth, collector in self._try_import_collectors:
                if depth == self._deferred_flow_depth:
                    collector.append(snapshot)

    def visit_Import(self, node: ast.Import) -> None:
        self._record_try_import(node)
        for alias in node.names:
            local_name = alias.asname or alias.name.split(".", maxsplit=1)[0]
            if alias.name == "builtins":
                binding = frozenset({BUILTINS_MODULE})
            elif alias.name == "finstack_quant" or alias.name.startswith("finstack_quant."):
                binding = frozenset({FIRST_PARTY})
            else:
                binding = UNKNOWN_BINDING
            self.scope.bind(local_name, binding)

    def visit_ImportFrom(self, node: ast.ImportFrom) -> None:
        self._record_try_import(node)
        for alias in node.names:
            if alias.name == "*":
                continue
            local_name = alias.asname or alias.name
            if node.module == "builtins" and alias.name in TRACKED_BUILTINS:
                binding = frozenset({alias.name})
            elif node.module == "contextlib" and alias.name == "nullcontext":
                binding = frozenset({PASSTHROUGH_CONTEXT})
            elif node.module == "finstack_quant" or (
                node.module is not None and node.module.startswith("finstack_quant.")
            ):
                binding = frozenset({FIRST_PARTY})
            else:
                binding = UNKNOWN_BINDING
            self.scope.bind(local_name, binding)

    def visit_Assign(self, node: ast.Assign) -> None:
        self.visit(node.value)
        for target in node.targets:
            self._bind_assignment_target(target, node.value)

    def visit_AnnAssign(self, node: ast.AnnAssign) -> None:
        if node.annotation is not None:
            self.visit(node.annotation)
        if node.value is not None:
            self.visit(node.value)
        binding = _binding_for_expr(node.value, self.scope) if node.value is not None else UNKNOWN_BINDING
        self._bind_target(node.target, binding)

    def visit_AugAssign(self, node: ast.AugAssign) -> None:
        self.visit(node.value)
        self._bind_target(node.target)

    def visit_NamedExpr(self, node: ast.NamedExpr) -> None:
        self.visit(node.value)
        binding = _binding_for_expr(node.value, self.scope)
        if self._comprehension_contexts:
            containing_scope, definitely_runs = self._comprehension_contexts[-1]
            if not definitely_runs:
                binding = frozenset({*binding, *containing_scope.lookup(node.target.id)})
            containing_scope.bind(node.target.id, binding)
        else:
            self._bind_target(node.target, binding)

    def visit_Delete(self, node: ast.Delete) -> None:
        for target in node.targets:
            self._bind_target(target)

    def visit_If(self, node: ast.If) -> None:
        self.visit(node.test)
        baseline = self._snapshot()
        branches = [self._branch(baseline, node.body)]
        branches.append(self._branch(baseline, node.orelse) if node.orelse else baseline)
        self._merge_snapshots(baseline, branches)

    def _visit_loop(self, node: ast.For | ast.AsyncFor) -> None:
        self.visit(node.iter)
        baseline = self._snapshot()
        body_state = self._branch(
            baseline,
            node.body,
            lambda: self._bind_iteration_target(node.target, node.iter),
        )
        self._merge_snapshots(baseline, [baseline, body_state])
        loop_state = self._snapshot()
        if node.orelse:
            else_state = self._branch(loop_state, node.orelse)
            self._merge_snapshots(loop_state, [loop_state, else_state])

    def visit_For(self, node: ast.For) -> None:
        self._visit_loop(node)

    def visit_AsyncFor(self, node: ast.AsyncFor) -> None:
        self._visit_loop(node)

    def visit_While(self, node: ast.While) -> None:
        self.visit(node.test)
        baseline = self._snapshot()
        body_state = self._branch(baseline, node.body)
        self._merge_snapshots(baseline, [baseline, body_state])
        loop_state = self._snapshot()
        if node.orelse:
            else_state = self._branch(loop_state, node.orelse)
            self._merge_snapshots(loop_state, [loop_state, else_state])

    def _visit_with(self, node: ast.With | ast.AsyncWith) -> None:
        for item in node.items:
            self.visit(item.context_expr)
            if item.optional_vars is None:
                continue
            binding = UNKNOWN_BINDING
            if (
                isinstance(item.context_expr, ast.Call)
                and item.context_expr.args
                and PASSTHROUGH_CONTEXT in _binding_for_expr(item.context_expr.func, self.scope)
            ):
                binding = _binding_for_expr(item.context_expr.args[0], self.scope)
            self._bind_target(item.optional_vars, binding)
        self._visit_block(node.body)

    def visit_With(self, node: ast.With) -> None:
        self._visit_with(node)

    def visit_AsyncWith(self, node: ast.AsyncWith) -> None:
        self._visit_with(node)

    def _function_scope(
        self,
        node: ast.FunctionDef | ast.AsyncFunctionDef | ast.Lambda,
        body: list[ast.stmt],
    ) -> Scope:
        collector = _LocalBindingCollector()
        for statement in body:
            collector.visit(statement)
        arguments = {
            argument.arg
            for argument in (
                [*node.args.posonlyargs, *node.args.args, *node.args.kwonlyargs]
                + ([node.args.vararg] if node.args.vararg is not None else [])
                + ([node.args.kwarg] if node.args.kwarg is not None else [])
            )
        }
        local_names = (collector.names | arguments) - collector.global_names - collector.nonlocal_names
        parent = self.scope.parent if self.scope.is_class else self.scope
        return Scope(
            bindings=dict.fromkeys(local_names, UNKNOWN_BINDING),
            parent=parent,
            global_names=collector.global_names,
            nonlocal_names=collector.nonlocal_names,
        )

    def _visit_function(self, node: ast.FunctionDef | ast.AsyncFunctionDef) -> None:
        for decorator in node.decorator_list:
            self.visit(decorator)
        for default in [*node.args.defaults, *node.args.kw_defaults]:
            if default is not None:
                self.visit(default)
        if node.returns is not None:
            self.visit(node.returns)
        self.scope.bind(node.name, UNKNOWN_BINDING)
        outer_scope = self.scope
        self.scope = self._function_scope(node, node.body)
        self._deferred_flow_depth += 1
        self._visit_block(node.body)
        self._deferred_flow_depth -= 1
        self.scope = outer_scope

    def visit_FunctionDef(self, node: ast.FunctionDef) -> None:
        self._visit_function(node)

    def visit_AsyncFunctionDef(self, node: ast.AsyncFunctionDef) -> None:
        self._visit_function(node)

    def visit_Lambda(self, node: ast.Lambda) -> None:
        for default in [*node.args.defaults, *node.args.kw_defaults]:
            if default is not None:
                self.visit(default)
        outer_scope = self.scope
        self.scope = self._function_scope(node, [])
        self._deferred_flow_depth += 1
        self.visit(node.body)
        self._deferred_flow_depth -= 1
        self.scope = outer_scope

    def visit_ClassDef(self, node: ast.ClassDef) -> None:
        for decorator in node.decorator_list:
            self.visit(decorator)
        for base in node.bases:
            self.visit(base)
        for keyword in node.keywords:
            self.visit(keyword.value)
        outer_scope = self.scope
        self.scope = Scope({}, parent=outer_scope, is_class=True)
        self._deferred_flow_depth += 1
        self._visit_block(node.body)
        self._deferred_flow_depth -= 1
        self.scope = outer_scope
        self.scope.bind(node.name, UNKNOWN_BINDING)

    def _visit_comprehension(
        self,
        node: ast.ListComp | ast.SetComp | ast.GeneratorExp | ast.DictComp,
    ) -> None:
        first, *remaining = node.generators
        self.visit(first.iter)
        outer_scope = self.scope
        if outer_scope.is_class:
            assert outer_scope.parent is not None
            parent = outer_scope.parent
        else:
            parent = outer_scope
        containing_scope = self._comprehension_contexts[-1][0] if self._comprehension_contexts else parent
        definitely_runs = not isinstance(node, ast.GeneratorExp) and all(
            isinstance(generator.iter, (ast.List, ast.Set, ast.Tuple))
            and bool(generator.iter.elts)
            and not generator.ifs
            for generator in node.generators
        )
        if self._comprehension_contexts:
            definitely_runs = definitely_runs and self._comprehension_contexts[-1][1]
        self._comprehension_contexts.append((containing_scope, definitely_runs))
        self.scope = Scope({}, parent=parent)
        self._bind_iteration_target(first.target, first.iter)
        for condition in first.ifs:
            self.visit(condition)
        for generator in remaining:
            self.visit(generator.iter)
            self._bind_iteration_target(generator.target, generator.iter)
            for condition in generator.ifs:
                self.visit(condition)
        if isinstance(node, ast.DictComp):
            self.visit(node.key)
            self.visit(node.value)
        else:
            self.visit(node.elt)
        self.scope = outer_scope
        self._comprehension_contexts.pop()

    def visit_ListComp(self, node: ast.ListComp) -> None:
        self._visit_comprehension(node)

    def visit_SetComp(self, node: ast.SetComp) -> None:
        self._visit_comprehension(node)

    def visit_GeneratorExp(self, node: ast.GeneratorExp) -> None:
        self._visit_comprehension(node)

    def visit_DictComp(self, node: ast.DictComp) -> None:
        self._visit_comprehension(node)

    def _bind_pattern(self, pattern: ast.pattern) -> None:
        for name in _pattern_names(pattern):
            self.scope.bind(name, UNKNOWN_BINDING)

    @staticmethod
    def _is_irrefutable_case(case: ast.match_case) -> bool:
        return case.guard is None and isinstance(case.pattern, ast.MatchAs) and case.pattern.pattern is None

    def visit_Match(self, node: ast.Match) -> None:
        self.visit(node.subject)
        baseline = self._snapshot()
        branches = []
        for case in node.cases:

            def setup(case: ast.match_case = case) -> None:
                self._bind_pattern(case.pattern)
                if case.guard is not None:
                    self.visit(case.guard)

            branches.append(self._branch(baseline, case.body, setup))
        if not any(self._is_irrefutable_case(case) for case in node.cases):
            branches.append(baseline)
        self._merge_snapshots(baseline, branches)

    def visit_Call(self, node: ast.Call) -> None:
        self.visit(node.func)
        function_names = _binding_for_expr(node.func, self.scope)
        for argument in node.args:
            self.visit(argument)
        for keyword in node.keywords:
            self.visit(keyword.value)
        if node.args:
            targets = _binding_for_expr(node.args[0], self.scope)
            if FIRST_PARTY in targets and "hasattr" in function_names:
                self._finding(
                    node,
                    "public-api-probe",
                    "call the first-party API directly instead of using hasattr",
                )
            elif FIRST_PARTY in targets and "getattr" in function_names and len(node.args) >= 3:
                self._finding(
                    node,
                    "public-api-probe",
                    "call the first-party API directly instead of using a getattr default",
                )

    @staticmethod
    def _is_non_raising_expr(node: ast.expr) -> bool:
        if isinstance(node, (ast.Constant, ast.Name)):
            return True
        if isinstance(node, (ast.List, ast.Set, ast.Tuple)):
            return all(_HygieneScanner._is_non_raising_expr(item) for item in node.elts)
        if isinstance(node, ast.Dict):
            return all(key is None or _HygieneScanner._is_non_raising_expr(key) for key in node.keys) and all(
                _HygieneScanner._is_non_raising_expr(value) for value in node.values
            )
        return False

    @classmethod
    def _statement_may_raise(cls, node: ast.stmt) -> bool:
        if isinstance(node, ast.Assign):
            return not (
                all(isinstance(target, ast.Name) for target in node.targets) and cls._is_non_raising_expr(node.value)
            )
        if isinstance(node, ast.AnnAssign):
            return not (
                isinstance(node.target, ast.Name) and node.value is not None and cls._is_non_raising_expr(node.value)
            )
        return not isinstance(node, (ast.Global, ast.Nonlocal, ast.Pass))

    def _exception_names_at(
        self,
        handler: ast.ExceptHandler,
        snapshot: tuple[dict[str, frozenset[str]], ...],
    ) -> set[str]:
        current = self._snapshot()
        self._restore(snapshot)
        names = _exception_names(handler, self.scope)
        self._restore(current)
        return names

    def _visit_try(self, node: ast.Try | ast.TryStar) -> None:
        baseline = self._snapshot()
        for handler in node.handlers:
            names = _exception_names(handler, self.scope)
            if names & {"<bare>", "BaseException", "Exception"}:
                self._finding(
                    handler,
                    "broad-catch",
                    "catch the exact documented exception instead",
                )

        import_snapshots: list[tuple[dict[str, frozenset[str]], ...]] = []
        handler_entries: list[tuple[dict[str, frozenset[str]], ...]] = []
        self._try_import_collectors.append((self._deferred_flow_depth, import_snapshots))
        self._restore(baseline)
        for statement in node.body:
            if self._statement_may_raise(statement):
                handler_entries.append(self._snapshot())
            self.visit(statement)
        self._try_import_collectors.pop()
        self._visit_block(node.orelse)
        normal_state = self._snapshot()

        for handler in node.handlers:
            if any(
                self._exception_names_at(handler, snapshot) & {"ImportError", "ModuleNotFoundError"}
                for snapshot in import_snapshots
            ):
                self._finding(
                    handler,
                    "first-party-import-soft-fail",
                    "finstack_quant imports must fail visibly",
                )

        branches = [normal_state]
        if handler_entries:
            self._restore(handler_entries[0])
            self._merge_snapshots(handler_entries[0], handler_entries)
            handler_base = self._snapshot()
        else:
            handler_base = baseline
        for handler in node.handlers:

            def setup(handler: ast.ExceptHandler = handler) -> None:
                if handler.name is not None:
                    self.scope.bind(handler.name, UNKNOWN_BINDING)

            handler_state = self._branch(handler_base, handler.body, setup)
            if handler_entries:
                branches.append(handler_state)
        self._merge_snapshots(baseline, branches)
        self._visit_block(node.finalbody)

    def visit_Try(self, node: ast.Try) -> None:
        self._visit_try(node)

    def visit_TryStar(self, node: ast.TryStar) -> None:
        self._visit_try(node)


def scan_source(
    source: str,
    *,
    path: str = "<synthetic>",
    tags: frozenset[str] = frozenset(),
    symbols: SymbolTable | None = None,
) -> list[Finding]:
    """Find forbidden failure-hiding constructs in one code cell."""
    try:
        tree = ast.parse(source)
    except SyntaxError:
        return []

    active_symbols = symbols if symbols is not None else SymbolTable()
    fingerprint = cell_fingerprint(source)
    scanner = _HygieneScanner(
        path=path,
        fingerprint=fingerprint,
        symbols=active_symbols,
    )
    scanner.visit(tree)
    if INTENTIONAL_NEGATIVE_TAG not in tags:
        return scanner.findings
    return scanner.findings


def _code_cells(notebook: NotebookNode) -> list[NotebookNode]:
    return [cell for cell in notebook.cells if cell.cell_type == "code"]


def _notebook_paths(root: Path = NOTEBOOKS_DIR) -> list[Path]:
    return [path for path in sorted(root.rglob("*.ipynb")) if ".ipynb_checkpoints" not in path.parts]


def scan_notebooks(root: Path = NOTEBOOKS_DIR) -> list[Finding]:
    """Scan notebooks in deterministic repository-relative order."""
    findings: list[Finding] = []
    for notebook_path in _notebook_paths(root):
        relative = notebook_path.relative_to(root).as_posix()
        notebook = nbformat.read(notebook_path, as_version=4)
        symbols = SymbolTable()
        for cell in _code_cells(notebook):
            tags = frozenset(cell.metadata.get("tags", []))
            for finding in scan_source(
                cell.source,
                path=relative,
                tags=tags,
                symbols=symbols,
            ):
                key = (relative, finding.fingerprint)
                reason = ALLOWLIST.get(key)
                if reason is None:
                    findings.append(finding)
                    continue
                assert reason.strip(), f"Allowlist entry {key!r} requires a reason"
                assert INTENTIONAL_NEGATIVE_TAG in tags, (
                    f"Allowlisted cell {key!r} must be tagged {INTENTIONAL_NEGATIVE_TAG!r}"
                )
    return sorted(findings)


@pytest.mark.parametrize(
    ("source", "code"),
    [
        ("try:\n    run()\nexcept Exception:\n    pass", "broad-catch"),
        ("try:\n    run()\nexcept BaseException:\n    pass", "broad-catch"),
        ("try:\n    run()\nexcept:\n    pass", "broad-catch"),
        (
            "from builtins import Exception as CatchAll\ntry:\n    run()\nexcept CatchAll:\n    pass",
            "broad-catch",
        ),
        (
            "import builtins as bi\ntry:\n    run()\nexcept bi.BaseException:\n    pass",
            "broad-catch",
        ),
        (
            "import builtins\ntry:\n    run()\nexcept (ValueError, builtins.Exception):\n    pass",
            "broad-catch",
        ),
        (
            "async def demo():\n"
            "    try:\n        await run()\n"
            "    except Exception as exc:\n"
            "        raise RuntimeError(f'failed: {exc}') from exc",
            "broad-catch",
        ),
        (
            "try:\n    from finstack_quant import Money\nexcept ImportError:\n    Money = None",
            "first-party-import-soft-fail",
        ),
        (
            "from builtins import hasattr as has\nfrom finstack_quant import Money as Cash\nhas(Cash, 'from_json')",
            "public-api-probe",
        ),
        (
            "import builtins\nfrom finstack_quant import Money\nbuiltins.hasattr(Money, 'from_json')",
            "public-api-probe",
        ),
        (
            "from builtins import getattr as get\nfrom finstack_quant import Money\nget(Money, 'from_json', None)",
            "public-api-probe",
        ),
        (
            "import builtins as bi\nimport finstack_quant as fq\nbi.getattr(fq.valuations, 'price', None)",
            "public-api-probe",
        ),
        (
            "CatchAll = Exception\ntry:\n    run()\nexcept CatchAll:\n    pass",
            "broad-catch",
        ),
        (
            "from finstack_quant import Money\nprobe = hasattr\nprobe(Money, 'from_json')",
            "public-api-probe",
        ),
        (
            "from finstack_quant import Money\nprobe = getattr\nprobe(Money, 'from_json', None)",
            "public-api-probe",
        ),
        (
            "try:\n    run_group()\nexcept* Exception:\n    pass",
            "broad-catch",
        ),
        (
            "try:\n    import finstack_quant\nexcept* ImportError:\n    pass",
            "first-party-import-soft-fail",
        ),
        (
            "try:\n"
            "    ImportProblem = ImportError\n"
            "    if should_import:\n"
            "        import finstack_quant\n"
            "except ImportProblem:\n"
            "    pass",
            "first-party-import-soft-fail",
        ),
        (
            "def load():\n    try:\n        import finstack_quant\n    except ImportError:\n        pass",
            "first-party-import-soft-fail",
        ),
    ],
)
def test_scanner_rejects_forbidden_synthetic_snippets(source: str, code: str) -> None:
    """Each forbidden construct should produce a specific diagnostic."""
    assert code in {finding.code for finding in scan_source(source)}


@pytest.mark.parametrize(
    "source",
    [
        "try:\n    run()\nexcept ValueError:\n    pass",
        "hasattr(str, 'upper')",
        "getattr(str, 'upper', None)",
        ("from finstack_quant import Money\nassert hasattr(str, 'upper')\nparser = getattr(str, 'upper', None)"),
        "from finstack_quant import Money\ngetattr(Money, 'from_json')",
        "from finstack_quant import Money\nMoney = str\nhasattr(Money, 'from_json')",
        ("from finstack_quant import Money\nhasattr = custom_probe\nhasattr(Money, 'from_json')"),
        ("DomainError = RuntimeError\nException = DomainError\ntry:\n    run()\nexcept Exception:\n    pass"),
        ("from finstack_quant import Money\ndef inspect(hasattr):\n    return hasattr(Money, 'from_json')"),
        ("def execute(Exception):\n    try:\n        run()\n    except Exception:\n        pass"),
        (
            "from finstack_quant import Money\n"
            "def inspect():\n"
            "    getattr = custom_probe\n"
            "    return getattr(Money, 'from_json', None)"
        ),
        ("from finstack_quant import Money\nprobe = hasattr\nprobe = custom_probe\nprobe(Money, 'from_json')"),
        ("try:\n    def load():\n        import finstack_quant\nexcept ImportError:\n    pass"),
        ("try:\n    class Loader:\n        import finstack_quant\nexcept ImportError:\n    pass"),
        ("try:\n    import finstack_quant\n    ImportProblem = ImportError\nexcept ImportProblem:\n    pass"),
    ],
)
def test_scanner_allows_precise_or_unrelated_synthetic_snippets(source: str) -> None:
    """Precise catches and unrelated introspection are legitimate."""
    assert scan_source(source) == []


@pytest.mark.parametrize(
    "source",
    [
        ("from finstack_quant import Money\nprobe, target = [hasattr, Money]\nprobe(target, 'from_json')"),
        ("from finstack_quant import Money\nfor probe in [hasattr]:\n    probe(Money, 'from_json')"),
        (
            "from contextlib import nullcontext\n"
            "from finstack_quant import Money\n"
            "with nullcontext(hasattr) as probe:\n"
            "    probe(Money, 'from_json')"
        ),
        ("from finstack_quant import Money\n[probe(Money, 'from_json') for probe in [hasattr]]"),
        ("from finstack_quant import Money\n[(probe := hasattr) for _ in [1]]\nprobe(Money, 'from_json')"),
        ("from finstack_quant import Money\n[(probe := hasattr)(Money, 'from_json') for _ in [1]]"),
        ("from finstack_quant import Money\n[(hasattr := custom_probe) for _ in values]\nhasattr(Money, 'from_json')"),
        (
            "from finstack_quant import Money\n"
            "generator = ((hasattr := custom_probe) for _ in [1])\n"
            "hasattr(Money, 'from_json')"
        ),
        ("from finstack_quant import Money\n[None for hasattr in custom_probes]\nhasattr(Money, 'from_json')"),
        (
            "from finstack_quant import Money\n"
            "def inspect():\n"
            "    [None for hasattr in custom_probes]\n"
            "    return hasattr(Money, 'from_json')"
        ),
        (
            "from finstack_quant import Money\n"
            "class Checks:\n"
            "    probe = hasattr\n"
            "    supported = probe(Money, 'from_json')"
        ),
        (
            "from finstack_quant import Money\n"
            "class Checks:\n"
            "    hasattr = custom_probe\n"
            "    def inspect(self):\n"
            "        return hasattr(Money, 'from_json')"
        ),
        ("from finstack_quant import Money\nif condition:\n    hasattr = custom_probe\nhasattr(Money, 'from_json')"),
        (
            "from finstack_quant import Money\n"
            "if condition:\n"
            "    probe = hasattr\n"
            "else:\n"
            "    probe = hasattr\n"
            "probe(Money, 'from_json')"
        ),
        (
            "from finstack_quant import Money\n"
            "match value:\n"
            "    case 1:\n"
            "        Money = str\n"
            "    case _:\n"
            "        pass\n"
            "hasattr(Money, 'from_json')"
        ),
        ("from finstack_quant import Money\nfor hasattr in custom_probes:\n    pass\nhasattr(Money, 'from_json')"),
        (
            "from finstack_quant import Money\n"
            "async def inspect(stream):\n"
            "    probe = hasattr\n"
            "    async for item in stream:\n"
            "        probe(Money, 'from_json')"
        ),
        (
            "from finstack_quant import Money\n"
            "async def inspect():\n"
            "    async for probe in [hasattr]:\n"
            "        probe(Money, 'from_json')"
        ),
        (
            "from finstack_quant import Money\n"
            "async def inspect(manager):\n"
            "    probe = hasattr\n"
            "    async with manager as resource:\n"
            "        probe(Money, 'from_json')"
        ),
    ],
)
def test_scanner_tracks_bindings_through_compound_syntax(source: str) -> None:
    """Potential first-party probes remain visible through compound syntax."""
    assert "public-api-probe" in {finding.code for finding in scan_source(source)}


@pytest.mark.parametrize(
    "source",
    [
        ("from finstack_quant import Money\n[probe, target] = (custom_probe, str)\nprobe(target, 'from_json')"),
        ("from finstack_quant import Money\nfor hasattr in [custom_probe]:\n    hasattr(Money, 'from_json')"),
        (
            "from finstack_quant import Money\n"
            "async def inspect(stream):\n"
            "    async for hasattr in stream:\n"
            "        hasattr(Money, 'from_json')"
        ),
        ("from finstack_quant import Money\nwith manager as hasattr:\n    hasattr(Money, 'from_json')"),
        (
            "from finstack_quant import Money\n"
            "async def inspect(manager):\n"
            "    async with manager as getattr:\n"
            "        getattr(Money, 'from_json', None)"
        ),
        (
            "from finstack_quant import Money\n"
            "async def inspect(manager):\n"
            "    async with manager(hasattr) as probe:\n"
            "        probe(Money, 'from_json')"
        ),
        ("from finstack_quant import Money\n[hasattr(Money, 'from_json') for hasattr in [custom_probe]]"),
        ("from finstack_quant import Money\n[(hasattr := custom_probe) for _ in [1]]\nhasattr(Money, 'from_json')"),
        ("from finstack_quant import Money\n[(custom := custom_probe)(Money, 'from_json') for _ in [1]]"),
        ("from finstack_quant import Money\nclass Checks:\n    probe = hasattr\nprobe(Money, 'from_json')"),
        (
            "from finstack_quant import Money\n"
            "def inspect(value):\n"
            "    hasattr(Money, 'from_json')\n"
            "    match value:\n"
            "        case hasattr:\n"
            "            pass"
        ),
        ("import finstack_quant as fq\ntry:\n    fq = str\nexcept ValueError:\n    pass\nhasattr(fq, 'valuations')"),
        (
            "from finstack_quant import Money\n"
            "try:\n"
            "    hasattr = custom_probe\n"
            "except* ValueError:\n"
            "    pass\n"
            "hasattr(Money, 'from_json')"
        ),
        (
            "from finstack_quant import Money\n"
            "if condition:\n"
            "    hasattr = first_custom_probe\n"
            "else:\n"
            "    hasattr = second_custom_probe\n"
            "hasattr(Money, 'from_json')"
        ),
        (
            "from finstack_quant import Money\n"
            "if condition:\n"
            "    Money = str\n"
            "else:\n"
            "    Money = bytes\n"
            "hasattr(Money, 'from_json')"
        ),
        (
            "from finstack_quant import Money\n"
            "try:\n"
            "    Money = str\n"
            "except ValueError:\n"
            "    Money = bytes\n"
            "hasattr(Money, 'from_json')"
        ),
        (
            "from finstack_quant import Money\n"
            "try:\n"
            "    getattr = first_custom_probe\n"
            "except* ValueError:\n"
            "    getattr = second_custom_probe\n"
            "getattr(Money, 'from_json', None)"
        ),
        (
            "from finstack_quant import Money\n"
            "match value:\n"
            "    case 1:\n"
            "        Money = str\n"
            "    case _:\n"
            "        Money = bytes\n"
            "hasattr(Money, 'from_json')"
        ),
    ],
)
def test_scanner_respects_definite_shadowing_in_compound_syntax(source: str) -> None:
    """Bindings shadowed on every reachable path do not produce diagnostics."""
    assert scan_source(source) == []


def test_repository_notebooks_have_transparent_failures() -> None:
    """Notebooks must not hide first-party API or expected-success failures."""
    findings = scan_notebooks()
    details = "\n".join(finding.render() for finding in findings)
    assert not findings, f"Notebook hygiene violations:\n{details}"


def test_repository_notebooks_are_valid_and_compile() -> None:
    """Every notebook should be valid nbformat and contain compilable code cells."""
    for notebook_path in _notebook_paths():
        notebook = nbformat.read(notebook_path, as_version=4)
        nbformat.validate(notebook)
        relative = notebook_path.relative_to(NOTEBOOKS_DIR).as_posix()
        for cell in _code_cells(notebook):
            compile(
                cell.source,
                f"{relative} [{cell_fingerprint(cell.source)}]",
                "exec",
            )


def test_allowlist_has_no_pending_f6_entries() -> None:
    """Slice 14 must not retain temporary F6 migration exemptions."""
    assert all("pending_f6" not in reason for reason in ALLOWLIST.values())
