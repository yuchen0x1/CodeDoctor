//! Complexity and bad-smell rules driven by parser metrics: long functions,
//! too many parameters, deep nesting, high cyclomatic/cognitive complexity,
//! god classes, and file overload.

use super::{IssueDraft, Rule, RuleContext, clamp};
use crate::domain::types::{CoverageKind, Severity};

fn function_metrics_usable(coverage: CoverageKind) -> bool {
    coverage == CoverageKind::Ast || coverage == CoverageKind::Structural
}

pub struct LongFunctionRule;

impl Rule for LongFunctionRule {
    fn id(&self) -> &'static str {
        "complexity.long-function"
    }
    fn category(&self) -> &'static str {
        "complexity"
    }
    fn default_severity(&self) -> Severity {
        Severity::Medium
    }
    fn supports(&self, _language: &str, coverage: CoverageKind) -> bool {
        function_metrics_usable(coverage)
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let Some(parse) = ctx.parse else {
            return vec![];
        };
        let limit = ctx.config.max_function_lines;
        let mut drafts = vec![];
        for function in &parse.functions {
            let Some(length) = function.metrics.length else {
                continue;
            };
            if length <= limit {
                continue;
            }
            let severity = if length > limit * 2 {
                Severity::High
            } else {
                Severity::Medium
            };
            drafts.push(
                IssueDraft::new(
                    self.id(),
                    self.category(),
                    severity,
                    function.range.start_line,
                    function.range.end_line,
                )
                .evidence(clamp(
                    format!("函数 `{}` 共 {length} 行", function.name),
                    200,
                ))
                .title("超长函数")
                .explanation(format!(
                    "函数 `{}` 共 {length} 行，超过阈值 {limit} 行{}。",
                    function.name,
                    if length > limit * 2 {
                        "（超过阈值两倍）"
                    } else {
                        ""
                    }
                ))
                .impact("函数难以单独测试与理解，通常承担多个职责，是修改出错的高发区。")
                .recommendation("按职责拆分为多个小函数；先补齐特征测试再重构，避免改变行为。")
                .confidence(0.95),
            );
            if drafts.len() >= 10 {
                break;
            }
        }
        drafts
    }
}

pub struct TooManyParamsRule;

impl Rule for TooManyParamsRule {
    fn id(&self) -> &'static str {
        "complexity.too-many-params"
    }
    fn category(&self) -> &'static str {
        "complexity"
    }
    fn default_severity(&self) -> Severity {
        Severity::Medium
    }
    fn supports(&self, _language: &str, coverage: CoverageKind) -> bool {
        function_metrics_usable(coverage)
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let Some(parse) = ctx.parse else {
            return vec![];
        };
        let limit = ctx.config.max_parameters;
        let mut drafts = vec![];
        for function in &parse.functions {
            let Some(count) = function.metrics.parameter_count else {
                continue;
            };
            if count <= limit {
                continue;
            }
            let severity = if count > limit * 2 {
                Severity::High
            } else {
                Severity::Medium
            };
            drafts.push(
                IssueDraft::new(
                    self.id(),
                    self.category(),
                    severity,
                    function.range.start_line,
                    function.range.start_line,
                )
                .evidence(clamp(
                    format!("函数 `{}` 有 {count} 个参数", function.name),
                    200,
                ))
                .title("参数过多")
                .explanation(format!(
                    "函数 `{}` 有 {count} 个参数，超过阈值 {limit}。",
                    function.name
                ))
                .impact("调用点容易传错顺序，扩展时修改面大。")
                .recommendation("引入参数对象/结构体，或拆分函数职责。")
                .confidence(0.95),
            );
            if drafts.len() >= 10 {
                break;
            }
        }
        drafts
    }
}

pub struct DeepNestingRule;

