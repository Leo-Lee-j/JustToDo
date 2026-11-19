# JustToDo

轻量级 Windows 桌面待办应用，提供桌面便签挂件、到期提醒与本地数据存储。

## 功能

- 启动后在桌面右上角显示便签。其他应用可覆盖便签，按 `Win+D` 或选择托盘的「显示桌面便签」即可回到桌面操作。
- 从系统托盘打开任务列表
- Tab 分类、任务管理、优先级、到期时间与拖拽排序
- 跨 Tab 搜索标题和备注，编辑任务状态、分类和所属 Tab
- 到期通知、任务历史、回收站恢复和自动清理
- 删除后 5 秒撤销、每日数据备份与清理前备份
- 字体、透明度、开机自启、快捷键与系统托盘设置
- 便签自动收起，悬停或点击展开
- 本地 JSON 数据存储，无需账号或云端服务

功能完成情况、验证范围与待完善项目见 [功能评估与修复记录](docs/FEATURE-ASSESSMENT.md)。

## 下载与更新

前往 [Releases](https://github.com/Leo-Lee-j/JustToDo/releases) 下载 Windows 安装包。安装后可在设置中选择“检查更新”。

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
