import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";
import { isDeepStrictEqual } from "node:util";
import { itemMetadata, metadataInputs, pinWasm } from "./registry-metadata.mjs";
import { loadRegistry } from "shadcn/registry";
import { shadcnItems, shadcnClosure, checkShadcn } from "./shadcn.mjs";

const tiers = {
  "registry:ui": 1,
  "registry:component": 2,
  "registry:block": 3,
};
/** Item types that render: the tiers plus the theme. */
export const visualTypes = new Set([...Object.keys(tiers), "registry:theme"]);
const packageName = (specifier) =>
  specifier.startsWith("@")
    ? specifier.split("/").slice(0, 2).join("/")
    : specifier.split("/")[0];
export const dependencyName = (dependency) =>
  dependency.replace(/@[^@/]+$/, "");
export function checkGraph(items) {
  const byName = new Map();
  const owners = new Map();
  for (const item of items) {
    if (byName.has(item.name)) throw new Error(`Duplicate item: ${item.name}`);
    byName.set(item.name, item);
    // shadcn search, list and MCP read only these two; `docs` is an optional post-install note.
    for (const field of ["title", "description"])
      if (!item[field]?.trim())
        throw new Error(`Missing ${field}: ${item.name}`);
    for (const file of item.files ?? []) {
      const target = file.target;
      if (
        !target ||
        target.includes("\\") ||
        path.posix.isAbsolute(target) ||
        path.posix.normalize(target) !== target ||
        target.startsWith("../")
      )
        throw new Error(`Invalid installed target: ${target}`);
      if (owners.has(target))
        throw new Error(`Conflicting target owner: ${target}`);
      owners.set(target, item.name);
    }
  }
  const closures = new Map();
  const visiting = new Set();
  function visit(name) {
    if (visiting.has(name)) throw new Error(`Dependency cycle: ${name}`);
    if (closures.has(name)) return closures.get(name);
    const item = byName.get(name);
    if (!item) throw new Error(`Unresolved dependency: ${name}`);
    visiting.add(name);
    const closure = new Set([name]);
    let stock = false;
    for (const reference of item.registryDependencies ?? []) {
      if (shadcnItems.has(reference)) {
        stock = true;
        continue;
      }
      if (!reference.startsWith("@finstack/"))
        throw new Error(
          `Dependency must use @finstack namespace: ${reference}`,
        );
      const dependency = reference.slice("@finstack/".length);
      const target = byName.get(dependency);
      if ((tiers[target?.type] ?? 0) > (tiers[item.type] ?? 0))
        throw new Error(`Invalid tier dependency: ${name} -> ${dependency}`);
      for (const child of visit(dependency)) closure.add(child);
    }
    visiting.delete(name);
    if (tiers[item.type] && !closure.has("finstack-theme"))
      throw new Error(`Visual item missing theme: ${name}`);
    // finstack-base declares the packages stock shadcn sources import (cn,
    // class-variance-authority, lucide-react); a dependency's closure is part of
    // this one, so checking direct stock references covers the whole closure.
    if (stock && !closure.has("finstack-base"))
      throw new Error(`Stock shadcn dependency without finstack-base: ${name}`);
    closures.set(name, closure);
    return closure;
  }
  for (const name of byName.keys()) visit(name);
  return { byName, owners, closures };
}

/**
 * Resolve a relative or `@/` import to the installed target that owns it, as a
 * consumer's bundler would; `undefined` when no registry file is installed there.
 */
export function resolveInstalled(owners, from, specifier) {
  const base = specifier.startsWith("@/")
    ? specifier.slice(2)
    : path.posix.normalize(
        path.posix.join(path.posix.dirname(from), specifier),
      );
  return [
    base,
    ...[".ts", ".tsx", ".mjs", ".js", ".json", "/index.ts", "/index.tsx"].map(
      (ext) => base + ext,
    ),
  ].find((candidate) => owners.has(candidate));
}

export function checkImports(items, contents) {
  const { byName, owners, closures } = checkGraph(items);
  for (const item of items) {
    const closure = closures.get(item.name);
    const stock = shadcnClosure(
      [...closure]
        .flatMap((name) => byName.get(name).registryDependencies ?? [])
        .filter((name) => shadcnItems.has(name)),
    );
    const packages = new Set(
      [...closure]
        .flatMap((name) => [
          ...(byName.get(name).dependencies ?? []),
          ...(byName.get(name).devDependencies ?? []),
        ])
        .map(dependencyName),
    );
    for (const file of item.files ?? []) {
      if (!/\.(?:[cm]?[jt]sx?)$/.test(file.target)) continue;
      const source = contents.get(file.target);
      if (source === undefined)
        throw new Error(`Missing installed content: ${file.target}`);
      const imports = ts.preProcessFile(source, true, true).importedFiles;
      for (const { fileName: specifier } of imports) {
        if (specifier.startsWith("@base-ui/"))
          throw new Error(
            `Compose stock shadcn components instead of raw Base UI: ${file.target}`,
          );
        if (
          specifier.startsWith("@/components/ui/") &&
          stock.has(specifier.slice("@/components/ui/".length))
        )
          continue;
        if (specifier.startsWith(".") || specifier.startsWith("@/")) {
          const target = resolveInstalled(owners, file.target, specifier);
          if (!target || !closure.has(owners.get(target)))
            throw new Error(
              `Unresolved installed import: ${file.target} -> ${specifier}`,
            );
        } else if (!packages.has(packageName(specifier)))
          throw new Error(
            `Undeclared package import: ${file.target} -> ${specifier}`,
          );
      }
    }
  }
}

