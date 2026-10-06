//! 类型事件系统：对应 QFramework 的 `TypeEventSystem` / `EasyEvent`。
//!
//! 事件以「类型」作为频道，天然获得编译期检查，避免字符串频道的拼写错误。
//! 每个事件类型 `T` 唯一映射一个 [`EasyEvent<T>`]。

use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use crate::unregister::IUnRegister;

type Handler<T> = Rc<RefCell<dyn FnMut(&T)>>;

/// 单一类型的事件持有器，内部维护订阅者列表。
///
/// 一般不需要直接使用，通过 [`TypeEventSystem`] 即可。
pub struct EasyEvent<T> {
    handlers: RefCell<Vec<(u64, Handler<T>)>>,
    next_id: Cell<u64>,
}

impl<T> EasyEvent<T> {
    /// 创建一个没有任何订阅者的事件。
    pub fn new() -> Self {
        Self {
            handlers: RefCell::new(Vec::new()),
            next_id: Cell::new(0),
        }
    }

    /// 当前订阅者数量。
    pub fn handler_count(&self) -> usize {
        self.handlers.borrow().len()
    }

    /// 清空所有订阅。
    pub fn clear(&self) {
        let handlers = std::mem::take(&mut *self.handlers.borrow_mut());
        drop(handlers);
    }
}

impl<T> Default for EasyEvent<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: 'static> EasyEvent<T> {
    /// 注册一个订阅者，返回可用于注销的句柄。
    pub fn register(self: Rc<Self>, handler: impl FnMut(&T) + 'static) -> IUnRegister {
        let id = self.next_id.get();
        self.next_id
            .set(id.checked_add(1).expect("event subscription IDs exhausted"));
        self.handlers
            .borrow_mut()
            .push((id, Rc::new(RefCell::new(handler))));

        let weak = Rc::downgrade(&self);
        IUnRegister::new(move || {
            if let Some(event) = weak.upgrade() {
                event.unregister_by_id(id);
            }
        })
    }

    /// 根据内部 id 移除订阅者。
    pub fn unregister_by_id(&self, id: u64) {
        let removed = {
            let mut handlers = self.handlers.borrow_mut();
            handlers
                .iter()
                .position(|(handler_id, _)| *handler_id == id)
                .map(|index| handlers.remove(index))
        };
        drop(removed);
    }

    /// 广播事件。
    ///
    /// 派发前会对订阅者列表做一次快照，因此订阅者在回调中注册/注销自己是安全的。
    pub fn send(&self, event: &T) {
        let snapshot: Vec<Handler<T>> = {
            let handlers = self.handlers.borrow();
            handlers
                .iter()
                .map(|(_, handler)| handler.clone())
                .collect()
        };

        for handler in snapshot {
            handler
                .try_borrow_mut()
                .expect("an event callback cannot recursively invoke itself")(event);
        }
    }
}

/// 类型事件总线。
///
/// 架构内嵌一个实例（[`Architecture::events`](crate::Architecture)），
/// 另外还提供了一个全局实例 [`TypeEventSystem::global`]，用于跨架构通信。
#[derive(Default)]
pub struct TypeEventSystem {
    events: RefCell<HashMap<TypeId, Rc<dyn Any>>>,
}

impl TypeEventSystem {
    /// 创建空的事件系统。
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前线程共享的全局事件系统，适合框架级事件。
    pub fn global() -> Rc<TypeEventSystem> {
        thread_local! {
            static GLOBAL: Rc<TypeEventSystem> = Rc::new(TypeEventSystem::new());
        }
        GLOBAL.with(Rc::clone)
    }

    fn easy_event<T: 'static>(&self) -> Rc<EasyEvent<T>> {
        let mut events = self.events.borrow_mut();
        if let Some(existing) = events.get(&TypeId::of::<T>())
            && let Ok(event) = existing.clone().downcast::<EasyEvent<T>>()
        {
            return event;
        }

        let event = Rc::new(EasyEvent::<T>::new());
        events.insert(TypeId::of::<T>(), event.clone());
        event
    }

    /// 注册事件订阅，返回注销句柄。
    pub fn register<T: 'static>(&self, handler: impl FnMut(&T) + 'static) -> IUnRegister {
        self.easy_event::<T>().register(handler)
    }

    /// 是否已经有人订阅了事件 `T`。
    pub fn has_listener<T: 'static>(&self) -> bool {
        let existing = self.events.borrow().get(&TypeId::of::<T>()).cloned();
        existing
            .and_then(|event| event.downcast::<EasyEvent<T>>().ok())
            .is_some_and(|event| event.handler_count() > 0)
    }

    /// 移除事件 `T` 的所有订阅者。
    pub fn unregister_all<T: 'static>(&self) {
        let existing = self.events.borrow().get(&TypeId::of::<T>()).cloned();
        if let Some(event) = existing
            && let Ok(event) = event.downcast::<EasyEvent<T>>()
        {
            event.clear();
        }
    }

    /// 广播一个事件实例。
    pub fn send<T: 'static>(&self, event: T) {
        let existing = self.events.borrow().get(&TypeId::of::<T>()).cloned();
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
        let events = std::mem::take(&mut *self.events.borrow_mut());
        drop(events);
    }
}

impl std::fmt::Debug for TypeEventSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self.events.borrow().len();
        f.debug_struct("TypeEventSystem")
            .field("event_types", &count)
            .finish()
    }
}
