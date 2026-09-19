//! 数据层（`IModel`）。
//!
//! Model 的职责边界：
//!
//! - **是**：数据定义、数据的增删改查、数据变化时通过事件通知外界
//! - **不是**：跨 Model 的业务规则（`IModel` 拿不到别的 Model，这是编译期约束）
//! - **不是**：访问 System / Controller（`IModel` 没有对应能力接口）

pub mod achievement;
pub mod battle;
pub mod inventory;
pub mod item;
pub mod player;
pub mod shop;

pub use achievement::AchievementModel;
pub use battle::{BattleModel, Enemy};
pub use inventory::InventoryModel;
pub use item::ItemId;
pub use player::PlayerModel;
pub use shop::{ShopError, ShopModel};
