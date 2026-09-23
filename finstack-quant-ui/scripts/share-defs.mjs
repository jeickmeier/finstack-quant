import { sharedDefsId } from "../src/schema.mjs";

const wideDefCount = 80;

function canonical(value) {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value && typeof value === "object")
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`)
      .join(",")}}`;
  return JSON.stringify(value);
}
const dataKeys = new Set([
  "default",
  "examples",
  "const",
  "enum",
  "description",
  "title",
]);

/** Definitions repeated by wide instrument schemas, keyed by their source name. */
export function planSharedDefs(schemas) {
  const wide = schemas.filter(
    (schema) => Object.keys(schema.$defs ?? {}).length > wideDefCount,
  );
  const bodies = new Map();
  for (const schema of schemas) {
    const isWide = Object.keys(schema.$defs ?? {}).length > wideDefCount;
    for (const [name, body] of Object.entries(schema.$defs ?? {})) {
      const text = canonical(body);
      const prior = bodies.get(name);
      if (!prior) bodies.set(name, { text, body, count: 1, wide: isWide });
      else if (prior.text !== text) prior.conflict = true;
      else {
        prior.count += 1;
        prior.wide ||= isWide;
      }
    }
  }
  const names = new Set(
    [...bodies]
      .filter(([, info]) => info.count > 1 && info.wide && !info.conflict)
      .map(([name]) => name),
  );
  let removed = true;
  while (removed) {
    removed = false;
    for (const name of names) {
      const missing = localRefNames(bodies.get(name).body).filter(
        (target) => !names.has(target),
      );
      if (missing.length) {
        names.delete(name);
        removed = true;
      }
    }
  }
  const ordered = [...names].sort();
  const shared = {
    $id: sharedDefsId,
    $schema: "https://json-schema.org/draft/2020-12/schema",
    title: "SharedDefs",
    properties: Object.fromEntries(
      ordered.map((name) => [name, { $ref: `#/$defs/${name}` }]),
    ),
    $defs: Object.fromEntries(
      ordered.map((name) => [name, structuredClone(bodies.get(name).body)]),
    ),
  };
  return { names, schema: names.size ? shared : null };
}

/** Drop pooled definitions and point the remaining document at the shared id. */
export function externalize(schema, names) {
  if (!names.size) return schema;
  const prefix = `${sharedDefsId}#/$defs/`;
  const rewrite = (node) => {
    if (typeof node !== "object" || node === null) return node;
    if (Array.isArray(node)) return node.map(rewrite);
    const next = {};
    for (const [key, value] of Object.entries(node)) {
      if (
        key === "$ref" &&
        typeof value === "string" &&
        value.startsWith("#/$defs/")
      ) {
        const rest = value.slice("#/$defs/".length);
        const name = rest.split("/")[0];
        next[key] = names.has(name)
          ? `${prefix}${name}${rest.slice(name.length)}`
          : value;
      } else next[key] = dataKeys.has(key) ? value : rewrite(value);
    }
    return next;
  };
  const rewritten = rewrite(schema);
  if (rewritten.$defs) {
    rewritten.$defs = Object.fromEntries(
      Object.entries(rewritten.$defs).filter(([name]) => !names.has(name)),
    );
    if (!Object.keys(rewritten.$defs).length) delete rewritten.$defs;
  }
  return rewritten;
}

export function sharedRefNames(schema) {
  const prefix = `${sharedDefsId}#/$defs/`;
  const names = new Set();
  const visit = (node) => {
    if (typeof node !== "object" || node === null) return;
    if (Array.isArray(node)) {
      node.forEach(visit);
      return;
    }
    if (typeof node.$ref === "string" && node.$ref.startsWith(prefix)) {
      const name = node.$ref.slice(prefix.length).split("/")[0];
      if (name) names.add(name);
    }
    for (const [key, value] of Object.entries(node)) {
      if (key === "$ref" || dataKeys.has(key)) continue;
      visit(value);
    }
  };
  visit(schema);
  return [...names].sort();
}

function localRefNames(node, names = []) {
  if (typeof node !== "object" || node === null) return names;
  if (Array.isArray(node)) {
    for (const child of node) localRefNames(child, names);
    return names;
  }
  if (typeof node.$ref === "string" && node.$ref.startsWith("#/$defs/")) {
    const name = node.$ref.slice("#/$defs/".length).split("/")[0];
    if (name) names.push(name);
  }
  for (const [key, value] of Object.entries(node)) {
    if (!dataKeys.has(key)) localRefNames(value, names);
  }
  return names;
}
