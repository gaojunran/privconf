# src/

## Responsibility

This directory is the crate root of `privconf`, a Rust CLI for managing project-specific private config files. It defines the entry point (`main.rs`), the shared configuration and state layer (`config.rs`), terminal styling helpers (`style.rs`), and declares the `cmd` submodule that houses all subcommand implementations.

## Design

### Entry Point (main.rs)

`main.rs` declares three sibling modules (`cmd`, `config`, `style`) and defines the CLI surface using `usage` derive macros (usage-rs, the Rust compile-time framework from jdx/usage):

- `Cli` — a struct with `#[derive(Cli)]` and a single `command: Commands` field annotated with `#[usage(subcommand)]`.
- `Commands` — an enum with `#[derive(Subcommands)]` whose variants map 1:1 to the supported subcommands: `Init`, `Add`, `Remove`, `Link`, `Unlink`, `Status`, `Sync`, `List`, `Hook`, `Ignore`. Each variant carries the CLI arguments defined with `#[usage(long, short)]` and positional parameters.

`main() -> anyhow::Result<()>` parses arguments with `Cli::parse()` (same exit contract as clap: `-h`/`--help` exits 0, parse errors exit 2) and dispatches via a `match` on `cli.command` to the corresponding `cmd::<subcommand>::run(...)` function. The `main` function itself is the only place where CLI parsing and top-level dispatch occur.

### Configuration and State (config.rs)

`config.rs` owns the data model, persistence, and a set of git-oriented path helpers.

**Storage layout** — The store directory is resolved by `store_dir()`:
- If the `PRIVCONF_DIR` environment variable is set, its value is used verbatim.
- Otherwise it falls back to `dirs::home_dir()` joined with `.privconf`.

Inside the store:
- `config.toml` — user-managed project definitions. Located via `config_path()`.
- `state.toml` — runtime link state. Located via `state_path()`.
- `projects/<name>/` — per-project stored files/directories.

**Core types** —
- `Config` — `#[derive(Debug, serde::Serialize, serde::Deserialize)]` with `#[serde(deny_unknown_fields)]`. Contains a single field `project: Vec<ProjectEntry>` (with `#[serde(default)]`).
- `ProjectEntry` — `name: String`, optional `match_remote: Option<String>`, optional `match_path: Option<String>`, plus `files: Vec<String>` and `ignored: Vec<String>` (both defaulting to empty).
- `State` — `#[derive(Default)]` wrapper around `linked: Vec<LinkedEntry>`.
- `LinkedEntry` — `project: String`, `file: String`, `target: PathBuf`, `skip_worktree: bool`, `ignored: bool`.

**Loading and saving** —
- `load_config()` reads `config.toml` via `std::fs::read_to_string` and deserializes with `toml::from_str`. If the file does not exist it returns `Config { project: vec![] }`. Errors are wrapped with `anyhow::Context`.
- `save_config()` serializes via `toml::to_string_pretty` and writes atomically with `std::fs::write`.
- `load_state()` and `save_state()` follow the same pattern for `state.toml`.

**Matching and resolution** —
- `ProjectEntry::matches_dir` tests a directory against `match_path` (via `shellexpand::tilde` + `glob::Pattern`) and `match_remote` (by querying `git remote get-url origin`).
- `find_project_for_dir(config, dir)` iterates `config.project` to locate the first entry matching the current directory by remote or path.
- `derive_project_name(git_root)` extracts the project name from the git remote URL (last path segment, stripping `.git`).

**Git helpers** — `config.rs` contains a suite of git wrappers used across the crate: `git_root`, `git_common_dir`, `git_is_tracked`, `git_add_to_exclude`, `git_remove_from_exclude`, `git_set_skip_worktree`, `git_unset_skip_worktree`, `rel_to_git_root`.

**Link/unlink/ignore operations** —
- `link_file` creates a Unix symlink from the store into the working directory, optionally backing up the original, and updates git exclude/skip-worktree state.
- `unlink_file` removes the symlink and restores from `.privconf.bak` or git checkout.
- `ignore_file` and `unignore_file` manipulate git state without creating symlinks, recording entries in `State` with `ignored: true`.

### Style Helpers (style.rs)

`style.rs` is a minimal ANSI formatting module. It defines constants for escape codes (`GREEN`, `YELLOW`, `RED`, `DIM`, `BOLD`, `RESET`) and wrapper functions `green`, `yellow`, `red`, `dim`, `bold`, `check`, `cross`, `arrow` that wrap a `&str` and append the reset sequence. Used by `config.rs` and `cmd/` submodules for consistent terminal output.

### Module Hierarchy

```
main.rs
├── cmd/
│   └── mod.rs (declares add, hook, ignore, init, link, list, remove, status, sync, unlink)
├── config.rs
└── style.rs
```

`cmd/` is a directory module referenced by `mod cmd;` in `main.rs`. Subcommand logic lives there and is not detailed here; see `src/cmd/codemap.md`.

## Flow

1. **Startup** — `main()` calls `Cli::parse()`, which uses `clap` derive to parse `std::env::args()` into the `Cli` struct.
2. **Dispatch** — The `Commands` variant is matched and the corresponding `cmd::<name>::run(...)` is invoked directly. There is no intermediate command registry; dispatch is a single `match` expression.
3. **Config load** — Most subcommands call `config::ensure_initialized()` (which asserts `config.toml` exists in the store directory) and then `config::load_config()` / `config::load_state()` as needed.
4. **Execution** — Subcommands perform filesystem and git operations via helpers in `config.rs`, writing results back with `save_config()` or `save_state()`.
5. **Error propagation** — Nearly every fallible function returns `anyhow::Result<...>`. Errors bubble up through the call stack to `main()`, where `anyhow` prints the error chain and exits non-zero. No custom error types are defined.

**Config file I/O** — `config.toml` and `state.toml` are read on demand, not cached globally. Each read/write is a straightforward `read_to_string` / `fs::write` round-trip through `toml`.

## Integration

- `main.rs` — CLI argument parsing and top-level dispatch. References `cmd/`, `config.rs`, `style.rs`.
- `config.rs` — Shared data model (`Config`, `State`, `ProjectEntry`, `LinkedEntry`), persistence, git integration, and link/unlink/ignore primitives. Used by all `cmd/` submodules.
- `style.rs` — Terminal formatting. Used by `config.rs` and `cmd/`.
- `cmd/` — Subcommand implementations. See `src/cmd/codemap.md` for internal structure.

**External crates**:
- `usage` (usage-rs, derive) — CLI parsing. `Cli` and `Commands` use `#[derive(Cli)]` and `#[derive(Subcommands)]`.
- `anyhow` — Error handling. All public functions return `anyhow::Result`. `Context` and `bail!` / `ensure!` are used for error messages.
- `serde` (derive) — Serialization of `Config`, `State`, `ProjectEntry`, `LinkedEntry`.
- `toml` — TOML encoding/decoding for `config.toml` and `state.toml`.
- `glob` — Pattern matching for `ProjectEntry::match_path`.
- `shellexpand` — Expands `~` in path patterns before glob matching.
- `dirs` — Resolves the user's home directory for the default store location.
- `tempfile` — Used within `cmd/` submodules for temporary file operations.
