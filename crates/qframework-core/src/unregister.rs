//! 注销句柄：对应 QFramework 的 `IUnRegister` / `IUnRegisterList`。
//!
//! 无论是订阅事件还是订阅 [`BindableProperty`](crate::BindableProperty) 的变化，
//! 都会返回一个 [`IUnRegister`]。忘记注销是内存泄漏的常见来源，因此框架统一用
//! 句柄来管理订阅的生命周期。

use std::sync::Arc;

/// 注销句柄。
///
/// 调用 [`unregister`](IUnRegister::unregister) 即可取消对应的订阅。
/// 句柄内部使用 [`Arc`]，可以安全地跨线程持有与拷贝。
#[derive(Clone)]
pub struct IUnRegister {
    inner: Arc<dyn Fn() + Send + Sync>,
}

impl IUnRegister {
    /// 用一段自定义逻辑构造注销句柄。
    pub fn new(unregister: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            inner: Arc::new(unregister),
        }
    }

    /// 一个什么都不做的空句柄。
    pub fn empty() -> Self {
        Self::new(|| {})
    }

    /// 执行注销。重复调用是安全的（幂等）。
    pub fn unregister(&self) {
        (self.inner)();
    }
}

impl std::fmt::Debug for IUnRegister {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IUnRegister").finish_non_exhaustive()
    }
}

/// 注销句柄列表，用于集中管理同一个对象的多个订阅。
///
/// 在 `Drop` 时会自动注销全部句柄，因此把 [`IUnRegisterList`] 保存在控制器里
/// 就能自动完成清理。
#[derive(Default)]
pub struct IUnRegisterList {
    items: Vec<IUnRegister>,
}

impl IUnRegisterList {
    /// 创建空列表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 加入一个句柄，返回 `&mut Self` 便于链式调用。
    pub fn add(&mut self, unregister: IUnRegister) -> &mut Self {
        self.items.push(unregister);
        self
    }

    /// 注销全部句柄并清空列表。
    pub fn unregister_all(&mut self) {
        for item in self.items.drain(..) {
            item.unregister();
        }
    }

    /// 已持有的句柄数量。
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl Drop for IUnRegisterList {
    fn drop(&mut self) {
        self.unregister_all();
    }
}

impl std::fmt::Debug for IUnRegisterList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IUnRegisterList")
            .field("len", &self.items.len())
            .finish()
    }
}
