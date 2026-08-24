# JustToDo

轻量级 Windows 桌面待办应用，提供便签主窗、任务栏要务、到期提醒与本地优先的数据存储。

## 功能

- 便签主窗、任务栏要务和任务列表弹窗
- Tab 分类、任务管理、优先级、到期时间与拖拽排序
- 到期通知、任务历史、回收站恢复和自动清理
- 字体、透明度、开机自启、快捷键与系统托盘设置
- 本地 JSON 数据存储，无需账号或云端服务
- 已签名的应用内更新

## 下载与更新

前往 [Releases](https://github.com/Leo-Lee-j/JustToDo/releases) 下载 Windows 安装包。安装后可在设置中选择“检查更新”。

自动更新需要公开可访问的 `latest.json`、安装包和签名文件，因此更新 Release 必须保持公开。

## 本地开发

需要 Node.js 22、Rust stable 和 Windows Tauri 开发环境。

```powershell
npm ci
npm run tauri:dev
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri:build
```

## 技术栈

Vue 3、TypeScript、Vite、Pinia、Tauri 2 和 Rust。

## 数据

任务和配置默认保存在本机 JustToDo 应用数据目录；Windows 下通常为 `%APPDATA%\JustToDo`。重要数据请自行定期备份。

## 许可证

本项目采用 [MIT License](LICENSE)，允许免费商业使用、修改、分发和私有使用。
