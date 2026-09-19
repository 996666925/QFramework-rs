//! 日志工具。

use qframework_core::prelude::*;

/// 统一输出格式的日志工具。
///
/// Utility 没有 `arch` 字段——它也拿不到架构。
#[derive(IUtility)]
pub struct LogUtility {
    indent: &'static str,
}

impl LogUtility {
    /// 创建日志工具。
    pub fn new() -> Self {
        Self { indent: "  ·" }
    }

    /// 一条普通信息。
    pub fn info(&self, message: impl std::fmt::Display) {
        println!("{} {message}", self.indent);
    }

    /// 一条警告。
    pub fn warn(&self, message: impl std::fmt::Display) {
        println!("{} [警告] {message}", self.indent);
    }

    /// 一个分节标题。
    pub fn section(&self, title: impl std::fmt::Display) {
        println!();
        println!("── {title} ──");
    }
}
