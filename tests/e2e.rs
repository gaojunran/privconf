use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct TestEnv {
    root: tempfile::TempDir,
    bin: String,
}

impl TestEnv {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("create temp dir");
        let bin = std::env::var("PRIVCONF_BIN")
            .unwrap_or_else(|_| format!("{}/target/debug/privconf", std::env::var("CARGO_MANIFEST_DIR").unwrap()));
        Self { root, bin }
    }

    fn store_dir(&self) -> &Path {
        self.root.path()
    }

    fn project_dir(&self, name: &str) -> PathBuf {
        self.store_dir().join("projects").join(name)
    }

    fn privconf(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(&self.bin);
        cmd.args(args).env("PRIVCONF_DIR", self.store_dir());
        cmd
    }

    fn git(&self, args: &[&str], cwd: &Path) -> Command {
        let mut cmd = Command::new("git");
        cmd.args(args).current_dir(cwd);
        cmd
    }

    fn create_git_repo(&self, name: &str, remote: Option<&str>) -> PathBuf {
        let repo = self.root.path().join("repos").join(name);
        fs::create_dir_all(&repo).unwrap();
        self.git(&["init"], &repo).assert_success();
        self.git(&["config", "user.name", "test"], &repo).assert_success();
        self.git(&["config", "user.email", "test@test.com"], &repo).assert_success();
        if let Some(url) = remote {
            self.git(&["remote", "add", "origin", url], &repo).assert_success();
        }
        self.git(&["commit", "--allow-empty", "-m", "init"], &repo).assert_success();
        repo
    }
}

trait CommandExt {
    fn assert_success(&mut self) -> std::process::Output;
    fn assert_failure(&mut self) -> std::process::Output;
}

impl CommandExt for Command {
    fn assert_success(&mut self) -> std::process::Output {
        let output = self.output().unwrap();
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!("command failed: {stderr}");
        }
        output
    }

    fn assert_failure(&mut self) -> std::process::Output {
        let output = self.output().unwrap();
        assert!(!output.status.success(), "expected command to fail but it succeeded");
        output
    }
}

#[test]
fn test_init_creates_store() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let store = env.store_dir();
    assert!(store.exists());
    assert!(store.join("config.toml").exists());
    assert!(store.join("state.toml").exists());
    assert!(store.join("projects").exists());
    assert!(store.join(".git").exists());
}

#[test]
fn test_init_idempotent() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();
    env.privconf(&["init"]).assert_failure();
}

#[test]
fn test_add_auto_detects_project() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();

    env.privconf(&["add", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    let config: toml::Value = toml::from_str(&fs::read_to_string(env.store_dir().join("config.toml")).unwrap()).unwrap();
    let projects = config.get("project").unwrap().as_array().unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].get("name").unwrap().as_str(), Some("myproj"));
    assert_eq!(projects[0].get("match_remote").unwrap().as_str(), Some("git@github.com:myco/myproj.git"));

    let stored = env.project_dir("myproj").join("mise.local.toml");
    assert!(stored.exists());
    assert_eq!(fs::read_to_string(stored).unwrap(), "node = '22'");

    assert!(repo.join("mise.local.toml").is_symlink());
}

#[test]
fn test_add_with_explicit_project_name() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();

    env.privconf(&["add", "-p", "custom-name", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    let config: toml::Value = toml::from_str(&fs::read_to_string(env.store_dir().join("config.toml")).unwrap()).unwrap();
    let project = &config.get("project").unwrap().as_array().unwrap()[0];
    assert_eq!(project.get("name").unwrap().as_str(), Some("custom-name"));
}

#[test]
fn test_add_appends_to_existing_project() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();
    fs::write(repo.join(".env.local"), "FOO=bar").unwrap();

    env.privconf(&["add", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();
    env.privconf(&["add", ".env.local"])
        .current_dir(&repo)
        .assert_success();

    let config: toml::Value = toml::from_str(&fs::read_to_string(env.store_dir().join("config.toml")).unwrap()).unwrap();
    let project = &config.get("project").unwrap().as_array().unwrap()[0];
    let files = project.get("files").unwrap().as_array().unwrap();
    assert_eq!(files.len(), 2);
}

#[test]
fn test_add_no_remote_requires_project_flag() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", None);
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();

    env.privconf(&["add", "mise.local.toml"])
        .current_dir(&repo)
        .assert_failure();

    env.privconf(&["add", "-p", "myproj", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();
}

#[test]
fn test_add_nonexistent_file_warns() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    let output = env.privconf(&["add", "nonexistent.toml"])
        .current_dir(&repo)
        .assert_success();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("warning"));
}

#[test]
fn test_add_creates_symlink_immediately() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();

    env.privconf(&["add", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    let linked = repo.join("mise.local.toml");
    assert!(linked.is_symlink());
    assert!(linked.read_link().unwrap().starts_with(env.store_dir()));

    let exclude = fs::read_to_string(repo.join(".git/info/exclude")).unwrap();
    assert!(exclude.contains("mise.local.toml"));

    let git_status = env.git(&["status", "--porcelain"], &repo).assert_success();
    assert!(String::from_utf8_lossy(&git_status.stdout).trim().is_empty());
}

#[test]
fn test_add_tracked_file_skip_worktree() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("config.override.toml"), "key = 'value'").unwrap();
    env.git(&["add", "config.override.toml"], &repo).assert_success();
    env.git(&["commit", "-m", "add tracked config"], &repo).assert_success();

    env.privconf(&["add", "config.override.toml"])
        .current_dir(&repo)
        .assert_success();

    let ls_files = env.git(&["ls-files", "-v", "config.override.toml"], &repo).assert_success();
    assert!(String::from_utf8_lossy(&ls_files.stdout).starts_with('S'));

    let git_status = env.git(&["status", "--porcelain"], &repo).assert_success();
    assert!(String::from_utf8_lossy(&git_status.stdout).trim().is_empty());
}

