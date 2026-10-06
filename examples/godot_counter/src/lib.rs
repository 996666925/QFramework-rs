use godot::classes::{INode, INode2D, Node, Node2D};
use godot::prelude::*;
use qframework_godot::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

#[derive(Default, IModel)]
struct CounterModel {
    arch: ArchRef,
    count: BindableProperty<i32>,
}

#[derive(Clone)]
struct CountChanged(i32);

struct IncreaseCount;

impl ICommand for IncreaseCount {
    type Output = ();

    fn execute(&self, ctx: &CommandContext) {
        let model = ctx.get_model::<CounterModel>();
        model.count.modify(|value| *value += 1);
        ctx.send_event(CountChanged(model.count.get()));
    }
}

struct GetCount;

impl IQuery for GetCount {
    type Result = i32;

    fn do_query(&self, ctx: &QueryContext) -> i32 {
        ctx.get_model::<CounterModel>().count.get()
    }
}

struct CounterApp;

impl QApplication for CounterApp {
    fn build() -> ArchitectureBuilder {
        ArchitectureBuilder::new("GodotCounter").model(CounterModel::default())
    }
}

/// 场景只管理节点；应用架构由 CounterApp 按需初始化。
#[derive(GodotClass)]
#[class(base = Node)]
struct CounterScene {
    smoke_test: bool,
    base: Base<Node>,
}

#[godot_api]
impl INode for CounterScene {
    fn init(base: Base<Node>) -> Self {
        Self {
            smoke_test: std::env::var_os("QFRAMEWORK_GODOT_SMOKE").is_some(),
            base,
        }
    }

    fn ready(&mut self) {
        if self.smoke_test {
            self.base_mut().call_deferred("run_smoke", &[]);
        }
    }
}

#[godot_api]
impl CounterScene {
    #[func]
    fn run_smoke(&mut self) {
        let mut controller = self.base().get_node_as::<CounterNode>("Counter");
        let display = controller.get_node_as::<Node2D>("Display");
        let label = self.base().get_node_as::<Node>("UI/Count");
        let count = CounterApp::interface()
            .get_model::<CounterModel>()
            .count
            .clone();
        let assert_display = |value: i32| {
            assert_eq!(display.get_position().x, value as f32 * 10.0);
            assert_eq!(
                label.get("text").to::<GString>().to_string(),
                format!("Count: {value}")
            );
        };

        assert_eq!(controller.bind().count(), 0);
        assert_eq!(controller.bind().inits.get(), 1);
        assert_eq!(count.handler_count(), 1);
        assert_display(0);

        // 命令返回前界面已更新；回调不会重新借用仍被 bind_mut 持有的 Controller。
        {
            let mut node = controller.bind_mut();
            node.increase();
            assert_display(1);
            assert_eq!(node.count(), 1);
        }
        CounterApp::interface().send_command(IncreaseCount);
        assert_display(2);

        self.base_mut().remove_child(&controller);
        assert!(
            !CounterApp::interface()
                .events()
                .has_listener::<CountChanged>()
        );
        assert_eq!(count.handler_count(), 0);
        assert_eq!(controller.bind().deinits.get(), 1);
        CounterApp::interface().send_command(IncreaseCount);
        assert_display(2);

        // 重新入树后 ready 重新订阅，初值回调立即恢复最新显示。
        controller.request_ready();
        self.base_mut().add_child(&controller);
        assert_eq!(controller.bind().inits.get(), 2);
        assert_eq!(count.handler_count(), 1);
        assert!(
            CounterApp::interface()
                .events()
                .has_listener::<CountChanged>()
        );
        assert_display(3);
        controller.bind_mut().increase();
        assert_display(4);

        let mut node2d = self.base().get_node_as::<SpatialCounter>("SpatialCounter");
        assert_eq!(node2d.bind().send_query(GetCount), 4);
        node2d.bind_mut().move_from_count();
        assert_eq!(node2d.get_position().x, 4.0);

        self.base_mut().remove_child(&controller);
        assert_eq!(controller.bind().deinits.get(), 2);
        controller.free();
        assert_eq!(count.handler_count(), 0);
        assert!(
            !CounterApp::interface()
                .events()
                .has_listener::<CountChanged>()
        );
        CounterApp::interface().send_command(IncreaseCount);
        assert_eq!(label.get("text").to::<GString>().to_string(), "Count: 4");
        self.check_subscription_lifetimes();
        godot_print!(
            "QFRAMEWORK_GODOT_SMOKE_OK: ready subscriptions, immediate UI, commands, queries, reentry and lifecycle cleanup"
        );
        self.base().get_tree().quit();
    }
}

