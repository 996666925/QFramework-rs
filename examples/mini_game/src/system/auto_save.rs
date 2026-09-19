//! 自动存档系统：升级时把状态落盘。

use std::sync::Mutex;

use qframework_core::prelude::*;

use crate::event::LevelUpEvent;
use crate::model::PlayerModel;
use crate::utility::{LogUtility, SaveUtility};

/// 自动存档系统。
///
/// System 是唯一**既能读 Model 又能使用 Utility** 的业务层，所以「把游戏状态
/// 落盘」这类事最适合放在这里。表现层做不到（`IController` 没有 Utility 能力），
/// 命令也做不到（`CommandContext` 同样没有）。
#[derive(Default, ISystem)]
#[system(init = Self::subscribe)]
pub struct AutoSaveSystem {
    arch: ArchRef,
    subscriptions: Mutex<IUnRegisterList>,
}

impl AutoSaveSystem {
    fn subscribe(&self) {
        // 回调必须是 'static，所以先取出 'static 的架构句柄
        let architecture = self.architecture();

        let un = self.register_event::<LevelUpEvent, _>(move |event| {
            let player = architecture.get_model::<PlayerModel>();
            let save = architecture.get_utility::<SaveUtility>();
            let log = architecture.get_utility::<LogUtility>();

            let payload = format!(
                "level={},gold={},max_hp={}",
                event.level,
                player.gold.get(),
                event.max_hp
            );

            match save.save("autosave", &payload) {
                Ok(path) => log.info(format_args!("自动存档 -> {}", path.display())),
                Err(error) => log.warn(format_args!("自动存档失败：{error}")),
            }
        });

        self.subscriptions.lock().unwrap().add(un);
    }
}
