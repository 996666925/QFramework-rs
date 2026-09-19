//! 中心枢纽：对应 QFramework 的 `IArchitecture` / `Architecture<T>`。
//!
//! 架构负责：
//! 1. 承载 IOC 容器与事件系统；
//! 2. 管理 System / Model / Utility 的注册与两阶段初始化；
//! 3. 作为 Command / Query 的中介。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock, Weak};

use crate::command::ICommand;
use crate::context::{CommandContext, QueryContext};
use crate::event::TypeEventSystem;
use crate::ioc::IOCContainer;
use crate::layers::{IController, IModel, ISystem, IUtility};
use crate::query::IQuery;
use crate::unregister::IUnRegister;

/// 延迟执行的生命周期回调。
type BoxedLifecycle = Box<dyn FnOnce() + Send + Sync>;

/// 构建阶段的一次注册动作。
type BoxedRegistration = Box<dyn FnOnce(&Arc<Architecture>) + Send + Sync>;

/// 应用架构。通常通过 [`ArchitectureBuilder`] 创建。
///
/// ```ignore
/// let architecture = ArchitectureBuilder::new("CounterApp")
///     .model(CounterModel::default())
///     .system(CounterSystem::default())
///     .build();
/// ```
pub struct Architecture {
    name: &'static str,
    ioc: RwLock<IOCContainer>,
    events: TypeEventSystem,
    inited: AtomicBool,
    self_ref: OnceLock<Weak<Architecture>>,
    model_inits: Mutex<Vec<BoxedLifecycle>>,
    system_inits: Mutex<Vec<BoxedLifecycle>>,
    model_deinits: Mutex<Vec<BoxedLifecycle>>,
    system_deinits: Mutex<Vec<BoxedLifecycle>>,
}

