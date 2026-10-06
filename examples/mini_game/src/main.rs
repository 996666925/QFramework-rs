//! # QFramework 迷你地牢
//!
//! 一个「从零开始、业务架构与 Bevy 表现层配合」的完整示例：
//!
//! | 角色 | 位置 | 代表 |
//! |---|---|---|
//! | 工具层 `IUtility` | [`utility`] | `LogUtility`、`SaveUtility` |
//! | 数据层 `IModel` | [`model`] | `PlayerModel`、`InventoryModel`、`ShopModel`、`BattleModel`、`AchievementModel` |
//! | 命令 `ICommand` | [`command`] | `BuyItemCommand`、`PlayerAttackCommand` |
//! | 查询 `IQuery` | [`query`] | `GetPlayerSnapshotQuery`、`GetInventoryQuery` |
//! | 业务逻辑层 `ISystem` | [`system`] | `AchievementSystem`、`AutoSaveSystem` |
//! | Bevy 表现层 | [`controller`] | `render_hud`、`run_script` |
//!
//! 运行：`cargo run -p mini-game`
//!
//! 整个游戏无窗口运行（只用到 `MinimalPlugins`），剧本由 [`controller::script`] 驱动。

mod app;
mod command;
mod controller;
mod event;
mod model;
mod query;
mod system;
mod utility;

use bevy::prelude::*;
use qframework_bevy::prelude::*;

use crate::app::MiniGame;
use crate::controller::{HudState, render_hud, run_script, setup_hud};
use crate::event::EnemyDefeatedEvent;
use crate::model::AchievementModel;
use crate::query::{GetInventoryQuery, GetPlayerSnapshotQuery, PlayerSnapshot};
use crate::utility::SaveUtility;

/// 剧本总共需要的帧数（每个步骤间隔 4 帧，共 10 步）。
const SCRIPT_FRAMES: i32 = 48;

/// 一个普通的 Bevy 系统：读取被桥接过来的 QFramework 事件。
///
/// 事件由 `BattleModel` 在战斗结算时触发，`bridge_messages` 把它转成 Bevy 消息，
/// 这里用 `MessageReader` 消费——QFramework 的业务逻辑完全不需要知道 Bevy 的存在。
fn battle_report(mut reader: MessageReader<EnemyDefeatedEvent>) {
    for event in reader.read() {
        println!(
            "  [Bevy 系统] 收到战斗结算：{} 掉落 {} 金币 / {} 经验",
            event.name, event.gold_reward, event.exp_reward
        );
    }
}

/// 剧本跑完后打印最终结算。
///
/// 这里演示「引擎侧如何使用架构」：直接取 `QArchitecture` 资源，
/// 用 Query 读数据，用 Utility 落盘。
fn print_summary(app: &mut App) {
    let architecture = app.world().resource::<QArchitecture>().arc();

    let player: PlayerSnapshot = architecture.send_query(GetPlayerSnapshotQuery);
    let inventory = architecture.send_query(GetInventoryQuery);
    let achievements = architecture
        .get_model::<AchievementModel>()
        .unlocked
        .snapshot();

    let backpack = if inventory.is_empty() {
        "空".to_owned()
    } else {
        inventory
            .iter()
            .map(|(item, count)| format!("{} ×{count}", item.label()))
            .collect::<Vec<_>>()
            .join("、")
    };

    println!();
    println!("════════════════════ 结算 ════════════════════");
    println!("  等级      {}（{} exp）", player.level, player.exp);
    println!("  生命      {}/{}", player.hp, player.max_hp);
    println!("  攻击力    {}", player.attack);
    println!("  金币      {}", player.gold);
    println!("  背包      {backpack}");
    println!("  成就      {} 项", achievements.len());

    for title in &achievements {
        println!("              · {title}");
    }

    // 存档：引擎侧通过 Architecture 直接取 Utility
    let save = architecture.get_utility::<SaveUtility>();
    match save.save(
        "final",
        &format!("level={},gold={}", player.level, player.gold),
    ) {
        Ok(path) => {
            println!("  存档目录  {}", save.directory().display());
            println!("  存档文件  {}", path.display());

            // 读回来验证一次往返
            if let Ok(content) = save.load("final") {
                println!("  存档内容  {content}");
            }
        }
        Err(error) => println!("  存档失败  {error}"),
    }

    println!("══════════════════════════════════════════════");
}

fn main() {
    let mut app = App::new();

    app.add_plugins(MinimalPlugins)
        // 1. 安装架构：注册所有 Model / System / Utility
        .install_architecture::<MiniGame>()
        // 2. 把 QFramework 事件桥接成 Bevy 消息
        .bridge_messages::<EnemyDefeatedEvent>()
        // 3. 表现层使用 Bevy 系统，先执行输入，再刷新 HUD
        .init_resource::<HudState>()
        .add_systems(Startup, setup_hud)
        .add_systems(Update, (run_script, render_hud, battle_report).chain());

    println!("════════════════ QFramework 迷你地牢 ════════════════");
    println!("  业务架构：Utility / Model / Command / Query / System，表现层使用 Bevy 系统");
    println!("  无窗口运行，剧本由 run_script 驱动");

    // 无窗口环境下手动推进帧循环
    for _ in 0..SCRIPT_FRAMES {
        app.update();
    }

    print_summary(&mut app);
    app.cleanup();
}
