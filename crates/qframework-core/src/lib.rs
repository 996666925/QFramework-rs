//! # QFramework for Rust（核心）
//!
//! 这是 [QFramework](https://github.com/liangxiegame/QFramework) 的 Rust 实现核心，
//! 与引擎无关，只依赖标准库。若要在 Bevy 中使用，请配合 `qframework-bevy`。
//!
//! ## 四层架构
//!
//! QFramework 用四层 + Command 来组织代码，并强制单向依赖（上层可以直接调用下层，
//! 下层只能通过事件向上层通信）：
//!
//! | 层 | 接口 | 派生宏 | 可访问 |
//! |---|---|---|---|
//! | 表现层 | [`IController`] | [`IController`](derive@IController) | System、Model（只读）、Command、Query、注册事件 |
//! | 业务逻辑层 | [`ISystem`] | [`ISystem`](derive@ISystem) | System、Model、Utility、发送/注册事件 |
//! | 数据层 | [`IModel`] | [`IModel`](derive@IModel) | Utility、发送事件 |
//! | 工具层 | [`IUtility`] | [`IUtility`](derive@IUtility) | 无 |
//!
//! 除四层外还有 [`ICommand`]：只负责改变状态，不能持有状态、不能注册事件。
//!
//! ## 能力接口（编译期约束）
//!
//! 能力被拆成细粒度 trait，只有拥有对应能力的层才能调用对应方法，越权调用会直接
//! 编译失败（对应 C# 版的 `ICanGetModel`、`ICanSendCommand` 等扩展方法）：
//!
//! | 能力接口 | 提供的方法 |
//! |---|---|
//! | [`ICanGetModel`] | [`get_model`](ICanGetModel::get_model) |
//! | [`ICanGetSystem`] | [`get_system`](ICanGetSystem::get_system) |
//! | [`ICanGetUtility`] | [`get_utility`](ICanGetUtility::get_utility) |
//! | [`ICanSendCommand`] | [`send_command`](ICanSendCommand::send_command) |
//! | [`ICanSendQuery`] | [`send_query`](ICanSendQuery::send_query) |
//! | [`ICanSendEvent`] | [`send_event`](ICanSendEvent::send_event) |
//! | [`ICanRegisterEvent`] | [`register_event`](ICanRegisterEvent::register_event) |
//!
//! ## 快速开始
//!
//! ```rust
//! use qframework_core::prelude::*;
//!
//! // 1. 数据层：Model 只能使用 Utility 与发送事件
//! #[derive(Default, IModel)]
//! struct CounterModel {
//!     arch: ArchRef,
//!     pub count: BindableProperty<i32>,
//! }
//!
//! // 2. 命令：唯一允许改变状态的地方
//! struct IncreaseCountCommand;
//! impl ICommand for IncreaseCountCommand {
//!     type Output = ();
//!     fn execute(&self, ctx: &CommandContext) {
//!         let model = ctx.get_model::<CounterModel>();
//!         let next = model.count.get() + 1;
//!         model.count.set(next);
//!     }
//! }
//!
//! // 3. 组装架构
//! let architecture = ArchitectureBuilder::new("CounterApp")
//!     .model(CounterModel::default())
//!     .build();
//!
//! architecture.send_command(IncreaseCountCommand);
//! assert_eq!(architecture.get_model::<CounterModel>().count.get(), 1);
//! ```
//!
//! ## 生命周期钩子
//!
//! 派生宏支持可选的 `init` / `deinit` 钩子：
//!
//! ```rust
//! # use qframework_core::prelude::*;
//! #[derive(Default, ISystem)]
//! #[system(init = |this: &MySystem| { this.send_event_default::<AppStarted>(); })]
//! struct MySystem {
//!     arch: ArchRef,
//! }
//!
//! #[derive(Debug, Default)]
//! struct AppStarted;
//! ```

pub mod architecture;
pub mod bindable;
pub mod command;
pub mod context;
pub mod event;
pub mod ioc;
pub mod layers;
pub mod query;
pub mod unregister;

pub use architecture::{Architecture, ArchitectureBuilder};
pub use bindable::{
    BindableDictionary, BindableList, BindableProperty, DictAdd, DictClear, DictCountChanged,
    DictRemove, DictReplace, ListAdd, ListClear, ListCountChanged, ListRemove,
};
pub use command::ICommand;
pub use context::{CommandContext, QueryContext};
pub use event::{EasyEvent, TypeEventSystem};
pub use ioc::IOCContainer;
pub use layers::{
    ArchRef, HasArchRef, ICanGetArchitecture, ICanGetModel, ICanGetSystem, ICanGetUtility,
    ICanRegisterEvent, ICanSendCommand, ICanSendEvent, ICanSendQuery, IController, IModel,
    ISystem, IUtility,
};
pub use query::IQuery;
pub use unregister::{IUnRegister, IUnRegisterList};

// 派生宏与同名 trait 共存于不同命名空间（与 serde 的 `Serialize` 同理）。
pub use qframework_macros::{IController, IModel, ISystem, IUtility};

/// 常用类型与派生宏的预导入集合。
pub mod prelude {
    pub use crate::{
        ArchRef, Architecture, ArchitectureBuilder, BindableDictionary, BindableList,
        BindableProperty, CommandContext, DictAdd, DictClear, DictCountChanged, DictRemove,
        DictReplace, EasyEvent, HasArchRef, ICanGetArchitecture, ICanGetModel, ICanGetSystem,
        ICanGetUtility, ICanRegisterEvent, ICanSendCommand, ICanSendEvent, ICanSendQuery,
        ICommand, IController, IOCContainer, IModel, IQuery, ISystem, IUnRegister, IUnRegisterList,
        IUtility, ListAdd, ListClear, ListCountChanged, ListRemove, QueryContext, TypeEventSystem,
    };
}
