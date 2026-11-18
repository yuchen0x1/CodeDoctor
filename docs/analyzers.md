# 本机分析器

CodeDoctor 的内部规则和 Tree-sitter 分析始终在本机运行。外部分析器是可选增强，不是扫描结果成立的前提。

| ID | 工具 | 典型输出 | 状态 |
|---|---|---|---|
| `ruff` | Python | JSON 诊断 | 可在设置中启用本机安装版 |
| `eslint` | JavaScript/TypeScript | JSON 诊断 | 可解析输出，扫描时跳过执行 |
| `typescript` | `tsc` | 编译器文本诊断 | 可解析输出，扫描时跳过执行 |
| `golangci` | Go | JSON 诊断 | 可解析输出，扫描时跳过执行 |
| `clippy` | Rust | JSON compiler message | 可解析输出，扫描时跳过执行 |

默认启用列表为空。Ruff 使用隔离配置，只处理扫描索引中符合条件的 Python 文件。CodeDoctor 在扫描任务页显示各适配器的启用、可用性与执行结果，同时记录实际观察到的版本、耗时与失败原因。可能加载项目插件、配置或构建脚本的命令跳过执行。

外部结果保留工具 ID、规则代码、相对路径、行列位置与证据。工具输出异常、执行失败和未安装各自具有明确的状态。

Pylint、Semgrep、SpotBugs、PMD、Checkstyle、Roslyn Analyzers 与 PHPStan 尚未接入。