#[test]
fn test_add_removes_original_file() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "original").unwrap();

    env.privconf(&["add", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    assert!(repo.join("mise.local.toml").is_symlink());
    assert!(!repo.join("mise.local.toml.privconf.bak").exists());
    assert_eq!(fs::read_to_string(repo.join("mise.local.toml")).unwrap(), "original");
}

#[test]
fn test_link_idempotent() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();

    env.privconf(&["add", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    env.privconf(&["link"]).current_dir(&repo).assert_success();

    let linked = repo.join("mise.local.toml");
    assert!(linked.is_symlink());
}

#[test]
fn test_link_no_matching_project() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:other/unknown.git"));
    env.privconf(&["link"]).current_dir(&repo).assert_failure();
}

#[test]
fn test_link_quiet_mode() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();

    env.privconf(&["add", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    let output = env.privconf(&["link", "--quiet"])
        .current_dir(&repo)
        .assert_success();

    assert!(String::from_utf8_lossy(&output.stderr).trim().is_empty());
}

#[test]
fn test_unlink_untracked_no_backup() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();

    env.privconf(&["add", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    env.privconf(&["unlink"]).current_dir(&repo).assert_success();

    assert!(!repo.join("mise.local.toml").exists());
    let exclude = fs::read_to_string(repo.join(".git/info/exclude")).unwrap();
    assert!(!exclude.contains("mise.local.toml"));
}

#[test]
fn test_unlink_restores_tracked_from_git() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("config.override.toml"), "original").unwrap();
    env.git(&["add", "config.override.toml"], &repo).assert_success();
    env.git(&["commit", "-m", "add config"], &repo).assert_success();

    env.privconf(&["add", "config.override.toml"])
        .current_dir(&repo)
        .assert_success();

    env.privconf(&["unlink"]).current_dir(&repo).assert_success();

    assert!(repo.join("config.override.toml").exists());
    assert_eq!(fs::read_to_string(repo.join("config.override.toml")).unwrap(), "original");

    let ls_files = env.git(&["ls-files", "-v", "config.override.toml"], &repo).assert_success();
    assert!(String::from_utf8_lossy(&ls_files.stdout).starts_with('H'));
}

#[test]
fn test_unlink_restores_from_backup() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "original").unwrap();

    env.privconf(&["add", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    fs::write(repo.join("mise.local.toml.privconf.bak"), "modified").unwrap();

    env.privconf(&["unlink"]).current_dir(&repo).assert_success();

    assert!(repo.join("mise.local.toml").exists());
    assert_eq!(fs::read_to_string(repo.join("mise.local.toml")).unwrap(), "modified");
    assert!(!repo.join("mise.local.toml.privconf.bak").exists());
}

#[test]
fn test_unlink_no_linked_files() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    env.privconf(&["unlink"]).current_dir(&repo).assert_failure();
}

