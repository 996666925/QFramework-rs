//! 纯 Rust 版的 QFramework 计数器示例（不依赖任何引擎）。
//!
//! 运行：`cargo run -p qframework-core --example counter`

use std::sync::Mutex;

use qframework_core::prelude::*;

// ---------------------------------------------------------------------------
// 工具层（Utility）：基础设施，不承载业务，也拿不到架构
// ---------------------------------------------------------------------------

#[derive(Default, IUtility)]
struct ConsoleUtility;

impl ConsoleUtility {
    fn log(&self, message: impl std::fmt::Display) {
        println!("[QFramework] {message}");
    }
}

// ---------------------------------------------------------------------------
// 数据层（Model）：只允许使用 Utility 与发送事件
// ---------------------------------------------------------------------------

#[derive(Default, IModel)]
struct CounterModel {
    arch: ArchRef,
    pub count: BindableProperty<i32>,
}

impl CounterModel {
    /// 修改数据并广播事件（Model 只能通过事件向上层通信）。
    ///
    /// 用 `modify` 而不是 `get()` + `set()`：前者在一次写锁内完成「读-改-写」，
    /// 是原子的，多线程下不会丢更新。
    fn add(&self, delta: i32) {
        self.count.modify(|count| *count += delta);
        self.send_event(CountChangedEvent {
            count: self.count.get(),
        });
    }

    fn reset(&self) {
        self.count.set(0);
        self.send_event(CountChangedEvent { count: 0 });
    }
}

/// 状态变化事件。
#[derive(Debug, Clone, Copy)]
struct CountChangedEvent {
    count: i32,
}

// ---------------------------------------------------------------------------
// 命令（Command）：唯一可以改变状态的地方，且不能持有状态
// ---------------------------------------------------------------------------

/// 计数 +1。
struct IncreaseCountCommand;

impl ICommand for IncreaseCountCommand {
    type Output = ();

    fn execute(&self, ctx: &CommandContext) {
        ctx.get_model::<CounterModel>().add(1);
    }
}

/// 计数 +N，并返回变化后的值。
struct AddCountCommand {
    delta: i32,
}

impl ICommand for AddCountCommand {
    type Output = i32;

    fn execute(&self, ctx: &CommandContext) -> i32 {
        let model = ctx.get_model::<CounterModel>();
        model.add(self.delta);
        model.count.get()
    }
}

// ---------------------------------------------------------------------------
// 查询（Query）：只读，无副作用
// ---------------------------------------------------------------------------

struct GetCountQuery;

impl IQuery for GetCountQuery {
    type Result = i32;

    fn do_query(&self, ctx: &QueryContext) -> i32 {
        ctx.get_model::<CounterModel>().count.get()
    }
}

// ---------------------------------------------------------------------------
// 业务逻辑层（System）：可被多个表现层复用的逻辑
// ---------------------------------------------------------------------------

#[derive(Default, ISystem)]
#[system(init = |this: &CounterSystem| {
    this.log("CounterSystem 初始化完成");
})]
struct CounterSystem {
    arch: ArchRef,
}

impl CounterSystem {
    fn reset(&self) {
        self.get_model::<CounterModel>().reset();
    }

    fn log(&self, message: impl std::fmt::Display) {
        self.get_utility::<ConsoleUtility>().log(message);
    }
}

// ---------------------------------------------------------------------------
// 表现层（Controller）：接收输入、响应状态变化，但只能通过 Command 改状态
// ---------------------------------------------------------------------------

#[derive(Default, IController)]
struct CounterController {
    arch: ArchRef,
    subscriptions: Mutex<IUnRegisterList>,
}

impl CounterController {
    /// 订阅状态变化事件（Controller 可以注册事件，但不能发送事件）。
    fn start(&self) {
        let unregister = self.register_event::<CountChangedEvent, _>(|event| {
            println!("[Controller] 收到事件，当前计数 = {}", event.count);
        });
        self.subscriptions.lock().unwrap().add(unregister);
    }
}

// ---------------------------------------------------------------------------

fn main() {
    let architecture = ArchitectureBuilder::new("CounterApp")
        .utility(ConsoleUtility)
        .model(CounterModel::default())
        .system(CounterSystem::default())
        .build();

    println!(
        "架构 `{}` 初始化完成（inited = {}）",
        architecture.name(),
        architecture.is_inited()
    );

    // Controller 由架构注入引用
    let controller = architecture.attach_controller(CounterController::default());
    controller.start();

    // 通过命令改变状态
    architecture.send_command(IncreaseCountCommand);
    architecture.send_command(IncreaseCountCommand);

    let count = architecture.send_command(AddCountCommand { delta: 5 });
    println!("AddCountCommand 返回: {count}");

    // 通过查询读取状态
    println!(
        "GetCountQuery 结果: {}",
        architecture.send_query(GetCountQuery)
    );

    // System 提供的复用逻辑
    architecture.get_system::<CounterSystem>().reset();
    println!("重置后: {}", architecture.send_query(GetCountQuery));

    // 注销后不再收到事件
    controller.subscriptions.lock().unwrap().unregister_all();
    architecture.send_command(IncreaseCountCommand);
    println!("（事件已注销，上方没有 Controller 的日志）");

    architecture.deinit();
    println!("架构已反初始化");
}
