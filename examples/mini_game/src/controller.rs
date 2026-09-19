//! 表现层（`IController`）。
//!
//! 表现层只做两件事：
//!
//! - 把**输入**翻译成命令（[`ScriptController`] 扮演这个角色，用固定剧本模拟玩家）
//! - 把**状态变化**翻译成表现（[`HudController`] 扮演这个角色）
//!
//! 它**不能**：直接改 Model、发送事件、使用 Utility。
//!
//! 注意控制器以 `Arc` 共享，所有可变状态都必须靠内部可变性
//! （`Atomic*`、`Mutex`、`BindableProperty`）。

pub mod hud;
pub mod script;

pub use hud::HudController;
pub use script::ScriptController;