/**
 * Installed targets that must stay out of an item's first static load: the
 * instrument, schema and example modules of each `instrument-catalogue` type,
 * which the catalogue's loaders import on demand. A type whose contract another
 * member of the closure depends on directly is exempt: pricing-params-form reads
 * the bond schema's `$defs` statically. Empty when the closure lacks the catalogue.
 *
 * @param graph - `checkGraph` result for the catalogue.
 * @param name - Registry item whose closure is checked.
 * @returns Installed target paths, such as `lib/finstack/generated/schemas/bond.json`.
 */
export function lazyCatalogueTargets(graph, name) {
  const closure = graph.closures.get(name);
  if (!closure.has("instrument-catalogue")) return [];
  const lazy = new Set(
    graph.byName.get("instrument-catalogue").registryDependencies,
  );
  for (const member of closure)
    if (member !== "instrument-catalogue")
      for (const reference of graph.byName.get(member).registryDependencies ??
        [])
        lazy.delete(reference);
  return [...lazy].flatMap((reference) =>
    graph.byName
      .get(reference.slice("@finstack/".length))
      .files.map((file) => file.target)
      .filter((target) =>
        /\/generated\/(?:instrument|schemas|examples)\//.test(target),
      ),
  );
}

const providers = new Set(["FinstackQueryProvider", "FinstackProvider"]);
/**
 * Whether `source`, installed at `target`, renders a `FinstackQueryProvider` or
 * `FinstackProvider` element: a JSX opening or self-closing tag whose name is bound
 * by a named import of either provider, under any local alias, from a module the
 * `use-finstack` item installs. Comments, strings and type positions never count.
 */
function rendersProvider(graph, target, source) {
  const file = ts.createSourceFile(target, source, ts.ScriptTarget.Latest);
  const local = new Set();
  for (const statement of file.statements) {
    if (!ts.isImportDeclaration(statement)) continue;
    const specifier = statement.moduleSpecifier.text;
    if (
      !(specifier.startsWith(".") || specifier.startsWith("@/")) ||
      graph.owners.get(resolveInstalled(graph.owners, target, specifier)) !==
        "use-finstack"
    )
      continue;
    for (const element of statement.importClause?.namedBindings?.elements ?? [])
      if (providers.has((element.propertyName ?? element.name).text))
        local.add(element.name.text);
  }
  const visit = (node) =>
    ((ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) &&
      ts.isIdentifier(node.tagName) &&
      local.has(node.tagName.text)) ||
    ts.forEachChild(node, visit);
  return local.size > 0 && Boolean(visit(file));
}

/**
 * Classify each visual item by how its page reaches the Rust worker:
 * - `host`: its closure contains `use-finstack`, so the page must mount
 *   `FinstackQueryProvider` around it;
 * - `self`: one of its own files renders `FinstackQueryProvider` or
 *   `FinstackProvider` imported from `use-finstack` (see `rendersProvider`).
 *   FinstackProvider starts one worker per mount, so the page must not wrap it again;
 * - `none`: it never reaches the worker and must not load WASM.
 * Non-visual items are absent from the map.
 */
export async function workerRoles(cwd, items, graph) {
  const roles = new Map();
  for (const item of items) {
    if (!visualTypes.has(item.type)) continue;
    if (!graph.closures.get(item.name).has("use-finstack")) {
      roles.set(item.name, "none");
      continue;
    }
    let self = false;
    for (const file of item.files ?? [])
      self ||=
        /\.(?:[cm]?[jt]sx?)$/.test(file.target) &&
        rendersProvider(
          graph,
          file.target,
          await readFile(path.resolve(cwd, file.path), "utf8"),
        );
    roles.set(item.name, self ? "self" : "host");
  }
  return roles;
}

export async function checkRegistry(cwd) {
  await checkShadcn(cwd);
  const registry = await loadRegistry({ cwd });
  const contents = new Map();
  const sources = new Set();
  for (const item of registry.items)
    for (const file of item.files ?? []) {
      contents.set(
        file.target,
        await readFile(path.resolve(cwd, file.path), "utf8"),
      );
      if (sources.has(file.path))
        throw new Error(`Conflicting source owner: ${file.path}`);
      sources.add(file.path);
    }
  checkImports(registry.items, contents);
  const graph = checkGraph(registry.items),
    inputs = await metadataInputs(cwd);
  for (const item of registry.items) {
    if (
      !isDeepStrictEqual(
        { categories: item.categories, meta: item.meta },
        itemMetadata(item, graph, inputs),
      )
    )
      throw new Error(`Registry metadata drift: ${item.name}`);
    if (
      !isDeepStrictEqual(
        item.dependencies,
        pinWasm(item.dependencies, inputs.wasmVersion),
      )
    )
      throw new Error(
        `Registry WASM pin drift: ${item.name} must depend on finstack-quant-wasm@${inputs.wasmVersion}`,
      );
  }
  const generated = await readdir(path.join(cwd, "src/generated"), {
    recursive: true,
    withFileTypes: true,
  });
  for (const file of generated)
    if (file.isFile()) {
      const source = path.relative(cwd, path.join(file.parentPath, file.name));
      if (!sources.has(source))
        throw new Error(`Unowned generated file: ${source}`);
    }
  return registry;
}
if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const registry = await checkRegistry(
    fileURLToPath(new URL("../", import.meta.url)),
  );
  console.log(
    `Checked ${registry.items.length} registry items and installed import closures.`,
  );
}
