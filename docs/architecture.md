# 架构总览

> 读完这篇你会知道：QFramework 为什么要分四层、每一层能做什么、这些规则在 Rust 里是怎么变成编译错误的。

---

## 一、一个要解决的问题

游戏代码最容易烂掉的方式是「什么都能访问什么」：

```rust
// 反例：UI 直接改了存档数据，存档又回头通知了 UI，谁都说不清数据从哪来
fn update_hp_bar(hp_text: &mut Text, save: &mut SaveData, events: &mut EventWriter<DamageEvent>) {
    let hp = save.player.hp - events.read().map(|e| e.damage).sum::<i32>();
    save.player.hp = hp;
    hp_text.0 = hp.to_string();
    if hp <= 0 { save.player.dead = true; }
}
```

这段代码没法单独测试、没法复用、改一处不知道会崩哪一处。QFramework 的解法是**强制单向依赖 + 单一状态修改入口**，并且——关键点——**把规则变成编译期错误，而不是靠 code review**。

---

## 二、四层

| 层 | 接口 | 派生宏 | 职责 | 典型内容 |
|---|---|---|---|---|
| 表现层 | `IController` | `#[derive(IController)]` | 接收输入、响应状态变化 | 输入处理、UI 刷新、相机控制 |
| 业务逻辑层 | `ISystem` | `#[derive(ISystem)]` | 跨多个表现层复用的逻辑 | 计时器、成就、商店、回合调度 |
| 数据层 | `IModel` | `#[derive(IModel)]` | 数据定义 + 增删改查 | 玩家属性、背包、关卡进度 |
| 工具层 | `IUtility` | `#[derive(IUtility)]` | 基础设施，不含业务 | 存档读写、序列化、平台 SDK |

外加一个不占层级的核心概念：

| 概念 | 接口 | 职责 |
|---|---|---|
| 命令 | `ICommand` | **唯一**允许改变应用状态的地方；不能持有字段状态 |
| 查询 | `IQuery` | 只读取数；不能有任何副作用 |

---

## 三、依赖规则

### 3.1 规则表

「调用方能否……」——✓ 可以，✗ 编译不过。

| 调用方 \ 能力 | 取 System | 取 Model | 取 Utility | 发 Command | 发 Query | 发 Event | 注册 Event |
|---|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| `IController` | ✓ | ✓ | ✗ | ✓ | ✓ | **✗** | ✓ |
| `ISystem` | ✓ | ✓ | ✓ | ✗ | ✗ | ✓ | ✓ |
| `IModel` | ✗ | ✗ | ✓ | ✗ | ✗ | ✓ | ✗ |
| `IUtility` | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| `ICommand` | ✓ | ✓ | ✗ | ✓ | ✗ | ✓ | **✗** |
| `IQuery` | ✓ | ✓ | ✓ | ✗ | ✗ | **✗** | ✗ |

几条值得单独说明的规则：

- **Controller 不能发送事件**，只能监听。发送事件意味着「我是状态变化的源头」，而 Controller 状态的唯一来源应该是 Command。
- **Model 不能获取其它 Model**（`IModel` 没有 `ICanGetModel`）。这是有意为之：需要跨 Model 协调的逻辑属于 System 或 Command，放在 Model 里会形成数据层内部网状的互相依赖。
- **Model 不能注册事件**，只能发送。数据层向上层单向广播，不反向监听。
- **Command 不能注册事件**。Command 是瞬时的、执行完就没了的，长期持有订阅没有意义。
- **Query 什么都不发送**。查询必须是纯只读、无副作用的。
- **Utility 什么都拿不到**。它只做封装，不承载业务，因此不许反向依赖架构。

### 3.2 上层可以直接调用下层，下层只能靠事件向上通信

```
                  ┌──────────────────────────────────────┐
                  │            表现层 IController         │
                  │   （输入 → Command；事件 → 刷新表现）  │
                  └───────┬──────────────────────▲───────┘
            发 Command / Query                   │ 注册 Event
                          ▼                      │
   ┌──────────────────────────────────────┐     │
   │          业务逻辑层 ISystem            │     │
   └───────┬──────────────────────▲───────┘     │
      取 Model / Utility          │ 发送 Event    │
           ▼                      │              │
   ┌──────────────────────────────┴───────┐     │
   │             数据层 IModel             │─────┘
   └───────────────┬──────────────────────┘
                   ▼
   ┌──────────────────────────────────────┐
   │            工具层 IUtility            │
   └──────────────────────────────────────┘
```

