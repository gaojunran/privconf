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
privconf sync -m "update secrets"     # custom commit message
privconf sync --dry-run               # preview without making changes
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
| `init [<remote>] [--no-git]` | Initialize privconf store at `~/.privconf/`. With a remote URL, clone an existing store repo instead. `--no-git` skips git init for embedding inside an existing repo. |
| `add [-p <name>] [-a] [files...]` | Add files/dirs to store and create symlinks. Project name auto-detected from git remote. Omit files to create project only. `-a`/`--all` adds to the `[all]` block (applies to every project). |
| `ignore [-p <name>] [-a] <files...>` | Ignore files in current project (add to `.git/info/exclude` or `skip-worktree`, no symlink, no store copy). `-a`/`--all` adds to the `[all]` block. |
| `remove [-p <name>] [-a] <files...>` | Remove files from store, remove symlinks, restore originals. `-a`/`--all` removes from the `[all]` block. |
| `link [-s] [-q]` | Rebuild symlinks for current project. `--sync` / `-s` pulls store first. `--quiet` / `-q` suppresses output. |
| `unlink` | Remove all symlinks and restore original files |
| `status` | Show link status for current directory |
| `sync [-m <msg>] [--dry-run]` | Pull, commit, and push the store repo. `--message` / `-m` sets commit message (default: "sync"). `--dry-run` previews changes without committing. |
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
# Files/ignored that apply to ALL projects.
# Linked on every `privconf link`, regardless of which project matches.
[all]
files = [".editorconfig", ".envrc"]
ignored = ["*.log"]

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

## `--all` Flag (Applies to Every Project)

`add --all`, `ignore --all`, and `remove --all` operate on a separate `[all]` block in `config.toml` instead of a specific project. Files in the `[all]` block are:

- **Linked on every `privconf link`** — no matter which project matches the current directory, the all-files are symlinked in alongside the project's own files.
- **Applied to new projects automatically** — when you create a new project later, `privconf link` picks up the all-files without any extra setup.
- **Stored under `projects/__all__/`** in the privconf store.

```bash
privconf add --all .editorconfig .envrc    # link these in every project
privconf ignore --all "*.log"              # ignore these in every project
privconf remove --all .envrc               # remove from the all block
```

