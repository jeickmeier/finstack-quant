import { expect, it } from "vitest";
import {
  mkdtemp,
  mkdir,
  readFile,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import {
  registryOutputs,
  applyRegistryOutputs,
} from "../../scripts/gen-registry.mjs";

const inputs = {
  version: "1.2.3",
  wasmVersion: "4.5.6",
  roots: [{ schema: "schemas/new.json", uri: "https://example.test/new" }],
  provenance: { entries: [] },
};
const item = (name, registryDependencies = [], extra = {}) => ({
  name,
  type: "registry:lib",
  title: name,
  description: `Fixture ${name}`,
  files: [],
  registryDependencies,
  ...extra,
});
const registry = (items, include = []) => ({
  name: "fixture",
  homepage: "https://example.test",
  items,
  include,
});
async function fixture(run) {
  const cwd = await mkdtemp(
    path.join(tmpdir(), "finstack-registry-generation-"),
  );
  try {
    await mkdir(path.join(cwd, "parts/nested"), { recursive: true });
    await run(cwd);
  } finally {
    await rm(cwd, { recursive: true, force: true });
  }
}
const json = (cwd, file, value) =>
  writeFile(path.join(cwd, file), JSON.stringify(value));

it("overlays additions and removals before deriving nested metadata and WASM pins", async () => {
  await fixture(async (cwd) => {
    await json(cwd, "registry.json", registry([item("obsolete")]));
    await json(cwd, "parts/registry.json", {
      items: [
        item("consumer", ["@finstack/contract-new"], {
          dependencies: ["finstack-quant-wasm@0.0.1", "react@19.2.8"],
        }),
      ],
      include: ["nested/registry.json"],
    });
    await json(cwd, "parts/nested/registry.json", {
      items: [item("nested", ["@finstack/consumer"])],
    });
    const proposed = new Map([
      [
        "registry.json",
        registry(
          [
            item("contract-new", [], {
              files: [
                {
                  path: "new.json",
                  type: "registry:file",
                  target: "lib/finstack/generated/schemas/new.json",
                },
              ],
            }),
          ],
          ["parts/registry.json"],
        ),
      ],
    ]);
    const outputs = await registryOutputs(cwd, proposed, inputs);
    expect([...outputs.keys()]).toEqual([
      "registry.json",
      "parts/registry.json",
      "parts/nested/registry.json",
    ]);
    expect(
      JSON.parse(outputs.get("registry.json")).items.map((entry) => entry.name),
    ).toEqual(["contract-new"]);
    const child = JSON.parse(outputs.get("parts/registry.json"));
    expect(child).not.toHaveProperty("name");
    expect(child).not.toHaveProperty("homepage");
    expect(child.items[0].dependencies).toEqual([
      "finstack-quant-wasm@4.5.6",
      "react@19.2.8",
    ]);
    expect(child.items[0].meta).toEqual({
      version: "1.2.3",
      wasmVersion: "4.5.6",
      schemaIds: ["https://example.test/new"],
    });
    expect(
      JSON.parse(outputs.get("parts/nested/registry.json")).items[0].meta
        .schemaIds,
    ).toEqual(["https://example.test/new"]);
    await applyRegistryOutputs(cwd, outputs);
    const repeated = await registryOutputs(cwd, proposed, inputs);
    expect(repeated).toEqual(outputs);
    const before = await Promise.all(
      [...outputs.keys()].map(async (file) => [
        await readFile(path.join(cwd, file), "utf8"),
        (await stat(path.join(cwd, file))).mtimeMs,
      ]),
    );
    await applyRegistryOutputs(cwd, repeated, true);
    const after = await Promise.all(
      [...outputs.keys()].map(async (file) => [
        await readFile(path.join(cwd, file), "utf8"),
        (await stat(path.join(cwd, file))).mtimeMs,
      ]),
    );
    expect(after).toEqual(before);
  });
});
it("detects stale or missing final artifacts without writing", async () => {
  await fixture(async (cwd) => {
    const proposed = new Map([["registry.json", registry([item("leaf")])]]);
    const outputs = await registryOutputs(cwd, proposed, inputs);
    await expect(applyRegistryOutputs(cwd, outputs, true)).rejects.toThrow(
      "Generated registry drift: registry.json",
    );
    await expect(
      readFile(path.join(cwd, "registry.json")),
    ).rejects.toMatchObject({ code: "ENOENT" });
    await applyRegistryOutputs(cwd, outputs);
    const stale = JSON.parse(outputs.get("registry.json"));
    stale.items[0].meta.wasmVersion = "0.0.1";
    await json(cwd, "registry.json", stale);
    const before = await readFile(path.join(cwd, "registry.json"), "utf8");
    await expect(applyRegistryOutputs(cwd, outputs, true)).rejects.toThrow(
      "Generated registry drift",
    );
    expect(await readFile(path.join(cwd, "registry.json"), "utf8")).toBe(
      before,
    );
  });
});
it("rejects missing dependencies and dependency cycles before writing", async () => {
  await fixture(async (cwd) => {
    const proposed = (items) => new Map([["registry.json", registry(items)]]);
    await expect(
      registryOutputs(
        cwd,
        proposed([item("a", ["@finstack/missing"])]),
        inputs,
      ),
    ).rejects.toThrow("Unresolved dependency: missing");
    await expect(
      registryOutputs(
        cwd,
        proposed([item("a", ["@finstack/b"]), item("b", ["@finstack/a"])]),
        inputs,
      ),
    ).rejects.toThrow("Dependency cycle");
    await expect(
      readFile(path.join(cwd, "registry.json")),
    ).rejects.toMatchObject({ code: "ENOENT" });
  });
});
it("rejects include cycles, repeated includes and paths outside the package", async () => {
  await fixture(async (cwd) => {
    await json(cwd, "parts/registry.json", {
      items: [],
      include: ["../registry.json"],
    });
    await expect(
      registryOutputs(
        cwd,
        new Map([["registry.json", registry([], ["parts/registry.json"])]]),
        inputs,
      ),
    ).rejects.toThrow("include cycle");
    await json(cwd, "parts/registry.json", { items: [] });
    await expect(
      registryOutputs(
        cwd,
        new Map([
          [
            "registry.json",
            registry([], ["parts/registry.json", "parts/registry.json"]),
          ],
        ]),
        inputs,
      ),
    ).rejects.toThrow("included more than once");
    await expect(
      registryOutputs(
        cwd,
        new Map([["registry.json", registry([], ["../registry.json"])]]),
        inputs,
      ),
    ).rejects.toThrow("escapes package");
  });
});
it("validates proposed manifests with the shadcn schema before writing", async () => {
  await fixture(async (cwd) => {
    const proposed = new Map([
      [
        "registry.json",
        registry([item("bad", [], { type: "registry:unknown" })]),
      ],
    ]);
    await expect(registryOutputs(cwd, proposed, inputs)).rejects.toThrow();
    await expect(
      readFile(path.join(cwd, "registry.json")),
    ).rejects.toMatchObject({ code: "ENOENT" });
  });
});
