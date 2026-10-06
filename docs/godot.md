# Godot 集成

`qframework-godot` 让 gdext 的 Node、Node2D、Node3D 等节点直接获得核心 `IController` 的能力。
它不接管场景树和节点所有权，也不要求 Bevy。

## 安装

```toml
[dependencies]
godot = "0.5.5"
qframework-core = "0.1"
qframework-godot = "0.1"

[lib]
crate-type = ["cdylib"]
```

节点同时派生 `GodotClass` 与 `IController`，拥有 `arch: ArchRef` 和 `base: Base<Node>`。
Godot 控制器与核心计数器使用同一套能力和派生宏，无需实现第二个 Controller trait。

```rust
use godot::prelude::*;
use qframework_godot::prelude::*;

#[derive(GodotClass, IController)]
#[class(base = Node)]
#[controller(architecture = GameApp)]
struct PlayerController {
    arch: ArchRef,
    base: Base<Node>,
}
```

完整 `INode` 实现、场景和 GDExtension 入口见 [godot_counter](../examples/godot_counter/README.md)。

## 架构与生命周期

1. 定义实现 `QApplication` 的应用类型，并在节点上声明 `#[controller(architecture = GameApp)]`。
2. 在节点 `ready` 中建立订阅；首次使用能力时会自动获取应用架构。
3. 节点使用 `get_model` / `get_system` / `send_command` / `send_query` / `register_event`。
4. 订阅使用 `unregister_when_tree_exited` 自动注销；在 `exit_tree` 中释放可选的 inbox。
5. 应用结束、所有节点已退出后调用 `GameApp::deinit_interface()`。

```rust
struct GameApp;

impl QApplication for GameApp {
    fn build() -> ArchitectureBuilder {
        ArchitectureBuilder::new("GameApp").model(PlayerModel::default())
    }
}
```

节点无需访问父节点来寻找架构，移动到其他场景仍使用同一应用。
共享架构跨场景保留；不要在单个场景或 Controller 退出时清理它。
示例在 GDExtension 的 `on_stage_deinit(InitStage::Scene)` 中清理应用入口。
清理后再次访问会创建新架构，原来已经绑定的节点不会重新绑定。

多实例、测试或与 Bevy 共用架构时，仍可显式调用 `bind_architecture`，优先使用注入实例。
不声明 `architecture` 属性的 Controller 必须显式注入。
`bind_architecture` 只注入弱引用，不管理节点、不调用 `init`。
同一架构的重复绑定是幂等的；绑定不同架构或未初始化的架构会返回明确错误。
`ArchRef` 只绑定一次，切换架构应创建新的节点。
Godot 默认只调用一次 `ready`，重新入树前调用 `request_ready()` 可再次初始化和订阅，
也可由应用在 `enter_tree` 等回调中管理重新订阅。

Godot 节点使用 `Gd` 与场景树管理，**不要**使用 `Architecture::attach_controller` 创建 `Rc<Node>`。
所有层、绑定容器和回调在单线程使用，可以直接持有节点与 `Rc` / `RefCell`。
不要在节点内保存一个指向自身的强 `Gd<Self>` 引用。

## 场景访问

```rust,ignore
let model = self.get_model::<PlayerModel>();
self.send_command(TakeDamageCommand { amount: 10 });
let snapshot = self.send_query(GetPlayerSnapshotQuery);

let mut label = self.base().get_node_as::<Label>("HpLabel");
label.set_text(&format!("HP: {}", model.hp.get()));
```

`base()` / `base_mut()` 是 gdext 的借用 guard。调用可能同步触发自己的 Godot 回调时，
使用 `base_mut()` 或 `unbind()` 管理重入，遵守 gdext 的借用规则。

## 事件和属性通知
框架在单线程运行，属性和事件支持 `FnMut + 'static`。在 `ready` 中直接监听：
```rust,ignore
let mut display = self.base().get_node_as::<Node2D>("Display");
self.get_model::<CounterModel>().count.register_with_init_value(
    move |count| {
        if display.is_instance_valid() {
            display.set_position(Vector2::new(*count as f32 * 10.0, 0.0));
        }
    },
).unregister_when_tree_exited(&mut self.base_mut());
```
重新入树时重新订阅，初值回调立即恢复最新显示，无需 `process` 轮询。
事件使用 `self.register_event::<CountChanged, _>(move |event| { ... })`。

### 自动注销

导入 `qframework_godot::prelude::*` 后，订阅句柄支持两个链式方法：

| 方法 | 注销时机 |
|---|---|
| `unregister_when_node_destroyed(&mut node)` | 节点真正销毁；移出树或重新入树时保留订阅 |
| `unregister_when_tree_exited(&mut node)` | 节点下一次退出场景树；适合 `ready` 中的 UI 订阅 |

`unregister_when_node_destroyed` 对应原版 `UnRegisterWhenGameObjectDestroyed`。
两个方法都适用于 Node、Node2D、Node3D 等节点，返回原句柄，仍可提前 `unregister()`。
在 Controller 内可以传 `&mut self.base_mut()`，不需要保存 `IUnRegisterList` 字段。
内部的隐藏子节点负责清理，无需手动接信号或传对象 ID。
从未入树的节点在销毁时也会注销两种订阅；`queue_free()` 在实际销毁时才注销。

也可继续使用 `IUnRegisterList`，在 `exit_tree` 手动调用 `unregister_all()`。

闭包可以直接捕获 `Gd`，不需要 `instance_id` 或本地回调注册表。
避免捕获 Controller 自身形成引用环，或在发送命令的同步回调中重新借用它。
展示节点可能单独释放时，使用 `is_instance_valid()` 检查。

需要延后处理通知时仍可使用 `ControllerInbox`：`observe_property` 包含属性初值与后续通知，
`subscribe_event` 只接收新事件。`drain` / `try_recv` 非阻塞，释放 inbox 自动注销。
它是单线程通知队列，不用于跨线程通信。

## 与 godot-bevy 共用

[godot-bevy](https://github.com/bytemeadow/godot-bevy) 0.12 使用 Bevy 0.19 与 gdext 0.5。
本 crate 直接使用 [gdext](https://github.com/godot-rust/gdext) 0.5.5，同一项目可以同时依赖两个集成层。
在 Bevy 系统或应用组装时从 `QArchitecture::rc()` 取得架构句柄，
再于 Godot 主线程通过 `Gd<T>::bind_mut()` 注入 Controller。
架构和 Node / `Gd` 都留在 Godot 主线程。Bevy 系统通过 `NonSend<QArchitecture>` 使用架构；
工作线程的计算结果应由应用先送回主线程，再执行命令或修改 Model。

## 验证

```powershell
cargo test --workspace --all-targets --offline
./examples/godot_counter/run.ps1 -Smoke -GodotPath 'E:\Godot\Godot_v4.7-stable_win64.exe'
```

脚本构建 cdylib，导入 Godot 项目，然后 headless 检查命令、查询、场景节点、
属性初值、同步界面更新、退出清理和重新入树。成功标记为 `QFRAMEWORK_GODOT_SMOKE_OK`。
