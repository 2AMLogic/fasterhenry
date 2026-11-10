import { expect, test, type Page } from "@playwright/test";

const PAGES = [
  { path: "/", title: "fasterhenry: clean-room PEEC R/L extraction in Rust", h1: "fasterhenry" },
  { path: "/gallery/", title: "Gallery: fasterhenry", h1: "Gallery" },
  { path: "/benchmarks/", title: "Benchmarks: fasterhenry", h1: "Benchmarks" },
  { path: "/validation/", title: "Validation: fasterhenry", h1: /.+/ },
  { path: "/fasthenry-users/", title: "FastHenry users: fasterhenry", h1: /.+/ },
  { path: "/about/", title: "About: fasterhenry", h1: "About" },
];
const NAV = ["Gallery", "Benchmarks", "Validation", "FastHenry users", "About"];

/** Collect everything that goes wrong while a page loads. */
function watch(page: Page): string[] {
  const problems: string[] = [];
  page.on("console", (m) => m.type() === "error" && problems.push(`console: ${m.text()}`));
  page.on("pageerror", (e) => problems.push(`pageerror: ${e.message}`));
  page.on("requestfailed", (r) => problems.push(`failed: ${r.url()}`));
  page.on("response", (r) => r.status() >= 400 && problems.push(`${r.status()}: ${r.url()}`));
  return problems;
}

for (const p of PAGES) {
  test.describe(p.path, () => {
    test("loads cleanly with its title, heading and navigation", async ({ page }) => {
      const problems = watch(page);
      const response = await page.goto(p.path, { waitUntil: "load" });
      expect(response?.status()).toBe(200);
      await expect(page).toHaveTitle(p.title);
      await expect(page.locator("h1")).toHaveCount(1);
      await expect(page.locator("h1")).toHaveText(p.h1);
      await expect(page.locator("html")).toHaveAttribute("lang", "en");
      await expect(page.locator('meta[name="description"]')).toHaveAttribute("content", /\S{3,}/);

      const nav = page.getByRole("navigation", { name: "Primary" });
      for (const label of NAV) await expect(nav.getByRole("link", { name: label, exact: true })).toBeVisible();
      // a page that is in the nav marks itself as the current one
      const current = nav.locator('[aria-current="page"]');
      if (p.path === "/") await expect(current).toHaveCount(0);
      else await expect(current).toHaveAttribute("href", p.path);

      // the build must not leave template placeholders in the page
      expect(await page.content()).not.toContain("{{");
      expect(problems).toEqual([]);
    });

    test("every image renders and has alt text", async ({ page }) => {
      await page.goto(p.path, { waitUntil: "load" });
      for (const img of await page.locator("img").all()) {
        await img.scrollIntoViewIfNeeded();
        const src = await img.getAttribute("src");
        expect(await img.evaluate((el: HTMLImageElement) => el.complete && el.naturalWidth > 0), `${src} did not render`).toBe(true);
        expect(await img.getAttribute("alt"), `${src} has no alt text`).toBeTruthy();
      }
    });

    test("does not scroll sideways", async ({ page }) => {
      await page.goto(p.path, { waitUntil: "load" });
      const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
      expect(overflow, "page is wider than the viewport").toBeLessThanOrEqual(0);
    });

    test("every same-site link and in-page anchor resolves", async ({ page, request }) => {
      await page.goto(p.path, { waitUntil: "load" });
      const origin = new URL(page.url()).origin;
      const hrefs = await page.locator("a[href]").evaluateAll((as) => as.map((a) => (a as HTMLAnchorElement).href));
      const ids = new Set(await page.locator("[id]").evaluateAll((els) => els.map((e) => e.id)));
      const here = page.url().split("#")[0];
      const targets = new Set<string>();
      for (const href of hrefs) {
        const url = new URL(href);
        if (url.origin !== origin) continue;
        const bare = href.split("#")[0];
        if (bare === here && url.hash) {
          expect(ids.has(decodeURIComponent(url.hash.slice(1))), `no element with id ${url.hash}`).toBe(true);
        } else {
          targets.add(bare);
        }
      }
      for (const target of targets) {
        const r = await request.get(target);
        expect(r.status(), target).toBe(200);
      }
    });
  });
}

test("the primary navigation and the brand link go where they say", async ({ page }) => {
  await page.goto("/");
  const nav = page.getByRole("navigation", { name: "Primary" });
  for (const p of PAGES.slice(1)) {
    const label = NAV[PAGES.indexOf(p) - 1];
    await nav.getByRole("link", { name: label, exact: true }).click();
    await expect(page).toHaveURL(p.path);
    await expect(page).toHaveTitle(p.title);
  }
  await page.getByRole("link", { name: "fasterhenry", exact: true }).first().click();
  await expect(page).toHaveURL("/");
});

