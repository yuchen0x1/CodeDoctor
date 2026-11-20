# CodeDoctor UI/UX 工程改造与验收报告

日期：2026-11-20；工作分支：`chore/prepare-0.2.1`；版本：`0.2.1`。

> 本报告区分代码修复、自动化回归、真实原生验证和未测范围。没有提交、推送、发布或运行安装器。测试通过不代表产品不存在其他问题。

## 1. 【体检总览】

### 平台与技术栈

- 目标产品是 **Windows x64 桌面应用**：Tauri 2、Rust edition 2024、WebView2；不是移动 App。
- 前端为 React 19、TypeScript 6、Vite 8、原生 CSS；没有引入 Tailwind、UI 组件库或新路由库。
- Linux 分发为 CLI，不以 Linux GUI、macOS 或移动平台的交互标准冒充本轮验收范围。
- 页面由 `src/app/App.tsx` 的 PageId 状态切换；保留全部 10 页与原有业务结构。
- 主题仍为设置中的 `dark` / `light`，保存后应用；没有新增 system 配置或更改 AppSettings/API。
- 原生窗口配置仍为 1280×820、最小 960×640；640px 浏览器用例是额外重排压力测试。

### 完整界面清单

| 页面/独立模块 | 检查范围 | 本轮覆盖 |
| --- | --- | --- |
| 总览仪表盘 | 项目、统计卡片、问题分布、历史、评分明细 | 双主题布局、真实扫描、加载/失败回归 |
| 项目体检 | 文件夹/ZIP/路径入口、项目预览、开始扫描 | 双主题、原生目录和 ZIP 选择/取消 |
| 扫描任务 | 阶段、进度、当前文件、暂停/恢复/取消、分析器状态 | 双主题、真实扫描、状态播报测试；未穷举每种中断时序 |
| 问题分析 | 7 类筛选/排序、展开、源码、分页、JSON 导出 | 双主题、真实筛选与源码、边界测试、原生保存/取消 |
| 架构分析 | 覆盖说明、关系筛选、SVG、选择器、文件列表、循环组 | 真实 14 节点、键盘测试、双主题图和标签复查 |
| 注释治理 | 分类结果、证据和空状态 | 双主题、真实分类数据 |
| 重复代码 | 分类结果、相似度/来源证据和空状态 | 双主题、真实分类数据 |
| 修复提示词 | 模式、范围、定点选择、预览、复制/MD/TXT 导出 | 真实生成 5,237 字任务书；MD 保存框打开/取消；其他分支由现有测试覆盖，未全程手工穷举 |
| 历史报告 | 基线/目标/报告快照、比较、格式、路径、导出 | 选择竞态测试、原生保存/取消；真实 UI 只有一个完成快照，未手工比较两次扫描 |
| 设置 | 主题、排除目录、10 项阈值、Ruff、保存/默认值 | 双主题保存与恢复、字段错误/focus 回归 |
| 软件更新区域 | 版本、检查频率、检查/失败、发布记录 | 存储故障与组件回归；没有触发实际安装 |
| 更新详情 dialog | 标题、说明、安装/稍后/详情、进度、Escape/focus | 代码与挂载测试；真实候选弹窗视觉与下载交接未完整验收 |
| 全局组件 | 导航、通知条、按钮、输入/选择、状态点、代码块、空/加载/失败 | Token、状态、焦点、窄窗与测试 |
| 原生对话框（5 类） | 项目文件夹、项目 ZIP、检测报告、治理提示词、筛选问题清单 | 全部实际打开并取消；没有把取消流程记作写盘成功 |
| 独立 HTML 报告 | 完整文档、主题、问题/证据、响应式、打印 | 真实 CLI 输出、结构/转义测试、浏览器配色及宽度验证 |

没有新增自绘菜单或通知浮层；下拉选择和文件对话框继续采用浏览器/操作系统原生行为。导航图标仍是 Unicode 符号，SVG 用于架构图，没有引入外部图标资源。

### 工程评审评分

以下是按固定维度给出的**人工工程评审估分**，不是用户研究、性能测量或 WCAG 认证。改造前依据起点源码和截图，改造后依据实现、测试与本报告列出的实测；未测范围保留扣分。