#[test]
fn test_remove_file() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();

    env.privconf(&["add", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    assert!(repo.join("mise.local.toml").is_symlink());

    env.privconf(&["remove", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    assert!(!repo.join("mise.local.toml").is_symlink());
    assert!(!env.project_dir("myproj").join("mise.local.toml").exists());

    let config: toml::Value = toml::from_str(&fs::read_to_string(env.store_dir().join("config.toml")).unwrap()).unwrap();
    let projects = config.get("project").unwrap().as_array().unwrap();
    assert!(projects.is_empty());
}

#[test]
fn test_remove_with_backup_restore() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "original").unwrap();

    env.privconf(&["add", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    env.privconf(&["unlink"]).current_dir(&repo).assert_success();

    fs::write(repo.join("mise.local.toml"), "modified").unwrap();

    env.privconf(&["link"])
        .current_dir(&repo)
        .assert_success();

    assert!(repo.join("mise.local.toml.privconf.bak").exists());

    env.privconf(&["remove", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    assert!(repo.join("mise.local.toml").exists());
    assert_eq!(fs::read_to_string(repo.join("mise.local.toml")).unwrap(), "modified");
    assert!(!repo.join("mise.local.toml.privconf.bak").exists());
}

#[test]
fn test_remove_directory() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::create_dir_all(repo.join("scripts")).unwrap();
    fs::write(repo.join("scripts/deploy.sh"), "#!/bin/sh\necho deploy").unwrap();

    env.privconf(&["add", "scripts"])
        .current_dir(&repo)
        .assert_success();

    assert!(repo.join("scripts").is_symlink());

    env.privconf(&["remove", "scripts"])
        .current_dir(&repo)
        .assert_success();

    assert!(!repo.join("scripts").is_symlink());
    assert!(!env.project_dir("myproj").join("scripts").exists());
}

#[test]
fn test_remove_partial_project() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();
    fs::write(repo.join(".env.local"), "FOO=bar").unwrap();

    env.privconf(&["add", "mise.local.toml", ".env.local"])
        .current_dir(&repo)
        .assert_success();

    env.privconf(&["remove", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    assert!(!repo.join("mise.local.toml").is_symlink());
    assert!(repo.join(".env.local").is_symlink());

    let config: toml::Value = toml::from_str(&fs::read_to_string(env.store_dir().join("config.toml")).unwrap()).unwrap();
    let project = &config.get("project").unwrap().as_array().unwrap()[0];
    let files = project.get("files").unwrap().as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].as_str(), Some(".env.local"));
}

#[test]
fn test_status_shows_project() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();

    env.privconf(&["add", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    let output = env.privconf(&["status"])
        .current_dir(&repo)
        .assert_success();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("myproj"));
    assert!(stdout.contains("mise.local.toml"));
}

#[test]
fn test_status_no_project() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:other/unknown.git"));
    let output = env.privconf(&["status"])
        .current_dir(&repo)
        .assert_success();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("no project matches"));
}

#[test]
fn test_hook_zsh() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let output = env.privconf(&["hook", "zsh"]).assert_success();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("chpwd_functions"));
    assert!(stdout.contains("privconf link --quiet"));
}

#[test]
fn test_hook_bash() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let output = env.privconf(&["hook", "bash"]).assert_success();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("PROMPT_COMMAND"));
    assert!(stdout.contains("privconf link --quiet"));
}

#[test]
fn test_hook_fish() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let output = env.privconf(&["hook", "fish"]).assert_success();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("__privconf_chpwd_hook"));
    assert!(stdout.contains("on-variable PWD"));
    assert!(stdout.contains("privconf link --quiet"));
}

#[test]
fn test_hook_unsupported_shell() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    env.privconf(&["hook", "powershell"]).assert_failure();
}

#[test]
fn test_full_workflow() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));

    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();
    fs::create_dir_all(repo.join("scripts")).unwrap();
    fs::write(repo.join("scripts/deploy.sh"), "#!/bin/sh\necho deploy").unwrap();
    fs::write(repo.join(".env.local"), "SECRET=abc").unwrap();

    env.git(&["add", "scripts/deploy.sh"], &repo).assert_success();
    env.git(&["commit", "-m", "add deploy script"], &repo).assert_success();

    env.privconf(&["add", "mise.local.toml", "scripts/deploy.sh", ".env.local"])
        .current_dir(&repo)
        .assert_success();

    assert!(repo.join("mise.local.toml").is_symlink());
    assert!(repo.join("scripts/deploy.sh").is_symlink());
    assert!(repo.join(".env.local").is_symlink());

    let git_status = env.git(&["status", "--porcelain"], &repo).assert_success();
    assert!(String::from_utf8_lossy(&git_status.stdout).trim().is_empty());

    let exclude = fs::read_to_string(repo.join(".git/info/exclude")).unwrap();
    assert!(exclude.contains("mise.local.toml"));
    assert!(exclude.contains(".env.local"));

    let ls_files = env.git(&["ls-files", "-v", "scripts/deploy.sh"], &repo).assert_success();
    assert!(String::from_utf8_lossy(&ls_files.stdout).starts_with('S'));

    env.privconf(&["unlink"]).current_dir(&repo).assert_success();

    assert!(!repo.join("mise.local.toml").exists());
    assert!(repo.join("scripts/deploy.sh").exists());
    assert_eq!(fs::read_to_string(repo.join("scripts/deploy.sh")).unwrap(), "#!/bin/sh\necho deploy");
}

#[test]
fn test_match_by_remote() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo1 = env.create_git_repo("proj1", Some("git@github.com:myco/proj1.git"));
    fs::write(repo1.join("mise.local.toml"), "node = '22'").unwrap();
    env.privconf(&["add", "mise.local.toml"])
        .current_dir(&repo1)
        .assert_success();

    let repo2 = env.create_git_repo("proj2", Some("git@github.com:other/proj2.git"));
    env.privconf(&["link"]).current_dir(&repo2).assert_failure();
}

#[test]
fn test_link_subdirectory_file() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::create_dir_all(repo.join("scripts")).unwrap();
    fs::write(repo.join("scripts/deploy.sh"), "#!/bin/sh\necho deploy").unwrap();

    env.privconf(&["add", "scripts/deploy.sh"])
        .current_dir(&repo)
        .assert_success();

    let linked = repo.join("scripts/deploy.sh");
    assert!(linked.is_symlink());
}