impl Rule for DeepNestingRule {
    fn id(&self) -> &'static str {
        "complexity.deep-nesting"
    }
    fn category(&self) -> &'static str {
        "complexity"
    }
    fn default_severity(&self) -> Severity {
        Severity::Medium
    }
    fn supports(&self, _language: &str, coverage: CoverageKind) -> bool {
        function_metrics_usable(coverage)
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let Some(parse) = ctx.parse else {
            return vec![];
        };
        let limit = ctx.config.max_nesting;
        let mut drafts = vec![];
        for function in &parse.functions {
            let Some(depth) = function.metrics.nesting_depth else {
                continue;
            };
            if depth <= limit {
                continue;
            }
            drafts.push(
                IssueDraft::new(
                    self.id(),
                    self.category(),
                    self.default_severity(),
                    function.range.start_line,
                    function.range.end_line,
                )
                .evidence(clamp(
                    format!("函数 `{}` 控制流嵌套 {depth} 层", function.name),
                    200,
                ))
                .title("嵌套过深")
                .explanation(format!(
                    "函数 `{}` 内部控制流嵌套达 {depth} 层，超过阈值 {limit}。",
                    function.name
                ))
                .impact("条件路径组合爆炸，边界情况难以人工验证。")
                .recommendation("提前返回/卫语句，或提取内层逻辑为独立函数。")
                .confidence(0.9),
            );
            if drafts.len() >= 10 {
                break;
            }
        }
        drafts
    }
}

pub struct HighCyclomaticRule;

impl Rule for HighCyclomaticRule {
    fn id(&self) -> &'static str {
        "complexity.high-cyclomatic"
    }
    fn category(&self) -> &'static str {
        "complexity"
    }
    fn default_severity(&self) -> Severity {
        Severity::Medium
    }
    fn supports(&self, _language: &str, coverage: CoverageKind) -> bool {
        function_metrics_usable(coverage)
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let Some(parse) = ctx.parse else {
            return vec![];
        };
        let limit = ctx.config.max_cyclomatic;
        let mut drafts = vec![];
        for function in &parse.functions {
            let Some(value) = function.metrics.cyclomatic_complexity else {
                continue;
            };
            if value <= limit {
                continue;
            }
            let severity = if value > (limit as f32 * 1.5) as u32 {
                Severity::High
            } else {
                Severity::Medium
            };
            drafts.push(
                IssueDraft::new(
                    self.id(),
                    self.category(),
                    severity,
                    function.range.start_line,
                    function.range.end_line,
                )
                .evidence(clamp(
                    format!("函数 `{}` 圈复杂度 {value}", function.name),
                    200,
                ))
                .title("圈复杂度过高")
                .explanation(format!(
                    "函数 `{}` 圈复杂度 {value}，超过阈值 {limit}（分支决策点过多）。",
                    function.name
                ))
                .impact("测试路径数量随分支指数增长，难以完整覆盖。")
                .recommendation("拆分分支为独立函数或使用策略/查表替代条件链。")
                .confidence(0.9),
            );
            if drafts.len() >= 10 {
                break;
            }
        }
        drafts
    }
}

pub struct HighCognitiveRule;

impl Rule for HighCognitiveRule {
    fn id(&self) -> &'static str {
        "complexity.high-cognitive"
    }
    fn category(&self) -> &'static str {
        "complexity"
    }
    fn default_severity(&self) -> Severity {
        Severity::Medium
    }
    fn supports(&self, _language: &str, coverage: CoverageKind) -> bool {
        function_metrics_usable(coverage)
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let Some(parse) = ctx.parse else {
            return vec![];
        };
        let limit = ctx.config.max_cognitive;
        let mut drafts = vec![];
        for function in &parse.functions {
            let Some(value) = function.metrics.cognitive_complexity else {
                continue;
            };
            if value <= limit {
                continue;
            }
            drafts.push(
                IssueDraft::new(
                    self.id(),
                    self.category(),
                    self.default_severity(),
                    function.range.start_line,
                    function.range.end_line,
                )
                .evidence(clamp(
                    format!("函数 `{}` 认知复杂度 {value}", function.name),
                    200,
                ))
                .title("认知复杂度过高")
                .explanation(format!(
                    "函数 `{}` 认知复杂度 {value}，超过阈值 {limit}（嵌套权重过高）。",
                    function.name
                ))
                .impact("阅读与评审成本高，修改时容易破坏隐含前提。")
                .recommendation("扁平化嵌套结构，合并相邻的重复条件。")
                .confidence(0.9),
            );
            if drafts.len() >= 10 {
                break;
            }
        }
        drafts
    }
}

