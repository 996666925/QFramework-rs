# Bevy 集成

`qframework-bevy` 把四层架构接入 Bevy 0.19。它只做四件事：

1. 把架构装成 Bevy 资源 `QArchitecture`；
2. 让 `IController` 能跟着帧循环跑；
3. 把 QFramework 事件桥接成 Bevy 消息；
4. 应用退出时自动清理。

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
        .add_qframework::<MyGame>()               // 安装架构（必须最先）
        .bridge_q_messages::<HpChangedMessage>()  // 可选：事件桥接
        .add_q_controller(HudController::default())// 可选：注册控制器
        .add_systems(Update, sync_camera)
        .run();
}
```

顺序要求：

| 调用 | 要求 |
|---|---|
| `add_qframework` | 每个 App 只能调用一次（第二次会 panic，避免架构被静默覆盖） |
| `bridge_q_messages` | 必须在 `add_qframework` **之后** |
| `add_q_controller` | 必须在 `add_qframework` **之后** |

也可以直接 `.add_plugins(QFrameworkPlugin::<MyGame>::default())`，效果相同。

### 插件做的事

```rust
fn build(&self, app: &mut App) {
    let architecture = A::build().build();      // 注册 + 两阶段初始化
    app.insert_resource(QArchitecture(architecture));
    app.init_resource::<QControllers>();
    app.add_systems(Update, run_controllers.in_set(QFrameworkSet::Controllers));
}

fn cleanup(&self, app: &mut App) {
    controllers.deinit_all();                    // 控制器 deinit
    architecture.deinit();                       // System / Model deinit + 清空
}
```

`cleanup` 由 Bevy 在事件循环结束时调用（`App::run()` 之后）。

---

## 四、在系统里访问架构

```rust
use bevy::prelude::*;
use qframework_bevy::prelude::*;

fn send_damage(
    keyboard: Res<ButtonInput<KeyCode>>,
    architecture: Res<QArchitecture>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        architecture.send_command(TakeDamageCommand { amount: 10 });
    }
}

fn read_state(architecture: Res<QArchitecture>) {
    let hp = architecture.get_model::<PlayerModel>().hp.get();
    println!("HP = {hp}");
}
```

`QArchitecture` 实现了 `Deref<Target = Architecture>`，所以**在它上面可以直接调用 `Architecture` 的全部方法**——`send_command`、`send_query`、`get_model`、`register_event` 都能用。

需要把架构带出系统（存到别处、发给另一个线程）时：

```rust
let handle: Arc<Architecture> = architecture.arc();   // 或 QArchitecture::arc(&q_arch)
```

---

## 五、控制器

QFramework 的 `IController` 在 Bevy 里变成一个「每帧更新的对象」。

### 定义

```rust
#[derive(Default, IController)]
#[controller(init = Self::start)]
struct HudController {
    arch: ArchRef,
    frames: AtomicI32,
    subscriptions: Mutex<IUnRegisterList>,
}

impl HudController {
    fn start(&self) {
        // 注册事件订阅：IController 可以监听，但不能发送事件
        let un = self.register_event::<HpChangedEvent, _>(|event| {
            println!("HP -> {}", event.hp);
        });
        self.subscriptions.lock().unwrap().add(un);
    }
}

impl QControllerUpdate for HudController {
    fn update(&self, delta: Duration) {
        // 每帧被调用一次
        if self.frames.fetch_add(1, Ordering::SeqCst) % 60 == 0 {
            self.send_command(TickCommand { delta });
        }
    }
}
```

注意 `update` 接收的是 `&self`——控制器以 `Arc` 共享，**可变状态必须用内部可变性**（`Atomic*`、`Mutex`、`BindableProperty`）。

### 注册

```rust
app.add_q_controller(HudController::default());
```

`add_q_controller` 会依次：

1. 调用 `Architecture::attach_controller` 注入架构引用；
2. 调用 `IController::init`（如果有 `#[controller(init = ...)]` 钩子）；
3. 把控制器放进 `QControllers` 资源，登记 `deinit` 回调。

### 调度

所有控制器在 `Update` 的 `QFrameworkSet::Controllers` 集合里更新：

