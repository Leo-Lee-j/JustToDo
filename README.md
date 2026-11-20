# JustToDo

[简体中文](README.zh-CN.md)

JustToDo is a lightweight, open-source Windows to-do app that keeps a small task note on your desktop. Your tasks stay on your device; no account or cloud service is required.

## Features

- Keep a movable task note on the Windows desktop and reopen the full list from the system tray.
- Organize tasks with tabs, categories, priorities, due dates, and reminders.
- Search across tabs and filter by status, category, and priority.
- Reorder tasks and tabs, pin tabs, and use keyboard shortcuts for common actions.
- Review task history, recover deleted tasks, and restore local backups.
- Store task and settings data locally as JSON files.

## Download

Download the latest **Windows x64 NSIS installer** from [GitHub Releases](https://github.com/Leo-Lee-j/JustToDo/releases/latest). Install a newer release manually when you want to update.

The installer is not code-signed, so Windows may show a security notice. Only download it from this repository's Releases page.

## Development

The release workflow builds Windows installers on GitHub Actions. To build locally, use Windows with Node.js 22, pnpm 9 or newer, Rust stable, and the [Tauri Windows prerequisites](https://v2.tauri.app/start/prerequisites/#windows).

```powershell
pnpm install --frozen-lockfile
pnpm tauri:dev
pnpm build
cargo test --locked --manifest-path src-tauri/Cargo.toml
pnpm tauri:build
```

To publish a release, update the version in `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`, add release notes to `CHANGELOG.md`, push the commit, and push a matching version tag such as `vX.Y.Z`. GitHub Actions will build and publish the Windows installer. No signing key or manually configured repository secret is needed.

## Data and privacy

Tasks, history, backups, and settings are stored locally. On Windows, the default data folder is `%APPDATA%\JustToDo`. JustToDo does not sync your data to a server. Keep your own backups of anything important.

## License

JustToDo is released under the [MIT License](LICENSE).
