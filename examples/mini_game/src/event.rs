//! 全局事件定义。
//!
//! 事件集中放在一个文件，方便一眼看清整个游戏的通信拓扑。
//!
//! 注意区分**事件**和**状态**：
//!
//! - 「现在有多少金币」→ `PlayerModel::gold`（状态，可随时读取当前值）
//! - 「刚刚获得了一笔金币」→ [`GoldChangedEvent`]（事件，一次性通知）
//!
//! 事件是同步派发且**不缓冲**的：发送时没有订阅者就会被丢弃。

use bevy::prelude::Message;

use crate::model::ItemId;

/// 游戏开始。
#[derive(Debug, Clone, Copy, Default)]
pub struct GameStartedEvent;

/// 金币发生变化。
#[derive(Debug, Clone, Copy)]
pub struct GoldChangedEvent {
    /// 变化后的金币。
    pub gold: i32,
    /// 本次变化量（可为负）。
    pub delta: i32,
}

/// 血量发生变化。
#[derive(Debug, Clone, Copy)]
pub struct HpChangedEvent {
    /// 当前血量。
    pub hp: i32,
    /// 最大血量。
    pub max_hp: i32,
}

/// 购买成功。
#[derive(Debug, Clone, Copy)]
pub struct ItemPurchasedEvent {
    /// 购买的商品。
    pub item: ItemId,
    /// 数量。
    pub quantity: u32,
    /// 总花费。
    pub cost: i32,
}

/// 战斗开始。
#[derive(Debug, Clone, Copy)]
pub struct BattleStartedEvent {
    /// 敌人名字。
    pub enemy: &'static str,
    /// 敌人血量。
    pub enemy_hp: i32,
}

/// 敌人被击败。
///
/// 它既是 QFramework 事件，也是 Bevy 消息——通过 `bridge_q_messages` 转发给
/// Bevy 系统处理（见 `main.rs` 的 `battle_report`）。
#[derive(Message, Debug, Clone, Copy)]
pub struct EnemyDefeatedEvent {
    /// 敌人名字。
    pub name: &'static str,
    /// 掉落金币。
    pub gold_reward: i32,
    /// 获得经验。
    pub exp_reward: i32,
}

/// 升级。
#[derive(Debug, Clone, Copy)]
pub struct LevelUpEvent {
    /// 新的等级。
    pub level: u32,
    /// 新的最大血量。
    pub max_hp: i32,
}

/// 解锁成就。
#[derive(Debug, Clone, Copy)]
pub struct AchievementUnlockedEvent {
    /// 成就名。
    pub title: &'static str,
}
