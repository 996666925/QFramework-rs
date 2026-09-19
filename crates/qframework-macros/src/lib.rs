//! QFramework 的派生宏。
//!
//! 这些宏由 `qframework_core` 重新导出（见 `qframework_core::prelude`），
//! 一般情况下不需要直接依赖本 crate。
//!
//! 每个派生宏都会：
//!
//! 1. 生成层的**能力接口**实现（`ICanGetModel`、`ICanSendCommand` 等），
//!    从而在编译期限制该层能访问什么；
//! 2. 生成对应的**层接口**实现（`IModel`、`ISystem` …）。
//!
//! 约定：Controller / System / Model 需要一个 `arch: ArchRef` 字段用于注入架构
//! 引用（可以用 `#[arch]` 标注别的字段名）；Utility 不需要。

use proc_macro::TokenStream;
use syn::DeriveInput;

mod layer;

/// 为**数据层**对象生成实现。
///
/// 只能使用 Utility 与发送事件，无法反向获取 System / Controller。
///
/// ```ignore
/// #[derive(Default, IModel)]
/// struct CounterModel {
///     arch: ArchRef,
///     pub count: BindableProperty<i32>,
/// }
/// ```
///
/// 生命周期钩子用 `#[model(init = ..., deinit = ...)]` 指定，
/// 值可以是闭包，也可以是函数/方法路径：
///
/// ```ignore
/// #[derive(Default, IModel)]
/// #[model(init = Self::on_init)]
/// struct CounterModel { arch: ArchRef }
///
/// impl CounterModel {
///     fn on_init(&self) { /* ... */ }
/// }
/// ```
#[proc_macro_derive(IModel, attributes(arch, model))]
pub fn derive_i_model(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    layer::expand(input, layer::Layer::Model).into()
}

/// 为**业务逻辑层**对象生成实现。
///
/// 可以获取 System / Model / Utility，并发送与注册事件。
///
/// ```ignore
/// #[derive(Default, ISystem)]
/// #[system(init = |this: &CounterSystem| { this.log("ready"); })]
/// struct CounterSystem {
///     arch: ArchRef,
/// }
/// ```
#[proc_macro_derive(ISystem, attributes(arch, system))]
pub fn derive_i_system(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    layer::expand(input, layer::Layer::System).into()
}

/// 为**表现层**对象生成实现。
///
/// 可以获取 System / Model，发送 Command / Query，并注册事件；
/// 但**不能**发送事件、也不能直接修改 Model。
///
/// ```ignore
/// #[derive(IController)]
/// struct CounterController {
///     arch: ArchRef,
/// }
/// ```
#[proc_macro_derive(IController, attributes(arch, controller))]
pub fn derive_i_controller(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    layer::expand(input, layer::Layer::Controller).into()
}

/// 为**工具层**对象生成实现。
///
/// Utility 不持有架构引用，因此不需要 `arch` 字段。
///
/// ```ignore
/// #[derive(Default, IUtility)]
/// struct ConsoleUtility;
/// ```
#[proc_macro_derive(IUtility, attributes(utility))]
pub fn derive_i_utility(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    layer::expand(input, layer::Layer::Utility).into()
}
