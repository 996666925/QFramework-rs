//! 四层架构与能力接口。
//!
//! 这一层是 QFramework 最核心的设计：把「谁能访问谁」变成编译期约束，
//! 而不是靠开发者自觉。
//!
//! ## 如何实现一个层对象
//!
//! 用 [`qframework_macros`] 提供的派生宏即可，宏会自动生成能力接口与层接口：
//!
//! ```ignore
//! #[derive(Default, IModel)]
//! struct CounterModel {
//!     arch: ArchRef,
//!     pub count: BindableProperty<i32>,
//! }
//! ```
//!
//! Controller / System / Model 需要一个 `arch: ArchRef` 字段（可用 `#[arch]` 标记
//! 其它名字的字段），Utility 不需要。

use std::sync::{Arc, OnceLock, Weak};

use crate::architecture::Architecture;
use crate::command::ICommand;
use crate::query::IQuery;
use crate::unregister::IUnRegister;

/// 架构引用的持有者。
///
/// 每个 Controller / System / Model 都拥有一个 `ArchRef` 字段（约定字段名为
/// `arch`），注册时由架构自动注入。内部保存 [`Weak`] 引用，避免与架构形成
/// 引用环导致内存泄漏。
#[derive(Default)]
pub struct ArchRef(OnceLock<Weak<Architecture>>);

impl ArchRef {
    /// 创建一个尚未绑定的引用。
    pub fn new() -> Self {
        Self::default()
    }

    /// 绑定到指定架构（由框架在注册时调用）。
    pub fn set(&self, architecture: &Arc<Architecture>) {
        let _ = self.0.set(Arc::downgrade(architecture));
    }

    /// 取得架构句柄，未绑定时 panic。
    pub fn get(&self) -> Arc<Architecture> {
        self.try_get()
            .expect("ArchRef 尚未绑定 Architecture：请通过 ArchitectureBuilder 注册该对象")
    }

    /// 尝试取得架构句柄。
    pub fn try_get(&self) -> Option<Arc<Architecture>> {
        self.0.get().and_then(Weak::upgrade)
    }

    /// 是否已经绑定且架构仍然存活。
    pub fn is_bound(&self) -> bool {
        self.try_get().is_some()
    }
}

impl std::fmt::Debug for ArchRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArchRef")
            .field("bound", &self.is_bound())
            .finish()
    }
}

/// 让架构能够拿到层对象内部的 [`ArchRef`]（用于注册时注入引用）。
///
/// 由派生宏自动实现，一般不需要手写。
pub trait HasArchRef {
    /// 返回内部持有的架构引用。
    fn arch_ref(&self) -> &ArchRef;
}

// ---------------------------------------------------------------------------
// 能力接口
// ---------------------------------------------------------------------------

/// 能取得所属架构。
pub trait ICanGetArchitecture {
    /// 返回所属架构。
    fn architecture(&self) -> Arc<Architecture>;
}

/// 能取得 Model。
pub trait ICanGetModel: ICanGetArchitecture {
    /// 取得已注册的 Model。
    fn get_model<M: IModel>(&self) -> Arc<M> {
        self.architecture().get_model::<M>()
    }

    /// 取得已注册的 Model，不存在时返回 `None`。
    fn try_get_model<M: IModel>(&self) -> Option<Arc<M>> {
        self.architecture().try_get_model::<M>()
    }
}

/// 能取得 System。
pub trait ICanGetSystem: ICanGetArchitecture {
    /// 取得已注册的 System。
    fn get_system<S: ISystem>(&self) -> Arc<S> {
        self.architecture().get_system::<S>()
    }
}

/// 能取得 Utility。
pub trait ICanGetUtility: ICanGetArchitecture {
    /// 取得已注册的 Utility。
    fn get_utility<U: IUtility>(&self) -> Arc<U> {
        self.architecture().get_utility::<U>()
    }
}

/// 能发送 Command。
pub trait ICanSendCommand: ICanGetArchitecture {
    /// 发送命令并等待其执行完成。
    fn send_command<C: ICommand>(&self, command: C) -> C::Output {
        self.architecture().send_command(command)
    }
}

/// 能发送 Query。
pub trait ICanSendQuery: ICanGetArchitecture {
    /// 发送查询并取得结果。
    fn send_query<Q: IQuery>(&self, query: Q) -> Q::Result {
        self.architecture().send_query(query)
    }
}

/// 能发送事件。
pub trait ICanSendEvent: ICanGetArchitecture {
    /// 广播一个事件实例。
    fn send_event<E: 'static>(&self, event: E) {
        self.architecture().send_event(event);
    }

    /// 用 `E::default()` 广播一个事件。
    fn send_event_default<E: Default + 'static>(&self) {
        self.architecture().send_event_default::<E>();
    }
}

/// 能注册（监听）事件。
pub trait ICanRegisterEvent: ICanGetArchitecture {
    /// 注册事件监听，返回注销句柄。
    fn register_event<E: 'static, F>(&self, handler: F) -> IUnRegister
    where
        F: Fn(&E) + Send + Sync + 'static,
    {
        self.architecture().register_event::<E, F>(handler)
    }
}

// ---------------------------------------------------------------------------
// 四层接口
// ---------------------------------------------------------------------------

/// 表现层：接收输入、响应状态变化。
///
/// 只能通过 Command 改变状态，不能发送事件（但可以注册事件）。
///
/// 用 `#[derive(IController)]` 自动实现。
pub trait IController:
    HasArchRef
    + ICanGetModel
    + ICanGetSystem
    + ICanSendCommand
    + ICanSendQuery
    + ICanRegisterEvent
    + Send
    + Sync
    + 'static
{
    /// 架构初始化完成后调用。
    fn init(&self) {}
    /// 架构销毁时调用。
    fn deinit(&self) {}
}

/// 业务逻辑层：承载可被多个表现层复用的逻辑。
///
/// 用 `#[derive(ISystem)]` 自动实现。
pub trait ISystem:
    HasArchRef
    + ICanGetModel
    + ICanGetSystem
    + ICanGetUtility
    + ICanSendEvent
    + ICanRegisterEvent
    + Send
    + Sync
    + 'static
{
    /// 架构初始化完成后调用（在 Model 之后）。
    fn init(&self) {}
    /// 架构销毁时调用（在 Model 之前）。
    fn deinit(&self) {}
}

/// 数据层：定义数据与数据的增删改查。
///
/// 只能使用 Utility 并发送事件，不能反向获取 System / Controller。
///
/// 用 `#[derive(IModel)]` 自动实现。
pub trait IModel: HasArchRef + ICanGetUtility + ICanSendEvent + Send + Sync + 'static {
    /// 架构初始化完成后调用（在 System 之前）。
    fn init(&self) {}
    /// 架构销毁时调用。
    fn deinit(&self) {}
}

/// 工具层：基础设施（存储、序列化、SDK 接入），不承载业务。
///
/// 用 `#[derive(IUtility)]` 自动实现。
pub trait IUtility: Send + Sync + 'static {
    /// 架构初始化完成后调用。
    fn init(&self) {}
    /// 架构销毁时调用。
    fn deinit(&self) {}
}
