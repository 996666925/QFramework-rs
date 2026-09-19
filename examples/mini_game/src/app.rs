//! 架构定义：整个游戏的「目录」。
//!
//! 把注册集中在一个文件的好处是——看这一个文件就知道游戏有哪些模块。

use qframework_bevy::prelude::*;

use crate::model::{AchievementModel, BattleModel, InventoryModel, PlayerModel, ShopModel};
use crate::system::{AchievementSystem, AutoSaveSystem};
use crate::utility::{LogUtility, SaveUtility};

/// 迷你地牢的应用架构。
///
/// `QApplication` 只是一个「型别标签」，不需要实例化。
pub struct MiniGame;

impl QApplication for MiniGame {
    fn build() -> ArchitectureBuilder {
        ArchitectureBuilder::new("MiniGame")
            // ── 工具层：基础设施，不承载业务 ──
            .utility(LogUtility::new())
            .utility(SaveUtility::new())
            // ── 数据层：注册顺序 = 初始化顺序 ──
            .model(PlayerModel::new_player())
            .model(InventoryModel::default())
            .model(ShopModel::default()) // 商品表由 #[model(init = ..)] 装载
            .model(BattleModel::default())
            .model(AchievementModel::default())
            // ── 业务逻辑层：跨 Model 协调、监听事件 ──
            .system(AchievementSystem::default())
            .system(AutoSaveSystem::default())
    }
}
