//! 商店数据。
//!
//! 这个 Model 用 `#[model(init = Self::stock)]` 演示派生宏的生命周期钩子：
//! 商品表在架构初始化阶段装载，而不是在构造函数里。
//!
//! 两种写法都可以：
//!
//! - 需要「读别的 Model / Utility」才能初始化 → 用 `init` 钩子（那时架构已就绪）
//! - 纯静态数据 → 具名构造函数（如 `PlayerModel::new_player`）更直观

use qframework_core::prelude::*;

use crate::model::ItemId;

/// 商店操作可能出现的错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopError {
    /// 金币不足。
    NotEnoughGold {
        /// 需要的金币。
        needed: i32,
        /// 当前持有。
        owned: i32,
    },
    /// 商品不在售。
    NotForSale,
}

impl std::fmt::Display for ShopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShopError::NotEnoughGold { needed, owned } => {
                write!(f, "金币不足（需要 {needed}，只有 {owned}）")
            }
            ShopError::NotForSale => write!(f, "该商品不在售"),
        }
    }
}

/// 商店。
#[derive(Default, IModel)]
#[model(init = Self::stock)]
pub struct ShopModel {
    arch: ArchRef,
    /// 商品价格表。
    pub catalog: BindableDictionary<ItemId, i32>,
}

impl ShopModel {
    /// 上架商品（架构初始化时自动调用）。
    fn stock(&self) {
        self.catalog.insert(ItemId::HealthPotion, 30);
        self.catalog.insert(ItemId::IronSword, 80);
        self.catalog.insert(ItemId::WoodenShield, 50);
    }

    /// 查询单价。
    pub fn price_of(&self, item: ItemId) -> Option<i32> {
        self.catalog.get(&item)
    }

    /// 在售商品数量。
    pub fn listed(&self) -> usize {
        self.catalog.len()
    }
}
