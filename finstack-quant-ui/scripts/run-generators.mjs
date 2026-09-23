import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const scripts = [
  "gen.mjs",
  "gen-registry.mjs",
  "gen-theme.mjs",
  "encoding-report.mjs",
  "catalogue-docs.mjs",
  "gen-metadata.mjs",
  "gen-gallery.mjs",
];
const check = process.argv.includes("--check") ? ["--check"] : [];
const root = fileURLToPath(new URL("..", import.meta.url));
for (const script of scripts) {
  const result = spawnSync(
    process.execPath,
    [fileURLToPath(new URL(script, import.meta.url)), ...check],
    { stdio: "inherit", cwd: root },
  );
  if (result.status !== 0) process.exit(result.status ?? 1);
}
