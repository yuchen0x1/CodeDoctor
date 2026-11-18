# CodeDoctor

CodeDoctor 是一个离线优先的 Windows 桌面代码质量体检工具。它在本机读取项目或 ZIP 快照，建立文件索引，使用 Tree-sitter、词法规则、重复代码和依赖图分析生成可追溯的问题、评分、报告和治理提示词。

## 最新更新

### 0.1.1（2026-11-18）

- 重新打开当前扫描快照后，仍可查看问题、评分和分析器结果。
- 重复导入同一文件夹或 ZIP 时，保留当前扫描和历史报告，并刷新导入预览。
- 原始文件夹或 ZIP 移动、删除后，仍可打开和导出之前保存的报告。

详见[更新记录](docs/releases/release-notes.md)。

## Windows 安装与启动

在本仓库执行 `pnpm tauri build --ci` 后，Windows x64 产物位于 `src-tauri/target/release`：

| 产物 | 用法 |
| --- | --- |
| `bundle/nsis/CodeDoctor_0.1.1_x64-setup.exe` | 双击，按向导完成当前用户安装 |
| `bundle/msi/CodeDoctor_0.1.1_x64_en-US.msi` | 双击，通过 Windows Installer 安装 |
| `codedoctor.exe` | 直接运行，免安装 |

应用需要 Microsoft Edge WebView2 Runtime。免安装版也使用当前用户的本地扫描数据库，详情见[隐私与数据边界](docs/privacy.md)。

## 本地开发

前置要求：Node.js、pnpm、Rust stable，以及 Tauri 2 所需的 Windows WebView2/构建工具。

```powershell
pnpm install
pnpm dev
pnpm typecheck
pnpm test
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml --offline
pnpm tauri dev
pnpm tauri build --ci
```

Rust 依赖缓存可通过 `CARGO_HOME` 指向本机缓存目录；项目不需要联网下载分析器或项目依赖。

## 工作流

1. 在“项目体检”打开原生文件夹或 ZIP 选择器，也可拖入目录或 ZIP，或粘贴绝对路径。导入预览提供语言、框架、文件数、行数、深度和配置清单。
2. 在“设置”保存扫描阈值、额外排除目录和本机 Ruff 偏好。扫描任务支持暂停、继续和取消。中断后可按原设置发起新扫描，复用校验通过的文件规则缓存。
3. 在“问题分析”按文件、语言、模块、类型或严重度筛选，展开证据和对应源码行，或导出筛选后的 JSON 清单。架构图可查看静态依赖、循环和重复代码关系。
4. 在“修复提示词”选择全面、重点或定点模式。任选同项目的已完成基线以生成增量任务书。生成后可复制或保存为 Markdown、TXT。
5. 在“历史报告”比较同一项目的两个完成快照，或导出 JSON、Markdown、TXT、HTML 报告。

## 可选本机分析器

扫描默认仅使用内置分析。用户可启用已安装的 Ruff。其余适配器的执行限制及尚未接入的工具见[本机分析器](docs/analyzers.md)。

## 评分与报告

评分包含可读性、结构、可维护性、复杂度、重复代码、架构、安全与风险七个维度。各维度根据实际检测证据加权，缺少证据的维度显示“未评估”。重复率按有依据的重复文件比例呈现。技术债务按已发现问题的治理工作量区间估算。历史比较显示新增、已解决、持续、位置与严重度变化，以及评分和覆盖率差异。

## 语言覆盖

详见 [语言能力矩阵](docs/language-capability-matrix.md)。分析器行为详见 [本机分析器](docs/analyzers.md)，隐私和数据边界详见 [隐私说明](docs/privacy.md)。
