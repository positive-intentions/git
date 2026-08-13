# Clone

Optional network demo. Default stories use **Init**; this one exercises `GitRepo::clone`.

| Platform | Notes |
|----------|--------|
| Desktop | `gix::prepare_clone` + fetch/checkout (HTTPS via reqwest/rustls) |
| Web | isomorphic-git + OPFS; **CORS proxy required** for GitHub |

Knobs:

- **URL** — remote to clone (prefer a tiny public repo)
- **CORS proxy** — used only on web (default `https://cors.isomorphic-git.org`, demo-only)

After a successful clone, **List** shows the worktree root.
