//! 战斗相关命令。

use qframework_core::prelude::*;

use crate::model::{BattleModel, Enemy, PlayerModel};

/// 开始一场战斗。
pub struct StartBattleCommand {
    /// 对手。
    pub enemy: Enemy,
}

impl ICommand for StartBattleCommand {
    type Output = ();

    fn execute(&self, ctx: &CommandContext) {
        ctx.get_model::<BattleModel>().start(self.enemy);
    }
}

/// 一次攻击的结果。
///
/// 调用方需要的信息直接放在 `Output` 里，这样就不用再额外查一次战斗状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackOutcome {
    /// 当前不在战斗中。
    NotInBattle,
    /// 命中敌人，敌人成功反击。
    Hit {
        /// 敌人名字。
        enemy: &'static str,
        /// 敌人剩余血量。
        hp: i32,
        /// 敌人最大血量。
        max_hp: i32,
    },
    /// 击杀敌人。
    Defeated {
        /// 敌人名字。
        name: &'static str,
    },
    /// 玩家被反击致死。
    PlayerDied,
}

/// 玩家攻击一次（包含击杀结算与敌人反击）。
///
/// 这是「一条命令做一件完整的事」的示例。奖励结算放在这里而不是
/// `BattleModel` 里，因为那需要同时读 `PlayerModel` 和 `BattleModel`——
/// 数据层做不到（`IModel` 拿不到别的 Model），而这个约束正是在提示你
/// 「这是业务逻辑，应该属于命令或 System」。
pub struct PlayerAttackCommand;

impl ICommand for PlayerAttackCommand {
    type Output = AttackOutcome;

    fn execute(&self, ctx: &CommandContext) -> AttackOutcome {
        let battle = ctx.get_model::<BattleModel>();
        let player = ctx.get_model::<PlayerModel>();

        if !battle.in_battle.get() {
            return AttackOutcome::NotInBattle;
        }

        // 1) 玩家出手
        if battle.damage_enemy(player.attack.get()) {
            let enemy = battle.enemy.get();
            battle.finish();

            // 2) 结算奖励（先给奖励，再广播，保证监听者读到的是结算后的状态）
            player.add_gold(enemy.gold_reward);
            player.gain_exp(enemy.exp_reward);

            // 3) 广播。桥接插件会把它转成 Bevy 消息
            battle.announce_defeat();

            return AttackOutcome::Defeated { name: enemy.name };
        }

        // 4) 敌人反击
        let enemy = battle.enemy.get();
        if player.take_damage(enemy.attack) {
            battle.finish();
            return AttackOutcome::PlayerDied;
        }

        AttackOutcome::Hit {
            enemy: enemy.name,
            hp: enemy.hp,
            max_hp: enemy.max_hp,
        }
    }
}
