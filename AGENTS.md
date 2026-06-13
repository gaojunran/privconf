# AGENTS.md

## Project

privconf - Private config manager for project-specific files. Rust CLI, edition 2024, uses clap/anyhow/serde/toml.

## Commands

- Build: `cargo build`
- Test: `cargo test`
- Lint: `cargo clippy -- -D warnings`

## Rules

- 功能更新时必须同步更新 README.md（命令表、用法示例、changelog 等）
- 新功能必须添加真实场景的 e2e 测试（`tests/e2e.rs`），跑过 `cargo test` 才能提交
- 有功能更新时需要发新版：bump `Cargo.toml` version → commit → tag `vX.Y.Z` → push
