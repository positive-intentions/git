# Clone

Comprehensive remote workflow debugger. Exercises authenticated `GitRepo::clone` plus
commit, branches, log, diff, fetch, fast-forward pull, and push.

| Platform | Notes |
|----------|--------|
| Desktop | `gix` clone (optional HTTPS credentials) + system `git` for commit/push/pull/branches/log/diff |
| Web | isomorphic-git + OPFS; **CORS proxy** required for GitHub; Monaco editor for file edits |

Connection fields (URL, username, access token, CORS proxy) are entered in the demo pane
(not gallery knobs) and persisted in **localStorage** under `git-gallery:clone` on web.

Private remotes need **username + access token** (PAT). Tokens are never written into
`.git/config` or remote URLs — only passed at request time via isomorphic-git `onAuth`
(web) or gix credentials / git askpass (desktop).

**Web CORS note:** the public `cors.isomorphic-git.org` proxy may not forward
`Authorization` for private repos. If clone fails on a private URL, try a proxy that
forwards auth headers, or run the desktop gallery.

After clone:

- Browse the file tree; open a file in **Monaco** (web) or a textarea (desktop)
- **Save + stage** writes through `GitRepo::write_file`
- Status / Commit / Branches / Log / Diff / Fetch / Pull / Push use the shared trait

Clone fetches a **shallow** tip (`depth: 1`, single branch) so browser demos finish
quickly through the CORS proxy. Log shows the fetched history; huge remotes are still
slow — prefer a small public repo for smoke tests.
