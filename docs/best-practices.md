# 最佳实践

> 这份文档是「写代码前先看一遍」的那种。每一条都给出了理由和正反例。

目录：

- [一、思维模型：这块数据属于谁](#一思维模型这块数据属于谁)
- [二、Command 设计](#二command-设计)
- [三、并发相关（最重要的一节）](#三并发相关最重要的一节)
- [四、Model 设计](#四model-设计)
- [五、事件 vs 状态：怎么选](#五事件-vs-状态怎么选)
- [六、性能](#六性能)
- [七、订阅生命周期](#七订阅生命周期)
- [八、测试策略](#八测试策略)
- [九、命名与文件组织](#九命名与文件组织)
- [十、反模式清单](#十反模式清单)

---

## 一、思维模型：这块数据属于谁

动手写之前先回答三个问题：

1. **它是游戏逻辑还是渲染表现？**
   是游戏逻辑 → Model；是渲染表现（位置、旋转、动画进度）→ Bevy Component。
   不要两边各存一份。

2. **谁有权改它？**
   只有 Command（以及 Model 自己的方法，由 Command 调用）。

3. **谁需要知道它变了？**
   需要「当前值」 → `BindableProperty` + `register`；需要「发生了一件事」 → 事件。

一条经验法则：**如果一个数据被两个以上 Controller 读取，它就该在 Model 里，而不是某个 Controller 的私有状态。**

---

## 二、Command 设计

### 2.1 命名要表达意图，不要暴露实现

Command 是「玩家/系统想做什么」，不是「把某字段改成某值」。

```rust
// ❌ 这是 setter，不是 Command
struct SetCurrencyCommand { value: i32 }

// ✓ 这是意图，可以加规则、可以校验、可以记录
struct PurchaseItemCommand { item_id: ItemId, quantity: u32 }
```

好处是当规则变化时（比如加「打折」「限购」），改的是 `PurchaseItemCommand` 一个地方，所有调用方自动获得新行为。而 `SetCurrencyCommand` 永远无法承载规则。

### 2.2 一个 Command 做一件事

```rust
// ❌ 做了三件事，失败时不知道哪一步失败、无法部分重试
struct DoEverythingCommand;

// ✓
struct PurchaseItemCommand { item_id: ItemId, quantity: u32 }
struct EquipItemCommand { item_id: ItemId }
```

### 2.3 Command 不能持有状态

字段只能是「本次调用的输入」。不要缓存 `Arc`、不要存 `&mut` 引用。

```rust
// ❌
struct BadCommand {
    cached_model: Arc<PlayerModel>,   // 状态
}

// ✓
struct GoodCommand {
    amount: i32,                      // 纯输入
}
```

无参数的命令用单元结构体：`struct TickCommand;`

### 2.4 返回值用 `Output`，跨层通知用事件

```rust
impl ICommand for PurchaseItemCommand {
    // 调用方立刻需要知道的结果 → Output
    type Output = Result<Receipt, ShopError>;

    fn execute(&self, ctx: &CommandContext) -> Self::Output {
        // ...
        // 其它模块需要知道的变化 → 事件
        ctx.send_event(ItemPurchasedEvent { item_id: self.item_id });
        Ok(receipt)
    }
}
```

判断标准：**「谁需要知道」**。只有调用方需要 → `Output`；别的地方（UI、成就系统、音效）也可能需要 → 事件。

### 2.5 命令里不要吞掉错误

```rust
// ❌ 出错只打日志，调用方以为成功了
fn execute(&self, ctx: &CommandContext) {
    let _ = self.do_thing();
}

// ✓
fn execute(&self, ctx: &CommandContext) -> Result<(), MyError> {
    self.do_thing()?;
    Ok(())
}
```

---

## 三、并发相关（最重要的一节）

这是 Rust 版和 C# 版**语义差异最大**的地方。C# 版跑在 Unity 主线程，这里可能跑在 Bevy 的并行系统里。

### 3.1 复合修改必须用 `modify`

```rust
// ❌ 会丢更新。两个系统并发执行时，可能都读到 10，都写 11
let next = model.gold.get() + 100;
model.gold.set(next);

// ✓ 整个「读-改-写」在一次写锁内完成，是原子的
model.gold.modify(|gold| *gold += 100);
```

**规则**：任何形如「读出来、算一下、写回去」的操作都要用 `modify`。只有「无条件覆盖成某个已知值」才可以用 `set`。

这条同样适用于 `send_command`——架构不对命令做排队或串行化，命令在调用线程上同步执行：

```rust
// 如果两个系统同时发这个命令，下面的加法必须在命令内部是原子的
architecture.send_command(AddGoldCommand { amount: 100 });

impl ICommand for AddGoldCommand {
    type Output = ();
    fn execute(&self, ctx: &CommandContext) {
        ctx.get_model::<PlayerModel>()
            .gold
            .modify(|gold| *gold += self.amount);   // ← 关键在这里
    }
}
```

### 3.2 不要在 `modify` / `with_value` 的闭包里重入同一个属性

`std::sync::RwLock` 不可重入，以下代码会**死锁**（不是编译错误，是运行时卡死）：

```rust
// ❌ 死锁
model.gold.modify(|gold| {
    *gold += 100;
    println!("{}", model.gold.get());    // 重新获取读锁 → 死锁
});

// ✓ 先算完，再在外面读
model.gold.modify(|gold| *gold += 100);
println!("{}", model.gold.get());
```

访问**其它**属性是安全的（锁顺序一致）：

```rust
// ✓ 没问题
model.gold.modify(|gold| {
    let cost = model.price.get();        // 另一个属性的读锁
    *gold -= cost;
});
```

### 3.3 初始化阶段是单线程的，可以放心用 `set`

`init()` 在 `build()` 里同步执行，此时还没有并行系统。用 `set` 完全没问题：

```rust
#[model(init = Self::on_init)]
impl PlayerModel {
    fn on_init(&self) {
        self.hp.set(100);        // ✓ 初始化阶段，安全
        self.gold.set(0);
    }
}
```

### 3.4 集合的并发写入

`BindableList` / `BindableDictionary` 的每次操作本身是原子的，但**事件顺序不保证与写入顺序一致**（事件在释放锁之后发出）。

如果消费方依赖顺序，要么保证只有一个写入方（推荐：所有写入都经过同一个 System），要么在事件里带上自己的序号。

---

## 四、Model 设计

### 4.1 用 `BindableProperty` 而不是裸字段

```rust
// ❌ 外界无法感知变化，只能轮询
#[derive(Default, IModel)]
struct PlayerModel {
    arch: ArchRef,
    pub hp: i32,
}

// ✓
#[derive(Default, IModel)]
struct PlayerModel {
    arch: ArchRef,
    pub hp: BindableProperty<i32>,
}
```

裸字段不是不能用（比如纯内部计算用的临时量），但**需要被外部观察的状态一律用 `BindableProperty`**。

### 4.2 相关字段聚合成一个属性

```rust
// ❌ 一次命令改 3 个属性 → 3 次通知 → UI 刷新 3 次
struct StatsModel {
    arch: ArchRef,
    pub attack: BindableProperty<i32>,
    pub defense: BindableProperty<i32>,
    pub speed: BindableProperty<i32>,
}

// ✓ 一个属性，一次通知
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Stats {
    pub attack: i32,
    pub defense: i32,
    pub speed: i32,
}

#[derive(Default, IModel)]
struct StatsModel {
    arch: ArchRef,
    pub stats: BindableProperty<Stats>,
}

impl StatsModel {
    fn apply_buff(&self, buff: Buff) {
        self.stats.modify(|stats| {
            stats.attack += buff.attack;
            stats.defense += buff.defense;
            stats.speed += buff.speed;
        });   // 只通知一次
    }
}
```

如果确实需要分开的字段，用「静默写 + 一次通知」：

```rust
self.attack.set_value_without_event(new_attack);
self.defense.set_value_without_event(new_defense);
self.version.modify(|v| *v += 1);   // 只通知 version 的订阅者
```

### 4.3 业务规则不要放 Model

Model 的职责是**数据 + 它的 CRUD + 变更通知**。

```rust
impl PlayerModel {
    // ✓ 数据层的规则：血量不能为负
    pub fn take_damage(&self, amount: i32) {
        self.hp.modify(|hp| *hp = (*hp - amount).max(0));
        self.send_event(HpChangedEvent { hp: self.hp.get() });
    }
}

// ✓ 业务规则放到 Command
impl ICommand for TakeDamageCommand {
    type Output = ();
    fn execute(&self, ctx: &CommandContext) {
        let player = ctx.get_model::<PlayerModel>();

        // 业务规则：无敌帧内免疫
        if ctx.get_model::<BattleModel>().invincible.get() {
            return;
        }

        player.take_damage(self.amount);
    }
}
```

判断标准：**需要读另一个 Model 才能决定的，就不是 Model 的职责**（因为 Model 拿不到别的 Model）。

### 4.4 Model 之间不互相引用

`IModel` 没有 `ICanGetModel`，这是设计不是缺陷。需要跨 Model 协调 → 放到 Command 或 System。

```rust
// ❌ 编译不过：IModel 没有 get_model
self.get_model::<InventoryModel>().remove(item);

// ✓ 在 Command 里协调
let inventory = ctx.get_model::<InventoryModel>();
let shop = ctx.get_model::<ShopModel>();
inventory.remove(self.item_id);
shop.currency.modify(|c| *c -= price);
```

---

## 五、事件 vs 状态：怎么选

这是最容易搞混的一组概念。

| 问题 | 用这个 |
|---|---|
| 「现在有多少金币？」 | `BindableProperty` + `register` |
| 「刚刚发生了购买」 | 事件 `ItemPurchasedEvent` |
| 「玩家血量持续可见」 | `BindableProperty` |
| 「玩家死了（一次性动作）」 | 事件 `PlayerDiedEvent` |
| 「背包现在有什么」 | `BindableList` |
| 「获得了一件物品（动作）」 | 事件 `ItemGainedEvent` |

**决策规则**：

- 需要**读取当前值** → 状态（`BindableProperty` / `BindableList` / `BindableDictionary`）
- 只关心**发生了一次** → 事件
- 两者都需要 → 状态存数据，事件做通知（这是最常见的组合）

### 常见错误：用事件存状态

```rust
// ❌ 事件不缓冲，晚订阅的人永远收不到，无法回答「现在是多少」
struct GoldChangedEvent { gold: i32 }

// ✓ 状态 + 事件都要
#[derive(Default, IModel)]
struct PlayerModel {
    arch: ArchRef,
    pub gold: BindableProperty<i32>,   // 回答「现在是多少」
}

impl PlayerModel {
    pub fn add_gold(&self, amount: i32) {
        self.gold.modify(|gold| *gold += amount);
        self.send_event(GoldChangedEvent { gold: self.gold.get() });   // 通知「变了」
    }
}
```

### 常见错误：忘记事件不缓冲

```rust
// 注册发生在发送之后 → 永远收不到
architecture.send_event(GameStartedEvent);
architecture.register_event::<GameStartedEvent, _>(|_| println!("started"));
```

事件在发送时如果没有订阅者，就被**丢弃**了。需要「晚订阅也能拿到」的语义，就用状态。

### 常见错误：用事件做请求-响应

```rust
// ❌ 发个事件，指望别人回一个事件——拿不到结果，也说不清谁该回
self.send_event(NeedConfigEvent);
let config = ???;

// ✓ 直接问
let config = ctx.get_model::<ConfigModel>();         // 或
let config = self.send_query(GetConfigQuery);        // Controller 可用
```

---

## 六、性能

### 6.1 读大对象用借用式 API

```rust
// ❌ 每次读都克隆整个背包
let has_sword = inventory.items.get().contains(&ItemId::Sword);

// ✓ 零拷贝
let has_sword = inventory.items.with_value(|items| items.contains(&ItemId::Sword));

// 列表 / 字典同理
inventory.items.with_items(|items| items.len());
stats.with_entries(|entries| entries.len());
```

| 方法 | 拷贝 | 用于 |
|---|---|---|
| `get()` / `snapshot()` | 整块克隆 | 需要把值带走 |
| `with_value()` / `with_items()` / `with_entries()` | 零拷贝 | 只看一眼 |

### 6.2 不要在事件回调里做重活

事件是**同步派发**的：`send_event` 返回时所有回调都跑完了。回调里的耗时直接变成发送方的阻塞时间。

```rust
// ❌ 在 Model 的发送路径上做重活
self.send_event(SomethingHappened);
// 某个订阅者在这里读文件 / 发网络请求 / 加载资源

// ✓ 订阅者只做轻量的事：改标记、推队列、发命令
```
重活应该：① 放到 Command 里由调用方显式触发；② 在 Bevy 侧用消息桥接 + 异步任务处理。

### 6.3 用推送代替轮询

```rust
// ❌ 每帧读一次，即使值没变也刷新 UI
fn sync_hp(architecture: Res<QArchitecture>, mut text: Query<&mut Text>) {
    let hp = architecture.get_model::<PlayerModel>().hp.get();
    for mut t in &mut text { **t = hp.to_string(); }
}

// ✓ 只在变化时推送
// 1) Model 发事件 -> 2) 桥接成 Bevy 消息 -> 3) 系统按需响应
fn on_hp_changed(mut reader: MessageReader<HpChangedMessage>, mut text: Query<&mut Text>) {
    for message in reader.read() {
        for mut t in &mut text { **t = message.hp.to_string(); }
    }
}
```

对 UI 初始化，用 `register_with_init_value` 一次搞定「初值 + 后续变化」：

```rust
let un = self.get_model::<PlayerModel>()
    .hp
    .register_with_init_value(|hp| { /* 更新 UI */ });
self.subscriptions.lock().unwrap().add(un);
```

### 6.4 每帧一次的控制器里别做全量重算

```rust
impl QControllerUpdate for BoardController {
    fn update(&self, _delta: Duration) {
        // ❌ 每帧把整个棋盘重算一遍
        self.rebuild_board();

        // ✓ 只在脏标记为真时重算
        if self.dirty.swap(false, Ordering::SeqCst) {
            self.rebuild_board();
        }
    }
}
```

脏标记由事件订阅者设置——这就是「事件驱动」的正确用法。

### 6.5 事件订阅数量

每次 `send_event` 都会克隆一遍订阅者列表（`Arc` 自增），然后逐个调用。订阅者数量在几十的量级完全没问题，但不要用它做「每帧上万次发送 + 上百订阅者」的事。

---

## 七、订阅生命周期

### 7.1 一定要注销

回调对捕获的环境是**强引用**。不注销 = 内存泄漏。

```rust
#[derive(Default, IController)]
struct HudController {
    arch: ArchRef,
    subscriptions: Mutex<IUnRegisterList>,
}

impl HudController {
    fn start(&self) {
        let mut subs = self.subscriptions.lock().unwrap();
        subs.add(self.get_model::<PlayerModel>().hp.register(|_| { /* ... */ }));
        subs.add(self.register_event::<DamageTakenEvent, _>(|_| { /* ... */ }));
    }
}
// HudController 析构 -> Mutex 析构 -> IUnRegisterList 析构 -> 自动注销
```

`IUnRegisterList` 在 `Drop` 时自动调用 `unregister_all()`，所以「把句柄放进列表」就等价于「自动清理」。

### 7.2 常量引用要小心

```rust
// ❌ 闭包捕获了 hp_text 的 &mut 借用，生命周期对不上（编译不过，这是好事）
self.get_model::<PlayerModel>().hp.register(|hp| {
    hp_text.text = hp.to_string();
});
```

回调必须是 `Fn(&T) + Send + Sync + 'static`，不能捕获非 `'static` 的借用。正确做法是捕获 `Arc<Mutex<...>>` 或（在 Bevy 里）用消息桥接让真正的系统去改 UI。

### 7.3 一次性订阅

```rust
// 只关心第一次触发
let un = architecture.register_event::<GameStartedEvent, _>(|_| { /* ... */ });
// 在回调内部拿不到 un（自引用），用共享的 Cell / 在回调外注销
```

需要在回调内注销自己时，用 `Arc<Mutex<Option<IUnRegister>>>` 中转：

```rust
let slot: Arc<Mutex<Option<IUnRegister>>> = Arc::new(Mutex::new(None));
let slot_in_cb = Arc::clone(&slot);
let un = architecture.register_event::<GameStartedEvent, _>(move |_| {
    if let Some(un) = slot_in_cb.lock().unwrap().take() {
        un.unregister();
    }
});
*slot.lock().unwrap() = Some(un);
```

---

## 八、测试策略

### 8.1 三层测试

| 层次 | 工具 | 覆盖什么 | 占比 |
|---|---|---|---|
| 纯逻辑测试 | `ArchitectureBuilder` + `send_command` | 规则、数值、状态流转 | ~80% |
| Bevy 集成测试 | `MinimalPlugins` + `app.update()` | 桥接、控制器、系统排序 | ~15% |
| 端到端 | 手动 / 录屏 | 手感、表现 | ~5% |

**大部分测试不应该依赖 Bevy。** 这是把架构和引擎解耦的最大回报：

```rust
#[test]
fn purchase_fails_when_not_enough_gold() {
    let architecture = ArchitectureBuilder::new("Test")
        .model(ShopModel::default())
        .model(PlayerModel::default())
        .build();

    let result = architecture.send_command(PurchaseItemCommand { item_id: 1, quantity: 1 });

    assert!(matches!(result, Err(ShopError::NotEnoughCurrency)));
    assert_eq!(architecture.send_query(GetGoldQuery), 0);
}
```

### 8.2 给每个测试一个干净的架构

不要复用架构实例——`ArchitectureBuilder::build()` 很便宜（几个 `Arc` 分配 + HashMap 插入），每个测试新建一个。

### 8.3 测事件用「记录 + 断言」

```rust
#[test]
fn item_purchased_event_is_emitted() {
    let architecture = ArchitectureBuilder::new("Test")
        .model(ShopModel::default())
        .build();

    let received = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&received);
    let _un = architecture.register_event::<ItemPurchasedEvent, _>(move |e| {
        sink.lock().unwrap().push(e.item_id);
    });

    architecture.send_command(PurchaseItemCommand { item_id: 7, quantity: 1 });

    assert_eq!(*received.lock().unwrap(), vec![7]);
}
```

### 8.4 测 Bevy 集成

见 [Bevy 集成 · 无窗口测试](bevy.md#八无窗口测试)。

---

## 九、命名与文件组织

### 9.1 命名约定

| 类型 | 约定 | 例子 |
|---|---|---|
| Model | `XxxModel` | `PlayerModel`、`InventoryModel` |
| System | `XxxSystem` | `BattleSystem`、`AchievementSystem` |
| Controller | `XxxController` | `HudController`、`InputController` |
| Utility | `XxxUtility` | `SaveUtility`、`PlatformUtility` |
| Command | 动词 + 名词 + `Command` | `PurchaseItemCommand`、`StartBattleCommand` |
| Query | `Get` + 名词 + `Query` | `GetGoldQuery`、`GetInventoryQuery` |
| Event | 名词 + 过去式/`Changed` + `Event` | `ItemPurchasedEvent`、`HpChangedEvent` |

### 9.2 目录组织

```
src/
├── main.rs
├── app.rs                # QApplication 实现：所有注册集中在一处
├── model/
│   ├── mod.rs
│   ├── player.rs         # PlayerModel + 它的事件
│   └── inventory.rs
├── system/
│   └── achievement.rs
├── command/
│   ├── mod.rs
│   └── shop.rs           # 按功能域聚合，不按类型拆
├── query/
│   └── inventory.rs
├── controller/
│   └── hud.rs
├── utility/
│   └── save.rs
└── event.rs              # 跨域事件集中定义（可选）
```

把所有注册集中在 `app.rs` 的好处是——**看一个文件就知道整个游戏有哪些模块**：

```rust
impl QApplication for MyGame {
    fn build() -> ArchitectureBuilder {
        ArchitectureBuilder::new("MyGame")
            // 工具层
            .utility(SaveUtility)
            .utility(PlatformUtility)
            // 数据层
            .model(PlayerModel::default())
            .model(InventoryModel::default())
            .model(ShopModel::default())
            // 业务逻辑层
            .system(AchievementSystem::default())
            .system(BattleSystem::default())
    }
}
```

注意 `.model(A).model(B)` 保证 A 先于 B 初始化。

### 9.3 事件定义放哪

两种风格都可以，选一种并保持一致：

- **集中在 `event.rs`**：一眼看到全局通信拓扑，适合事件较少的项目。
- **跟 Model 放一起**：内聚性更高，适合模块化明显的项目。

---

## 十、反模式清单

| 反模式 | 问题 | 正确做法 |
|---|---|---|
| Controller 直接改 Model（跳过 Command） | 状态变更失控 | 一律 `send_command` |
| Model 里调 `get_system` | **编译不过** | 把逻辑挪到 Command / System |
| Model 里调另一个 Model | **编译不过** | 在 Command 里协调 |
| Command 持有 `Arc<Model>` 等状态 | 无法序列化/回放，生命周期混乱 | 只保留输入参数 |
| Query 里改状态 | 破坏只读契约，调试困难 | 用 Command |
| `get()` + `set()` 做自增 | **并发丢更新** | `modify()` |
| 在 `modify` 闭包里读同一属性 | **死锁** | 闭包外读 |
| 订阅后不注销 | 内存泄漏 | `IUnRegisterList` |
| 用事件当状态查询 | 晚订阅者收不到 | `BindableProperty` |
| 用事件做请求-响应 | 拿不到结果 | Query / 直接 `get_model` |
| 在事件回调里读文件 / 发网络请求 | 阻塞发送方 | 交给 Command 或 Bevy 异步任务 |
| 每帧轮询 Model 刷新 UI | 无谓开销 | 事件驱动 + 脏标记 |
| 一次命令里连续 `set` 多个属性 | 多次通知 | 聚合属性 / 静默写 + 一次通知 |
| 把整个大 `Vec` / `HashMap` 拷出来只看一个字段 | 无谓拷贝 | `with_value` / `with_items` / `with_entries` |
| Utility 里访问游戏数据 | **编译不过** | 由调用方把数据传进去 |

---

## 下一步

- 把这些约定落到代码里 → [`examples/mini_game`](../examples/mini_game/README.md)（六种角色全部参与的完整示例）
- 遇到报错 → [排错手册](troubleshooting.md)
- 查具体 API → [核心概念](core-concepts.md)
- Bevy 相关细节 → [Bevy 集成](bevy.md)