#[test]
fn test_link_source_missing_in_store() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();

    env.privconf(&["add", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    fs::remove_file(env.project_dir("myproj").join("mise.local.toml")).unwrap();

    let output = env.privconf(&["link"])
        .current_dir(&repo)
        .assert_success();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("skip"));
}

#[test]
fn test_commands_without_init() {
    let env = TestEnv::new();
    env.privconf(&["link"]).assert_failure();
    env.privconf(&["add", "file"]).assert_failure();
    env.privconf(&["unlink"]).assert_failure();
    env.privconf(&["status"]).assert_failure();
    env.privconf(&["sync"]).assert_failure();
}

#[test]
fn test_add_directory() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::create_dir_all(repo.join("scripts")).unwrap();
    fs::write(repo.join("scripts/deploy.sh"), "#!/bin/sh\necho deploy").unwrap();
    fs::write(repo.join("scripts/build.sh"), "#!/bin/sh\necho build").unwrap();

    env.privconf(&["add", "scripts"])
        .current_dir(&repo)
        .assert_success();

    let stored_dir = env.project_dir("myproj").join("scripts");
    assert!(stored_dir.is_dir());
    assert!(stored_dir.join("deploy.sh").exists());
    assert!(stored_dir.join("build.sh").exists());

    let config: toml::Value = toml::from_str(&fs::read_to_string(env.store_dir().join("config.toml")).unwrap()).unwrap();
    let files = config.get("project").unwrap().as_array().unwrap()[0].get("files").unwrap().as_array().unwrap();
    assert!(files.iter().any(|f| f.as_str() == Some("scripts")));

    assert!(repo.join("scripts").is_symlink());
}

#[test]
fn test_link_directory() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::create_dir_all(repo.join("scripts")).unwrap();
    fs::write(repo.join("scripts/deploy.sh"), "#!/bin/sh\necho deploy").unwrap();

    env.privconf(&["add", "scripts"])
        .current_dir(&repo)
        .assert_success();

    let linked = repo.join("scripts");
    assert!(linked.is_symlink());
    assert!(linked.read_link().unwrap().starts_with(env.store_dir()));
    assert!(linked.join("deploy.sh").exists());

    let exclude = fs::read_to_string(repo.join(".git/info/exclude")).unwrap();
    assert!(exclude.contains("scripts"));
}

#[test]
fn test_link_directory_backs_up_existing() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::create_dir_all(repo.join("scripts")).unwrap();
    fs::write(repo.join("scripts/deploy.sh"), "original").unwrap();

    env.privconf(&["add", "scripts"])
        .current_dir(&repo)
        .assert_success();

    env.privconf(&["unlink"]).current_dir(&repo).assert_success();

    fs::create_dir_all(repo.join("scripts")).unwrap();
    fs::write(repo.join("scripts/deploy.sh"), "modified").unwrap();

    env.privconf(&["link"])
        .current_dir(&repo)
        .assert_success();

    assert!(repo.join("scripts.privconf.bak").is_dir());
    assert_eq!(fs::read_to_string(repo.join("scripts.privconf.bak/deploy.sh")).unwrap(), "modified");
}

#[test]
fn test_unlink_directory_restores_from_backup() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::create_dir_all(repo.join("scripts")).unwrap();
    fs::write(repo.join("scripts/deploy.sh"), "original").unwrap();

    env.privconf(&["add", "scripts"])
        .current_dir(&repo)
        .assert_success();

    env.privconf(&["unlink"]).current_dir(&repo).assert_success();

    fs::create_dir_all(repo.join("scripts")).unwrap();
    fs::write(repo.join("scripts/deploy.sh"), "modified").unwrap();

    env.privconf(&["link"])
        .current_dir(&repo)
        .assert_success();

    env.privconf(&["unlink"]).current_dir(&repo).assert_success();

    assert!(repo.join("scripts").is_dir());
    assert!(!repo.join("scripts").is_symlink());
    assert_eq!(fs::read_to_string(repo.join("scripts/deploy.sh")).unwrap(), "modified");
    assert!(!repo.join("scripts.privconf.bak").exists());
}

#[test]
fn test_add_nested_directory() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::create_dir_all(repo.join("scripts/ci")).unwrap();
    fs::write(repo.join("scripts/deploy.sh"), "#!/bin/sh\necho deploy").unwrap();
    fs::write(repo.join("scripts/ci/test.sh"), "#!/bin/sh\necho test").unwrap();

    env.privconf(&["add", "scripts"])
        .current_dir(&repo)
        .assert_success();

    let stored = env.project_dir("myproj").join("scripts");
    assert!(stored.join("deploy.sh").exists());
    assert!(stored.join("ci/test.sh").exists());
}

