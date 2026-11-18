# 语言能力矩阵（Language Capability Matrix）

更新日期：2026-11-18 · 解析器版本对应 `src-tauri/Cargo.toml` 中的 grammar crate 版本。

语言名称先由扩展名识别，再进入解析器注册表。扩展名识别和语法覆盖是两件事：例如 Kotlin、Swift 能被识别，但当前只进入词法回退。

覆盖等级定义（诚实边界，不做虚假宣称）：

| 等级 | 含义 |
|---|---|
| `ast` | 真实 tree-sitter 语法树解析；函数/类/导入/调用/控制流/注释/指标均来自语法节点。文件存在语法错误时降级为 `structural` 并保留诊断。 |
| `structural` | 复合文件（Vue/Svelte/HTML）按嵌入 script 块分区解析，行号与字节范围映射回原文件；模板与样式不做完整解析。语法错误的 grammar 文件也降级为此级并保留诊断。 |
| `lexical` | 该语言暂无语法解析器：仅统计行数与注释（词法扫描），函数/类/导入指标不参与统计，并输出 `coverage` 诊断。 |

## 语法解析（tree-sitter）

| 语言 | grammar crate | 覆盖 | 提取内容 |
|---|---|---|---|
| Python | tree-sitter-python 0.23 | ast | 函数/类/继承/静态导入（含相对导入）/调用/注释/docstring/控制流/全部指标 |
| JavaScript / JSX | tree-sitter-javascript 0.23 | ast | 函数（含箭头函数父级命名）/类/导入（ESM、`require()`、动态 `import()`）/调用/JSX 组件引用/注释/控制流/指标 |
| TypeScript | tree-sitter-typescript 0.23 | ast | 同 JS，另含 interface/enum、implements、property_signature |
| TSX | tree-sitter-typescript (tsx) 0.23 | ast | 同 TS + JSX 组件引用 |
| Java | tree-sitter-java 0.23 | ast | 方法/构造器/类/接口/enum/record/继承与接口实现/导入/调用/反射类动态调用 |
| Go | tree-sitter-go 0.23 | ast | 函数/方法（receiver 归属）/struct、interface/导入/调用 |
| Rust | tree-sitter-rust 0.23 | ast | 函数/struct、enum、trait（impl 块方法聚合到类型）/use 导入/调用/宏调用/`pub` 导出标记 |
| C | tree-sitter-c 0.23 | ast | 函数（declarator 链命名）/`#include`/调用/控制流 |
| C++ | tree-sitter-cpp 0.23 | ast | 同 C + class/struct/方法/成员/基类（base_class_clause） |
| C# | tree-sitter-c-sharp 0.23 | ast | 方法/类/接口/struct/record/enum/using/调用（invocation、new）/property |
| PHP | tree-sitter-php 0.23 | ast | 函数/方法/类/接口/trait/命名空间 use/调用（含动态 `eval`、`call_user_func` 族） |

指标口径：函数长度 = 语法节点行区间；参数数 = 参数容器命名子节点；branch_count = if/elif/for/while/do/case 臂/catch/三元/`&&`、`||`、`and`、`or`、`??` 决策点；nesting_depth = 函数体内控制流嵌套层数；cyclomatic = branch_count + 1；cognitive = Σ(1 + 内层嵌套层级)。

## 复合文件（分区解析）

| 类型 | 处理 | 覆盖 |
|---|---|---|
| Vue SFC | 提取全部 `<script>`/`<script lang="ts"|"tsx">` 块，用对应 grammar 解析，范围映射回原文件 | structural |
| Svelte | 同 Vue | structural |
| HTML | 内联 `<script>`（忽略 `src=` 外链）按 JavaScript/TypeScript 解析；模板本身不做完整 AST 分析 | structural |

## 词法级（lexical，明确不提供 AST 指标）

| 语言 | 说明 |
|---|---|
| Kotlin | 社区 grammar crate（tree-sitter-kotlin 0.3.8）锁死 tree-sitter 0.21 运行时，与 0.25 原生链接冲突，暂缓接入；词法级。待上游升级后启用。 |
| Swift | 社区 grammar 兼容性未验证，暂缓接入；词法级。 |
| Shell / PowerShell | 无成熟 grammar 接入；词法级（支持 `#` 注释统计）。 |
| CSS / SCSS | 词法级（`/* */`、`//` 注释统计）。 |
| SQL | 词法级（`--`、`/* */`）。 |
| JSON / YAML / XML / Dockerfile | 词法级；不产出函数/类/导入指标。 |
| 其他未知类型 | 词法级兜底，输出 `coverage` 诊断。 |

## 跨文件架构分析

跨文件架构图会将可定位的相对路径和部分语言的模块命名约定与扫描索引匹配。Python 静态导入和 ECMAScript 家族的显式导入绑定在具备匹配的目标声明时，可以支持调用、继承和 JSX 引用等额外关系。继承识别取决于相应语法提取器是否提供基类记录，调用和 JSX 引用也仅覆盖可匹配的形式。图保留证据行号，扇入、扇出和循环依赖按不同文件对计算。

架构页将已索引的独立文件与已保存的关系一并展示。重复代码关系提供双方的文件及行号。耦合度和循环依据静态依赖关系计算。运行时动态分派与模块路径、任意构建别名、传递式重导出、Vue/Svelte 模板和完整的跨文件符号解析属于部分覆盖范围，历史快照中无法查看未解析导入清单。

## 与规则引擎的关系

- `ast` 覆盖语言运行适用于该语言的内部规则，包括复杂度、结构、注释治理与有语法证据的安全模式。
- `structural` 覆盖语言获得 script 块内的全部内部规则；模板/样式仅参与词法规则。
- `lexical` 覆盖语言仅获得词法规则（超长行、注释治理、安全正则模式），复杂度/结构规则不参与，评分相应降低证据覆盖。

Python 的 `quality.unreachable-statement` 检查函数体中直接位于 `return` 或 `raise` 之后的同层语句。它跳过作用域指令和定义，不推断条件分支是否恒真、间接调用是否终止或其他文件能否访问某段代码。

Python 的 `quality.unused-local-variable` 仅检查直接赋值给单个局部名称且在函数语法树中无其他同名标识符的情况。嵌套作用域和 f-string 引用会计入判断。已知帧检查函数、作用域指令、Unicode 标识符和超过分析上限的文件会降低或阻止此类报告。复核时保留初始化表达式的必要副作用。

## 解析失败和指标口径

- grammar 可加载但源文件存在语法错误时，仍返回已提取的节点，并把覆盖标记为 `structural`；诊断会带有错误位置。
- grammar 初始化失败时返回词法级结果和解析器诊断，不会补造函数、类或导入数量。
- `line_count`、`code_lines` 和 `comment_lines` 来自解码后的文本；函数长度使用语法节点的起止行。
- 分支数统计控制流节点、case/select 臂、三元表达式和 `&&`、`||`、`and`、`or`、`??`；圈复杂度为分支数加一，认知复杂度按嵌套深度累计。
- 反射、动态导入、插件注册和依赖注入只能记录为疑似调用或关系，不能证明运行时完整拓扑。
