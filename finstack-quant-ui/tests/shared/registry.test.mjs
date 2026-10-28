import { describe, it, expect } from "vitest";
import { shadcnItems, copyShadcn } from "../../scripts/shadcn.mjs";
import {
  mkdtemp,
  mkdir,
  writeFile,
  readFile,
  rm,
  symlink,
} from "node:fs/promises";
import path from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import ts from "typescript";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import {
  checkGraph,
  checkImports,
  checkRegistry,
  lazyCatalogueTargets,
  workerRoles,
} from "../../scripts/check-registry.mjs";

const entry = (
  name,
  deps = [],
  target = `${name}.ts`,
  type = "registry:lib",
) => ({
  name,
  title: name,
  description: name,
  type,
  registryDependencies: deps.map((name) => `@finstack/${name}`),
  files: [{ target }],
});
describe("registry dependency boundaries", () => {
  it("accepts shared dependencies reached through a diamond", () => {
    expect(
      checkGraph([
        entry("a", ["b", "c"]),
        entry("b", ["d"]),
        entry("c", ["d"]),
        entry("d"),
      ]).closures.get("a").size,
    ).toBe(4);
  });
  it("has no arbitrary graph depth cap", () => {
    expect(
      checkGraph(
        Array.from({ length: 150 }, (_, i) =>
          entry(`n${i}`, i < 149 ? [`n${i + 1}`] : []),
        ),
      ).closures.get("n0").size,
    ).toBe(150);
  });
  it.each([
    [[entry("a", ["b"]), entry("b", ["a"])], /cycle/],
    [[entry("a", ["missing"])], /Unresolved/],
    [[entry("a"), entry("a")], /Duplicate/],
    [[entry("a", [], "same.ts"), entry("b", [], "same.ts")], /Conflicting/],
    [[entry("a", [], "../escape.ts")], /Invalid installed/],
    [[entry("a", ["b"]), entry("b", [], "b.ts", "registry:block")], /tier/],
    [[entry("a", [], "a.ts", "registry:ui")], /missing theme/],
  ])("rejects invalid graph %#", (items, message) =>
    expect(() => checkGraph(items)).toThrow(message),
  );
  it("rejects undocumented and unnamespaced entries", () => {
    expect(() => checkGraph([{ ...entry("a"), description: "" }])).toThrow(
      /description/,
    );
    expect(() =>
      checkGraph([{ ...entry("a"), registryDependencies: ["b"] }, entry("b")]),
    ).toThrow(/namespace/);
  });
  it("requires stock imports to be declared as upstream dependencies", () => {
    const a = entry("a"),
      base = entry("finstack-base");
    const stock = (name) => ({
      ...a,
      registryDependencies: ["@finstack/finstack-base", name],
    });
    const content = new Map([
      ["a.ts", 'export { RadioGroup } from "@/components/ui/radio-group";'],
      ["finstack-base.ts", ""],
    ]);
    expect(() => checkImports([a], content)).toThrow(/Unresolved installed/);
    expect(() =>
      checkImports([stock("radio-group"), base], content),
    ).not.toThrow();
    expect(() => checkImports([stock("input"), base], content)).toThrow(
      /Unresolved installed/,
    );
  });
  it("requires finstack-base wherever a closure reaches stock shadcn items", () => {
    const items = [
      entry("a", ["b"]),
      { ...entry("b"), registryDependencies: ["button"] },
    ];
    expect(() => checkGraph(items)).toThrow(
      "Stock shadcn dependency without finstack-base: b",
    );
    items[1].registryDependencies.push("@finstack/finstack-base");
    expect(
      checkGraph([...items, entry("finstack-base")]).closures.get("a"),
    ).toEqual(new Set(["a", "b", "finstack-base"]));
  });
  it("requires installed imports to belong to the importing item's closure", () => {
    const content = new Map([
      ["a.ts", 'export * from "./b";'],
      ["b.ts", "export const b = 1;"],
    ]);
    expect(() => checkImports([entry("a"), entry("b")], content)).toThrow(
      /Unresolved installed/,
    );
    expect(() =>
      checkImports([entry("a", ["b"]), entry("b")], content),
    ).not.toThrow();
    content.set("a.ts", 'export const load = () => import("./missing");');
    expect(() =>
      checkImports([entry("a", ["b"]), entry("b")], content),
    ).toThrow(/Unresolved installed/);
    content.set("a.ts", 'import x from "not-installed";');
    expect(() => checkImports([entry("a")], content)).toThrow(
      /Undeclared package/,
    );
  });
});

