//! HUD 控制器：把状态变化翻译成表现。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use qframework_bevy::prelude::*;

use crate::event::{BattleStartedEvent, GoldChangedEvent, HpChangedEvent};
use crate::model::PlayerModel;
use crate::query::{GetInventoryQuery, GetPlayerSnapshotQuery};

/// 状态栏。
///
/// 演示两件事：
///
/// 1. **数据绑定**：`register_with_init_value` 一次搞定「初值 + 后续变化」，
///    不需要额外写一遍初始化显示逻辑，也不会漏掉注册窗口内的变化；
/// 2. **脏标记**：回调只置位标记，真正的渲染每帧最多一次。
///    一条命令如果改了多个属性，HUD 也只会重绘一次。
///
/// 闭包必须是 `'static`，不能捕获 `&self`，所以共享的是
/// `Arc<AtomicBool>`（这正是「控制器靠内部可变性持有状态」的体现）。
#[derive(Default, IController)]
#[controller(init = Self::start)]
pub struct HudController {
    arch: ArchRef,
    subscriptions: Mutex<IUnRegisterList>,
    /// 数据是否变化过（下一帧需要重绘）。
    dirty: Arc<AtomicBool>,
    /// 上一次渲染的内容。
    ///
    /// `set` / `modify` **不做相等判断**——值没变也会通知，所以脏标记可能被
    /// 无意义地置位。这里再比对一次内容，确保不会重复输出同一行。
    last_rendered: Mutex<String>,
}

impl HudController {
    fn start(&self) {
        let player = self.get_model::<PlayerModel>();
        let mut subscriptions = self.subscriptions.lock().unwrap();

        // ── 状态绑定：变化时只置脏，由 update() 合并成每帧最多一次重绘 ──
        let dirty = Arc::clone(&self.dirty);
        subscriptions.add(player.hp.register_with_init_value(move |_| {
            dirty.store(true, Ordering::SeqCst);
        }));

        let dirty = Arc::clone(&self.dirty);
        subscriptions.add(player.gold.register_with_init_value(move |_| {
            dirty.store(true, Ordering::SeqCst);
        }));

        let dirty = Arc::clone(&self.dirty);
        subscriptions.add(player.level.register_with_init_value(move |_| {
            dirty.store(true, Ordering::SeqCst);
        }));

        // ── 事件订阅：一次性的「发生了什么」，立刻给一行反馈 ──
        subscriptions.add(self.register_event::<HpChangedEvent, _>(|event| {
            println!("  [HUD·事件] 血量 -> {}/{}", event.hp, event.max_hp);
        }));

        subscriptions.add(self.register_event::<GoldChangedEvent, _>(|event| {
            let sign = if event.delta >= 0 { "+" } else { "" };
            println!("  [HUD·事件] 金币 {sign}{} -> {}", event.delta, event.gold);
        }));

        subscriptions.add(self.register_event::<BattleStartedEvent, _>(|event| {
            println!("  [HUD·事件] 遭遇 {}（{} HP）", event.enemy, event.enemy_hp);
        }));
    }

    /// 渲染状态栏（内容没变就跳过）。
    ///
    /// 用 Query 取数，而不是直接摸 Model——表现层只关心「要显示什么」。
    fn render(&self) {
        let line = self.compose_status();

        let mut last = self.last_rendered.lock().unwrap();
        if *last == line {
            return;
        }

        *last = line.clone();
        println!("{line}");
    }

    /// 拼出状态栏文本。
    fn compose_status(&self) -> String {
        let player = self.send_query(GetPlayerSnapshotQuery);
        let items = self.send_query(GetInventoryQuery);

        let bag: u32 = items.iter().map(|(_, count)| *count).sum();

        format!(
            "  [HUD] HP {}/{}  |  金币 {}  |  等级 {}  |  背包 {} 件",
            player.hp, player.max_hp, player.gold, player.level, bag
        )
    }
}

impl QControllerUpdate for HudController {
    fn update(&self, _delta: Duration) {
        // 数据没变就什么都不做——避免每帧无谓地读取和打印
        if !self.dirty.swap(false, Ordering::SeqCst) {
            return;
        }

        self.render();
    }
}
