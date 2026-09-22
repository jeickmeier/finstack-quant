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
import { fileURLToPath } from "node:url";
import ts from "typescript";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import {
  checkGraph,
  checkImports,
  checkRegistry,
} from "../../scripts/check-registry.mjs";

const entry = (
  name,
  deps = [],
  target = `${name}.ts`,
  type = "registry:lib",
) => ({
  name,
  docs: name,
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
    expect(() => checkGraph([{ ...entry("a"), docs: "" }])).toThrow(/docs/);
    expect(() =>
      checkGraph([{ ...entry("a"), registryDependencies: ["b"] }, entry("b")]),
    ).toThrow(/namespace/);
  });
  it("requires stock imports to be declared as upstream dependencies", () => {
    const a = entry("a");
    const content = new Map([
      ["a.ts", 'export { RadioGroup } from "@/components/ui/radio-group";'],
    ]);
    expect(() => checkImports([a], content)).toThrow(/Unresolved installed/);
    expect(() =>
      checkImports([{ ...a, registryDependencies: ["radio-group"] }], content),
    ).not.toThrow();
    expect(() =>
      checkImports([{ ...a, registryDependencies: ["input"] }], content),
    ).toThrow(/Unresolved installed/);
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
