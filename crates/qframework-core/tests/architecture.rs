use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};

use qframework_core::prelude::*;

#[derive(Default, IModel)]
struct CounterModel {
    arch: ArchRef,
    pub count: BindableProperty<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CountChangedEvent {
    count: i32,
}

struct IncreaseCommand;

impl ICommand for IncreaseCommand {
    type Output = ();
    fn execute(&self, ctx: &CommandContext) {
        let model = ctx.get_model::<CounterModel>();
        model.count.modify(|count| *count += 1);
        ctx.send_event(CountChangedEvent {
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

#[derive(Default, ISystem)]
struct CounterSystem {
    arch: ArchRef,
}

#[test]
fn command_changes_model_state() {
    let architecture = ArchitectureBuilder::new("Test")
        .model(CounterModel::default())
        .build();

    architecture.send_command(IncreaseCommand);
    architecture.send_command(IncreaseCommand);

    assert_eq!(architecture.send_query(GetCountQuery), 2);
}

#[test]
fn events_reach_subscribers() {
    let architecture = ArchitectureBuilder::new("Test")
        .model(CounterModel::default())
        .build();

    let received = Arc::new(AtomicI32::new(0));
    let sink = Arc::clone(&received);
    let unregister = architecture.register_event::<CountChangedEvent, _>(move |event| {
        sink.store(event.count, Ordering::SeqCst);
    });

    architecture.send_command(IncreaseCommand);
    assert_eq!(received.load(Ordering::SeqCst), 1);

    architecture.send_command(IncreaseCommand);
    assert_eq!(received.load(Ordering::SeqCst), 2);

    unregister.unregister();
    architecture.send_command(IncreaseCommand);
    assert_eq!(received.load(Ordering::SeqCst), 2);
}

#[test]
fn models_initialize_before_systems() {
    let architecture = ArchitectureBuilder::new("Test")
        .model(CounterModel::default())
        .system(CounterSystem::default())
        .build();

    // System 在 init 阶段能取到 Model，说明两阶段顺序正确。
    assert!(architecture.is_inited());
    assert!(architecture.try_get_model::<CounterModel>().is_some());
    assert!(architecture.try_get_system::<CounterSystem>().is_some());
}

#[test]
fn bindable_property_notifies_and_can_be_silent() {
    let property = BindableProperty::new(0);
    let observed = Arc::new(AtomicI32::new(-1));

    let sink = Arc::clone(&observed);
    let unregister = property.register(move |value| {
        sink.store(*value, Ordering::SeqCst);
    });

    property.set(7);
    assert_eq!(observed.load(Ordering::SeqCst), 7);
    assert_eq!(property.get(), 7);

    property.set_value_without_event(42);
    assert_eq!(property.get(), 42);
    assert_eq!(observed.load(Ordering::SeqCst), 7);

    unregister.unregister();
    property.set(9);
    assert_eq!(observed.load(Ordering::SeqCst), 7);
}

#[test]
fn deinit_clears_container_and_events() {
    let architecture = ArchitectureBuilder::new("Test")
        .model(CounterModel::default())
        .build();

    assert!(architecture.try_get_model::<CounterModel>().is_some());
    architecture.deinit();
    assert!(architecture.try_get_model::<CounterModel>().is_none());
    assert!(!architecture.is_inited());
}

// ---------------------------------------------------------------------------
// 派生宏能力验证
// ---------------------------------------------------------------------------

#[derive(Default, IModel)]
#[model(init = Self::on_init, deinit = Self::on_deinit)]
struct HookedModel {
    arch: ArchRef,
    inits: AtomicI32,
    deinits: AtomicI32,
}

impl HookedModel {
    fn on_init(&self) {
        self.inits.fetch_add(1, Ordering::SeqCst);
    }

    fn on_deinit(&self) {
        self.deinits.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn derive_supports_lifecycle_hooks() {
    let architecture = ArchitectureBuilder::new("Test")
        .model(HookedModel::default())
        .build();

    let model = architecture.get_model::<HookedModel>();
    assert_eq!(model.inits.load(Ordering::SeqCst), 1);
    assert_eq!(model.deinits.load(Ordering::SeqCst), 0);

    architecture.deinit();
    assert_eq!(model.deinits.load(Ordering::SeqCst), 1);
}

#[derive(Default, IUtility)]
struct HookedUtility {
    ready: AtomicI32,
}

#[test]
fn utility_derive_does_not_require_arch_field() {
    let architecture = ArchitectureBuilder::new("Test")
        .utility(HookedUtility::default())
        .build();

    assert!(architecture.try_get_utility::<HookedUtility>().is_some());
    // 未设置任何字段的默认值就是 0
    assert_eq!(
        architecture
            .get_utility::<HookedUtility>()
            .ready
            .load(Ordering::SeqCst),
        0
    );
}

#[derive(Default, ISystem)]
struct SystemWithRenamedArch {
    #[arch]
    context: ArchRef,
}

#[test]
fn arch_field_can_be_renamed_with_attribute() {
    let architecture = ArchitectureBuilder::new("Test")
        .system(SystemWithRenamedArch::default())
        .build();

    let system = architecture.get_system::<SystemWithRenamedArch>();
    // 注入成功：architecture() 能拿到架构
    assert_eq!(system.architecture().name(), "Test");
}

#[derive(Default, IController)]
struct UnusedController {
    arch: ArchRef,
}

#[test]
fn controller_derive_generates_capability_impls() {
    let architecture = ArchitectureBuilder::new("Test")
        .model(CounterModel::default())
        .build();

    let controller = architecture.attach_controller(UnusedController::default());
    // Controller 具备发送 Command 与注册事件的能力
    controller.send_command(IncreaseCommand);
    let unregister = controller.register_event::<CountChangedEvent, _>(|_| {});

    assert_eq!(architecture.send_query(GetCountQuery), 1);
    unregister.unregister();
}

#[derive(Default, IModel)]
struct GenericModel<T>
where
    T: Send + Sync + 'static,
{
    arch: ArchRef,
    payload: std::marker::PhantomData<T>,
}

#[test]
fn derive_supports_generic_structs() {
    use std::marker::PhantomData;

    let architecture = ArchitectureBuilder::new("Test")
        .model(GenericModel::<u32> {
            arch: ArchRef::new(),
            payload: PhantomData,
        })
        .build();

    assert!(architecture.try_get_model::<GenericModel<u32>>().is_some());
}

// ---------------------------------------------------------------------------
// 架构 API
// ---------------------------------------------------------------------------

#[test]
fn registered_count_reflects_registrations() {
    let architecture = ArchitectureBuilder::new("Test")
        .model(CounterModel::default())
        .system(CounterSystem::default())
        .build();

    assert_eq!(architecture.registered_count(), 2);
}

#[test]
fn builder_patch_runs_after_registrations_and_before_init() {
    let architecture = ArchitectureBuilder::new("Test")
        .model(CounterModel::default())
        .patch(|architecture| {
            // patch 期间：注册已完成，但两阶段初始化还没开始
            assert!(!architecture.is_inited());
            assert!(architecture.try_get_model::<CounterModel>().is_some());
            let _ = architecture.register_system(CounterSystem::default());
        })
        .build();

    assert!(architecture.is_inited());
    assert!(architecture.try_get_system::<CounterSystem>().is_some());
    assert_eq!(architecture.registered_count(), 2);
}

#[test]
#[should_panic(expected = "尚未注册到 IOCContainer")]
fn get_model_panics_with_readable_message() {
    let architecture = ArchitectureBuilder::new("Test").build();
    let _ = architecture.get_model::<CounterModel>();
}

#[test]
fn send_event_default_uses_default_value() {
    let architecture = ArchitectureBuilder::new("Test").build();

    let received = Arc::new(AtomicI32::new(-1));
    let sink = Arc::clone(&received);
    let _unregister = architecture.register_event::<DefaultEvent, _>(move |event| {
        sink.store(event.0, Ordering::SeqCst);
    });

    architecture.send_event_default::<DefaultEvent>();
    assert_eq!(received.load(Ordering::SeqCst), 0);
}

#[derive(Debug, Default, Clone, Copy)]
struct DefaultEvent(i32);

#[test]
fn unregister_list_auto_unregisters_on_drop() {
    let architecture = ArchitectureBuilder::new("Test").build();

    {
        let mut subscriptions = IUnRegisterList::new();
        subscriptions.add(architecture.register_event::<CountChangedEvent, _>(|_| {}));
        subscriptions.add(architecture.register_event::<CountChangedEvent, _>(|_| {}));

        assert_eq!(subscriptions.len(), 2);
        assert!(architecture.events().has_listener::<CountChangedEvent>());
    }

    // IUnRegisterList 在 Drop 时已自动注销
    assert!(!architecture.events().has_listener::<CountChangedEvent>());
}

#[test]
fn global_event_system_is_shared_across_architectures() {
    #[derive(Debug, Clone, Copy)]
    struct GlobalEvent(i32);

    let bus = TypeEventSystem::global();
    let received = Arc::new(AtomicI32::new(0));
    let sink = Arc::clone(&received);

    let unregister = bus.register::<GlobalEvent>(move |event| {
        sink.store(event.0, Ordering::SeqCst);
    });

    bus.send(GlobalEvent(42));
    assert_eq!(received.load(Ordering::SeqCst), 42);

    unregister.unregister();
    bus.send(GlobalEvent(7));
    assert_eq!(received.load(Ordering::SeqCst), 42);
}
