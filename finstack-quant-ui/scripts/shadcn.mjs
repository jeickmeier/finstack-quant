import { readFileSync } from "node:fs";
import { readFile, mkdir, copyFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import path from "node:path";

const manifest = JSON.parse(
  readFileSync(new URL("../components/shadcn.json", import.meta.url), "utf8"),
);
export const shadcnItems = new Map(
  manifest.items.map((item) => [item.name, item]),
);
export function shadcnClosure(names) {
  const result = new Set();
  function visit(name) {
    if (result.has(name)) return;
    const item = shadcnItems.get(name);
    if (!item) throw Error(`Unknown shadcn component: ${name}`);
    result.add(name);
    for (const dependency of item.registryDependencies ?? []) visit(dependency);
  }
  for (const name of names) visit(name);
  return result;
}
export async function checkShadcn(root) {
  for (const file of [
    ...manifest.items.flatMap((item) => item.files),
    ...(manifest.utils ? [manifest.utils] : []),
  ]) {
    const bytes = await readFile(path.join(root, file.path));
    const actual = createHash("sha256").update(bytes).digest("hex");
    if (actual !== file.sha256)
      throw Error(`Modified stock shadcn source: ${file.path}`);
  }
}
/** Offline unit/browser fixtures use the same verified CLI output as the docs host. */
export async function copyShadcn(root, destination, names) {
  const files = [...shadcnClosure(names)].flatMap(
    (name) => shadcnItems.get(name).files,
  );
  if (files.length && manifest.utils) files.push(manifest.utils);
  for (const file of files) {
    const target = path.join(destination, file.path);
    await mkdir(path.dirname(target), { recursive: true });
    await copyFile(path.join(root, file.path), target);
  }
  return files.map((file) => path.join(destination, file.path));
}
