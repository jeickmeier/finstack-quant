import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";
import path from "node:path";
import { chromium } from "playwright";
const directory = path.resolve(process.argv[2]);
const browser = await chromium.launch({ headless: true });
try {
  const context = await browser.newContext({
      offline: true,
      viewport: { width: 900, height: 600 },
    }),
    page = await context.newPage(),
    errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(pathToFileURL(path.join(directory, "publication.svg")).href);
  await page.evaluate(() => document.fonts.ready);
  const svg = await page.evaluate(() => ({
    fonts: [...document.fonts].map((face) => ({
      family: face.family,
      weight: face.weight,
      status: face.status,
    })),
    text: [...document.querySelectorAll("text")].map((node) => {
      const box = node.getBoundingClientRect();
      return {
        text: node.textContent,
        x: box.x,
        y: box.y,
        width: box.width,
        height: box.height,
      };
    }),
    externalReferences: [...document.querySelectorAll("[href],[src]")]
      .map((node) => node.getAttribute("href") ?? node.getAttribute("src"))
      .filter((url) => /^https?:/.test(url)),
  }));
  assert(
    svg.fonts.filter((face) => face.status === "loaded").length >= 2,
    "Standalone SVG must load its embedded font faces offline",
  );
  assert(svg.text.length > 10);
  assert.deepEqual(svg.externalReferences, []);
  const outside = svg.text.filter(
    (box) =>
      box.x < -0.5 ||
      box.y < -0.5 ||
      box.x + box.width > 900.5 ||
      box.y + box.height > 600.5,
  );
  assert.deepEqual(
    outside,
    [],
    "Standalone labels must fit the intended figure size",
  );
  await page.screenshot({
    path: path.join(directory, "publication-offline.png"),
  });
  await page.goto(pathToFileURL(path.join(directory, "publication.png")).href);
  const dimensions = await page.locator("img").evaluate((image) => ({
    width: image.naturalWidth,
    height: image.naturalHeight,
    complete: image.complete,
  }));
  assert.deepEqual(dimensions, { width: 1800, height: 1200, complete: true });
  assert.deepEqual(errors, []);
  await writeFile(
    path.join(directory, "offline.json"),
    JSON.stringify(
      { offline: true, svg, png: dimensions, errors, verdict: "pass" },
      null,
      2,
    ) + "\n",
  );
} finally {
  await browser.close();
}