| 维度 | 满分 | 改造前 | 改造后 | 主要依据 |
| --- | ---: | ---: | ---: | --- |
| 双主题与可读性 | 30 | 12 | 27 | 架构浅色残留修复；对比度、53 色 Token 完整映射 |
| 交互状态与反馈 | 20 | 9 | 18 | 禁用/按下/失败/重试、独立结果加载、焦点修复 |
| 布局与窗口适配 | 20 | 10 | 18 | 80 个最终生产包布局用例；真实长路径和架构标签 |
| 键盘与无障碍语义 | 20 | 8 | 16 | disclosure、SVG、字段说明、dialog、live status；未做读屏器全流程 |
| 独立报告体验 | 10 | 2 | 8 | 完整中文文档、双主题、窄窗、打印配色；未实际打印 |
| **合计** | **100** | **41** | **87** | 不以满分掩盖平台和验收边界 |

## 2. 【问题清单】

计数口径：按根因/修复单元归为 **21 组**，不是按每个选择器、颜色或测试断言计数。仅统计本轮 UI/报告问题，不把此前解析列号、缓存、发布 CI 等修复重复计入。P2 为正确性/主要可用性，P3 为一致性/维护性；本轮未确认 P0/P1，不代表其他范围绝对没有。

**21 组已实施修复：P2 16 组、P3 5 组。** 表中的“回归”不自动等同于完整原生视觉验收。

| ID | 级别 | 确认的问题 | 实际修复与证据 | 当前状态 |
| --- | --- | --- | --- | --- |
| U01 | P2 | 架构样式独立硬编码暗色，浅色输入、警告和图形不匹配 | ArchitectureView.css 全部使用语义 Token；真实双主题图复查 | 已修复；原生+自动化 |
| U02 | P3 | 主题颜色散落，light 补丁难以覆盖组件 | 组件 CSS 直接颜色字面量 120→0；53 个双主题颜色 Token | 已修复；静态契约测试 |
| U03 | P2 | 暗色次要文字及浅色残留文字对比不足 | faint、code、warning、severity 等成对映射；见对比度表 | 已修复；计算+关键视觉 |
| U04 | P3 | 后置导航透明背景覆盖 active 背景 | 合并 nav 状态规则，当前页保留边框/底色/aria-current | 已修复；双主题截图 |
| U05 | P2 | 禁用主要靠 opacity，text-button 状态不完整，部分焦点被裁切 | 明确 disabled/hover/pressed/focus Token；issue 容器不再裁切焦点 | 已修复；样式+组件回归 |
| U06 | P3 | 通用 panel 最大宽度限制内容，路径和行布局易挤压 | 面板铺满可用区，设置单独限宽，minmax(0,…) 与长文本换行 | 已修复；窗口矩阵+真实路径 |
| U07 | P2 | 窄宽直接隐藏导航文字/状态与问题置信度 | 全标签导航可滚动，置信度换行而非隐藏 | 已修复；80 用例+CSS 契约 |
| U08 | P2 | 更新 dialog 缺少整体视口高度限制 | max-height 基于 100dvh、dialog 与说明独立滚动 | 已修复；自动化，真实候选视觉未完整验收 |
| U09 | P2 | 更新详情命名、描述、进度、键盘焦点恢复不完整 | unique ids、标题 focus、Escape/返回触发按钮、progress/status | 已修复；5 项 Updates 挂载测试 |
| U10 | P2 | 设置错误与目录字段关联不足，重复失败缺少焦点引导 | aria-describedby/invalid、关联 error、每次失败 focus | 已修复；挂载测试 |
| U11 | P2 | SVG img 语义不适合内部按钮，Space 默认滚动，选中状态不明确 | group + aria-pressed；Enter/Space preventDefault；列表同步 | 已修复；原生选择+键盘回归 |
| U12 | P2 | 问题展开缺少稳定控制关系，源码滚动块不可键盘聚焦 | useId/aria-controls/hidden；源码 tabIndex 与名称 | 已修复；原生双主题源码+回归 |
| U13 | P2 | findings 刷新变短后 offset 越界导致空页 | 有效 pageOffset 夹到最后一页；101→51→1 测试 | 已修复；回归 |
| U14 | P2 | 源码读取失败后不能原地重试 | 明确重试入口，不离开问题即可重新读取 | 已修复；失败→成功测试 |
| U15 | P3 | 扫描阶段缺少独立、低噪声状态播报 | 永久 role=status，只播报阶段/状态而非逐文件刷屏 | 已修复；ScanStatus 测试 |
| U16 | P2 | HTML 导出缺完整文档/语言/viewport 和可读排版 | 独立中文 HTML、内联 CSS、语义区块；保留 escaping/API | 已修复；Rust+真实 CLI+浏览器 |
| U17 | P2 | 深色环境打印分支仍继承深色卡片/文字 Token | print 显式重置整套主题 Token，状态码避免断成单字 | 已修复；CSS 激活验证+Rust；未实际打印 |
| U18 | P2 | 二审：问题加载失败仍展示 5 个零分布，且分布依赖 score | loadedIssues 区分 null/[]，四态展示与评分独立 | 已修复；新增失败与独立加载断言 |
| U19 | P2 | 二审：结果/源码重试按钮卸载导致焦点落到 BODY | 结果重试移到稳定 h1；源码重试保留按钮/guard，成功后移到上下文标题 | 已修复；新增 focus/single-flight 断言 |
| U20 | P3 | 二审：设置 invalid 边框被更高 specificity 覆盖；重试禁用仍有 hover | scoped invalid 规则；disabled/aria-disabled 排除 hover/active | 已修复；CSS 契约，重点样式检查 |
| U21 | P2 | 最终真实图中标签放在圆下方，覆盖相邻圆 | 标签沿半径向外布局、按方向锚定；viewBox 留足标签空间 | 已修复；最终原生深/浅复查+布局断言 |

