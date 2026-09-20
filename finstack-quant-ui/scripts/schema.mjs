import { createHash } from "node:crypto";
import { glob, readFile } from "node:fs/promises";
import { dirname, relative, resolve } from "node:path";
import $RefParser from "@apidevtools/json-schema-ref-parser";

import {
  mapChildren,
  maps,
  arrays,
  singles,
  annotations,
} from "../src/schema.mjs";

export const json = (value) => `${JSON.stringify(value, null, 2)}\n`;
const supported = new Set([
  ...maps,
  ...arrays,
  ...singles,
  ...annotations,
  "$id",
  "$schema",
  "$ref",
  "type",
  "const",
  "enum",
  "required",
  "format",
  "minimum",
  "maximum",
  "exclusiveMinimum",
  "exclusiveMaximum",
  "multipleOf",
  "minLength",
  "maxLength",
  "pattern",
  "minItems",
  "maxItems",
  "uniqueItems",
  "unevaluatedProperties",
]);

/** Index checked-in contracts by canonical URI. Network/file fallback is never enabled. */
export async function readContracts(repo) {
  const contracts = new Map();
  const indexes = [];
  for await (const path of glob("finstack-quant/*/schemas/index.json", {
    cwd: repo,
  }))
    indexes.push(path);
  for (const path of indexes.sort()) {
    const index = JSON.parse(await readFile(resolve(repo, path), "utf8"));
    if (index.schema_index_version !== 1)
      throw new Error(`Unsupported schema index: ${path}`);
    for (const artifact of index.artifacts) {
      const source = relative(
        repo,
        resolve(repo, dirname(path), "..", artifact.path),
      );
      const schema = JSON.parse(await readFile(resolve(repo, source), "utf8"));
      if (schema.$id !== artifact.$id)
        throw new Error(`Schema identity mismatch: ${source}`);
      const prior = contracts.get(schema.$id);
      if (prior && json(prior.schema) !== json(schema))
        throw new Error(`Conflicting schema URI: ${schema.$id}`);
      if (!prior) contracts.set(schema.$id, { ...artifact, source, schema });
    }
  }
  return contracts;
}

/** Resolve with Ref Parser, then project only reachable schema nodes into local $defs. */
export async function bundleRoot(uri, contracts) {
  if (!contracts.has(uri)) throw new Error(`Unknown root: ${uri}`);
  const parser = new $RefParser();
  await parser.resolve(uri, {
    resolve: {
      file: false,
      http: false,
      canonical: {
        order: 1,
        canRead: () => true,
        read: ({ url }) => {
          const contract = contracts.get(url);
          if (!contract) throw new Error(`Unresolved canonical schema: ${url}`);
          return json(resolutionSchema(contract.schema));
        },
      },
    },
  });
  const definitions = {};
  const identities = new Map();
  const metadata = [];
  const rootIdentity = `${uri}#`;

  function localRef(identity) {
    if (identity === rootIdentity) return "#";
    const key = `d_${createHash("sha256").update(identity).digest("hex").slice(0, 20)}`;
    if (identities.has(key) && identities.get(key) !== identity)
      throw new Error(`Definition hash collision: ${identity}`);
    if (!identities.has(key)) {
      identities.set(key, identity);
      definitions[key] = null; // Reserve before visiting recursive targets.
      parser.$refs.get(identity); // Verify resolution through the maintained resolver.
      const [document, fragment] = identity.split("#");
      const original = fragment
        ? decodeURIComponent(fragment)
            .slice(1)
            .split("/")
            .reduce(
              (value, part) =>
                value[part.replaceAll("~1", "/").replaceAll("~0", "~")],
              contracts.get(document).schema,
            )
        : contracts.get(document).schema;
      definitions[key] = visit(original, identity, `#/$defs/${key}`);
    }
    return `#/$defs/${key}`;
  }

  function visit(schema, identity, path) {
    if (typeof schema === "boolean") return schema;
    const { $defs, $id, ...body } = schema;
    if ($id && $id !== identity.split("#")[0])
      throw new Error(`Nested schema identity is unsupported: ${identity}`);
    for (const key of Object.keys(body)) {
      if (!supported.has(key) && !key.startsWith("x-"))
        throw new Error(`Unsupported keyword ${key} at ${identity}`);
    }
    const meta = { path, source: identity };
    for (const [key, value] of Object.entries(schema)) {
      if (
        annotations.has(key) ||
        [
          "format",
          "enum",
          "const",
          "minimum",
          "maximum",
          "exclusiveMinimum",
          "exclusiveMaximum",
          "pattern",
        ].includes(key) ||
        key.startsWith("x-")
      )
        meta[key] = value;
    }
    if (schema.$ref) {
      meta.ref = schema.$ref;
      const absolute = new URL(schema.$ref, identity.split("#")[0]).href;
      meta.resolvedRef = absolute.includes("#") ? absolute : `${absolute}#`;
      body.$ref = localRef(meta.resolvedRef);
    }
    metadata.push(meta);
    return mapChildren(body, (child, suffix) =>
      visit(child, `${identity}/${suffix}`, `${path}/${suffix}`),
    );
  }

  const schema = visit(contracts.get(uri).schema, rootIdentity, "#");
  schema.$id = uri;
  schema.$defs = Object.fromEntries(
    Object.entries(definitions).sort(([a], [b]) => a.localeCompare(b)),
  );
  metadata.sort((a, b) => a.path.localeCompare(b.path));
  return { schema, metadata };
}

// Resolve schema positions only: Ref Parser also accepts arbitrary JSON, and
// would otherwise treat literal $ref keys inside defaults/examples as links.
function resolutionSchema(schema) {
  if (typeof schema === "boolean") return schema;
  return mapChildren(
    Object.fromEntries(
      Object.entries(schema).filter(
        ([key]) =>
          !annotations.has(key) &&
          !key.startsWith("x-") &&
          key !== "const" &&
          key !== "enum",
      ),
    ),
    resolutionSchema,
  );
}
