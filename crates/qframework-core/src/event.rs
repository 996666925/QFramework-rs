//! 类型事件系统：对应 QFramework 的 `TypeEventSystem` / `EasyEvent`。
//!
//! 事件以「类型」作为频道，天然获得编译期检查，避免字符串频道的拼写错误。
//! 每个事件类型 `T` 唯一映射一个 [`EasyEvent<T>`]。

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use crate::unregister::IUnRegister;

type Handler<T> = Arc<dyn Fn(&T) + Send + Sync>;

/// 单一类型的事件持有器，内部维护订阅者列表。
///
/// 一般不需要直接使用，通过 [`TypeEventSystem`] 即可。
pub struct EasyEvent<T> {
    handlers: RwLock<Vec<(u64, Handler<T>)>>,
    next_id: AtomicU64,
}

impl<T> EasyEvent<T> {
    /// 创建一个没有任何订阅者的事件。
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(Vec::new()),
            next_id: AtomicU64::new(0),
        }
    }

    /// 当前订阅者数量。
    pub fn handler_count(&self) -> usize {
        self.handlers.read().unwrap().len()
    }

    /// 清空所有订阅。
    pub fn clear(&self) {
        self.handlers.write().unwrap().clear();
    }
}

impl<T> Default for EasyEvent<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: 'static> EasyEvent<T> {
    /// 注册一个订阅者，返回可用于注销的句柄。
    pub fn register(self: Arc<Self>, handler: impl Fn(&T) + Send + Sync + 'static) -> IUnRegister {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.handlers.write().unwrap().push((id, Arc::new(handler)));

        let weak = Arc::downgrade(&self);
        IUnRegister::new(move || {
            if let Some(event) = weak.upgrade() {
                event.unregister_by_id(id);
            }
        })
    }

    /// 根据内部 id 移除订阅者。
    pub fn unregister_by_id(&self, id: u64) {
        self.handlers
            .write()
            .unwrap()
            .retain(|(handler_id, _)| *handler_id != id);
    }

    /// 广播事件。
    ///
    /// 派发前会对订阅者列表做一次快照，因此订阅者在回调中注册/注销自己是安全的。
    pub fn send(&self, event: &T) {
        let snapshot: Vec<Handler<T>> = {
            let handlers = self.handlers.read().unwrap();
            handlers.iter().map(|(_, handler)| handler.clone()).collect()
        };

        for handler in snapshot {
            handler(event);
        }
    }
}

/// 类型事件总线。
///
/// 架构内嵌一个实例（[`Architecture::events`](crate::Architecture)），
/// 另外还提供了一个全局实例 [`TypeEventSystem::global`]，用于跨架构通信。
#[derive(Default)]
pub struct TypeEventSystem {
    events: Mutex<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl TypeEventSystem {
    /// 创建空的事件系统。
    pub fn new() -> Self {
        Self::default()
    }

    /// 进程级全局事件系统，适合框架级事件或不持有架构的代码使用。
    pub fn global() -> &'static TypeEventSystem {
        static GLOBAL: OnceLock<TypeEventSystem> = OnceLock::new();
        GLOBAL.get_or_init(TypeEventSystem::new)
    }

    fn easy_event<T: 'static>(&self) -> Arc<EasyEvent<T>> {
        let mut events = self.events.lock().unwrap();
        if let Some(existing) = events.get(&TypeId::of::<T>())
            && let Ok(event) = existing.clone().downcast::<EasyEvent<T>>()
        {
            return event;
        }

        let event = Arc::new(EasyEvent::<T>::new());
        events.insert(TypeId::of::<T>(), event.clone());
        event
    }

    /// 注册事件订阅，返回注销句柄。
    pub fn register<T: 'static>(
        &self,
        handler: impl Fn(&T) + Send + Sync + 'static,
    ) -> IUnRegister {
        self.easy_event::<T>().register(handler)
    }

    /// 是否已经有人订阅了事件 `T`。
    pub fn has_listener<T: 'static>(&self) -> bool {
        let existing = self.events.lock().unwrap().get(&TypeId::of::<T>()).cloned();
        existing
            .and_then(|event| event.downcast::<EasyEvent<T>>().ok())
            .is_some_and(|event| event.handler_count() > 0)
    }

    /// 移除事件 `T` 的所有订阅者。
    pub fn unregister_all<T: 'static>(&self) {
        let existing = self.events.lock().unwrap().get(&TypeId::of::<T>()).cloned();
        if let Some(event) = existing
            && let Ok(event) = event.downcast::<EasyEvent<T>>()
        {
            event.clear();
        }
    }

    /// 广播一个事件实例。
    pub fn send<T: 'static>(&self, event: T) {
        let existing = self.events.lock().unwrap().get(&TypeId::of::<T>()).cloned();
        if let Some(stored) = existing
            && let Ok(stored) = stored.downcast::<EasyEvent<T>>()
        {
            stored.send(&event);
        }
    }

    /// 用 `T::default()` 广播一个事件。
    pub fn send_default<T: Default + 'static>(&self) {
        self.send(T::default());
    }

    /// 清空整个事件系统。
    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }
}

impl std::fmt::Debug for TypeEventSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self.events.lock().unwrap().len();
        f.debug_struct("TypeEventSystem")
            .field("event_types", &count)
            .finish()
    }
}
