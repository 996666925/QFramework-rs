//! 物品定义。
//!
//! 这里只有「物品是什么」这类静态信息；**价格**属于游戏数据，放在 `ShopModel` 里。

/// 物品标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ItemId {
    /// 治疗药水：使用后恢复 30 点血量。
    #[default]
    HealthPotion,
    /// 铁剑：提升攻击力（本示例中暂未实现装备逻辑）。
    IronSword,
    /// 木盾：提升防御（本示例中暂未实现装备逻辑）。
    WoodenShield,
}

impl ItemId {
    /// 用于显示的名字。
    pub const fn label(self) -> &'static str {
        match self {
            ItemId::HealthPotion => "治疗药水",
            ItemId::IronSword => "铁剑",
            ItemId::WoodenShield => "木盾",
        }
    }

    /// 使用后恢复的血量；非消耗品返回 0，表示「不可使用」。
    pub const fn heal_amount(self) -> i32 {
        match self {
            ItemId::HealthPotion => 30,
            ItemId::IronSword | ItemId::WoodenShield => 0,
        }
    }
}
