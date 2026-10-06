use godot::classes::node::InternalMode;
use godot::classes::{INode, Node};
use godot::prelude::*;
use qframework_core::{IUnRegister, IUnRegisterList};

/// 将订阅的注销时机绑定到 Godot 节点生命周期。
///
/// ```no_run
/// use godot::prelude::*;
/// use qframework_godot::prelude::*;
///
/// let mut node = Node2D::new_alloc();
/// let count = BindableProperty::new(0);
/// let subscription = count.register(|value| println!("{value}"))
///     .unregister_when_node_destroyed(&mut node);
/// // 返回的句柄仍可提前注销。
/// subscription.unregister();
/// node.free();
/// ```
pub trait GodotUnRegisterExt {
    /// 节点真正销毁时注销；移出场景树或重新入树不会注销。
    ///
    /// 使用内部子节点保存句柄，也适用于从未入树的节点。
    /// 返回原句柄，仍可提前调用 `unregister()`。
    fn unregister_when_node_destroyed(self, node: &mut Node) -> Self;

    /// 节点下一次退出场景树时注销；未入树就销毁时也会注销。
    ///
    /// 重新入树后需要重新订阅。返回原句柄，仍可提前调用 `unregister()`。
    fn unregister_when_tree_exited(self, node: &mut Node) -> Self;
}

impl GodotUnRegisterExt for IUnRegister {
    fn unregister_when_node_destroyed(self, node: &mut Node) -> Self {
        cleanup_for(node).bind_mut().destroyed.add(self.clone());
        self
    }

    fn unregister_when_tree_exited(self, node: &mut Node) -> Self {
        cleanup_for(node).bind_mut().exited.add(self.clone());
        self
    }
}

fn cleanup_for(node: &mut Node) -> Gd<QSubscriptionCleanup> {
    for child in node
        .get_children_ex()
        .include_internal(true)
        .done()
        .iter_shared()
    {
        if let Ok(cleanup) = child.try_cast::<QSubscriptionCleanup>() {
            return cleanup;
        }
    }

    let cleanup = QSubscriptionCleanup::new_alloc();
    node.add_child_ex(&cleanup)
        .internal(InternalMode::BACK)
        .done();
    cleanup
}

// Godot 销毁父节点时一并销毁此节点，两个列表的 Drop 负责最终注销。
#[derive(GodotClass)]
#[class(base = Node, internal)]
struct QSubscriptionCleanup {
    destroyed: IUnRegisterList,
    exited: IUnRegisterList,
    base: Base<Node>,
}

#[godot_api]
impl INode for QSubscriptionCleanup {
    fn init(base: Base<Node>) -> Self {
        Self {
            destroyed: IUnRegisterList::new(),
            exited: IUnRegisterList::new(),
            base,
        }
    }

    fn exit_tree(&mut self) {
        self.exited.unregister_all();
    }
}
