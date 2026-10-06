//! 架构与 Bevy `App` 的接入。

use std::marker::PhantomData;
use std::ops::Deref;
use std::rc::Rc;

use bevy::prelude::*;
use qframework_core::Architecture;
pub use qframework_core::QApplication;

/// Bevy 世界中的 QFramework 架构资源。
///
/// 作为 NonSend 数据安装；使用它的系统在主线程运行。
///
/// 通过 [`Deref`] 可以像使用 [`Architecture`] 一样使用它：
///
/// ```ignore
/// fn my_system(architecture: NonSend<QArchitecture>) {
///     architecture.send_command(IncreaseCountCommand);
///     let count = architecture.get_model::<CounterModel>().count.get();
/// }
/// ```
#[derive(Clone, Debug)]
pub struct QArchitecture(pub Rc<Architecture>);

impl QArchitecture {
    /// 取得共享句柄，便于把架构传递到非系统的代码中。
    pub fn rc(&self) -> Rc<Architecture> {
        Rc::clone(&self.0)
    }
}

impl Deref for QArchitecture {
    type Target = Architecture;

    fn deref(&self) -> &Architecture {
        &self.0
    }
}

impl From<Rc<Architecture>> for QArchitecture {
    fn from(architecture: Rc<Architecture>) -> Self {
        Self(architecture)
    }
}

/// 把 QFramework 架构安装进 Bevy 世界。
///
/// 插件会在 `build` 阶段完成架构的创建与两阶段初始化。
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
            !app.world().contains_non_send::<QArchitecture>(),
            "只能安装一个 QFrameworkPlugin：`QArchitecture` 资源已存在"
        );

        let architecture = A::build().build();
        app.insert_non_send(QArchitecture(architecture));
    }

    fn cleanup(&self, app: &mut App) {
        if let Some(architecture) = app.world().get_non_send::<QArchitecture>() {
            architecture.deinit();
        }
    }
}

/// 为 [`App`] 提供 QFramework 相关的便捷方法。
pub trait AppQFrameworkExt {
    /// 创建并初始化应用架构，通过 [`QFrameworkPlugin`] 安装到 Bevy 世界。
    ///
    /// ```ignore
    /// app.install_architecture::<MyApp>();
    /// ```
    fn install_architecture<A: QApplication>(&mut self) -> &mut Self;

    /// 把 QFramework 事件桥接成 Bevy 的 [`Message`]。
    ///
    /// 必须在 [`install_architecture`](AppQFrameworkExt::install_architecture) 之后调用。
    fn bridge_messages<M>(&mut self) -> &mut Self
    where
        M: Message + Clone;

    /// 取得架构句柄（需要先 [`install_architecture`](AppQFrameworkExt::install_architecture)）。
    fn architecture(&self) -> Rc<Architecture>;
}

impl AppQFrameworkExt for App {
    fn install_architecture<A: QApplication>(&mut self) -> &mut Self {
        self.add_plugins(QFrameworkPlugin::<A>::default())
    }

    fn bridge_messages<M>(&mut self) -> &mut Self
    where
        M: Message + Clone,
    {
        self.add_plugins(crate::bridge::QEventBridgePlugin::<M>::default())
    }

    fn architecture(&self) -> Rc<Architecture> {
        self.world()
            .get_non_send::<QArchitecture>()
            .expect("未找到 QArchitecture 资源：请先调用 App::install_architecture::<A>()")
            .rc()
    }
}
