# 竞品与技术研究

2026-11-18 GitHub 快照。依据 README、源码目录、固定提交实现、公开 Issue 样本、提交及发行记录。30/90/180 天为默认分支提交数。最近更新的 Issue 样本用于了解现象，频次需要独立统计。外部实现用于比较和设计参考。

## 32 个相关项目

| 序号 | 项目 | Star / Fork | 主语言 / 许可证 | 30 / 90 / 180 天 | 推送 | GitHub Release | 能力与实现证据 |
| --- | --- | ---: | --- | ---: | --- | --- | --- |
| 1 | [SonarSource/sonarqube](https://github.com/SonarSource/sonarqube) | 11195 / 2245 | Java / LGPL-3.0 | 137 / 634 / 1086 | 2026-11-18 | [26.11.0.134175](https://github.com/SonarSource/sonarqube/releases/tag/26.11.0.134175) 2026-11-04 | 直接｜JVM 服务端以重复检测和趋势支持集中质量门禁，适用集中部署。 目录 buildSrc、docs、gradle、plugins。[实现](https://github.com/SonarSource/sonarqube/blob/457cee5aabea2e1eabb151afb33f6c65a6e02e69/sonar-duplications/src/main/java/org/sonar/duplications/detector/original/OriginalCloneDetectionAlgorithm.java) |
| 2 | [semgrep/semgrep](https://github.com/semgrep/semgrep) | 17794 / 1196 | C / LGPL-2.1 | 103 / 233 / 411 | 2026-11-18 | [v1.186.0](https://github.com/semgrep/semgrep/releases/tag/v1.186.0) 2026-11-18 | 直接｜Python CLI 分离配置、扫描目标与机器报告，参考本地分析入口。 目录 changelog.d、cli、dev、images。[实现](https://github.com/semgrep/semgrep/blob/cd3b439c36f43a299dffcd1e7544f6ce7ebb03cc/cli/src/semgrep/commands/scan.py) |
| 3 | [pmd/pmd](https://github.com/pmd/pmd) | 5534 / 1619 | Java / 查阅许可声明 | 106 / 235 / 628 | 2026-11-18 | [pmd_releases/7.29.0](https://github.com/pmd/pmd/releases/tag/pmd_releases/7.29.0) 2026-10-30 | 直接｜Java 规则、报告及 CPD 各有边界，参考 CLI 与集成包。 目录 docs、pmd-ant、pmd-apex、pmd-cli。[实现](https://github.com/pmd/pmd/blob/c043ab830746a58810926a87efae699fde34ec5f/pmd-core/src/main/java/net/sourceforge/pmd/reporting/Report.java) |
| 4 | [Ericsson/codechecker](https://github.com/Ericsson/codechecker) | 2637 / 505 | Python / Apache-2.0 | 35 / 181 / 467 | 2026-11-18 | [v6.29.3](https://github.com/Ericsson/codechecker/releases/tag/v6.29.3) 2026-11-05 | 直接｜Python 分析器与 Web 审阅分层，参考基线对比和过滤。 目录 analyzer、bin、ci、codechecker_api。[实现](https://github.com/Ericsson/codechecker/blob/53222cd5d7566c553d8d7edbb5134a35a56ef15e/analyzer/codechecker_analyzer/analyzers/clangsa/analyzer.py) |
| 5 | [cppcheck-opensource/cppcheck](https://github.com/cppcheck-opensource/cppcheck) | 6802 / 1611 | C++ / GPL-3.0 | 86 / 192 / 394 | 2026-11-18 | [2.22.0](https://github.com/cppcheck-opensource/cppcheck/releases/tag/2.22.0) 2026-09-19 | 直接｜C++ CLI、GUI 与 HTML 各自分发，参考机器入口参数。 目录 addons、cfg、cli、cmake。[实现](https://github.com/cppcheck-opensource/cppcheck/blob/68bf94a42b463d773d146efd0602d2031e53aa11/cli/cmdlineparser.cpp) |
| 6 | [kucherenko/jscpd](https://github.com/kucherenko/jscpd) | 6575 / 276 | Rust / MIT | 142 / 712 / 1148 | 2026-11-18 | [v5.4.3](https://github.com/kucherenko/jscpd/releases/tag/v5.4.3) 2026-11-08 | 直接｜Rust 重复检测提供阈值、基线和 CI 输出，参考退出码与 SARIF。 目录 assets、benchmark、docs、examples。[实现](https://github.com/kucherenko/jscpd/blob/b79927a32fbfa38f9580c224d679c8ef855c1d5f/examples/rust-cpd-finder/src/main.rs) |
| 7 | [terryyin/lizard](https://github.com/terryyin/lizard) | 2604 / 306 | JavaScript / 查阅许可声明 | 731 / 801 / 846 | 2026-11-18 | [1.26.0](https://github.com/terryyin/lizard/releases/tag/1.26.0) 2026-11-10 | 直接｜Python 多语言适配及扩展入口，参考复杂度阈值与语言限制。 目录 docs、lizard_core、lizard_ext、lizard_languages。[实现](https://github.com/terryyin/lizard/blob/94a3bece620d0120ca6557dbfbac30e660e8b76f/lizard.py) |
| 8 | [rubik/radon](https://github.com/rubik/radon) | 2045 / 146 | Python / MIT | 0 / 0 / 0 | 2024-10-20 | 无 GitHub Release 记录 | 直接｜Python 语法树访客和结果输出分离，参考复杂度指标说明。 目录 docs、radon。[实现](https://github.com/rubik/radon/blob/54b88e5878b2724bf4d77f97349588b811abdff2/radon/complexity.py) |
| 9 | [astral-sh/ruff](https://github.com/astral-sh/ruff) | 51288 / 2757 | Rust / MIT | 549 / 1384 / 2899 | 2026-11-18 | [0.17.5](https://github.com/astral-sh/ruff/releases/tag/0.17.5) 2026-11-12 | 间接｜Rust 原生 CLI 有结构化诊断与隔离检查，现有 Ruff 适配可复用。 目录 assets、changelogs、crates、docs。[实现](https://github.com/astral-sh/ruff/blob/5203c350a703ece7092d8ef33b51f0878efe56d7/crates/ruff/src/lib.rs) |
| 10 | [pylint-dev/pylint](https://github.com/pylint-dev/pylint) | 5750 / 1452 | Python / GPL-2.0 | 90 / 330 / 504 | 2026-11-18 | [v4.1.4](https://github.com/pylint-dev/pylint/releases/tag/v4.1.4) 2026-11-14 | 间接｜Python checker 注册与分类，参考错误和诊断区分。 目录 doc、examples、pylint、script。[实现](https://github.com/pylint-dev/pylint/blob/120d9f3389af5093849be5dd25a2df74af5be9dd/pylint/lint/pylinter.py) |
| 11 | [PyCQA/bandit](https://github.com/PyCQA/bandit) | 8393 / 893 | Python / Apache-2.0 | 3 / 6 / 16 | 2026-11-09 | [1.9.4](https://github.com/PyCQA/bandit/releases/tag/1.9.4) 2026-02-25 | 间接｜Python AST 规则组织严重度和置信度，适合证据界面。 目录 bandit、doc、docker、examples。[实现](https://github.com/PyCQA/bandit/blob/2d5467c45c3189f4d7d07e8022276bc73a478abd/bandit/core/manager.py) |
| 12 | [eslint/eslint](https://github.com/eslint/eslint) | 27627 / 5204 | JavaScript / MIT | 56 / 152 / 350 | 2026-11-18 | [v10.15.0](https://github.com/eslint/eslint/releases/tag/v10.15.0) 2026-11-13 | 间接｜Node linter 分离解析、规则及报告。项目侧可执行配置不适合离线导入。 目录 bin、conf、docs、lib。[实现](https://github.com/eslint/eslint/blob/21026992e3520869b31ca4500905665942bb2cf0/lib/linter/linter.js) |
| 13 | [biomejs/biome](https://github.com/biomejs/biome) | 26130 / 1446 | Rust / Apache-2.0 | 474 / 1111 / 1633 | 2026-11-18 | [@biomejs/biome@2.6.8](https://github.com/biomejs/biome/releases/tag/%40biomejs/biome%402.6.8) 2026-11-15 | 间接｜Rust workspace 服务让 CLI 和集成入口共享引擎与诊断。 目录 crates、e2e-tests、fuzz、packages。[实现](https://github.com/biomejs/biome/blob/73125bdb0326e211b796f1bf893e844609dc6b70/crates/biome_service/src/workspace.rs) |
| 14 | [oxc-project/oxc](https://github.com/oxc-project/oxc) | 23493 / 1469 | Rust / MIT | 1191 / 2483 / 4674 | 2026-11-18 | [oxlint_v1.94.0](https://github.com/oxc-project/oxc/releases/tag/oxlint_v1.94.0) 2026-11-16 | 间接｜Rust 语义模型和 linter 分工，参考缓存与大型仓库并发。 目录 apps、crates、napi、npm。[实现](https://github.com/oxc-project/oxc/blob/77ba01850fbe192da2becd172c914974925a445e/crates/oxc_linter/src/lib.rs) |
| 15 | [golangci/golangci-lint](https://github.com/golangci/golangci-lint) | 19541 / 1641 | Go / GPL-3.0 | 28 / 132 / 217 | 2026-11-17 | [v2.15.0](https://github.com/golangci/golangci-lint/releases/tag/v2.15.0) 2026-10-16 | 间接｜Go CLI 使用统一 issue 模型，参考输出与质量门禁。 目录 assets、build、cmd、docs。[实现](https://github.com/golangci/golangci-lint/blob/319b86d903b57a53a34b6b41a0968d6145b1ed5a/pkg/result/issue.go) |
| 16 | [dominikh/go-tools](https://github.com/dominikh/go-tools) | 6937 / 434 | Go / MIT | 5 / 46 / 92 | 2026-10-24 | [2026.2.2](https://github.com/dominikh/go-tools/releases/tag/2026.2.2) 2026-10-12 | 间接｜Go 静态分析从 AST 到 SARIF，参考位置映射。 目录 _benchmarks、analysis、cmd、config。[实现](https://github.com/dominikh/go-tools/blob/8371db75e9daf10073dc2ba684c2a24a1d6ead95/lintcmd/lint.go) |
| 17 | [rust-lang/rust-clippy](https://github.com/rust-lang/rust-clippy) | 13682 / 2361 | Rust / Apache-2.0 | 235 / 633 / 1411 | 2026-11-17 | 无 GitHub Release 记录 | 间接｜Rust 编译语义 lint 规则和测试精细，需编译工具链。 目录 book、clippy_config、clippy_dev、clippy_dummy。[实现](https://github.com/rust-lang/rust-clippy/blob/56afb1133b89d84e0458047b4d51611710f111bb/clippy_lints/src/lib.rs) |
| 18 | [spotbugs/spotbugs](https://github.com/spotbugs/spotbugs) | 3969 / 706 | Java / LGPL-2.1 | 36 / 168 / 275 | 2026-11-18 | [4.10.5](https://github.com/spotbugs/spotbugs/releases/tag/4.10.5) 2026-10-12 | 间接｜Java 字节码和插件依赖构建产物，参考原始源码覆盖率边界。 目录 buildSrc、docs、eclipsePlugin-junit、eclipsePlugin-test。[实现](https://github.com/spotbugs/spotbugs/blob/ccdff0605e3447098b2e6f118d5f9e95fa05cb39/spotbugs/src/main/java/edu/umd/cs/findbugs/FindBugs2.java) |
| 19 | [checkstyle/checkstyle](https://github.com/checkstyle/checkstyle) | 9661 / 4414 | Java / LGPL-2.1 | 453 / 1020 / 1832 | 2026-11-18 | [checkstyle-14.4.0](https://github.com/checkstyle/checkstyle/releases/tag/checkstyle-14.4.0) 2026-10-31 | 间接｜Java 检查器按生命周期输出事件，参考进度和出错定位。 目录 config、docs、src。[实现](https://github.com/checkstyle/checkstyle/blob/2e0f0c4b8dafa1de23a3d44012e6c3838dbbd54e/src/main/java/com/puppycrawl/tools/checkstyle/Checker.java) |
| 20 | [detekt/detekt](https://github.com/detekt/detekt) | 7120 / 889 | Kotlin / Apache-2.0 | 80 / 200 / 414 | 2026-11-17 | [v1.23.8](https://github.com/detekt/detekt/releases/tag/v1.23.8) 2025-02-21 | 间接｜Kotlin 分析后端与报告独立，参考按能力筛选规则。 目录 bots、build-logic、code-coverage-report、config。[实现](https://github.com/detekt/detekt/blob/e2c7e71be037ea10eea51d5b8e0ff3d84bff0d84/detekt-core/src/main/kotlin/dev/detekt/core/tooling/AnalysisFacade.kt) |
| 21 | [gitleaks/gitleaks](https://github.com/gitleaks/gitleaks) | 31159 / 2388 | Go / MIT | 1 / 2 / 21 | 2026-10-28 | [v8.30.1](https://github.com/gitleaks/gitleaks/releases/tag/v8.30.1) 2026-03-21 | 间接｜Go 发现模型和 SARIF 分离，参考报告敏感证据边界。 目录 cmd、config、detect、logging。[实现](https://github.com/gitleaks/gitleaks/blob/5f6ec56cfa6f8c67648d863582c7ae2f206d5c1c/report/sarif.go) |
| 22 | [sverweij/dependency-cruiser](https://github.com/sverweij/dependency-cruiser) | 7442 / 312 | JavaScript / MIT | 26 / 69 / 106 | 2026-11-18 | [v18.8.0](https://github.com/sverweij/dependency-cruiser/releases/tag/v18.8.0) 2026-11-16 | 直接｜Node 分离依赖分析与报告，参考模块规则及视图。 目录 bin、configs、doc、docs。[实现](https://github.com/sverweij/dependency-cruiser/blob/7bf8972978e7e8b3de6cd70d1987d8ad64091a09/src/main/index.mjs) |
| 23 | [pahen/madge](https://github.com/pahen/madge) | 10186 / 351 | JavaScript / MIT | 0 / 0 / 0 | 2026-01-21 | 无 GitHub Release 记录 | 直接｜Node 依赖 API 分别提供关系、循环和输出，参考环路视图。 目录 bin、lib、test。[实现](https://github.com/pahen/madge/blob/456057b85c2d063dad9c10147d7686ded4fc5ce5/lib/api.js) |
| 24 | [thebjorn/pydeps](https://github.com/thebjorn/pydeps) | 2121 / 134 | Python / BSD-2-Clause | 0 / 14 / 25 | 2026-11-09 | [v3.0.9](https://github.com/thebjorn/pydeps/releases/tag/v3.0.9) 2026-10-01 | 直接｜Python 库分离循环与图渲染，参考筛选和包边界。 目录 docker-images、docs、pydeps、tests。[实现](https://github.com/thebjorn/pydeps/blob/9f086edb4edc1ad5cafc56d26f36d16c116baafa/pydeps/pydeps.py) |
| 25 | [boyter/scc](https://github.com/boyter/scc) | 8886 / 365 | Go / MIT | 35 / 69 / 143 | 2026-11-17 | [v4.1.0](https://github.com/boyter/scc/releases/tag/v4.1.0) 2026-09-07 | 直接｜Go 跨平台并发统计和报告，参考 CLI 分发、内存与退出码。 目录 cmd、examples、packages、processor。[实现](https://github.com/boyter/scc/blob/324f65ef2045b0e37f5ba1c43e0b2f093345c4db/processor/processor.go) |
| 26 | [AlDanial/cloc](https://github.com/AlDanial/cloc) | 23675 / 1126 | Perl / GPL-2.0 | 24 / 33 / 56 | 2026-11-18 | [v2.12](https://github.com/AlDanial/cloc/releases/tag/v2.12) 2026-11-13 | 直接｜Perl 本地统计明确源码、注释和空行，参考报告标注。 目录 Unix、tests。[实现](https://github.com/AlDanial/cloc/blob/414fe1617b75fa2847844295ac423180db9ca4ea/cloc) |
| 27 | [reviewdog/reviewdog](https://github.com/reviewdog/reviewdog) | 9751 / 518 | Go / MIT | 236 / 558 / 821 | 2026-11-18 | [v0.21.3](https://github.com/reviewdog/reviewdog/releases/tag/v0.21.3) 2026-11-07 | 技术｜Go 诊断和 diff 过滤，参考新增问题与 CI 门禁。 目录 _testdata、assets、cienv、cmd。[实现](https://github.com/reviewdog/reviewdog/blob/f42d2a2763b3fee771905bdfe1b71269844c21e1/filter/filter.go) |
| 28 | [rustsec/rustsec](https://github.com/rustsec/rustsec) | 1992 / 214 | Rust / 查阅许可声明 | 42 / 81 / 217 | 2026-11-18 | [cargo-audit/v0.23.0](https://github.com/rustsec/rustsec/releases/tag/cargo-audit/v0.23.0) 2026-11-10 | 技术｜Rust 读取锁文件和公告，参考跨目标依赖审核。 目录 admin、cargo-audit、cargo-lock、cvss。[实现](https://github.com/rustsec/rustsec/blob/bd67dede0d87d24fe8f1e0964e985bb805bfb6e8/cargo-audit/src/auditor.rs) |
| 29 | [tree-sitter/tree-sitter](https://github.com/tree-sitter/tree-sitter) | 27503 / 3040 | Rust / MIT | 91 / 243 / 388 | 2026-11-18 | [v0.27.2](https://github.com/tree-sitter/tree-sitter/releases/tag/v0.27.2) 2026-11-07 | 技术｜C 解析器及语言绑定，参考语法树版本、位置和缓存。 目录 crates、docs、lib、test。[实现](https://github.com/tree-sitter/tree-sitter/blob/564fa307125d78d51479b12dc079ac69abe46a30/lib/src/parser.c) |
| 30 | [tauri-apps/tauri](https://github.com/tauri-apps/tauri) | 112999 / 4230 | Rust / Apache-2.0 | 90 / 288 / 445 | 2026-11-18 | [tauri-cli-v2.12.6](https://github.com/tauri-apps/tauri/releases/tag/tauri-cli-v2.12.6) 2026-11-18 | 技术｜Rust Tauri 的原生 WebView 与 IPC，平台安装和运行分别验收。 目录 audits、bench、crates、examples。[实现](https://github.com/tauri-apps/tauri/blob/e1d7dca25598b8a9843cec2de120337de5c19c34/crates/tauri/src/lib.rs) |
| 31 | [tauri-apps/plugins-workspace](https://github.com/tauri-apps/plugins-workspace) | 1843 / 640 | Rust / Apache-2.0 | 32 / 186 / 231 | 2026-11-18 | [persisted-scope-v2.4.3](https://github.com/tauri-apps/plugins-workspace/releases/tag/persisted-scope-v2.4.3) 2026-11-10 | 技术｜Rust updater 使用签名绑定版本，适用原生分发与显式更新。 目录 examples、packages、plugins、shared。[实现](https://github.com/tauri-apps/plugins-workspace/blob/21f01aa3d5d171f7e5deb5262480bb6ae20e3d0c/plugins/updater/src/updater.rs) |
| 32 | [sanks205/getobserver](https://github.com/sanks205/getobserver) | 16 / 0 | Go / MIT | 52 / 103 / 140 | 2026-10-31 | [v0.8.2](https://github.com/sanks205/getobserver/releases/tag/v0.8.2) 2026-10-29 | 直接｜Go CLI 与单文件报告支持本机扫描及机器工作流。 目录 cmd、docs、examples、internal。[实现](https://github.com/sanks205/getobserver/blob/10eb9e94e70612cb5a500311ff73c87627dbfc71/cmd/cli/main.go) |

## 近期活动样本

- **SonarSource/sonarqube** · 提交「SCA-3199 Use shared Slack telemetry contract in Enterprise」 · 无公开待处理问题样本。[提交](https://github.com/SonarSource/sonarqube/commit/457cee5aabea2e1eabb151afb33f6c65a6e02e69)
- **semgrep/semgrep** · 提交「Cron - update nix flakes (semgrep/semgrep-proprietary#7544)」 · 问题「Taint: exact sanitizer ignored when result is assigned to a variable」。[提交](https://github.com/semgrep/semgrep/commit/cd3b439c36f43a299dffcd1e7544f6ce7ebb03cc) [Issue](https://github.com/semgrep/semgrep/issues/12088)
- **pmd/pmd** · 提交「chore(deps): bump actions/setup-java from 6.2.0 to 6.3.0 (#7356)」 · 问题「java UnusedPrivateMethod: any annotation string equal to the method name counts as a use」。[提交](https://github.com/pmd/pmd/commit/c043ab830746a58810926a87efae699fde34ec5f) [Issue](https://github.com/pmd/pmd/issues/7340)
- **Ericsson/codechecker** · 提交「Merge pull request #5186 from gulyasgergely902/fix-enabled-disabled-r…」 · 问题「CTU analysis misses bugs when the same source file is compiled with different flags」。[提交](https://github.com/Ericsson/codechecker/commit/53222cd5d7566c553d8d7edbb5134a35a56ef15e) [Issue](https://github.com/Ericsson/codechecker/issues/5078)
- **cppcheck-opensource/cppcheck** · 提交「Fix #15221 (Deprecate the clang import) (#9031)」 · 无公开待处理问题样本。[提交](https://github.com/cppcheck-opensource/cppcheck/commit/68bf94a42b463d773d146efd0602d2031e53aa11)
- **kucherenko/jscpd** · 提交「Merge pull request #1300 from kucherenko/dependabot/cargo/rust/tree-s…」 · 问题「RUSTSEC-2026-0543: chrono is unmaintained」。[提交](https://github.com/kucherenko/jscpd/commit/b79927a32fbfa38f9580c224d679c8ef855c1d5f) [Issue](https://github.com/kucherenko/jscpd/issues/1309)
- **terryyin/lizard** · 提交「Shorten the shared retrospective findings log」 · 问题「cognitive_complexity not in function __dict__ if score is 0」。[提交](https://github.com/terryyin/lizard/commit/94a3bece620d0120ca6557dbfbac30e660e8b76f) [Issue](https://github.com/terryyin/lizard/issues/570)
- **rubik/radon** · 近 30 天无默认分支提交 · 问题「Is the project still maintained?」。 [Issue](https://github.com/rubik/radon/issues/287)
- **astral-sh/ruff** · 提交「ty Show progress and handle Ctrl-C during initial file indexing (#3…」 · 问题「PYI019 does not catch all instances of where it could convert a function to use Self」。[提交](https://github.com/astral-sh/ruff/commit/5203c350a703ece7092d8ef33b51f0878efe56d7) [Issue](https://github.com/astral-sh/ruff/issues/30530)
- **pylint-dev/pylint** · 提交「Replace pytest-benchmark with CodSpeed (#11831)」 · 问题「False positive `unbalanced-dict-unpacking`」。[提交](https://github.com/pylint-dev/pylint/commit/120d9f3389af5093849be5dd25a2df74af5be9dd) [Issue](https://github.com/pylint-dev/pylint/issues/9986)
- **PyCQA/bandit** · 提交「Bump docker/login-action from 4.7.0 to 4.8.0 (#1487)」 · 问题「Turn off progress bar in CI」。[提交](https://github.com/PyCQA/bandit/commit/2d5467c45c3189f4d7d07e8022276bc73a478abd) [Issue](https://github.com/PyCQA/bandit/issues/1476)
- **eslint/eslint** · 提交「chore: update dependency prettier to v3.10.0 (#21567)」 · 问题「Dependency Dashboard」。[提交](https://github.com/eslint/eslint/commit/21026992e3520869b31ca4500905665942bb2cf0) [Issue](https://github.com/eslint/eslint/issues/17733)
- **biomejs/biome** · 提交「feat(lint): add convex rule source and domain (#13284)」 · 问题「Dependency Dashboard」。[提交](https://github.com/biomejs/biome/commit/73125bdb0326e211b796f1bf893e844609dc6b70) [Issue](https://github.com/biomejs/biome/issues/2188)
- **oxc-project/oxc** · 提交「docs(skills): add PR title guidance (#29833)」 · 问题「parser: JSX text is recorded as a comment in TSX」。[提交](https://github.com/oxc-project/oxc/commit/77ba01850fbe192da2becd172c914974925a445e) [Issue](https://github.com/oxc-project/oxc/issues/29836)
- **golangci/golangci-lint** · 提交「build(deps): bump golang.org/x/oauth2 from 0.37.0 to 0.38.0 in /scrip…」 · 问题「gosec: G104 -> ignoring functions doesn't work for types containing `.`」。[提交](https://github.com/golangci/golangci-lint/commit/319b86d903b57a53a34b6b41a0968d6145b1ed5a) [Issue](https://github.com/golangci/golangci-lint/issues/3749)
- **dominikh/go-tools** · 提交「lintcmd/runner: don't use temporary files to encode results」 · 问题「Support incorrect `slices` package usage for SA1 checks」。[提交](https://github.com/dominikh/go-tools/commit/8371db75e9daf10073dc2ba684c2a24a1d6ead95) [Issue](https://github.com/dominikh/go-tools/issues/1850)
- **rust-lang/rust-clippy** · 提交「Set merge-base commit SHA to latest Rustup commit (#18077)」 · 问题「Suggest bool::toggle() instead of manual boolean negation」。[提交](https://github.com/rust-lang/rust-clippy/commit/56afb1133b89d84e0458047b4d51611710f111bb) [Issue](https://github.com/rust-lang/rust-clippy/issues/18094)
- **spotbugs/spotbugs** · 提交「chore(deps): update plugin org.sonarqube to v7.6.0.18749 (#4470)」 · 问题「Dependency Dashboard」。[提交](https://github.com/spotbugs/spotbugs/commit/ccdff0605e3447098b2e6f118d5f9e95fa05cb39) [Issue](https://github.com/spotbugs/spotbugs/issues/2222)
- **checkstyle/checkstyle** · 提交「Issue #22785: doc: Update regression report workflow in workflows README」 · 问题「Extend violationBetweenAnnotationAndMethod and violationBetweenJavadocAndMethod xpath to catch comments outside MODIFIERS」。[提交](https://github.com/checkstyle/checkstyle/commit/2e0f0c4b8dafa1de23a3d44012e6c3838dbbd54e) [Issue](https://github.com/checkstyle/checkstyle/issues/22775)
- **detekt/detekt** · 提交「Update plugin com.autonomousapps.build-health to v3.20.4 (#9903)」 · 问题「Dependency Dashboard」。[提交](https://github.com/detekt/detekt/commit/e2c7e71be037ea10eea51d5b8e0ff3d84bff0d84) [Issue](https://github.com/detekt/detekt/issues/4298)
- **gitleaks/gitleaks** · 提交「Merge pull request #2283 from gitleaks/dependabot/github_actions/acti…」 · 问题「Can't scan compressed files with unknown extension」。[提交](https://github.com/gitleaks/gitleaks/commit/5f6ec56cfa6f8c67648d863582c7ae2f206d5c1c) [Issue](https://github.com/gitleaks/gitleaks/issues/2010)
- **sverweij/dependency-cruiser** · 提交「fix(extract): ensures 'stub' modules only land once in the dc result …」 · 问题「Feature request: Support processing Astro files」。[提交](https://github.com/sverweij/dependency-cruiser/commit/7bf8972978e7e8b3de6cd70d1987d8ad64091a09) [Issue](https://github.com/sverweij/dependency-cruiser/issues/1007)
- **pahen/madge** · 近 30 天无默认分支提交 · 问题「TypeScript v6: aliased imports skipped when tsconfig has no baseUrl」。 [Issue](https://github.com/pahen/madge/issues/463)
- **thebjorn/pydeps** · 近 30 天无默认分支提交 · 问题「Support for namespace packages (PEP 420)」。 [Issue](https://github.com/thebjorn/pydeps/issues/19)
- **boyter/scc** · 提交「fix: skip a complexity check at the end of a file (#826)」 · 问题「SCC's representation and involvement in LineJudge」。[提交](https://github.com/boyter/scc/commit/324f65ef2045b0e37f5ba1c43e0b2f093345c4db) [Issue](https://github.com/boyter/scc/issues/769)
- **AlDanial/cloc** · 提交「bump version -> 2.13」 · 问题「Fix regex backtracking in Gradle/Groovy/Nextflow/Xtend filters」。[提交](https://github.com/AlDanial/cloc/commit/414fe1617b75fa2847844295ac423180db9ca4ea) [Issue](https://github.com/AlDanial/cloc/issues/999)
- **reviewdog/reviewdog** · 提交「fix(deps): update module github.com/mattn/go-shellwords to v1.0.17 (#…」 · 问题「Dependency Dashboard」。[提交](https://github.com/reviewdog/reviewdog/commit/f42d2a2763b3fee771905bdfe1b71269844c21e1) [Issue](https://github.com/reviewdog/reviewdog/issues/1040)
- **rustsec/rustsec** · 提交「fix(rustsec): ignore a git hook's GIT_INDEX_FILE when opening the adv…」 · 问题「Idea/Wish List: Generate the report in more than one format for cargo-audit」。[提交](https://github.com/rustsec/rustsec/commit/bd67dede0d87d24fe8f1e0964e985bb805bfb6e8) [Issue](https://github.com/rustsec/rustsec/issues/1756)
- **tree-sitter/tree-sitter** · 提交「fix(lib): declare Wasmtime C API static for wasm builds (#6157)」 · 问题「Generated lexer accepts a zero-width `end` token mid-input when a grammar has no internal tokens」。[提交](https://github.com/tree-sitter/tree-sitter/commit/564fa307125d78d51479b12dc079ac69abe46a30) [Issue](https://github.com/tree-sitter/tree-sitter/issues/6147)
- **tauri-apps/tauri** · 提交「feat(bundler): add TAURIDIR NSIS define (closes #14048) (#16267)」 · 问题「Dependency Dashboard」。[提交](https://github.com/tauri-apps/tauri/commit/e1d7dca25598b8a9843cec2de120337de5c19c34) [Issue](https://github.com/tauri-apps/tauri/issues/2552)
- **tauri-apps/plugins-workspace** · 提交「fix(ci): `webkit2gtk-driver` is now `webkitgtk-webdriver` on ubuntu 2…」 · 问题「Migrate notification, opener and single-instance to zbus 6」。[提交](https://github.com/tauri-apps/plugins-workspace/commit/21f01aa3d5d171f7e5deb5262480bb6ae20e3d0c) [Issue](https://github.com/tauri-apps/plugins-workspace/issues/3716)
- **sanks205/getobserver** · 提交「Add Community sample report, rule inventory, and reproducible detecti…」 · 无公开待处理问题样本。[提交](https://github.com/sanks205/getobserver/commit/10eb9e94e70612cb5a500311ff73c87627dbfc71)

## 功能差距与设计决定

| 能力 | CodeDoctor 基线 | 参考项目 | 决定 |
| --- | --- | --- | --- |
| 离线分析与历史 | 有证据、覆盖和差异 | CodeChecker、SonarQube | 保留桌面职责，提升历史与独立工具可用性 |
| 报告互操作 | 自有 JSON、Markdown、TXT、HTML | jscpd、Gitleaks、Staticcheck | 提供标准 SARIF，输出最小必要证据 |
| 自动化入口 | 桌面启动扫描 | Cppcheck、scc | 共享 Rust，独立 CLI、退出码和 stdout |
| 交付运维 | 手工构建下载 | Tauri、RustSec、reviewdog | 按平台验证、固定 Actions 和版本，产物签名更新 |
| 用户界面 | 十个桌面工作区 | CodeChecker 筛选与基线，Madge 关系和环路 | 保持已有工作流，在更新设置就近呈现状态及操作 |
| 平台适配 | Windows x64 已发行 | Biome 多入口，Tauri 多 Runner | 先验证 Windows 桌面和 Linux CLI |

## 33 项升级候选

| # | 候选 | 参考 | 优先 / 难度 | 决定 |
| --- | --- | --- | --- | --- |
| 1 | 一致版本校验 | Tauri | A / 低 | 实施 |
| 2 | PR CI 与必要检查 | reviewdog | A / 中 | 实施 |
| 3 | 标签发布与工件校验 | Tauri | A / 中 | 实施 |
| 4 | 签名绑定更新版本 | Tauri Updater | A / 高 | 实施 |
| 5 | 发布来源证明 | GitHub Actions | A / 中 | 实施 |
| 6 | Windows 安装升级验证 | Tauri | A / 高 | 实施 |
| 7 | 周期依赖审核 | RustSec | A / 低 | 实施 |
| 8 | Linux CLI | Cppcheck、scc | A / 中 | 实施 |
| 9 | SARIF 输出 | Staticcheck、Gitleaks | A / 中 | 实施 |
| 10 | CLI 严重度门禁 | jscpd | B / 低 | 实施 |
| 11 | 问题模板与私密报告 | Ruff、Semgrep | A / 低 | 实施 |
| 12 | 实机截图与支持矩阵 | CodeChecker | B / 低 | 实施 |
| 13 | 随包第三方许可 | OXC | A / 中 | 实施 |
| 14 | 历史基线筛选 | CodeChecker | B / 中 | 扩充现有差异视图 |
| 15 | 声明式规则配置 | Checkstyle | B / 中 | 后续扩充 |
| 16 | 项目排除规则 | cloc | B / 低 | 复用现有默认策略 |
| 17 | 重复基线门禁 | jscpd | B / 中 | 待统一历史模型 |
| 18 | PR 新增问题批注 | reviewdog | B / 中 | 先验证 SARIF |
| 19 | 报告敏感数据最小化 | Gitleaks | A / 低 | SARIF 只带必要诊断 |
| 20 | 可脚本化退出码 | golangci-lint | B / 低 | 实施 |
| 21 | CLI 持久取消 | scc | B / 中 | 先由调用进程管理 |
| 22 | 按语言说明覆盖率 | SpotBugs、Ruff | B / 低 | 保留现有矩阵 |
| 23 | 模块依赖约束 | dependency-cruiser | B / 高 | 待确定配置与误报成本 |
| 24 | 复杂度分项趋势 | Radon、Lizard | B / 中 | 后续扩充 |
| 25 | 多入口共享缓存 | Biome、OXC | B / 高 | 先验数据与并发边界 |
| 26 | 图导出与环路过滤 | Madge、pydeps | B / 中 | 后续桌面验证 |
| 27 | SARIF 标准校验 | PMD、Staticcheck | B / 低 | 实施 |
| 28 | Linux GUI 多发行版 | Tauri | C / 高 | 待图形及发行维护能力 |
| 29 | macOS 签名公证 | Tauri | C / 高 | 待证书及双架构验证 |
| 30 | 只读挂载容器 | scc、Cloc | C / 中 | CLI 发行稳定后评估 |
| 31 | 编译期深度分析 | SpotBugs | C / 高 | 引入构建执行需重新评估 |
| 32 | 浏览器扩展 | CodeChecker Web | D / 高 | 本地目录权限与目标不合 |
| 33 | 集中托管服务 | SonarQube | D / 高 | 改变隐私边界 |

平台适配见 [平台评估](platforms.md)。按 [维护流程](maintenance.md) 的周期复查版本、社区问题和许可证。上述第三方代码均作为参考。

## 2026-11-21 SARIF 与本地 CI 工作流复核

本次另行核读以下公开仓库的 README、许可证及相关实现；只借鉴架构与互操作思路，不复制代码。没有安装或基准测试第三方工具。性能、Windows 适配与 CodeDoctor 报告在各消费端的呈现均未实测。

| 项目与许可证 | 核验的设计 | 对 CodeDoctor 的启发 |
| --- | --- | --- |
| [semgrep/semgrep](https://github.com/semgrep/semgrep) · LGPL-2.1 | [扫描入口](https://github.com/semgrep/semgrep/blob/cd3b439c36f43a299dffcd1e7544f6ce7ebb03cc/cli/src/semgrep/commands/scan.py)、本地机器报告与[基线比较](https://github.com/semgrep/semgrep/blob/cd3b439c36f43a299dffcd1e7544f6ce7ebb03cc/cli/src/semgrep/run_scan.py) | 门禁失败必须区别于分析执行失败；显式报告状态，不把失败的基线比较说成“没有新增发现”。 |
| [biomejs/biome](https://github.com/biomejs/biome) · MIT / Apache-2.0 | [共享 workspace API](https://github.com/biomejs/biome/blob/73125bdb0326e211b796f1bf893e844609dc6b70/crates/biome_service/src/workspace.rs)，分离项目、诊断与代码操作 | 桌面和 CLI 继续复用同一 Rust 报告引擎；先测解析成本，再考虑增量缓存，不加常驻服务。 |
| [trunk-io/plugins](https://github.com/trunk-io/plugins) · MIT | [Ruff 插件描述](https://github.com/trunk-io/plugins/blob/581fae83afe3b48e62faf1e35c89757d115efbb1/linters/ruff/plugin.yaml)声名版本、文件类型、命令和输出格式 | 扩展能力需显式约束版本和输入；不自动下载、执行项目钩子或接入任意命令。Trunk CLI 仓库本次未成功核验，未将插件仓许可证误作 CLI 许可证。 |
| [SonarSource/sonarqube](https://github.com/SonarSource/sonarqube) · LGPL-3.0 | [质量门禁求值](https://github.com/SonarSource/sonarqube/blob/457cee5aabea2e1eabb151afb33f6c65a6e02e69/server/sonar-server-common/src/main/java/org/sonar/server/qualitygate/QualityGateEvaluatorImpl.java)和分析缓存 | 明确门禁条件和比较基线；保留 CodeDoctor 本地数据边界，不引入服务端。 |
| [reviewdog/reviewdog](https://github.com/reviewdog/reviewdog) · MIT | [SARIF 解析](https://github.com/reviewdog/reviewdog/blob/f42d2a2763b3fee771905bdfe1b71269844c21e1/parser/sarif.go)及[变更行过滤](https://github.com/reviewdog/reviewdog/blob/f42d2a2763b3fee771905bdfe1b71269844c21e1/filter/filter.go) | 过滤展示范围不等于扫描基线；本次使用 OASIS schema 做验证，未声称已在 reviewdog 或 GitHub 实测呈现。 |
| [Microsoft SARIF Tutorials](https://github.com/microsoft/sarif-tutorials) · 文档 CC BY-4.0、代码 MIT | [SARIF 字段样例](https://github.com/microsoft/sarif-tutorials/tree/819b0f62f47ecde9a8f24dfc387c41926f5edabe/samples)与[工具说明](https://github.com/microsoft/sarif-tutorials/blob/819b0f62f47ecde9a8f24dfc387c41926f5edabe/docs/Multitool.md) | 对照 OASIS 2.1.0 schema 验证报告；优先给出规则引用和可靠文件位置，不默认嵌入源码。 |
| [Ericsson/codechecker](https://github.com/Ericsson/codechecker) · Apache-2.0 | [架构](https://github.com/Ericsson/codechecker/blob/53222cd5d7566c553d8d7edbb5134a35a56ef15e/docs/architecture.md)区分分析器、报告转换和可选 Web 服务 | 先支持安全的报告互操作；导入目录/ZIP 不触发构建或执行外部代码。 |

上述比较支持本次的低依赖改进：SARIF 结果引用去重的规则表、区分分析执行状态与门禁退出码，并避免输出猜测的位置；文件保存对话框只在其异步选择期间锁定对应表单。下一步应以真实消费端验证行级注释与跨扫描去重，再评估是否增加 SARIF 上传或新增问题基线门禁。

## 2026-11-21 桌面扫描器与报告生态复核

本次复核了下列公开项目的 README、相关实现和许可证声明，只借鉴方案，不复制代码或新增运行时依赖。社区活动仅用于判断维护状态，不代表性能对比；仓库许可证也不覆盖所有规则集和传递依赖。

| 参考项目与许可证 | 已核验的做法 | 可借鉴方向与边界 |
| --- | --- | --- |
| [semgrep/semgrep](https://github.com/semgrep/semgrep) · LGPL-2.1 | [CLI 文档](https://docs.semgrep.dev/cli-reference)包含机器报告、超时、并发和内存控制；架构区分 Python CLI 与 OCaml 分析引擎。 | 将诊断与工具执行状态分开；本地分析不等于完全离线，网络遥测、规则获取需独立核实。 |
| [oxsecurity/megalinter](https://github.com/oxsecurity/megalinter) · AGPL-3.0 | [插件描述](https://github.com/oxsecurity/megalinter/blob/main/docs/plugins.md)及超时、分组和报告配置集中描述多工具集成；SARIF 仅聚合支持它的工具。 | 为适配器明确工具版本、覆盖和失败状态；插件/命令会扩大执行信任边界，不宜照搬至导入项目。 |
| [golangci/golangci-lint](https://github.com/golangci/golangci-lint) · GPL-3.0 | [架构文档](https://golangci-lint.run/docs/contributing/architecture/)分离运行、过滤、去重和输出阶段。 | 保持分析、归一化和展示分层；其共享 Go 类型信息优化不可直接推断适用于 Tree-sitter 多语言管线。 |
| [microsoft/sarif-vscode-extension](https://github.com/microsoft/sarif-vscode-extension) · MIT | [查看器](https://github.com/microsoft/sarif-vscode-extension/blob/main/README.md)提供可筛选结果、详情、键盘导航和源码位置对接。 | 继续优先完善结果筛选与定位；这是 SARIF 阅读器，不是扫描引擎或历史数据库。 |
| [SonarSource/sonarqube](https://github.com/SonarSource/sonarqube) · LGPL-3.0 | [问题生命周期](https://docs.sonarsource.com/sonarqube-community-build/user-guide/issues/solution-overview)及[历史维护](https://docs.sonarsource.com/sonarqube-community-build/project-administration/maintaining-project/managing-project-history)区分持久问题状态和分析快照。 | 可研究保留期及问题状态语义；本地 SQLite 产品无需引入其服务端架构或商业版能力。 |
| [tauri-apps/plugins-workspace](https://github.com/tauri-apps/plugins-workspace) · Apache-2.0 OR MIT | [SQL 插件文档](https://v2.tauri.app/plugin/sql/)采用版本化迁移和受限权限。 | 本项目继续由窄 Rust 命令访问 SQLite，不开放前端任意 SQL；插件许可与 API 不代表加密或沙箱能力。 |

另看了 [Bearer](https://github.com/Bearer/bearer)：其源码标注 Elastic License 2.0（并非宽松开源许可证）；基线报告、指纹忽略和 JSON/SARIF/HTML 输出可供交互设计参考，但不在本次开放源码许可项目统计中。所有许可证均为仓库/版本层面的初步核验，集成前仍应审查具体组件、规则和传递依赖。

本轮综合结论：离线扫描项目应明确分析完整性、工具失败和中断状态；保留诊断时应有长度/控制字符边界，避免路径和源码内容流入共享报告；大量本地源文件的总资源限制需先实测再设计。安全和性能观察是后续候选，不把未复现的影响描述成已发生缺陷。

## 2026-11-21 凭据预览与报告证据边界复核

本次实际通过 GitHub API 重读以下五仓库的 README、许可证及所列实现/文档，核对未归档状态；没有安装这些工具、运行第三方性能基准或复制代码。功能说明与本项目的实际源码审查对照，不把工具自身的性能宣传视作 CodeDoctor 的测量结果。

| 仓库与已核验许可 | 功能、架构与交互 | 数据、安全和扩展边界 | CodeDoctor 对照与本轮决定 |
| --- | --- | --- | --- |
| [Gitleaks](https://github.com/gitleaks/gitleaks) · MIT | Go CLI 面向 Git、目录和 stdin 的凭据扫描，支持报告；本次 README 明确功能冻结、以后以安全补丁为主，不能称为持续新增功能。 | [Finding.Redact](https://github.com/gitleaks/gitleaks/blob/master/report/finding.go)同步替换 Line、Match 和 Secret；CLI 源码许可不能代替 GitHub Action 的独立许可条件。 | 本项目不仅有源码预览，还有规则证据、缓存和历史报告；只隐藏一个视图不够。采用同一敏感行分类，在证据进入缓存前处理。 |
| [Ruff](https://github.com/astral-sh/ruff) · MIT | Rust Python 检查/格式化与编辑器集成；[配置文档](https://github.com/astral-sh/ruff/blob/main/docs/configuration.md)说明层级配置及 `--isolated`。 | `--isolated` 忽略配置；格式化/修复属于另一类写操作。可复用诊断格式，不等于允许执行项目脚本。 | 保留现有可选 Ruff 的隔离只读入口，不为本次安全修复引入新工具或依赖。 |
| [Semgrep](https://github.com/semgrep/semgrep) · LGPL-2.1 | 规则扫描引擎与 Python CLI，机器格式化与引擎分层；[SARIF formatter](https://github.com/semgrep/semgrep/blob/develop/cli/src/semgrep/formatter/sarif.py)从结构化诊断生成输出。 | README 区分社区引擎与专有规则/能力；远程规则配置和指标上报的网络行为需单独核查。 | 保持本机分析、诊断与渲染分离；不把标准 SARIF 当作其它报告已完成脱敏的证明。没有复制 LGPL 实现或引入联网规则。 |
| [Biome](https://github.com/biomejs/biome) · MIT OR Apache-2.0 | Rust Web 工具链，CLI/编辑器使用共享诊断服务；[Workspace](https://github.com/biomejs/biome/blob/main/crates/biome_service/src/workspace.rs)明确线程安全、打开文档与缓存生命周期。 | 共享引擎减少不同客户端的行为漂移；缓存必须随语义失效。它并非 SQLite 历史审阅桌面应用。 | 将预览与逐文件规则证据的分类提取为共享小模块；规则版本升至 `rules-4`，不复用旧脱敏语义缓存。 |
| [Trivy](https://github.com/aquasecurity/trivy) · Apache-2.0 | Go CLI 聚合漏洞、配置、许可和凭据检测；[Secret Scanning](https://github.com/aquasecurity/trivy/blob/main/docs/guide/scanner/secret.md)列出规则、允许路径及遮蔽输出示例。 | 凭据覆盖由规则与排除边界决定；未命中不代表不存在凭据，漏洞数据库等网络需求不等于本地扫描。 | 凭据遮蔽仍明示为保守启发式；不把新的敏感行判断包装为全量 DLP，不新增大型扫描依赖。 |

这些项目主要提供 CLI 或编辑器诊断；本次没有对其 GUI、安装器、资源占用或实际扫描速度作实测比较。CodeDoctor 保留 Windows 十工作区、SQLite 历史、离线目录/ZIP 与 Linux CLI 的定位，不引入中央服务。新增回归覆盖带类型注解的多行值，以及规则→缓存→数据库→五种报告的完整证据通路；历史记录与已导出的文件不被自动改写，分享前仍需检查。

## 2026-11-21 发布收敛：缓存身份与字面报告

本次在 00:07–00:18 UTC 通过 GitHub API 实际重读下列五个仓库的 README、许可证和所列实现；五仓均未归档。这里只记录相对前述研究新增的具体证据，不把推送日期等同于发布日期，不复制第三方代码，不声称实测过其界面或性能。

| 项目及本次核验许可 | 具体实现证据 | CodeDoctor 对照与决定 |
| --- | --- | --- |
| [Semgrep](https://github.com/semgrep/semgrep) · LGPL-2.1 | [OCaml SARIF 输出](https://github.com/semgrep/semgrep/blob/develop/src/osemgrep/reporting/Sarif_output.ml)会写入源码 snippet；[metrics 说明](https://github.com/semgrep/semgrep/blob/develop/metrics.md)区分配置与遥测。 | 本地执行不保证导出无源码；继续对整条 Issue、缓存和五格式报告断言无合成凭据，不仅检查 evidence。 |
| [Biome](https://github.com/biomejs/biome) · MIT OR Apache-2.0 | [Workspace](https://github.com/biomejs/biome/blob/main/crates/biome_service/src/workspace.rs)统一客户端 API，并描述 AST/诊断缓存生命周期。 | 维持 Rust 核心与薄界面；参数定位由解析器统一提供，复合文件坐标同样映射回原文，保留旧序列化结果兼容性。 |
| [Ruff](https://github.com/astral-sh/ruff) · MIT，含第三方声明 | [缓存](https://github.com/astral-sh/ruff/blob/main/crates/ruff/src/cache.rs)隔离版本、项目与配置；所读文件缓存键包含 mtime/权限，缓存状态为 linted/formatted，而非任意诊断全文。 | 借鉴语义版本失效，不照搬 mtime 为内容一致性保证；保留本项目内容哈希与完整配置键，规则语义升为 `rules-5`。 |
| [reviewdog](https://github.com/reviewdog/reviewdog) · MIT | [SARIF parser](https://github.com/reviewdog/reviewdog/blob/master/parser/sarif.go)优先 Markdown message、保留原始结果；[diff filter](https://github.com/reviewdog/reviewdog/blob/master/filter/diff_filter.go)只控制呈现范围。 | 互操作格式不是净化器，展示过滤也不是增量分析正确性；对 Markdown 字段和多行证据分别处理上下文。 |
| [SonarQube](https://github.com/SonarSource/sonarqube) · 根 LGPL v3；所读 Java 文件 LGPL-3.0-or-later | [报告提交](https://github.com/SonarSource/sonarqube/blob/master/server/sonar-webserver-webapi/src/main/java/org/sonar/server/ce/ws/SubmitAction.java)进入异步任务并检查分析权限；[缓存持久化](https://github.com/SonarSource/sonarqube/blob/master/server/sonar-ce-task-projectanalysis/src/main/java/org/sonar/ce/task/projectanalysis/step/PersistScannerAnalysisCacheStep.java)按分支事务替换。 | 保留本地 SQLite 阶段事务；在落库前确保最终指纹唯一、真实位置不同的发现不被吞并。不引入服务端，也不外推商业产品或分析器的许可。 |

报告渲染另对照 [CommonMark 0.31.2 官方规范](https://github.com/commonmark/commonmark-spec/blob/0.31.2/spec.txt)：段落不能跨空行，行内代码将换行归一为空格；普通字段可转义 ASCII 标点，原始 HTML 默认不转义，多行证据宜使用独立围栏。采用超过证据中最长反引号串的围栏，并用 MIT 许可的 `pulldown-cmark` 0.13.6 在测试中实际解析事件，断言不可信文本没有生成 HTML、链接或图片。该依赖仅用于开发测试，未新增发行运行时依赖。下游查看器的非标准扩展、清洗和网络设置仍需单独审查。
