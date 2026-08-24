# JustToDo

> 轻量级 Windows 桌面待办应用：便签主窗、任务栏要务、到期提醒与本地优先的数据存储。

JustToDo 是一个基于 Tauri 2 构建的桌面 TODO 工具，适合将待办事项常驻在桌面附近，快速记录、整理并跟进任务。

## 功能

- 无边框便签主窗口，用于快速创建和管理任务。
- 独立任务栏要务窗口和任务列表弹窗，便于查看当前重点。
- Tab 分类、任务创建/编辑、状态管理、优先级和到期时间。
- 任务拖拽排序。
- 到期提醒与测试通知。
- 任务历史、软删除、回收站恢复，以及 30 天后自动清理的回收站任务。
- 字体、窗口透明度、开机自启动、常用快捷键和任务栏位置等设置。
- 系统托盘菜单与单实例运行保护。
- 本地 JSON 数据存储，不依赖账号或云端服务。
- 已签名的应用内更新机制；更新源必须保持为公开可访问的 HTTPS 地址。

## 系统要求

当前发布工作流面向 Windows 构建和发布。使用应用内更新时，请保持可以访问 GitHub Release 更新资源的网络连接。

## 安装与更新

1. 前往 [Releases](https://github.com/Leo-Lee-j/JustToDo/releases) 页面下载最新版 Windows 安装包。
2. 安装后，在应用设置中选择“检查更新”。
3. 如有新版本，应用会下载已签名的更新包并重启安装。

> 自动更新依赖公开可访问的 `latest.json`、安装包和签名文件。将仓库或 Release 设为私有时，普通客户端无法携带 GitHub 凭据访问这些资源。

## 数据与隐私

- 任务和配置默认保存到本机的 JustToDo 应用数据目录；Windows 下通常位于 `%APPDATA%\JustToDo`。
- 应用不需要账号，也不会将任务数据同步到远程服务。
- 执行回收站永久清理前会创建本地备份；重要数据仍建议自行定期备份。

## 技术栈

- 前端：Vue 3、TypeScript、Vite、Pinia
- 桌面运行时：Tauri 2
- 后端：Rust
- 数据存储：本地 JSON 文件

## 本地开发

### 环境

- Node.js 22
- Rust stable toolchain
- Windows 开发环境及 Tauri 所需依赖

### 常用命令

```powershell
# 安装前端依赖
npm ci

# 启动桌面开发模式
npm run tauri:dev

# 仅构建前端
npm run build

# 运行 Rust 测试
cargo test --manifest-path src-tauri/Cargo.toml

# 构建桌面安装包
npm run tauri:build
```

## 发布

推送格式为 `v*` 的 Git 标签会触发 GitHub Actions Release 工作流。工作流会：

1. 安装依赖并运行前端构建和 Rust 测试；
2. 从 `CHANGELOG.md` 生成 Release 说明；
3. 使用 Tauri 签名密钥构建安装包和 updater 元数据；
4. 发布 Release 资产并检查 `latest.json` 是否已上传。

发布需要在 GitHub Actions Secrets 中配置 Tauri 签名私钥及其密码。私钥绝不能提交到仓库或打包进客户端。

## 项目结构

```text
src/              Vue 前端、组件、状态管理与样式
src-tauri/        Tauri/Rust 桌面端、原生命令和打包配置
.github/workflows/ GitHub Actions 发布工作流
docs/             产品需求与评审资料
```

## 许可证

当前仓库尚未声明开源许可证。在添加许可证前，除法律允许的范围外，请勿假定可以复制、修改或再分发本项目。
