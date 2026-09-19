//! 可观察数据容器：对应 QFramework 的 `BindableProperty` / `BindableList` /
//! `BindableDictionary`。
//!
//! 它们是 MVVM 风格的数据绑定基础：Model 持有数据，UI（Controller）订阅变化，
//! 数据一变就自动刷新，避免轮询或手动刷新。

use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::sync::{Arc, RwLock};

use crate::event::EasyEvent;
use crate::unregister::IUnRegister;

// ---------------------------------------------------------------------------
// BindableProperty
// ---------------------------------------------------------------------------

/// 可观察的单个值。
///
/// ```ignore
/// let count = BindableProperty::new(0);
/// let _un = count.register(|value| println!("count = {value}"));
/// count.set(1);              // 触发回调
/// count.set_value_without_event(2); // 静默写入，不触发回调
/// assert_eq!(count.get(), 2);
/// ```
pub struct BindableProperty<T> {
    inner: Arc<BindablePropertyInner<T>>,
}

struct BindablePropertyInner<T> {
    value: RwLock<T>,
    on_changed: Arc<EasyEvent<T>>,
}

impl<T> BindableProperty<T> {
    /// 用初始值创建。
    pub fn new(value: T) -> Self {
        Self {
            inner: Arc::new(BindablePropertyInner {
                value: RwLock::new(value),
                on_changed: Arc::new(EasyEvent::new()),
            }),
        }
    }
}

impl<T> Clone for BindableProperty<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T: Default> Default for BindableProperty<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: Clone + Send + Sync + 'static> BindableProperty<T> {
    /// 读取当前值（返回拷贝）。
    pub fn get(&self) -> T {
        self.inner.value.read().unwrap().clone()
    }

    /// 以只读方式访问当前值，避免拷贝。
    ///
    /// 值比较大（例如 `Vec`）或只需要读取其中某个字段时，用这个比 [`get`](Self::get) 更省。
    ///
    /// 注意：不要在闭包里再操作同一个属性（`set` / `modify` / `get`），会死锁。
    pub fn with_value<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        let guard = self.inner.value.read().unwrap();
        f(&guard)
    }

    /// 写入新值并通知所有订阅者。
    pub fn set(&self, value: T) {
        let current = {
            let mut guard = self.inner.value.write().unwrap();
            *guard = value;
            guard.clone()
        };
        self.inner.on_changed.send(&current);
    }

    /// 写入新值但**不**触发回调。适合初始化或批量同步场景。
    pub fn set_value_without_event(&self, value: T) {
        let mut guard = self.inner.value.write().unwrap();
        *guard = value;
    }

    /// 就地修改值并通知订阅者。
    ///
    /// 闭包在整个「读-改-写」过程中持有写锁，因此 `modify` 是**原子**的：
    /// `count += 1` 这类复合操作应当使用它，而不是 `get()` 后再 `set()`——
    /// 后者在多线程（例如 Bevy 并行系统）下会丢更新。
    ///
    /// 注意：不要在闭包里再操作同一个属性，会死锁。
    pub fn modify(&self, f: impl FnOnce(&mut T)) {
        let current = {
            let mut guard = self.inner.value.write().unwrap();
            f(&mut guard);
            guard.clone()
        };
        self.inner.on_changed.send(&current);
    }

    /// 订阅值变化。
    pub fn register(&self, handler: impl Fn(&T) + Send + Sync + 'static) -> IUnRegister {
        Arc::clone(&self.inner.on_changed).register(handler)
    }

    /// 订阅值变化，并立即用当前值回调一次。
    ///
    /// 实现上**先注册、再读取当前值回调**，因此不会漏掉注册窗口内发生的变更；
    /// 并发写入时可能出现一次重复回调，但不会丢更新。
    pub fn register_with_init_value<F>(&self, handler: F) -> IUnRegister
    where
        F: Fn(&T) + Send + Sync + 'static,
    {
        let handler = Arc::new(handler);
        let shared = Arc::clone(&handler);

        let unregister =
            Arc::clone(&self.inner.on_changed).register(move |value: &T| (*shared)(value));

        let current = self.get();
        (*handler)(&current);

        unregister
    }

    /// 当前订阅者数量。
    pub fn handler_count(&self) -> usize {
        self.inner.on_changed.handler_count()
    }
}