**下层不知道上层存在**：Model 只负责 `send_event`，谁来听、听了干什么，它一无所知。这就是 Model 可以脱离 UI 单独存在、单独测试的原因。

### 3.3 状态变更必须走 Command

这是整套设计里最重要的一条。对比：

```rust
// ❌ 任何地方都能改，改了几次、什么时候改的，全凭自觉
architecture.get_model::<PlayerModel>().hp.set(0);

// ✓ 状态变更被收拢到一个有名字、可审计、可回放的地方
architecture.send_command(KillPlayerCommand { reason: DeathReason::Fall });
```

好处是实打实的：

1. **可审计**：所有写操作都在 `ICommand` 实现里，搜索一下就能列全。
2. **可回放 / 可联网**：Command 是数据结构，序列化后能做录制回放或帧同步。
3. **可测试**：构造一个空架构，发一个 Command，断言 Model 的变化。
4. **横切关注点有唯一挂载点**：日志、权限校验、埋点都可以在 Command 执行路径上做。

---

## 四、规则是怎么变成编译错误的

C# 版靠「能力接口 + 扩展方法」，Rust 版用 trait 继承，效果一样但更彻底。

能力被拆成 8 个细粒度 trait：

| 能力 trait | 提供的方法 |
|---|---|
| `ICanGetArchitecture` | `architecture() -> Arc<Architecture>` |
| `ICanGetModel` | `get_model::<M>()` / `try_get_model::<M>()` |
| `ICanGetSystem` | `get_system::<S>()` |
| `ICanGetUtility` | `get_utility::<U>()` |
| `ICanSendCommand` | `send_command::<C>(cmd) -> C::Output` |
| `ICanSendQuery` | `send_query::<Q>(q) -> Q::Result` |
| `ICanSendEvent` | `send_event(e)` / `send_event_default::<E>()` |
| `ICanRegisterEvent` | `register_event::<E, _>(f) -> IUnRegister` |

然后层接口只继承自己该有的能力：

```rust
pub trait IModel: HasArchRef + ICanGetUtility + ICanSendEvent + Send + Sync + 'static { .. }
pub trait ISystem: HasArchRef + ICanGetModel + ICanGetSystem + ICanGetUtility
                        + ICanSendEvent + ICanRegisterEvent + Send + Sync + 'static { .. }
```

于是「Model 里调用 `get_system`」直接变成方法不存在：

```
error[E0599]: no method named `get_system` found for reference `&CounterModel` in the current scope
  --> src/model.rs:18:22
   |
18 |         let _ = self.get_system::<BattleSystem>();
   |                      ^^^^^^^^^^ method not found in `&CounterModel`
   |
   = note: the following trait defines an item `get_system`, perhaps you need to implement it:
           candidate #1: `qframework_core::ICanGetSystem`
```

而 Command / Query 连「层」都不是——它们拿到的是能力受限的**上下文**：`CommandContext` 没实现 `ICanRegisterEvent`，`QueryContext` 什么发送能力都没有。

---

## 五、两阶段初始化

对应 C# 版的 `Architecture<T>.Interface` 懒加载 + 分阶段 `Init()`：

1. **注册阶段**：`ArchitectureBuilder` 依次执行 `model` / `system` / `utility` / `patch` 注册动作，全部进入 IOC 容器并注入架构引用。
2. **Model 初始化**：按注册顺序调用每个 `IModel::init()`。
3. **System / Utility 初始化**：按注册顺序调用 `init()`。

**Model 先于 System** 是关键：System 的 `init()` 里可以放心地 `get_model::<M>()` 并读取已就绪的数据，无需担心顺序。

运行期再注册的层对象会**立即初始化**（延迟注册），不需要重新跑一遍全流程。

反初始化顺序相反：System → Model → 清空 IOC 与事件。

---

## 六、与 Bevy ECS 的关系

两者不是竞争关系，而是**不同维度**：

| 维度 | QFramework | Bevy ECS |
|---|---|---|
| 关注点 | 应用逻辑怎么组织、谁能访问谁 | 实体/组件数据怎么布局、系统怎么并行调度 |
| 数据单位 | Model（业务对象） | Component（细粒度数据） |
| 逻辑单位 | Command / System / Controller | System |
| 强制力 | 编译期类型系统 | 运行时访问冲突检测 |

推荐的分工：

