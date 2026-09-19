//! 背包数据。
//!
//! 用 `BindableDictionary` 存「物品 -> 数量」，方便演示集合的增删改事件。

use qframework_core::prelude::*;

use crate::model::ItemId;

/// 玩家背包。
#[derive(Default, IModel)]
pub struct InventoryModel {
    arch: ArchRef,
    /// 物品与数量。数量为 0 的物品不会留在字典里。
    pub items: BindableDictionary<ItemId, u32>,
}

impl InventoryModel {
    /// 增加物品数量。
    pub fn add(&self, item: ItemId, quantity: u32) {
        if quantity == 0 {
            return;
        }

        let current = self.count(item);
        self.items.insert(item, current + quantity);
    }

    /// 消耗一个物品；数量不足时返回 `false` 且不改变状态。
    pub fn consume(&self, item: ItemId) -> bool {
        let current = self.count(item);
        if current == 0 {
            return false;
        }

        if current == 1 {
            self.items.remove(&item);
        } else {
            self.items.insert(item, current - 1);
        }

        true
    }

    /// 某物品的数量。
    pub fn count(&self, item: ItemId) -> u32 {
        self.items.get(&item).unwrap_or(0)
    }
}