impl Architecture {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            ioc: RwLock::new(IOCContainer::new()),
            events: TypeEventSystem::new(),
            inited: AtomicBool::new(false),
            self_ref: OnceLock::new(),
            model_inits: Mutex::new(Vec::new()),
            system_inits: Mutex::new(Vec::new()),
            model_deinits: Mutex::new(Vec::new()),
            system_deinits: Mutex::new(Vec::new()),
        }
    }

    /// 架构名称。
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// 是否已经完成初始化。
    pub fn is_inited(&self) -> bool {
        self.inited.load(Ordering::SeqCst)
    }

    /// 取得架构自身的 [`Arc`] 句柄。
    ///
    /// 只有通过 [`ArchitectureBuilder::build`] 创建的架构才具备自引用。
    pub fn arc(&self) -> Arc<Architecture> {
        self.self_ref
            .get()
            .and_then(Weak::upgrade)
            .expect("Architecture 必须通过 ArchitectureBuilder::build 创建")
    }

    /// 事件系统。
    pub fn events(&self) -> &TypeEventSystem {
        &self.events
    }

    /// 已注册的层对象数量（Model + System + Utility）。
    pub fn registered_count(&self) -> usize {
        self.ioc.read().unwrap().len()
    }

    // -----------------------------------------------------------------------
    // 注册
    // -----------------------------------------------------------------------

    /// 注册 Model，返回共享句柄。
    pub fn register_model<M: IModel>(&self, model: M) -> Arc<M> {
        let model = Arc::new(model);
        model.arch_ref().set(&self.arc());
        self.ioc.write().unwrap().register_arc(model.clone());

        if self.inited.load(Ordering::SeqCst) {
            model.init();
        } else {
            let deferred = model.clone();
            self.model_inits
                .lock()
                .unwrap()
                .push(Box::new(move || deferred.init()));
        }

        let deferred = model.clone();
        self.model_deinits
            .lock()
            .unwrap()
            .push(Box::new(move || deferred.deinit()));

        model
    }

    /// 注册 System，返回共享句柄。
    pub fn register_system<S: ISystem>(&self, system: S) -> Arc<S> {
        let system = Arc::new(system);
        system.arch_ref().set(&self.arc());
        self.ioc.write().unwrap().register_arc(system.clone());

        if self.inited.load(Ordering::SeqCst) {
            system.init();
        } else {
            let deferred = system.clone();
            self.system_inits
                .lock()
                .unwrap()
                .push(Box::new(move || deferred.init()));
        }

        let deferred = system.clone();
        self.system_deinits
            .lock()
            .unwrap()
            .push(Box::new(move || deferred.deinit()));

        system
    }

    /// 注册 Utility。Utility 不需要架构引用。
    pub fn register_utility<U: IUtility>(&self, utility: U) -> Arc<U> {
        let utility = Arc::new(utility);
        self.ioc.write().unwrap().register_arc(utility.clone());

        if self.inited.load(Ordering::SeqCst) {
            utility.init();
        } else {
            let deferred = utility.clone();
            self.system_inits
                .lock()
                .unwrap()
                .push(Box::new(move || deferred.init()));
        }

        let deferred = utility.clone();
        self.system_deinits
            .lock()
            .unwrap()
            .push(Box::new(move || deferred.deinit()));

        utility
    }

    /// 绑定一个 Controller（Controller 不进入 IOC 容器，只注入架构引用）。
    ///
    /// 在 Bevy 中通常交给 `App::add_q_controller` 处理：它会调用本方法完成绑定，
    /// 再把控制器交给 `QControllers` 每帧调度。
    pub fn attach_controller<C: IController>(&self, controller: C) -> Arc<C> {
        let controller = Arc::new(controller);
        controller.arch_ref().set(&self.arc());
        controller
    }

    // -----------------------------------------------------------------------
    // 获取
    // -----------------------------------------------------------------------

    /// 取得 Model，不存在时 panic。
    pub fn get_model<M: IModel>(&self) -> Arc<M> {
        self.ioc.read().unwrap().expect::<M>()
    }

    /// 尝试取得 Model。
    pub fn try_get_model<M: IModel>(&self) -> Option<Arc<M>> {
        self.ioc.read().unwrap().get::<M>()
    }

    /// 取得 System，不存在时 panic。
    pub fn get_system<S: ISystem>(&self) -> Arc<S> {
        self.ioc.read().unwrap().expect::<S>()
    }

    /// 尝试取得 System。
    pub fn try_get_system<S: ISystem>(&self) -> Option<Arc<S>> {
        self.ioc.read().unwrap().get::<S>()
    }

    /// 取得 Utility，不存在时 panic。
    pub fn get_utility<U: IUtility>(&self) -> Arc<U> {
        self.ioc.read().unwrap().expect::<U>()
    }

    /// 尝试取得 Utility。
    pub fn try_get_utility<U: IUtility>(&self) -> Option<Arc<U>> {
        self.ioc.read().unwrap().get::<U>()
    }

    // -----------------------------------------------------------------------
    // 通信
    // -----------------------------------------------------------------------

    /// 发送命令并等待其执行结果。
    ///
    /// 命令在**调用线程**上同步执行，架构不会对命令做排队或串行化。因此如果多个
    /// 线程（例如 Bevy 的并行系统）并发发送同一个「读-改-写」命令，需要保证操作
    /// 本身是原子的（例如用 [`BindableProperty::modify`](crate::BindableProperty::modify)）。
    pub fn send_command<C: ICommand>(&self, command: C) -> C::Output {
        let context = CommandContext::new(self.arc());
        command.execute(&context)
    }

    /// 发送查询并取得结果。
    pub fn send_query<Q: IQuery>(&self, query: Q) -> Q::Result {
        let context = QueryContext::new(self.arc());
        query.do_query(&context)
    }

    /// 广播事件。
    pub fn send_event<E: 'static>(&self, event: E) {
        self.events.send(event);
    }

    /// 用 `E::default()` 广播事件。
    pub fn send_event_default<E: Default + 'static>(&self) {
        self.events.send_default::<E>();
    }

    /// 注册事件监听，返回注销句柄。
    pub fn register_event<E: 'static, F>(&self, handler: F) -> IUnRegister
    where
        F: Fn(&E) + Send + Sync + 'static,
    {
        self.events.register::<E>(handler)
    }

    // -----------------------------------------------------------------------
    // 生命周期
    // -----------------------------------------------------------------------

    /// 两阶段初始化的第二阶段：先初始化 Model，再初始化 System 与 Utility。
    ///
    /// 由 [`ArchitectureBuilder::build`] 自动调用，一般无需手动调用。
    /// 重复调用是安全的（第二次为空操作）。
    pub fn init(&self) {
        if self.inited.swap(true, Ordering::SeqCst) {
            return;
        }

        run_all(&self.model_inits);
        run_all(&self.system_inits);
    }

    /// 反初始化：先清理 System，再清理 Model，最后清空容器与事件。
    ///
    /// 这是**终态**操作：调用之后 IOC 容器被清空、事件被清空、`is_inited()` 变回
    /// `false`，此时再调用 `get_model` / `get_system` / `get_utility` 会 panic。
    /// 重复调用是安全的（第二次为空操作）。
    ///
    /// `qframework-bevy` 会在应用退出时自动调用它。
    pub fn deinit(&self) {
        run_all(&self.system_deinits);
        run_all(&self.model_deinits);

        self.ioc.write().unwrap().clear();
        self.events.clear();
        self.inited.store(false, Ordering::SeqCst);
    }
}

