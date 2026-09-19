//! 架构与 Bevy `App` 的接入。

use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;

use bevy::prelude::*;
use qframework_core::{Architecture, ArchitectureBuilder};

/// 用类型描述应用的 QFramework 架构。
///
/// 一个类型只需要描述「注册了哪些 Model / System / Utility」，
/// 架构的创建、初始化时机由 [`QFrameworkPlugin`] 负责。
pub trait QApplication: Send + Sync + 'static {
    /// 构建架构（在这里注册 Model / System / Utility）。
    fn build() -> ArchitectureBuilder;
}

/// Bevy 世界中的 QFramework 架构资源。
///
/// 通过 [`Deref`] 可以像使用 [`Architecture`] 一样使用它：
///
/// ```ignore
/// fn my_system(architecture: Res<QArchitecture>) {
///     architecture.send_command(IncreaseCountCommand);
///     let count = architecture.get_model::<CounterModel>().count.get();
/// }
/// ```
#[derive(Resource, Clone, Debug)]
pub struct QArchitecture(pub Arc<Architecture>);

impl QArchitecture {
    /// 取得共享句柄，便于把架构传递到非系统的代码中。
    pub fn arc(&self) -> Arc<Architecture> {
        Arc::clone(&self.0)
    }
}

impl Deref for QArchitecture {
    type Target = Architecture;

    fn deref(&self) -> &Architecture {
        &self.0
    }
}

impl From<Arc<Architecture>> for QArchitecture {
    fn from(architecture: Arc<Architecture>) -> Self {
        Self(architecture)
    }
}

/// 把 QFramework 架构安装进 Bevy 世界。
///
/// 插件会在 `build` 阶段完成架构的创建与两阶段初始化，
/// 并注册控制器调度系统。
pub struct QFrameworkPlugin<A: QApplication> {
    _application: PhantomData<fn() -> A>,
}

impl<A: QApplication> Default for QFrameworkPlugin<A> {
    fn default() -> Self {
        Self {
            _application: PhantomData,
        }
    }
}

impl<A: QApplication> QFrameworkPlugin<A> {
    /// 创建插件。
    pub fn new() -> Self {
        Self::default()
    }
}

impl<A: QApplication> Plugin for QFrameworkPlugin<A> {
    fn build(&self, app: &mut App) {
        assert!(
            !app.world().contains_resource::<QArchitecture>(),
            "只能安装一个 QFrameworkPlugin：`QArchitecture` 资源已存在"
        );

        let architecture = A::build().build();
        app.insert_resource(QArchitecture(architecture));
        app.init_resource::<crate::controller::QControllers>();
        app.add_systems(
            Update,
            crate::controller::run_controllers.in_set(crate::controller::QFrameworkSet::Controllers),
        );
    }

    fn cleanup(&self, app: &mut App) {
        if let Some(controllers) = app
            .world_mut()
            .get_resource_mut::<crate::controller::QControllers>()
        {
            controllers.deinit_all();
        }

        if let Some(architecture) = app.world().get_resource::<QArchitecture>() {
            architecture.deinit();
        }
    }
}

/// 为 [`App`] 提供 QFramework 相关的便捷方法。
pub trait AppQFrameworkExt {
    /// 安装 [`QFrameworkPlugin`]。
    ///
    /// ```ignore
    /// app.add_qframework::<MyApp>();
    /// ```
    fn add_qframework<A: QApplication>(&mut self) -> &mut Self;

    /// 把 QFramework 事件桥接成 Bevy 的 [`Message`]。
    ///
    /// 必须在 [`add_qframework`](AppQFrameworkExt::add_qframework) 之后调用。
    fn bridge_q_messages<M>(&mut self) -> &mut Self
    where
        M: Message + Clone;

    /// 注册一个每帧被驱动的控制器。
    ///
    /// 控制器会被注入架构引用，并在 [`QFrameworkSet::Controllers`](crate::QFrameworkSet::Controllers) 中执行。
    fn add_q_controller<C>(&mut self, controller: C) -> &mut Self
    where
        C: qframework_core::IController + crate::controller::QControllerUpdate;

    /// 取得架构句柄（需要先 [`add_qframework`](AppQFrameworkExt::add_qframework)）。
    fn q_architecture(&self) -> Arc<Architecture>;
}

impl AppQFrameworkExt for App {
    fn add_qframework<A: QApplication>(&mut self) -> &mut Self {
        self.add_plugins(QFrameworkPlugin::<A>::default())
    }

    fn bridge_q_messages<M>(&mut self) -> &mut Self
    where
        M: Message + Clone,
    {
        self.add_plugins(crate::bridge::QEventBridgePlugin::<M>::default())
    }

    fn add_q_controller<C>(&mut self, controller: C) -> &mut Self
    where
        C: qframework_core::IController + crate::controller::QControllerUpdate,
    {
        let architecture = self.q_architecture();

        if !self.world().contains_resource::<crate::controller::QControllers>() {
            self.init_resource::<crate::controller::QControllers>();
        }

        let controller = architecture.attach_controller(controller);

        // `QControllers::add` 会负责调用 `IController::init` 并登记 deinit
        self.world_mut()
            .resource_mut::<crate::controller::QControllers>()
            .add(controller);

        self
    }

    fn q_architecture(&self) -> Arc<Architecture> {
        self.world()
            .get_resource::<QArchitecture>()
            .expect("未找到 QArchitecture 资源：请先调用 App::add_qframework::<A>()")
            .arc()
    }
}