关键源码：`src/styles/tokens.css`、`src/styles/app.css`、`src/app/ArchitectureView.css`、`src/app/App.tsx`、`src/app/IssuesView.tsx`、`src/app/SettingsView.tsx`、`src/app/Updates.tsx`、`src-tauri/src/report/html.rs`、`src-tauri/src/report/report.css`。

## 3. 【主题残留专项报告】

### 源码证据

改造前基线可通过 `git show HEAD:<path>` 核对；本轮起点截图与最终截图见下表。

| 改造前证据 | 风险 | 改造后 |
| --- | --- | --- |
| HEAD ArchitectureView.css:63 `color: #fde68a`，:122 `background: #0d1829` | warning 及输入框没有可靠浅色映射 | warning/control Token，各主题成对映射 |
| HEAD app.css:329 `color: #b8c7dc`，:333 `color: #c5d5e8` | 证据/源码文字沿用暗色配色 | code-text/code-bg、text/muted 语义色 |
| HEAD app.css:284 `.button:disabled { opacity: .45; }` | 禁用文字随容器变淡，text-button 未完整覆盖 | 独立 disabled 前景/背景，不靠整块透明 |
| HEAD app.css:145 active 导航，:279 又设透明背景 | 当前页底色被级联覆盖 | 单一模块化状态规则 |
| HEAD app.css:396 隐藏 nav label/status/footer，:402 隐藏 confidence | 窄窗丢失信息而不是解决排版 | 全文字导航与换行保留信息 |

按 `hex / rgb(a)` 字面量统一统计：

| 组件样式 | 改造前 | 改造后 |
| --- | ---: | ---: |
| src/styles/app.css | 96 | 0 |
| src/app/ArchitectureView.css | 24 | 0 |
| **合计** | **120** | **0** |

这表示颜色从组件抽离，不表示原先 120 处全是错误。`tokens.css`、独立报告 palette 及明确的 print palette 仍包含合法字面量。强制颜色分支使用 Canvas/CanvasText/Highlight 等系统语义颜色，不强行改成品牌色。

### 对比度抽样与自动约束

采用 WCAG 相对亮度公式；以下为声明色值的静态配对。旧 warning 的 1.21:1 是底色配对示例，不冒充半透明合成后的精确像素结果。

| 配对 | 改造前 | 改造后 |
| --- | ---: | ---: |
| 暗色 faint / panel | 4.49:1 | 7.47:1 |
| 浅色架构输入 text / control | 1.21:1 | 正常 text/control 配对，契约测试≥4.5:1 |
| 浅色 warning / warning-bg | 静态底色示例 1.21:1 | 6.56:1 |
| 浅色 faint / panel | — | 6.23:1 |
| 浅色 accent / panel | — | 5.96:1 |
| 暗色 accent / panel | — | 11.12:1 |
| 浅色 critical / critical-bg | — | 6.29:1 |
| 暗色 critical / critical-bg | — | 7.32:1 |
| 浅色 disabled text / bg | — | 4.65:1 |
| 暗色 disabled text / bg | — | 6.62:1 |
| 浅色 control-border / control | — | 3.77:1 |
| 暗色 control-border / panel | — | 3.59:1 |