#[test]
fn test_remove_with_explicit_project() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();

    env.privconf(&["add", "-p", "custom", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    env.privconf(&["remove", "-p", "custom", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    assert!(!repo.join("mise.local.toml").is_symlink());
    assert!(!env.project_dir("custom").exists());
}

#[test]
fn test_add_multiple_files_at_once() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();
    fs::write(repo.join(".env.local"), "FOO=bar").unwrap();

    env.privconf(&["add", "mise.local.toml", ".env.local"])
        .current_dir(&repo)
        .assert_success();

    assert!(repo.join("mise.local.toml").is_symlink());
    assert!(repo.join(".env.local").is_symlink());

    let config: toml::Value = toml::from_str(&fs::read_to_string(env.store_dir().join("config.toml")).unwrap()).unwrap();
    let files = config.get("project").unwrap().as_array().unwrap()[0].get("files").unwrap().as_array().unwrap();
    assert_eq!(files.len(), 2);
}

#[test]
fn test_add_without_files_creates_project() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));

    env.privconf(&["add"])
        .current_dir(&repo)
        .assert_success();

    let config: toml::Value = toml::from_str(&fs::read_to_string(env.store_dir().join("config.toml")).unwrap()).unwrap();
    let projects = config.get("project").unwrap().as_array().unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].get("name").unwrap().as_str(), Some("myproj"));
    assert_eq!(projects[0].get("match_remote").unwrap().as_str(), Some("git@github.com:myco/myproj.git"));
    assert!(projects[0].get("files").unwrap().as_array().unwrap().is_empty());
}

#[test]
fn test_link_prefers_remote_match_over_path() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();

    env.privconf(&["add", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    let config: toml::Value = toml::from_str(&fs::read_to_string(env.store_dir().join("config.toml")).unwrap()).unwrap();
    let projects = config.get("project").unwrap().as_array().unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].get("match_remote").unwrap().as_str(), Some("git@github.com:myco/myproj.git"));
}

#[test]
fn test_ignore_untracked_file() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("debug.log"), "debug info").unwrap();

    env.privconf(&["ignore", "debug.log"])
        .current_dir(&repo)
        .assert_success();

    let exclude = fs::read_to_string(repo.join(".git/info/exclude")).unwrap();
    assert!(exclude.contains("debug.log"));

    let git_status = env.git(&["status", "--porcelain"], &repo).assert_success();
    assert!(String::from_utf8_lossy(&git_status.stdout).trim().is_empty());

    assert!(!repo.join("debug.log").is_symlink());
    assert_eq!(fs::read_to_string(repo.join("debug.log")).unwrap(), "debug info");
}

#[test]
fn test_ignore_tracked_file_skip_worktree() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("config.override.toml"), "original").unwrap();
    env.git(&["add", "config.override.toml"], &repo).assert_success();
    env.git(&["commit", "-m", "add config"], &repo).assert_success();

    fs::write(repo.join("config.override.toml"), "modified").unwrap();

    env.privconf(&["ignore", "config.override.toml"])
        .current_dir(&repo)
        .assert_success();

    let ls_files = env.git(&["ls-files", "-v", "config.override.toml"], &repo).assert_success();
    assert!(String::from_utf8_lossy(&ls_files.stdout).starts_with('S'));

    let git_status = env.git(&["status", "--porcelain"], &repo).assert_success();
    assert!(String::from_utf8_lossy(&git_status.stdout).trim().is_empty());
}

#[test]
fn test_ignore_saves_to_config() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("debug.log"), "debug info").unwrap();

    env.privconf(&["ignore", "debug.log"])
        .current_dir(&repo)
        .assert_success();

    let config: toml::Value = toml::from_str(&fs::read_to_string(env.store_dir().join("config.toml")).unwrap()).unwrap();
    let project = &config.get("project").unwrap().as_array().unwrap()[0];
    let ignored = project.get("ignored").unwrap().as_array().unwrap();
    assert_eq!(ignored.len(), 1);
    assert_eq!(ignored[0].as_str(), Some("debug.log"));
    assert!(project.get("files").unwrap().as_array().unwrap().is_empty());
}

#[test]
fn test_ignore_multiple_files() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("debug.log"), "debug").unwrap();
    fs::write(repo.join("error.log"), "error").unwrap();

    env.privconf(&["ignore", "debug.log", "error.log"])
        .current_dir(&repo)
        .assert_success();

    let exclude = fs::read_to_string(repo.join(".git/info/exclude")).unwrap();
    assert!(exclude.contains("debug.log"));
    assert!(exclude.contains("error.log"));
}

#[test]
fn test_ignore_appends_to_existing_project() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();
    fs::write(repo.join("debug.log"), "debug").unwrap();

    env.privconf(&["add", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();
    env.privconf(&["ignore", "debug.log"])
        .current_dir(&repo)
        .assert_success();

    let config: toml::Value = toml::from_str(&fs::read_to_string(env.store_dir().join("config.toml")).unwrap()).unwrap();
    let project = &config.get("project").unwrap().as_array().unwrap()[0];
    assert_eq!(project.get("files").unwrap().as_array().unwrap().len(), 1);
    assert_eq!(project.get("ignored").unwrap().as_array().unwrap().len(), 1);
}

#[test]
fn test_remove_ignored_file() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("debug.log"), "debug info").unwrap();

    env.privconf(&["ignore", "debug.log"])
        .current_dir(&repo)
        .assert_success();

    let exclude = fs::read_to_string(repo.join(".git/info/exclude")).unwrap();
    assert!(exclude.contains("debug.log"));

    env.privconf(&["remove", "debug.log"])
        .current_dir(&repo)
        .assert_success();

    let exclude = fs::read_to_string(repo.join(".git/info/exclude")).unwrap();
    assert!(!exclude.contains("debug.log"));

    let config: toml::Value = toml::from_str(&fs::read_to_string(env.store_dir().join("config.toml")).unwrap()).unwrap();
    let projects = config.get("project").unwrap().as_array().unwrap();
    assert!(projects.is_empty());
}