impl<T: fmt::Display + Clone + Send + Sync + 'static> fmt::Display for BindableProperty<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.get(), f)
    }
}

impl<T: fmt::Debug + Clone + Send + Sync + 'static> fmt::Debug for BindableProperty<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BindableProperty").field(&self.get()).finish()
    }
}

impl<T: PartialEq + Clone + Send + Sync + 'static> PartialEq<T> for BindableProperty<T> {
    fn eq(&self, other: &T) -> bool {
        &self.get() == other
    }
}

// ---------------------------------------------------------------------------
// BindableList
// ---------------------------------------------------------------------------

/// 列表新增事件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListAdd<T> {
    /// 插入位置。
    pub index: usize,
    /// 新增的值（拷贝）。
    pub value: T,
}

/// 列表移除事件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListRemove<T> {
    /// 被移除的位置。
    pub index: usize,
    /// 被移除的值（拷贝）。
    pub value: T,
}

/// 列表清空事件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListClear;

/// 列表数量变化事件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListCountChanged {
    /// 变化后的数量。
    pub count: usize,
}

/// 可观察列表。
///
/// 事件在数据写入**之后**发出（而不是持有锁期间发出），因此在多线程并发写入时，
/// 事件的到达顺序不保证与写入顺序一致；但每个事件携带的值一定是当时真实写入的值。
pub struct BindableList<T> {
    inner: Arc<BindableListInner<T>>,
}

struct BindableListInner<T> {
    items: RwLock<Vec<T>>,
    on_add: Arc<EasyEvent<ListAdd<T>>>,
    on_remove: Arc<EasyEvent<ListRemove<T>>>,
    on_clear: Arc<EasyEvent<ListClear>>,
    on_count_changed: Arc<EasyEvent<ListCountChanged>>,
}

impl<T> BindableList<T> {
    /// 创建空列表。
    pub fn new() -> Self {
        Self {
            inner: Arc::new(BindableListInner {
                items: RwLock::new(Vec::new()),
                on_add: Arc::new(EasyEvent::new()),
                on_remove: Arc::new(EasyEvent::new()),
                on_clear: Arc::new(EasyEvent::new()),
                on_count_changed: Arc::new(EasyEvent::new()),
            }),
        }
    }