`src/styles/theme.test.ts` 对常用表面上的 text/muted/faint/accent、五级 severity、code/status/on-accent 设置 ≥4.5:1 断言；control-border、focus、graph 关系色设置 ≥3:1 断言。这是 Token 契约，不是全页面逐像素 WCAG 认证。

### 截图证据

| 场景 | 改造前 | 最终改造后 |
| --- | --- | --- |
| 浅色总览 | [before-light](assets/ui-ux-2026-11-20/before-light-overview.png) | [after-final-light](assets/ui-ux-2026-11-20/after-final-light-overview.png) |
| 深色总览 | [before-dark](assets/ui-ux-2026-11-20/before-dark-overview.png) | [after-final-dark](assets/ui-ux-2026-11-20/after-final-dark-overview.png) |
| HTML 窄窗 | 无原样式截图 | [dark 640](assets/ui-ux-2026-11-20/html-dark-640.png)、[light 640](assets/ui-ux-2026-11-20/html-light-640.png) |
| HTML 打印配色 | 深色卡片继承已由计算样式确认 | [print palette](assets/ui-ux-2026-11-20/html-print-palette.png) |

总览图是在普通浏览器中拍摄，缺少 Tauri invoke bridge，因此包含“内核不可用 / invoke”环境提示；原生应用已另行运行并确认“本地内核已就绪”。没有注入假的 bridge 或以静态结果代替扫描。通知条/状态并非全部严格相同，不作像素差分结论。

原生截图已在交互过程中展示：最终深色图 `frame-9e66519a`、浅色图 `frame-d74ee2f6`、浅色源码 `frame-59748510`、生产包深色总览 `frame-80f19dc2`、浅色总览 `frame-2f46e5d0`。这些是会话帧引用，不伪装成仓库 PNG 文件。

## 4. 【改造方案与实际修改】

1. **设计系统收口**：新增 `src/styles/tokens.css`，组件消费同一套语义色，不再依赖不断叠加 light selector 补丁。
2. **全局样式整理**：按导航、按钮/输入、统计、导入、问题、设置、历史、更新与响应式组织既有 CSS；保留不确定使用的 legacy graph selector 并令其使用 Token，没有随意删组件。
3. **布局修复**：去掉全局 panel 720px 限宽；main/sidebar 可滚动、flex 内容不被短窗口压缩；长路径允许换行；两栏在适当宽度堆叠；窄宽不隐藏重要信息。
4. **主题初始化**：`src/main.tsx` 在 React render 前用既有 readSettings 设置 data-theme，App effect 继续同步保存后的主题。原生重启主题保持已验证；未以高速摄影测量首帧闪烁。
5. **交互与无障碍**：稳定 disclosure id、可聚焦源码/发布说明、字段错误关联、SVG pressed 状态、低噪声扫描播报、dialog 焦点和进度语义。
6. **错误恢复**：问题/评分独立加载；findings readiness 与 score 解耦；只重试失败的结果；分页夹取有效页；源码可原地重试并保留键盘焦点。
7. **架构图**：图例进入正常文档流；inspector 可堆叠；路径列表完整换行；SVG group 暴露内部按钮；最终标签向外布局，完整路径仍可通过标题和列表查看。
8. **独立报告**：HTML render API 不变；完整 lang/viewport/charset 文档、inline CSS，无外部 JS/字体/样式；全部扫描数据继续转义；print 明确重置深色 palette。
9. **文档**：README 与 0.2.1 更新记录补充本轮改造，不自动发布版本。

保留原有数据结构、Tauri/Rust API、页面 PageId、SQLite 历史快照、扫描/报告/更新安全边界。没有放宽 CSP，没有删除用户功能，没有用隐藏元素掩盖布局问题。凭据证据遮蔽与更新校验继续保留。

## 5. 【设计 Token 建议】

以下不是空泛提案，均与当前 `src/styles/tokens.css` 一致。建议后续组件继续消费语义 Token，不复制颜色；独立 HTML 报告保持无前端目录依赖的自足 palette。

### 完整颜色映射（53 个）

表中名称均省略统一的 `--color-` 前缀。

