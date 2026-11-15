use std::cell::Cell;

use bevy::app::{AppExit, ScheduleRunnerPlugin};
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
        ArchitectureBuilder::new().model(CounterModel::default())
    }
}

#[derive(Resource, Default)]
struct Observed(Vec<i32>);

fn collect_messages(
    mut reader: MessageReader<CountChangedMessage>,
    mut observed: ResMut<Observed>,
) {
    for message in reader.read() {
        observed.0.push(message.count);
    }
}

fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .install_architecture::<TestApp>()
        .bridge_messages::<CountChangedMessage>()
        .init_resource::<Observed>()
        .add_systems(Update, collect_messages);
    app
}

#[test]
fn bevy_worlds_and_shared_application_keep_separate_instances() {
    let first = build_app();
    let second = build_app();
    let shared = TestApp::interface();

    first.architecture().send_command(IncreaseCountCommand);
    assert_eq!(first.architecture().send_query(GetCountQuery), 1);
    assert_eq!(second.architecture().send_query(GetCountQuery), 0);
    assert_eq!(shared.send_query(GetCountQuery), 0);
    assert!(!std::rc::Rc::ptr_eq(&first.architecture(), &shared));

    let first_architecture = first.architecture();
    drop(first);
    assert!(!first_architecture.is_inited());
    assert!(second.architecture().is_inited());
    assert!(shared.is_inited());
    drop(second);
    TestApp::deinit_interface();
}

#[test]
fn architecture_is_available_as_resource() {
    let mut app = build_app();
    app.update();

    let architecture = app.world().non_send::<QArchitecture>();
    assert!(architecture.is_inited());
    assert!(architecture.try_get_model::<CounterModel>().is_some());
}

#[test]
fn bevy_systems_can_send_commands() {
    let mut app = build_app();

    {
        let architecture = app.architecture();
        architecture.send_command(IncreaseCountCommand);
        architecture.send_command(IncreaseCountCommand);
    }

    app.update();

    let architecture = app.world().non_send::<QArchitecture>();
    assert_eq!(architecture.send_query(GetCountQuery), 2);
}

#[test]
fn qframework_events_become_bevy_messages() {
    let mut app = build_app();

    {
        let architecture = app.world().non_send::<QArchitecture>().rc();
        architecture.send_command(IncreaseCountCommand);
        architecture.send_command(IncreaseCountCommand);
    }

    // PreUpdate 把事件转发进 Bevy 消息队列，Update 里被系统读取
    app.update();
    app.update();

    let observed = app.world().resource::<Observed>();
    assert_eq!(observed.0, vec![1, 2]);
}

#[derive(Component, Default)]
struct DisplayedCount(i32);

fn tick(architecture: NonSend<QArchitecture>, mut ticks: Local<u32>) {
    if *ticks < 4 {
        architecture.send_command(IncreaseCountCommand);
        *ticks += 1;
    }
}

fn refresh_scene(architecture: NonSend<QArchitecture>, mut nodes: Query<&mut DisplayedCount>) {
    for mut node in &mut nodes {
        node.0 = architecture.send_query(GetCountQuery);
    }
}

#[test]
fn bevy_systems_update_scene_from_architecture() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .install_architecture::<TestApp>()
        .add_systems(Update, (tick, refresh_scene).chain());
    let node = app.world_mut().spawn(DisplayedCount::default()).id();

    for _ in 0..5 {
        app.update();
    }

    let architecture = app.world().non_send::<QArchitecture>();
    assert_eq!(architecture.send_query(GetCountQuery), 4);
    assert_eq!(app.world().get::<DisplayedCount>(node).unwrap().0, 4);
}

struct OtherApp;

impl QApplication for OtherApp {
    fn build() -> ArchitectureBuilder {
        ArchitectureBuilder::new()
    }
}

#[test]
#[should_panic(expected = "只能安装一个 QFrameworkPlugin")]
fn only_one_qframework_plugin_is_allowed() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .install_architecture::<TestApp>()
        .install_architecture::<OtherApp>();
}