describe("worker roles", () => {
  const item = (name, deps, type, source) => ({
    ...entry(name, deps, `${name}.tsx`, type),
    files: [{ path: `${name}.tsx`, target: `${name}.tsx` }],
    source,
  });
  it("derives host, self and none from the closure and the item's own files", async () => {
    const items = [
      item("finstack-theme", [], "registry:theme", ""),
      item("use-finstack", [], "registry:hook", ""),
      item("plain", ["finstack-theme"], "registry:component", ""),
      item(
        "host",
        ["finstack-theme", "use-finstack"],
        "registry:component",
        "export const Host = () => <p>Host</p>;",
      ),
      item(
        "self",
        ["finstack-theme", "use-finstack"],
        "registry:component",
        'import { FinstackQueryProvider } from "./use-finstack";\nexport const Self = () => <FinstackQueryProvider><p /></FinstackQueryProvider>;',
      ),
      item(
        "closed",
        ["finstack-theme", "use-finstack"],
        "registry:component",
        'import { FinstackQueryProvider } from "./use-finstack";\nexport const Closed = () => <FinstackQueryProvider/>;',
      ),
      item(
        "aliased",
        ["finstack-theme", "use-finstack"],
        "registry:component",
        'import { FinstackQueryProvider as Provider } from "@/use-finstack";\nexport const Aliased = () => <Provider><p /></Provider>;',
      ),
      item(
        "low-level",
        ["finstack-theme", "use-finstack"],
        "registry:component",
        'import { FinstackProvider } from "./use-finstack";\nexport const LowLevel = () => <FinstackProvider><p /></FinstackProvider>;',
      ),
      item(
        "mention",
        ["finstack-theme", "use-finstack"],
        "registry:component",
        [
          'import { FinstackQueryProvider } from "./use-finstack";',
          "/** Render inside <FinstackQueryProvider> first. */",
          "// <FinstackQueryProvider/>",
          'export const label = "<FinstackQueryProvider>";',
          "export type Props = Parameters<typeof FinstackQueryProvider>[0];",
          "export const Mention = () => <p />;",
        ].join("\n"),
      ),
      item(
        "elsewhere",
        ["finstack-theme", "use-finstack", "plain"],
        "registry:component",
        'import { FinstackQueryProvider } from "./plain";\nexport const Elsewhere = () => <FinstackQueryProvider />;',
      ),
    ];
    const cwd = await mkdtemp(path.join(tmpdir(), "worker-roles-"));
    try {
      for (const { name, source } of items)
        await writeFile(path.join(cwd, `${name}.tsx`), source);
      const roles = await workerRoles(cwd, items, checkGraph(items));
      expect(Object.fromEntries(roles)).toEqual({
        "finstack-theme": "none",
        plain: "none",
        host: "host",
        self: "self",
        closed: "self",
        aliased: "self",
        "low-level": "self",
        mention: "host",
        elsewhere: "host",
      });
      expect(roles.has("use-finstack")).toBe(false);
    } finally {
      await rm(cwd, { recursive: true, force: true });
    }
  });
});

describe("lazy catalogue targets", () => {
  const contract = (type) => ({
    ...entry(`contract-${type}`),
    files: ["instrument", "schemas", "examples", "meta"].map((kind) => ({
      target: `lib/finstack/generated/${kind}/${type}.${kind === "instrument" || kind === "meta" ? "ts" : "json"}`,
    })),
  });
  const targets = (type) => [
    `lib/finstack/generated/instrument/${type}.ts`,
    `lib/finstack/generated/schemas/${type}.json`,
    `lib/finstack/generated/examples/${type}.json`,
  ];
  const graph = checkGraph([
    contract("a"),
    contract("b"),
    entry("instrument-catalogue", ["contract-a", "contract-b"]),
    entry("form", ["instrument-catalogue"]),
    entry("params", ["contract-b"]),
    entry("workbench", ["form", "params"]),
    entry("other", ["contract-a"]),
  ]);
  it("keeps every type lazy that the closure reaches only through the catalogue", () => {
    expect(lazyCatalogueTargets(graph, "form").sort()).toEqual(
      [...targets("a"), ...targets("b")].sort(),
    );
    expect(lazyCatalogueTargets(graph, "instrument-catalogue").sort()).toEqual(
      [...targets("a"), ...targets("b")].sort(),
    );
    expect(lazyCatalogueTargets(graph, "workbench").sort()).toEqual(
      targets("a").sort(),
    );
    expect(lazyCatalogueTargets(graph, "other")).toEqual([]);
  });
});

it.each([false, true])(
  "type-checks the installed corpus without workspace aliases (src=%s)",
  async (src) => {
    const root = fileURLToPath(new URL("../../", import.meta.url));
    const registry = await checkRegistry(root);
    const consumer = await mkdtemp(path.join(root, ".consumer-"));
    try {
      await symlink(
        path.join(root, "node_modules"),
        path.join(consumer, "node_modules"),
        "dir",
      );
      await promisify(execFile)(
        process.execPath,
        [
          path.join(root, "node_modules/shadcn/dist/index.js"),
          "build",
          "--output",
          path.join(consumer, "r"),
        ],
        { cwd: root },
      );
      const files = [];
      for (const entry of registry.items) {
        const item = JSON.parse(
          await readFile(
            path.join(consumer, "r", `${entry.name}.json`),
            "utf8",
          ),
        );
        for (const file of item.files ?? []) {
          const target = path.join(
            consumer,
            src && !file.target.startsWith("~/") ? "src" : "",
            file.target.replace(/^~\//, ""),
          );
          await mkdir(path.dirname(target), { recursive: true });
          expect(typeof file.content).toBe("string");
          await writeFile(target, file.content);
          if (/\.(?:tsx?|mts)$/.test(target)) files.push(target);
        }
      }
      files.push(
        ...(await copyShadcn(
          root,
          src ? path.join(consumer, "src") : consumer,
          [...shadcnItems.keys()],
        )),
      );
      const program = ts.createProgram(files, {
        target: ts.ScriptTarget.ES2022,
        jsx: ts.JsxEmit.ReactJSX,
        lib: ["lib.es2024.d.ts", "lib.dom.d.ts", "lib.dom.iterable.d.ts"],
        baseUrl: consumer,
        paths: { "@/*": [src ? "./src/*" : "./*"] },
        module: ts.ModuleKind.ESNext,
        moduleResolution: ts.ModuleResolutionKind.Bundler,
        strict: true,
        resolveJsonModule: true,
        skipLibCheck: true,
        noEmit: true,
      });
      const diagnostics = ts.getPreEmitDiagnostics(program);
      expect(
        diagnostics.map(
          (d) =>
            `${d.file?.fileName}: ${ts.flattenDiagnosticMessageText(d.messageText, "\n")}`,
        ),
      ).toEqual([]);
    } finally {
      await rm(consumer, { recursive: true, force: true });
    }
  },
  60_000,
);
