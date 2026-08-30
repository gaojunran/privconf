# Repository Atlas: privconf

## Project Responsibility

`privconf` is a private config manager for project-specific files. It is a single-binary Rust CLI (edition 2024) that stores private/local config files (secrets, machine-specific settings, untracked state) in a central version-controlled store and symlinks them into project working directories on demand. The store itself is a git repository, so private files sync across machines via `git push`/`pull` while never appearing in the host project's own tree.

## System Entry Points

- `src/main.rs` — binary entry point. Defines the `Cli` and `Commands` usage-rs types, parses argv, and dispatches to `cmd::<subcommand>::run(...)`.
- `Cargo.toml` — manifest. `name = "privconf"`, `edition = "2024"`, single binary target. Dependencies: `usage` (usage-rs derive), `anyhow`, `serde`, `toml`, `glob`, `shellexpand`, `dirs`, `tempfile`.
- `tests/e2e.rs` — end-to-end test harness driving the compiled binary against temp directories.
- `README.md` — user-facing command reference, examples, and changelog.

## Core Concepts

- **Store directory** — resolved by `config::store_dir()`: the `PRIVCONF_DIR` env var, else `~/.privconf`. Contains `config.toml` (user-managed project definitions), `state.toml` (runtime link state), and `projects/<name>/` (stored file copies).
- **Project** — an entry in `config.toml` identified by name, matched to a working directory by git remote (`match_remote`) or path glob (`match_path`).
- **Link** — a Unix symlink from the store into the working tree, recorded in `state.toml` with git exclude/skip-worktree so the host project ignores the file.
- **Ignore** — registers a file as ignored for a project without symlinking; manipulates git state only.
- **Sync** — `git pull --rebase` / `add` / `commit` / `push` inside the store directory.

## Directory Map (Aggregated)

| Directory | Responsibility Summary | Detailed Map |
|-----------|------------------------|--------------|
| `src/` | Crate root. CLI entry point (`main.rs`), shared config/state data model and persistence (`config.rs`), ANSI styling helpers (`style.rs`). | [View Map](src/codemap.md) |
| `src/cmd/` | Command-handler layer. One module per clap subcommand, each exporting `pub fn run(...) -> anyhow::Result<()>`. | [View Map](src/cmd/codemap.md) |

## Architecture at a Glance

```
main.rs  ──parse──>  Commands enum  ──match──>  cmd::<sub>::run(...)
                                                      │
                                                      ▼
                                          config.rs (Config / State)
                                                      │
                                    load_* / save_* / link_file / ignore_file
                                                      │
                                                      ▼
                                  ~/.privconf/{config.toml, state.toml, projects/}
                                                      │
                                                      ▼
                                          git (store repo) + symlinks
```

- **Dispatch**: static `match` on a clap-derived `Commands` enum. No trait objects, no registry.
- **Error handling**: `anyhow::Result` throughout. No custom error types.
- **Persistence**: TOML via `serde` + `toml`. Read-on-demand, no caching.
- **Matching**: `shellexpand::tilde` + `glob::Pattern` for path globs; `git remote get-url origin` for remote matching.

## Subcommands

| Command | Purpose |
|---------|---------|
| `init` | Bootstrap the store, optionally clone a remote, run `git init`. |
| `add` | Add files to a project, copy into store, symlink into cwd. |
| `remove` | Remove files from a project, unlink, delete store copies. |
| `link` | Symlink all files for the cwd's project; apply ignore rules. |
| `unlink` | Remove symlinks/ignore rules for the cwd's project. |
| `status` | Show which project matches cwd and file link/ignore state. |
| `sync` | Pull, commit, and push the store via git. |
| `list` | Print all projects and their file/ignore counts. |
| `hook` | Print a shell hook (bash/zsh/fish) for auto-link on cd. |
| `ignore` | Register files as ignored without symlinking. |

## Conventions

- Every mutating subcommand follows: `ensure_initialized` -> `load_config`/`load_state` -> mutate -> `save_config`/`save_state`.
- Project name defaults to the git remote's last path segment (minus `.git`) when not given on the command line.
- Terminal output uses the `style.rs` helpers (`green`, `check`, `cross`, `arrow`, `dim`) for consistent formatting.

## Build and Test

- Build: `cargo build`
- Test: `cargo test`
- Lint: `cargo clippy -- -D warnings`
- Release: bump `Cargo.toml` version, commit, tag `vX.Y.Z`, push.
