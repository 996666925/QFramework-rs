//! 剧本控制器：模拟玩家输入。
//!
//! 表现层的职责是「把输入翻译成命令」。这里用固定剧本模拟玩家，
//! 真实项目里换成键盘 / 手柄 / 触屏输入即可——命令层完全不用改。
//!
//! 注意这里**没有**任何 `get_model().gold.set(...)` 这类写法：
//! 表现层不碰数据，只发命令。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use qframework_bevy::prelude::*;

use crate::command::{
    AttackOutcome, BuyItemCommand, PlayerAttackCommand, StartBattleCommand, StartGameCommand,
    UseItemCommand,
};
use crate::model::{Enemy, ItemId, ShopModel};
use crate::query::{GetInventoryQuery, GetPlayerSnapshotQuery};
use crate::system::AchievementSystem;

/// 每几帧执行一个剧本步骤，让 HUD 的刷新有机会体现出来。
const FRAMES_PER_STEP: usize = 4;

/// 本场的对手。
const GOBLIN: Enemy = Enemy {
    name: "哥布林",
    hp: 30,
    attack: 8,
    gold_reward: 50,
    exp_reward: 60,
};

/// 用固定剧本驱动的「玩家输入」。
#[derive(Default, IController)]
pub struct ScriptController {
    arch: ArchRef,
    frame: AtomicUsize,
    step: AtomicUsize,
}

impl QControllerUpdate for ScriptController {
    fn update(&self, _delta: Duration) {
        let frame = self.frame.fetch_add(1, Ordering::SeqCst);
        if !frame.is_multiple_of(FRAMES_PER_STEP) {
            return;
        }

        self.play(self.step.fetch_add(1, Ordering::SeqCst));
    }
}

impl ScriptController {
    fn play(&self, step: usize) {
        match step {
            0 => {
                println!();
                println!("══ 剧本 1：开始游戏 ══");
                self.send_command(StartGameCommand);
            }
            1 => {
                println!();
                println!("══ 剧本 2：商店买两瓶药水 ══");
                // 表现层可以直接读 Model 做展示，但改状态必须走命令
                println!(
                    "  · 本店在售 {} 种商品",
                    self.get_model::<ShopModel>().listed()
                );
                self.buy(ItemId::HealthPotion, 2);
            }
            2 => {
                println!();
                println!("══ 剧本 3：试试买一把剑（金币不够）══");
                self.buy(ItemId::IronSword, 1);
            }
            3 => {
                println!();
                println!("══ 剧本 4：进入战斗 ══");
                self.send_command(StartBattleCommand { enemy: GOBLIN });
            }
            4..=6 => self.attack(),
            7 => {
                println!();
                println!("══ 剧本 5：喝药回血 ══");
                match self.send_command(UseItemCommand {
                    item: ItemId::HealthPotion,
                }) {
                    Ok(effective) => println!("  · 喝下治疗药水，实际恢复 {effective} 点血量"),
                    Err(error) => println!("  · 使用失败：{error}"),
                }
            }
            8 => self.report(),
            _ => {}
        }
    }

    /// 购买并汇报结果。
    fn buy(&self, item: ItemId, quantity: u32) {
        match self.send_command(BuyItemCommand { item, quantity }) {
            Ok(total) => println!("  · 买入 {} ×{quantity}，花费 {total} 金币", item.label()),
            Err(error) => println!("  · 购买 {} 失败：{error}", item.label()),
        }
    }

    /// 攻击一次并汇报结果。
    fn attack(&self) {
        match self.send_command(PlayerAttackCommand) {
            AttackOutcome::NotInBattle => println!("  · 当前不在战斗中"),
            AttackOutcome::Hit { enemy, hp, max_hp } => {
                println!("  · 命中！{enemy} 还剩 {hp}/{max_hp} 点血")
            }
            AttackOutcome::Defeated { name } => println!("  · 击败了 {name}！"),
            AttackOutcome::PlayerDied => println!("  · 你倒下了……"),
        }
    }

    /// 中场报告：用 Query 读数据，用 System 取聚合信息。
    fn report(&self) {
        let player = self.send_query(GetPlayerSnapshotQuery);
        let inventory = self.send_query(GetInventoryQuery);
        // Controller 没有 Utility 能力，但可以取 System
        let achievements = self.get_system::<AchievementSystem>().unlocked_count();

        println!();
        println!("══ 剧本 6：中场报告 ══");
        println!(
            "  · 等级 {}（{} exp），金币 {}，HP {}/{}，攻击力 {}",
            player.level, player.exp, player.gold, player.hp, player.max_hp, player.attack
        );

        for (item, count) in &inventory {
            println!("  · 背包：{} ×{count}", item.label());
        }

        println!("  · 已解锁成就 {achievements} 项");
    }
}
