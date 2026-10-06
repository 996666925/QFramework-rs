//! 按应用类型提供单线程共享架构，对应原版 Architecture<T>.Interface。
use crate::{Architecture, ArchitectureBuilder};
use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// 应用注册项与懒加载的共享入口。
pub trait QApplication: 'static {
    /// 描述注册项；Self::build().build() 创建独立实例。
    fn build() -> ArchitectureBuilder;
    /// 当前线程首次访问时创建架构，之后返回同一实例。
    /// 初始化钩子可访问此入口，build 本身不能递归访问自己。
    fn interface() -> Rc<Architecture>
    where
        Self: Sized,
    {
        slot::<Self>().interface::<Self>()
    }
    /// 所有使用者退出后清理入口；下次访问重建，旧对象不会重新绑定。
    /// 生命周期钩子不能调用此方法；直接调用 Architecture::deinit 不释放入口。
    fn deinit_interface()
    where
        Self: Sized,
    {
        slot::<Self>().deinit();
    }
}
#[derive(Default)]
enum State {
    #[default]
    Empty,
    Initializing(Option<Rc<Architecture>>),
    Ready(Rc<Architecture>),
    Deinitializing(Rc<Architecture>),
}
#[derive(Default)]
struct ApplicationSlot {
    state: RefCell<State>,
}
fn slot<A: QApplication>() -> Rc<ApplicationSlot> {
    thread_local! {
        static APPLICATIONS: RefCell<HashMap<TypeId, Rc<ApplicationSlot>>> = RefCell::default();
    }
    APPLICATIONS.with(|applications| {
        applications
            .borrow_mut()
            .entry(TypeId::of::<A>())
            .or_default()
            .clone()
    })
}
impl ApplicationSlot {
    fn interface<A: QApplication>(&self) -> Rc<Architecture> {
        {
            let mut state = self.state.borrow_mut();
            match &*state {
                State::Ready(architecture)
                | State::Deinitializing(architecture)
                | State::Initializing(Some(architecture)) => return architecture.clone(),
                State::Initializing(None) => {
                    drop(state);
                    panic!("QApplication::build cannot recursively call its own interface");
                }
                State::Empty => *state = State::Initializing(None),
            }
        }
        let mut reset = ResetSlot {
            slot: self,
            armed: true,
        };
        let builder = A::build();
        let architecture = builder.create();
        *self.state.borrow_mut() = State::Initializing(Some(architecture.clone()));
        builder.initialize(&architecture);
        *self.state.borrow_mut() = State::Ready(architecture.clone());
        reset.armed = false;
        architecture
    }
    fn deinit(&self) {
        let architecture = {
            let mut state = self.state.borrow_mut();
            match &*state {
                State::Empty => return,
                State::Ready(architecture) => {
                    let architecture = architecture.clone();
                    *state = State::Deinitializing(architecture.clone());
                    architecture
                }
                _ => {
                    drop(state);
                    panic!(
                        "QApplication::deinit_interface cannot run inside its own lifecycle hooks"
                    );
                }
            }
        };
        let _reset = ResetSlot {
            slot: self,
            armed: true,
        };
        architecture.deinit();
    }
}
struct ResetSlot<'a> {
    slot: &'a ApplicationSlot,
    armed: bool,
}
impl Drop for ResetSlot<'_> {
    fn drop(&mut self) {
        if self.armed {
            let previous = std::mem::take(&mut *self.slot.state.borrow_mut());
            drop(previous);
        }
    }
}