impl CounterScene {
    fn check_subscription_lifetimes(&mut self) {
        let property = BindableProperty::new(0);
        let mut owner = Node2D::new_alloc();
        let destroyed_calls = Rc::new(Cell::new(0));
        let exited_calls = Rc::new(Cell::new(0));
        let capture = Rc::new(());
        let captured = capture.clone();
        let sink = destroyed_calls.clone();
        property
            .register(move |_| {
                let _ = &captured;
                sink.set(sink.get() + 1);
            })
            .unregister_when_node_destroyed(&mut owner);
        let sink = exited_calls.clone();
        property
            .register(move |_| sink.set(sink.get() + 1))
            .unregister_when_tree_exited(&mut owner);

        // 两种生命周期共用一个隐藏的内部节点，普通场景遍历看不到它。
        assert_eq!(owner.get_child_count(), 0);
        assert_eq!(
            owner.get_children_ex().include_internal(true).done().len(),
            1
        );
        self.base_mut().add_child(&owner);
        property.set(1);
        assert_eq!(destroyed_calls.get(), 1);
        assert_eq!(exited_calls.get(), 1);

        self.base_mut().remove_child(&owner);
        assert_eq!(property.handler_count(), 1);
        property.set(2);
        assert_eq!(destroyed_calls.get(), 2);
        assert_eq!(exited_calls.get(), 1);

        self.base_mut().add_child(&owner);
        property.set(3);
        assert_eq!(destroyed_calls.get(), 3);
        assert_eq!(exited_calls.get(), 1);

        // 保留返回的句柄仍可提前注销，之后销毁节点再次注销也安全。
        let manual = property
            .register(|_| panic!("manually cancelled subscription was invoked"))
            .unregister_when_node_destroyed(&mut owner);
        manual.unregister();
        property.set(4);
        owner.free();
        assert_eq!(property.handler_count(), 0);
        assert_eq!(Rc::strong_count(&capture), 1);
        manual.unregister();

        // 从未进入场景树的节点也必须在 free 时清理两种订阅。
        let mut detached = Node::new_alloc();
        property
            .register(|_| {})
            .unregister_when_node_destroyed(&mut detached);
        property
            .register(|_| {})
            .unregister_when_tree_exited(&mut detached);
        assert_eq!(property.handler_count(), 2);
        detached.free();
        assert_eq!(property.handler_count(), 0);
    }
}

/// 节点直接拥有 Controller 能力，不由 Architecture / Rc 托管。
#[derive(GodotClass, IController)]
#[class(base = Node)]
#[controller(architecture = CounterApp, init = Self::on_init, deinit = Self::on_deinit)]
struct CounterNode {
    arch: ArchRef,
    inits: Cell<u32>,
    deinits: Cell<u32>,
    base: Base<Node>,
}

#[godot_api]
impl INode for CounterNode {
    fn init(base: Base<Node>) -> Self {
        Self {
            arch: ArchRef::new(),
            inits: Cell::new(0),
            deinits: Cell::new(0),
            base,
        }
    }

    fn ready(&mut self) {
        IController::init(self);
        let mut display = self.base().get_node_as::<Node2D>("Display");
        let mut ui = self.base().get_node_as::<Node>("../UI");

        // 本地回调直接捕获展示节点，不重新借用正在发送命令的 Controller。
        self.get_model::<CounterModel>()
            .count
            .register_with_init_value(move |value| {
                if display.is_instance_valid() {
                    display.set_position(Vector2::new(*value as f32 * 10.0, 0.0));
                }
                if ui.is_instance_valid() {
                    ui.call("update_count", &[value.to_variant()]);
                }
            })
            .unregister_when_tree_exited(&mut self.base_mut());

        self.register_event::<CountChanged, _>(move |event| {
            godot_print!("[Godot Controller] count = {}", event.0);
        })
        .unregister_when_tree_exited(&mut self.base_mut());
    }

    fn exit_tree(&mut self) {
        IController::deinit(self);
    }
}

#[godot_api]
impl CounterNode {
    #[func]
    fn increase(&mut self) {
        self.send_command(IncreaseCount);
    }

    #[func]
    fn count(&self) -> i32 {
        self.send_query(GetCount)
    }
}

impl CounterNode {
    fn on_init(&self) {
        self.inits.set(self.inits.get() + 1);
    }

    fn on_deinit(&self) {
        self.deinits.set(self.deinits.get() + 1);
    }
}

/// 同一扩展接口也适用于 Node2D / Node3D 等 Node 子类。
#[derive(GodotClass, IController)]
#[class(base = Node2D)]
#[controller(architecture = CounterApp)]
struct SpatialCounter {
    arch: ArchRef,
    base: Base<Node2D>,
}

#[godot_api]
impl INode2D for SpatialCounter {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            arch: ArchRef::new(),
            base,
        }
    }

    fn ready(&mut self) {
        IController::init(self);
    }

    fn exit_tree(&mut self) {
        IController::deinit(self);
    }
}

impl SpatialCounter {
    fn move_from_count(&mut self) {
        let count = self.send_query(GetCount);
        self.base_mut()
            .set_position(Vector2::new(count as f32, 0.0));
    }
}

struct CounterExtension;

#[gdextension]
unsafe impl ExtensionLibrary for CounterExtension {
    fn on_stage_deinit(stage: InitStage) {
        if stage == InitStage::Scene {
            CounterApp::deinit_interface();
        }
    }
}
