//! 玩家数据。
//!
//! 这个 Model 展示了数据层的标准形态：**数据 + 它的 CRUD + 变更通知**。
//! 所有写操作都用 `modify`（原子的读-改-写），改完立刻广播事件。

use qframework_core::prelude::*;

use crate::event::{GoldChangedEvent, HpChangedEvent, LevelUpEvent};

/// 升到下一级所需的经验。
fn exp_needed(level: u32) -> i32 {
    level as i32 * 50
}

/// 玩家核心数据。
///
/// 注意这里**没有**任何跨 Model 的规则——「金币够不够买某件商品」这类判断
/// 属于 Command，因为那需要同时看商店和背包。
#[derive(IModel)]
pub struct PlayerModel {
    arch: ArchRef,
    /// 当前血量。
    pub hp: BindableProperty<i32>,
    /// 最大血量。
    pub max_hp: BindableProperty<i32>,
    /// 金币。
    pub gold: BindableProperty<i32>,
    /// 等级。
    pub level: BindableProperty<u32>,
    /// 当前经验（升级后会扣除对应消耗）。
    pub exp: BindableProperty<i32>,
    /// 攻击力。
    pub attack: BindableProperty<i32>,
}

impl PlayerModel {
    /// 创建初始状态的玩家。
    ///
    /// 没有派生 `Default`——初始值不是「零值」，用一个具名构造函数更清楚。
    pub fn new_player() -> Self {
        Self {
            arch: ArchRef::new(),
            hp: BindableProperty::new(100),
            max_hp: BindableProperty::new(100),
            gold: BindableProperty::new(100),
            level: BindableProperty::new(1),
            exp: BindableProperty::new(0),
            attack: BindableProperty::new(12),
        }
    }

    /// 是否存活。
    pub fn is_alive(&self) -> bool {
        self.hp.get() > 0
    }

    /// 受伤，返回是否因此死亡。
    pub fn take_damage(&self, amount: i32) -> bool {
        self.hp.modify(|hp| *hp = (*hp - amount).max(0));
        self.publish_hp();
        !self.is_alive()
    }

    /// 治疗，返回实际恢复量（会被最大血量截断）。
    pub fn heal(&self, amount: i32) -> i32 {
        let before = self.hp.get();
        let max_hp = self.max_hp.get();

        self.hp.modify(|hp| *hp = (*hp + amount).min(max_hp));
        self.publish_hp();

        self.hp.get() - before
    }

    /// 增加金币。
    pub fn add_gold(&self, delta: i32) {
        self.gold.modify(|gold| *gold += delta);
        self.send_event(GoldChangedEvent {
            gold: self.gold.get(),
            delta,
        });
    }

    /// 扣除金币。余额不足时**不改变任何状态**并返回 `false`。
    ///
    /// 判断与扣减在同一个写锁内完成，所以是原子的——即使多个线程同时调用
    /// 也不会出现「扣成负数」。
    pub fn spend_gold(&self, amount: i32) -> bool {
        let mut paid = false;

        self.gold.modify(|gold| {
            if *gold >= amount {
                *gold -= amount;
                paid = true;
            }
        });

        if paid {
            self.send_event(GoldChangedEvent {
                gold: self.gold.get(),
                delta: -amount,
            });
        }

        paid
    }

    /// 增加经验，返回本次是否升级。
    pub fn gain_exp(&self, amount: i32) -> bool {
        self.exp.modify(|exp| *exp += amount);

        let mut leveled = false;
        while self.exp.get() >= exp_needed(self.level.get()) {
            let needed = exp_needed(self.level.get());
            self.exp.modify(|exp| *exp -= needed);
            self.level.modify(|level| *level += 1);
            leveled = true;
        }

        if leveled {
            // 升级奖励：最大血量 +20、攻击力 +3，并立即恢复 20 点
            let max_hp = self.max_hp.get() + 20;
            self.max_hp.set(max_hp);
            self.attack.modify(|attack| *attack += 3);
            self.hp.modify(|hp| *hp = (*hp + 20).min(max_hp));

            self.send_event(LevelUpEvent {
                level: self.level.get(),
                max_hp,
            });
            self.publish_hp();
        }

        leveled
    }

    /// 血量变化后统一广播。
    fn publish_hp(&self) {
        self.send_event(HpChangedEvent {
            hp: self.hp.get(),
            max_hp: self.max_hp.get(),
        });
    }
}
