//! 物品使用相关命令。

use qframework_core::prelude::*;

use crate::model::{InventoryModel, ItemId, PlayerModel};

/// 使用物品可能出现的错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UseItemError {
    /// 该物品不能使用。
    NotUsable,
    /// 背包里没有这件物品。
    NotOwned,
}

impl std::fmt::Display for UseItemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UseItemError::NotUsable => write!(f, "该物品无法使用"),
            UseItemError::NotOwned => write!(f, "背包里没有这件物品"),
        }
    }
}

/// 使用物品。
pub struct UseItemCommand {
    /// 要使用的物品。
    pub item: ItemId,
}

impl ICommand for UseItemCommand {
    /// 成功时返回**实际生效量**（例如实际恢复的血量，可能被上限截断）。
    type Output = Result<i32, UseItemError>;

    fn execute(&self, ctx: &CommandContext) -> Self::Output {
        let inventory = ctx.get_model::<InventoryModel>();
        let player = ctx.get_model::<PlayerModel>();

        let heal = self.item.heal_amount();
        if heal == 0 {
            return Err(UseItemError::NotUsable);
        }

        // 先扣物品再治疗：没有物品就不该产生任何效果
        if !inventory.consume(self.item) {
            return Err(UseItemError::NotOwned);
        }

        // 这里不需要再广播事件：`PlayerModel::heal` 已经发出了 `HpChangedEvent`。
        // 「一次状态变化只广播一次」是保持事件流清晰的关键。
        Ok(player.heal(heal))
    }
}
