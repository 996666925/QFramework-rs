//! Query：只读的数据获取（CQRS 的读侧）。

use crate::context::QueryContext;

/// 查询。查询不允许有副作用，也不能注册事件。
///
/// ```ignore
/// struct GetPlayerScoreQuery;
///
/// impl IQuery for GetPlayerScoreQuery {
///     type Result = i32;
///
///     fn do_query(&self, ctx: &QueryContext) -> Self::Result {
///         ctx.get_model::<PlayerModel>().score.get()
///     }
/// }
///
/// let score = architecture.send_query(GetPlayerScoreQuery);
/// ```
pub trait IQuery: Send + Sync + 'static {
    /// 查询结果类型。
    type Result: Send + 'static;

    /// 执行查询。
    fn do_query(&self, context: &QueryContext) -> Self::Result;
}
