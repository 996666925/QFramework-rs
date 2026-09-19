# 排错手册

按症状查。每一条都给出「为什么会这样」和「怎么改」。

---

## 一、编译错误

### E0599: `no method named 'get_system' found for reference '&XxxModel'`

**这不是 bug，是分层规则在起作用。** 你正在用一个没有该能力的层去访问受限资源。

| 你在用 | 想做的事 | 结果 |
|---|---|---|
| `IModel` | 取其它 Model | 编译失败 |
| `IModel` | 取 System | 编译失败 |
| `IModel` | 发 Command / Query | 编译失败 |
| `IController` | 发事件 | 编译失败 |
| `ISystem` | 发 Command / Query | 编译失败 |
| `IUtility` | 取任何东西 | 编译失败 |

编译器的提示会直接告诉你缺哪个 trait：

```
= note: the following trait defines an item `get_system`, perhaps you need to implement it:
        candidate #1: `qframework_core::ICanGetSystem`
```

**怎么改**：不要在层里绕过去，而是把这段逻辑挪到有权限的地方。

```rust
// 需求：Model 在保存时想读配置
// ❌ Model 拿不到别的 Model
impl IModel for PlayerModel {
    fn save(&self) { self.get_model::<ConfigModel>(); }
}

// ✓ 方案 A：让 Command 把配置读出来传给 Model
impl ICommand for SaveCommand {
    fn execute(&self, ctx: &CommandContext) {
        let config = ctx.get_model::<ConfigModel>();
        ctx.get_model::<PlayerModel>().save(&config);
    }
}

// ✓ 方案 B：把配置封装进 Utility（Utility 可以被 Model 取到）
impl IModel for PlayerModel {
    fn save(&self) {
        let config = self.get_utility::<ConfigUtility>();
        // ...
    }
}
```

### `#[derive(IModel)] 找不到架构引用字段`

```
error: #[derive(IModel)] 找不到架构引用字段：请添加 `arch: ArchRef`，或用 `#[arch]` 标记已有字段
```

Controller / System / Model 需要一个 `ArchRef` 字段。加上即可：

```rust
#[derive(Default, IModel)]
struct PlayerModel {
    arch: ArchRef,                 // ← 补上这行
    pub hp: BindableProperty<i32>,
}
```

字段名不叫 `arch` 时用 `#[arch]` 标注：

```rust
#[derive(Default, IModel)]
struct PlayerModel {
    #[arch]
    ctx: ArchRef,
    pub hp: BindableProperty<i32>,
}
```

（`IUtility` 不需要这个字段。）

### `#[derive(IModel)] 只能用于结构体` / `需要具名字段`

派生宏不支持枚举、union、元组结构体。改成具名字段的结构体：

```rust
// ❌
#[derive(IModel)]
struct PlayerModel(i32);

// ✓
#[derive(Default, IModel)]
struct PlayerModel {
    arch: ArchRef,
    pub hp: i32,
}
```

### conflicting implementations

**症状 A**：派生宏 + 手写 impl 冲突。

```
error[E0119]: conflicting implementations of trait `IModel` for type `PlayerModel`
```

```rust
#[derive(IModel)]
struct PlayerModel { arch: ArchRef }

impl IModel for PlayerModel { }     // ❌ 已经有了
```

去掉其中一个。需要 `init` / `deinit` 就用辅助属性，不要手写整个 impl：

```rust
#[derive(IModel)]
#[model(init = Self::on_init)]
struct PlayerModel { arch: ArchRef }
```

**症状 B**：同一个类型 derive 了两个层宏。

```
error[E0119]: conflicting implementations of trait `HasArchRef` for type `PlayerModel`
```

一个类型只能是一个层。需要「既是 Model 又是 System」说明职责没拆干净。

### `the trait bound 'XxxModel: Send + Sync + 'static' is not satisfied`

层对象必须能跨线程共享（架构要放进 Bevy 的资源系统）。字段里出现了非 `Send`/`Sync` 的类型：

```rust
// ❌ Rc / RefCell / Cell 都不是 Sync
#[derive(Default, IModel)]
struct BadModel {
    arch: ArchRef,
    cache: Rc<RefCell<i32>>,
}

// ✓
#[derive(Default, IModel)]
struct GoodModel {
    arch: ArchRef,
    cache: Arc<Mutex<i32>>,      // 或 AtomicI32
}
```

同理，用了 `Rc` 的泛型参数也会触发这条错误。

### `cannot find derive macro 'IModel' in this scope`

没引入 prelude。加上：

```rust
use qframework_core::prelude::*;   // 同时引入 trait 和派生宏
```

### `the trait bound 'PurchaseItemCommand: ICommand' is not satisfied`

多半是漏写了关联类型：

```rust
impl ICommand for PurchaseItemCommand {
    type Output = ();        // ← 无返回值也要写
    fn execute(&self, ctx: &CommandContext) { }
}
```

同理 `IQuery` 需要 `type Result = ...;`。

### `borrowed value does not live long enough`（在事件回调里）

