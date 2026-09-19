//! 文档示例的可执行校验。
//!
//! 这个测试文件存在的唯一目的，是保证 `docs/` 与根 `README` 里的代码片段
//! 与实际 API 保持一致：任何 API 变更如果让文档里的写法失效，这里会编译失败。
//!
//! 覆盖：`docs/core-concepts.md`、`docs/bevy.md`、`docs/derive-macros.md`
//! 中标记为「正确」的示例（文档里标了 ❌ 的反例不会出现在这里）。

#![allow(dead_code)] // 示例里有大量只用于展示的类型与字段

use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bevy::prelude::*;
use qframework_bevy::prelude::*;

// ===========================================================================
// docs/core-concepts.md
// ===========================================================================

#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Stats {
    pub attack: i32,
    pub defense: i32,
    pub speed: i32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct ItemId(pub u32);

#[derive(Debug, Clone, Copy)]
pub struct HpChangedEvent {
    pub hp: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct DamageTakenEvent {
    pub amount: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct LevelCompletedEvent {
    pub level_id: u32,
    pub stars: u8,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GameStartedEvent;

#[derive(Debug, Clone, Copy)]
pub struct ItemPurchasedEvent {
    pub item_id: ItemId,
}

#[derive(Debug, Clone, Copy)]
pub struct PlayerDiedEvent;

// --- IOCContainer ---------------------------------------------------------

#[derive(Debug)]
struct MyConfig {
    retries: u32,
}

#[derive(Debug)]
struct MyService {
    name: String,
}

impl MyService {
    fn new() -> Self {
        Self {
            name: "svc".into(),
        }
    }
}

fn check_ioc() {
    let mut container = qframework_core::IOCContainer::new();
    container.register(MyConfig { retries: 3 });
    container.register_arc(Arc::new(MyService::new()));

    assert!(container.contains::<MyConfig>());
    let _config: Arc<MyConfig> = container.get::<MyConfig>().unwrap();
    assert_eq!(container.len(), 2);
    assert!(!container.is_empty());
    let _panic_free: Arc<MyService> = container.expect::<MyService>();
    container.clear();
}

// --- EasyEvent ------------------------------------------------------------

fn check_easy_event() {
    use qframework_core::EasyEvent;

    let on_refresh = Arc::new(EasyEvent::<()>::new());
    let _unregister = Arc::clone(&on_refresh).register(|_| println!("refresh!"));
    on_refresh.send(&());
    assert_eq!(on_refresh.handler_count(), 1);
    on_refresh.clear();
}

// --- 全局事件系统 ---------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct AppPausedEvent;

fn check_global_bus() {
    let bus = qframework_core::TypeEventSystem::global();
    let _unregister = bus.register::<AppPausedEvent>(|_| {});
    bus.send(AppPausedEvent);
    assert!(bus.has_listener::<AppPausedEvent>());
    bus.unregister_all::<AppPausedEvent>();
    assert!(!bus.has_listener::<AppPausedEvent>());
}

// --- Model ----------------------------------------------------------------

#[derive(Default, IModel)]
struct PlayerModel {
    arch: ArchRef,
    pub hp: BindableProperty<i32>,
    pub gold: BindableProperty<i32>,
    pub inventory: BindableList<ItemId>,
}

impl PlayerModel {
    pub fn take_damage(&self, amount: i32) -> bool {
        let mut died = false;
        self.hp.modify(|hp| {
            *hp = (*hp - amount).max(0);
            died = *hp == 0;
        });
        self.send_event(HpChangedEvent { hp: self.hp.get() });
        if died {
            self.send_event(PlayerDiedEvent);
        }
        died
    }

    pub fn heal(&self, amount: i32) {
        self.hp.modify(|hp| *hp += amount);
        self.send_event(HpChangedEvent { hp: self.hp.get() });
    }
}

// --- Utility --------------------------------------------------------------

#[derive(Default, IUtility)]
struct SaveUtility;

impl SaveUtility {
    pub fn save(&self, slot: u32, data: &str) -> std::io::Result<()> {
        std::fs::write(format!("slot{slot}.json"), data)
    }
}

// --- Controller -----------------------------------------------------------

#[derive(Default, IController)]
#[controller(init = Self::start)]
struct HudController {
    arch: ArchRef,
    subscriptions: Mutex<IUnRegisterList>,
}

impl HudController {
    fn start(&self) {
        let un = self.get_model::<PlayerModel>().hp.register_with_init_value(|hp| {
            println!("HP: {hp}");
        });
        self.subscriptions.lock().unwrap().add(un);

        let un = self.register_event::<DamageTakenEvent, _>(|event| {
            println!("受到 {} 点伤害", event.amount);
        });
        self.subscriptions.lock().unwrap().add(un);
    }
}

// --- System ---------------------------------------------------------------

#[derive(Default, ISystem)]
#[system(init = |this: &AchievementSystem| { this.subscribe(); })]
struct AchievementSystem {
    arch: ArchRef,
    subscriptions: Mutex<IUnRegisterList>,
}

impl AchievementSystem {
    fn subscribe(&self) {
        let un = self.register_event::<DamageTakenEvent, _>(|event| {
            println!("检查成就 {}", event.amount);
        });
        self.subscriptions.lock().unwrap().add(un);
    }
}

#[derive(Default, ISystem)]
struct DebugPanelSystem {
    arch: ArchRef,
}

// --- Command / Query ------------------------------------------------------

#[derive(Debug)]
enum ShopError {
    NotEnoughCurrency,
}

struct PurchaseItemCommand {
    item_id: ItemId,
    quantity: u32,
}

impl ICommand for PurchaseItemCommand {
    type Output = Result<(), ShopError>;

    fn execute(&self, ctx: &CommandContext) -> Self::Output {
        let player = ctx.get_model::<PlayerModel>();
        let price = 10 * self.quantity as i32;

        if player.gold.get() < price {
            return Err(ShopError::NotEnoughCurrency);
        }

        player.gold.modify(|c| *c -= price);
        player.inventory.add(self.item_id);
        ctx.send_event(ItemPurchasedEvent {
            item_id: self.item_id,
        });
        Ok(())
    }
}

struct AddGoldCommand {
    amount: i32,
}

impl ICommand for AddGoldCommand {
    type Output = ();

    fn execute(&self, ctx: &CommandContext) {
        ctx.get_model::<PlayerModel>()
            .gold
            .modify(|gold| *gold += self.amount);
    }
}

struct TakeDamageCommand {
    amount: i32,
}

impl ICommand for TakeDamageCommand {
    type Output = ();
    fn execute(&self, ctx: &CommandContext) {
        ctx.get_model::<PlayerModel>().take_damage(self.amount);
    }
}

struct TickCommand {
    delta: Duration,
}

impl ICommand for TickCommand {
    type Output = ();
    fn execute(&self, _ctx: &CommandContext) {
        let _ = self.delta;
    }
}

struct GetInventoryQuery;

impl IQuery for GetInventoryQuery {
    type Result = Vec<ItemId>;

    fn do_query(&self, ctx: &QueryContext) -> Vec<ItemId> {
        ctx.get_model::<PlayerModel>()
            .inventory
            .with_items(|items| items.to_vec())
    }
}

struct GetGoldQuery;

impl IQuery for GetGoldQuery {
    type Result = i32;
    fn do_query(&self, ctx: &QueryContext) -> i32 {
        ctx.get_model::<PlayerModel>().gold.get()
    }
}

// --- BindableProperty 用法 -------------------------------------------------

fn check_bindable_property() {
    let property = BindableProperty::new(0);

    let _un = property.register(|value| println!("{value}"));
    let _un = property.register_with_init_value(|value| println!("{value}"));

    property.set(7);
    property.modify(|value| *value += 1);
    property.set_value_without_event(42);

    let _: i32 = property.get();
    let _: bool = property.with_value(|value| *value < 20);
    let _: usize = property.handler_count();
    let cloned = property.clone();
    assert_eq!(cloned.get(), 42);
    assert!(property == 42);
    println!("{property}");
    println!("{property:?}");
}

// --- BindableList / BindableDictionary ------------------------------------

fn check_bindable_collections() {
    let list: BindableList<ItemId> = BindableList::new();
    let _un = list.on_add(|e| println!("+{:?} @{}", e.value, e.index));
    let _un = list.on_remove(|e| println!("-{:?} @{}", e.value, e.index));
    let _un = list.on_clear(|_| println!("clear"));
    let _un = list.on_count_changed(|e| println!("count = {}", e.count));

    let _index: usize = list.add(ItemId(1));
    list.insert(0, ItemId(2));
    let _removed: Option<ItemId> = list.remove_at(0);
    let _: usize = list.len();
    let _: bool = list.is_empty();
    let _: Option<ItemId> = list.get(0);
    let _: Vec<ItemId> = list.snapshot();
    let _: usize = list.with_items(|items| items.len());
    list.clear();

    let list = BindableList::from_vec(vec![ItemId(1)]);
    assert_eq!(list.len(), 1);

    let dictionary: BindableDictionary<ItemId, i32> = BindableDictionary::new();
    let _un = dictionary.on_add(|_| {});
    let _un = dictionary.on_remove(|_| {});
    let _un = dictionary.on_replace(|e| println!("{} -> {}", e.previous, e.current));
    let _un = dictionary.on_clear(|_| {});
    let _un = dictionary.on_count_changed(|_| {});

    dictionary.insert(ItemId(1), 10);
    dictionary.insert(ItemId(1), 20);
    let _: Option<i32> = dictionary.get(&ItemId(1));
    let _: bool = dictionary.contains_key(&ItemId(1));
    let _: HashMap<ItemId, i32> = dictionary.snapshot();
    let _: usize = dictionary.with_entries(|entries| entries.len());
    let _: Option<i32> = dictionary.remove(&ItemId(1));
    dictionary.clear();
}

// --- IUnRegister ----------------------------------------------------------

fn check_unregister() {
    let architecture = ArchitectureBuilder::new("Unregister").build();

    let un = architecture.register_event::<GameStartedEvent, _>(|_| {});
    un.unregister();
    un.unregister(); // 幂等

    let mut list = IUnRegisterList::new();
    list.add(architecture.register_event::<GameStartedEvent, _>(|_| {}));
    assert_eq!(list.len(), 1);
    assert!(!list.is_empty());
    list.unregister_all();

    let _empty = IUnRegister::empty();
}

// --- Architecture 其它 API -------------------------------------------------

fn check_architecture_api() {
    let architecture = ArchitectureBuilder::new("Api")
        .utility(SaveUtility)
        .model(PlayerModel::default())
        .system(AchievementSystem::default())
        .patch(|architecture| {
            assert!(!architecture.is_inited());
            let _ = architecture.register_system(DebugPanelSystem::default());
        })
        .build();

    assert_eq!(architecture.name(), "Api");
    assert!(architecture.is_inited());
    assert_eq!(architecture.registered_count(), 4);
    assert!(architecture.try_get_system::<DebugPanelSystem>().is_some());

    // 重复注册同一类型会覆盖先注册的实例
    let replaced = ArchitectureBuilder::new("Replaced")
        .model(PlayerModel::default())
        .model(PlayerModel::default())
        .build();
    assert_eq!(replaced.registered_count(), 1);

    let _: Arc<Architecture> = architecture.arc();
    let _: &qframework_core::TypeEventSystem = architecture.events();
    let _: Option<Arc<PlayerModel>> = architecture.try_get_model::<PlayerModel>();
    let _: Option<Arc<AchievementSystem>> = architecture.try_get_system::<AchievementSystem>();
    let _: Option<Arc<SaveUtility>> = architecture.try_get_utility::<SaveUtility>();

    architecture.send_event(GameStartedEvent);
    architecture.send_event_default::<GameStartedEvent>();
    let _: i32 = architecture.send_query(GetGoldQuery);
    let result: Result<(), ShopError> = architecture.send_command(PurchaseItemCommand {
        item_id: ItemId(1),
        quantity: 1,
    });
    assert!(result.is_err());

    // attach_controller + 手动使用
    let controller: Arc<HudController> = architecture.attach_controller(HudController::default());
    IController::init(controller.as_ref());
    controller.send_command(AddGoldCommand { amount: 5 });
    let _ = controller.send_query(GetInventoryQuery);
    let _un = controller.register_event::<GameStartedEvent, _>(|_| {});
    assert_eq!(controller.architecture().name(), "Api");

    architecture.deinit();
    assert!(!architecture.is_inited());
}

// ===========================================================================
// docs/bevy.md
// ===========================================================================

#[derive(Default, IModel)]
struct CounterModel {
    arch: ArchRef,
    pub count: BindableProperty<i32>,
}

#[derive(Message, Clone, Debug)]
struct CountChangedMessage {
    count: i32,
}

struct IncreaseCountCommand;

impl ICommand for IncreaseCountCommand {
    type Output = ();
    fn execute(&self, ctx: &CommandContext) {
        ctx.get_model::<CounterModel>()
            .count
            .modify(|count| *count += 1);
    }
}

#[derive(IController)]
#[controller(init = Self::start)]
struct CounterController {
    arch: ArchRef,
    frames: AtomicI32,
    subscriptions: Mutex<IUnRegisterList>,
}

impl Default for CounterController {
    fn default() -> Self {
        Self {
            arch: ArchRef::new(),
            frames: AtomicI32::new(0),
            subscriptions: Mutex::new(IUnRegisterList::new()),
        }
    }
}

impl CounterController {
    fn start(&self) {
        let un = self.register_event::<CountChangedMessage, _>(|event| {
            println!("count -> {}", event.count);
        });
        self.subscriptions.lock().unwrap().add(un);
    }
}

impl QControllerUpdate for CounterController {
    fn update(&self, _delta: Duration) {
        if self.frames.fetch_add(1, Ordering::SeqCst) % 60 == 0 {
            self.send_command(IncreaseCountCommand);
        }
    }
}

struct CounterApp;

impl QApplication for CounterApp {
    fn build() -> ArchitectureBuilder {
        ArchitectureBuilder::new("CounterApp")
            .utility(SaveUtility)
            .model(CounterModel::default())
    }
}

fn on_count_changed(mut reader: MessageReader<CountChangedMessage>) {
    for message in reader.read() {
        println!("count = {}", message.count);
    }
}

fn refresh_ui(architecture: Res<QArchitecture>) {
    let _ = architecture.get_model::<CounterModel>().count.get();
}

fn peek(bridge: Res<QEventBridge<CountChangedMessage>>) {
    println!("积压 {}", bridge.len());
    let _queue = bridge.queue();
    bridge.push(CountChangedMessage { count: 1 });
    let drained: Vec<CountChangedMessage> = bridge.drain();
    assert_eq!(drained.len(), 1);
}

fn check_bevy() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_qframework::<CounterApp>()
        .bridge_q_messages::<CountChangedMessage>()
        .add_q_controller(CounterController::default())
        .add_systems(Update, on_count_changed)
        .add_systems(Update, refresh_ui.after(QFrameworkSet::Controllers))
        .add_systems(PreUpdate, peek);

    app.update();

    let controllers = app.world().resource::<QControllers>();
    assert_eq!(controllers.len(), 1);
    assert!(!controllers.is_empty());
    let _count = controllers.iter().count();
    let _handle: Arc<Architecture> = app.world().resource::<QArchitecture>().arc();

    let _ = &controllers;
    app.cleanup();
}

// ===========================================================================
// 手写替代派生宏（docs/derive-macros.md 的「展开后是什么」）
// ===========================================================================

struct ManualModel {
    arch: ArchRef,
}

impl Default for ManualModel {
    fn default() -> Self {
        Self {
            arch: ArchRef::new(),
        }
    }
}

impl qframework_core::HasArchRef for ManualModel {
    fn arch_ref(&self) -> &ArchRef {
        &self.arch
    }
}

impl qframework_core::ICanGetArchitecture for ManualModel {
    fn architecture(&self) -> Arc<Architecture> {
        self.arch.get()
    }
}

impl qframework_core::ICanGetUtility for ManualModel {}
impl qframework_core::ICanSendEvent for ManualModel {}
impl qframework_core::IModel for ManualModel {}

fn check_manual_impl() {
    let manual = ManualModel::default();
    assert!(!manual.arch.is_bound());

    let architecture = ArchitectureBuilder::new("Manual")
        .model(ManualModel::default())
        .build();
    assert!(architecture.get_model::<ManualModel>().arch.is_bound());
}

#[test]
fn ioc_and_event_bus_examples() {
    check_ioc();
    check_easy_event();
    check_global_bus();
}

#[test]
fn bindable_examples() {
    check_bindable_property();
    check_bindable_collections();
}

#[test]
fn unregister_examples() {
    check_unregister();
}

#[test]
fn architecture_api_examples() {
    check_architecture_api();
}

#[test]
fn bevy_examples() {
    check_bevy();
}

#[test]
fn derive_macro_expansion_examples() {
    check_manual_impl();
}
