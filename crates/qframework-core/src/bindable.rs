//! 可观察数据容器：对应 QFramework 的 `BindableProperty` / `BindableList` /
//! `BindableDictionary`。
//!
//! 它们是 MVVM 风格的数据绑定基础：Model 持有数据，UI（Controller）订阅变化，
//! 数据一变就自动刷新，避免轮询或手动刷新。

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::rc::Rc;

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
///
/// 绑定属性只在单线程共享，不能移动到工作线程：
/// ```compile_fail
/// use qframework_core::BindableProperty;
/// let property = BindableProperty::new(0);
/// std::thread::spawn(move || property.set(1));
/// ```
pub struct BindableProperty<T> {
    inner: Rc<BindablePropertyInner<T>>,
}

struct BindablePropertyInner<T> {
    value: RefCell<T>,
    on_changed: Rc<EasyEvent<T>>,
}

impl<T> BindableProperty<T> {
    /// 用初始值创建。
    pub fn new(value: T) -> Self {
        Self {
            inner: Rc::new(BindablePropertyInner {
                value: RefCell::new(value),
                on_changed: Rc::new(EasyEvent::new()),
            }),
        }
    }
}

impl<T> Clone for BindableProperty<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<T: Default> Default for BindableProperty<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: Clone + 'static> BindableProperty<T> {
    /// 读取当前值（返回拷贝）。
    pub fn get(&self) -> T {
        self.inner.value.borrow().clone()
    }

    /// 以只读方式访问当前值，避免拷贝。
    ///
    /// 值比较大（例如 `Vec`）或只需要读取其中某个字段时，用这个比 [`get`](Self::get) 更省。
    ///
    /// 闭包持有只读借用；可读取同一属性，但不能修改它，否则会发生借用冲突 panic。
    pub fn with_value<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        let guard = self.inner.value.borrow();
        f(&guard)
    }

    /// 写入新值并通知所有订阅者。
    pub fn set(&self, value: T) {
        let current = {
            let mut guard = self.inner.value.borrow_mut();
            *guard = value;
            guard.clone()
        };
        self.inner.on_changed.send(&current);
    }

    /// 写入新值但**不**触发回调。适合初始化或批量同步场景。
    pub fn set_value_without_event(&self, value: T) {
        let mut guard = self.inner.value.borrow_mut();
        *guard = value;
    }

    /// 就地修改值并通知订阅者。
    ///
    /// 闭包持有可变借用，结束后释放借用并同步通知一次。
    /// 不要在闭包内读写同一个属性，否则会发生借用冲突 panic。
    pub fn modify(&self, f: impl FnOnce(&mut T)) {
        let current = {
            let mut guard = self.inner.value.borrow_mut();
            f(&mut guard);
            guard.clone()
        };
        self.inner.on_changed.send(&current);
    }

    /// 订阅值变化。
    pub fn register(&self, handler: impl FnMut(&T) + 'static) -> IUnRegister {
        Rc::clone(&self.inner.on_changed).register(handler)
    }

    /// 订阅值变化，并立即用当前值回调一次。
    ///
    /// 先注册，再读取初值；初值回调 panic 时会自动撤销订阅。
    pub fn register_with_init_value<F>(&self, handler: F) -> IUnRegister
    where
        F: FnMut(&T) + 'static,
    {
        let handler = Rc::new(RefCell::new(handler));
        let shared = Rc::clone(&handler);

        let unregister = Rc::clone(&self.inner.on_changed)
            .register(move |value: &T| (*shared.borrow_mut())(value));

        let current = self.get();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            (*handler.borrow_mut())(&current);
        }));
        if let Err(error) = result {
            unregister.unregister();
            std::panic::resume_unwind(error);
        }

        unregister
    }

    /// 当前订阅者数量。
    pub fn handler_count(&self) -> usize {
        self.inner.on_changed.handler_count()
    }
}

impl<T: fmt::Display + Clone + 'static> fmt::Display for BindableProperty<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.get(), f)
    }
}

