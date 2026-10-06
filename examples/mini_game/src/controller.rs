//! Controller 表现层：使用 Bevy 系统处理输入和 HUD 刷新。

pub mod hud;
pub mod script;

pub use hud::{HudState, render_hud, setup_hud};
pub use script::run_script;
