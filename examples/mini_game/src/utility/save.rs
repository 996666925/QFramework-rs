//! 存档工具（真实落盘）。

use std::path::{Path, PathBuf};

use qframework_core::prelude::*;

/// 把游戏状态写到磁盘。
///
/// 为了保持示例零额外依赖，存档格式是极简的 `key=value` 文本；
/// 真实项目里换成 JSON / 二进制序列化即可，接口形状不需要变。
///
/// 落盘位置是系统临时目录下的 `qframework_mini_game/`，不会污染工程目录。
#[derive(IUtility)]
pub struct SaveUtility {
    directory: PathBuf,
}

impl SaveUtility {
    /// 创建存档工具。
    pub fn new() -> Self {
        Self {
            directory: std::env::temp_dir().join("qframework_mini_game"),
        }
    }

    /// 存档目录。
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// 写入一个存档槽，返回文件路径。
    pub fn save(&self, slot: &str, payload: &str) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(&self.directory)?;

        let path = self.directory.join(format!("{slot}.save"));
        std::fs::write(&path, payload)?;

        Ok(path)
    }

    /// 读取一个存档槽。
    pub fn load(&self, slot: &str) -> std::io::Result<String> {
        std::fs::read_to_string(self.directory.join(format!("{slot}.save")))
    }
}
