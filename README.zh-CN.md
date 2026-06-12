# privconf

项目私有配置文件管理器。通过独立的 git 仓库同步 `mise.local.toml`、`.env`、脚本等私有文件，使用符号链接部署到项目目录，同时保持 `git status` 干净。

## 工作原理

1. **仓库**：独立的 git 仓库（`~/.privconf/`）按项目名存放所有私有配置文件。
2. **链接**：`privconf link` 在项目目录创建指向仓库的符号链接。编辑即时同步，无需双向同步逻辑。
3. **无感**：未跟踪文件加入 `.git/info/exclude`；已跟踪文件使用 `git update-index --skip-worktree`。`git status` 始终干净。
4. **自动链接**：Shell hook 在每次 `cd` 时自动运行 `privconf link --quiet`，完全无感。

## 安装

```bash
cargo install --git https://github.com/gaojunran/privconf
```

## 快速开始

```bash
# 初始化仓库
privconf init

# 从当前项目添加私有文件
cd ~/Projects/myproj
privconf add myproj mise.local.toml .env.local

# 链接文件（创建符号链接）
privconf link

# 查看状态
privconf status

# 取消链接（恢复原始文件）
privconf unlink

# 同步仓库到远程
privconf sync
```

## Shell Hook

切换目录时自动链接：

```bash
# Bash — 添加到 ~/.bashrc
eval "$(privconf hook bash)"

# Zsh — 添加到 ~/.zshrc
eval "$(privconf hook zsh)"

# Fish — 添加到 ~/.config/fish/conf.d/privconf.fish
privconf hook fish > ~/.config/fish/conf.d/privconf.fish
```

## 命令

| 命令 | 说明 |
|------|------|
| `init` | 初始化 privconf 仓库至 `~/.privconf/` |
| `add <name> <files...>` | 从当前项目添加文件到仓库 |
| `link [--quiet]` | 创建符号链接并从 git 隐藏 |
| `unlink` | 移除符号链接并恢复原始文件 |
| `status` | 显示当前目录的链接状态 |
| `sync` | 拉取、提交并推送仓库 |
| `hook <bash\|zsh\|fish>` | 输出 shell hook 脚本 |

## 配置

仓库默认位于 `~/.privconf/`，可通过 `PRIVCONF_DIR` 自定义：

```bash
export PRIVCONF_DIR=/path/to/custom/store
```

### `config.toml`

项目通过 git remote URL 或路径 glob 匹配：

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

跟踪已链接的文件（自动管理）：

```toml
[[linked]]
project = "myproj"
file = "mise.local.toml"
target = "/home/user/Projects/myproj/mise.local.toml"
skip_worktree = false
```

## 文件隐藏机制

- **未跟踪文件**（不在 git 中）：加入 `.git/info/exclude`
- **已跟踪文件**（已提交到 git）：`git update-index --skip-worktree`
- **备份文件**（`*.privconf.bak`）：同样加入 `.git/info/exclude`

`unlink` 时，原始文件从备份恢复，或通过 `git checkout HEAD` 恢复。

## 许可证

MIT
