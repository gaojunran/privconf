# src/cmd/

## Responsibility

The `src/cmd/` directory contains the implementation of every CLI subcommand. It is the command-handler layer: each module maps one clap-derived subcommand to a concrete operation that reads or mutates the privconf store (`~/.privconf/`), its `config.toml`, its `state.toml`, and the user's project directories. No business logic is shared across commands beyond the conventions of their `run` signatures and the common `config`/`style` utilities.

## Design

**Command Pattern via clap derive.** `src/main.rs` defines a single `Cli` struct with `#[command(subcommand)]` and a `Commands` enum annotated with `#[derive(clap::Subcommand)]`. Each variant carries the arguments for one command. After `Cli::parse()`, `main` matches on `Commands` and calls the corresponding `cmd::<module>::run(...)` function. There is no dynamic dispatch and no shared trait; the binding is static and explicit.

**Module-per-command convention.** Every file in this directory (except `mod.rs`) exports exactly one public entry point:

```rust
pub fn run(...) -> anyhow::Result<()>
```

The parameter list varies per command because clap arguments are passed directly. For example:

- `init::run(remote: Option<&str>, no_git: bool)`
- `add::run(project_name: Option<String>, files: Vec<String>)`
- `link::run(quiet: bool, sync: bool)`
- `sync::run(message: Option<&str>, dry_run: bool)`

**Project name resolution.** Several commands (`add`, `remove`, `ignore`) share an identical idiom: if `project_name` is `None`, derive it from the current directory's git remote via `config::git_root`, `config::derive_project_name`, and `config::get_git_remote_from_root`. This is duplicated inline rather than abstracted into a helper.

**Config/State read-write cycle.** Commands that mutate data follow the same lifecycle:
1. `config::ensure_initialized()?`
2. `config::load_config()?` and/or `config::load_state()?`
3. Mutate the in-memory `Config` or `State`
4. `config::save_config(&config)?` and/or `config::save_state(&state)?`

**Store layout assumptions.** All commands rely on `config::store_dir()` (default `~/.privconf/`), `config::project_dir(name)`, and the fixed filenames `config.toml` and `state.toml`. The `init` command bootstraps this layout, creates a `projects/` subdirectory, writes `.gitignore`, and optionally runs `git init`.

## Flow

1. **Parsing.** `main.rs` invokes `Cli::parse()`. clap populates the `Commands` enum with variant-specific fields.
2. **Dispatch.** A `match cli.command` block maps each variant to the corresponding `cmd::` function, extracting owned fields (e.g., `project`, `files`) and passing them as arguments.
3. **Guard.** Nearly every command calls `config::ensure_initialized()` first, which checks that `store_dir()/config.toml` exists and returns an error if the user has not run `init`.
4. **Execution.** The command loads `Config` and/or `State`, performs filesystem operations (copy, symlink, git shell-outs), and writes results back. For example:
   - `add` copies files into the store, updates `Config.project[].files`, then calls `config::link_file` to create symlinks and update `State.linked`.
   - `link` calls `config::find_project_for_dir` to locate the matching `ProjectEntry`, then iterates over its files calling `config::link_file` and over its ignored files calling `config::ignore_file`.
   - `sync` shells out to `git pull --rebase origin main`, `git add -A`, `git commit`, and `git push origin main` inside the store directory.
   - `remove` looks up `State.linked` entries, calls `config::unlink_file` or `config::unignore_file`, deletes store copies, and prunes empty projects from `Config`.
5. **Return.** Each `run` returns `anyhow::Result<()>`; errors propagate up to `main`, which prints them and exits non-zero.

## Integration

**Consumers:**
- `src/main.rs` — owns the `Cli`/`Commands` clap definitions and dispatches into each `cmd::` module.
- `src/config.rs` — provides `Config`, `State`, `ProjectEntry`, `LinkedEntry`, and all load/save/link/unlink/ignore/git helper functions used by commands.
- `src/style.rs` — ANSI styling helpers (`green`, `bold`, `dim`, `check`, `cross`, `arrow`) used for CLI output.

**Subcommands:**

| Module | Purpose |
|--------|---------|
| `mod.rs` | Module registry; re-exports all command modules. |
| `init.rs` | Bootstrap the privconf store, optionally clone from a remote URL, run `git init`, and write initial `config.toml`/`state.toml`. |
| `add.rs` | Add files or directories to a project, copy them into the store, update `Config`, and symlink them into the current directory. |
| `remove.rs` | Remove files from a project, unlink or unignore them, restore backups, delete from store, and remove empty projects. |
| `link.rs` | Symlink all files and apply ignore rules for the project matching the current directory; optionally sync first. |
| `unlink.rs` | Remove symlinks and ignore rules for all entries in `State.linked` that belong to the current directory. |
| `status.rs` | Display which project matches the current directory and the link/ignore state of each configured file. |
| `sync.rs` | Pull, commit, and push the store directory via git; supports `--dry-run`. |
| `list.rs` | Print all projects and their file/ignore counts. |
| `hook.rs` | Print a shell hook script (bash/zsh/fish) that runs `privconf link --quiet` on directory change. |
| `ignore.rs` | Register files as ignored for a project; updates `Config` and applies git exclude/skip-worktree without creating symlinks. |
