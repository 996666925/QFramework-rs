//! 中心枢纽：对应 QFramework 的 `IArchitecture` / `Architecture<T>`。
//!
//! 架构负责：
//! 1. 承载 IOC 容器与事件系统；
//! 2. 管理 System / Model / Utility 的注册与两阶段初始化；
//! 3. 作为 Command / Query 的中介。

use std::cell::{Cell, OnceCell, RefCell};
use std::rc::{Rc, Weak};

use crate::command::ICommand;
use crate::context::{CommandContext, QueryContext};
use crate::event::TypeEventSystem;
use crate::ioc::IOCContainer;
use crate::layers::{IController, IModel, ISystem, IUtility};
use crate::query::IQuery;
use crate::unregister::IUnRegister;

/// 延迟执行的生命周期回调。
type BoxedLifecycle = Box<dyn FnOnce()>;

/// 构建阶段的一次注册动作。
type BoxedRegistration = Box<dyn FnOnce(&Rc<Architecture>)>;

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
    ioc: RefCell<IOCContainer>,
    events: TypeEventSystem,
    inited: Cell<bool>,
    self_ref: OnceCell<Weak<Architecture>>,
    model_inits: RefCell<Vec<BoxedLifecycle>>,
    system_inits: RefCell<Vec<BoxedLifecycle>>,
    model_deinits: RefCell<Vec<BoxedLifecycle>>,
    system_deinits: RefCell<Vec<BoxedLifecycle>>,
}

impl Architecture {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            ioc: RefCell::new(IOCContainer::new()),
            events: TypeEventSystem::new(),
            inited: Cell::new(false),
            self_ref: OnceCell::new(),
            model_inits: RefCell::new(Vec::new()),
            system_inits: RefCell::new(Vec::new()),
            model_deinits: RefCell::new(Vec::new()),
            system_deinits: RefCell::new(Vec::new()),
        }
    }

    /// 架构名称。
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// 是否已经完成初始化。
    pub fn is_inited(&self) -> bool {
        self.inited.get()
    }

    /// 取得架构自身的 [`Rc`] 句柄。
    ///
    /// 只有通过 [`ArchitectureBuilder::build`] 创建的架构才具备自引用。
    pub fn rc(&self) -> Rc<Architecture> {
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
        self.ioc.borrow().len()
    }

    // -----------------------------------------------------------------------
    // 注册
    // -----------------------------------------------------------------------

    /// 注册 Model，返回共享句柄。
    pub fn register_model<M: IModel>(&self, model: M) -> Rc<M> {
        let model = Rc::new(model);
        model.arch_ref().set(&self.rc());
        self.ioc.borrow_mut().register_rc(model.clone());

        if self.inited.get() {
            model.init();
        } else {
            let deferred = model.clone();
            self.model_inits
                .borrow_mut()
                .push(Box::new(move || deferred.init()));
        }

        let deferred = model.clone();
        self.model_deinits
            .borrow_mut()
            .push(Box::new(move || deferred.deinit()));

        model
    }

    /// 注册 System，返回共享句柄。
    pub fn register_system<S: ISystem>(&self, system: S) -> Rc<S> {
        let system = Rc::new(system);
        system.arch_ref().set(&self.rc());
        self.ioc.borrow_mut().register_rc(system.clone());

        if self.inited.get() {
            system.init();
        } else {
            let deferred = system.clone();
            self.system_inits
                .borrow_mut()
                .push(Box::new(move || deferred.init()));
        }

        let deferred = system.clone();
        self.system_deinits
            .borrow_mut()
            .push(Box::new(move || deferred.deinit()));

        system
    }

    /// 注册 Utility。Utility 不需要架构引用。
    pub fn register_utility<U: IUtility>(&self, utility: U) -> Rc<U> {
        let utility = Rc::new(utility);
        self.ioc.borrow_mut().register_rc(utility.clone());

        if self.inited.get() {
            utility.init();
        } else {
            let deferred = utility.clone();
            self.system_inits
                .borrow_mut()
                .push(Box::new(move || deferred.init()));
        }

        let deferred = utility.clone();
        self.system_deinits
            .borrow_mut()
            .push(Box::new(move || deferred.deinit()));

        utility
    }

    /// 绑定一个 Controller（Controller 不进入 IOC 容器，只注入架构引用）。
    ///
    /// 调用方负责控制器的初始化、运行和反初始化。
    /// Bevy 表现层通常直接使用系统，无需绑定控制器对象。
    pub fn attach_controller<C: IController>(&self, controller: C) -> Rc<C> {
        let controller = Rc::new(controller);
        controller.arch_ref().set(&self.rc());
        controller
    }

    // -----------------------------------------------------------------------
    // 获取
    // -----------------------------------------------------------------------

    /// 取得 Model，不存在时 panic。
    pub fn get_model<M: IModel>(&self) -> Rc<M> {
        self.ioc.borrow().expect::<M>()
    }

    /// 尝试取得 Model。
    pub fn try_get_model<M: IModel>(&self) -> Option<Rc<M>> {
        self.ioc.borrow().get::<M>()
    }

    /// 取得 System，不存在时 panic。
    pub fn get_system<S: ISystem>(&self) -> Rc<S> {
        self.ioc.borrow().expect::<S>()
    }

    /// 尝试取得 System。
    pub fn try_get_system<S: ISystem>(&self) -> Option<Rc<S>> {
        self.ioc.borrow().get::<S>()
    }

    /// 取得 Utility，不存在时 panic。
    pub fn get_utility<U: IUtility>(&self) -> Rc<U> {
        self.ioc.borrow().expect::<U>()
    }

    /// 尝试取得 Utility。
    pub fn try_get_utility<U: IUtility>(&self) -> Option<Rc<U>> {
        self.ioc.borrow().get::<U>()
    }

    // -----------------------------------------------------------------------
    // 通信
    // -----------------------------------------------------------------------

    /// 发送命令并等待其执行结果。
    ///
    /// 命令在当前线程同步执行，不排队。架构和层对象不能跨线程传递。
    pub fn send_command<C: ICommand>(&self, command: C) -> C::Output {
        let context = CommandContext::new(self.rc());
        command.execute(&context)
    }

    /// 发送查询并取得结果。
    pub fn send_query<Q: IQuery>(&self, query: Q) -> Q::Result {
        let context = QueryContext::new(self.rc());
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
        F: FnMut(&E) + 'static,
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
        if self.inited.replace(true) {
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

        self.ioc.borrow_mut().clear();
        self.events.clear();
        self.inited.set(false);
    }
}

/// 依次执行队列中的回调，并允许回调在运行过程中继续追加。
fn run_all(queue: &RefCell<Vec<BoxedLifecycle>>) {
    loop {
        let next = {
            let mut queue = queue.borrow_mut();
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
        P: FnOnce(&Rc<Architecture>) + 'static,
    {
        self.registrations.push(Box::new(patch));
        self
    }

    /// 构建架构并完成两阶段初始化。
    pub fn build(self) -> Rc<Architecture> {
        let architecture = self.create();
        self.initialize(&architecture);
        architecture
    }

    pub(crate) fn create(&self) -> Rc<Architecture> {
        let architecture = Rc::new(Architecture::new(self.name));
        let _ = architecture.self_ref.set(Rc::downgrade(&architecture));
        architecture
    }

    pub(crate) fn initialize(self, architecture: &Rc<Architecture>) {
        for register in self.registrations {
            register(architecture);
        }

        architecture.init();
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