pub struct GodClassRule;

impl Rule for GodClassRule {
    fn id(&self) -> &'static str {
        "complexity.god-class"
    }
    fn category(&self) -> &'static str {
        "structure"
    }
    fn default_severity(&self) -> Severity {
        Severity::Medium
    }
    fn supports(&self, _language: &str, coverage: CoverageKind) -> bool {
        function_metrics_usable(coverage)
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let Some(parse) = ctx.parse else {
            return vec![];
        };
        let method_limit = ctx.config.max_class_methods;
        let line_limit = ctx.config.max_class_lines;
        let mut drafts = vec![];
        for class in &parse.classes {
            let span = class
                .range
                .end_line
                .saturating_sub(class.range.start_line)
                .saturating_add(1);
            let methods = class.method_count.unwrap_or(0);
            if methods > method_limit {
                drafts.push(
                    IssueDraft::new(self.id(), self.category(), Severity::High, class.range.start_line, class.range.end_line)
                        .evidence(clamp(format!("类 `{}` 有 {methods} 个方法", class.name), 200))
                        .title("巨型类嫌疑")
                        .explanation(format!("类 `{}` 包含 {methods} 个方法，超过阈值 {method_limit}，可能承担多个职责。", class.name))
                        .impact("职责过多导致改动互相影响，测试面大。")
                        .recommendation("按业务职责拆分类；先识别内聚的成员子集再迁移。")
                        .confidence(0.85),
                );
            } else if span > line_limit {
                drafts.push(
                    IssueDraft::new(
                        self.id(),
                        self.category(),
                        self.default_severity(),
                        class.range.start_line,
                        class.range.end_line,
                    )
                    .evidence(clamp(format!("类 `{}` 跨 {span} 行", class.name), 200))
                    .title("类规模过大")
                    .explanation(format!(
                        "类 `{}` 代码跨 {span} 行，超过阈值 {line_limit}。",
                        class.name
                    ))
                    .impact("类内逻辑过多，定位与评审困难。")
                    .recommendation("提取独立职责为协作类或模块。")
                    .confidence(0.85),
                );
            }
            if drafts.len() >= 10 {
                break;
            }
        }
        drafts
    }
}

pub struct FileOverloadRule;

impl Rule for FileOverloadRule {
    fn id(&self) -> &'static str {
        "complexity.file-overload"
    }
    fn category(&self) -> &'static str {
        "structure"
    }
    fn default_severity(&self) -> Severity {
        Severity::Medium
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let Some(parse) = ctx.parse else {
            return vec![];
        };
        let limit = ctx.config.max_file_lines;
        let code_lines = parse.metrics.code_lines;
        if code_lines <= limit {
            return vec![];
        }
        let functions = parse.metrics.function_count.unwrap_or(0);
        let imports = parse.metrics.import_count.unwrap_or(0);
        vec![
            IssueDraft::new(self.id(), self.category(), self.default_severity(), 1, 1)
                .evidence(clamp(
                    format!(
                        "文件有效代码约 {code_lines} 行，{functions} 个函数/方法，{imports} 个导入"
                    ),
                    200,
                ))
                .title("文件职责过载嫌疑")
                .explanation(format!(
                    "单文件有效代码约 {code_lines} 行，超过阈值 {limit}，通常混合了多个不相关职责。"
                ))
                .impact("文件成为修改热点，合并冲突与误改概率高。")
                .recommendation("按业务职责拆分文件/模块；优先拆出独立的工具函数与数据结构。")
                .confidence(0.8),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_gate_accepts_structural() {
        assert!(LongFunctionRule.supports("Python", CoverageKind::Ast));
        assert!(LongFunctionRule.supports("Vue", CoverageKind::Structural));
        assert!(!LongFunctionRule.supports("PowerShell", CoverageKind::Lexical));
    }
}