/// 依次执行队列中的回调，并允许回调在运行过程中继续追加。
fn run_all(queue: &Mutex<Vec<BoxedLifecycle>>) {
    loop {
        let next = {
            let mut queue = queue.lock().unwrap();
            if queue.is_empty() {
                None
            } else {
                Some(queue.remove(0))
            }
        };

        match next {
            Some(callback) => callback(),
            None => break,
        }
    }
}

impl std::fmt::Debug for Architecture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Architecture")
            .field("name", &self.name)
            .field("inited", &self.is_inited())
            .field("registered", &self.registered_count())
            .finish()
    }
}

/// 架构构建器：注册所有层对象，并产出共享的 [`Architecture`]。
pub struct ArchitectureBuilder {
    name: &'static str,
    registrations: Vec<BoxedRegistration>,
}

impl ArchitectureBuilder {
    /// 创建构建器。
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            registrations: Vec::new(),
        }
    }

    /// 注册一个 Model。
    pub fn model<M: IModel>(mut self, model: M) -> Self {
        self.registrations.push(Box::new(move |architecture| {
            architecture.register_model(model);
        }));
        self
    }

    /// 注册一个 System。
    pub fn system<S: ISystem>(mut self, system: S) -> Self {
        self.registrations.push(Box::new(move |architecture| {
            architecture.register_system(system);
        }));
        self
    }

    /// 注册一个 Utility。
    pub fn utility<U: IUtility>(mut self, utility: U) -> Self {
        self.registrations.push(Box::new(move |architecture| {
            architecture.register_utility(utility);
        }));
        self
    }

    /// 追加一段自定义注册逻辑。
    ///
    /// 对应 C# 版的 `Architecture<T>.OnRegisterPatch`：在所有 `model` / `system` /
    /// `utility` 之后、两阶段初始化之前执行，适合按需注册可选模块。
    ///
    /// ```ignore
    /// let architecture = ArchitectureBuilder::new("MyApp")
    ///     .model(PlayerModel::default())
    ///     .patch(|architecture| {
    ///         if enable_debug_panel {
    ///             architecture.register_system(DebugPanelSystem::default());
    ///         }
    ///     })
    ///     .build();
    /// ```
    pub fn patch<P>(mut self, patch: P) -> Self
    where
        P: FnOnce(&Arc<Architecture>) + Send + Sync + 'static,
    {
        self.registrations.push(Box::new(patch));
        self
    }

    /// 构建架构并完成两阶段初始化。
    pub fn build(self) -> Arc<Architecture> {
        let architecture = Arc::new(Architecture::new(self.name));
        let _ = architecture.self_ref.set(Arc::downgrade(&architecture));

        for register in self.registrations {
            register(&architecture);
        }

        architecture.init();
        architecture
    }
}

impl std::fmt::Debug for ArchitectureBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArchitectureBuilder")
            .field("name", &self.name)
            .field("registrations", &self.registrations.len())
            .finish()
    }
}
