# Files

Basic worktree + staging operations (not a full editor):

| Button | API |
|--------|-----|
| Init | `GitRepo::init` |
| Write | `write_file` (create + stage) |
| Edit | same `write_file` with new content |
| Read | `read_file` |
| Remove | `remove_file` |
| List | `list` |

Use the knobs for path and content, then press the buttons.
