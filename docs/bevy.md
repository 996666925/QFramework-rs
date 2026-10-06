# Bevy 集成

`qframework-bevy` 把业务架构接入 Bevy 0.19。它只做三件事：

1. 把架构装成 Bevy 资源 `QArchitecture`；
2. 把 QFramework 事件桥接成 Bevy 消息；
3. 清理架构（由插件的 `cleanup` 钩子执行）。

表现层直接使用 Bevy 系统，输入、场景节点和 UI 通过 `Query`、`Commands`、Resource 访问。
核心在单线程运行，`QArchitecture` 和桥接队列作为 `NonSend` 数据安装，访问它们的系统在主线程执行。

---

## 一、安装

```toml
[dependencies]
bevy = "0.19"
qframework-core = "0.1"
qframework-bevy = "0.1"
```

`qframework-bevy` 只为 bevy 打开 `std` feature（headless 所需的最小集合）。Bevy 的其它 feature 由你自己的 `bevy` 依赖决定，Cargo 会做 feature 合并，不会冲突。

---

## 二、定义架构

一个类型描述「注册了哪些层对象」，用 `QApplication` 表达：

```rust
use bevy::prelude::*;
use qframework_bevy::prelude::*;

struct MyGame;

impl QApplication for MyGame {
    fn build() -> ArchitectureBuilder {
        ArchitectureBuilder::new("MyGame")
            .utility(SaveUtility)
            .model(PlayerModel::default())
            .model(InventoryModel::default())
            .system(AchievementSystem::default())
    }
}
```

注意 `build()` 是**关联函数**，没有 `self`——你不需要实例化这个类型，它只是架构的「型别标签」。

---

## 三、安装插件

```rust
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .install_architecture::<MyGame>()         // 安装架构（必须最先）
        .bridge_messages::<HpChangedMessage>()    // 可选：事件桥接
        .add_systems(Update, sync_camera)
        .run();
}
```

顺序要求：

| 调用 | 要求 |
|---|---|
| `install_architecture` | 每个 App 只能调用一次（第二次会 panic，避免架构被静默覆盖） |
| `bridge_messages` | 必须在 `install_architecture` **之后** |

也可以直接 `.add_plugins(QFrameworkPlugin::<MyGame>::default())`，效果相同。

### 插件做的事

```rust
fn build(&self, app: &mut App) {
    let architecture = A::build().build();      // 注册 + 两阶段初始化
    app.insert_non_send(QArchitecture(architecture));
}

fn cleanup(&self, app: &mut App) {
    architecture.deinit();                       // System / Model deinit + 清空
}
```

`cleanup` 是 Bevy 的插件生命周期钩子。手动推进帧的无窗口示例在使用完架构后显式调用 `app.cleanup()`；若架构需要贯穿运行循环，应由应用在结束时安排清理。

`QApplication` 来自核心 crate，此处重导出以保持已有用法。插件通过 `A::build().build()`
创建本世界的独立架构，多个 Bevy App 不共享状态。`A::interface()` 是另一份按应用类型
共享的实例；Bevy 系统继续通过 `NonSend<QArchitecture>` 访问本世界的数据。
需要让 Godot 节点访问同一 Bevy 世界时，显式注入 `app.architecture()`。

---

## 四、在系统里访问架构

```rust
use bevy::prelude::*;
use qframework_bevy::prelude::*;

fn send_damage(
    keyboard: Res<ButtonInput<KeyCode>>,
    architecture: NonSend<QArchitecture>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        architecture.send_command(TakeDamageCommand { amount: 10 });
    }
}

fn read_state(architecture: NonSend<QArchitecture>) {
    let hp = architecture.get_model::<PlayerModel>().hp.get();
    println!("HP = {hp}");
}
```

`QArchitecture` 实现了 `Deref<Target = Architecture>`，所以**在它上面可以直接调用 `Architecture` 的全部方法**——`send_command`、`send_query`、`get_model`、`register_event` 都能用。

需要在当前线程把架构句柄保存到其他位置时：

