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
  schemaAt,
  sharedDefsId,
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

/**
 * Resolve with Ref Parser, then project only reachable schema nodes into local $defs.
 * `schema` replaces the indexed root document. `stableDefinitionNames` keeps
 * `#/$defs/<name>` keys for definitions owned by `sharedDefsId`. References to
 * that document stay external instead of being copied into the root.
 */
export async function bundleRoot(uri, contracts, options = {}) {
  const sourceSchema = options.schema ?? contracts.get(uri)?.schema;
  if (!sourceSchema) throw new Error(`Unknown root: ${uri}`);
  const parser = new $RefParser();
  await parser.resolve(uri, {
    resolve: {
      file: false,
      http: false,
      canonical: {
        order: 1,
        canRead: () => true,
        read: ({ url }) => {
          const document = String(url).split("#")[0];
          if (url === uri || document === uri)
            return json(resolutionSchema(sourceSchema));
          if (document === sharedDefsId) return "{}";
          const contract = contracts.get(url) ?? contracts.get(document);
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

  function definitionKey(identity) {
    const prefix = `${sharedDefsId}#/$defs/`;
    if (options.stableDefinitionNames && identity.startsWith(prefix)) {
      const name = identity.slice(prefix.length);
      if (!name.includes("/")) return name;
    }
    return `d_${createHash("sha256").update(identity).digest("hex").slice(0, 20)}`;
  }
  function localRef(identity) {
    if (identity === rootIdentity) return "#";
    const key = definitionKey(identity);
    if (identities.has(key) && identities.get(key) !== identity)
      throw new Error(`Definition hash collision: ${identity}`);
    if (!identities.has(key)) {
      identities.set(key, identity);
      definitions[key] = null; // Reserve before visiting recursive targets.
      parser.$refs.get(identity); // Verify resolution through the maintained resolver.
      const [document, fragment] = identity.split("#");
      const documentSchema =
        document === uri ? sourceSchema : contracts.get(document)?.schema;
      if (!documentSchema)
        throw new Error(`Unresolved canonical schema: ${document}`);
      const original = schemaAt(
        documentSchema,
        fragment ? `#${decodeURIComponent(fragment)}` : "#",
      );
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
      const externalShared =
        uri !== sharedDefsId &&
        meta.resolvedRef.startsWith(`${sharedDefsId}#/$defs/`);
      body.$ref = externalShared
        ? meta.resolvedRef
        : localRef(meta.resolvedRef);
    }
    metadata.push(meta);
    return mapChildren(body, (child, suffix) =>
      visit(child, `${identity}/${suffix}`, `${path}/${suffix}`),
    );
  }

  const schema = visit(sourceSchema, rootIdentity, "#");
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
