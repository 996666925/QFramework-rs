//! 背包查询。

use qframework_core::prelude::*;

use crate::model::{InventoryModel, ItemId};

/// 读取背包内容。
///
/// 结果按物品名排序，便于显示和断言。
pub struct GetInventoryQuery;

impl IQuery for GetInventoryQuery {
    /// 物品与数量的列表。
    type Result = Vec<(ItemId, u32)>;

    fn do_query(&self, ctx: &QueryContext) -> Self::Result {
        ctx.get_model::<InventoryModel>()
            .items
            .with_entries(|entries| {
                // `with_entries` 是借用式访问，不拷贝整张表
                let mut list: Vec<(ItemId, u32)> =
                    entries.iter().map(|(item, count)| (*item, *count)).collect();
                list.sort_by_key(|(item, _)| item.label());
                list
            })
    }
}
