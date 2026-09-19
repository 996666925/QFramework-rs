//! 商店相关命令。

use qframework_core::prelude::*;

use crate::event::ItemPurchasedEvent;
use crate::model::{InventoryModel, ItemId, PlayerModel, ShopError, ShopModel};

/// 购买物品。
///
/// 返回值是「调用方立刻需要知道的结果」；其它模块可能需要知道的变化用事件广播。
/// 判断标准是**谁需要知道**：只有发起者关心 → `Output`；别的模块也关心 → 事件。
pub struct BuyItemCommand {
    /// 要买的物品。
    pub item: ItemId,
    /// 数量。
    pub quantity: u32,
}

impl ICommand for BuyItemCommand {
    /// 成功时返回总花费。
    type Output = Result<i32, ShopError>;

    fn execute(&self, ctx: &CommandContext) -> Self::Output {
        let shop = ctx.get_model::<ShopModel>();
        let player = ctx.get_model::<PlayerModel>();
        let inventory = ctx.get_model::<InventoryModel>();

        // 1) 商品必须在售
        let unit_price = shop.price_of(self.item).ok_or(ShopError::NotForSale)?;
        let total = unit_price * self.quantity as i32;

        // 2) 扣钱。`spend_gold` 是原子的：余额不足时不会发生任何改变
        if !player.spend_gold(total) {
            return Err(ShopError::NotEnoughGold {
                needed: total,
                owned: player.gold.get(),
            });
        }

        // 3) 发货并广播
        inventory.add(self.item, self.quantity);
        ctx.send_event(ItemPurchasedEvent {
            item: self.item,
            quantity: self.quantity,
            cost: total,
        });

        Ok(total)
    }
}