```rust
// 让某个系统在控制器之后跑
app.add_systems(Update, refresh_ui.after(QFrameworkSet::Controllers));

// 某个系统必须最先跑
app.add_systems(Update, read_input.before(QFrameworkSet::Controllers));

// 只在特定状态下驱动控制器（需要 bevy_state feature）
app.configure_sets(Update, QFrameworkSet::Controllers.run_if(in_state(GameState::Playing)));
```

### 手动管理

不用 `add_q_controller` 也可以：

```rust
let architecture = app.world().resource::<QArchitecture>().arc();
let controller = architecture.attach_controller(MyController::default());
// 想怎么用就怎么用——不注册进 QControllers 就不会被每帧驱动
```

`QControllers` 的公开 API：

| 方法 | 说明 |
|---|---|
| `add::<C>(Arc<C>)` | 加入并立即 `init`（要求已绑定架构） |
| `len()` / `is_empty()` | 数量 |
| `iter()` | 遍历 `&Arc<dyn QControllerUpdate>` |
| `deinit_all()` | 依次调用 `IController::deinit` |
| `clear()` | 清空（**不会**调用 deinit） |

---

## 六、消息桥接

### 为什么需要它

QFramework 的事件可能在 **Model / Command / System** 里被触发，而 Bevy 的消息（`Message`）只能在**系统**里写入。桥接插件在两者之间加了一个线程安全队列：

```
QFramework 事件（任意位置触发）
        │  handler（注册在架构事件总线上）
        ▼
   Arc<Mutex<Vec<M>>> 队列
        │  forward_q_messages（PreUpdate）
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
app.add_qframework::<MyGame>()
   .bridge_q_messages::<HpChangedMessage>();

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
| N+1 | PreUpdate | `forward_q_messages` → 写入 `Messages<M>` |
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
fn peek(bridge: Res<QEventBridge<HpChangedMessage>>) {
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

fn start_battle(architecture: Res<QArchitecture>) {
    architecture.send_command(StartBattleCommand);
}

fn end_battle(architecture: Res<QArchitecture>) {
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
fn controllers_run_every_frame() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)          // 包含 Time / TaskPool / FrameCount
       .add_qframework::<MyGame>()
       .add_q_controller(TickController::default());

    app.update();
    app.update();

    assert_eq!(app.world().resource::<QControllers>().len(), 1);
}
```

要点：

- `MinimalPlugins` 提供 `TimePlugin`，所以控制器里的 `delta` 是可用的。
- **不要调用 `app.run()`**（会进入无限循环）；用 `app.update()` 手动推进帧。
- 验证 `deinit` 行为可以直接调用 `app.cleanup()`，它会触发 `Plugin::cleanup`。
- 想断言 Bevy 消息，读写之间要隔一次 `update()`（见上面的时序表）。

`qframework-bevy` 自己的 `tests/integration.rs` 就是这套写法，可以直接参考。

---

## 九、常见问题

**`QEventBridgePlugin 必须在 QFrameworkPlugin 之后添加`**

调用顺序反了。把 `.add_qframework::<A>()` 放在 `.bridge_q_messages::<M>()` 前面。

**`只能安装一个 QFrameworkPlugin`**

同一个 App 里添加了两次 `QFrameworkPlugin`（即使架构类型不同）。Bevy 不会帮你拦——不同泛型参数是不同类型。

**`未找到 QArchitecture 资源：请先调用 App::add_qframework::<A>()`**

在装插件之前就用了 `Res<QArchitecture>` 或 `add_q_controller`。

**消息读不到**

按顺序检查：① 事件类型 derive 了 `Message` 吗；② 事件真的被发出了吗（发送时有没有订阅者不影响桥接，桥接本身就是订阅者）；③ 是不是在同一帧读的——需要等一帧。

**`Res<QArchitecture>` 和别的系统冲突**

`Res<T>` 是共享只读借用，本身不会冲突。真正冲突的话，通常是同时借了 `ResMut<QControllers>` 或某个宽泛查询（Bevy 0.19 里 `Query<Entity>` 之类的宽泛查询会和资源访问冲突，需要加 `Without<IsResource>`）。

---

## 下一步

- 写业务代码时的推荐做法 → [最佳实践](best-practices.md)
- 遇到报错 → [排错手册](troubleshooting.md)
