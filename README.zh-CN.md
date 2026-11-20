# JustToDo

[English](README.md)

JustToDo 是一款轻量、开源的 Windows 待办应用，会在桌面保留一张小型任务便签。任务数据保存在本机，无需账号或云服务。

## 功能

- 在 Windows 桌面显示可移动的任务便签，并可从系统托盘打开完整任务列表。
- 使用 Tab、分类、优先级、到期时间和提醒整理任务。
- 跨 Tab 搜索，并按状态、分类和优先级筛选。
- 拖动排序任务和 Tab、置顶 Tab，并使用快捷键处理常见操作。
- 查看任务历史、恢复已删除任务和本地备份。
- 任务与设置以 JSON 文件保存在本机。

## 下载

前往 [GitHub Releases](https://github.com/Leo-Lee-j/JustToDo/releases/latest) 下载最新的 **Windows x64 NSIS 安装包**。安装后可在「设置 → 关于 JustToDo」点击「检查更新」，发现新版本后点击「安装并重启」。应用会显示下载进度并校验安装包，随后退出应用进行安装，完成后自动重启。也可手动下载并安装新版本。

安装包尚未进行代码签名，Windows 可能会显示安全提示。请只从本仓库的 Releases 页面下载安装包。

## 本地开发

发版工作流会在 GitHub Actions 上构建 Windows 安装包。本机开发需要 Windows、Node.js 22、pnpm 9 或更高版本、Rust stable 和 [Tauri Windows 开发环境](https://v2.tauri.app/start/prerequisites/#windows)。

```powershell
pnpm install --frozen-lockfile
pnpm tauri:dev
pnpm build
cargo test --locked --manifest-path src-tauri/Cargo.toml
pnpm tauri:build
```

发布新版本时，先同步修改 `package.json`、`src-tauri/Cargo.toml` 和 `src-tauri/tauri.conf.json` 中的版本号，在 `CHANGELOG.md` 添加版本说明并推送提交，再推送匹配的版本标签，例如 `vX.Y.Z`。GitHub Actions 会自动构建并发布 Windows 安装包。无需签名密钥或手动配置仓库 Secret。

## 数据与隐私

任务、历史、备份和设置均保存在本机。Windows 默认数据目录为 `%APPDATA%\JustToDo`。JustToDo 不会将数据同步到服务器；检查更新时会连接 GitHub 获取版本信息和下载安装包，不会上传任务或设置。请自行备份重要数据。

## 许可证

JustToDo 使用 [MIT License](LICENSE)。
