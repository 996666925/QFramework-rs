//! # QFramework for Rust —— Bevy 集成层
//!
//! 把 [`qframework_core`] 的四层架构接入 Bevy ECS：
//!
//! - [`QFrameworkPlugin`]：把架构放入 Bevy 世界，成为 [`QArchitecture`] 资源；
//! - [`QArchitecture`]：`Deref` 到 [`qframework_core::Architecture`]，可在任意系统里使用；
//! - [`QEventBridgePlugin`]：把 QFramework 事件桥接成 Bevy 的 [`Message`](bevy::prelude::Message)；
//!
//! 表现层直接使用 Bevy 系统，通过 `NonSend<QArchitecture>` 访问业务架构。
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
//!         ArchitectureBuilder::new().model(MyModel::default())
//!     }
//! }
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(MinimalPlugins)
//!         .install_architecture::<MyApp>()
//!         .bridge_messages::<MyEvent>()
//!         .add_systems(Update, my_system)
//!         .run();
//! }
//!
//! fn my_system(architecture: NonSend<QArchitecture>) {
//!     architecture.send_command(DoSomething);
//! }
//! ```

pub mod app;
pub mod bridge;

pub use app::{AppQFrameworkExt, QApplication, QArchitecture, QFrameworkPlugin};
pub use bridge::{QEventBridge, QEventBridgePlugin};

/// Bevy + QFramework 的常用导入集合。
pub mod prelude {
    pub use crate::{
        AppQFrameworkExt, QApplication, QArchitecture, QEventBridge, QEventBridgePlugin,
        QFrameworkPlugin,
    };
    pub use qframework_core::prelude::*;
}
