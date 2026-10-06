# QFramework Godot

让 Godot Node 及其子类直接实现 `qframework_core::IController`。
基于 [gdext](https://github.com/godot-rust/gdext) 的 `godot` 0.5.5，
不依赖 Bevy，也不改变节点在 Godot 中的所有权。

```toml
[dependencies]
godot = "0.5.5"
qframework-core = "0.1"
qframework-godot = "0.1"
```

```rust,ignore
use godot::prelude::*;
use qframework_godot::prelude::*;

#[derive(GodotClass, IController)]
#[class(base = Node)]
#[controller(architecture = GameApp)]
struct Hud {
    arch: ArchRef,
    base: Base<Node>,
}

#[godot_api]
impl INode for Hud {
    fn init(base: Base<Node>) -> Self {
        Self { arch: ArchRef::new(), base }
    }

    fn ready(&mut self) {
        IController::init(self);
        let mut label = self.base().get_node_as::<Label>("HpLabel");
        self.get_model::<PlayerModel>().hp.register_with_init_value(
            move |hp| {
                if label.is_instance_valid() {
                    label.set_text(&format!("HP: {hp}"));
                }
            },
        ).unregister_when_tree_exited(&mut self.base_mut());
    }

    fn exit_tree(&mut self) {
        IController::deinit(self);
    }
}
```

`GameApp`（实现核心 `QApplication`）、`PlayerModel` 是应用定义的入口和模型。
完整可运行项目见 [godot_counter](../../examples/godot_counter/README.md)。

- `bind_architecture` 原地绑定，不调用 `Architecture::attach_controller`，不创建 `Rc<Node>`。
- `self.get_model` / `get_system` / `send_command` / `send_query` / `register_event` 使用核心 Controller 能力。
- `subscribe_event::<E>()` 返回自动注销的 `ControllerInbox<E>`。
- `observe_property` 包含属性初值与后续通知；`drain` 和 `try_recv` 非阻塞。
- 普通属性与事件订阅的闭包可直接捕获 `Gd` / `Rc`，无需额外的本地订阅 API。
- `unregister_when_node_destroyed` 在节点真正销毁时注销，包括从未入树的节点。
- `unregister_when_tree_exited` 在下一次退出树时注销，适合在 `ready` 中建立的 UI 订阅。
- Godot 节点可在主线程回调中正常使用 `base()`、`base_mut()`、`get_node_as`、信号与场景树。
- `IController::init` / `deinit` 由节点生命周期显式调用，重新入树时重新订阅。
- `ControllerInbox` 被析构或 `unsubscribe` 时取消订阅，不反初始化共享架构。

核心架构和所有层都在单线程使用，普通回调支持 `FnMut` 和非线程安全捕获。
不在同步回调中重新借用正在发送命令的 Controller，也不要捕获控制器自身形成引用环。
`ControllerInbox` 是可选的单线程延后处理队列，不用于工作线程通信。

这两个注销方法由 prelude 导出的 `GodotUnRegisterExt` 提供，返回原 `IUnRegister`，
可继续手动调用 `unregister()`。内部使用一个隐藏的子节点管理句柄，不需要字段或对象 ID。
`remove_child` 只会触发退出树版本；重新入树后应重新建立退出树订阅。
`free` / `queue_free` 真正销毁节点时，两种订阅都会清理。

这个 crate 默认使用 gdext 的最小代码生成集合；项目需要 Label、Button 等完整 Rust API 时，
在自身依赖中使用 `godot = "0.5.5"` 即可通过 Cargo feature 合并启用完整生成。

可与 [godot-bevy](https://github.com/bytemeadow/godot-bevy) 共用架构：
从 `qframework_bevy::QArchitecture::rc()` 取得共享句柄，在 Godot 主线程注入节点。
节点只保存弱引用，场景根节点、Autoload 或 Bevy Resource 必须持有架构的强引用。
