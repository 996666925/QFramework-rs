//! 命令层（`ICommand`）。
//!
//! 命令是**唯一**允许改变应用状态的地方，并且不能持有状态（字段只能是本次调用的输入）。
//!
//! 命名约定：动词 + 名词 + `Command`。
//!
//! 搜索 `impl ICommand for` 就能列出这个游戏所有的写操作——这就是把状态变更
//! 收拢到命令里的价值。

pub mod battle;
pub mod item;
pub mod session;
pub mod shop;

pub use battle::{AttackOutcome, PlayerAttackCommand, StartBattleCommand};
pub use item::UseItemCommand;
pub use session::StartGameCommand;
pub use shop::BuyItemCommand;
