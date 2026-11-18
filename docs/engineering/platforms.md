# 平台决策

2026-11-18 · 产品保留本地分析能力，依赖项目内容的工作流默认离线。正式交付以实际通过构建、运行及对应分发检查的平台为准。评级 A 为本轮首要维护，B 为有实际价值的独立入口，C 为待评估，D 为与当前边界冲突。

| 平台 | 级别 | 技术、体验、权限和成本 | 分发与更新 |
| --- | --- | --- | --- |
| Windows 11 x64 | A | WebView2、原生文件与 ZIP、键鼠、拖入、SQLite、源码定位均可用。既有桌面入口最低维护成本。 | NSIS、MSI、免安装 EXE。已发布 x64。 |
| Windows 10 22H2 x64 | A | 与既有构建兼容性预期一致，仍需在该系统单独验证。系统维护依赖 ESU 或适用的 LTSC，WebView2 有独立生命周期。 | 相同 x64 安装包，系统矩阵单列实测状态。 |
| Windows ARM64 | C | Rust 和 WebView2 可用，需对应签名安装及真机验证。现有 x64 发行未覆盖。 | 候选 ARM64 EXE/MSI。 |
| Linux x64 CLI（Ubuntu、Debian、Fedora、Arch） | B | Rust / SQLite 核心可独立于桌面 WebView 构建。支持 shell 和 CI 的目录、ZIP 分析与机器报告。对各发行版的实际运行条件以验证记录为准。 | 优先使用 x86-64 CLI 归档，校验来源与 SHA-256 后显式替换。 |
| Linux 桌面（Ubuntu、Debian） | C | WebKitGTK 4.1 和桌面文件管理器可提供原生窗口。仍需处理 GTK 依赖、源码定位、桌面权限和升级验证。 | AppImage / DEB，需原生图形和更新测试。 |
| Linux 桌面（Fedora、Arch） | C | GTK、Wayland、ABI 和发行版打包需要各自测试，现有 Windows 桌面验证无法覆盖。 | RPM、AppImage 或发行版包。Flatpak 需验证挂载权限与分析器沙盒。 |
| macOS Intel | C | Rust 可移植，WKWebView 和文件选择可复用。需要 Intel 运行与覆盖升级、Gatekeeper 签名及公证。 | 有验证环境和证书后评估 DMG。 |
| macOS Apple Silicon | C | 独立 arm64 构建、集成验证和公证。现有环境无 macOS 日常真机验证。 | DMG / APP，按 Apple 分发规则。 |
| Android 手机 | C | SAF 可导入 ZIP，整目录、超大项目、暂停恢复和外部 Ruff 需重做，手机工作流收益有限。 | APK/AAB 需要移动 UI、设备验证和正式签名。 |
| Android 平板与折叠屏 | C | 大屏有双栏价值，窗口尺寸、铰链、休眠和后台任务需实机测试。 | 与 Android 渠道协同。 |
| iOS 与 iPadOS | C | 文档选择和 ZIP 可实现，目录和后台运行受沙盒约束。签名、审核、UI 和更新另有维护成本。 | App Store 及 TestFlight，待实际设备与签名。 |
| 桌面 Web | C | WASM 可承载部分规则。浏览器文件访问、持久化和原生工具能力有差异。服务端上传会改变源码保留和权限边界。 | 明确独立价值后再评估部署。 |
| 移动 Web、响应式 Web、PWA | C | 小屏报告查看可行，离线大项目、浏览器存储配额、Service Worker 升级与数据恢复需专项设计。 | 部署与浏览器缓存机制，待验证端到端体验。 |
| Chrome、Edge、Firefox 扩展 | D | 产品需要分析本地完整目录。浏览器扩展权限与该核心工作流不匹配，新增各商店兼容成本。 | 当前产品无扩展发行渠道。 |
| Docker 与 NAS | C | CLI 将来可进入只读挂载容器。需要验证用户 ID、卷访问和 Linux 架构。 | 待确定镜像需求和签名维护主体。 |
| 私有化服务、云服务器 | D | 多租户、鉴权、隔离队列、源码存留与离线产品的信任边界不同。 | 本轮继续维护本地分析。 |

参考 [Tauri 分发指南](https://v2.tauri.app/distribute/) 和 [Microsoft WebView2 生命周期](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-support-lifecycle)。
