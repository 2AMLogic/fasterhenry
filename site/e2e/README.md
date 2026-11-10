# site/e2e — browser tests for fasterhenry.org

[Playwright](https://playwright.dev) tests that load the site in Chromium, at a
desktop and a phone viewport, and check that it works for a reader. They are
separate from `site/test_build.py`, which unit-tests the renderer.

## Run

```sh
scripts/deploy-site.sh --no-deploy     # build site/dist first
cd site/e2e
npm ci
npx playwright install chromium        # once
npm test                               # against the local build
npm run test:live                      # against https://fasterhenry.org
```

With `BASE_URL` unset the tests start `serve.py`, which serves `site/dist` on
`127.0.0.1:4173` and answers unknown paths with `404.html` and status 404, as
Cloudflare Pages does. With `BASE_URL` set they test that host and also run
the tests tagged `@live`.

## What is covered

| Check | Local | Live |
|---|---|---|
| Each page returns 200 with its title, one `h1`, description, nav, and no console errors or failed requests | yes | yes |
| Every image renders and has alt text | yes | yes |
| No page scrolls sideways (desktop and phone) | yes | yes |
| Every same-site link and in-page anchor resolves | yes | yes |
| Nav and brand links, the skip link, dark colour scheme | yes | yes |
| Hero figure, install line, gallery figure count, scaling plot, doc tables | yes | yes |
| Unknown path returns 404 with the not-found page | yes | yes |
| `www` redirects to the apex; `http` redirects to `https` | no | yes |
| Response headers from `_headers` (nosniff, referrer policy, asset caching) | no | yes |

## CI

`.github/workflows/site.yml` runs the local tests on every pull request that
touches the site and before any deploy. After a deploy from `main`, and once a
day, it runs the whole suite against the live site.

A new page needs a row in `PAGES` at the top of `tests/site.spec.ts`.
