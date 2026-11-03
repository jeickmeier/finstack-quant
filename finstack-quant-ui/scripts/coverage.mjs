import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
import ts from "typescript";
import { emit } from "./fixture-output.mjs";

// Facade coverage ledger. One unit is a function or class key of
// finstack-quant-wasm/facade-surface.json; a class is covered when the worker
// closure reaches any of its members through a `<Crate>Native` Pick leaf.
// Default mode rewrites facade-coverage.json; `--check` verifies it.

/**
 * Reasons a listed exclusion may give; rule exclusions are computed. A unit
 * a worker later picks leaves `excluded` (the check says so).
 * - `data-constructor`: builds a plain value from its fields or a label
 *   (`::new`, `from_str`, `parse`), or is a handle class with no method
 *   beyond its constructors.
 * - `schema-form`: an enum-variant or preset value a schema form builds.
 * - `setter`: returns a copy of its input with fields set (`with_*`, `add_*`).
 * - `getter`: reads one stored field, row or entry of a value (`get_*`,
 *   `node_ids`), or is a result handle class whose only members are getters.
 * - `validator`: a shape check returning nothing or the canonical JSON of its
 *   input; one returning a boolean, number or report stays counted.
 * - `performance-duplicate`: an `analytics` free function on one series that
 *   runs the same kernel as the `Performance` method of the same name.
 */
export const listedReasons = new Set([
  "data-constructor",
  "getter",
  "performance-duplicate",
  "schema-form",
  "setter",
  "validator",
]);
const unitKinds = new Set(["function", "class"]);
const leafName = (key) => key.split(".").at(-1);
const byCodeUnit = (a, b) => (a < b ? -1 : a > b ? 1 : 0);
const fix = "run `npm --prefix finstack-quant-ui run coverage`";

/** True for a facade function or class key. */
export const isUnit = (surface, key) => unitKinds.has(surface[key]?.kind);

/** The typed twin of a `<x>Json` function whose `<x>` is also a facade function, else null. */
export function twinOf(surface, key) {
  if (!key.endsWith("Json") || surface[key]?.kind !== "function") return null;
  const typed = key.slice(0, -"Json".length);
  return surface[typed]?.kind === "function" ? typed : null;
}

/** The unit a picked key counts toward: a wire twin counts as its typed twin. */
export const unitOf = (surface, key) => twinOf(surface, key) ?? key;

/** Rule exclusion of a key (`schema-registry`, `wire-twin`, `builder`), or null. */
export function ruleExclusion(surface, key) {
  if (key.split(".").includes("schema")) return "schema-registry";
  if (twinOf(surface, key)) return "wire-twin";
  if (/Builder$/.test(leafName(key))) return "builder";
  return null;
}

/**
 * Pick leaves declared in one source file: every `Pick<typeof <facade path>, "...">`
 * whose path starts at a facade namespace, as `{ file, key }`.
 */
export function collectPicks(sourceFile, surface, file = sourceFile.fileName) {
  const picks = [];
  const visit = (node) => {
    if (
      ts.isTypeReferenceNode(node) &&
      node.typeName.getText(sourceFile) === "Pick" &&
      node.typeArguments?.length === 2 &&
      ts.isTypeQueryNode(node.typeArguments[0])
    ) {
      const [query, names] = node.typeArguments;
      const owner = query.exprName.getText(sourceFile);
      if (surface[owner]?.kind === "namespace")
        for (const name of ts.isUnionTypeNode(names) ? names.types : [names])
          if (ts.isLiteralTypeNode(name) && ts.isStringLiteral(name.literal))
            picks.push({ file, key: `${owner}.${name.literal.text}` });
    }
    ts.forEachChild(node, visit);
  };
  visit(sourceFile);
  return picks;
}

