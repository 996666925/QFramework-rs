//! 成就数据。
//!
//! 这个 Model 只负责「存成就 + 去重 + 广播」，**什么时候解锁**由
//! `AchievementSystem` 决定（那是需要读多个 Model 的业务规则）。

use qframework_core::prelude::*;

use crate::event::AchievementUnlockedEvent;

/// 成就清单。
#[derive(Default, IModel)]
pub struct AchievementModel {
    arch: ArchRef,
    /// 已解锁的成就（按解锁顺序）。
    pub unlocked: BindableList<&'static str>,
}

impl AchievementModel {
    /// 解锁一个成就；已解锁过则忽略。
    pub fn unlock(&self, title: &'static str) {
        // 先读一次判断是否已经解锁（读锁在闭包结束后释放，不会和下面的写锁重叠）
        let already = self.unlocked.with_items(|titles| titles.contains(&title));
        if already {
            return;
        }

        self.unlocked.add(title);
        self.send_event(AchievementUnlockedEvent { title });
    }

    /// 已解锁数量。
    pub fn count(&self) -> usize {
        self.unlocked.len()
    }
}
