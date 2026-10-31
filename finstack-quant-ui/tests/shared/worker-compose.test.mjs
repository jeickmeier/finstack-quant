import { describe, expect, it } from "vitest";
import { scanSource } from "../../scripts/check-worker-compose.mjs";

const service = (body) =>
  `export function create(native, context) {\n  const api = native.scenarios;\n  return { run(request) {\n${body}\n  } };\n}`;
const errorsIn = (body) =>
  scanSource(service(body), "registry/shared/file/services/a.ts").errors;

describe("check-worker-compose", () => {
  it.each([
    ["const a = api.run(); if (a.ok) return 1;", "if"],
    ["const a = api.run(); return a.ok ? 1 : 2;", "?:"],
    ["const a = api.run(); return a.date ?? request.date;", "??"],
    ["const a = api.run(); for (const b of a) use(b);", "for...of"],
    ["const a = api.run(); for (const b in a) use(b);", "for...in"],
    ["const a = api.run(); while (a.next) use(a);", "while"],
    ["const a = api.run(); return a.map((b) => b);", ".map"],
    ["return api.run().filter((b) => b);", ".filter"],
    ["const { a } = api.run(); if (a) return 1;", "if"],
    ["if (api.predicate(request)) return 1;", "if"],
  ])("flags %s and says how to resolve it", (body, kind) => {
    const [error, ...rest] = errorsIn(body);
    expect(rest).toEqual([]);
    expect(error).toContain(`${kind} reads a native result`);
    expect(error).toContain("// finstack-compose: <reason>");
  });

  it("follows a value through variables, assignments, callbacks and local functions", () => {
    expect(errorsIn("let a; a = api.run(); if (a) return 1;")).toHaveLength(1);
    expect(
      errorsIn("const a = api.run(); const b = a.items; if (b) return 1;"),
    ).toHaveLength(1);
    expect(
      errorsIn(
        "const make = () => api.run(); const a = make(); if (a) return 1;",
      ),
    ).toHaveLength(1);
    expect(
      errorsIn(
        "// finstack-compose: per item\nreturn api.run().map((b) =>\n(b.ok ? 1 : 2));",
      ),
    ).toHaveLength(1);
  });

  it.each([
    ["if (request.mode) return api.run();"],
    ["return request.bump ?? undefined;"],
    ["return request.points.map((point) => api.run(point));"],
    ["const a = api.first(); return api.second(a);"],
    ["const a = api.run(); return a.value ?? null;"],
    ["const a = api.run(); return a.value ?? undefined;"],
    ["const a = api.run(); return request.rows.map((a) => (a ? 1 : 2));"],
  ])("does not flag %s", (body) => {
    expect(errorsIn(body)).toEqual([]);
  });

  it("allows a comment with a reason on or directly above the line", () => {
    expect(
      errorsIn(
        "const a = api.run(); if (a) return 1; // finstack-compose: routes the request",
      ),
    ).toEqual([]);
    expect(
      errorsIn(
        "const a = api.run();\n// finstack-compose: routes the request\nif (a) return 1;",
      ),
    ).toEqual([]);
  });

  it("fails a comment with no reason, too far above or covering nothing", () => {
    const empty = errorsIn(
      "const a = api.run();\n// finstack-compose:\nif (a) return 1;",
    );
    expect(empty[0]).toContain("finstack-compose comment has no reason");
    expect(empty[1]).toContain("if reads a native result");
    const far = errorsIn(
      "const a = api.run();\n// finstack-compose: stale\n\nif (a) return 1;",
    );
    expect(far[0]).toContain("covers no decision");
    expect(far[1]).toContain("if reads a native result");
    expect(errorsIn("// finstack-compose: stale\nreturn 1;")[0]).toContain(
      "covers no decision",
    );
  });
});
