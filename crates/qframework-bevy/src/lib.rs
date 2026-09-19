//! # QFramework for Rust —— Bevy 集成层
//!
//! 把 [`qframework_core`] 的四层架构接入 Bevy ECS：
//!
//! - [`QFrameworkPlugin`]：把架构放入 Bevy 世界，成为 [`QArchitecture`] 资源；
//! - [`QArchitecture`]：`Deref` 到 [`qframework_core::Architecture`]，可在任意系统里使用；
//! - [`QEventBridgePlugin`]：把 QFramework 事件桥接成 Bevy 的 [`Message`](bevy::prelude::Message)；
//! - [`QControllers`]：让实现了 [`QControllerUpdate`] 的控制器每帧被驱动。
//!
//! ## 快速开始
//!
//! ```ignore
//! use bevy::prelude::*;
//! use qframework_bevy::prelude::*;
//! use qframework_core::prelude::*;
//!
//! struct MyApp;
//!
//! impl QApplication for MyApp {
//!     fn build() -> ArchitectureBuilder {
//!         ArchitectureBuilder::new("MyApp").model(MyModel::default())
//!     }
//! }
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(MinimalPlugins)
//!         .add_qframework::<MyApp>()
//!         .bridge_q_messages::<MyEvent>()
//!         .add_systems(Update, my_system)
//!         .run();
//! }
//!
//! fn my_system(architecture: Res<QArchitecture>) {
//!     architecture.send_command(DoSomething);
//! }
//! ```

pub mod app;
pub mod bridge;
pub mod controller;

pub use app::{AppQFrameworkExt, QApplication, QArchitecture, QFrameworkPlugin};
pub use bridge::{QEventBridge, QEventBridgePlugin};
pub use controller::{QControllerUpdate, QControllers, QFrameworkSet};

/// Bevy + QFramework 的常用导入集合。
pub mod prelude {
    pub use crate::{
        AppQFrameworkExt, QApplication, QArchitecture, QControllerUpdate, QControllers,
        QEventBridge, QEventBridgePlugin, QFrameworkPlugin, QFrameworkSet,
    };
    pub use qframework_core::prelude::*;
}