#[test]
fn test_unlink_ignores_ignored_files() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();
    fs::write(repo.join("debug.log"), "debug").unwrap();

    env.privconf(&["add", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();
    env.privconf(&["ignore", "debug.log"])
        .current_dir(&repo)
        .assert_success();

    env.privconf(&["unlink"]).current_dir(&repo).assert_success();

    assert!(!repo.join("mise.local.toml").exists());
    let exclude = fs::read_to_string(repo.join(".git/info/exclude")).unwrap();
    assert!(!exclude.contains("debug.log"));
}

#[test]
fn test_link_processes_ignored_files() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();
    fs::write(repo.join("debug.log"), "debug").unwrap();

    env.privconf(&["add", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();
    env.privconf(&["ignore", "debug.log"])
        .current_dir(&repo)
        .assert_success();

    env.privconf(&["unlink"]).current_dir(&repo).assert_success();

    let exclude = fs::read_to_string(repo.join(".git/info/exclude")).unwrap();
    assert!(!exclude.contains("debug.log"));
    assert!(!exclude.contains("mise.local.toml"));

    env.privconf(&["link"]).current_dir(&repo).assert_success();

    assert!(repo.join("mise.local.toml").is_symlink());
    let exclude = fs::read_to_string(repo.join(".git/info/exclude")).unwrap();
    assert!(exclude.contains("debug.log"));
    assert!(exclude.contains("mise.local.toml"));
}

#[test]
fn test_status_shows_ignored_files() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("debug.log"), "debug").unwrap();

    env.privconf(&["ignore", "debug.log"])
        .current_dir(&repo)
        .assert_success();

    let output = env.privconf(&["status"])
        .current_dir(&repo)
        .assert_success();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("debug.log"));
    assert!(stdout.contains("ignored"));
}

#[test]
fn test_ignore_no_remote_requires_project_flag() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", None);
    fs::write(repo.join("debug.log"), "debug").unwrap();

    env.privconf(&["ignore", "debug.log"])
        .current_dir(&repo)
        .assert_failure();

    env.privconf(&["ignore", "-p", "myproj", "debug.log"])
        .current_dir(&repo)
        .assert_success();
}

#[test]
fn test_ignore_idempotent() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("debug.log"), "debug").unwrap();

    env.privconf(&["ignore", "debug.log"])
        .current_dir(&repo)
        .assert_success();
    env.privconf(&["ignore", "debug.log"])
        .current_dir(&repo)
        .assert_success();

    let config: toml::Value = toml::from_str(&fs::read_to_string(env.store_dir().join("config.toml")).unwrap()).unwrap();
    let ignored = config.get("project").unwrap().as_array().unwrap()[0].get("ignored").unwrap().as_array().unwrap();
    assert_eq!(ignored.len(), 1);
}

#[test]
fn test_list_no_projects() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let output = env.privconf(&["list"]).assert_success();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("no projects"));
}

#[test]
fn test_list_shows_projects() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo1 = env.create_git_repo("proj1", Some("git@github.com:myco/proj1.git"));
    fs::write(repo1.join("mise.local.toml"), "node = '22'").unwrap();
    fs::write(repo1.join(".env.local"), "FOO=bar").unwrap();
    env.privconf(&["add", "mise.local.toml", ".env.local"])
        .current_dir(&repo1)
        .assert_success();

    let repo2 = env.create_git_repo("proj2", Some("git@github.com:myco/proj2.git"));
    fs::write(repo2.join("debug.log"), "debug").unwrap();
    env.privconf(&["ignore", "debug.log"])
        .current_dir(&repo2)
        .assert_success();

    let output = env.privconf(&["list"]).assert_success();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("proj1"));
    assert!(stdout.contains("2 file(s)"));
    assert!(stdout.contains("proj2"));
    assert!(stdout.contains("1 ignored"));
}

#[test]
fn test_list_shows_project_with_both() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();
    fs::write(repo.join("debug.log"), "debug").unwrap();
    env.privconf(&["add", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();
    env.privconf(&["ignore", "debug.log"])
        .current_dir(&repo)
        .assert_success();

    let output = env.privconf(&["list"]).assert_success();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("myproj"));
    assert!(stdout.contains("1 file(s)"));
    assert!(stdout.contains("1 ignored"));
}

#[test]
fn test_list_empty_project() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    env.privconf(&["add"])
        .current_dir(&repo)
        .assert_success();

    let output = env.privconf(&["list"]).assert_success();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("myproj"));
    assert!(!stdout.contains("file(s)"));
    assert!(!stdout.contains("ignored"));
}

