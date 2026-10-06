use qframework_core::{Architecture, BindableProperty, IUnRegister};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

/// 可选的通知队列，用于在稍后的 Godot 回调中处理通知。
/// 同步界面绑定直接使用 register_with_init_value 即可。
pub struct ControllerInbox<T> {
    queue: Rc<RefCell<VecDeque<T>>>,
    subscription: IUnRegister,
}
impl<T: Clone + 'static> ControllerInbox<T> {
    /// 订阅之后的业务事件。
    pub fn event(architecture: &Architecture) -> Self {
        let queue = Rc::new(RefCell::new(VecDeque::new()));
        let sink = queue.clone();
        let subscription = architecture
            .register_event(move |event: &T| sink.borrow_mut().push_back(event.clone()));
        Self {
            queue,
            subscription,
        }
    }
    /// 订阅属性初值与后续通知。
    pub fn property(property: &BindableProperty<T>) -> Self {
        let queue = Rc::new(RefCell::new(VecDeque::new()));
        let sink = queue.clone();
        let subscription = property
            .register_with_init_value(move |value| sink.borrow_mut().push_back(value.clone()));
        Self {
            queue,
            subscription,
        }
    }
}
impl<T> ControllerInbox<T> {
    pub fn try_recv(&self) -> Option<T> {
        self.queue.borrow_mut().pop_front()
    }
    pub fn drain(&self) -> Vec<T> {
        self.queue.borrow_mut().drain(..).collect()
    }
    /// 停止后续订阅，已入队的数据仍可读取。
    pub fn unsubscribe(&self) {
        self.subscription.unregister();
    }
}
impl<T> Drop for ControllerInbox<T> {
    fn drop(&mut self) {
        self.subscription.unregister();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn events_are_queued_and_unregistered_on_drop() {
        let architecture = qframework_core::ArchitectureBuilder::new().build();
        let inbox = ControllerInbox::<Rc<i32>>::event(&architecture);
        architecture.send_event(Rc::new(1));
        architecture.send_event(Rc::new(2));
        assert_eq!(inbox.drain(), vec![Rc::new(1), Rc::new(2)]);
        assert_eq!(inbox.try_recv(), None);
        drop(inbox);
        assert!(!architecture.events().has_listener::<Rc<i32>>());
    }
    #[test]
    fn property_includes_initial_value_and_stops_after_unsubscribe() {
        let property = BindableProperty::new(10);
        let inbox = ControllerInbox::property(&property);
        property.set(20);
        assert_eq!(inbox.drain(), vec![10, 20]);
        inbox.unsubscribe();
        property.set(30);
        assert_eq!(inbox.try_recv(), None);
    }
}