- **QFramework 管「游戏业务逻辑」**：玩家属性、背包、关卡状态、成就、存档。
- **Bevy ECS 管「场景与渲染」**：Transform、Sprite、Mesh、动画、物理。
- **两者的桥**：`QArchitecture` 资源。Bevy 系统通过 `Res<QArchitecture>` 发命令、读数据；QFramework 的事件通过 `QEventBridgePlugin` 变成 Bevy 消息。

三层的数据流：

```
Bevy 输入系统 ──┐
                ├─→ Res<QArchitecture> ─→ send_command ─→ Command ─→ Model
QFramework 控制器┘                                                  │
                                                                    │ send_event
                                                                    ▼
Bevy 表现层 ←── MessageReader<M> ←── MessageWriter ←── QEventBridge ←┘
```

一块数据只应该有一个「家」：

- 属于游戏逻辑（HP、金币）→ 放 Model，用 `BindableProperty`。
- 属于渲染表现（位置、旋转、动画播放进度）→ 放 Component。
- **不要两边各存一份**。需要派生时，用一个 Bevy 系统从 Model 读、写进 Component。

---

## 七、与 C# 版的差异

| 方面 | C# QFramework | Rust 版 | 原因 |
|---|---|---|---|
| 架构实例 | `Architecture<T>.Interface` 静态单例 | `ArchitectureBuilder::build() -> Arc<Architecture>` | Rust 不鼓励全局可变状态；显式传入便于测试和多实例 |
| 层的基类 | `AbstractModel` / `AbstractSystem` 等继承 | `#[derive(IModel)]` 等派生宏 | Rust 无继承；派生宏同样能消除样板代码 |
| 架构引用 | 强引用字段，靠 `UnRegisterWhenGameObjectDestroyed` 防泄漏 | `ArchRef` 内部存 `Weak` | 让「不泄漏」成为默认行为而不是纪律 |
| 订阅清理 | 手动调用 `UnRegisterWhen*` | `IUnRegisterList` 在 `Drop` 时自动注销 | RAII |
| 线程模型 | Unity 主线程，基本单线程 | 可能跑在 Bevy 并行系统里 | **需要主动保证复合操作的原子性**，见下 |
| 命令返回值 | `ICommand<TResult>` 泛型接口 | `ICommand` 关联类型 `Output` | Rust 的一个类型只能实现一个 `ICommand` |
| 查询 | `IQuery<TResult>` | `IQuery` 关联类型 `Result` | 同上 |
| 事件路由 | CLR 类型作频道 | `TypeId` 作频道 | 等价 |
| 集合绑定 | `BindableList` / `BindableDictionary` | 同名，接口更贴近 Rust 习惯 | — |

**最大的差异是线程模型。** C# 版默认单线程，所以 `count.Value = count.Value + 1` 是安全的；Rust 版可能并发执行，必须写成 `count.modify(|c| *c += 1)`。详见[最佳实践](best-practices.md#三并发相关最重要的一节)。

---

## 八、模块结构

```
qframework-core/          与引擎无关，运行时零依赖（只用 std）
├── architecture.rs       Architecture / ArchitectureBuilder（中心枢纽）
├── layers.rs             四层接口 + 8 个能力接口 + ArchRef
├── command.rs            ICommand
├── query.rs              IQuery
├── context.rs            CommandContext / QueryContext（能力边界）
├── event.rs              TypeEventSystem / EasyEvent（类型事件总线）
├── ioc.rs                IOCContainer（TypeId -> Arc<dyn Any>）
├── bindable.rs           BindableProperty / BindableList / BindableDictionary
└── unregister.rs         IUnRegister / IUnRegisterList

qframework-macros/        过程宏（唯一引入外部依赖的 crate）
└── layer.rs              四个派生宏的公共展开逻辑

qframework-bevy/          Bevy 0.19 集成
├── app.rs                QApplication / QFrameworkPlugin / QArchitecture
├── bridge.rs             QFramework 事件 -> Bevy Message
└── controller.rs         QControllerUpdate / QControllers / QFrameworkSet
```

依赖方向是单向的：`qframework-bevy` → `qframework-core` → `qframework-macros`。
`qframework-core` 完全不认识 Bevy，所以你可以在非 Bevy 环境（服务端、编辑器工具、纯逻辑测试）里复用它。

---

## 下一步

- 想了解每个 API 的细节 → [核心概念](core-concepts.md)
- 想知道怎么定义层对象 → [派生宏](derive-macros.md)
- 准备写业务代码了 → [最佳实践](best-practices.md)
