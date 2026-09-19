//! 类型化依赖容器：对应 QFramework 的 `IOCContainer`。
//!
//! 使用 `TypeId -> Arc<dyn Any>` 的映射按类型注册与解析实例。因为存的是
//! [`Arc`]，解析时只会做一次原子计数自增，开销可以忽略。

use std::any::{Any, TypeId, type_name};
use std::collections::HashMap;
use std::sync::Arc;

/// 基于类型注册 / 解析的容器。
#[derive(Default)]
pub struct IOCContainer {
    instances: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl IOCContainer {
    /// 创建空容器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个实例（内部会包装成 [`Arc`]）。
    pub fn register<T: Any + Send + Sync>(&mut self, instance: T) {
        self.register_arc(Arc::new(instance));
    }

    /// 注册一个已经包好的 [`Arc`] 实例。
    pub fn register_arc<T: Any + Send + Sync>(&mut self, instance: Arc<T>) {
        self.instances.insert(TypeId::of::<T>(), instance);
    }

    /// 按类型解析实例。
    pub fn get<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        self.instances
            .get(&TypeId::of::<T>())
            .cloned()
            .and_then(|instance| instance.downcast::<T>().ok())
    }

    /// 按类型解析实例，若不存在则 panic 并给出可读的类型名。
    pub fn expect<T: Any + Send + Sync>(&self) -> Arc<T> {
        self.get::<T>().unwrap_or_else(|| {
            panic!(
                "类型 `{}` 尚未注册到 IOCContainer，请检查 ArchitectureBuilder::model/system/utility",
                type_name::<T>()
            )
        })
    }

    /// 是否已经注册过类型 `T`。
    pub fn contains<T: Any + Send + Sync>(&self) -> bool {
        self.instances.contains_key(&TypeId::of::<T>())
    }

    /// 已注册的实例数量。
    pub fn len(&self) -> usize {
        self.instances.len()
    }

    /// 容器是否为空。
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    /// 清空容器。
    pub fn clear(&mut self) {
        self.instances.clear();
    }
}

impl std::fmt::Debug for IOCContainer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IOCContainer")
            .field("len", &self.instances.len())
            .finish()
    }
}
