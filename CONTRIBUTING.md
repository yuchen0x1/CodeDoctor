# 参与 CodeDoctor 开发

从 `main` 创建短期分支。每个 PR 聚焦一项可验证的变更，并说明受影响的平台、用户数据和更新路径。将新增行为的用例放到对应的 Vitest 或 Rust 测试模块。

前端执行 `pnpm install --frozen-lockfile`、`pnpm typecheck`、`pnpm test` 和 `pnpm build`。Rust 执行 `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`、`cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets` 和 `cargo test --manifest-path src-tauri/Cargo.toml --locked`。涉及界面、安装器或更新的 PR 还应提供本地 Windows 验证结果。

只提交有授权的源码、样例和截图。提交诊断前请用可公开的最小项目替代私有源码，移除令牌、私钥和个人路径。安全问题使用 [私密报告](https://github.com/yuchen0x1/CodeDoctor/security/advisories/new)。

维护者在检查通过后合并到 `main`，统一调整版本与更新记录，使用精确版本标签触发发布。紧急修复沿用 PR 和测试流程，递增补丁版本。已经发布的资产按原版本保存。[发布维护手册](docs/engineering/maintenance.md)