```rust
let handle: Rc<Architecture> = architecture.rc();   // 或 QArchitecture::rc(&architecture)
```

---

## 五、表现层使用 Bevy 系统

输入系统通过 Command 改变业务状态，展示系统通过 Query 或消息读取变化。
场景节点由 Bevy 管理，系统可以直接查询节点的组件：

```rust
#[derive(Component, Default)]
struct DisplayedCount(i32);

fn tick_counter(architecture: NonSend<QArchitecture>, mut frames: Local<u32>) {
    if (*frames).is_multiple_of(60) {
        architecture.send_command(IncreaseCountCommand);
    }
    *frames += 1;
}

fn refresh_scene(
    architecture: NonSend<QArchitecture>,
    mut nodes: Query<&mut DisplayedCount>,
) {
    let count = architecture.get_model::<CounterModel>().count.get();
    for mut node in &mut nodes {
        node.0 = count;
    }
}

app.world_mut().spawn(DisplayedCount::default());
app.add_systems(Update, (tick_counter, refresh_scene).chain());
```

`Local` 保存单个系统的跨帧状态；共享的表现状态使用 Resource。
`.chain()` 保证输入逻辑先于展示刷新；也可以用 `.before(...)`、`.after(...)` 或自定义 `SystemSet` 排序。
`NonSend<QArchitecture>` 保证相关系统在主线程运行，但不保证业务顺序，需要先后顺序时仍应显式排序。

### 订阅生命周期

优先用 `bridge_messages` 与 `MessageReader` 响应业务事件。
直接订阅可绑定属性时，把 `IUnRegisterList` 保存在 Resource 或 Component 中，
它随资源移除或实体销毁而析构，自动注销订阅。回调可以设置共享脏标记，实际 UI 修改在 Bevy 系统中执行。
完整示例见 `examples/mini_game/src/controller/hud.rs`。

核心库的 `IController` 仍可用于非 Bevy 场景；Controller 生命周期由宿主管理，Bevy 中直接使用系统生命周期。

---

## 六、消息桥接

### 为什么需要它

QFramework 的事件可能在 **Model / Command / System** 里被触发，桥接插件在回调和 Bevy 消息写入系统之间加了一个单线程队列：

```
QFramework 事件（任意位置触发）
        │  handler（注册在架构事件总线上）
        ▼
   Rc<RefCell<Vec<M>>> 队列
        │  forward_messages（PreUpdate）
        ▼
   Bevy Messages<M>
        │  MessageReader<M>（Update）
        ▼
   你的 Bevy 系统
```

### 用法

同一个类型既是 QFramework 事件，也是 Bevy 消息：

```rust
// 1. 定义（注意 Clone 是必须的）
#[derive(Message, Clone, Debug)]
struct HpChangedMessage { hp: i32 }

// 2. 在 Model 里作为 QFramework 事件发出
impl PlayerModel {
    pub fn take_damage(&self, amount: i32) {
        self.hp.modify(|hp| *hp = (*hp - amount).max(0));
        self.send_event(HpChangedMessage { hp: self.hp.get() });
    }
}

// 3. 插件里注册桥接
app.install_architecture::<MyGame>()
   .bridge_messages::<HpChangedMessage>();

// 4. 在任意 Bevy 系统里读取
fn on_hp_changed(mut reader: MessageReader<HpChangedMessage>) {
    for message in reader.read() {
        println!("HP -> {}", message.hp);
    }
}
```

### 时序：延迟一帧

事件在 `Update` 发出时，队列在**下一帧**的 `PreUpdate` 才被转发：

| 帧 | 阶段 | 发生的事 |
|---|---|---|
| N | Update | Model 发事件 → 入队 |
| N+1 | PreUpdate | `forward_messages` → 写入 `Messages<M>` |
| N+1 | Update | `MessageReader` 读到 |

也就是说**从发送到被 Bevy 系统读到，中间隔了一帧**。做表现层时通常无所谓；做帧同步 / 精确回放时要注意这个偏移。

### 什么时候不需要桥接

