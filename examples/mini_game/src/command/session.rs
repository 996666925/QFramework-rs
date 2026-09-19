//! 会话相关命令。

use qframework_core::prelude::*;

use crate::event::GameStartedEvent;

/// 开始游戏：广播开始事件。
///
/// 为什么不是 Controller 直接发事件？因为 `IController` **没有**发送事件的能力
/// （它只能注册事件）。表现层负责表达意图，广播由命令完成——这正是分层规则想要的。
pub struct StartGameCommand;

impl ICommand for StartGameCommand {
    type Output = ();

    fn execute(&self, ctx: &CommandContext) {
        ctx.send_event(GameStartedEvent);
    }
}
