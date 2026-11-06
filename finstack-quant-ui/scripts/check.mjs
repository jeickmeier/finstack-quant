import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

// `npm run check`: the registry gate. The checks only read the package, so
// independent ones run concurrently: the quick static checks in order, the
// native fixture check, the type check and the Vitest suite. Each branch's
// output prints when it finishes; every branch runs to completion so one run
// reports every failure.
const root = fileURLToPath(new URL("..", import.meta.url));
const node = (script, ...args) => [process.execPath, script, ...args];
const bin = (name, ...args) => [`node_modules/.bin/${name}`, ...args];
const branches = {
  static: [
    node("scripts/run-generators.mjs", "--check"),
    bin("prettier", "--check", "."),
    node("scripts/check-tokens.mjs"),
    node("scripts/check-no-arithmetic.mjs"),
    node("scripts/check-worker-compose.mjs"),
    node("scripts/check-registry.mjs"),
    node("scripts/coverage.mjs", "--check"),
  ],
  fixtures: [node("scripts/run-fixtures.mjs", "--check")],
  typecheck: [bin("tsc", "--noEmit")],
  vitest: [bin("vitest", "run")],
};

/** Run one command, buffering its output; resolves to its exit status. */
function run([command, ...args], output) {
  return new Promise((resolve) => {
    output.push(Buffer.from(`$ ${[command, ...args].join(" ")}\n`));
    const child = spawn(command, args, {
      cwd: root,
      stdio: ["ignore", "pipe", "pipe"],
    });
    child.stdout.on("data", (chunk) => output.push(chunk));
    child.stderr.on("data", (chunk) => output.push(chunk));
    child.once("error", (error) => {
      output.push(Buffer.from(`${error.message}\n`));
      resolve(1);
    });
    child.once("close", (code) => resolve(code ?? 1));
  });
}

const failed = [];
await Promise.all(
  Object.entries(branches).map(async ([name, commands]) => {
    const output = [];
    let status = 0;
    for (const command of commands) {
      status = await run(command, output);
      if (status !== 0) break;
    }
    process.stdout.write(`── ${name}\n${Buffer.concat(output)}`);
    if (status !== 0) {
      console.error(`── ${name} failed (exit ${status}).`);
      failed.push(name);
    }
  }),
);
if (failed.length) process.exit(1);
