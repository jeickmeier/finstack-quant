import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";
import { spawn } from "node:child_process";
const base = new URL(process.argv[2]);
assert(["https:", "http:"].includes(base.protocol));
if (!base.pathname.endsWith("/")) base.pathname += "/";
const built =
  process.argv[3] ?? fileURLToPath(new URL("../public/r/", import.meta.url));
const evidence = fileURLToPath(
  new URL("../test-results/public/", import.meta.url),
);
await mkdir(evidence, { recursive: true });
const consumer = await mkdtemp(
  path.join(tmpdir(), "finstack-public-consumer-"),
);
const config = {
  $schema: "https://ui.shadcn.com/schema.json",
  style: "base-nova",
  rsc: true,
  tsx: true,
  tailwind: {
    config: "",
    css: "globals.css",
    baseColor: "neutral",
    cssVariables: true,
  },
  aliases: {
    components: "@/components",
    ui: "@/components/ui",
    lib: "@/lib",
    utils: "@/lib/utils",
    hooks: "@/hooks",
  },
  registries: {
    "@finstack": new URL("r/{name}.json", base).href.replace(
      "%7Bname%7D",
      "{name}",
    ),
  },
};
await writeFile(
  path.join(consumer, "components.json"),
  JSON.stringify(config, null, 2),
);
await writeFile(
  path.join(consumer, "package.json"),
  JSON.stringify({ private: true, type: "module" }),
);
await writeFile(
  path.join(consumer, "tsconfig.json"),
  JSON.stringify({
    compilerOptions: { baseUrl: ".", paths: { "@/*": ["./*"] } },
  }),
);
await writeFile(path.join(consumer, "globals.css"), "@import 'tailwindcss';\n");
const commands = [];
async function run(args, filename) {
  const parts = [];
  const code = await new Promise((resolve, reject) => {
    const child = spawn(args[0], args.slice(1), {
      cwd: consumer,
      env: process.env,
    });
    child.stdout.on("data", (chunk) => parts.push(chunk));
    child.stderr.on("data", (chunk) => parts.push(chunk));
    child.once("error", reject);
    child.once("exit", resolve);
  });
  const output = Buffer.concat(parts).toString();
  await writeFile(path.join(evidence, filename), output);
  commands.push({ command: args.join(" "), exitCode: code });
  assert.equal(code, 0, output.slice(-2000));
  return output;
}
const registryResponse = await fetch(new URL("r/registry.json", base));
assert.equal(registryResponse.status, 200);
const registry = await registryResponse.json();
assert.deepEqual(
  registry,
  JSON.parse(await readFile(path.join(built, "registry.json"), "utf8")),
  "Published index differs from the verified deployment artifact",
);
assert(registry.items.length > 0);
for (const item of registry.items) {
  assert(item.docs?.trim());
  assert.equal(typeof item.meta.version, "string");
  assert.equal(typeof item.meta.wasmVersion, "string");
  assert(Array.isArray(item.meta.schemaIds));
}
for (let offset = 0; offset < registry.items.length; offset += 8)
  await Promise.all(
    registry.items.slice(offset, offset + 8).map(async ({ name }) => {
      const response = await fetch(new URL(`r/${name}.json`, base));
      assert.equal(response.status, 200, `Published item: ${name}`);
      assert.deepEqual(
        await response.json(),
        JSON.parse(await readFile(path.join(built, `${name}.json`), "utf8")),
        `Published source differs from the verified deployment artifact: ${name}`,
      );
    }),
  );
const view = await run(
  ["npx", "--yes", "shadcn@4.21.0", "view", "@finstack/pricing-workbench"],
  "view.txt",
);
assert(
  view.includes('"name": "pricing-workbench"') &&
    view.includes('"wasmVersion"'),
);
await run(
  ["npx", "--yes", "shadcn@4.21.0", "mcp", "init", "--client", "claude"],
  "mcp-init.txt",
);
const mcpConfig = JSON.parse(
  await readFile(path.join(consumer, ".mcp.json"), "utf8"),
);
assert(mcpConfig.mcpServers.shadcn);
await run(
  [
    "npm",
    "install",
    "--ignore-scripts",
    "--save-dev",
    "@modelcontextprotocol/sdk@1.30.0",
  ],
  "sdk-install.txt",
);
const require = createRequire(path.join(consumer, "package.json"));
const { Client } = require("@modelcontextprotocol/sdk/client/index.js");
const {
  StdioClientTransport,
} = require("@modelcontextprotocol/sdk/client/stdio.js");
const server = mcpConfig.mcpServers.shadcn;
const client = new Client({
  name: "registry-public-verification",
  version: "1.0.0",
});
const transport = new StdioClientTransport({
  command: server.command,
  args: server.args,
  cwd: consumer,
  env: process.env,
  stderr: "pipe",
});
const stderr = [];
transport.stderr?.on("data", (chunk) => stderr.push(chunk));
try {
  await client.connect(transport);
  const tools = await client.listTools();
  assert(tools.tools.some((tool) => tool.name === "view_items_in_registries"));
  const listed = await client.callTool({
    name: "list_items_in_registries",
    arguments: { registries: ["@finstack"], limit: 0 },
  });
  const viewed = await client.callTool({
    name: "view_items_in_registries",
    arguments: { items: ["@finstack/pricing-workbench"] },
  });
  for (const result of [listed, viewed]) {
    assert(!result.isError, JSON.stringify(result));
    assert(JSON.stringify(result).includes("pricing-workbench"));
  }
  await writeFile(
    path.join(evidence, "mcp.json"),
    JSON.stringify({ listed, viewed }, null, 2),
  );
  await writeFile(
    path.join(evidence, "result.json"),
    JSON.stringify(
      {
        url: base.href,
        consumer,
        registryItems: registry.items.length,
        publishedItemsMatchArtifact: registry.items.length,
        commands,
        configuredMcp: mcpConfig,
        mcpList: "pass",
        mcpView: "pass",
        verdict: "pass",
      },
      null,
      2,
    ) + "\n",
  );
  console.log(
    `Registry CLI/MCP and artifact checks passed for ${registry.items.length} items at ${base.href}`,
  );
} finally {
  await client.close();
  await writeFile(path.join(evidence, "mcp-stderr.txt"), Buffer.concat(stderr));
}