#[test]
fn test_init_clone_from_remote() {
    let env = TestEnv::new();

    let remote_store = env.root.path().join("remote-store");
    fs::create_dir_all(remote_store.join("projects")).unwrap();
    fs::write(remote_store.join("config.toml"), "[[project]]\nname = \"myproj\"\nmatch_remote = \"git@github.com:myco/myproj.git\"\nfiles = [\"mise.local.toml\"]\n").unwrap();
    fs::write(remote_store.join("state.toml"), "").unwrap();
    let git = |args: &[&str]| -> std::process::Output {
        let mut cmd = Command::new("git");
        cmd.args(args).current_dir(&remote_store);
        cmd.output().unwrap()
    };
    git(&["init"]);
    git(&["config", "user.name", "test"]);
    git(&["config", "user.email", "test@test.com"]);
    git(&["add", "-A"]);
    git(&["commit", "-m", "init"]);

    let clone_url = remote_store.to_string_lossy().to_string();

    env.privconf(&["init", &clone_url]).assert_success();

    let store = env.store_dir();
    assert!(store.join("config.toml").exists());
    assert!(store.join("state.toml").exists());
    assert!(store.join("projects").exists());
    assert!(store.join(".git").exists());

    let config: toml::Value = toml::from_str(&fs::read_to_string(store.join("config.toml")).unwrap()).unwrap();
    let projects = config.get("project").unwrap().as_array().unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].get("name").unwrap().as_str(), Some("myproj"));
}

#[test]
fn test_init_clone_creates_missing_files() {
    let env = TestEnv::new();

    let remote_store = env.root.path().join("remote-store");
    fs::create_dir_all(remote_store.join("projects")).unwrap();
    let git = |args: &[&str]| -> std::process::Output {
        let mut cmd = Command::new("git");
        cmd.args(args).current_dir(&remote_store);
        cmd.output().unwrap()
    };
    git(&["init"]);
    git(&["config", "user.name", "test"]);
    git(&["config", "user.email", "test@test.com"]);
    git(&["commit", "--allow-empty", "-m", "init"]);

    let clone_url = remote_store.to_string_lossy().to_string();

    env.privconf(&["init", &clone_url]).assert_success();

    let store = env.store_dir();
    assert!(store.join("config.toml").exists());
    assert!(store.join("state.toml").exists());
    assert!(store.join("projects").exists());
}

#[test]
fn test_init_clone_fails_on_bad_remote() {
    let env = TestEnv::new();

    env.privconf(&["init", "file:///nonexistent/path"])
        .assert_failure();
}

#[test]
fn test_init_clone_idempotent_check() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    env.privconf(&["init", "file:///some/remote"])
        .assert_failure();
}

#[test]
fn test_add_in_worktree() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let main_repo = env.create_git_repo("main", Some("git@github.com:myco/myproj.git"));
    fs::write(main_repo.join("README.md"), "# project").unwrap();
    env.git(&["add", "README.md"], &main_repo).assert_success();
    env.git(&["commit", "-m", "add readme"], &main_repo).assert_success();

    let worktree = env.root.path().join("repos").join("worktree");
    env.git(&["worktree", "add", &worktree.to_string_lossy()], &main_repo).assert_success();

    fs::write(worktree.join("debug.log"), "debug info").unwrap();

    env.privconf(&["add", "debug.log"])
        .current_dir(&worktree)
        .assert_success();

    assert!(worktree.join("debug.log").is_symlink());

    let git_common_dir_output = env.git(&["rev-parse", "--git-common-dir"], &worktree).assert_success();
    let git_common_dir = String::from_utf8_lossy(&git_common_dir_output.stdout).trim().to_string();
    let git_common_dir_path = if std::path::Path::new(&git_common_dir).is_absolute() {
        std::path::PathBuf::from(&git_common_dir)
    } else {
        worktree.join(&git_common_dir)
    };
    let exclude = fs::read_to_string(git_common_dir_path.join("info").join("exclude")).unwrap();
    assert!(exclude.contains("debug.log"));

    let git_status = env.git(&["status", "--porcelain"], &worktree).assert_success();
    assert!(String::from_utf8_lossy(&git_status.stdout).trim().is_empty());
}

#[test]
fn test_ignore_in_worktree() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let main_repo = env.create_git_repo("main2", Some("git@github.com:myco/myproj2.git"));
    fs::write(main_repo.join("config.override.toml"), "original").unwrap();
    env.git(&["add", "config.override.toml"], &main_repo).assert_success();
    env.git(&["commit", "-m", "add config"], &main_repo).assert_success();

    let worktree = env.root.path().join("repos").join("worktree2");
    env.git(&["worktree", "add", &worktree.to_string_lossy()], &main_repo).assert_success();

    fs::write(worktree.join("config.override.toml"), "modified").unwrap();

    env.privconf(&["ignore", "config.override.toml"])
        .current_dir(&worktree)
        .assert_success();

    let ls_files = env.git(&["ls-files", "-v", "config.override.toml"], &worktree).assert_success();
    assert!(String::from_utf8_lossy(&ls_files.stdout).starts_with('S'));

    let git_status = env.git(&["status", "--porcelain"], &worktree).assert_success();
    assert!(String::from_utf8_lossy(&git_status.stdout).trim().is_empty());
}

