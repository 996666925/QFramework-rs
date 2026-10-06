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
    let mut first = build_app();
    let mut second = build_app();
    let shared = TestApp::interface();

    first.architecture().send_command(IncreaseCountCommand);
    assert_eq!(first.architecture().send_query(GetCountQuery), 1);
    assert_eq!(second.architecture().send_query(GetCountQuery), 0);
    assert_eq!(shared.send_query(GetCountQuery), 0);
    assert!(!std::rc::Rc::ptr_eq(&first.architecture(), &shared));

    first.cleanup();
    assert!(second.architecture().is_inited());
    assert!(shared.is_inited());
    second.cleanup();
    TestApp::deinit_interface();
}

#[test]
fn architecture_is_available_as_resource() {
    let mut app = build_app();
    app.update();

    let architecture = app.world().non_send::<QArchitecture>();
    assert_eq!(architecture.name(), "TestApp");
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
        ArchitectureBuilder::new("OtherApp")
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
fn architecture_is_deinited_on_cleanup() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .install_architecture::<TestApp>();

    app.update();
    let architecture = app.architecture();
    assert!(architecture.is_inited());

    app.cleanup();
    assert!(!architecture.is_inited());
    assert!(architecture.try_get_model::<CounterModel>().is_none());
}
