//! 战斗数据。
//!
//! 敌人的所有属性被打包进**一个** `BindableProperty<EnemyState>`，
//! 而不是拆成 6 个属性——这样一次修改只产生一次通知（见最佳实践「性能」）。
//!
//! 另外注意：`BattleModel` 自己会广播 `BattleStartedEvent` / `EnemyDefeatedEvent`，
//! 而**是否给奖励、奖励多少**这种跨 Model 的决策在 Command 里。

use qframework_core::prelude::*;

use crate::event::{BattleStartedEvent, EnemyDefeatedEvent};

/// 一场战斗的初始配置。
#[derive(Debug, Clone, Copy)]
pub struct Enemy {
    /// 名字。
    pub name: &'static str,
    /// 血量。
    pub hp: i32,
    /// 攻击力。
    pub attack: i32,
    /// 击败后掉落的金币。
    pub gold_reward: i32,
    /// 击败后给予的经验。
    pub exp_reward: i32,
}

/// 当前敌人的运行时状态。
#[derive(Debug, Clone, Copy, Default)]
pub struct EnemyState {
    /// 名字。
    pub name: &'static str,
    /// 当前血量。
    pub hp: i32,
    /// 最大血量。
    pub max_hp: i32,
    /// 攻击力。
    pub attack: i32,
    /// 掉落金币。
    pub gold_reward: i32,
    /// 给予经验。
    pub exp_reward: i32,
}

impl EnemyState {
    /// 是否已被击败。
    pub fn is_defeated(&self) -> bool {
        self.hp <= 0
    }
}

/// 战斗状态。
#[derive(Default, IModel)]
pub struct BattleModel {
    arch: ArchRef,
    /// 是否正在战斗。
    pub in_battle: BindableProperty<bool>,
    /// 当前敌人（一次战斗只产生一次通知）。
    pub enemy: BindableProperty<EnemyState>,
}

impl BattleModel {
    /// 开始一场战斗。
    pub fn start(&self, enemy: Enemy) {
        self.enemy.set(EnemyState {
            name: enemy.name,
            hp: enemy.hp,
            max_hp: enemy.hp,
            attack: enemy.attack,
            gold_reward: enemy.gold_reward,
            exp_reward: enemy.exp_reward,
        });
        self.in_battle.set(true);
        self.send_event(BattleStartedEvent {
            enemy: enemy.name,
            enemy_hp: enemy.hp,
        });
    }

    /// 对敌人造成伤害，返回是否因此击杀。
    pub fn damage_enemy(&self, amount: i32) -> bool {
        let mut defeated = false;

        self.enemy.modify(|enemy| {
            enemy.hp = (enemy.hp - amount).max(0);
            defeated = enemy.is_defeated();
        });

        defeated
    }

    /// 结束战斗。
    pub fn finish(&self) {
        self.in_battle.set(false);
    }

    /// 广播击败事件（由 Command 在结算奖励后调用）。
    pub fn announce_defeat(&self) {
        let enemy = self.enemy.get();
        self.send_event(EnemyDefeatedEvent {
            name: enemy.name,
            gold_reward: enemy.gold_reward,
            exp_reward: enemy.exp_reward,
        });
    }
}