#[test]
fn test_unlink_in_worktree() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let main_repo = env.create_git_repo("main3", Some("git@github.com:myco/myproj3.git"));
    fs::write(main_repo.join("README.md"), "# project").unwrap();
    env.git(&["add", "README.md"], &main_repo).assert_success();
    env.git(&["commit", "-m", "add readme"], &main_repo).assert_success();

    let worktree = env.root.path().join("repos").join("worktree3");
    env.git(&["worktree", "add", &worktree.to_string_lossy()], &main_repo).assert_success();

    fs::write(worktree.join("debug.log"), "debug info").unwrap();

    env.privconf(&["add", "debug.log"])
        .current_dir(&worktree)
        .assert_success();

    assert!(worktree.join("debug.log").is_symlink());

    env.privconf(&["unlink"])
        .current_dir(&worktree)
        .assert_success();

    assert!(!worktree.join("debug.log").exists());

    let git_common_dir_output = env.git(&["rev-parse", "--git-common-dir"], &worktree).assert_success();
    let git_common_dir = String::from_utf8_lossy(&git_common_dir_output.stdout).trim().to_string();
    let git_common_dir_path = if std::path::Path::new(&git_common_dir).is_absolute() {
        std::path::PathBuf::from(&git_common_dir)
    } else {
        worktree.join(&git_common_dir)
    };
    let exclude = fs::read_to_string(git_common_dir_path.join("info").join("exclude")).unwrap();
    assert!(!exclude.contains("debug.log"));
}

#[test]
fn test_add_preserves_executable_permission() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    let script = repo.join("deploy.sh");
    fs::write(&script, "#!/bin/sh\necho deploy").unwrap();

    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

    env.privconf(&["add", "deploy.sh"])
        .current_dir(&repo)
        .assert_success();

    let store_file = env.project_dir("myproj").join("deploy.sh");
    assert!(store_file.exists());
    let mode = store_file.metadata().unwrap().permissions().mode();
    assert_eq!(mode & 0o111, 0o111, "executable bit should be preserved");
}

#[test]
fn test_add_directory_preserves_executable_permission() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::create_dir_all(repo.join("scripts")).unwrap();
    let script = repo.join("scripts/deploy.sh");
    fs::write(&script, "#!/bin/sh\necho deploy").unwrap();

    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

    env.privconf(&["add", "scripts"])
        .current_dir(&repo)
        .assert_success();

    let store_file = env.project_dir("myproj").join("scripts/deploy.sh");
    assert!(store_file.exists());
    let mode = store_file.metadata().unwrap().permissions().mode();
    assert_eq!(mode & 0o111, 0o111, "executable bit should be preserved in directory copy");
}

#[test]
fn test_sync_no_changes_skips_commit() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let output = env.privconf(&["sync"]).assert_success();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no changes to commit") || stderr.contains("no remote configured"));
}

#[test]
fn test_sync_no_remote_skips_push() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("config.local"), "key = value").unwrap();
    env.privconf(&["add", "config.local"])
        .current_dir(&repo)
        .assert_success();

    let output = env.privconf(&["sync"]).assert_success();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no remote configured"));
}

#[test]
fn test_sync_with_custom_message() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("config.local"), "key = value").unwrap();
    env.privconf(&["add", "config.local"])
        .current_dir(&repo)
        .assert_success();

    env.privconf(&["sync", "--message", "custom commit msg"]).assert_success();

    let store = env.store_dir();
    let log = env.git(&["log", "--oneline", "-1"], store).assert_success();
    let msg = String::from_utf8_lossy(&log.stdout).trim().to_string();
    assert!(msg.contains("custom commit msg"));
}

#[test]
fn test_sync_dry_run() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("config.local"), "key = value").unwrap();
    env.privconf(&["add", "config.local"])
        .current_dir(&repo)
        .assert_success();

    let output = env.privconf(&["sync", "--dry-run"]).assert_success();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("would stage and commit"));

    let store = env.store_dir();
    let log_before = env.git(&["log", "--oneline"], store).assert_success();
    let log_after = env.git(&["log", "--oneline"], store).assert_success();
    assert_eq!(
        String::from_utf8_lossy(&log_before.stdout),
        String::from_utf8_lossy(&log_after.stdout),
        "dry-run should not create commits"
    );
}

#[test]
fn test_backup_path_multi_extension() {
    let path = std::path::PathBuf::from("foo.tar.gz");
    let bak = crate_test_backup_path(&path);
    assert!(bak.to_string_lossy().ends_with("foo.tar.gz.privconf.bak"), "backup path was: {}", bak.display());
}

fn crate_test_backup_path(path: &std::path::Path) -> std::path::PathBuf {
    let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let parent = path.parent();
    let mut new_name = file_name;
    new_name.push_str(".privconf.bak");
    match parent {
        Some(p) => p.join(new_name),
        None => std::path::PathBuf::from(new_name),
    }
}