| Token | dark | light |
| --- | --- | --- |
| bg | #0b1220 | #f2f5fa |
| bg-raised | #101b2c | #e8eef6 |
| panel | #142137 | #ffffff |
| panel-muted | #1b2e49 | #e8eef6 |
| border | #304460 | #c5d0de |
| border-strong | #617995 | #74859b |
| text | #edf3fc | #182a40 |
| text-muted | #b5c5db | #405570 |
| text-faint | #a1b2c9 | #50627a |
| accent | #67e8f9 | #006d85 |
| accent-strong | #22d3ee | #00586e |
| accent-soft | #163b4d | #d9edf3 |
| on-accent | #082430 | #ffffff |
| control | #0e192a | #ffffff |
| control-border | #617995 | #74859b |
| control-hover | #233a56 | #e3edf5 |
| control-pressed | #2c4965 | #d3e2ef |
| disabled-bg | #1c2b40 | #e8edf4 |
| disabled-text | #a1b2c9 | #586b82 |
| code-bg | #0c1726 | #f0f4f9 |
| code-text | #d4e2f4 | #263d58 |
| track | #304460 | #c5d0de |
| selection | #23536a | #cee7ef |
| ok | #6ee7b7 | #176344 |
| ok-bg | #103c33 | #e0f3e9 |
| error | #fda4af | #a51d3e |
| error-bg | #481f32 | #fce8ed |
| warning | #fde68a | #745000 |
| warning-bg | #3c321c | #fff3cd |
| severity-critical | #fda4af | #a51d3e |
| severity-critical-bg | #481f32 | #fce8ed |
| severity-high | #fdba74 | #913b09 |
| severity-high-bg | #442c1e | #fff0df |
| severity-medium | #fde68a | #745000 |
| severity-medium-bg | #3c321c | #fff3cd |
| severity-low | #7dd3fc | #075d86 |
| severity-low-bg | #17384f | #e0f2fe |
| severity-info | #cbd5e1 | #405570 |
| severity-info-bg | #2a3648 | #e8eef6 |
| hero-start | #143449 | #e2f2f6 |
| hero-end | #142137 | #ffffff |
| hero-border | #42617e | #91acbe |
| dropzone | #102c40 | #eaf3f8 |
| dropzone-hover | #173b50 | #deedf5 |
| overlay | rgba(2, 9, 18, .72) | rgba(18, 35, 55, .48) |
| graph-edge | #7dd3fc | #086b96 |
| graph-cycle | #fda4af | #b52148 |
| graph-clone | #c4b5fd | #7250b4 |
| graph-node | #18455f | #dceef7 |
| graph-selected | #236b7b | #b9e3ee |
| graph-selected-border | #a5f3fc | #00586e |
| graph-label-halo | #142137 | #ffffff |
| focus | #67e8f9 | #006d85 |

### 阴影、尺寸、字体与动效

| Token/类别 | 当前值 | 应用原则 |
| --- | --- | --- |
| shadow-panel | dark: 0 2px 8px rgba(0,0,0,.12)；light: 0 2px 8px rgba(28,48,76,.05) | 轻量层次，不用阴影替代边界 |
| shadow-dialog | dark: 0 24px 80px rgba(0,0,0,.4)；light: 0 24px 80px rgba(28,48,76,.24) | 与 backdrop 分工 |
| radius-sm/md/lg | 6 / 10 / 14px | 控件 / 容器 / 大卡片 |
| space-1…6 | 4 / 8 / 12 / 16 / 24 / 32px | 后续逐步复用，不为机械 Token 化重写所有局部间距 |
| control-height | 40px | 主要表单控件高度，不代表所有次级按钮都强制 40px |
| sidebar-width | 224px；≤1100 时 192px | ≤760 改顶部全标签滚动导航 |
| font-size-xs/sm/md/lg/xl | 12 / 13 / 14 / 18 / 26px | SVG 标签另使用 viewBox 单位，不冒充等价 CSS 像素 |
| font-sans | Segoe UI Variable Text / Segoe UI / Microsoft YaHei UI / Microsoft YaHei / system-ui | 本机系统字体，不增加外部字体请求 |
| font-mono | Cascadia Mono / Consolas / SFMono-Regular | 路径、代码和证据 |
| duration-fast / ease-standard | 140ms / ease-out | hover/边界反馈；reduced-motion 降低动效 |
| focus-ring | 2px solid var(--color-focus) | focus-visible；forced-colors 使用系统 Highlight |

