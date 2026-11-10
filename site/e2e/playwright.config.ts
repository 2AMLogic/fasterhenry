import { defineConfig, devices } from "@playwright/test";

// BASE_URL unset: test the local build (site/dist) through serve.py.
// BASE_URL set (e.g. https://fasterhenry.org): test that deployment, and also
// run the tests tagged @live, which need the real host (redirects, headers).
const baseURL = process.env.BASE_URL;
const local = "http://127.0.0.1:4173";

export default defineConfig({
  testDir: "tests",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: baseURL ? 2 : 0,
  reporter: process.env.CI ? [["github"], ["list"]] : "list",
  grepInvert: baseURL ? undefined : /@live/,
  use: { baseURL: baseURL ?? local, trace: "retain-on-failure" },
  webServer: baseURL
    ? undefined
    : { command: "python3 serve.py 4173", url: local, reuseExistingServer: !process.env.CI },
  projects: [
    { name: "desktop", use: { ...devices["Desktop Chrome"] } },
    { name: "mobile", use: { ...devices["Pixel 7"] } },
  ],
});
