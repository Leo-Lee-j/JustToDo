# GitHub Release 自动更新

## 首次配置

1. 安装 Tauri signer 并生成密钥：`pnpm tauri signer generate -w ~/.tauri/justtodo.key`。
2. 将生成的公钥替换 `src-tauri/tauri.conf.json` 中的 `REPLACE_WITH_TAURI_SIGNER_PUBLIC_KEY`。
3. 在 GitHub 仓库 Settings -> Secrets and variables -> Actions 中添加：
   - `TAURI_SIGNING_PRIVATE_KEY`：私钥文件内容
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：生成密钥时设置的密码

## 发布

提交并推送语义化版本 tag，例如：`git tag v0.0.1; git push origin v0.0.1`。
GitHub Actions 会构建 Windows 安装包、签名文件和 `latest.json`，并上传到 Release。

客户端设置中的“检查更新”从 `releases/latest/download/latest.json` 查询更新，显示版本、更新说明和下载进度，下载完成后自动重启安装。

## 注意

- 私钥严禁提交到仓库或写入前端代码。
- 发布版本必须高于当前版本号；同时更新 `package.json`、`src-tauri/Cargo.toml` 和 `src-tauri/tauri.conf.json` 的版本。
- 当前 workflow 先支持 Windows；新增 macOS/Linux 时，在矩阵中增加对应 runner，并确认对应安装包可正常更新。
