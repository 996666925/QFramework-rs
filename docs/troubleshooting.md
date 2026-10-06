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

### 层对象的生命周期约束
层对象必须满足 `'static`，不能保存借用局部变量的字段。使用拥有所有权的数据或 `Rc`。
`Rc`、`RefCell`、`Cell` 可以直接放入 Model / System / Utility，框架不要求线程安全。
Bevy 使用 `NonSend<QArchitecture>`，不要把架构包装为普通 `Resource`。

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

回调必须是 `FnMut(&T) + 'static`，不能捕获栈上的借用：

```rust
// ❌ hp_text 是局部借用
let mut hp_text = String::new();
architecture.register_event::<HpChangedEvent, _>(|e| { hp_text = e.hp.to_string(); });

// ✓ 捕获 Rc<RefCell<..>>
let hp_text = Rc::new(RefCell::new(String::new()));
let sink = Rc::clone(&hp_text);
architecture.register_event::<HpChangedEvent, _>(move |e| {
    *sink.borrow_mut() = e.hp.to_string();
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
let architecture = ArchitectureBuilder::new()
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
let architecture = ArchitectureBuilder::new()
    .model(PlayerModel::default())        // ← 检查这里
    .build();

architecture.get_model::<PlayerModel>();  // 对上才行
```

用 `try_get_model` 可以在不确定时避免 panic。

### `Architecture 必须通过 ArchitectureBuilder::build 创建`

`Architecture::rc()` 依赖 `build()` 阶段写入的自引用。不要试图自己构造架构——构造函数是私有的，正常也构造不出来。如果你看到这个 panic，说明有地方绕过了 builder。

### 属性修改闭包发生 panic
`RefCell` 没有锁中毒状态，展开栈时会释放借用。已经写入的修改不会自动回滚，
中断的操作也可能尚未发送通知。先验证输入，再修改属性：
```rust
let Some(hp) = data.get("hp").and_then(|v| v.parse().ok()) else { return; };
model.hp.set(hp);
```

---

## 三、借用冲突
`modify` 持有可变借用，期间重新读取同一属性会 panic：
```rust
model.hp.modify(|hp| {
    *hp -= 10;
    let current = model.hp.get(); // 借用冲突
});
```
修改结束后再读即可。格式化、比较操作也会读取属性，不要放在同一属性的 `modify` 闭包里。
`with_value` / `with_items` / `with_entries` 只读闭包内可以读取相同数据，但不能修改它。

事件回调使用 `FnMut`。同步回调递归触发自身会发生借用冲突；调整通知链，避免循环触发。
Godot 回调不要重新借用正在发送命令的 Controller；直接捕获需要更新的展示节点。

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

`set` 不做相等判断；重复设置相同值也会通知。需要去重时在修改前比较，或在回调里做幂等判断。

### 事件顺序和预期不一致

- 同一类型内的顺序 = **注册顺序**。
- 跨类型没有顺序保证。
- 集合回调里继续修改集合会产生嵌套通知，避免循环触发同一回调。

---

## 五、Bevy 相关问题

### `QEventBridgePlugin 必须在 QFrameworkPlugin 之后添加`

```rust
// ❌
app.bridge_messages::<HpChangedMessage>()
   .install_architecture::<MyGame>();

// ✓
app.install_architecture::<MyGame>()
   .bridge_messages::<HpChangedMessage>();
```

### `只能安装一个 QFrameworkPlugin`

同一个 App 里装了两次。Bevy 的唯一性检查拦不住不同泛型参数的插件类型：

```rust
// ❌
app.install_architecture::<GameA>()
   .install_architecture::<GameB>();
```

把两个架构合并成一个，或者把其中一个作为子 App。

### `未找到 QArchitecture 资源：请先调用 App::install_architecture::<A>()`

在装插件之前用了 `NonSend<QArchitecture>` 或 `architecture()`。

### 消息读不到 / 慢一帧

这是设计如此——事件在 `Update` 发出，下一帧 `PreUpdate` 才进入 Bevy 消息队列。如果必须在同帧响应，就不要走桥接，直接在 QFramework 侧注册 handler。

### 表现层系统不更新

1. 是否通过 `add_systems(Update, ...)` 注册了系统？
2. 是否运行了帧循环（无窗口测试需调用 `app.update()`）？
3. 是否被 `run_if` 条件拦截？
4. 是否需要用 `.chain()` 或 `.before(...)` / `.after(...)` 明确执行顺序？

### 架构访问报 Resource 约束错误
`QArchitecture` 使用 `Rc`，不是普通 `Resource`。系统参数使用 `NonSend<QArchitecture>`，
World 读取使用 `world.non_send::<QArchitecture>()`。
同一系统不能同时使用 `NonSend<QArchitecture>` 和 `NonSendMut<QArchitecture>`。
普通实体查询与 NonSend 数据没有资源实体冲突。

---

## 六、行为不符合预期
### 通知值和最新状态不一致
事件携带发送时的值。回调可以触发其他状态修改，因此稍后读取 Model 时可能已经变化。
避免循环通知；只需要最新展示状态时，可以在更新阶段重新读取 Model。

### `deinit()` 之后一切 panic

`deinit` 是终态操作。清空 IOC 后 `get_model` / `get_system` / `get_utility` 都会 panic，`is_inited()` 变回 `false`。架构不可复用，需要新架构就重新 `build()` 一个。

---

## 七、自查清单

遇到问题先做这几件事：

```rust
// 1. 打印架构状态
println!("{:?}", architecture);
// Architecture { inited: true, registered: 5 }

// 2. 确认类型注册了
println!("{:?}", architecture.try_get_model::<PlayerModel>().is_some());

// 3. 确认事件有订阅者
println!("{}", architecture.events().has_listener::<HpChangedEvent>());

// 4. 确认架构资源已安装（Bevy）
println!("{}", app.world().contains_non_send::<QArchitecture>());
```

按「数据从哪来 → 谁改了它 → 谁该收到通知」这条链**从后往前**排查，通常比从前往后快。

---

## 下一步

- 推荐写法 → [最佳实践](best-practices.md)
- API 细节 → [核心概念](core-concepts.md)
