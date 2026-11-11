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

const LONG = ["/validation/", "/fasthenry-users/"];
const SHORT = ["/", "/about/", "/gallery/"];

test("long pages have an On this page list whose links resolve; short pages have none", async ({ page }) => {
  for (const path of LONG) {
    await page.goto(path);
    const toc = page.getByRole("navigation", { name: "On this page" });
    await expect(toc).toHaveCount(1);
    const links = await toc.locator("a[href^='#']").evaluateAll((as) => as.map((a) => a.getAttribute("href")!));
    expect(links.length, path).toBeGreaterThanOrEqual(8);
    for (const h of links) await expect(page.locator(`[id="${h.slice(1)}"]`), `${path} ${h}`).toHaveCount(1);
  }
  for (const path of SHORT) {
    await page.goto(path);
    await expect(page.getByRole("navigation", { name: "On this page" }), path).toHaveCount(0);
  }
});

test("the contents list is a rail on wide screens and a closed block below", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/validation/");
  const rail = page.locator(".toc-rail");
  await expect(rail).toBeVisible();
  await expect(page.locator(".toc details")).toBeHidden();
  const h1 = await page.locator("h1").boundingBox();
  const box = await rail.boundingBox();
  expect(box!.x, "rail sits right of the text column").toBeGreaterThan(h1!.x + 600);
  await page.mouse.wheel(0, 2000);
  await page.waitForTimeout(100);
  expect((await rail.boundingBox())!.y, "rail stays in view").toBeLessThan(200);
  await page.setViewportSize({ width: 800, height: 900 });
  await expect(rail).toBeHidden();
  const details = page.locator(".toc details");
  await expect(details).toBeVisible();
  expect(await details.evaluate((d: HTMLDetailsElement) => d.open)).toBe(false);
});

test("following Next from home visits every page once and ends", async ({ page }) => {
  const visited = ["/"];
  await page.goto("/");
  for (let i = 0; i < 20; i++) {
    const next = page.locator("a[rel=next]");
    if ((await next.count()) === 0) break;
    await next.click();
    visited.push(new URL(page.url()).pathname);
  }
  expect(visited).toEqual(["/", ...NAV.map((_, i) => PAGES[i + 1].path)]);
});

test("every page has a Next link except the last, and doc pages end with a Source line", async ({ page }) => {
  for (const p of PAGES.slice(0, -1)) {
    await page.goto(p.path);
    await expect(page.locator("a[rel=next]"), p.path).toHaveCount(1);
  }
  for (const path of ["/benchmarks/", ...LONG]) {
    await page.goto(path);
    await expect(page.locator("p.source"), path).toContainText("Source: docs/");
  }
});

test("the 404 page lists every page", async ({ page }) => {
  await page.goto("/no-such-page/");
  for (const label of NAV) await expect(page.locator("main").getByRole("link", { name: label, exact: true })).toBeVisible();
});

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

test("the benchmarks page leads with the chart, shows accuracy, and has no placeholder", async ({ page }) => {
  await page.goto("/benchmarks/");
  const chart = await page.locator('img[src="/assets/benchmarks/scaling.svg"]').boundingBox();
  const firstTable = await page.locator("table").first().boundingBox();
  expect(chart!.y).toBeLessThan(firstTable!.y);
  await expect(page.locator("table", { hasText: "relative difference" })).toHaveCount(1);
  await expect(page.locator("body")).not.toContainText("Not yet regenerated");
});

test("the doc-derived pages render their tables", async ({ page }) => {
  for (const path of ["/validation/", "/fasthenry-users/"]) {
    await page.goto(path);
    expect(await page.locator("table").count(), path).toBeGreaterThan(0);
    await expect(page.locator("table").first()).toBeVisible();
  }
});