impl<T: fmt::Debug + Clone + 'static> fmt::Debug for BindableProperty<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BindableProperty")
            .field(&self.get())
            .finish()
    }
}

impl<T: PartialEq + Clone + 'static> PartialEq<T> for BindableProperty<T> {
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
/// 事件在数据写入**之后**发出（此时已释放可变借用），
/// 回调可以读取容器；回调中继续修改数据会产生嵌套通知。
pub struct BindableList<T> {
    inner: Rc<BindableListInner<T>>,
}

struct BindableListInner<T> {
    items: RefCell<Vec<T>>,
    on_add: Rc<EasyEvent<ListAdd<T>>>,
    on_remove: Rc<EasyEvent<ListRemove<T>>>,
    on_clear: Rc<EasyEvent<ListClear>>,
    on_count_changed: Rc<EasyEvent<ListCountChanged>>,
}

impl<T> BindableList<T> {
    /// 创建空列表。
    pub fn new() -> Self {
        Self {
            inner: Rc::new(BindableListInner {
                items: RefCell::new(Vec::new()),
                on_add: Rc::new(EasyEvent::new()),
                on_remove: Rc::new(EasyEvent::new()),
                on_clear: Rc::new(EasyEvent::new()),
                on_count_changed: Rc::new(EasyEvent::new()),
            }),
        }
    }