test("the skip link is the first tab stop and moves to the main content", async ({ page }) => {
  await page.goto("/");
  await page.keyboard.press("Tab");
  const skip = page.getByRole("link", { name: "Skip to content" });
  await expect(skip).toBeFocused();
  await expect(skip).toBeInViewport();
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/#main$/);
});

test("the landing page shows the hero figure, its data and the install line", async ({ page, request }) => {
  await page.goto("/");
  await expect(page.locator('img[src="/assets/hero-spiral.svg"]')).toBeVisible();
  await expect(page.locator("pre", { hasText: "cargo install fasterhenry-cli" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Repository" })).toHaveAttribute("href", "https://github.com/2AMLogic/fasterhenry");
  const data = await request.get("/assets/hero-spiral.json");
  expect(data.status()).toBe(200);
  const hero = await data.json();
  // the caption quotes the version and command the committed data was made with
  await expect(page.locator("figcaption")).toContainText(hero.fasterhenry_version);
  await expect(page.locator("figcaption")).toContainText(hero.command);
});

test("the gallery shows one rendered figure per committed gallery entry", async ({ page, request }) => {
  const manifest = await (await request.get("/assets/gallery/gallery.json")).json();
  const entries = Array.isArray(manifest) ? manifest : (manifest.entries ?? manifest.decks ?? Object.values(manifest));
  expect(entries.length).toBeGreaterThanOrEqual(6);
  await page.goto("/gallery/");
  await expect(page.locator('img[src^="/assets/gallery/"]')).toHaveCount(entries.length);
});

test("the benchmarks page draws the scaling plot", async ({ page }) => {
  await page.goto("/benchmarks/");
  await expect(page.locator('img[src="/assets/benchmarks/scaling.svg"]')).toBeVisible();
});

test("the doc-derived pages render their tables", async ({ page }) => {
  for (const path of ["/validation/", "/fasthenry-users/"]) {
    await page.goto(path);
    expect(await page.locator("table").count(), path).toBeGreaterThan(0);
    await expect(page.locator("table").first()).toBeVisible();
  }
});

test("follows the reader's dark colour scheme", async ({ page }) => {
  const background = () => page.evaluate(() => getComputedStyle(document.body).backgroundColor);
  await page.emulateMedia({ colorScheme: "light" });
  await page.goto("/");
  expect(await background()).toBe("rgb(255, 255, 255)");
  await page.emulateMedia({ colorScheme: "dark" });
  expect(await background()).not.toBe("rgb(255, 255, 255)");
});

test("an unknown path answers 404 with the not-found page", async ({ page }) => {
  const response = await page.goto("/no-such-page/");
  expect(response?.status()).toBe(404);
  await expect(page.locator("h1")).toHaveText("Page not found");
  await page.getByRole("link", { name: "home page" }).click();
  await expect(page).toHaveURL("/");
});

// Checks that only make sense against the deployed host. Run with BASE_URL set.
test.describe("deployment @live", () => {
  test("www redirects permanently to the apex, keeping path and query", async ({ request, baseURL }) => {
    const apex = new URL(baseURL!);
    const r = await request.get(`https://www.${apex.host}/gallery/?x=1`, { maxRedirects: 0 });
    expect(r.status()).toBe(301);
    expect(r.headers()["location"]).toBe(`https://${apex.host}/gallery/?x=1`);
  });

  test("plain http redirects to https", async ({ request, baseURL }) => {
    const apex = new URL(baseURL!);
    const r = await request.get(`http://${apex.host}/about/`, { maxRedirects: 0 });
    expect(r.status()).toBe(301);
    expect(r.headers()["location"]).toBe(`https://${apex.host}/about/`);
  });

  test("pages and assets carry the headers the build declares", async ({ request }) => {
    const home = await request.get("/");
    expect(home.headers()["content-type"]).toContain("text/html");
    expect(home.headers()["x-content-type-options"]).toBe("nosniff");
    expect(home.headers()["referrer-policy"]).toBe("strict-origin-when-cross-origin");
    const svg = await request.get("/assets/hero-spiral.svg");
    expect(svg.headers()["content-type"]).toContain("image/svg+xml");
    expect(svg.headers()["cache-control"]).toContain("max-age=3600");
  });
});
