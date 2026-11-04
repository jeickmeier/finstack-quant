import { mkdtemp, rm } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { build } from "vite";
import type { TestProject } from "vitest/node";

declare module "vitest" {
  export interface ProvidedContext {
    /** The bundled production worker service that `startWorker` spawns. */
    workerBundle: string;
  }
}

/** Vitest global setup: bundles tests/shared/worker/node.mjs once per run. */
export default async function setup(project: TestProject) {
  const root = path.resolve(
    path.dirname(fileURLToPath(import.meta.url)),
    "../../..",
  );
  // Inside the package so the bundle resolves its external dependencies from node_modules.
  const directory = await mkdtemp(path.join(root, ".worker-test-"));
  try {
    await build({
      root,
      configFile: false,
      logLevel: "error",
      resolve: {
        alias: {
          "@/lib/finstack": path.join(root, "src"),
          "@/workers": path.join(root, "registry/shared/file"),
        },
      },
      build: {
        ssr: true,
        target: "node24",
        outDir: directory,
        emptyOutDir: false,
        rolldownOptions: { output: { entryFileNames: "node.mjs" } },
        lib: {
          entry: path.join(root, "tests/shared/worker/node.mjs"),
          formats: ["es"],
          fileName: () => "node.mjs",
        },
      },
    });
  } catch (error) {
    await rm(directory, { recursive: true, force: true });
    throw error;
  }
  project.provide("workerBundle", path.join(directory, "node.mjs"));
  return () => rm(directory, { recursive: true, force: true });
}