回调必须是 `Fn(&T) + Send + Sync + 'static`，不能捕获栈上的借用：

```rust
// ❌ hp_text 是局部借用
let mut hp_text = String::new();
architecture.register_event::<HpChangedEvent, _>(|e| { hp_text = e.hp.to_string(); });

// ✓ 捕获 Arc<Mutex<..>>
let hp_text = Arc::new(Mutex::new(String::new()));
let sink = Arc::clone(&hp_text);
architecture.register_event::<HpChangedEvent, _>(move |e| {
    *sink.lock().unwrap() = e.hp.to_string();
});
```

在 Bevy 里更好的做法是走[消息桥接](bevy.md#六消息桥接)，让真正的渲染系统去改 UI。

---

## 二、运行时 panic

### `ArchRef 尚未绑定 Architecture：请通过 ArchitectureBuilder 注册该对象`

层对象被用在了没有被注册的地方。

```rust
// ❌ 手动 new 出来的 Model，没经过 builder
let model = PlayerModel::default();
model.get_utility::<SaveUtility>();      // panic

// ✓
let architecture = ArchitectureBuilder::new("App")
    .model(PlayerModel::default())
    .build();
architecture.get_model::<PlayerModel>().get_utility::<SaveUtility>();
```

`ArchRef` 保存的是 `Weak<Architecture>`，所以架构已经被 `deinit` 或整体析构后，`get()` 同样会 panic。用 `try_get()` 可以安全探测：

```rust
if let Some(architecture) = self.arch.try_get() {
    // ...
}
```

### `类型 'my_game::PlayerModel' 尚未注册到 IOCContainer`

忘了在 builder 里注册，或者类型写错了（比如注册的是 `PlayerModel`，取的是 `PlayerStateModel`）。

```rust
let architecture = ArchitectureBuilder::new("App")
    .model(PlayerModel::default())        // ← 检查这里
    .build();

architecture.get_model::<PlayerModel>();  // 对上才行
```

用 `try_get_model` 可以在不确定时避免 panic。

### `Architecture 必须通过 ArchitectureBuilder::build 创建`

`Architecture::arc()` 依赖 `build()` 阶段写入的自引用。不要试图自己构造架构——构造函数是私有的，正常也构造不出来。如果你看到这个 panic，说明有地方绕过了 builder。

### `PoisonError` / 后续访问全部 panic

之前有线程在**持有锁的情况下** panic 了，锁被标记为中毒。

最可能的来源是 `modify` / `with_value` 的闭包里 panic：

```rust
// ❌ 闭包里的 unwrap 一旦失败，这个属性之后永久不可用
model.hp.modify(|hp| *hp = data.get("hp").unwrap().parse().unwrap());
```

**不要在锁内做可能 panic 的操作。** 先在闭包外算好：

```rust
// ✓ 在外面算，算不出来就提前返回
let Some(hp) = data.get("hp").and_then(|v| v.parse().ok()) else { return; };
model.hp.modify(|current| *current = hp);
```

---

## 三、死锁（程序卡住不动）

`std::sync::RwLock` **不可重入**。在持有某个锁的时候再去获取同一个锁，线程会永久阻塞。

### 症状：调用 `modify` / `with_value` 之后程序卡死

```rust
// ❌ 在写锁里读同一个属性 → 死锁
model.hp.modify(|hp| {
    *hp -= 10;
    let current = model.hp.get();      // 卡在这里
});

// ✓
model.hp.modify(|hp| *hp -= 10);
let current = model.hp.get();
```

### 症状：`Display` / `Debug` 输出时卡死

`BindableProperty` 的 `Display` / `Debug` / `PartialEq` 都会读锁，所以它们同样不能出现在锁内：

```rust
// ❌
model.hp.modify(|hp| {
    *hp -= 10;
    println!("{}", model.hp);          // 卡住
});
```

### 安全的部分

访问**不同的**属性是安全的（锁顺序一致，不会形成环）：

```rust
// ✓
model.gold.modify(|gold| {
    let price = model.price.get();     // 另一个属性
    *gold -= price;
});
```

### 排查方法

1. 搜索所有 `modify(` / `with_value(` / `with_items(` / `with_entries(` 的闭包体。
2. 检查闭包里有没有出现**同一个**属性、或同一集合的读取方法。
3. 检查闭包里有没有调用外部函数——那个函数如果回过来碰了同一个属性也会死锁。

---

## 四、事件相关问题

### 事件收不到

按顺序检查：

1. **注册和发送的类型是不是同一个？**（最容易犯）
   ```rust
   architecture.register_event::<HpChangedEvent, _>(|_| {});
   architecture.send_event(HpChangedEvent { hp: 1 });      // ✓
   architecture.send_event(PlayerHpChangedEvent { hp: 1 }); // ✗ 不同类型
   ```

2. **注册是不是发生在发送之后？** 事件**不缓冲**，发送时没有订阅者就被丢弃。
   ```rust
   architecture.send_event(GameStartedEvent);                        // 丢弃
   architecture.register_event::<GameStartedEvent, _>(|_| {});       // 太晚了
   ```

3. **是不是被注销了？** `IUnRegister` 被 `unregister()`，或者持有它的 `IUnRegisterList` 已经被 `Drop`。

4. **是不是在 `deinit()` 之后发的？** `deinit` 会 `events.clear()`。

5. **`send_event` 是从哪个架构实例发的？** 有两个架构实例时，事件不会跨架构传播。跨架构请用 `TypeEventSystem::global()`。

用 `has_listener` 快速确认：

```rust
println!("有人订阅吗: {}", architecture.events().has_listener::<HpChangedEvent>());
```

### 事件被处理了两次

`register_with_init_value` 在并发写入时可能重复回调一次（这是**故意的**：宁可重复不可丢失）。如果重复有害，在回调里做幂等判断。

### 事件顺序和预期不一致

- 同一类型内的顺序 = **注册顺序**。
- 跨类型没有顺序保证。
- 集合（`BindableList` / `BindableDictionary`）的事件在多线程并发写入时顺序不保证与写入顺序一致。

---

## 五、Bevy 相关问题

### `QEventBridgePlugin 必须在 QFrameworkPlugin 之后添加`

```rust
// ❌
app.bridge_q_messages::<HpChangedMessage>()
   .add_qframework::<MyGame>();

// ✓
app.add_qframework::<MyGame>()
   .bridge_q_messages::<HpChangedMessage>();
```

### `只能安装一个 QFrameworkPlugin`

同一个 App 里装了两次。Bevy 的唯一性检查拦不住不同泛型参数的插件类型：

```rust
// ❌
app.add_qframework::<GameA>()
   .add_qframework::<GameB>();
```

把两个架构合并成一个，或者把其中一个作为子 App。

### `未找到 QArchitecture 资源：请先调用 App::add_qframework::<A>()`

在装插件之前用了 `Res<QArchitecture>`、`q_architecture()` 或 `add_q_controller()`。

### `QFrameworkSet` 相关的排序报错

`QFrameworkSet::Controllers` 是由 `QFrameworkPlugin` 注册的。如果排序用到了它但插件还没装，Bevy 会报找不到集合。确保 `.add_qframework::<A>()` 在 `.add_systems(..)` 之前。

### 消息读不到 / 慢一帧

这是设计如此——事件在 `Update` 发出，下一帧 `PreUpdate` 才进入 Bevy 消息队列。如果必须在同帧响应，就不要走桥接，直接在 QFramework 侧注册 handler。

### 控制器不更新

1. 是不是用 `add_q_controller` 注册的？（手动 `attach_controller` 不会进 `QControllers`，也就不会被驱动）
2. 实现了 `QControllerUpdate` 吗？
3. 集合被 `run_if` 条件拦了吗？
4. 用 `app.world().resource::<QControllers>().len()` 确认注册成功。

### `Res<QArchitecture>` 报访问冲突

Bevy 0.19 里资源存储在单例实体上，宽泛查询会与之冲突：

```rust
// ❌ 和资源访问冲突
fn bad(query: Query<EntityMut>, architecture: Res<QArchitecture>) {}

// ✓ 用 Without 排除资源实体
fn good(query: Query<EntityMut, Without<IsResource>>, architecture: Res<QArchitecture>) {}
```

---

## 六、行为不符合预期

### 订阅者收到的是别人写的值

并发场景下，`send_event` 的参数是**发送方自己写入的值**（不是「读取时的最新值」）。所以可能出现：

- 线程 A 写 5，通知 5
- 线程 B 写 6，通知 6
- 但顺序可能是 B 的 6 先到，A 的 5 后到

如果消费方需要最终一致，就在回调里重新读一次：

```rust
model.hp.register(|_| {
    let current = model.hp.get();   // 读最新值，而不是用事件参数
});
```

### 计数器少加了

八成是 `get()` + `set()` 而不是 `modify()`。见[最佳实践 · 并发](best-practices.md#三并发相关最重要的一节)。

### `deinit()` 之后一切 panic

`deinit` 是终态操作。清空 IOC 后 `get_model` / `get_system` / `get_utility` 都会 panic，`is_inited()` 变回 `false`。架构不可复用，需要新架构就重新 `build()` 一个。

---

## 七、自查清单

遇到问题先做这几件事：

```rust
// 1. 打印架构状态
println!("{:?}", architecture);
// Architecture { name: "MyGame", inited: true, registered: 5 }

// 2. 确认类型注册了
println!("{:?}", architecture.try_get_model::<PlayerModel>().is_some());

// 3. 确认事件有订阅者
println!("{}", architecture.events().has_listener::<HpChangedEvent>());

// 4. 确认控制器注册了（Bevy）
println!("{}", app.world().resource::<QControllers>().len());
```

按「数据从哪来 → 谁改了它 → 谁该收到通知」这条链**从后往前**排查，通常比从前往后快。

---

## 下一步

- 推荐写法 → [最佳实践](best-practices.md)
- API 细节 → [核心概念](core-concepts.md)
