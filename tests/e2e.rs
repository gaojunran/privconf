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
fn test_add_creates_project() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();

    env.privconf(&["add", "myproj", "mise.local.toml"])
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
}

#[test]
fn test_add_appends_to_existing_project() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();
    fs::write(repo.join(".env.local"), "FOO=bar").unwrap();

    env.privconf(&["add", "myproj", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();
    env.privconf(&["add", "myproj", ".env.local"])
        .current_dir(&repo)
        .assert_success();

    let config: toml::Value = toml::from_str(&fs::read_to_string(env.store_dir().join("config.toml")).unwrap()).unwrap();
    let project = &config.get("project").unwrap().as_array().unwrap()[0];
    let files = project.get("files").unwrap().as_array().unwrap();
    assert_eq!(files.len(), 2);
}

#[test]
fn test_add_no_remote() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", None);
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();

    env.privconf(&["add", "myproj", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    let config: toml::Value = toml::from_str(&fs::read_to_string(env.store_dir().join("config.toml")).unwrap()).unwrap();
    let project = &config.get("project").unwrap().as_array().unwrap()[0];
    assert!(project.get("match_remote").is_none() || project.get("match_remote").unwrap().as_str().is_none());
}

#[test]
fn test_add_nonexistent_file_warns() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", None);
    let output = env.privconf(&["add", "myproj", "nonexistent.toml"])
        .current_dir(&repo)
        .assert_success();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("warning"));
}

#[test]
fn test_link_untracked_file() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();

    env.privconf(&["add", "myproj", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    fs::remove_file(repo.join("mise.local.toml")).unwrap();

    env.privconf(&["link"])
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
fn test_link_tracked_file_skip_worktree() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("config.override.toml"), "key = 'value'").unwrap();
    env.git(&["add", "config.override.toml"], &repo).assert_success();
    env.git(&["commit", "-m", "add tracked config"], &repo).assert_success();

    env.privconf(&["add", "myproj", "config.override.toml"])
        .current_dir(&repo)
        .assert_success();

    env.privconf(&["link"])
        .current_dir(&repo)
        .assert_success();

    let ls_files = env.git(&["ls-files", "-v", "config.override.toml"], &repo).assert_success();
    assert!(String::from_utf8_lossy(&ls_files.stdout).starts_with('S'));

    let git_status = env.git(&["status", "--porcelain"], &repo).assert_success();
    assert!(String::from_utf8_lossy(&git_status.stdout).trim().is_empty());
}

#[test]
fn test_link_backs_up_existing_file() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "original").unwrap();

    env.privconf(&["add", "myproj", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    fs::write(repo.join("mise.local.toml"), "modified").unwrap();

    env.privconf(&["link"])
        .current_dir(&repo)
        .assert_success();

    assert!(repo.join("mise.local.privconf.bak").exists());
    assert_eq!(fs::read_to_string(repo.join("mise.local.privconf.bak")).unwrap(), "modified");
}

#[test]
fn test_link_idempotent() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();

    env.privconf(&["add", "myproj", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    fs::remove_file(repo.join("mise.local.toml")).unwrap();

    env.privconf(&["link"]).current_dir(&repo).assert_success();
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

    env.privconf(&["add", "myproj", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    fs::remove_file(repo.join("mise.local.toml")).unwrap();

    let output = env.privconf(&["link", "--quiet"])
        .current_dir(&repo)
        .assert_success();

    assert!(String::from_utf8_lossy(&output.stderr).trim().is_empty());
}

#[test]
fn test_unlink_restores_untracked() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();

    env.privconf(&["add", "myproj", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    fs::remove_file(repo.join("mise.local.toml")).unwrap();
    env.privconf(&["link"]).current_dir(&repo).assert_success();
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

    env.privconf(&["add", "myproj", "config.override.toml"])
        .current_dir(&repo)
        .assert_success();

    env.privconf(&["link"]).current_dir(&repo).assert_success();
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

    env.privconf(&["add", "myproj", "mise.local.toml"])
        .current_dir(&repo)
        .assert_success();

    fs::write(repo.join("mise.local.toml"), "modified").unwrap();

    env.privconf(&["link"]).current_dir(&repo).assert_success();
    env.privconf(&["unlink"]).current_dir(&repo).assert_success();

    assert!(repo.join("mise.local.toml").exists());
    assert_eq!(fs::read_to_string(repo.join("mise.local.toml")).unwrap(), "modified");
    assert!(!repo.join("mise.local.privconf.bak").exists());
}

#[test]
fn test_unlink_no_linked_files() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    env.privconf(&["unlink"]).current_dir(&repo).assert_failure();
}

#[test]
fn test_status_shows_project() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();

    env.privconf(&["add", "myproj", "mise.local.toml"])
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

    env.privconf(&["add", "myproj", "mise.local.toml", "scripts/deploy.sh", ".env.local"])
        .current_dir(&repo)
        .assert_success();

    fs::remove_file(repo.join("mise.local.toml")).unwrap();
    fs::remove_file(repo.join(".env.local")).unwrap();

    env.privconf(&["link"]).current_dir(&repo).assert_success();

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
    env.privconf(&["add", "proj1", "mise.local.toml"])
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

    env.privconf(&["add", "myproj", "scripts/deploy.sh"])
        .current_dir(&repo)
        .assert_success();

    fs::remove_dir_all(repo.join("scripts")).unwrap();

    env.privconf(&["link"]).current_dir(&repo).assert_success();

    let linked = repo.join("scripts/deploy.sh");
    assert!(linked.is_symlink());
}

#[test]
fn test_link_source_missing_in_store() {
    let env = TestEnv::new();
    env.privconf(&["init"]).assert_success();

    let repo = env.create_git_repo("myproj", Some("git@github.com:myco/myproj.git"));
    fs::write(repo.join("mise.local.toml"), "node = '22'").unwrap();

    env.privconf(&["add", "myproj", "mise.local.toml"])
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
    env.privconf(&["add", "test", "file"]).assert_failure();
    env.privconf(&["unlink"]).assert_failure();
    env.privconf(&["status"]).assert_failure();
    env.privconf(&["sync"]).assert_failure();
}
