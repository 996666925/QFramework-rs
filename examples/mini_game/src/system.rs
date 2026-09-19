//! 业务逻辑层（`ISystem`）。
//!
//! System 的存在意义是「承载可被多个表现层复用的逻辑」。本示例里的两个系统
//! 分别代表两种典型用法：
//!
//! - [`AchievementSystem`]：监听事件 → **跨 Model 协调**（数据层做不到）
//! - [`AutoSaveSystem`]：监听事件 → **使用 Utility 落盘**（表现层做不到）

pub mod achievement;
pub mod auto_save;

pub use achievement::AchievementSystem;
pub use auto_save::AutoSaveSystem;
