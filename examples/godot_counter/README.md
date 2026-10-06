# Godot Node Controller 计数器

`CounterNode` 同时派生 `GodotClass` 与 `IController`。
通过 Command 更新 Model，通过 Query 读取计数，在 `ready` 中监听属性和业务事件。
属性回调立即修改子节点 `Display` 的位置和界面文本，无需 `process` 轮询。
`SpatialCounter` 演示 Node2D 也可获得相同能力。

```powershell
cargo build -p godot-counter
./examples/godot_counter/run.ps1 -GodotPath 'E:\Godot\Godot_v4.7-stable_win64.exe'
```

示例窗口包含计数文本与 `+1` 按钮，按钮调用节点导出的 `increase` 方法。
`CounterApp::interface()` 按需创建共享 Architecture。
节点通过 `#[controller(architecture = CounterApp)]` 自动获取架构，
在 `ready` 中初始化并订阅，通过 `unregister_when_tree_exited` 自动注销，
在 `exit_tree` 中调用 `deinit`。
场景切换不销毁应用数据，GDExtension 退出时调用 `CounterApp::deinit_interface()`。

框架只在单线程使用。`register_with_init_value` 的闭包直接捕获展示节点，
不重新借用 Controller；订阅后链式调用 `.unregister_when_tree_exited(&mut self.base_mut())`，
退出树时自动注销。需要在真正销毁时才注销，可用 `unregister_when_node_destroyed`。
无需对象 ID、本地回调注册表或 `process` 轮询。

## 无窗口验证

先下载依赖，之后脚本使用离线构建：

```powershell
cargo fetch
./examples/godot_counter/run.ps1 -Smoke -GodotPath 'E:\Godot\Godot_v4.7-stable_win64.exe'
```

检查场景节点访问、Command / Query、属性初值与同步界面更新、
Controller 生命周期、退出清理、重新入树、销毁订阅、提前注销和从未入树的节点释放。
成功输出 `QFRAMEWORK_GODOT_SMOKE_OK`；缺少成功标记、引擎错误或断言失败都会使脚本失败。

示例 `.gdextension` 路径使用 workspace 默认的 `target` 目录。
自定义 `CARGO_TARGET_DIR` 时需同步修改 `counter.gdextension` 中的动态库路径。
