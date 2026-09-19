//! 成就系统：监听各模块的事件，决定解锁哪些成就。

use std::sync::{Arc, Mutex};

use qframework_core::prelude::*;

use crate::event::{
    AchievementUnlockedEvent, EnemyDefeatedEvent, GameStartedEvent, ItemPurchasedEvent, LevelUpEvent,
};
use crate::model::{AchievementModel, ItemId};
use crate::utility::LogUtility;

/// 成就系统。
///
/// 为什么成就判断放在 System 而不是 Model？因为它需要**同时**看
/// 「发生了什么事件」和「要改哪个 Model」，`IModel` 拿不到别的 Model，
/// 这正是分层规则在提示你「这是业务逻辑」。
///
/// 订阅回调必须是 `'static` 的，**不能捕获 `&self`**。需要访问 Model 时，
/// 先把 `Arc<Architecture>` 取出来捕获进闭包。
#[derive(Default, ISystem)]
#[system(init = Self::subscribe)]
pub struct AchievementSystem {
    arch: ArchRef,
    subscriptions: Mutex<IUnRegisterList>,
}

impl AchievementSystem {
    fn subscribe(&self) {
        let architecture = self.architecture();
        let log = self.get_utility::<LogUtility>();

        // ① 只监听、不碰数据：ICanRegisterEvent 就够了
        let log_sink = Arc::clone(&log);
        self.track(self.register_event::<GameStartedEvent, _>(move |_| {
            log_sink.section("成就系统启动");
            log_sink.info("已就绪，开始监听事件");
        }));

        // ② 需要「读事件 + 改 Model」：额外捕获架构句柄
        let handle = Arc::clone(&architecture);
        self.track(self.register_event::<ItemPurchasedEvent, _>(move |event| {
            let achievements = handle.get_model::<AchievementModel>();

            if event.item == ItemId::HealthPotion && event.quantity >= 2 {
                achievements.unlock("有备无患");
            }

            // 单笔消费满 50 金币
            if event.cost >= 50 {
                achievements.unlock("大客户");
            }
        }));

        // ③ 击败敌人
        let handle = Arc::clone(&architecture);
        let log_sink = Arc::clone(&log);
        self.track(self.register_event::<EnemyDefeatedEvent, _>(move |event| {
            let achievements = handle.get_model::<AchievementModel>();
            achievements.unlock("初战告捷");

            if event.gold_reward >= 50 {
                achievements.unlock("赏金猎人");
            }

            log_sink.info(format_args!("{} 被击败了", event.name));
        }));

        // ④ 升级
        let handle = Arc::clone(&architecture);
        self.track(self.register_event::<LevelUpEvent, _>(move |event| {
            if event.level >= 2 {
                handle.get_model::<AchievementModel>().unlock("初露锋芒");
            }
        }));

        // ⑤ 自己也关心成就解锁，用来打日志
        let log_sink = Arc::clone(&log);
        self.track(
            self.register_event::<AchievementUnlockedEvent, _>(move |event| {
                log_sink.info(format_args!("解锁成就「{}」", event.title));
            }),
        );
    }

    /// 收集注销句柄，`IUnRegisterList` 在 `Drop` 时会自动清理。
    fn track(&self, unregister: IUnRegister) {
        self.subscriptions.lock().unwrap().add(unregister);
    }

    /// 已解锁的成就数量。
    ///
    /// 这是一个公开方法，表现层可以通过 `get_system::<AchievementSystem>()`
    /// 调用它（Controller 没有 Utility 能力，但可以取 System）。
    pub fn unlocked_count(&self) -> usize {
        self.get_model::<AchievementModel>().count()
    }
}