## 6. 【改造优先级路线图】

| 优先级 | 工作 | 状态/发布要求 |
| --- | --- | --- |
| 本轮先修 | 双主题残留、文字/控件对比、导航和 disabled/focus | 已完成代码与对应验证 |
| 本轮再修 | 窄窗、源码/设置/SVG/dialog 语义、分页和错误恢复 | 已完成；dialog 实际候选视觉仍是验收缺口 |
| 独立二审闭环 | readiness 零值误导、重试焦点、CSS 状态级联 | 3 组发现已修；主助手追加 5 项回归测试 |
| 最终视觉闭环 | 真实架构标签覆盖与报告深色打印继承 | 已修；原生图与打印 CSS 配色复测 |
| 发布前门禁 | 隔离 NSIS/MSI 安装升级、签名更新/备份/安装交接 | 未执行，不绕过隔离 guard，不在本机直接跑安装器 |
| 后续质量验收 | DPI/多显示器、读屏器、高对比模式实际运行、超大图、真实打印分页 | 未完成；不能由 CSS 窗口测试替代 |
| 既有维护债 | Clippy 风格警告、上游 RustSec 警告、受代理阻塞的 Linux 依赖追踪 | 独立于本轮 UI 改造，不宣称清零 |

没有为了上游维护警告强行升级不兼容依赖，也没有把原生系统对话框改成外观一致但行为退化的自绘文件选择器。

## 7. 【测试验收结果】

### 构建与自动化

| 验证 | 结果 | 边界/说明 |
| --- | --- | --- |
| pnpm typecheck | 通过 | strict TS |
| pnpm test | **12 文件、113 项通过** | 其中 App 14、Views.accessibility 6、Updates 5、theme 6、ScanStatus 1；其余模型/桥接/设置测试保留 |
| pnpm build | 通过 | 36 modules；CSS 35.11 kB / gzip 6.93；JS 304.40 kB / gzip 92.57；不是性能基准 |
| cargo test --lib | **160 通过，0 失败** | 最终 Rust/报告实现 |
| report 定向测试 | **11 通过** | 含完整文档、转义、字段、无外部依赖、print palette 完整性 |
| cargo fmt -- --check | 通过 | 无格式错误 |
| git diff --check | 通过 | 有 Windows LF→CRLF 提示，不是 patch whitespace 失败 |
| node scripts/release/version.mjs --check | 通过，0.2.1 | README/更新记录/包版本一致 |
| 真实 CLI HTML 输出 | 通过 | fixtures 14 文件、37 articles，完整独立文档；没有脚本/外部样式资源 |

前端测试保留 Node 26 的 localStorage ExperimentalWarning；测试对存储异常做了明确模拟与安全降级，不把该环境警告写成零日志。jsdom 的 showModal/close 是测试替身，不能代替 WebView 原生 focus trap 的全流程验收。

### 最终生产包双主题布局矩阵

每个用例都执行导航激活，再核对 h1、根主题与 main/root 的 scrollWidth/clientWidth，只有页面确实切换才计入。使用 DOM activation 测布局，不把它冒充全部真实鼠标操作。最初工具点击超时或页面未切换的批次不计通过；已对最终构建重跑完整矩阵。

| CSS viewport | dark 10 页 | light 10 页 | 横向溢出 |
| --- | --- | --- | --- |
| 640×640 | 10/10 | 10/10 | 无；低于原生最小宽度的压力用例 |
| 960×640 | 10/10 | 10/10 | 无 |
| 1280×820 | 10/10 | 10/10 | 无 |
| 1920×1080 | 10/10 | 10/10 | 无 |
| **合计** | **40/40** | **40/40** | **80 个用例通过** |

浏览器没有 Tauri bridge，所以该矩阵主要覆盖无项目/空状态的页面结构和设置表单；真实数据布局由下方原生测试补充。未以此声称大量数据和所有 DPI 均无溢出。

### Windows 原生真实数据验收

使用隔离标识 `com.codedoctor.ui-audit`，避免改动正常用户数据库。最终一轮运行的是构建后的 dist 前端和真实 Tauri debug 内核，不再受开发时多步骤编辑/HMR 中间态干扰；这不是已发布安装包验收。

