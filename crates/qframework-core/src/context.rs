//! Command / Query 的执行上下文。
//!
//! 两个上下文都只暴露「被允许」的能力，因此 Command 无法注册事件、Query 无法发送
//! 事件或命令——越权调用会编译失败。

use std::sync::Arc;

use crate::architecture::Architecture;
use crate::layers::{
    ICanGetArchitecture, ICanGetModel, ICanGetSystem, ICanGetUtility, ICanSendCommand,
    ICanSendEvent,
};

/// Command 的执行上下文：可取 System / Model、发送事件与命令。
#[derive(Clone)]
pub struct CommandContext {
    architecture: Arc<Architecture>,
}

impl CommandContext {
    /// 创建上下文。
    pub fn new(architecture: Arc<Architecture>) -> Self {
        Self { architecture }
    }

    /// 直接访问架构。
    pub fn arch(&self) -> &Architecture {
        &self.architecture
    }
}

impl ICanGetArchitecture for CommandContext {
    fn architecture(&self) -> Arc<Architecture> {
        self.architecture.clone()
    }
}

impl ICanGetModel for CommandContext {}
impl ICanGetSystem for CommandContext {}
impl ICanSendEvent for CommandContext {}
impl ICanSendCommand for CommandContext {}

/// Query 的执行上下文：只可取 System / Model / Utility（只读语义）。
#[derive(Clone)]
pub struct QueryContext {
    architecture: Arc<Architecture>,
}

impl QueryContext {
    /// 创建上下文。
    pub fn new(architecture: Arc<Architecture>) -> Self {
        Self { architecture }
    }

    /// 直接访问架构。
    pub fn arch(&self) -> &Architecture {
        &self.architecture
    }
}

impl ICanGetArchitecture for QueryContext {
    fn architecture(&self) -> Arc<Architecture> {
        self.architecture.clone()
    }
}

impl ICanGetModel for QueryContext {}
impl ICanGetSystem for QueryContext {}
impl ICanGetUtility for QueryContext {}
