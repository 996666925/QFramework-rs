//! 会话相关命令。

use qframework_core::prelude::*;

use crate::event::GameStartedEvent;

/// 开始游戏：广播开始事件。
///
/// 表现层负责表达意图，事件广播由命令完成，业务流程不依赖 Bevy。
pub struct StartGameCommand;

impl ICommand for StartGameCommand {
    type Output = ();

    fn execute(&self, ctx: &CommandContext) {
        ctx.send_event(GameStartedEvent);
    }
}
