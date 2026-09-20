import { readFile, readdir } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { converter, parse, wcagContrast } from "culori";
import ts from "typescript";
import postcss from "postcss";

export function checkContrast(palette) {
  for (const [key, value] of Object.entries(palette)) {
    const color = parse(value);
    if (!color || (color.alpha !== undefined && color.alpha !== 1))
      throw new Error(`Expected opaque colour: ${key}`);
  }
  const check = (a, b, minimum) => {
    if (
      !palette[a] ||
      !palette[b] ||
      wcagContrast(palette[a], palette[b]) < minimum
    )
      throw new Error(`Contrast below ${minimum}: ${a}/${b}`);
  };
  for (const bg of ["background", "card"]) {
    for (const fg of [
      "foreground",
      "muted-foreground",
      "positive",
      "negative",
      "warning",
      "error",
      "info",
      "par",
    ])
      check(fg, bg, 4.5);
    for (const mark of [
      "input",
      "ring",
      ...Array.from({ length: 6 }, (_, i) => `chart-${i + 1}`),
    ])
      check(mark, bg, 3);
  }
  for (const bg of ["primary", "accent", "card", "popover", "secondary"])
    check(`${bg}-foreground`, bg, 4.5);
  check("muted-foreground", "muted", 4.5);
  for (const fill of Object.keys(palette).filter((key) =>
    /^(sequential-|diverging-)/.test(key),
  )) {
    if (
      Math.max(
        wcagContrast(palette[fill], palette["cell-light"]),
        wcagContrast(palette[fill], palette["cell-dark"]),
      ) < 4.5
    )
      throw new Error(`Cell text contrast: ${fill}`);
  }
}

// Stock Tabs use foreground/60 in light mode and muted-foreground in dark mode.
export function checkStockContrast(palette, mode) {
  const rgb = converter("rgb");
  const foreground = rgb(
    palette[mode === "light" ? "foreground" : "muted-foreground"],
  );
  const opacity = mode === "light" ? 0.6 : 1;
  for (const key of ["muted", "background"]) {
    const background = rgb(palette[key]);
    const painted = {
      mode: "rgb",
      r: foreground.r * opacity + background.r * (1 - opacity),
      g: foreground.g * opacity + background.g * (1 - opacity),
      b: foreground.b * opacity + background.b * (1 - opacity),
    };
    if (wcagContrast(painted, background) < 4.5)
      throw new Error(`Stock Tabs contrast below 4.5: ${mode}/${key}`);
  }
}

export function checkLiterals(source, name) {
  const checkValue = (value) => {
    if (
      /(?:#[\da-f]{3,8}\b|\b(?:rgb|hsl|hwb|lab|lch|oklab|oklch|color)\s*\()/i.test(
        value,
      ) ||
      value.split(/\s+/).some((part) => parse(part)?.mode) ||
      /(?:bg|text|border|ring|fill|stroke)-(?:red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose|slate|gray|zinc|neutral|stone|white|black)(?:-\d+)?(?:\b|$)/.test(
        value,
      )
    )
      throw new Error(`Colour literal outside theme: ${name}`);
  };
  if (name.endsWith(".css")) {
    postcss.parse(source).walkDecls((decl) => {
      if (
        decl.prop.includes("font") &&
        !/^(var\(|inherit|initial|unset)/.test(decl.value) &&
        /family|sans|mono|^font$/.test(decl.prop)
      )
        throw new Error(`Font literal outside theme: ${name}`);
      checkValue(decl.value);
    });
  } else {
    const ast = ts.createSourceFile(name, source, ts.ScriptTarget.Latest, true);
    const visit = (node) => {
      if (ts.isStringLiteralLike(node)) checkValue(node.text);
      if (
        ts.isPropertyAssignment(node) &&
        ["fontFamily", "font"].includes(
          node.name.getText(ast).replaceAll(/["']/g, ""),
        ) &&
        ts.isStringLiteralLike(node.initializer) &&
        !node.initializer.text.startsWith("var(")
      )
        throw new Error(`Font literal outside theme: ${name}`);
      ts.forEachChild(node, visit);
    };
    visit(ast);
  }
}
export function tenantPalettes(tokens, css) {
  const palettes = { light: { ...tokens.light }, dark: { ...tokens.dark } };
  postcss.parse(css).walkRules((rule) => {
    for (const mode of ["light", "dark"])
      if (
        rule.selector.includes(`[data-theme="${mode}"]`) ||
        (mode === "light" && rule.selector.includes(":root"))
      )
        rule.walkDecls((decl) => {
          if (
            decl.prop.startsWith("--") &&
            decl.prop.slice(2) in palettes[mode]
          )
            palettes[mode][decl.prop.slice(2)] = decl.value;
        });
  });
  return palettes;
}
export async function checkTokens(root) {
  const tokens = JSON.parse(
    await readFile(
      path.join(root, "registry/theme/finstack-theme/tokens.json"),
      "utf8",
    ),
  );
  if (
    Object.keys(tokens.light).sort().join() !==
    Object.keys(tokens.dark).sort().join()
  )
    throw new Error("Light/dark token keys differ");
  for (const mode of ["light", "dark"]) {
    checkContrast(tokens[mode]);
    checkStockContrast(tokens[mode], mode);
  }
  const tenant = tenantPalettes(
    tokens,
    await readFile(path.join(root, "tests/fixtures/tenant.css"), "utf8"),
  );
  for (const [mode, palette] of Object.entries(tenant)) {
    checkContrast(palette);
    checkStockContrast(palette, mode);
  }
  for (const directory of ["src", "registry"]) {
    const files = await readdir(path.join(root, directory), {
      recursive: true,
      withFileTypes: true,
    });
    for (const file of files) {
      const absolute = path.join(file.parentPath, file.name);
      const relative = path.relative(root, absolute);
      if (
        !file.isFile() ||
        /^(registry\/theme\/|src\/generated\/)/.test(relative) ||
        !/\.(css|[cm]?[jt]sx?)$/.test(file.name)
      )
        continue;
      checkLiterals(await readFile(absolute, "utf8"), relative);
    }
  }
}
if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  await checkTokens(fileURLToPath(new URL("../", import.meta.url)));
  console.log("Checked theme, tenant contrast and source token usage.");
}
