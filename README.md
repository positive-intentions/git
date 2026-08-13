# Git

[![CI](https://img.shields.io/github/actions/workflow/status/positive-intentions/git/ci.yml?branch=staging&label=CI)](https://github.com/positive-intentions/git/actions/workflows/ci.yml)
[![Pages](https://img.shields.io/github/actions/workflow/status/positive-intentions/git/deploy-pages.yml?branch=staging&label=Pages)](https://github.com/positive-intentions/git/actions/workflows/deploy-pages.yml)
[![Gallery](https://img.shields.io/badge/gallery-live-brightgreen)](https://positive-intentions.github.io/git/)

Interactive demos for basic Git operations, using a shared Rust `GitRepo` API and a
[whatsup-ui](https://github.com/positive-intentions/whatsup-ui) gallery (same pattern as
[`signal-protocol-gallery`](../signal-protocol/signal-protocol-gallery)).

| Crate | Role |
|-------|------|
| [`git-core`](git-core/) | `GitRepo` trait + platform backends |
| [`git-gallery`](git-gallery/) | Dioxus + whatsup-ui demos (standalone package) |

**Live gallery:** [positive-intentions.github.io/git](https://positive-intentions.github.io/git/)

## Dual backend

```
Gallery stories  →  GitRepo trait
                      ├─ desktop: gitoxide (`gix`) on the native filesystem
                      └─ web:     isomorphic-git + OPFS (via wasm-bindgen → JS)
```

**Why not gitoxide in the browser?** `gix` does not yet compile/run as a whole on
`wasm32-unknown-unknown` (index, refs, config, tempfile, sec, transport). It also
memory-maps packfiles; OPFS has no mmap. Until that lands upstream, the web gallery
uses [isomorphic-git](https://isomorphic-git.org/) behind the same Rust trait so
stories stay platform-agnostic.

## Run

### Desktop (native `gix`)

```bash
cd git-gallery
dx serve --bin git-gallery --platform desktop
# or:
cargo run --no-default-features --features desktop
```

Desktop needs system libs (GTK/WebKit). On Debian/Ubuntu:

```bash
sudo apt-get install -y libxdo-dev libwebkit2gtk-4.1-dev libgtk-3-dev
```

### Web (isomorphic-git + OPFS)

```bash
cd git-gallery
# once: npm install   # isomorphic-git for the browser helper
dx serve --bin git-gallery --platform web
```

Open the printed URL. Stories that only `init` / read / write / status work offline.
**Clone** needs a CORS proxy (default knob: `https://cors.isomorphic-git.org` — demo only).

## Stories

| Demo | What it exercises |
|------|-------------------|
| Init | `GitRepo::init` in a throwaway workdir |
| Files | write / read / edit / remove / list files & folders |
| Status | mutations then `GitRepo::status` |
| Clone | Authenticated clone + commit / branches / log / diff / fetch / FF-pull / push; Monaco editor on web |

The **Clone** story is the full workflow debugger. Connection fields (URL, username,
access token, CORS proxy) live in the demo pane and persist in `localStorage` on web.
Monaco is gallery-only (web); desktop uses a textarea. Core git operations work on both
backends via the shared `GitRepo` trait.

## Development

```bash
# Native unit tests (git-core)
cargo test -p git-core
# or: npm run test:rust

# Line coverage (100% gate on native git-core; wasm-only paths use coverage(off))
npm run install:llvm-cov   # once
npm run test:rust:coverage      # HTML → git-gallery/assets/coverage-html/
npm run test:rust:coverage:ci   # --fail-under-lines 100 + lcov.info

# JS pure helpers (git-web-helpers.js)
npm run test:js

# Wasm glue (web.rs + mocked GitWeb)
npm run test:wasm:node      # requires wasm-pack

# Gallery browser smokes
cd git-gallery && dx build --platform web --bin git-gallery
cd ../e2e && npm ci && npx playwright install chromium
npm test                    # or from repo root: npm run test:e2e

cargo fmt --all --check
cargo clippy -p git-core --all-targets -- -D warnings
```

### Coverage notes

- CI enforces **100% line coverage** on native `git-core` via `cargo-llvm-cov`.
- `web.rs` is `wasm32`-only and is not part of the host llvm-cov gate; it is covered by
  `wasm-pack test --node`.
- Gallery UI Rust is covered by Playwright, not llvm-cov.
- Untestable native edges (path-escape after `..` rejection, non-UTF8 workdirs, rare
  status shapes) are marked with `#[cfg_attr(coverage_nightly, coverage(off))]`.
- Open the gallery **Coverage** page (`/coverage`) after generating the HTML report.

## Deploy (GitHub Pages)

Push to **`staging`** (or run **Deploy to GitHub Pages** via `workflow_dispatch`) builds a
release web gallery with llvm-cov HTML baked in and deploys to
[GitHub Pages](https://positive-intentions.github.io/git/).

Pages source must be **GitHub Actions** (Settings → Pages). The workflow sets Dioxus
`base_path = "git"` so assets and routes resolve under `/git/`.

## Layout notes

- Root workspace members: `git-core` only. `git-gallery` has its own `[workspace]` so
  `dx` treats it like `signal-protocol-gallery`.
- `whatsup-ui` is patched to the sibling checkout at `../whatsup-ui`.
- Web JS helper: [`git-gallery/assets/git-web.js`](git-gallery/assets/git-web.js)
  (pure helpers in [`git-web-helpers.js`](git-gallery/assets/git-web-helpers.js)).
