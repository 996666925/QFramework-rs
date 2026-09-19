//! 查询层（`IQuery`）。
//!
//! 查询必须是纯只读的：`IQuery` 拿不到任何发送能力，连 `send_event` 都没有。
//!
//! 什么时候值得写一个 Query？当同一段取数逻辑需要在多处复用时——
//! 如果调用方本来就持有架构引用，直接 `get_model` 也完全没问题。

pub mod inventory;
pub mod player;

pub use inventory::GetInventoryQuery;
pub use player::{GetPlayerSnapshotQuery, PlayerSnapshot};