#[test]
fn plugin_cleanup_keeps_architecture_available() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .install_architecture::<TestApp>()
        .add_systems(Update, tick);

    app.finish();
    app.cleanup();
    let architecture = app.architecture();
    assert!(architecture.is_inited());

    app.update();
    assert_eq!(architecture.send_query(GetCountQuery), 1);
}

#[derive(Default, IUtility)]
#[utility(deinit = |this: &LifecycleProbe| this.deinits.set(this.deinits.get() + 1))]
struct LifecycleProbe {
    deinits: Cell<u32>,
}

#[test]
fn world_drop_deinits_architecture_even_with_external_handles() {
    let app = build_app();
    let architecture = app.architecture();
    let probe = architecture.register_utility(LifecycleProbe::default());
    let resource_clone = app.world().non_send::<QArchitecture>().clone();
    drop(resource_clone);
    assert!(architecture.is_inited());

    drop(app);
    assert!(!architecture.is_inited());
    assert!(architecture.try_get_model::<CounterModel>().is_none());
    assert!(!architecture.events().has_listener::<CountChangedMessage>());
    assert_eq!(probe.deinits.get(), 1);
}

#[test]
fn explicit_deinit_then_world_drop_runs_hooks_once() {
    let app = build_app();
    let architecture = app.architecture();
    let probe = architecture.register_utility(LifecycleProbe::default());

    architecture.deinit();
    drop(app);
    assert_eq!(probe.deinits.get(), 1);
}

#[test]
fn default_runner_can_use_architecture_and_deinits_on_return() {
    let mut app = App::new();
    app.install_architecture::<TestApp>()
        .add_systems(Update, tick);
    let architecture = app.architecture();
    let model = architecture.get_model::<CounterModel>();

    assert_eq!(app.run(), AppExit::Success);
    assert_eq!(model.count.get(), 1);
    assert!(!architecture.is_inited());
}

#[test]
fn schedule_runner_can_use_architecture_and_deinits_on_return() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()))
        .install_architecture::<TestApp>()
        .add_systems(Update, tick);
    let architecture = app.architecture();
    let model = architecture.get_model::<CounterModel>();

    assert_eq!(app.run(), AppExit::Success);
    assert_eq!(model.count.get(), 1);
    assert!(!architecture.is_inited());
}

#[test]
fn loop_runner_keeps_architecture_and_bridge_alive_until_exit() {
    fn exit_after_messages(observed: Res<Observed>, mut exit: MessageWriter<AppExit>) {
        if observed.0.len() == 4 {
            assert_eq!(observed.0, vec![1, 2, 3, 4]);
            exit.write(AppExit::Success);
        }
    }

    let mut app = build_app();
    app.add_systems(Update, (tick, exit_after_messages.after(collect_messages)));
    let architecture = app.architecture();
    let probe = architecture.register_utility(LifecycleProbe::default());

    assert_eq!(app.run(), AppExit::Success);
    assert!(!architecture.is_inited());
    assert_eq!(probe.deinits.get(), 1);
}

#[test]
fn dropping_bridge_unregisters_listener_and_releases_queue() {
    let mut app = build_app();
    let architecture = app.architecture();
    let queue = app
        .world()
        .non_send::<QEventBridge<CountChangedMessage>>()
        .queue();
    let weak_queue = std::rc::Rc::downgrade(&queue);
    drop(queue);

    let bridge = app
        .world_mut()
        .remove_non_send::<QEventBridge<CountChangedMessage>>()
        .unwrap();
    drop(bridge);

    assert!(!architecture.events().has_listener::<CountChangedMessage>());
    assert!(weak_queue.upgrade().is_none());
    architecture.send_command(IncreaseCountCommand);
    app.update();
    assert!(app.world().resource::<Observed>().0.is_empty());
}

#[test]
fn update_events_are_forwarded_once_on_the_next_frame() {
    let mut app = build_app();
    app.add_systems(Update, tick);

    app.update();
    assert!(app.world().resource::<Observed>().0.is_empty());
    app.update();
    assert_eq!(app.world().resource::<Observed>().0, vec![1]);

    for _ in 0..4 {
        app.update();
    }
    assert_eq!(app.world().resource::<Observed>().0, vec![1, 2, 3, 4]);
}
