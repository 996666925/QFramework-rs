//! 玩家状态查询。

use qframework_core::prelude::*;

use crate::model::PlayerModel;

/// 玩家状态快照。
///
/// Query 的结果类型必须是 `Send + 'static`，所以这里返回一份拷贝而不是引用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerSnapshot {
    /// 当前血量。
    pub hp: i32,
    /// 最大血量。
    pub max_hp: i32,
    /// 金币。
    pub gold: i32,
    /// 等级。
    pub level: u32,
    /// 当前经验。
    pub exp: i32,
    /// 攻击力。
    pub attack: i32,
}

/// 读取玩家状态快照。
pub struct GetPlayerSnapshotQuery;

impl IQuery for GetPlayerSnapshotQuery {
    type Result = PlayerSnapshot;

    fn do_query(&self, ctx: &QueryContext) -> PlayerSnapshot {
        let player = ctx.get_model::<PlayerModel>();

        PlayerSnapshot {
            hp: player.hp.get(),
            max_hp: player.max_hp.get(),
            gold: player.gold.get(),
            level: player.level.get(),
            exp: player.exp.get(),
            attack: player.attack.get(),
        }
    }
}
