# CodeDoctor 工程基线

2026-11-18 · `v0.1.1`（`0f87234`）

## 现有产品

| 边界 | 模块及行为 |
| --- | --- |
| Windows 界面 | React、TypeScript 和 Vite 运行于 Tauri 2 / WebView2。总览、导入、任务、问题、依赖、注释、重复、提示词、历史、设置十个工作区。 |
| IPC | `src/lib/tauri.ts` 封装正式命令。Rust `commands` 调用本地服务并通过 `scan://progress` 发送进度。原生对话框权限限定到 `main`。 |
| 导入与索引 | `import`、`index` 支持文件夹与 ZIP、语言与框架识别、编码识别、默认及自定义目录排除。ZIP 按路径、类型和大小规则验证。 |
| 分析 | `parse`、`rules`、`clones`、`graph` 提供 Tree-sitter 和词法规则、复杂度、注释、重复与依赖图。可选 Ruff 使用隔离参数，本机其余分析器仅显示状态。 |
| 任务 | `scan` 与 `state` 持有暂停、继续、取消、异常退出恢复和扫描缓存。源项目由应用只读访问。 |
| 存储 | `storage` 以 SQLite 记录项目、快照、文件摘要、问题、评分、依赖和提示词。两个已有 SQL 迁移，Windows 目录为 `%APPDATA%\com.codedoctor.desktop`。 |
| 使用结果 | `score`、`history`、`report`、`prompt` 提供检测覆盖率、问题定位与摘要校验、快照差异、JSON/Markdown/TXT/HTML、全量或增量治理提示词。 |
| 部署边界 | 桌面核心无远程 API、账号、云数据库或遥测。当前稳定版由 Windows 本地手工构建 EXE、MSI 和 SHA-256 清单。 |

## 仓库和验证

原 GitHub 仓库为 `yuchen0x1/CodeDoctor`，公开，默认分支 `main`。已有 `v0.1.0` 和 `v0.1.1`，每版含 NSIS、MSI、免安装 EXE 和校验清单。Issue 和私密漏洞报告已启用。初次检查时主分支无生效规则，Actions 开启且默认令牌只读，秘密扫描及推送保护已启用。已有 Dependabot 服务流程，源码中尚无项目工作流。未配置 GitHub Pages 或发布环境。初始仓库无项目许可证，经项目所有者确认后采用 MIT。

| 检查 | 基线 |
| --- | --- |
| `pnpm install --frozen-lockfile`、`pnpm typecheck`、`pnpm build` | PASS |
| `pnpm test` | PASS，22 项 |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked --offline` | PASS，149 项 |
| Windows 桌面 | `v0.1.1` 目录与 ZIP 扫描、问题快照、历史对比、报告导出和提示词已经实测。新一轮复跑使用独立应用数据目录。 |

## 发布暴露面

1. **总览**。发布基线使用静态扫描器分别检查 `dist/` 和 Windows x64 EXE。前端 3 个文件，扫描器未报告 source map 或凭据。原生扫描器原始评分 44 / 100，含启发式误报与硬化项。
2. **发现清单**。Windows EXE 缺少 Authenticode 和 Control Flow Guard。CodeView 中仅有 `codedoctor.pdb` 文件名。`localhost:1420` 来自开发配置。一个 PEM 起始字面量来自源码内容识别。
3. **证据与修复**。PE 证书目录为空。`DllCharacteristics` 为 `0x8160`。二进制中完整的 PEM 私钥块计数为零，`commands/source.rs` 用 `PRIVATE KEY-----` 识别待遮蔽内容。发布流程应校验产物来源，Windows 签名另需受信任证书。保留本地调试能力，密钥仅存于签名环境。
4. **客户端边界**。产品以本地只读分析为核心，规则和版本校验公钥按设计随客户端交付。扫描报告可能留有用户源码证据，分享前应检查内容。签名私钥和仓库写入凭据需要独立于客户端管理。
5. **实施顺序**。先建立版本、测试和来源验证，再接入签名更新。将平台适配放进独立入口，避免给 CLI 引入桌面 WebView 依赖。
