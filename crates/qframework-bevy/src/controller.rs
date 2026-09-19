//! 控制器调度：让 QFramework 的 IController 能随 Bevy 帧循环运行。

use std::sync::Arc;
use std::time::Duration;

use bevy::prelude::*;
use qframework_core::IController;

/// QFramework 在 Bevy 中使用的系统集合。
///
/// ```ignore
/// app.add_systems(Update, my_system.after(QFrameworkSet::Controllers));
/// ```
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum QFrameworkSet {
    /// 所有通过 `App::add_q_controller` 注册的控制器都在这里更新。
    Controllers,
}

/// 由 Bevy 每帧驱动的控制器。
///
/// ```ignore
/// #[derive(Default, IController)]
/// struct CounterController {
///     arch: ArchRef,
///     frames: AtomicI32,
/// }
///
/// impl QControllerUpdate for CounterController {
///     fn update(&self, _delta: Duration) {
///         self.send_command(IncreaseCountCommand);
///     }
/// }
/// ```
pub trait QControllerUpdate: Send + Sync + 'static {
    /// 每帧调用一次。
    fn update(&self, delta: Duration);
}

/// 一个已注册的控制器：更新入口 + 退出时的清理回调。
struct QControllerEntry {
    update: Arc<dyn QControllerUpdate>,
    deinit: Box<dyn Fn() + Send + Sync>,
}

impl std::fmt::Debug for QControllerEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QControllerEntry").finish_non_exhaustive()
    }
}

/// 已注册的控制器集合。
#[derive(Resource, Default)]
pub struct QControllers {
    entries: Vec<QControllerEntry>,
}

impl QControllers {
    /// 创建空的控制器集合。
    pub fn new() -> Self {
        Self::default()
    }

    /// 加入一个控制器，并立即调用 [`IController::init`]。
    ///
    /// 一般通过 `App::add_q_controller` 调用。传入的控制器必须**已经绑定架构**
    /// （见 [`Architecture::attach_controller`](qframework_core::Architecture::attach_controller)），
    /// 否则 `init` 中访问架构时会 panic。
    ///
    /// 这里会一并登记 [`IController::deinit`] 的清理回调，使控制器与
    /// System / Model 一样随应用退出被正确清理。
    pub fn add<C>(&mut self, controller: Arc<C>)
    where
        C: IController + QControllerUpdate,
    {
        IController::init(controller.as_ref());

        let shared = Arc::clone(&controller);
        let update: Arc<dyn QControllerUpdate> = shared;

        self.entries.push(QControllerEntry {
            update,
            deinit: Box::new(move || IController::deinit(controller.as_ref())),
        });
    }

    /// 控制器数量。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 是否没有控制器。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 遍历所有控制器。
    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn QControllerUpdate>> {
        self.entries.iter().map(|entry| &entry.update)
    }

    /// 依次调用所有控制器的 [`IController::deinit`]。
    ///
    /// 由 `QFrameworkPlugin` 在应用退出时调用，也可以手动调用。
    pub fn deinit_all(&self) {
        for entry in &self.entries {
            (entry.deinit)();
        }
    }

    /// 清空控制器集合（不会调用 `deinit`，需要清理请先调用 [`deinit_all`](Self::deinit_all)）。
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl std::fmt::Debug for QControllers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QControllers")
            .field("len", &self.entries.len())
            .finish()
    }
}

/// 每帧驱动所有控制器。
pub(crate) fn run_controllers(time: Res<Time>, controllers: Res<QControllers>) {
    let delta = time.delta();

    for controller in controllers.iter() {
        controller.update(delta);
    }
}
