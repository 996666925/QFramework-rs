//! Godot Node 直接实现 QFramework 的 `IController`。
//!
//! 同时派生 `GodotClass` 和 `IController`，用 `#[controller(architecture = MyApp)]`
//! 自动绑定应用架构，也可在节点回调中显式注入独立架构。
//! Godot 负责节点生命周期和场景访问；QFramework 提供 Model / System、Command /
//! Query 和同步事件订阅能力。回调可直接捕获 `Gd`，[`ControllerInbox`] 用于可选的延后处理。

mod controller;
mod inbox;
mod unregister;

pub use controller::{BindArchitectureError, NodeControllerExt};
pub use inbox::ControllerInbox;
pub use qframework_core;
pub use unregister::GodotUnRegisterExt;

/// Godot Controller 的常用能力与派生宏。
pub mod prelude {
    pub use crate::{
        BindArchitectureError, ControllerInbox, GodotUnRegisterExt, NodeControllerExt,
    };
    pub use qframework_core::prelude::*;
}
