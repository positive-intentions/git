# git-gallery

Local Dioxus gallery for exercising [`git-core`](../git-core) through interactive demos.
UI chrome comes from [`whatsup-ui`](https://github.com/positive-intentions/whatsup-ui)
(`gallery` feature).

## Run (web)

```bash
cd git-gallery
# optional: npm install   # local isomorphic-git; otherwise esm.sh CDN is used
dx serve --bin git-gallery --platform web
```

## Run (desktop)

```bash
sudo apt-get install -y libxdo-dev libwebkit2gtk-4.1-dev libgtk-3-dev
cd git-gallery
dx serve --bin git-gallery --platform desktop
# or: cargo run --no-default-features --features desktop
```

## Stories

| Demo | API |
|------|-----|
| Init | `GitRepo::init` |
| Files | write / read / edit / remove / list |
| Status | mutations + `status` |
| Clone | authenticated clone + commit / branches / log / diff / fetch / pull / push; Monaco on web |

## Coverage page

After generating llvm-cov HTML from the repo root:

```bash
npm run test:rust:coverage
```

open `/coverage` in the gallery (or the Coverage link in the sidebar).

## Tests

JS helper unit tests: `npm test` (Vitest). Gallery Playwright smokes live in [`../e2e`](../e2e).
See the root [README](../README.md#development) for the full quality suite.
