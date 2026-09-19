use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bevy::prelude::*;
use qframework_bevy::prelude::*;

#[derive(Default, IModel)]
struct CounterModel {
    arch: ArchRef,
    pub count: BindableProperty<i32>,
}

#[derive(Message, Clone, Debug, PartialEq, Eq)]
struct CountChangedMessage {
    count: i32,
}

struct IncreaseCountCommand;

impl ICommand for IncreaseCountCommand {
    type Output = ();

    fn execute(&self, ctx: &CommandContext) {
        let model = ctx.get_model::<CounterModel>();
        model.count.modify(|count| *count += 1);
        ctx.send_event(CountChangedMessage {
            count: model.count.get(),
        });
    }
}

struct GetCountQuery;

impl IQuery for GetCountQuery {
    type Result = i32;

    fn do_query(&self, ctx: &QueryContext) -> i32 {
        ctx.get_model::<CounterModel>().count.get()
    }
}

struct TestApp;

impl QApplication for TestApp {
    fn build() -> ArchitectureBuilder {
        ArchitectureBuilder::new("TestApp").model(CounterModel::default())
    }
}

#[derive(Resource, Default)]
struct Observed(Vec<i32>);

fn collect_messages(mut reader: MessageReader<CountChangedMessage>, mut observed: ResMut<Observed>) {
    for message in reader.read() {
        observed.0.push(message.count);
    }
}

fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_qframework::<TestApp>()
        .bridge_q_messages::<CountChangedMessage>()
        .init_resource::<Observed>()
        .add_systems(Update, collect_messages);
    app
}

#[test]
fn architecture_is_available_as_resource() {
    let mut app = build_app();
    app.update();

    let architecture = app.world().resource::<QArchitecture>();
    assert_eq!(architecture.name(), "TestApp");
    assert!(architecture.is_inited());
    assert!(architecture.try_get_model::<CounterModel>().is_some());
}

#[test]
fn bevy_systems_can_send_commands() {
    let mut app = build_app();

    {
        let architecture = app.world().resource::<QArchitecture>().arc();
        architecture.send_command(IncreaseCountCommand);
        architecture.send_command(IncreaseCountCommand);
    }

    app.update();

    let architecture = app.world().resource::<QArchitecture>();
    assert_eq!(architecture.send_query(GetCountQuery), 2);
}

#[test]
fn qframework_events_become_bevy_messages() {
    let mut app = build_app();

    {
        let architecture = app.world().resource::<QArchitecture>().arc();
        architecture.send_command(IncreaseCountCommand);
        architecture.send_command(IncreaseCountCommand);
    }

    // PreUpdate 把事件转发进 Bevy 消息队列，Update 里被系统读取
    app.update();
    app.update();

    let observed = app.world().resource::<Observed>();
    assert_eq!(observed.0, vec![1, 2]);
}

#[derive(Default, IController)]
struct TickController {
    arch: ArchRef,
    ticks: AtomicI32,
}

impl QControllerUpdate for TickController {
    fn update(&self, _delta: Duration) {
        if self.ticks.fetch_add(1, Ordering::SeqCst) < 4 {
            self.send_command(IncreaseCountCommand);
        }
    }
}

#[test]
fn controllers_are_driven_every_frame() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_qframework::<TestApp>()
        .add_q_controller(TickController::default());

    assert_eq!(app.world().resource::<QControllers>().len(), 1);

    for _ in 0..5 {
        app.update();
    }

    let architecture = app.world().resource::<QArchitecture>();
    assert_eq!(architecture.send_query(GetCountQuery), 4);
}

#[derive(IController)]
#[controller(deinit = |this: &DeinitTrackingController| {
    this.deinits.fetch_add(1, Ordering::SeqCst);
})]
struct DeinitTrackingController {
    arch: ArchRef,
    deinits: Arc<AtomicI32>,
}

impl QControllerUpdate for DeinitTrackingController {
    fn update(&self, _delta: Duration) {}
}

struct OtherApp;

impl QApplication for OtherApp {
    fn build() -> ArchitectureBuilder {
        ArchitectureBuilder::new("OtherApp")
    }
}

#[test]
#[should_panic(expected = "只能安装一个 QFrameworkPlugin")]
fn only_one_qframework_plugin_is_allowed() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_qframework::<TestApp>()
        .add_qframework::<OtherApp>();
}

#[test]
fn controllers_are_deinited_on_cleanup() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_qframework::<TestApp>();

    let deinits = Arc::new(AtomicI32::new(0));
    app.add_q_controller(DeinitTrackingController {
        arch: ArchRef::new(),
        deinits: Arc::clone(&deinits),
    });

    app.update();
    assert_eq!(deinits.load(Ordering::SeqCst), 0);

    // 模拟 Bevy 在应用退出时调用 Plugin::cleanup
    app.cleanup();
    assert_eq!(deinits.load(Ordering::SeqCst), 1);
}
