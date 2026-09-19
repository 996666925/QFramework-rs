//! 工具层（`IUtility`）。
//!
//! Utility 是纯粹的基础设施封装，**拿不到架构**（`IUtility` 没有任何能力接口）。
//! 这不是限制，而是保证：它可以被单测、可以被替换成 mock、可以脱离游戏运行。
//!
//! 需要访问游戏数据时，由调用它的 System / 引擎代码把数据传进去。

pub mod log;
pub mod save;

pub use log::LogUtility;
pub use save::SaveUtility;