If a file appears in both `[all]` and a specific project, the project-specific entry takes precedence (the symlink points to the project's copy, not the all copy).

### `state.toml`

Tracks linked files (auto-managed, don't edit manually):

```toml
[[linked]]
project = "myproj"
file = "mise.local.toml"
target = "/home/user/Projects/myproj/mise.local.toml"
skip_worktree = false
```

## Embedding in a Dotfile Repo

If you already manage your dotfiles with a tool like [chezmoi](https://chezmoi.io), you can embed the privconf store as a subdirectory instead of maintaining a separate git repo:

```bash
# Persist PRIVCONF_DIR to point inside your dotfile repo
echo 'export PRIVCONF_DIR="$HOME/.local/share/chezmoi/dot_config/privconf"' >> ~/.bashrc
source ~/.bashrc

# Initialize without git (the outer repo handles version control)
cd "$PRIVCONF_DIR" && privconf init --no-git

# Add config files from your projects
cd ~/Projects/myproj && privconf add mise.local.toml .env

# Commit and push via your dotfile repo
cd "$HOME/.local/share/chezmoi" && git add -A && git commit -m "add privconf" && git push
```

`--no-git` skips `git init` and the initial commit, but still creates `.gitignore` so that `state.toml` (machine-specific) is excluded from the outer repo.

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

## Git Worktree Support

privconf correctly handles git worktrees by writing to the shared `info/exclude` (resolved via `git rev-parse --git-common-dir`), not the worktree-specific git directory.

## Jujutsu (jj) Limitations

privconf relies on `git update-index --skip-worktree` to hide local changes to **tracked** files (so replacing a tracked file with a symlink stays invisible to `git status`). [Jujutsu (jj)](https://github.com/jj-vcs/jj) does not implement the skip-worktree bit — its working copy is itself a commit, with no Git index layer — so jj will still report a tracked file replaced by a symlink as a type change (`F → L`), even though `git status` is clean in a colocated repo.

What works and what doesn't under jj:

| File type | `privconf add` / `ignore` | `git status` | `jj status` |
|-----------|---------------------------|--------------|-------------|
| **Untracked** (not in git) | writes `.git/info/exclude` | clean | clean (jj respects `info/exclude`) |
| **Tracked** (committed) | sets `skip-worktree` | clean | **dirty** (jj ignores skip-worktree) |

For tracked files under jj, there is no way for privconf to make the symlink replacement invisible. The jj-native approach is to keep local overrides in a separate private commit (merged into your working branch) and use [`git.private-commits`](https://docs.jj-vcs.dev/latest/config/#gitprivate-commits) to prevent pushing it. See the [jj FAQ](https://docs.jj-vcs.dev/latest/FAQ/#how-can-i-avoid-committing-my-local-only-changes-to-tracked-files) for details.

## Changelog

### v0.15.0

- **`--all` flag for `add`/`ignore`/`remove`**: files in the `[all]` block apply to every project. `privconf link` symlinks them alongside the project's own files, so new projects automatically inherit them. Project-specific entries override `[all]` entries when both exist.

### v0.14.0

- **Bug fix**: `add`/`ignore`/`unlink`/`remove` now compute git-root-relative paths correctly when run from a subdirectory of the repository. Previously, git commands (`ls-files`, `update-index`, `info/exclude`) received cwd-relative paths, so tracked files in subdirectories did not get `skip-worktree` set and untracked files got the wrong exclude entry, leaving `git status` dirty.

### v0.13.0

- **`init --no-git`**: skip `git init` for embedding the privconf store inside an existing git repo (e.g. chezmoi). `.gitignore` is still created so `state.toml` stays excluded from the outer repo.

### v0.12.0

- **Bug fix**: `link` now correctly handles ignored files for same project in different directories (checks current directory in state entries)
- **Unified output format**: all commands now follow consistent `✓/✗ action file (detail)` pattern
- **Colored CLI output**: status indicators (✓ green, ✗ red, ! yellow, - dim) for clearer visual feedback

### v0.11.0

- **Bug fix**: `link` now re-applies git exclude/skip-worktree for already-linked files and ignored files, fixing the case where a fresh machine has symlinks but git visibility is lost
- **Colored CLI output**: all commands now use colored status indicators (✓ green, ✗ red, dim details) for clearer visual feedback

### v0.10.0

- **Bug fix**: `add .local/` (trailing slash) no longer causes "No such file or directory" — trailing slashes are now stripped from file arguments in `add`, `ignore`, and `remove`

### v0.8.0

- **Bug fix**: `sync` now uses `origin main` explicitly for pull/push, no longer requires tracking branch to be set
- **`init` default branch**: `git init -b main` ensures consistent branch name

### v0.7.0

- **`.gitignore` in store**: `state.toml` is now excluded from git tracking (machine-specific, not meant to sync)

### v0.6.0

- **`.gitignore` in store**: `state.toml` is now excluded from git tracking (machine-specific, not meant to sync)
- **Bug fix**: `.git/info/exclude` entries now always start on a new line (previously could be appended to the last existing line)
- **Bug fix**: `init` now sets `user.name`/`user.email` in the store repo, fixing CI environments where git identity is not configured
- **`init` auto-commits**: initial `config.toml` and `state.toml` are committed automatically

### v0.5.0

- **File permissions preserved**: `add` now preserves executable bits when copying files and directories to the store
- **Sync improvements**:
  - `--dry-run` flag to preview changes without committing
  - `-m` / `--message` flag for custom commit messages
  - Merge conflict detection with clear error message
  - "no remote configured" hint when store has no remote
  - "no changes to commit" when store is clean
- **Git worktree support**: correctly writes to shared `info/exclude` via `--git-common-dir`
- **`init` now commits**: initial `config.toml` and `state.toml` are committed automatically
- **Bug fix**: `backup_path` now appends `.privconf.bak` suffix instead of replacing the last extension (`foo.tar.gz` → `foo.tar.gz.privconf.bak`, not `foo.tar.privconf.bak`)

### v0.4.0

- `privconf list` command
- `privconf init <remote>` to clone from URL
- Skip push when store has no remote
- Check staged changes before commit in sync

## Built With

[oh-my-opencode-slim](https://github.com/alvinunreal/oh-my-opencode-slim) — an agent orchestration plugin for OpenCode.

## License

MIT
