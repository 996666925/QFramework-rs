# QFramework for Rust · 文档

这是 [QFramework](https://github.com/liangxiegame/QFramework) 的 Rust 实现，为 [Bevy](https://bevy.org) 打造。
本目录是完整文档，根目录的 [`README.md`](../README.md) 只有概览和快速开始。

---

## 文档地图

| 文档 | 内容 | 适合什么时候读 |
|---|---|---|
| [架构总览](architecture.md) | 四层职责、依赖规则、CQRS、与 Bevy ECS 的关系 | 上手前，建立心智模型 |
| [核心概念](core-concepts.md) | `Architecture`、IOC、事件系统、Command/Query、可观察容器、生命周期 | 写代码时当参考手册 |
| [派生宏](derive-macros.md) | `#[derive(IModel)]` 等四个宏的完整规则与限制 | 定义层对象时 |
| [Bevy 集成](bevy.md) | 插件、资源、控制器调度、消息桥接、系统排序、无窗口测试 | 在 Bevy 项目里落地时 |
| [最佳实践](best-practices.md) | 推荐写法、反模式、命名约定、性能与测试策略 | **写业务代码前必读** |
| [排错手册](troubleshooting.md) | 编译错误、运行时 panic、死锁、事件收不到 | 遇到问题时 |

---

## 三条阅读路径

**我第一次接触 QFramework**

1. [架构总览](architecture.md) —— 先搞懂「为什么要分层」
2. 根目录 [README 的快速开始](../README.md#快速开始纯-rust)
3. `cargo run -p qframework-core --example counter` 跑起来看输出
4. [`examples/mini_game`](../examples/mini_game/README.md) —— 完整示例工程，六种角色全用上
5. [最佳实践](best-practices.md) —— 避免踩坑

**我要在 Bevy 项目里用**

1. [架构总览](architecture.md) 的「与 Bevy ECS 的关系」一节
2. [Bevy 集成](bevy.md)
3. `cargo run -p qframework-bevy --example bevy_counter`
4. [Bevy 集成](bevy.md) 的「系统排序」与「无窗口测试」

**我熟悉 C# 版 QFramework，想快速迁移**

1. [架构总览](architecture.md) 的「与 C# 版的差异」一节
2. [派生宏](derive-macros.md) —— 替代手写 `AbstractModel` 等基类
3. [最佳实践](best-practices.md) 的「并发相关」一节 —— 这是最大的语义差异来源

---

## 5 分钟速览

```rust
use qframework_core::prelude::*;

// 数据层：只放数据 + CRUD，只能使用 Utility、只能发事件
#[derive(Default, IModel)]
struct CounterModel {
    arch: ArchRef,
    pub count: BindableProperty<i32>,
}

// 命令：唯一允许改变状态的地方，且不能持有状态
struct IncreaseCountCommand;

impl ICommand for IncreaseCountCommand {
    type Output = ();
    fn execute(&self, ctx: &CommandContext) {
        let model = ctx.get_model::<CounterModel>();
        model.count.modify(|count| *count += 1);          // 原子读-改-写
        ctx.send_event(CountChangedEvent { count: model.count.get() });
    }
}

// 查询：只读、无副作用
struct GetCountQuery;

impl IQuery for GetCountQuery {
    type Result = i32;
    fn do_query(&self, ctx: &QueryContext) -> i32 {
        ctx.get_model::<CounterModel>().count.get()
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
    println!("{}", architecture.send_query(GetCountQuery));   // 1
}
```

在 Bevy 里：

```rust
App::new()
    .add_plugins(MinimalPlugins)
    .add_qframework::<CounterApp>()                  // 安装架构
    .bridge_q_messages::<CountChangedMessage>()      // QFramework 事件 -> Bevy 消息
    .add_q_controller(CounterController::default())  // 每帧驱动控制器
    .add_systems(Update, read_messages)
    .run();
```

更多细节见 [Bevy 集成](bevy.md)。

---

## 术语对照

| 本框架 | 含义 | 对应 Bevy 概念 |
|---|---|---|
| `Architecture` | 全局唯一的服务定位器 + 事件总线 | 一组 `Resource` 的集合 |
| `IModel` | 数据持有者 | `Resource` |
| `ISystem` | 跨表现层复用的业务逻辑 | `Resource` + 一批系统 |
| `IUtility` | 基础设施封装（存储 / SDK / 序列化） | `Resource` 或第三方库封装 |
| `IController` | 表现层：输入 → Command，事件 → 刷新 | `System` + 少量状态 |
| `ICommand` | 一次状态变更（写侧） | 直接改 `ResMut<T>` |
| `IQuery` | 一次只读取数（读侧） | 只读 `Res<T>` |
| `Event` | 一次性通知，不缓冲 | `Message`（`MessageReader/Writer`） |
| `BindableProperty<T>` | 可观察状态，保存当前值 | `Resource` + `Changed<T>` 的手动版 |
| `BindableList<T>` | 可观察列表 | — |
| `BindableDictionary<K, V>` | 可观察字典 | — |
| `IUnRegister` | 注销句柄 | — |