    /// 当前元素数量。
    pub fn len(&self) -> usize {
        self.inner.items.borrow().len()
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
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<T: Clone + 'static> BindableList<T> {
    /// 用一组初始值创建列表。
    pub fn from_vec(items: Vec<T>) -> Self {
        let list = Self::new();
        *list.inner.items.borrow_mut() = items;
        list
    }

    /// 取得指定位置的元素拷贝。
    pub fn get(&self, index: usize) -> Option<T> {
        self.inner.items.borrow().get(index).cloned()
    }

    /// 取得整个列表的拷贝。
    ///
    /// 只想读取时优先用 [`with_items`](Self::with_items)，可以避免整表拷贝。
    pub fn snapshot(&self) -> Vec<T> {
        self.inner.items.borrow().clone()
    }

    /// 以只读方式访问元素切片，避免整表拷贝。
    pub fn with_items<R>(&self, f: impl FnOnce(&[T]) -> R) -> R {
        let items = self.inner.items.borrow();
        f(&items)
    }

    /// 追加元素，返回其下标。
    pub fn add(&self, value: T) -> usize {
        let index = {
            let mut items = self.inner.items.borrow_mut();
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
            let mut items = self.inner.items.borrow_mut();
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
            let mut items = self.inner.items.borrow_mut();
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
            let mut items = self.inner.items.borrow_mut();
            if items.is_empty() {
                return;
            }
            items.clear();
        }

        self.inner.on_clear.send(&ListClear);
        self.notify_count();
    }

    /// 订阅新增事件。
    pub fn on_add(&self, handler: impl FnMut(&ListAdd<T>) + 'static) -> IUnRegister {
        Rc::clone(&self.inner.on_add).register(handler)
    }

    /// 订阅移除事件。
    pub fn on_remove(&self, handler: impl FnMut(&ListRemove<T>) + 'static) -> IUnRegister {
        Rc::clone(&self.inner.on_remove).register(handler)
    }

    /// 订阅清空事件。
    pub fn on_clear(&self, handler: impl FnMut(&ListClear) + 'static) -> IUnRegister {
        Rc::clone(&self.inner.on_clear).register(handler)
    }

    /// 订阅数量变化事件。
    pub fn on_count_changed(
        &self,
        handler: impl FnMut(&ListCountChanged) + 'static,
    ) -> IUnRegister {
        Rc::clone(&self.inner.on_count_changed).register(handler)
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
/// 事件在数据写入**之后**发出（此时已释放可变借用），
/// 回调可以读取容器；回调中继续修改数据会产生嵌套通知。
pub struct BindableDictionary<K, V> {
    inner: Rc<BindableDictionaryInner<K, V>>,
}

struct BindableDictionaryInner<K, V> {
    entries: RefCell<HashMap<K, V>>,
    on_add: Rc<EasyEvent<DictAdd<K, V>>>,
    on_remove: Rc<EasyEvent<DictRemove<K, V>>>,
    on_replace: Rc<EasyEvent<DictReplace<K, V>>>,
    on_clear: Rc<EasyEvent<DictClear>>,
    on_count_changed: Rc<EasyEvent<DictCountChanged>>,
}

impl<K, V> BindableDictionary<K, V> {
    /// 创建空字典。
    pub fn new() -> Self {
        Self {
            inner: Rc::new(BindableDictionaryInner {
                entries: RefCell::new(HashMap::new()),
                on_add: Rc::new(EasyEvent::new()),
                on_remove: Rc::new(EasyEvent::new()),
                on_replace: Rc::new(EasyEvent::new()),
                on_clear: Rc::new(EasyEvent::new()),
                on_count_changed: Rc::new(EasyEvent::new()),
            }),
        }
    }

    /// 当前条目数量。
    pub fn len(&self) -> usize {
        self.inner.entries.borrow().len()
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
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<K, V> BindableDictionary<K, V>
where
    K: Clone + Eq + Hash + 'static,
    V: Clone + 'static,
{
    /// 判断是否包含键。
    pub fn contains_key(&self, key: &K) -> bool {
        self.inner.entries.borrow().contains_key(key)
    }

    /// 取得值的拷贝。
    pub fn get(&self, key: &K) -> Option<V> {
        self.inner.entries.borrow().get(key).cloned()
    }

    /// 取得整个字典的拷贝。
    ///
    /// 只想读取时优先用 [`with_entries`](Self::with_entries)，可以避免整表拷贝。
    pub fn snapshot(&self) -> HashMap<K, V> {
        self.inner.entries.borrow().clone()
    }

    /// 以只读方式访问字典，避免整表拷贝。
    pub fn with_entries<R>(&self, f: impl FnOnce(&HashMap<K, V>) -> R) -> R {
        let entries = self.inner.entries.borrow();
        f(&entries)
    }

    /// 插入条目；若键已存在则触发替换事件。
    pub fn insert(&self, key: K, value: V) {
        let previous = {
            let mut entries = self.inner.entries.borrow_mut();
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
        let value = { self.inner.entries.borrow_mut().remove(key) };

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
            let mut entries = self.inner.entries.borrow_mut();
            if entries.is_empty() {
                return;
            }
            entries.clear();
        }

        self.inner.on_clear.send(&DictClear);
        self.notify_count();
    }

    /// 订阅新增事件。
    pub fn on_add(&self, handler: impl FnMut(&DictAdd<K, V>) + 'static) -> IUnRegister {
        Rc::clone(&self.inner.on_add).register(handler)
    }

    /// 订阅移除事件。
    pub fn on_remove(&self, handler: impl FnMut(&DictRemove<K, V>) + 'static) -> IUnRegister {
        Rc::clone(&self.inner.on_remove).register(handler)
    }

    /// 订阅替换事件。
    pub fn on_replace(&self, handler: impl FnMut(&DictReplace<K, V>) + 'static) -> IUnRegister {
        Rc::clone(&self.inner.on_replace).register(handler)
    }

    /// 订阅清空事件。
    pub fn on_clear(&self, handler: impl FnMut(&DictClear) + 'static) -> IUnRegister {
        Rc::clone(&self.inner.on_clear).register(handler)
    }

    /// 订阅数量变化事件。
    pub fn on_count_changed(
        &self,
        handler: impl FnMut(&DictCountChanged) + 'static,
    ) -> IUnRegister {
        Rc::clone(&self.inner.on_count_changed).register(handler)
    }

    fn notify_count(&self) {
        let count = self.len();
        self.inner
            .on_count_changed
            .send(&DictCountChanged { count });
    }
}