    /// 当前元素数量。
    pub fn len(&self) -> usize {
        self.inner.items.read().unwrap().len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T> Default for BindableList<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for BindableList<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T: Clone + Send + Sync + 'static> BindableList<T> {
    /// 用一组初始值创建列表。
    pub fn from_vec(items: Vec<T>) -> Self {
        let list = Self::new();
        *list.inner.items.write().unwrap() = items;
        list
    }

    /// 取得指定位置的元素拷贝。
    pub fn get(&self, index: usize) -> Option<T> {
        self.inner.items.read().unwrap().get(index).cloned()
    }

    /// 取得整个列表的拷贝。
    ///
    /// 只想读取时优先用 [`with_items`](Self::with_items)，可以避免整表拷贝。
    pub fn snapshot(&self) -> Vec<T> {
        self.inner.items.read().unwrap().clone()
    }

    /// 以只读方式访问元素切片，避免整表拷贝。
    pub fn with_items<R>(&self, f: impl FnOnce(&[T]) -> R) -> R {
        let items = self.inner.items.read().unwrap();
        f(&items)
    }

    /// 追加元素，返回其下标。
    pub fn add(&self, value: T) -> usize {
        let index = {
            let mut items = self.inner.items.write().unwrap();
            items.push(value.clone());
            items.len() - 1
        };

        self.inner.on_add.send(&ListAdd { index, value });
        self.notify_count();
        index
    }

    /// 在指定位置插入元素。
    pub fn insert(&self, index: usize, value: T) {
        let actual = {
            let mut items = self.inner.items.write().unwrap();
            let index = index.min(items.len());
            items.insert(index, value.clone());
            index
        };

        self.inner.on_add.send(&ListAdd {
            index: actual,
            value,
        });
        self.notify_count();
    }

    /// 移除指定位置的元素。
    pub fn remove_at(&self, index: usize) -> Option<T> {
        let value = {
            let mut items = self.inner.items.write().unwrap();
            if index < items.len() {
                Some(items.remove(index))
            } else {
                None
            }
        };

        if let Some(value) = value.clone() {
            self.inner.on_remove.send(&ListRemove { index, value });
            self.notify_count();
        }

        value
    }

    /// 清空列表。
    pub fn clear(&self) {
        {
            let mut items = self.inner.items.write().unwrap();
            if items.is_empty() {
                return;
            }
            items.clear();
        }

        self.inner.on_clear.send(&ListClear);
        self.notify_count();
    }

    /// 订阅新增事件。
    pub fn on_add(&self, handler: impl Fn(&ListAdd<T>) + Send + Sync + 'static) -> IUnRegister {
        Arc::clone(&self.inner.on_add).register(handler)
    }

    /// 订阅移除事件。
    pub fn on_remove(
        &self,
        handler: impl Fn(&ListRemove<T>) + Send + Sync + 'static,
    ) -> IUnRegister {
        Arc::clone(&self.inner.on_remove).register(handler)
    }

    /// 订阅清空事件。
    pub fn on_clear(&self, handler: impl Fn(&ListClear) + Send + Sync + 'static) -> IUnRegister {
        Arc::clone(&self.inner.on_clear).register(handler)
    }

    /// 订阅数量变化事件。
    pub fn on_count_changed(
        &self,
        handler: impl Fn(&ListCountChanged) + Send + Sync + 'static,
    ) -> IUnRegister {
        Arc::clone(&self.inner.on_count_changed).register(handler)
    }

    fn notify_count(&self) {
        let count = self.len();
        self.inner
            .on_count_changed
            .send(&ListCountChanged { count });
    }
}

// ---------------------------------------------------------------------------
// BindableDictionary
// ---------------------------------------------------------------------------

/// 字典新增事件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DictAdd<K, V> {
    /// 键。
    pub key: K,
    /// 值。
    pub value: V,
}

/// 字典移除事件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DictRemove<K, V> {
    /// 键。
    pub key: K,
    /// 值。
    pub value: V,
}

/// 字典值替换事件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DictReplace<K, V> {
    /// 键。
    pub key: K,
    /// 旧值。
    pub previous: V,
    /// 新值。
    pub current: V,
}

/// 字典清空事件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DictClear;

/// 字典数量变化事件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DictCountChanged {
    /// 变化后的数量。
    pub count: usize,
}

/// 可观察字典。
///
/// 事件在数据写入**之后**发出（而不是持有锁期间发出），因此在多线程并发写入时，
/// 事件的到达顺序不保证与写入顺序一致；但每个事件携带的值一定是当时真实写入的值。
pub struct BindableDictionary<K, V> {
    inner: Arc<BindableDictionaryInner<K, V>>,
}

struct BindableDictionaryInner<K, V> {
    entries: RwLock<HashMap<K, V>>,
    on_add: Arc<EasyEvent<DictAdd<K, V>>>,
    on_remove: Arc<EasyEvent<DictRemove<K, V>>>,
    on_replace: Arc<EasyEvent<DictReplace<K, V>>>,
    on_clear: Arc<EasyEvent<DictClear>>,
    on_count_changed: Arc<EasyEvent<DictCountChanged>>,
}

