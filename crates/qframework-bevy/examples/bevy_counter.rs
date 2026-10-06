//! Bevy 集成示例：QFramework 业务架构 + 消息桥接 + Bevy 表现层系统。
//!
//! 运行：`cargo run -p qframework-bevy --example bevy_counter`

use bevy::prelude::*;
use qframework_bevy::prelude::*;

// ---------------------------------------------------------------------------
// 数据层（Model）
// ---------------------------------------------------------------------------

#[derive(Default, IModel)]
struct CounterModel {
    arch: ArchRef,
    pub count: BindableProperty<i32>,
}

/// 计数变化事件：它同时是 QFramework 事件和 Bevy 消息。
#[derive(Message, Clone, Debug)]
struct CountChangedMessage {
    count: i32,
}

// ---------------------------------------------------------------------------
// 命令与查询
// ---------------------------------------------------------------------------

struct IncreaseCountCommand;

impl ICommand for IncreaseCountCommand {
    type Output = ();

    fn execute(&self, ctx: &CommandContext) {
        let model = ctx.get_model::<CounterModel>();
        // `modify` 就地修改，释放借用后统一通知
        model.count.modify(|count| *count += 1);
        // Model 通过事件向上层通信，桥接插件会把它转成 Bevy 消息
        ctx.send_event(CountChangedMessage {
            count: model.count.get(),
        });
    }
}

struct GetCountQuery;

impl IQuery for GetCountQuery {
    type Result = i32;

    fn do_query(&self, ctx: &QueryContext) -> i32 {
        ctx.get_model::<CounterModel>().count.get()
    }
}

// ---------------------------------------------------------------------------
// 业务逻辑层（System）
// ---------------------------------------------------------------------------

#[derive(Default, ISystem)]
struct CounterSystem {
    arch: ArchRef,
}

// ---------------------------------------------------------------------------
// 架构定义
// ---------------------------------------------------------------------------

struct CounterApp;

impl QApplication for CounterApp {
    fn build() -> ArchitectureBuilder {
        ArchitectureBuilder::new()
            .model(CounterModel::default())
            .system(CounterSystem::default())
    }
}

// ---------------------------------------------------------------------------
// 表现层：Bevy 系统通过命令改变业务状态
// ---------------------------------------------------------------------------

fn increase_count(architecture: NonSend<QArchitecture>, mut frames: Local<u32>) {
    if *frames < 3 {
        architecture.send_command(IncreaseCountCommand);
    }
    *frames += 1;
}

// ---------------------------------------------------------------------------
// Bevy 系统：读取桥接过来的消息
// ---------------------------------------------------------------------------

fn on_count_changed(mut reader: MessageReader<CountChangedMessage>) {
    for message in reader.read() {
        println!("[Bevy Message] 计数 -> {}", message.count);
    }
}

fn main() {
    let mut app = App::new();

    app.add_plugins(MinimalPlugins)
        .install_architecture::<CounterApp>()
        .bridge_messages::<CountChangedMessage>()
        .add_systems(Update, (increase_count, on_count_changed).chain());

    // 无窗口环境下手动推进若干帧
    for _ in 0..5 {
        app.update();
    }

    let architecture = app.world().non_send::<QArchitecture>();
    println!(
        "[Bevy System] 最终计数 = {}",
        architecture.send_query(GetCountQuery)
    );
}
