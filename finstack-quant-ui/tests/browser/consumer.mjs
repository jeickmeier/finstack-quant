import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
/** Extract a fresh pinned CLI build and its declared closure into consumer-owned targets. */
export async function installBuilt(root, consumer, names) {
  const output = path.join(consumer, "r");
  await promisify(execFile)(
    process.execPath,
    [
      path.join(root, "node_modules/shadcn/dist/index.js"),
      "build",
      "--output",
      output,
    ],
    { cwd: root },
  );
  const installed = new Set();
  async function install(name) {
    if (installed.has(name)) return;
    const item = JSON.parse(
      await readFile(path.join(output, `${name}.json`), "utf8"),
    );
    installed.add(name);
    for (const dependency of item.registryDependencies ?? [])
      await install(dependency.replace("@finstack/", ""));
    for (const file of item.files ?? []) {
      const target = path.join(consumer, file.target.replace(/^~\//, ""));
      await mkdir(path.dirname(target), { recursive: true });
      await writeFile(target, file.content);
    }
  }
  for (const name of names) await install(name);
  return [...installed];
}
