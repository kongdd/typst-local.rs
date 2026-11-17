# typst-local

管理 Typst 的本地包。

## Installation

```bash
cargo install --path .
```

## Usage

```sh
typ install https://github.com/user/package.git
typ install ./package.zip
typ install ./package.tar.gz
typ install ./package
typ update package
typ update --all
typ list
typ uninstall package 0.1.0
typ path
```

支持单版本包，以及以版本号为子目录的多版本包。各版本的 `typst.toml` 须声明相同包名，版本号须与目录名一致。安装不会覆盖已有包或版本；压缩包可以包含一层顶级目录。

Git 安装保留仓库，`update` 执行 `git pull --ff-only`，显示新增提交数和版本变化；`--all` 更新所有包。非 Git 包会跳过。单版本仓库的上游包名或版本变化时，更新会停止，需另行安装新版本。

默认安装到：

- Windows：`%APPDATA%\typst\packages\local`
- Linux：`$XDG_DATA_HOME/typst/packages/local`，未设置时使用 `~/.local/share/typst/packages/local`
- macOS：`~/Library/Application Support/typst/packages/local`

设置 `TYPST_PACKAGE_PATH` 可覆盖 `typst/packages` 目录，与 Typst 的同名环境变量一致。

## 切换包目录

首次执行 `typ cd <包名>` 时，程序会为 Bash、Zsh、Fish 或 PowerShell 自动配置 shell 集成。按提示重新加载配置或打开新终端后，再次执行即可切换目录。

加载后执行 `typ cd modern-cug-report`。未加载时，该命令输出目录路径。
