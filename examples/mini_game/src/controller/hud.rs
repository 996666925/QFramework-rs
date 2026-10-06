//! HUD 表现层系统：订阅状态变化，每帧最多刷新一次。

use std::cell::Cell;
use std::rc::Rc;

use bevy::prelude::*;
use qframework_bevy::prelude::*;

use crate::event::{BattleStartedEvent, GoldChangedEvent, HpChangedEvent};
use crate::model::PlayerModel;
use crate::query::{GetInventoryQuery, GetPlayerSnapshotQuery};

#[derive(Default)]
pub struct HudState {
    subscriptions: IUnRegisterList,
    dirty: Rc<Cell<bool>>,
    last_rendered: String,
}

pub fn setup_hud(architecture: NonSend<QArchitecture>, mut hud: NonSendMut<HudState>) {
    let player = architecture.get_model::<PlayerModel>();

    let dirty = Rc::clone(&hud.dirty);
    hud.subscriptions
        .add(player.hp.register_with_init_value(move |_| {
            dirty.set(true);
        }));

    let dirty = Rc::clone(&hud.dirty);
    hud.subscriptions
        .add(player.gold.register_with_init_value(move |_| {
            dirty.set(true);
        }));

    let dirty = Rc::clone(&hud.dirty);
    hud.subscriptions
        .add(player.level.register_with_init_value(move |_| {
            dirty.set(true);
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

pub fn render_hud(architecture: NonSend<QArchitecture>, mut hud: NonSendMut<HudState>) {
    if !hud.dirty.replace(false) {
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
        app.insert_non_send(QArchitecture::from(Rc::clone(&architecture)))
            .init_non_send::<HudState>()
            .add_systems(Startup, setup_hud);
        app.update();

        let dirty = Rc::clone(&app.world().non_send::<HudState>().dirty);
        assert!(dirty.replace(false));
        player.hp.set(90);
        assert!(dirty.replace(false));
        assert!(architecture.events().has_listener::<HpChangedEvent>());

        drop(app.world_mut().remove_non_send::<HudState>());
        player.hp.set(80);
        assert!(!dirty.get());
        assert!(!architecture.events().has_listener::<HpChangedEvent>());
        assert!(!architecture.events().has_listener::<GoldChangedEvent>());
        assert!(!architecture.events().has_listener::<BattleStartedEvent>());
    }
}
