import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";
import { loadRegistry } from "shadcn/registry";

const tiers = {
  "registry:ui": 1,
  "registry:component": 2,
  "registry:block": 3,
};
const packageName = (specifier) =>
  specifier.startsWith("@")
    ? specifier.split("/").slice(0, 2).join("/")
    : specifier.split("/")[0];
const dependencyName = (dependency) => dependency.replace(/@[^@/]+$/, "");
export function checkGraph(items) {
  const byName = new Map();
  const owners = new Map();
  for (const item of items) {
    if (byName.has(item.name)) throw new Error(`Duplicate item: ${item.name}`);
    byName.set(item.name, item);
    if (!item.docs?.trim()) throw new Error(`Missing docs: ${item.name}`);
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
    for (const reference of item.registryDependencies ?? []) {
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
    closures.set(name, closure);
    return closure;
  }
  for (const name of byName.keys()) visit(name);
  return { byName, owners, closures };
}

export function checkImports(items, contents) {
  const { byName, owners, closures } = checkGraph(items);
  for (const item of items) {
    const closure = closures.get(item.name);
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
        if (specifier.startsWith(".") || specifier.startsWith("@/")) {
          const base = specifier.startsWith("@/")
            ? specifier.slice(2)
            : path.posix.normalize(
                path.posix.join(path.posix.dirname(file.target), specifier),
              );
          const candidates = [
            base,
            ...[
              ".ts",
              ".tsx",
              ".mjs",
              ".js",
              ".json",
              "/index.ts",
              "/index.tsx",
            ].map((ext) => base + ext),
          ];
          const target = candidates.find((candidate) => owners.has(candidate));
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

export async function checkRegistry(cwd) {
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