test("validation opens with the summary table, before any code block", async ({ page }) => {
  await page.goto("/validation/");
  const first = page.locator("table").first();
  await expect(first).toBeVisible();
  await expect(page.locator("#what-was-checked table").first()).toHaveCount(1);
  expect(await first.evaluate((t) => !!t.closest("#what-was-checked"))).toBe(true);
  expect(await first.locator("tbody tr").count()).toBeGreaterThanOrEqual(5);
  const preAbove = await page.evaluate(() => {
    const t = document.querySelector("table")!;
    return Array.from(document.querySelectorAll("pre")).filter(
      (p) => t.compareDocumentPosition(p) & Node.DOCUMENT_POSITION_PRECEDING).length;
  });
  expect(preAbove).toBe(0);
  const box = await first.boundingBox();
  const vp = page.viewportSize()!;
  if (vp.width >= 1200) expect(box!.y).toBeLessThan(900);
});

test("no table cell breaks a word or number mid-token, and tables scroll inside their wrapper", async ({ page }) => {
  for (const path of ["/benchmarks/", "/validation/", "/fasthenry-users/"]) {
    await page.goto(path);
    const result = await page.evaluate(() => {
      const broken: string[] = [];
      // table cells stretch to the row height, so measure the text itself
      const lines = (el: Element) => {
        const r = document.createRange();
        r.selectNodeContents(el);
        const cs = getComputedStyle(el);
        const lh = cs.lineHeight === "normal" ? parseFloat(cs.fontSize) * 1.2 : parseFloat(cs.lineHeight);
        return Math.round(r.getBoundingClientRect().height / lh);
      };
      for (const cell of Array.from(document.querySelectorAll("th, td"))) {
        // a token (split on plain spaces only, so "19 600" is one) must sit on one line
        const walker = document.createTreeWalker(cell, NodeFilter.SHOW_TEXT);
        for (let n = walker.nextNode(); n; n = walker.nextNode()) {
          for (const m of (n.textContent ?? "").matchAll(/[^ ]+/g)) {
            const r = document.createRange();
            r.setStart(n, m.index!);
            r.setEnd(n, m.index! + m[0].length);
            const wrapped = new Set(Array.from(r.getClientRects()).map((q) => Math.round(q.top))).size > 1;
            // plain words may wrap after a hyphen ("line-numbered"); identifiers in <code> may not
            const hyphenated = m[0].includes("-") && !n.parentElement?.closest("code");
            if (wrapped && !hyphenated) broken.push(m[0]);
          }
        }
        if (cell.tagName === "TH" && lines(cell) > 2) broken.push(`th over two lines: ${cell.textContent}`);
        if (cell.classList.contains("num") && lines(cell) > 1) broken.push(`num cell over one line: ${cell.textContent}`);
      }
      const wraps = Array.from(document.querySelectorAll(".tablewrap")).map((w) => ({
        tabindex: w.getAttribute("tabindex"), role: w.getAttribute("role"), name: w.getAttribute("aria-label"),
      }));
      return { broken, wraps, overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth };
    });
    expect(result.broken, path).toEqual([]);
    expect(result.overflow, `${path} scrolls sideways`).toBeLessThanOrEqual(0);
    for (const w of result.wraps) expect(w, path).toEqual({ tabindex: "0", role: "region", name: expect.stringMatching(/^Table: \S/) });
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

test("every page has exactly one h1 and it is the first heading in main", async ({ page }) => {
  for (const p of PAGES) {
    await page.goto(p.path);
    await expect(page.locator("h1")).toHaveCount(1);
    const first = await page.locator("main").locator("h1, h2, h3, h4, h5, h6").first().evaluate((el) => el.tagName);
    expect(first, `${p.path}: first heading in main`).toBe("H1");
  }
});

test("the header stays compact on a phone", async ({ page }, info) => {
  test.skip(info.project.name !== "mobile", "phone project only");
  for (const p of PAGES) {
    await page.goto(p.path);
    const h = await page.locator("header").evaluate((el) => el.getBoundingClientRect().height);
    expect(h, `${p.path}: header height`).toBeLessThanOrEqual(110);
  }
});

test("the footer sits at the bottom of the viewport on a short page", async ({ page }) => {
  await page.goto("/about/");
  const gap = await page.evaluate(() => innerHeight - document.querySelector("footer")!.getBoundingClientRect().bottom);
  expect(Math.abs(gap) < 60 || gap < 0).toBe(true);
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
