import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import prettier from "prettier";
import { schemaAt } from "../src/schema.mjs";
const root = fileURLToPath(new URL("../", import.meta.url));
const read = async (name) =>
  JSON.parse(await readFile(path.join(root, name), "utf8"));
const roots = (await read("src/generated/roots.json")).filter((item) =>
  item.uri.includes("/instrument/"),
);
const fixtures = await read("src/generated/fixtures.json");
function resolve(root, node) {
  if (!node.$ref) return node;
  const found = schemaAt(root, node.$ref);
  const { $ref, ...rest } = node;
  return { ...found, ...rest };
}
const records = [];
for (const entry of roots) {
  const schema = await read(`src/generated/${entry.schema}`);
  const example = await read(
    `src/generated/examples/${path.basename(entry.schema)}`,
  );
  const metaText = await readFile(
    path.join(
      root,
      "src/generated/meta",
      path.basename(entry.schema, ".json") + ".ts",
    ),
    "utf8",
  );
  const metadata = new Map(
    JSON.parse(
      metaText
        .slice(metaText.indexOf("export default ") + 15)
        .trim()
        .replace(/;$/, ""),
    ).map((meta) => [meta.path, meta]),
  );
  const constructs = [],
    visited = new Set();
  function visit(raw, pointer, ancestors = []) {
    if (!raw || typeof raw !== "object") return;
    if (raw.$ref) {
      if (ancestors.includes(raw.$ref)) {
        constructs.push({
          kind: "recursive-reference",
          pointer,
          ref: raw.$ref,
        });
        return;
      }
      if (visited.has(raw.$ref)) return;
      visited.add(raw.$ref);
      ancestors = [...ancestors, raw.$ref];
      pointer = raw.$ref;
    }
    const node = resolve(schema, raw),
      type = Array.isArray(node.type)
        ? node.type.filter((value) => value !== "null")[0]
        : node.type;
    const add = (kind, extra = {}) =>
      constructs.push({ kind, pointer, ...extra });
    if (Array.isArray(node.type) && node.type.includes("null"))
      add(`nullable-${type}`);
    if (node.enum) add("enum", { values: node.enum });
    if (
      type === "string" &&
      (node.title === "Decimal" || node.pattern?.includes("\\d"))
    )
      add("decimal-or-pattern-string");
    if (node.format) add(node.format);
    if (/\/(?:[a-z_]+_)?calendar_id$/.test(pointer)) add("calendar-id");
    const source = metadata.get(pointer)?.source ?? "";
    for (const contract of [
      "money",
      "decimal",
      "tenor",
      "day_count",
      "calendar",
      "currency",
      "business_day_convention",
    ])
      if (
        source.includes(`/${contract}/`) ||
        source.includes(`/${contract}.schema`)
      )
        add(contract);
    if (type === "object") {
      if (typeof node.additionalProperties === "object") add("map");
      if (
        node.properties &&
        node.additionalProperties === false &&
        !node.required?.length
      )
        add("optional-property-record");
    }
    if (type === "array")
      add(
        node.prefixItems
          ? "tuple"
          : resolve(schema, node.items ?? {}).type === "object"
            ? "object-array"
            : "array",
      );
    for (const keyword of ["oneOf", "anyOf", "allOf"]) {
      if (!node[keyword]) continue;
      const branches = node[keyword].map((branch) => resolve(schema, branch));
      const tag = branches.map(
        (branch) =>
          Object.entries(branch.properties ?? {}).find(
            ([, value]) =>
              Object.hasOwn(value, "const") || value.enum?.length === 1,
          )?.[0],
      );
      const contentKeys = branches.map((branch, index) =>
        Object.keys(branch.properties ?? {}).filter(
          (key) => key !== tag[index],
        ),
      );
      const adjacent =
        tag.every(Boolean) &&
        contentKeys.every((keys) => keys.length === 1) &&
        new Set(contentKeys.map((keys) => keys[0])).size === 1;
      const external = (branch) =>
        Object.keys(branch.properties ?? {}).length === 1;
      const unit = (branch) => Object.hasOwn(branch, "const") || branch.enum;
      const kind = branches.some((branch) => branch.type === "null")
        ? "nullable-union"
        : tag.every(Boolean)
          ? adjacent
            ? "adjacent-union"
            : "internal-union"
          : branches.every(
                (branch) => Object.hasOwn(branch, "const") || branch.enum,
              )
            ? "unit-union"
            : branches.every(
                  (branch) => Object.keys(branch.properties ?? {}).length === 1,
                )
              ? "external-union"
              : branches.every((branch) => unit(branch) || external(branch))
                ? "mixed-unit-external-union"
                : "union";
      add(kind, {
        keyword,
        ...(tag.every(Boolean) ? { discriminators: [...new Set(tag)] } : {}),
      });
      node[keyword].forEach((branch, index) =>
        visit(branch, `${pointer}/${keyword}/${index}`, ancestors),
      );
    }
    for (const [key, value] of Object.entries(node.properties ?? {}))
      visit(value, `${pointer}/properties/${key}`, ancestors);
    for (const [index, value] of (node.prefixItems ?? []).entries())
      visit(value, `${pointer}/prefixItems/${index}`, ancestors);
    for (const keyword of ["items", "additionalProperties"])
      visit(node[keyword], `${pointer}/${keyword}`, ancestors);
  }
  visit(schema, "#");
  records.push({
    type: example.instrument.type,
    schema: entry.uri,
    fixtures: fixtures
      .filter((f) => f.kind === "instrument" && f.schema === entry.uri)
      .map((f) => f.source),
    constructs: Object.fromEntries(
      [...new Set(constructs.map((item) => item.kind))]
        .sort()
        .map((kind) => [
          kind,
          constructs
            .filter((item) => item.kind === kind)
            .map(({ kind, pointer, ...details }) =>
              Object.keys(details).length ? { pointer, ...details } : pointer,
            ),
        ]),
    ),
  });
}
records.sort((a, b) => a.type.localeCompare(b.type));
const output = await prettier.format(
  JSON.stringify({ instrumentCount: records.length, instruments: records }),
  { parser: "json" },
);
const target = path.join(root, "tests/instruments/encoding-report.json");
if (process.argv.includes("--check")) {
  if ((await readFile(target, "utf8")) !== output)
    throw new Error("Encoding report is stale");
} else await writeFile(target, output);
console.log(`Encoding report: ${records.length} instrument tags`);
