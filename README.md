# privconf

Private config manager for project-specific files. Sync `mise.local.toml`, `.env`, scripts, and other private files across devices via a separate git repo, using symlinks to deploy into project directories while keeping `git status` clean.

## TL;DR

```bash
privconf init                          # one-time setup
cd ~/Projects/myproj
privconf add mise.local.toml .env      # add files (project auto-detected from git remote)
privconf add                           # or just create the project entry, add files later
privconf add scripts/                  # directories work too
privconf ignore debug.log              # ignore a file (no symlink, just hide from git)
privconf remove mise.local.toml        # remove a file
privconf unlink                        # undo all links in this project
privconf sync                          # git pull/commit/push the store
privconf status                        # see what's linked
privconf list                          # list all projects
```

On another machine:

```bash
privconf init                          # one-time setup (or clone your store repo to ~/.privconf)
# or:
privconf init <remote-url>             # clone an existing store repo
cd ~/Projects/myproj
privconf link --sync                   # pull latest + create symlinks
```

Set up once and forget:

```bash
# Bash
eval "$(privconf hook bash)"

# Zsh
eval "$(privconf hook zsh)"

# Fish
privconf hook fish > ~/.config/fish/conf.d/privconf.fish
```

## How It Works

1. **Store**: A dedicated git repo (`~/.privconf/`) holds all your private config files, organized by project name.
2. **Add & Link**: `privconf add` copies files to the store and immediately creates symlinks. Edits sync instantly — no bidirectional sync logic needed.
3. **Invisible**: Untracked files are added to `.git/info/exclude`; tracked files use `git update-index --skip-worktree`. Your `git status` stays clean.
4. **Auto-link**: Shell hook runs `privconf link --quiet` on every `cd`, so you never think about it.

## Install

```bash
mise use -g github:gaojunran/privconf
```

Or build from source:

```bash
cargo install --git https://github.com/gaojunran/privconf
```

## Commands

| Command | Description |
|---------|-------------|
| `init [<remote>]` | Initialize privconf store at `~/.privconf/`. With a remote URL, clone an existing store repo instead. |
| `add [-p <name>] [files...]` | Add files/dirs to store and create symlinks. Project name auto-detected from git remote. Omit files to create project only. |
| `ignore [-p <name>] <files...>` | Ignore files in current project (add to `.git/info/exclude` or `skip-worktree`, no symlink, no store copy) |
| `remove [-p <name>] <files...>` | Remove files from store, remove symlinks, restore originals |
| `link [-s] [-q]` | Rebuild symlinks for current project. `--sync` / `-s` pulls store first. `--quiet` / `-q` suppresses output. |
| `unlink` | Remove all symlinks and restore original files |
| `status` | Show link status for current directory |
| `sync` | Pull, commit, and push the store repo |
| `list` | List all projects in the store |
| `hook <bash\|zsh\|fish>` | Print shell hook script |

## Project Matching

When `privconf link` or `privconf status` needs to find which project belongs to the current directory, it matches by:

1. **git remote** (priority) — if the repo's `origin` URL contains the project's `match_remote`
2. **path glob** — if the current directory matches the project's `match_path` pattern

Projects are auto-created by `privconf add` with `match_remote` set from the git remote URL. You can also add `match_path` manually in `config.toml` for projects without remotes.

## Configuration

The store lives at `~/.privconf/` by default. Override with `PRIVCONF_DIR`:

```bash
export PRIVCONF_DIR=/path/to/custom/store
```

### `config.toml`

```toml
[[project]]
name = "myproj"
match_remote = "git@github.com:myco/myproj.git"
files = ["mise.local.toml", ".env.local"]
ignored = ["debug.log"]

[[project]]
name = "work"
match_path = "~/Projects/work/*"
files = [".env", "scripts/deploy.sh"]
ignored = ["*.log"]
```

### `state.toml`

Tracks linked files (auto-managed, don't edit manually):

```toml
[[linked]]
project = "myproj"
file = "mise.local.toml"
target = "/home/user/Projects/myproj/mise.local.toml"
skip_worktree = false
```

## `add` vs `ignore`

- **`add`** — copies the file to the store, creates a symlink, and hides it from git. The file is synced across devices via the store repo.
- **`ignore`** — does NOT copy or symlink. Just hides the file from git (`.git/info/exclude` or `skip-worktree`). Use for files that are machine-specific and don't need syncing, like `debug.log` or local scratch files.

## How Existing Files Are Handled

**`privconf add`** — the file already exists locally (you're adding it for the first time):

1. **Already a correct symlink** — skipped, no action.
2. **Regular file or directory** — removed (content is already copied to the store), then symlink created. No backup needed since the store has the content.

**`privconf link`** — the file exists locally but differs from the store (e.g. syncing from another device):

1. **Already a correct symlink** — skipped, no action.
2. **Regular file or directory** — backed up as `<name>.privconf.bak`, then symlink created. The backup is added to `.git/info/exclude`.

**`privconf remove` / `privconf unlink`** — reversing the operation:

1. **Backup exists** — restored from backup (your local changes preserved).
2. **No backup, but file was tracked by git** — restored via `git checkout HEAD -- <file>`.
3. **No backup, untracked** — file is simply removed (the store still has the content).

## How Files Are Hidden from Git

- **Untracked files** (not in git): added to `.git/info/exclude`
- **Tracked files** (committed to git): `git update-index --skip-worktree`
- **Backup files** (`*.privconf.bak`): also added to `.git/info/exclude`

## License

MIT