如果只是想「在 QFramework 内部响应」，直接注册 handler 就行，不需要桥接：

```rust
architecture.register_event::<HpChangedMessage, _>(|event| { /* ... */ });
```

桥接只在**需要 Bevy 系统（渲染、音频、UI）感知事件**时才需要。

### 手动访问队列

```rust
fn peek(bridge: NonSend<QEventBridge<HpChangedMessage>>) {
    println!("积压 {}", bridge.len());
    // bridge.push(msg) / bridge.drain() / bridge.queue() 也可用
}
```

---

## 七、与 Bevy 状态机配合

```rust
#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
enum GameState {
    #[default]
    Menu,
    Playing,
}

app.init_state::<GameState>()
   .add_systems(OnEnter(GameState::Playing), start_battle)
   .add_systems(OnExit(GameState::Playing), end_battle);

fn start_battle(architecture: NonSend<QArchitecture>) {
    architecture.send_command(StartBattleCommand);
}

fn end_battle(architecture: NonSend<QArchitecture>) {
    architecture.send_command(EndBattleCommand);
}
```

状态切换本身是「输入」，影响的是应用状态，所以走 Command。

---

## 八、无窗口测试

QFramework 的核心不依赖 Bevy，**大部分逻辑不需要 Bevy 就能测**：

```rust
#[test]
fn gold_decreases_after_purchase() {
    let architecture = ArchitectureBuilder::new("Test")
        .model(ShopModel::default())
        .build();

    architecture.send_command(PurchaseItemCommand { item_id: 1, quantity: 2 });
    assert_eq!(architecture.send_query(GetGoldQuery), 80);
}
```

需要验证 Bevy 侧行为时，用 `MinimalPlugins` + 手动 `update()`，不需要窗口：

```rust
#[test]
fn systems_run_every_frame() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)          // 包含 Time / TaskPool / FrameCount
       .install_architecture::<MyGame>()
       .add_systems(Update, tick_counter);

    app.update();
    app.update();

    assert_eq!(app.architecture().get_model::<CounterModel>().count.get(), 1);
}
```

要点：

- `MinimalPlugins` 提供 `TimePlugin`，系统可通过 `Res<Time>` 获取帧间隔。
- **不要调用 `app.run()`**（会进入无限循环）；用 `app.update()` 手动推进帧。
- 验证 `deinit` 行为可以直接调用 `app.cleanup()`，它会触发 `Plugin::cleanup`。
- 想断言 Bevy 消息，读写之间要隔一次 `update()`（见上面的时序表）。

`qframework-bevy` 自己的 `tests/integration.rs` 就是这套写法，可以直接参考。

---

## 九、常见问题

**`QEventBridgePlugin 必须在 QFrameworkPlugin 之后添加`**

调用顺序反了。把 `.install_architecture::<A>()` 放在 `.bridge_messages::<M>()` 前面。

**`只能安装一个 QFrameworkPlugin`**

同一个 App 里添加了两次 `QFrameworkPlugin`（即使架构类型不同）。Bevy 不会帮你拦——不同泛型参数是不同类型。

**`未找到 QArchitecture 资源：请先调用 App::install_architecture::<A>()`**

在装插件之前就用了 `NonSend<QArchitecture>` 或 `app.architecture()`。

**消息读不到**

按顺序检查：① 事件类型 derive 了 `Message` 吗；② 事件真的被发出了吗（发送时有没有订阅者不影响桥接，桥接本身就是订阅者）；③ 是不是在同一帧读的——需要等一帧。

**`NonSend<QArchitecture>` 和别的系统冲突**

`NonSend<QArchitecture>` 与 `NonSendMut<QArchitecture>` 不能在同一个系统中同时借用。
架构存储在 Bevy 的 NonSend 数据区，普通实体查询不访问它。
宽泛查询与普通 `Resource` 的冲突仍按 Bevy 规则处理。

---

## 下一步

- 写业务代码时的推荐做法 → [最佳实践](best-practices.md)
- 遇到报错 → [排错手册](troubleshooting.md)
