//! Command：唯一允许改变应用状态的地方（CQRS 的写侧）。

use crate::context::CommandContext;

/// 命令。
///
/// QFramework 规定 Command **不能持有状态**（不保存字段），因此实现体通常写成
/// 单元结构体。`Output` 表示命令执行结果，无返回值时写 `type Output = ();`。
///
/// ```ignore
/// struct PurchaseItemCommand;
///
/// impl ICommand for PurchaseItemCommand {
///     type Output = ();
///
///     fn execute(&self, ctx: &CommandContext) {
///         let shop = ctx.get_model::<ShopModel>();
///         shop.currency.set(shop.currency.get() - 10);
///         ctx.send_event(CurrencyChangedEvent { currency: shop.currency.get() });
///     }
/// }
/// ```
pub trait ICommand: Send + Sync + 'static {
    /// 执行结果类型。
    type Output: Send + 'static;

    /// 执行命令。
    fn execute(&self, context: &CommandContext) -> Self::Output;
}
