# 维护与发布

`main` 保存可发布代码。功能、修复和依赖更新均走短期分支和 PR。CI 核对锁文件、版本、前端类型、Vitest、Rust 格式、Clippy、Rust 测试和依赖变更。合并冲突在 PR 分支解决，通过检查后合并。安全修复同样从当前稳定源分支发布新补丁版本。

发布时同步 `package.json`、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`、`src-tauri/tauri.conf.json` 和 [更新记录](../releases/release-notes.md)。`node scripts/release/version.mjs` 验证版本。Actions 的手动 Build and release 运行构建和冒烟测试并保存附件。已合并的 `vMAJOR.MINOR.PATCH` 标签才触发公开发布。

发布作业先构建并完成运行检查，之后用独立、只对发布作业可用的写权限创建草稿。远端的附件字节数和 SHA-256 与构建工件相符才公开。已有标签和正式版保留。失败草稿的核验记录可用于重试前定位问题。

每周执行依赖扫描。Dependabot 提议 npm、Cargo 与 GitHub Actions 升级，维护者检查许可证、平台影响和 CI，安全更新优先。公开报告先清理项目路径和源码证据。[安全联系](../../SECURITY.md)

每次发布检查平台矩阵、安装、升级、数据库迁移、下载与更新源。新增平台先补齐真实运行测试和后续更新渠道。每 30 天在维护审阅中刷新选定竞品的发布记录，大版本再执行全量深度调研。
