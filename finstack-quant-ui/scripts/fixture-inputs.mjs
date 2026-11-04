import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { sha256 } from "./fixture-output.mjs";

/** The repository root; fixture source paths are relative to it. */
const repository = new URL("../../", import.meta.url);

/** The Node WASM package (finstack-quant-wasm/pkg-node) every fixture generator calls. */
export const native = createRequire(import.meta.url)(
  "../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);

/** A repository file's text, by repository-relative path. */
export const readRepoText = (path) =>
  readFile(new URL(path, repository), "utf8");

/** A repository JSON file, parsed, by repository-relative path. */
export const readRepoJson = async (path) =>
  JSON.parse(await readRepoText(path));

/** `{ path, sha256 }` of a repository file, the record `emit --check` verifies. */
export const sourceRecord = async (path) => ({
  path,
  sha256: sha256(await readFile(new URL(path, repository))),
});

/** A repository JSON file, parsed, with its source record. */
export const readRepoSource = async (path) => ({
  value: await readRepoJson(path),
  source: await sourceRecord(path),
});

/** An instrument envelope with `spec` fields replaced. */
export const withSpec = (envelope, spec) => ({
  ...envelope,
  instrument: {
    ...envelope.instrument,
    spec: { ...envelope.instrument.spec, ...spec },
  },
});
