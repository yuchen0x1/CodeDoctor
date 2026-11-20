# CodeDoctor

CodeDoctor 是离线优先的代码质量体检工具。Windows 桌面工作台面向证据审阅、历史快照和治理提示词。Linux CLI 面向终端和 CI，在本机扫描目录或 ZIP，输出机器可读报告。

[![CI](https://github.com/yuchen0x1/CodeDoctor/actions/workflows/ci.yml/badge.svg)](https://github.com/yuchen0x1/CodeDoctor/actions/workflows/ci.yml)
[下载 CodeDoctor](https://github.com/yuchen0x1/CodeDoctor/releases/latest) · [安装和更新](docs/engineering/updating.md)

## 最新更新

### 0.2.3（2026-11-21）

- 桌面和 CLI 的 SARIF 导出包含规则索引、扫描完成标记和规则版本。文件位置保留编码后的项目相对路径，不猜测未知行号，也不输出未转换的字节列。
- 打开报告保存对话框时锁定导出控件，避免路径与格式错配；切换项目后，旧对话框的结果不会写入新项目。
- 持久保存受限制的扫描失败原因，重新打开任务仍可排查并按原设置重试。
- 源码预览与内置逐文件规则共用多行凭据遮蔽，兼容 TypeScript/Python 类型注解，避免识别到的凭据续行证据进入新缓存或报告；旧历史不会自动清理，分享前仍需检查，详见[隐私说明](docs/privacy.md)。
- 从 v0.2.2 平滑迁移本地数据；CLI 报告格式和门禁退出码保持兼容。

### 0.2.2（2026-11-21）

- 修复 Windows 安装升级验证的启动竞态，确认旧库就绪、新版恢复扫描及历史数据保留。
- 修复文件末行未换行时列号失真，并使旧规则缓存失效，避免继续复用错误位置。
- 修复启动恢复覆盖手动导入、历史报告选择被后台刷新重置、评分失败隐藏问题、语言筛选遗漏和更新偏好存储异常。
- 升级 Tauri API/CLI 补丁版本，补充前端竞态回归测试、Linux 依赖审计与隔离的 MSI 升级验证。
- 修复 Windows 下并行 ZIP 测试的临时目录冲突，避免测试互相删除文件。
- 统一双主题、交互状态和窄窗布局，补齐键盘/无障碍语义；HTML 导出支持独立双主题和打印样式。
- Windows 设置页可检查版本，安装版支持验证签名的应用内更新。
- Linux x64 CLI 可在终端和 CI 扫描目录或 ZIP，并按严重度设置门禁。
- 新增 SARIF 2.1.0 报告，可导入支持该格式的审阅工具。

详见[更新记录](docs/releases/release-notes.md)与[UI/UX 工程改造验收报告](docs/qa/ui-ux-audit-2026-11-20.md)。

## Windows 安装与启动

在本仓库执行 `pnpm tauri build --ci` 后，Windows x64 产物位于 `src-tauri/target/release`：

| 产物 | 用法 |
| --- | --- |
| `bundle/nsis/CodeDoctor_0.2.3_x64-setup.exe` | 双击，按向导完成当前用户安装 |
| `bundle/msi/CodeDoctor_0.2.3_x64_en-US.msi` | 双击，通过 Windows Installer 安装 |
| `codedoctor.exe` | 直接运行，免安装 |

应用需要 Microsoft Edge WebView2 Runtime。免安装版也使用当前用户的本地扫描数据库，详情见[隐私与数据边界](docs/privacy.md)。

## 平台能力

| 能力 | Windows 桌面 x64 | Linux CLI x64 |
| --- | --- | --- |
| 目录和 ZIP 本地分析，JSON、Markdown、TXT、HTML 和 SARIF | 支持 | 支持 |
| 源码证据视图、历史 SQLite、依赖图和治理提示词 | 支持 | 由桌面提供 |
| 批处理和严重度门禁 | 使用桌面审阅 | 支持 |
| 更新 | 安装版在应用内确认，免安装版手动替换 | 校验 GitHub 发行归档后替换 |

Linux 示例

```bash
codedoctor-cli scan ./project --format sarif --fail-on high > report.sarif
```

退出码 `0` 表示报告生成并符合门禁，`1` 表示命令或扫描失败，`2` 表示报告生成且发现达到严重度门槛的问题。工具不需要 GUI 或 WebView。查看 [平台决策](docs/engineering/platforms.md)。

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
5. 在“历史报告”比较同一项目的两个完成快照，或导出 JSON、Markdown、TXT、HTML、SARIF 报告。

## 可选本机分析器

扫描默认仅使用内置分析。用户可启用已安装的 Ruff。其余适配器的执行限制及尚未接入的工具见[本机分析器](docs/analyzers.md)。

## 评分与报告

评分包含可读性、结构、可维护性、复杂度、重复代码、架构、安全与风险七个维度。各维度根据实际检测证据加权，缺少证据的维度显示“未评估”。重复率按有依据的重复文件比例呈现。技术债务按已发现问题的治理工作量区间估算。历史比较显示新增、已解决、持续、位置与严重度变化，以及评分和覆盖率差异。

## 语言覆盖

详见 [语言能力矩阵](docs/language-capability-matrix.md)。分析器行为详见 [本机分析器](docs/analyzers.md)，隐私和数据边界详见 [隐私说明](docs/privacy.md)。

## 贡献与下载

[下载正式版](https://github.com/yuchen0x1/CodeDoctor/releases/latest) · [问题反馈](https://github.com/yuchen0x1/CodeDoctor/issues/new/choose) · [贡献指南](CONTRIBUTING.md) · [安全报告](SECURITY.md) · [MIT 许可证](LICENSE)

[架构与基线](docs/engineering/baseline.md) · [平台评估](docs/engineering/platforms.md) · [维护和发布](docs/engineering/maintenance.md) · [竞品与技术研究](docs/engineering/research.md)

## 常见问题与路线图

- 断网时可继续扫描、比较和导出。软件更新需要访问 GitHub。
- 原项目移动后，已保存的历史报告仍可查看。重新扫描需导入可用目录或 ZIP。
- 下一阶段优先维护 Windows 安装与更新链路、Linux CLI 的实际分发验证及标准报告互操作。各候选平台以独立验收结果为准。
