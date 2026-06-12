# privconf

Private config manager for project-specific files. Sync `mise.local.toml`, `.env`, scripts, and other private files across devices via a separate git repo, using symlinks to deploy into project directories while keeping `git status` clean.

## How It Works

1. **Store**: A dedicated git repo (`~/.privconf/`) holds all your private config files, organized by project name.
2. **Symlink**: `privconf link` creates symlinks from your project directory to the store. Edits sync immediately — no bidirectional sync logic needed.
3. **Invisible**: Untracked files are added to `.git/info/exclude`; tracked files use `git update-index --skip-worktree`. Your `git status` stays clean.
4. **Auto-link**: Shell hook runs `privconf link --quiet` on every `cd`, so you never think about it.

## Install

```bash
cargo install --git https://github.com/gaojunran/privconf
```

## Quick Start

```bash
# Initialize the store
privconf init

# Add private files from current project
cd ~/Projects/myproj
privconf add myproj mise.local.toml .env.local

# Link files (creates symlinks)
privconf link

# Check status
privconf status

# Unlink (restores original files)
privconf unlink

# Sync store with remote
privconf sync
```

## Shell Hook

Auto-link on `cd`:

```bash
# Bash — add to ~/.bashrc
eval "$(privconf hook bash)"

# Zsh — add to ~/.zshrc
eval "$(privconf hook zsh)"

# Fish — add to ~/.config/fish/conf.d/privconf.fish
privconf hook fish > ~/.config/fish/conf.d/privconf.fish
```

## Commands

| Command | Description |
|---------|-------------|
| `init` | Initialize privconf store at `~/.privconf/` |
| `add <name> <files...>` | Add files from current project to store |
| `link [--quiet]` | Create symlinks and hide from git |
| `unlink` | Remove symlinks and restore original files |
| `status` | Show link status for current directory |
| `sync` | Pull, commit, and push the store repo |
| `hook <bash\|zsh\|fish>` | Print shell hook script |

## Configuration

The store lives at `~/.privconf/` by default. Override with `PRIVCONF_DIR`:

```bash
export PRIVCONF_DIR=/path/to/custom/store
```

### `config.toml`

Projects are matched by git remote URL or path glob:

```toml
[[project]]
name = "myproj"
match_remote = "git@github.com:myco/myproj.git"
files = ["mise.local.toml", ".env.local"]

[[project]]
name = "work"
match_path = "~/Projects/work/*"
files = [".env", "scripts/deploy.sh"]
```

### `state.toml`

Tracks linked files (auto-managed):

```toml
[[linked]]
project = "myproj"
file = "mise.local.toml"
target = "/home/user/Projects/myproj/mise.local.toml"
skip_worktree = false
```

## How Files Are Hidden

- **Untracked files** (not in git): added to `.git/info/exclude`
- **Tracked files** (committed to git): `git update-index --skip-worktree`
- **Backup files** (`*.privconf.bak`): also added to `.git/info/exclude`

On `unlink`, original files are restored from backup or `git checkout HEAD`.

## License

MIT
