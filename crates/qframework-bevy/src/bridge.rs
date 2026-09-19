//! 事件桥接：把 QFramework 的事件转发成 Bevy 的消息。
//!
//! Bevy 的缓冲事件（现在叫 `Message`）只能在系统中写入，而 QFramework
//! 的事件可能在 Command / Model 的任意一层被触发。桥接的做法是：
//!
//! 1. 监听 QFramework 事件 `M`，把事件推进一个跨线程队列；
//! 2. 每帧在 `PreUpdate` 把队列里的内容写入 Bevy 的 `Messages<M>`；
//! 3. 其他系统照常使用 `MessageReader<M>`。
//!
//! ```ignore
//! app.bridge_q_messages::<CountChangedMessage>();
//!
//! fn on_count_changed(mut reader: MessageReader<CountChangedMessage>) {
//!     for message in reader.read() {
//!         println!("count = {}", message.count);
//!     }
//! }
//! ```

use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;

use crate::app::QArchitecture;

/// 存放「等待转发到 Bevy 的 QFramework 事件」的队列。
#[derive(Resource)]
pub struct QEventBridge<M: Message + Clone> {
    queue: Arc<Mutex<Vec<M>>>,
}

impl<M: Message + Clone> Default for QEventBridge<M> {
    fn default() -> Self {
        Self {
            queue: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl<M: Message + Clone> QEventBridge<M> {
    /// 推入一个待转发事件。
    pub fn push(&self, message: M) {
        self.queue.lock().unwrap().push(message);
    }

    /// 取出全部待转发事件。
    pub fn drain(&self) -> Vec<M> {
        std::mem::take(&mut self.queue.lock().unwrap())
    }

    /// 当前积压数量。
    pub fn len(&self) -> usize {
        self.queue.lock().unwrap().len()
    }

    /// 队列是否为空。
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 取得队列的共享句柄（供事件监听器使用）。
    pub fn queue(&self) -> Arc<Mutex<Vec<M>>> {
        Arc::clone(&self.queue)
    }
}

impl<M: Message + Clone> std::fmt::Debug for QEventBridge<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QEventBridge")
            .field("pending", &self.len())
            .finish()
    }
}

/// 把某个 QFramework 事件类型桥接为同名的 Bevy 消息。
///
/// 必须添加在 `QFrameworkPlugin` 之后。
pub struct QEventBridgePlugin<M: Message + Clone> {
    _message: PhantomData<fn() -> M>,
}

impl<M: Message + Clone> Default for QEventBridgePlugin<M> {
    fn default() -> Self {
        Self {
            _message: PhantomData,
        }
    }
}

impl<M: Message + Clone> QEventBridgePlugin<M> {
    /// 创建桥接插件。
    pub fn new() -> Self {
        Self::default()
    }
}

impl<M: Message + Clone> Plugin for QEventBridgePlugin<M> {
    fn build(&self, app: &mut App) {
        let architecture = app
            .world()
            .get_resource::<QArchitecture>()
            .expect("QEventBridgePlugin 必须在 QFrameworkPlugin 之后添加")
            .arc();

        let bridge = QEventBridge::<M>::default();
        let queue = bridge.queue();

        // QFramework 事件 -> 队列
        architecture.register_event::<M, _>(move |event| {
            queue.lock().unwrap().push(event.clone());
        });

        app.add_message::<M>();
        app.insert_resource(bridge);
        app.add_systems(PreUpdate, forward_q_messages::<M>);
    }
}

/// 把队列中的事件写入 Bevy 的 `Messages<M>`。
pub(crate) fn forward_q_messages<M: Message + Clone>(
    bridge: Res<QEventBridge<M>>,
    mut writer: MessageWriter<M>,
) {
    for message in bridge.drain() {
        writer.write(message);
    }
}