/** True when a source file value-imports (or re-exports) the bare `finstack-quant-wasm` specifier. */
export function importsWasmValue(sourceFile) {
  const valueImport = (node) => {
    if (
      (!ts.isImportDeclaration(node) && !ts.isExportDeclaration(node)) ||
      !ts.isStringLiteral(node.moduleSpecifier ?? {}) ||
      node.moduleSpecifier.text !== "finstack-quant-wasm"
    )
      return false;
    if (ts.isExportDeclaration(node))
      return (
        !node.isTypeOnly &&
        (!node.exportClause ||
          !ts.isNamedExports(node.exportClause) ||
          node.exportClause.elements.some((element) => !element.isTypeOnly))
      );
    const clause = node.importClause;
    if (!clause) return true; // side-effect import
    if (clause.isTypeOnly) return false;
    if (clause.name) return true;
    const bindings = clause.namedBindings;
    return (
      !bindings ||
      ts.isNamespaceImport(bindings) ||
      bindings.elements.some((element) => !element.isTypeOnly)
    );
  };
  let found = false;
  const visit = (node) => {
    if (
      valueImport(node) ||
      (ts.isCallExpression(node) &&
        node.expression.kind === ts.SyntaxKind.ImportKeyword &&
        ts.isStringLiteral(node.arguments[0] ?? {}) &&
        node.arguments[0].text === "finstack-quant-wasm")
    )
      found = true;
    else ts.forEachChild(node, visit);
  };
  visit(sourceFile);
  return found;
}

/**
 * Facade units and class members the worker closure references, resolved by
 * the checker to their declarations in the facade `index.d.ts`.
 */
export function collectUsage(program, facade, files, surface) {
  const checker = program.getTypeChecker();
  const namespaces = new Map(); // namespace interface -> facade path
  const classes = new Map(); // class or constructor interface -> class key
  const walk = (owner, type) => {
    const symbol = type.getSymbol() ?? type.aliasSymbol;
    if (symbol) namespaces.set(symbol.getName(), owner);
    for (const property of type.getProperties()) {
      const key = `${owner}.${property.getName()}`;
      const kind = surface[key]?.kind;
      const declaration = property.declarations?.[0];
      if (!kind || !declaration) continue;
      const value = checker.getTypeOfSymbolAtLocation(property, declaration);
      if (kind === "namespace") walk(key, value);
      if (kind !== "class") continue;
      const constructor = value.getSymbol()?.getName();
      if (constructor) classes.set(constructor, key);
      // Instances come from constructors or from statics such as `fromJson`.
      const signatures = [
        ...value.getConstructSignatures(),
        ...value.getCallSignatures(),
        ...value
          .getProperties()
          .flatMap((s) =>
            checker
              .getTypeOfSymbolAtLocation(s, s.declarations?.[0] ?? declaration)
              .getCallSignatures(),
          ),
      ];
      for (const signature of signatures) {
        const instance = signature.getReturnType().getSymbol()?.getName();
        if (instance === leafName(key)) classes.set(instance, key);
      }
    }
  };
  for (const statement of facade.statements)
    if (ts.isVariableStatement(statement))
      for (const declaration of statement.declarationList.declarations) {
        const name = declaration.name.getText(facade);
        if (surface[name]?.kind === "namespace")
          walk(name, checker.getTypeAtLocation(declaration));
      }
  // Remaining instance interfaces carry the class name (`VolCube` for `core.VolCube`).
  for (const [key, entry] of Object.entries(surface))
    if (entry.kind === "class" && !classes.has(leafName(key)))
      classes.set(leafName(key), key);
  const units = new Set();
  const members = new Map();
  const member = (key, name) =>
    (members.get(key) ?? members.set(key, new Set()).get(key)).add(name);
  const resolve = (node) => {
    const declaration = checker.getSymbolAtLocation(node.name)
      ?.declarations?.[0];
    if (declaration?.getSourceFile() !== facade) return;
    const owner = ts.isInterfaceDeclaration(declaration.parent)
      ? declaration.parent.name.text
      : null;
    const name = node.name.text;
    if (owner && namespaces.has(owner)) {
      const key = `${namespaces.get(owner)}.${name}`;
      if (isUnit(surface, key)) units.add(key);
      return;
    }
    // Shared declarations (`WasmOwned.free`) resolve through the receiver type.
    const receiver = checker
      .getTypeAtLocation(node.expression)
      .getSymbol()
      ?.getName();
    const key = classes.get(owner) ?? classes.get(receiver);
    if (!key) return;
    units.add(key);
    member(key, name);
  };
  const visit = (node) => {
    if (ts.isPropertyAccessExpression(node)) resolve(node);
    if (ts.isNewExpression(node)) {
      const constructor = checker
        .getTypeAtLocation(node.expression)
        .getSymbol()
        ?.getName();
      const key = classes.get(constructor);
      if (key) member(key, "constructor");
    }
    ts.forEachChild(node, visit);
  };
  for (const file of files) visit(file);
  return { units, members };
}

