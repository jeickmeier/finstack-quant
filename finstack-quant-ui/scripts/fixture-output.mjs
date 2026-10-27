import { readFile, writeFile } from "node:fs/promises";
import { relative } from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
import prettier from "prettier";

/** `--check` compares a fresh regeneration with the committed fixture instead of writing. */
export const checking = process.argv.includes("--check");

const packageRoot = fileURLToPath(new URL("..", import.meta.url));
const generator = relative(packageRoot, process.argv[1] ?? "");
const toPath = (target) =>
  target instanceof URL ? fileURLToPath(target) : target;

/**
 * Write a JSON fixture formatted like the rest of the package, or under
 * `--check` compare the committed file with this regeneration, ignoring
 * formatting and key order, and name the first differing value and the
 * generator to rerun.
 */
export async function emit(target, text) {
  const file = toPath(target);
  const name = relative(packageRoot, file);
  // Round-trip through JSON.stringify so the compared value is what write
  // mode stores (native text may carry -0.0, which stringifies as 0).
  const fresh = JSON.parse(JSON.stringify(JSON.parse(text)));
  if (!checking) {
    // Expanded JSON keeps one value per line in fixture diffs; prettier then
    // applies the package style (short arrays collapse) so `prettier --check` is clean.
    await writeFile(
      file,
      await prettier.format(JSON.stringify(fresh, null, 2), {
        parser: "json",
        filepath: file,
      }),
    );
    return;
  }
  let committed;
  try {
    committed = JSON.parse(await readFile(file, "utf8"));
  } catch (error) {
    throw new Error(
      `${name} is missing or not JSON (${error.message}); run \`mise run ui-fixtures\` (${generator}).`,
    );
  }
  const first = firstDifference(committed, fresh);
  if (first === null) return;
  throw new Error(
    `${name} drifted from a fresh regeneration at ${first}; review the native change, then run \`mise run ui-fixtures\` (${generator}) in the same commit.`,
  );
}

function firstDifference(committed, fresh, path = "$") {
  if (isDeepStrictEqual(committed, fresh)) return null;
  const objects =
    committed &&
    fresh &&
    typeof committed === "object" &&
    typeof fresh === "object" &&
    Array.isArray(committed) === Array.isArray(fresh);
  if (!objects) return path;
  for (const key of new Set([
    ...Object.keys(committed),
    ...Object.keys(fresh),
  ])) {
    const child = firstDifference(
      committed[key],
      fresh[key],
      Array.isArray(committed) ? `${path}[${key}]` : `${path}.${key}`,
    );
    if (child !== null) return child;
  }
  return path;
}
