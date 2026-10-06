//! HUD 表现层系统：订阅状态变化，每帧最多刷新一次。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;
use qframework_bevy::prelude::*;

use crate::event::{BattleStartedEvent, GoldChangedEvent, HpChangedEvent};
use crate::model::PlayerModel;
use crate::query::{GetInventoryQuery, GetPlayerSnapshotQuery};

#[derive(Resource, Default)]
pub struct HudState {
    subscriptions: IUnRegisterList,
    dirty: Arc<AtomicBool>,
    last_rendered: String,
}

pub fn setup_hud(architecture: Res<QArchitecture>, mut hud: ResMut<HudState>) {
    let player = architecture.get_model::<PlayerModel>();

    let dirty = Arc::clone(&hud.dirty);
    hud.subscriptions
        .add(player.hp.register_with_init_value(move |_| {
            dirty.store(true, Ordering::SeqCst);
        }));

    let dirty = Arc::clone(&hud.dirty);
    hud.subscriptions
        .add(player.gold.register_with_init_value(move |_| {
            dirty.store(true, Ordering::SeqCst);
        }));

    let dirty = Arc::clone(&hud.dirty);
    hud.subscriptions
        .add(player.level.register_with_init_value(move |_| {
            dirty.store(true, Ordering::SeqCst);
        }));

    hud.subscriptions
        .add(architecture.register_event::<HpChangedEvent, _>(|event| {
            println!("  [HUD·事件] 血量 -> {}/{}", event.hp, event.max_hp);
        }));
    hud.subscriptions
        .add(architecture.register_event::<GoldChangedEvent, _>(|event| {
            let sign = if event.delta >= 0 { "+" } else { "" };
            println!("  [HUD·事件] 金币 {sign}{} -> {}", event.delta, event.gold);
        }));
    hud.subscriptions.add(
        architecture.register_event::<BattleStartedEvent, _>(|event| {
            println!("  [HUD·事件] 遭遇 {}（{} HP）", event.enemy, event.enemy_hp);
        }),
    );
}

pub fn render_hud(architecture: Res<QArchitecture>, mut hud: ResMut<HudState>) {
    if !hud.dirty.swap(false, Ordering::SeqCst) {
        return;
    }

    let player = architecture.send_query(GetPlayerSnapshotQuery);
    let items = architecture.send_query(GetInventoryQuery);
    let bag: u32 = items.iter().map(|(_, count)| count).sum();
    let line = format!(
        "  [HUD] HP {}/{}  |  金币 {}  |  等级 {}  |  背包 {} 件",
        player.hp, player.max_hp, player.gold, player.level, bag
    );
    if hud.last_rendered != line {
        println!("{line}");
        hud.last_rendered = line;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removing_hud_resource_unregisters_subscriptions() {
        let architecture = ArchitectureBuilder::new("HudTest")
            .model(PlayerModel::new_player())
            .build();
        let player = architecture.get_model::<PlayerModel>();
        let mut app = App::new();
        app.insert_resource(QArchitecture::from(Arc::clone(&architecture)))
            .init_resource::<HudState>()
            .add_systems(Startup, setup_hud);
        app.update();

        let dirty = Arc::clone(&app.world().resource::<HudState>().dirty);
        assert!(dirty.swap(false, Ordering::SeqCst));
        player.hp.set(90);
        assert!(dirty.swap(false, Ordering::SeqCst));
        assert!(architecture.events().has_listener::<HpChangedEvent>());

        drop(app.world_mut().remove_resource::<HudState>());
        player.hp.set(80);
        assert!(!dirty.load(Ordering::SeqCst));
        assert!(!architecture.events().has_listener::<HpChangedEvent>());
        assert!(!architecture.events().has_listener::<GoldChangedEvent>());
        assert!(!architecture.events().has_listener::<BattleStartedEvent>());
    }
}