/** The ledger with `covered` replaced by the picked units, which leave `planned`. */
export function nextLedger(ledger, picked) {
  const covered = [...new Set(picked)].sort(byCodeUnit);
  return {
    covered,
    planned: Object.fromEntries(
      Object.entries(ledger.planned)
        .filter(([key]) => !covered.includes(key))
        .sort(([a], [b]) => byCodeUnit(a, b)),
    ),
    excluded: Object.fromEntries(
      Object.entries(ledger.excluded).sort(([a], [b]) => byCodeUnit(a, b)),
    ),
  };
}

/**
 * Every gate violation, each naming its fix.
 *
 * @param input.surface - facade-surface.json
 * @param input.ledger - facade-coverage.json
 * @param input.picks - Pick leaves `{ file, key }` in the worker closure
 * @param input.used - facade keys the worker closure references
 * @param input.valueImports - closure files other than the worker entry that value-import `finstack-quant-wasm`
 */
export function checkLedger({ surface, ledger, picks, used, valueImports }) {
  const problems = [];
  const picked = new Set();
  for (const { file, key } of picks) {
    const kind = surface[key]?.kind;
    if (kind === "namespace")
      problems.push(
        `${file} picks the namespace \`${key}\`; pick each facade member it calls instead.`,
      );
    else if (!kind)
      problems.push(
        `${file} picks \`${key}\`, which is not in finstack-quant-wasm/facade-surface.json; rename or drop the leaf.`,
      );
    else if (!used.has(key))
      problems.push(
        `${file} picks \`${key}\` but the worker closure never references it; drop the leaf.`,
      );
    else {
      const unit = unitOf(surface, key);
      const rule = ruleExclusion(surface, unit);
      if (rule)
        problems.push(
          `${file} picks \`${key}\`, excluded by the ${rule} rule; call a counted member instead.`,
        );
      else if (Object.hasOwn(ledger.excluded, unit))
        problems.push(
          `${file} picks \`${unit}\`, listed as excluded (${ledger.excluded[unit]}); remove it from \`excluded\` in facade-coverage.json.`,
        );
      else picked.add(unit);
    }
  }
  const covered = new Set(ledger.covered);
  for (const key of covered)
    if (!picked.has(key))
      problems.push(`Covered \`${key}\` is no longer picked; ${fix}.`);
  for (const key of picked)
    if (!covered.has(key))
      problems.push(`Picked \`${key}\` is not in \`covered\`; ${fix}.`);
  if (
    covered.size === picked.size &&
    !isDeepStrictEqual(ledger.covered, [...ledger.covered].sort(byCodeUnit))
  )
    problems.push(`\`covered\` is not sorted or has duplicates; ${fix}.`);
  for (const [key, slice] of Object.entries(ledger.planned)) {
    if (picked.has(key))
      problems.push(`Planned \`${key}\` (${slice}) is now picked; ${fix}.`);
    else if (!isUnit(surface, key) || ruleExclusion(surface, key))
      problems.push(
        `Planned \`${key}\` (${slice}) is not a counted facade function or class; delete it from \`planned\`.`,
      );
    else if (Object.hasOwn(ledger.excluded, key))
      problems.push(
        `\`${key}\` is both planned and excluded; keep one in facade-coverage.json.`,
      );
    if (typeof slice !== "string" || !slice)
      problems.push(`Planned \`${key}\` needs a slice id.`);
  }
  for (const [key, reason] of Object.entries(ledger.excluded)) {
    if (!listedReasons.has(reason))
      problems.push(
        `Excluded \`${key}\` gives unknown reason \`${reason}\`; use one of ${[...listedReasons].join(", ")}.`,
      );
    if (!isUnit(surface, key))
      problems.push(
        `Excluded \`${key}\` is not a facade function or class; delete it from \`excluded\`.`,
      );
    else if (ruleExclusion(surface, key))
      problems.push(
        `Excluded \`${key}\` is already excluded by the ${ruleExclusion(surface, key)} rule; delete it from \`excluded\`.`,
      );
  }
  for (const file of valueImports)
    problems.push(
      `${file} value-imports \`finstack-quant-wasm\`; only finstack.worker.ts may, other worker files take the facade as \`native\` and import types only.`,
    );
  return problems;
}

