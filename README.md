# QFramework for Rust

**仓库地址**：<https://github.com/996666925/QFramework-rs>

[QFramework](https://github.com/liangxiegame/QFramework) 的 Rust 实现，专为 [Bevy](https://bevy.org) 打造。

保留了 QFramework 最核心的设计——**四层架构 + 编译期依赖约束 + CQRS + 事件驱动 + 可绑定属性**，
并用 Rust 的 trait 系统把「谁能访问谁」变成**编译错误**，而不是靠开发者自觉。

- **兼容 Bevy 最新版（0.19.x）**
- **Cargo workspace 结构**：架构核心与引擎解耦
- **运行时零外部依赖**：`qframework-core` 只用标准库；派生宏放在独立的 `qframework-macros`，即使用户不想要过程宏也可以只依赖核心
- 全部测试与示例均可在无窗口环境下运行

---

## 目录结构

```
qframework/
├── Cargo.toml                     # workspace 根清单
├── docs/                          # 完整文档（见下方「文档」）
├── crates/
│   ├── qframework-core/           # 核心架构（不依赖任何引擎）
│   │   ├── src/
│   │   │   ├── architecture.rs    # Architecture / ArchitectureBuilder
│   │   │   ├── layers.rs          # 四层接口 + 能力接口
│   │   │   ├── command.rs         # ICommand（CQRS 写侧）
│   │   │   ├── query.rs           # IQuery（CQRS 读侧）
│   │   │   ├── context.rs         # CommandContext / QueryContext
│   │   │   ├── event.rs           # TypeEventSystem / EasyEvent
│   │   │   ├── ioc.rs             # IOCContainer
│   │   │   ├── bindable.rs        # BindableProperty / BindableList / BindableDictionary
│   │   │   └── unregister.rs      # IUnRegister / IUnRegisterList
│   │   ├── examples/counter.rs    # 纯 Rust 示例
│   │   └── tests/
│   │       ├── architecture.rs    # 架构 / 派生宏集成测试
│   │       └── bindable.rs        # 可观察容器测试
│   ├── qframework-macros/         # 过程宏 crate
│   │   ├── src/lib.rs             # #[derive(IModel / ISystem / IController / IUtility)]
│   │   └── src/layer.rs           # 四个派生宏的公共展开逻辑
│   └── qframework-bevy/           # Bevy 集成层
│       ├── src/
│       │   ├── app.rs             # QFrameworkPlugin / QArchitecture / QApplication
│       │   ├── bridge.rs          # QFramework 事件 -> Bevy Message 桥接
│       │   └── controller.rs      # 控制器每帧调度
│       ├── examples/bevy_counter.rs
│       └── tests/
│           ├── integration.rs     # Bevy 集成测试
│           └── docs_examples.rs   # 文档代码片段校验
└── examples/
    └── mini_game/                 # 完整示例工程（六种角色全部参与）
```

---

## 文档

完整的文档在 [`docs/`](docs/README.md)：

| 文档 | 内容 |
|---|---|
| [架构总览](docs/architecture.md) | 四层职责、依赖规则表、CQRS、与 Bevy ECS 的关系、与 C# 版的差异 |
| [核心概念](docs/core-concepts.md) | `Architecture`、IOC、事件系统、Command/Query、可观察容器、生命周期总表 |
| [派生宏](docs/derive-macros.md) | 四个派生宏的完整规则、`arch` 字段、生命周期钩子、展开后的代码、常见错误 |
| [Bevy 集成](docs/bevy.md) | 插件安装、资源访问、控制器调度、消息桥接、系统排序、无窗口测试 |
| [最佳实践](docs/best-practices.md) | 推荐写法、并发注意事项、性能、测试策略、命名与目录约定、反模式清单 |
| [排错手册](docs/troubleshooting.md) | 编译错误、运行时 panic、死锁、事件收不到、Bevy 相关问题 |

**第一次上手**：本页的[快速开始](#快速开始纯-rust) → [架构总览](docs/architecture.md) → [最佳实践](docs/best-practices.md)。

想直接看一个跑得起来的完整项目？[`examples/mini_game`](examples/mini_game/README.md)
把六种角色全部用上了（含 Bevy 集成、事件桥接、控制器调度），`cargo run -p mini-game` 即可运行。

---

## 与 C# 版 QFramework 的对应关系

| C# (QFramework) | Rust (本实现) | 说明 |
|---|---|---|
| `IController` | `IController` + `#[derive(IController)]` | 表现层 |
| `ISystem` | `ISystem` + `#[derive(ISystem)]` | 业务逻辑层 |
| `IModel` | `IModel` + `#[derive(IModel)]` | 数据层 |
| `IUtility` | `IUtility` + `#[derive(IUtility)]` | 工具层 |
| `ICommand` / `ICommand<TResult>` | `ICommand`（关联类型 `Output`） | 命令 |
| `IQuery<TResult>` | `IQuery`（关联类型 `Result`） | 查询 |
| `IArchitecture` / `Architecture<T>` | `Architecture` / `ArchitectureBuilder` | 中心枢纽 |
| `IOCContainer` | `IOCContainer` | 类型化依赖容器 |
| `TypeEventSystem` / `EasyEvent` | `TypeEventSystem` / `EasyEvent` | 类型事件总线 |
| `BindableProperty<T>` | `BindableProperty<T>` | 可观察属性 |
| `BindableList<T>` / `BindableDictionary<K,V>` | `BindableList<T>` / `BindableDictionary<K,V>` | 可观察集合 |
| `IUnRegister` / `IUnRegisterList` | `IUnRegister` / `IUnRegisterList` | 注销句柄 |
| `ICanGetModel` 等扩展方法 | `ICanGetModel` 等 trait | 能力接口 |
| `MonoBehaviour` 生命周期自动清理 | `Weak` 引用 + `Drop` 自动清理 | Rust 里不需要手动防泄漏 |

### 分层规则（编译期强约束）

| 层 | 接口 | 可以做什么 |
|---|---|---|
| 表现层 | `IController` | 取 System / Model，发 Command / Query，注册事件 |
| 业务逻辑层 | `ISystem` | 取 System / Model / Utility，发送与注册事件 |
| 数据层 | `IModel` | 取 Utility，发送事件 |
| 工具层 | `IUtility` | 什么都拿不到 |
| 命令 | `CommandContext` | 取 System / Model，发送事件与命令（**不能注册事件**） |
| 查询 | `QueryContext` | 取 System / Model / Utility（**不能发送任何东西**） |

例如在 `IModel` 上调用 `get_system::<T>()` 会直接编译失败——因为 `IModel` 没有实现 `ICanGetSystem`。

---

## 派生宏

四个派生宏会自动生成「能力接口 + 层接口」的全部实现。宏与同名 trait 位于不同命名空间，
因此 `#[derive(IModel)]` 与 `trait IModel` 可以共存（和 serde 的 `Serialize` 一样）。

```rust
#[derive(Default, IModel)]      // 生成 HasArchRef + ICanGetArchitecture
struct CounterModel {           //   + ICanGetUtility + ICanSendEvent + IModel
    arch: ArchRef,
    pub count: BindableProperty<i32>,
}
```

| 派生宏 | 生成的能力 |
|---|---|
| `#[derive(IController)]` | `ICanGetModel` `ICanGetSystem` `ICanSendCommand` `ICanSendQuery` `ICanRegisterEvent` |
| `#[derive(ISystem)]` | `ICanGetModel` `ICanGetSystem` `ICanGetUtility` `ICanSendEvent` `ICanRegisterEvent` |
| `#[derive(IModel)]` | `ICanGetUtility` `ICanSendEvent` |
| `#[derive(IUtility)]` | （无，Utility 不持有架构引用） |

**`arch` 字段**：Controller / System / Model 需要一个 `arch: ArchRef` 字段用于注入架构引用。
字段名不是 `arch` 时，用 `#[arch]` 标注即可；Utility 不需要该字段。

```rust
#[derive(Default, ISystem)]
struct MySystem {
    #[arch]
    context: ArchRef,           // 等价于 arch
}
```

**生命周期钩子**：用 `#[model(...)]` / `#[system(...)]` / `#[controller(...)]` / `#[utility(...)]`
指定 `init` / `deinit`，值可以是闭包，也可以是函数/方法路径：

```rust
#[derive(Default, ISystem)]
#[system(init = |this: &CounterSystem| { this.log("ready"); })]
struct CounterSystem { arch: ArchRef }

#[derive(Default, IModel)]
#[model(init = Self::on_init, deinit = Self::on_deinit)]
struct PlayerModel { arch: ArchRef }

impl PlayerModel {
    fn on_init(&self) { /* ... */ }
    fn on_deinit(&self) { /* ... */ }
}
```

---

## 快速开始（纯 Rust）

```rust
use qframework_core::prelude::*;

// 数据层
#[derive(Default, IModel)]
struct CounterModel {
    arch: ArchRef,                       // 约定字段名是 `arch`
    pub count: BindableProperty<i32>,
}

// 命令：唯一允许改变状态的地方
struct IncreaseCountCommand;

impl ICommand for IncreaseCountCommand {
    type Output = ();
    fn execute(&self, ctx: &CommandContext) {
        let model = ctx.get_model::<CounterModel>();
        model.count.modify(|count| *count += 1);   // 原子的「读-改-写」
        ctx.send_event(CountChangedEvent { count: model.count.get() });
    }
}

#[derive(Debug, Clone, Copy)]
struct CountChangedEvent { count: i32 }

fn main() {
    let architecture = ArchitectureBuilder::new("CounterApp")
        .model(CounterModel::default())
        .build();

    architecture.register_event::<CountChangedEvent, _>(|e| println!("count = {}", e.count));
    architecture.send_command(IncreaseCountCommand);
}
```

运行示例：

```bash
cargo run -p qframework-core --example counter
```

---

## 快速开始（Bevy）

```rust
use bevy::prelude::*;
use qframework_bevy::prelude::*;

#[derive(Default, IModel)]
struct CounterModel {
    arch: ArchRef,
    pub count: BindableProperty<i32>,
}

#[derive(Message, Clone)]
struct CountChangedMessage { count: i32 }

struct IncreaseCountCommand;
impl ICommand for IncreaseCountCommand {
    type Output = ();
    fn execute(&self, ctx: &CommandContext) {
        let model = ctx.get_model::<CounterModel>();
        model.count.modify(|count| *count += 1);   // 原子的「读-改-写」
        ctx.send_event(CountChangedMessage { count: model.count.get() });
    }
}

// 描述架构
struct CounterApp;

impl QApplication for CounterApp {
    fn build() -> ArchitectureBuilder {
        ArchitectureBuilder::new("CounterApp").model(CounterModel::default())
    }
}

fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_qframework::<CounterApp>()                       // 安装架构
        .bridge_q_messages::<CountChangedMessage>()           // QFramework 事件 -> Bevy 消息
        .add_systems(Update, read_messages)
        .run();
}

fn read_messages(mut reader: MessageReader<CountChangedMessage>) {
    for message in reader.read() {
        println!("count = {}", message.count);
    }
}
```

在任意 Bevy 系统里都可以直接拿到架构：

```rust
fn my_system(architecture: Res<QArchitecture>) {
    architecture.send_command(IncreaseCountCommand);            // 发命令
    let count = architecture.get_model::<CounterModel>();        // 取 Model
    println!("{}", count.count.get());
}
```

运行示例：

```bash
cargo run -p qframework-bevy --example bevy_counter
```

---

## 控制器：让 IController 跟着 Bevy 帧循环跑

```rust
#[derive(Default, IController)]
struct CounterController {
    arch: ArchRef,
    frames: AtomicI32,
}

impl QControllerUpdate for CounterController {
    fn update(&self, _delta: Duration) {
        if self.frames.fetch_add(1, Ordering::SeqCst) < 3 {
            self.send_command(IncreaseCountCommand);
        }
    }
}

// 注册后，控制器会在 `QFrameworkSet::Controllers` 中每帧被驱动
app.add_q_controller(CounterController::default());
```

控制器调度在 `Update` 的 `QFrameworkSet::Controllers` 集合中执行，
其他系统可以用 `.before(...)` / `.after(QFrameworkSet::Controllers)` 精确排序。

---

## 事件桥接说明

Bevy 的缓冲事件（`Message`）只能在系统中写入，而 QFramework 的事件可能由
Model / Command / System 在任意位置触发。桥接插件的工作方式是：

1. `PreUpdate` 之前，监听 QFramework 事件并推入一个跨线程队列；
2. `PreUpdate` 阶段把队列内容写入 Bevy 的 `Messages<M>`；
3. 其他系统照常使用 `MessageReader<M>`。

因此消息会在**下一帧**被读到（写入发生在 `PreUpdate`，读取通常在同帧的 `Update`）。

---

## 并发与性能

QFramework 原本面向 Unity 主线程，而这里的代码很可能跑在 Bevy 的并行系统里，因此有几点需要注意：

- **复合修改用 `modify`，不要 `get()` + `set()`。**
  `modify` 在一次写锁内完成「读-改-写」，是原子的；`get()` 后再 `set()` 在并发下会丢更新。
- **`send_command` 是同步执行的，架构不做排队或串行化。**
  命令里的「读-改-写」同样要保证原子性（即用 `modify`）。
- **只读时用 `with_value` / `with_items` / `with_entries`**，避免 `get()` / `snapshot()` 的整块拷贝。
- **不要在 `modify` / `with_value` 的闭包里再操作同一个属性。**
  `std::sync::RwLock` 不可重入，会死锁。
- **集合事件在写入之后发出。** 并发写入时，`BindableList` / `BindableDictionary` 的事件
  到达顺序不保证与写入顺序一致，但每个事件携带的值一定是当时真实写入的值。
- **每个事件订阅者持强引用。** 忘记注销会导致订阅者无法释放——用返回的 `IUnRegister`，
  或把句柄放进 `IUnRegisterList`（它在 `Drop` 时会自动注销）。

---

## 两阶段初始化

与 C# 版一致：

1. `ArchitectureBuilder::build()` 创建架构并按注册顺序执行注册动作；
2. **先**调用所有 `IModel::init()`，**再**调用所有 `ISystem::init()` / `IUtility::init()`；
3. 运行期再注册的层对象会立即初始化（延迟注册）。

反初始化顺序相反：System → Model → 清空 IOC 与事件。

`deinit` 是**终态**操作：调用之后架构不可再使用（`get_model` 等会 panic），重复调用是安全的。
在 Bevy 中由 `QFrameworkPlugin` 的 `cleanup` 自动触发，控制器也会在此时收到 `IController::deinit`。

如果需要按条件注册可选模块，用 `patch`——它对应 C# 版的 `OnRegisterPatch`，
在所有 `model` / `system` / `utility` 之后、初始化之前执行：

```rust
let architecture = ArchitectureBuilder::new("MyApp")
    .model(PlayerModel::default())
    .patch(|architecture| {
        if enable_debug_panel {
            architecture.register_system(DebugPanelSystem::default());
        }
    })
    .build();
```

---

## 开发

```bash
cargo test --workspace            # 单元测试 + 集成测试 + 文档测试
cargo clippy --workspace --all-targets
cargo run -p qframework-core --example counter     # 纯 Rust 计数器
cargo run -p qframework-bevy --example bevy_counter # Bevy 集成最小示例
cargo run -p mini-game                              # 完整示例工程
```

要求：Rust 1.95+（edition 2024）。`qframework-bevy` 依赖 Bevy 0.19。

---

## License

MIT