- 实际导入并扫描仓库 fixtures：14 文件、120 行、37 个问题，质量分 58.2；原生仪表盘显示严重 0、高危 6、一般 3、提示 21、信息 7。
- 架构实际显示 14 节点、10 条依赖证据、1 条重复关系、1 个循环组。节点选择和循环筛选已执行，筛选后图中为循环组的 3 个节点。
- 原生浅/深 10 页均已打开；长问题页受自动化 AX 树截断影响，问题页单独验证，不把截断当作导航消失。
- 深色和浅色均实际展开本地 `parser/metrics.py` 源码，行高亮和说明可见；浅色路径搜索将问题列表筛到 3/37。
- 最终标签向外布局在原生深/浅截图中复查，不再以先前旧半径截图作为新布局证据。
- 保存浅色/深色设置已执行；冷启动/重启后主题、fixtures 项目和完成扫描恢复正常。未声称逐帧测得完全无闪烁。
- 真实生成全面治理提示词，预览 5,237 字；原生 MD 保存框打开/取消后预览仍在。
- 项目目录、项目 ZIP、检测报告、治理提示词、筛选后问题清单五类原生对话框均实际打开并取消。报告取消后路径仍为空、导出按钮禁用；问题清单默认为 JSON。**没有把这些取消流程记为每种格式均已实际写盘。**
- 独立 HTML 在 640/1280 宽度实际渲染 37 articles，无根横向溢出。深色跟随当前系统；浅色通过仅在预览页禁用 dark media branch 检查原有 light 规则。打印通过预览页激活 print 规则核对 body/section 白色、标题黑色；未声称修改了 OS 主题或完成了打印机分页验收。

### 独立二审与诊断说明

独立只读审查先返回 U18/U19/U20 三组具体问题；随后实施修复。主助手检查实现并补上 5 项回归测试，以及图标签位置断言，最后复测生产构建和 80 个布局用例。

多步骤前端编辑期间，原生开发 WebView 曾出现空白内容；冷启动及最终静态生产包真实操作恢复正常。**HMR 空白根因未被证明或清零**，不能归结为已修复的发布 Bug，也不能将开发中间态错误隐瞒为全程零异常。

### 明确未通过/未覆盖范围

- 真实新版本候选弹窗的所有尺寸视觉、真实下载、签名验证后交给安装器、备份/升级/回滚流程。
- 隔离 NSIS/MSI 安装升级 CI 尚未在本次环境实际执行；本机安装器 guard 未绕过。
- Windows 多档 OS DPI、多显示器切换、读屏器完整操作、高对比模式实际运行。源码中已有 reduced-motion/forced-colors 支持，不等于系统级验收已通过。
- 超大/高密度依赖图、极大量历史/问题数据的交互性能、真实打印分页与打印机结果。
- Linux CLI runtime 的完整平台回归。第一阶段已有 CLI 测试/冒烟，不能把 Windows 运行当 Linux 实测。
- 既有 strict Clippy `-D warnings` 不通过：历史风格警告尚在；标准 Clippy 曾 exit 0，不等于零警告。
- 上游 RustSec：chrono 0.4.45 / proc-macro-error 1.0.4 的维护警告未清零；glib 0.18.5 告警的 Linux 完整依赖追踪受本机代理阻塞。未把这些描述成修复或安全审计全部通过。

## 8. 【最终结论】

本轮不是只给设计建议：21 组确认的 UI/报告问题已实际修改，并完成对应的自动化或明确范围内的原生验证；独立二审发现的 3 组问题已闭环，最终又修正了真实架构图标签与打印 palette。

最终可确认：113 项前端测试、160 项 Rust 测试、生产构建、格式/版本检查通过；最终生产包双主题 80 个页面/尺寸用例通过；真实原生扫描、双主题、源码、架构、提示词与五类对话框取消流程已验证。

**不能得出的结论：所有问题都不存在、所有功能和平台全部验收、已达到可无条件发布状态。** 发布前仍需隔离安装升级、真实签名更新、OS 级无障碍/DPI 和剩余诊断范围。此前解析/缓存、项目恢复竞态、历史选择、语言筛选与更新偏好等功能修复均保留，没有因本轮视觉改造回退或删除。

工作区尚未提交、推送或发布。为便于复核，本会话启动的隔离原生窗口、1420 生产包本地预览和 1421 HTML 报告预览暂时保留运行；它们不是安装完成的发布版本。
