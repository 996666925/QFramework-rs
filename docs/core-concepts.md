# 核心概念

> 这是一份参考手册。每个概念都给出了语义细节和「什么时候用」。

目录：

- [Architecture 与 ArchitectureBuilder](#architecture-与-architecturebuilder)
- [IOCContainer](#ioccontainer)
- [事件系统](#事件系统)
- [Command 与 Query](#command-与-query)
- [四层对象](#四层对象)
- [BindableProperty](#bindableproperty)
- [BindableList 与 BindableDictionary](#bindablelist-与-bindabledictionary)
- [IUnRegister 与 IUnRegisterList](#iunregister-与-iunregisterlist)
- [生命周期总表](#生命周期总表)

---

## Architecture 与 ArchitectureBuilder

### 创建

`Architecture` 永远通过 `ArchitectureBuilder` 创建，不存在别的构造方式：

```rust
let architecture: Arc<Architecture> = ArchitectureBuilder::new("MyApp")
    .model(PlayerModel::default())        // 注册数据层
    .system(BattleSystem::default())      // 注册业务逻辑层
    .utility(FileUtility::default())      // 注册工具层
    .patch(|architecture| {               // 追加注册（等价于 C# 的 OnRegisterPatch）
        if enable_debug_panel {
            architecture.register_system(DebugPanelSystem::default());
        }
    })
    .build();                             // 注册 + 两阶段初始化，返回 Arc
```

`patch` 的执行时机被精确定义为：**所有 `model` / `system` / `utility` 之后、两阶段初始化之前**。此时层对象已进容器、架构引用已注入，但 `init()` 还没调用，所以你可以从容器里取对象，也可以再注册新对象。

> ⚠️ **重复注册同一类型会静默覆盖。** IOC 以 `TypeId` 为键，每种类型只保留一个实例：
> ```rust
> let architecture = ArchitectureBuilder::new("App")
>     .model(PlayerModel::default())
>     .model(PlayerModel::default())      // 覆盖上面那个，不报错
>     .build();
> assert_eq!(architecture.registered_count(), 1);
> ```
> 这在 `patch` 里尤其容易踩到——注册一个已经注册过的类型等于替换它，而不是「再加一个」。想装多个同类实例，用 `BindableList` / `BindableDictionary` 把它们收进同一个 Model。

### 取层对象

```rust
let model  = architecture.get_model::<PlayerModel>();      // 不存在会 panic，附类型名
let model  = architecture.try_get_model::<PlayerModel>();  // Option
let system = architecture.get_system::<BattleSystem>();
let util   = architecture.get_utility::<FileUtility>();
```

返回的是 `Arc<T>`，不是引用。这意味着你可以安全地把它存进别的地方、发到别的线程——析构由引用计数管理。

`get_model` 的 panic 信息带类型全名：

```
类型 `my_game::model::PlayerModel` 尚未注册到 IOCContainer，
请检查 ArchitectureBuilder::model/system/utility
```

### 通信

```rust
// 命令：同步执行，返回 C::Output
let gained: i32 = architecture.send_command(AddGoldCommand { amount: 100 });

// 查询：同步执行，返回 Q::Result
let gold: i32 = architecture.send_query(GetGoldQuery);

// 事件：同步派发给所有订阅者，没有返回值
architecture.send_event(GoldChangedEvent { gold });

// 无字段的事件可以省掉构造：要求 E: Default
#[derive(Default)]
struct GameStartedEvent;
architecture.send_event_default::<GameStartedEvent>();
```

### 其它

| 方法 | 说明 |
|---|---|
| `name() -> &'static str` | 架构名，仅供日志与调试 |
| `is_inited() -> bool` | 是否已完成两阶段初始化 |
| `registered_count() -> usize` | 已注册的层对象数量 |
| `arc() -> Arc<Architecture>` | 取得自身句柄（层对象内部用它注入引用） |
| `events() -> &TypeEventSystem` | 直接访问事件总线 |
| `attach_controller(c) -> Arc<C>` | 给控制器注入架构引用（不进 IOC） |
| `init()` / `deinit()` | 生命周期，正常由 builder / Bevy 插件调用 |

**`deinit()` 是终态操作。** 调用后 IOC 与事件都被清空，`get_model` 之类会 panic。重复调用安全。在 Bevy 中由 `QFrameworkPlugin::cleanup` 自动触发。

---

## IOCContainer

基于 `TypeId -> Arc<dyn Any + Send + Sync>` 的类型化容器。一般不需要直接用——`Architecture` 已经把常用的取用封装好了。

```rust
use qframework_core::IOCContainer;

let mut container = IOCContainer::new();
container.register(MyConfig { retries: 3 });
container.register_arc(Arc::new(MyService::new()));

assert!(container.contains::<MyConfig>());
let config: Arc<MyConfig> = container.get::<MyConfig>().unwrap();
container.clear();
```

**关键约束：每种类型只能有一个实例。** 后注册的同类型会覆盖先注册的。这也解释了为什么架构是「每个 Model / System / Utility 类型各一个」。

想装多个同类实例？把它包进一个集合类型里：

```rust
#[derive(Default, IModel)]
struct EnemyRegistryModel {
    arch: ArchRef,
    pub enemies: BindableDictionary<EnemyId, EnemyData>,
}
```

---

## 事件系统

### 「类型即频道」

每个事件类型 `T` 对应一个 `EasyEvent<T>`，不存在字符串频道，因此拼错名字是不可能的。

```rust
#[derive(Debug, Clone, Copy)]
struct LevelCompletedEvent { level_id: u32, stars: u8 }

// 注册（返回注销句柄，务必保存）
let unregister = architecture.register_event::<LevelCompletedEvent, _>(|event| {
    println!("通关 {}，{} 星", event.level_id, event.stars);
});

// 发送
architecture.send_event(LevelCompletedEvent { level_id: 1, stars: 3 });

// 注销
unregister.unregister();
```

### 必须知道的 5 条语义

1. **事件不缓冲。** 发送时若该类型还没有任何订阅者，事件被**直接丢弃**。这是它和 Bevy `Message` 最大的区别——Bevy 的消息会保留一段时间供 `MessageReader` 读取，QFramework 的事件是立即推送。
2. **同步派发。** `send_event` 在调用线程上依次执行所有回调，返回时所有回调已经跑完。回调耗时 = 发送方被阻塞的时间。
3. **派发前会快照订阅者列表**，所以在回调里注册或注销订阅者是安全的，不会破坏迭代。
4. **按注册顺序调用**，没有优先级机制。
5. **订阅是强引用。** 忘记注销会让回调捕获的所有东西都无法释放。

### 单类型事件：EasyEvent

`EasyEvent<T>` 就是「某一个类型的事件」本身，如果你只需要一个局部的事件源（比如一个 UI 组件的内部通知），不需要动用全局总线：

```rust
use std::sync::Arc;
use qframework_core::EasyEvent;

let on_refresh = Arc::new(EasyEvent::<()>::new());
let _unregister = Arc::clone(&on_refresh).register(|_| println!("refresh!"));
on_refresh.send(&());
```

注意 `register` 的接收者是 `Arc<Self>`（内部要存一个 `Weak` 以便自动失效），所以要先 `Arc` 起来。

### 全局事件系统

不持有架构的代码（比如一个独立的工具模块）可以用进程级全局总线：

```rust
let bus = TypeEventSystem::global();
let _unregister = bus.register::<AppPausedEvent>(|_| { /* ... */ });
bus.send(AppPausedEvent);
```

它是 `&'static` 的，跨架构共享，但**注册项会在进程存活期间一直存在**（除了显式注销）。适合框架级、真正全局的事件，不要拿它当架构内事件用。

### 相关 API

| 方法 | 说明 |
|---|---|
| `TypeEventSystem::global() -> &'static Self` | 全局总线 |
| `register::<T, F>(f) -> IUnRegister` | 订阅，返回注销句柄 |
| `send::<T>(event)` / `send_default::<T>()` | 广播 |
| `has_listener::<T>() -> bool` | 是否有人订阅 |
| `unregister_all::<T>()` | 移除该类型的所有订阅者 |
| `clear()` | 清空整个总线（架构 `deinit` 时调用） |

---

## Command 与 Query

### 为什么要有它们

`architecture.get_model::<PlayerModel>().hp.set(0)` 能编译，但它是反模式。Command 的价值在于给「状态变更」一个**有名字的、唯一的位置**：

- 搜索 `impl ICommand for` 就能列出所有写操作。
- Command 是数据结构，可以序列化 → 支持录像回放、帧同步、网络同步。
- 测试时不需要启动完整游戏，构造空架构发一个 Command 断言结果即可。
- 日志、埋点、权限校验可以在 `send_command` 路径上统一挂载。

### 写一个 Command

```rust
struct PurchaseItemCommand {
    item_id: ItemId,
    quantity: u32,
}

impl ICommand for PurchaseItemCommand {
    type Output = Result<(), ShopError>;   // 无返回值时写 `type Output = ();`

    fn execute(&self, ctx: &CommandContext) -> Self::Output {
        let shop  = ctx.get_model::<ShopModel>();
        let price = shop.price_of(self.item_id) * self.quantity;

        if shop.currency.get() < price {
            return Err(ShopError::NotEnoughCurrency);
        }

        // 原子地「读-改-写」
        shop.currency.modify(|c| *c -= price);
        shop.owned.modify(|map| { *map += self.quantity; });

        ctx.send_event(ItemPurchasedEvent { item_id: self.item_id });
        Ok(())
    }
}
```

**Command 不应该有「状态」**：字段只能是「本次调用的输入参数」。不要在里面存 `Arc<Something>` 缓存、不要用 `&mut self` 之外的方式持有可变状态。用单元结构体表达无参数命令（`struct TickCommand;`）。

### 写一个 Query

```rust
struct GetInventoryQuery;

impl IQuery for GetInventoryQuery {
    type Result = Vec<ItemId>;

    fn do_query(&self, ctx: &QueryContext) -> Vec<ItemId> {
        ctx.get_model::<InventoryModel>().items.with_items(|items| items.to_vec())
    }
}
```

Query 必须是纯只读的。返回大对象时优先返回引用/切片**做不到**（结果必须是 `Send + 'static`），所以要么返回 `Rc`/`Arc` 包的数据，要么返回小的聚合结果（例如 `len()` 而不是整个 `Vec`）。

### 上下文的能力边界

| 能力 | `CommandContext` | `QueryContext` |
|---|:---:|:---:|
| `get_model` / `get_system` | ✓ | ✓ |
| `get_utility` | ✗ | ✓ |
| `send_command` | ✓ | ✗ |
| `send_query` | ✗ | ✗ |
| `send_event` | ✓ | ✗ |
| `register_event` | ✗ | ✗ |

如果 Command 里需要 Utility（比如读存档），把它改成从 Model 读，或者把这件事挪到 System 里——这正是分层规则想逼你做的思考。

### 什么时候可以不写 Query

Query 的价值是把「怎么取」和「取什么」解耦。如果调用方本来就持有架构引用（Bevy 系统里的 `Res<QArchitecture>`），直接 `architecture.get_model::<X>()` 完全没问题。

**Query 真正的价值在于**：跨层的只读契约、需要在多个地方复用同一段取数逻辑、或者你想让「读」这个动作本身有名字（便于日志/性能统计）。

---

## 四层对象

四层的共同点：

- 都实现 `Send + Sync + 'static`（架构要放进 Bevy Resource，必须能跨线程共享）。
- 都有可选的 `init(&self)` / `deinit(&self)` 钩子。
- Controller / System / Model 需要 `ArchRef` 字段（`Weak<Architecture>`），Utility 不需要。

### IController（表现层）

**能**：取 System / Model，发 Command / Query，注册事件。
**不能**：发事件、取 Utility。

```rust
#[derive(Default, IController)]
struct HudController {
    arch: ArchRef,
    subscriptions: Mutex<IUnRegisterList>,
}

impl HudController {
    fn start(&self) {
        // 用 register_with_init_value：注册的同时立刻刷新一次初始显示
        let un = self
            .get_model::<PlayerModel>()
            .hp
            .register_with_init_value(|hp| println!("HP: {hp}"));
        self.subscriptions.lock().unwrap().add(un);

        let un = self.register_event::<DamageTakenEvent, _>(|event| {
            println!("受到 {} 点伤害", event.amount);
        });
        self.subscriptions.lock().unwrap().add(un);
    }
}
```

Controller 不能发事件，所以它无法把「用户点了按钮」直接广播出去——正确做法是发一个 Command，让 Command 去改状态并广播事件：

```rust
// ❌ 编译不过：IController 没有 ICanSendEvent
self.send_event(ButtonClickedEvent);

// ✓
self.send_command(ClickButtonCommand { button_id });
```

### ISystem（业务逻辑层）

**能**：取 System / Model / Utility，发事件，注册事件。
**不能**：发 Command、发 Query。

System 是「跨多个 Controller 复用的逻辑」的归属地。典型例子：一个成就系统需要监听战斗、商店、任务三处的事件，这些逻辑放在任何一个 Controller 里都会重复。

```rust
#[derive(Default, ISystem)]
#[system(init = |this: &AchievementSystem| { this.subscribe(); })]
struct AchievementSystem {
    arch: ArchRef,
    subscriptions: Mutex<IUnRegisterList>,
}

impl AchievementSystem {
    fn subscribe(&self) {
        let un = self.register_event::<EnemyKilledEvent, _>(|event| {
            // 这里可以做跨 Model 的协调，这是 Model 做不到的
            println!("击杀 {} -> 检查成就", event.enemy_id);
        });
        self.subscriptions.lock().unwrap().add(un);
    }
}
```

### IModel（数据层）

**能**：取 Utility，发事件。
**不能**：取 Model / System，发 Command / Query，注册事件。

```rust
#[derive(Default, IModel)]
struct PlayerModel {
    arch: ArchRef,
    pub hp: BindableProperty<i32>,
    pub gold: BindableProperty<i32>,
    pub inventory: BindableList<ItemId>,
}

impl PlayerModel {
    /// 数据层自己负责「改完之后通知外界」
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
}
```

把「数据 + 它的 CRUD + 变更通知」放在一起，是 Model 的完整职责。**但业务规则不要放这里**：`take_damage` 里可以做「血量不低于 0」，但不应该判断「有没有无敌帧」「是不是 BOSS」——那是 Command 或 System 的事。

### IUtility（工具层）

**能**：什么都拿不到。它就是一层封装。

```rust
#[derive(Default, IUtility)]
struct SaveUtility;

impl SaveUtility {
    pub fn save(&self, slot: u32, data: &SaveData) -> std::io::Result<()> {
        let json = serde_json::to_vec(data)?;
        std::fs::write(format!("slot{slot}.json"), json)
    }

    pub fn load(&self, slot: u32) -> std::io::Result<SaveData> {
        let bytes = std::fs::read(format!("slot{slot}.json"))?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}
```

Utility 拿不到架构是刻意的：它可以是纯函数式的、可以被单测、可以被替换成 mock。需要访问游戏数据 → 由调用它的 Model / System 把数据传进去。

---

## BindableProperty

一个可观察的值。Model 状态的默认载体。

### 读

```rust
let hp: i32 = player.hp.get();                          // 克隆一份
let is_low: bool = player.hp.with_value(|hp| *hp < 20); // 借用，不克隆
```

**`get()` 会克隆整个 `T`。** 如果 `T` 很大（`Vec<Item>`、`HashMap<..>`），用 `with_value` 只读需要的那部分：

```rust
// ❌ 每次读都克隆整个背包
let has_sword = player.inventory.get().contains(&ItemId::Sword);

// ✓ 零拷贝
let has_sword = player.inventory.with_value(|items| items.contains(&ItemId::Sword));
```

### 写

```rust
player.hp.set(100);                                  // 覆盖，通知
player.hp.modify(|hp| *hp = (*hp - 10).max(0));       // 读-改-写，原子，通知
player.hp.set_value_without_event(100);               // 静默写入，不通知
```

| 方法 | 通知 | 原子性 | 用途 |
|---|:---:|:---:|---|
| `set(v)` | ✓ | 单次写入是原子的 | 直接赋值 |
| `modify(f)` | ✓ | **整个读-改-写是原子的** | 计数器、条件修改 |
| `set_value_without_event(v)` | ✗ | — | 初始化、批量同步 |

`set` 不做相等判断：即使新旧值相同也会通知。需要去重就在回调里自己判断，或者用 `if prop.get() != v { prop.set(v) }`。

### 订阅

```rust
// 只订阅未来的变化
let un = player.hp.register(|hp| println!("HP: {hp}"));

// 订阅 + 立刻用当前值回调一次（UI 初始化利器）
let un = player.hp.register_with_init_value(|hp| hp_label.text = hp.to_string());
```

`register_with_init_value` 的实现是**先注册、再读取当前值回调**，因此不会漏掉注册窗口内发生的其它变更；并发写入时可能出现一次重复回调，但不会丢更新。对 UI 刷新来说重复无害、丢失致命，这个取舍是有意的。

### 其它

```rust
player.hp == 100;                    // impl PartialEq<T>
println!("{}", player.hp);           // impl Display（读当前值）
println!("{:?}", player.hp);         // impl Debug
let hp2 = player.hp.clone();         // 克隆的是 Arc，共享同一份数据
player.hp.handler_count();           // 当前订阅者数量
```

---

## BindableList 与 BindableDictionary

### BindableList

```rust
#[derive(Default, IModel)]
struct InventoryModel {
    arch: ArchRef,
    pub items: BindableList<ItemId>,
}

// 读
model.items.len();
model.items.get(0);                                  // Option<ItemId>（克隆）
model.items.snapshot();                              // Vec<T>（整体克隆）
model.items.with_items(|items| items.len());         // 借用切片，零拷贝

// 写
model.items.add(item_id);                            // 返回下标
model.items.insert(0, item_id);
model.items.remove_at(0);                            // Option<T>
model.items.clear();

// 订阅
model.items.on_add(|e| println!("+{} @{}", e.value, e.index));
model.items.on_remove(|e| println!("-{} @{}", e.value, e.index));
model.items.on_clear(|_| println!("clear"));
model.items.on_count_changed(|e| println!("count = {}", e.count));
```

### BindableDictionary

```rust
#[derive(Default, IModel)]
struct StatsModel {
    arch: ArchRef,
    pub stats: BindableDictionary<StatKind, i32>,
}

model.stats.insert(StatKind::Attack, 10);
model.stats.insert(StatKind::Attack, 20);      // 已存在 → 触发 on_replace 而非 on_add
model.stats.get(&StatKind::Attack);            // Option<i32>
model.stats.contains_key(&StatKind::Attack);
model.stats.remove(&StatKind::Attack);
model.stats.with_entries(|entries| entries.len());
```

### 事件类型一览

| 类型 | 事件 | 字段 |
|---|---|---|
| List | `ListAdd<T>` | `index`, `value` |
| List | `ListRemove<T>` | `index`, `value` |
| List | `ListClear` | — |
| List | `ListCountChanged` | `count` |
| Dict | `DictAdd<K, V>` | `key`, `value` |
| Dict | `DictRemove<K, V>` | `key`, `value` |
| Dict | `DictReplace<K, V>` | `key`, `previous`, `current` |
| Dict | `DictClear` | — |
| Dict | `DictCountChanged` | `count` |

（`on_add` / `on_remove` / `on_clear` / `on_count_changed` / `on_replace` 的返回值都是 `IUnRegister`。）

### 语义注意

**事件在数据写入之后发出**，不是持有锁期间发出。因此在多线程并发写入时，事件到达顺序不保证与写入顺序一致；但每个事件携带的值一定是当时真实写入的值。

**`clear()` 对空集合是空操作**，不会发出 `on_clear`。

---

## IUnRegister 与 IUnRegisterList

任何订阅都会返回一个 `IUnRegister`。这是防止内存泄漏的唯一手段——回调是强引用，不注销就会一直活着。

### 三种注销方式

```rust
// 1. 手动注销（最简单）
let un = model.hp.register(|hp| println!("{hp}"));
un.unregister();

// 2. 交给列表，Drop 时自动注销（推荐）
struct MyController {
    arch: ArchRef,
    subscriptions: Mutex<IUnRegisterList>,
}

self.subscriptions.lock().unwrap().add(un);
// 控制器析构时 Mutex 析构 -> IUnRegisterList 析构 -> 逐个 unregister

// 3. 手动批量注销
subscriptions.lock().unwrap().unregister_all();
```

`unregister()` 是幂等的，重复调用安全。

### 会自动失效的句柄

`IUnRegister` 内部持有 `Weak<EasyEvent<T>>`。如果架构已经 `deinit` 或整个被析构，句柄会静默变成空操作，不会 panic。这是 Rust 版相比 C# 版的一个优势：不需要记得在 GameObject 销毁前清理。

---

## 生命周期总表

| 阶段 | Model | System | Utility | Controller |
|---|---|---|---|---|
| 注册 | `ArchitectureBuilder::model` | `.system` | `.utility` | `App::add_q_controller` / `attach_controller` |
| 注入架构引用 | ✓（Construction） | ✓ | ✗（不需要） | ✓ |
| `init(&self)` | 第 1 批 | 第 2 批 | 第 2 批 | 注册时立即调用 |
| `deinit(&self)` | 最后 | 最先（与 Utility 一起） | 最先（与 System 一起） | `QControllers::deinit_all()` |
| 析构 | 架构 `deinit()` 或整体 drop | 同左 | 同左 | `QControllers` drop |

**注册顺序 = `init` 顺序**（同一批次内）。所以 `.model(A).model(B)` 保证 A 先于 B 初始化。

**运行期注册**（`architecture.register_model(..)` 在 `build()` 之后调用）会立即执行 `init()`，不需要重新初始化整个架构。

---

## 下一步

- 怎么定义这些层对象 → [派生宏](derive-macros.md)
- 怎么写出好的业务代码 → [最佳实践](best-practices.md)
- 在 Bevy 里怎么落地 → [Bevy 集成](bevy.md)
