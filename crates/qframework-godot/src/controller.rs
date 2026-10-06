use std::fmt;
use std::rc::Rc;

use godot::classes::Node;
use godot::obj::{Inherits, WithBaseField};
use qframework_core::{ArchRef, Architecture, BindableProperty, IController};

use crate::ControllerInbox;

/// 节点绑定架构失败的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindArchitectureError {
    /// 架构未初始化，或已经反初始化。
    NotInitialized,
    /// 节点已绑定另一个架构，或之前绑定的架构已被释放。
    AlreadyBound,
}

impl fmt::Display for BindArchitectureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInitialized => {
                f.write_str("the architecture must be initialized before binding")
            }
            Self::AlreadyBound => {
                f.write_str("the node is already bound to a different or expired architecture")
            }
        }
    }
}

impl std::error::Error for BindArchitectureError {}

/// 为派生了 `IController` 的 Godot Node 提供接入方法。
///
/// 不把节点放进 `Rc` 或 IOC，不接管 `INode` 等 Godot 接口。
/// 在 `ready` 中绑定后显式调用 `IController::init(self)`；在 `exit_tree` 中
/// 调用 `IController::deinit(self)` 并释放 inbox。普通订阅可用
/// [`unregister_when_tree_exited`](crate::GodotUnRegisterExt::unregister_when_tree_exited)
/// 自动注销。重新入树时需要重新订阅，
/// 可调用 Godot 的 `request_ready()` 让 `ready` 再次执行。
pub trait NodeControllerExt: IController + WithBaseField + Inherits<Node> {
    /// 原地注入架构引用，不创建节点副本，也不调用 `init`。
    ///
    /// 重复绑定同一个存活架构是幂等的；不同架构返回错误。
    /// `ArchRef` 保存弱引用，架构应由根节点、Autoload 或 Bevy Resource 持有。
    fn bind_architecture(
        &self,
        architecture: &Rc<Architecture>,
    ) -> Result<(), BindArchitectureError> {
        bind_reference(self.arch_ref(), architecture)
    }

    /// 订阅业务事件。回调仅推送 Rust 数据，不访问 Godot 场景。
    ///
    /// 在 `process` 等主线程回调中消费返回的 inbox，销毁 inbox 时自动注销。
    fn subscribe_event<E: Clone + 'static>(&self) -> ControllerInbox<E> {
        ControllerInbox::event(&self.architecture())
    }

    /// 观察可绑定属性，包含当前值与后续通知。
    fn observe_property<T: Clone + 'static>(
        &self,
        property: &BindableProperty<T>,
    ) -> ControllerInbox<T> {
        ControllerInbox::property(property)
    }
}

impl<T> NodeControllerExt for T where T: IController + WithBaseField + Inherits<Node> {}

fn bind_reference(
    reference: &ArchRef,
    architecture: &Rc<Architecture>,
) -> Result<(), BindArchitectureError> {
    if !architecture.is_inited() {
        return Err(BindArchitectureError::NotInitialized);
    }
    reference.set(architecture);
    match reference.try_get() {
        Some(current) if Rc::ptr_eq(&current, architecture) => Ok(()),
        _ => Err(BindArchitectureError::AlreadyBound),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qframework_core::ArchitectureBuilder;

    #[test]
    fn binding_is_idempotent_and_rejects_replacement() {
        let reference = ArchRef::new();
        let architecture = ArchitectureBuilder::new("One").build();
        let other = ArchitectureBuilder::new("Two").build();
        assert_eq!(bind_reference(&reference, &architecture), Ok(()));
        assert_eq!(bind_reference(&reference, &architecture), Ok(()));
        assert_eq!(
            bind_reference(&reference, &other),
            Err(BindArchitectureError::AlreadyBound)
        );
        assert!(Rc::ptr_eq(&reference.get(), &architecture));
    }

    #[test]
    fn rejects_deinited_and_expired_architectures() {
        let reference = ArchRef::new();
        let architecture = ArchitectureBuilder::new("One").build();
        architecture.deinit();
        assert_eq!(
            bind_reference(&reference, &architecture),
            Err(BindArchitectureError::NotInitialized)
        );

        let live = ArchitectureBuilder::new("Live").build();
        assert_eq!(bind_reference(&reference, &live), Ok(()));
        drop(live);
        let replacement = ArchitectureBuilder::new("Replacement").build();
        assert_eq!(
            bind_reference(&reference, &replacement),
            Err(BindArchitectureError::AlreadyBound)
        );
    }
}
