import { defineConfig } from "playwright/test";
const baseURL = `http://127.0.0.1:${process.env.REGISTRY_GALLERY_PORT ?? 4178}`;
export default defineConfig({
  testDir: "./tests/e2e",
  testMatch: "**/*.spec.ts",
  workers: 1,
  retries: 0,
  timeout: 90000,
  forbidOnly: !!process.env.CI,
  updateSnapshots: "none",
  snapshotPathTemplate: "{testDir}/__screenshots__/{arg}{ext}",
  expect: {
    timeout: 20000,
    toHaveScreenshot: {
      animations: "disabled",
      maxDiffPixelRatio: 0.003,
      threshold: 0.2,
    },
  },
  use: {
    actionTimeout: 20000,
    baseURL,
    viewport: { width: 1440, height: 1700 },
    deviceScaleFactor: 1,
    colorScheme: "light",
    locale: "en-US",
    timezoneId: "UTC",
    trace: "retain-on-failure",
  },
  reporter: [["line"], ["json", { outputFile: "test-results/gallery.json" }]],
  webServer: {
    command: "node tests/e2e/serve.mjs",
    url: `${baseURL}/registry-gallery`,
    timeout: 30000,
    reuseExistingServer: false,
  },
});
