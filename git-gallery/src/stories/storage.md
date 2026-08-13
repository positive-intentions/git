# Storage

Git-backed file explorer. Clone a remote (including private repos with a PAT), then
browse, create, edit, and drop files. Changes sync automatically — commit, pull, and
push run on a timer and after local edits. You should not need to drive git manually.

| Platform | Notes |
|----------|--------|
| Desktop | `gix` / system `git`; explorer UI from whatsup-ui |
| Web | isomorphic-git + OPFS; CORS proxy for remotes; Monaco overlay for text edits |

Connection fields (URL, username, access token, CORS proxy) are entered in the demo pane
and persisted in **localStorage** under `git-gallery:storage` on web.

**Sync:** every 15 seconds (and after saves / creates / moves / drops) the story commits
dirty work, fast-forward pulls, then pushes. A toolbar indicator shows Checking /
Syncing / Synced / Error / Conflict.

**Conflicts:** pull is fast-forward only. When histories diverge, choose **Accept local**
(force-push with lease) or **Accept remote** (hard reset to `origin`). The choice is
remembered for the session.

Empty folders are stored as `folder/.keep` (hidden in the explorer).