impl<K, V> BindableDictionary<K, V> {
    /// 创建空字典。
    pub fn new() -> Self {
        Self {
            inner: Arc::new(BindableDictionaryInner {
                entries: RwLock::new(HashMap::new()),
                on_add: Arc::new(EasyEvent::new()),
                on_remove: Arc::new(EasyEvent::new()),
                on_replace: Arc::new(EasyEvent::new()),
                on_clear: Arc::new(EasyEvent::new()),
                on_count_changed: Arc::new(EasyEvent::new()),
            }),
        }
    }

    /// 当前条目数量。
    pub fn len(&self) -> usize {
        self.inner.entries.read().unwrap().len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<K, V> Default for BindableDictionary<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> Clone for BindableDictionary<K, V> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<K, V> BindableDictionary<K, V>
where
    K: Clone + Eq + Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// 判断是否包含键。
    pub fn contains_key(&self, key: &K) -> bool {
        self.inner.entries.read().unwrap().contains_key(key)
    }

    /// 取得值的拷贝。
    pub fn get(&self, key: &K) -> Option<V> {
        self.inner.entries.read().unwrap().get(key).cloned()
    }

    /// 取得整个字典的拷贝。
    ///
    /// 只想读取时优先用 [`with_entries`](Self::with_entries)，可以避免整表拷贝。
    pub fn snapshot(&self) -> HashMap<K, V> {
        self.inner.entries.read().unwrap().clone()
    }

    /// 以只读方式访问字典，避免整表拷贝。
    pub fn with_entries<R>(&self, f: impl FnOnce(&HashMap<K, V>) -> R) -> R {
        let entries = self.inner.entries.read().unwrap();
        f(&entries)
    }

    /// 插入条目；若键已存在则触发替换事件。
    pub fn insert(&self, key: K, value: V) {
        let previous = {
            let mut entries = self.inner.entries.write().unwrap();
            entries.insert(key.clone(), value.clone())
        };

        match previous {
            Some(previous) => {
                self.inner.on_replace.send(&DictReplace {
                    key,
                    previous,
                    current: value,
                });
            }
            None => {
                self.inner.on_add.send(&DictAdd { key, value });
                self.notify_count();
            }
        }
    }

    /// 移除条目。
    pub fn remove(&self, key: &K) -> Option<V> {
        let value = { self.inner.entries.write().unwrap().remove(key) };

        if let Some(value) = value.clone() {
            self.inner.on_remove.send(&DictRemove {
                key: key.clone(),
                value,
            });
            self.notify_count();
        }

        value
    }

    /// 清空字典。
    pub fn clear(&self) {
        {
            let mut entries = self.inner.entries.write().unwrap();
            if entries.is_empty() {
                return;
            }
            entries.clear();
        }

        self.inner.on_clear.send(&DictClear);
        self.notify_count();
    }

    /// 订阅新增事件。
    pub fn on_add(&self, handler: impl Fn(&DictAdd<K, V>) + Send + Sync + 'static) -> IUnRegister {
        Arc::clone(&self.inner.on_add).register(handler)
    }

    /// 订阅移除事件。
    pub fn on_remove(
        &self,
        handler: impl Fn(&DictRemove<K, V>) + Send + Sync + 'static,
    ) -> IUnRegister {
        Arc::clone(&self.inner.on_remove).register(handler)
    }

    /// 订阅替换事件。
    pub fn on_replace(
        &self,
        handler: impl Fn(&DictReplace<K, V>) + Send + Sync + 'static,
    ) -> IUnRegister {
        Arc::clone(&self.inner.on_replace).register(handler)
    }

    /// 订阅清空事件。
    pub fn on_clear(&self, handler: impl Fn(&DictClear) + Send + Sync + 'static) -> IUnRegister {
        Arc::clone(&self.inner.on_clear).register(handler)
    }

    /// 订阅数量变化事件。
    pub fn on_count_changed(
        &self,
        handler: impl Fn(&DictCountChanged) + Send + Sync + 'static,
    ) -> IUnRegister {
        Arc::clone(&self.inner.on_count_changed).register(handler)
    }

    fn notify_count(&self) {
        let count = self.len();
        self.inner
            .on_count_changed
            .send(&DictCountChanged { count });
    }
}