/** Covered and in-scope counts per top-level namespace, plus method-level use of in-scope classes. */
export function tally(surface, ledger, members) {
  const rows = {};
  let classMembers = 0;
  let usedMembers = 0;
  const covered = new Set(ledger.covered);
  for (const [key, entry] of Object.entries(surface)) {
    if (
      !isUnit(surface, key) ||
      ruleExclusion(surface, key) ||
      Object.hasOwn(ledger.excluded, key)
    )
      continue;
    const row = (rows[key.split(".")[0]] ??= { covered: 0, scope: 0 });
    row.scope++;
    if (covered.has(key)) row.covered++;
    if (entry.kind !== "class") continue;
    const names = new Set([
      ...(entry.constructor === true ? ["constructor"] : []),
      ...(entry.statics ?? []),
      ...(entry.methods ?? []),
      ...(entry.getters ?? []),
    ]);
    for (const plumbing of ["free", "toJson", "toString"])
      names.delete(plumbing);
    classMembers += names.size;
    usedMembers += [...(members.get(key) ?? [])].filter((name) =>
      names.has(name),
    ).length;
  }
  return { rows, classMembers, usedMembers };
}

const percent = (part, whole) =>
  whole ? `${((100 * part) / whole).toFixed(1)}%` : "-";

async function main() {
  const root = fileURLToPath(new URL("../", import.meta.url));
  const checking = process.argv.includes("--check");
  const ledgerPath = path.join(root, "facade-coverage.json");
  const surface = JSON.parse(
    await readFile(
      path.join(root, "../finstack-quant-wasm/facade-surface.json"),
      "utf8",
    ),
  );
  const ledger = JSON.parse(await readFile(ledgerPath, "utf8"));
  const config = ts.parseJsonConfigFileContent(
    ts.readConfigFile(path.join(root, "tsconfig.json"), ts.sys.readFile).config,
    ts.sys,
    root,
  );
  const entry = path.join(root, "registry/shared/file/finstack.worker.ts");
  const program = ts.createProgram([entry], config.options);
  const facade = program
    .getSourceFiles()
    .find((file) => /finstack-quant-wasm\/index\.d\.ts$/.test(file.fileName));
  if (!facade) throw new Error("finstack-quant-wasm/index.d.ts is unresolved");
  // The worker closure: every package source the worker entry reaches.
  const closure = program
    .getSourceFiles()
    .filter(
      (file) =>
        !file.isDeclarationFile &&
        !path.relative(root, file.fileName).startsWith("..") &&
        !file.fileName.includes("/node_modules/"),
    );
  const relative = (file) => path.relative(root, file.fileName);
  const picks = closure.flatMap((file) =>
    collectPicks(file, surface, relative(file)),
  );
  const { units: used, members } = collectUsage(
    program,
    facade,
    closure,
    surface,
  );
  const valueImports = closure
    .filter((file) => file.fileName !== entry && importsWasmValue(file))
    .map(relative);
  const pickedUnits = picks
    .filter(({ key }) => isUnit(surface, key))
    .map(({ key }) => unitOf(surface, key));
  const result = checking ? ledger : nextLedger(ledger, pickedUnits);
  const problems = checkLedger({
    surface,
    ledger: result,
    picks,
    used,
    valueImports,
  });
  if (!checking) await emit(ledgerPath, JSON.stringify(result));
  const { rows, classMembers, usedMembers } = tally(surface, result, members);
  let covered = 0;
  let scope = 0;
  for (const [namespace, row] of Object.entries(rows).sort(([a], [b]) =>
    byCodeUnit(a, b),
  )) {
    covered += row.covered;
    scope += row.scope;
    if (row.covered)
      console.log(
        `  ${namespace}: ${row.covered}/${row.scope} (${percent(row.covered, row.scope)})`,
      );
  }
  console.log(
    `Facade coverage: ${covered}/${scope} units (${percent(covered, scope)}), ${Object.keys(result.planned).length} planned, ${Object.keys(result.excluded).length} listed exclusions; class members used ${usedMembers}/${classMembers}.`,
  );
  if (problems.length)
    throw new Error(
      `facade-coverage.json:\n${problems.map((p) => `- ${p}`).join("\n")}`,
    );
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
)
  await main();
