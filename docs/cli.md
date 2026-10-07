# CLI 模板生成器

`qframework-cli` 提供 `qframework` 和简写 `qf` 两个命令，用于生成与核心 trait 和派生宏一致的 Rust 文件。

## 安装与运行

在仓库根目录安装：

```bash
cargo install --path crates/qframework-cli --offline
```

也可以直接运行，不安装：

```bash
cargo run -p qframework-cli --offline -- generate command PlayerAttack --path examples/mini_game
```

安装后，在目标 Rust 项目根目录运行：

```bash
qframework generate command PlayerAttack
qframework new query GetPlayer
qframework g system Combat
qframework model Player
qframework utility Save
qframework event PlayerChanged
qframework app Game
qframework controller Player --architecture crate::app::game_app::GameApp
```

`qframework` 与 `qf` 等价，安装时会同时安装两个可执行文件。
`generate`、`new`、`g` 等价，也可以直接写模板类型。例如：

```bash
qf g command PlayerAttack
qf g system Combat
qf model Player
```

不安装时可用 `cargo run -p qframework-cli --bin qf --offline -- g command PlayerAttack`。
默认的 `cargo run -p qframework-cli` 仍运行 `qframework`。

## 模板类型

| 类型 | 默认输出示例 | 内容 |
|---|---|---|
| `command` | `src/command/player_attack_command.rs` | 无状态结构体、`ICommand`、`Output`、`execute` |
| `query` | `src/query/get_player_query.rs` | `IQuery`、`Result`、`do_query` |
| `system` | `src/system/combat_system.rs` | `ISystem`、`ArchRef`、初始化与清理钩子 |
| `model` | `src/model/player_model.rs` | `IModel`、`ArchRef`、`Default` |
| `utility` | `src/utility/save_utility.rs` | `IUtility`、`Default`，不持有架构引用 |
| `controller` | `src/controller/player_controller.rs` | `IController`、`ArchRef`，可选应用绑定 |
| `event` | `src/event/player_changed_event.rs` | `Debug`、`Clone` 的事件结构体 |
| `app` | `src/app/game_app.rs` | `QApplication` 与 `ArchitectureBuilder` |

名称支持 `PlayerAttack`、`player_attack`、`player-attack` 等写法。
生成器将类型名转为 UpperCamelCase、文件名转为 snake_case，并补齐类型后缀；
例如 `PlayerAttackCommand` 不会再次追加 `Command`。名称限于 ASCII 字母、数字和词分隔符，不能是路径。

Command 和 Query 的方法使用 `todo!()`，需要填入业务逻辑后才能调用。
Query 默认结果为 `()`，请按业务需求修改 `Result` 和方法实现。

## 输出目录与模块

```bash
qframework system Combat --path ./my_game
qframework system Combat --path ./my_game/src
```

`--path` / `-p` 接受项目根目录或名为 `src` 的源目录，默认使用当前目录作为项目根目录。
生成器自动创建目录，在 `src/system.rs` 中追加 `pub mod combat_system;`；
若已有 `src/system/mod.rs`，则更新它，保留现有内容和已有声明的可见性。
两种模块文件同时存在、模块语法错误、同名内联模块或带属性的声明会报错。

首次使用某一类型时，需要在 `src/lib.rs` 或 `src/main.rs` 中声明对应根模块：

```rust
pub mod command;
pub mod system;
pub mod model;
```

使用生成类型时，可从具体子模块导入，例如：

```rust
use crate::model::player_model::PlayerModel;
use crate::system::combat_system::CombatSystem;

ArchitectureBuilder::new()
    .model(PlayerModel::default())
    .system(CombatSystem::default())
```

生成器不修改 Cargo 依赖、应用注册或根模块声明。核心模板要求目标项目依赖 `qframework-core`。

## Godot Controller

```bash
qframework controller Player --godot --architecture crate::app::game_app::GameApp
```

生成包含 `GodotClass`、`IController`、`Base<Node>` 和 `INode::init` / `ready` 的节点模板。
目标项目需依赖 `godot` 与 `qframework-godot`，并已有 GDExtension 入口。
`--architecture` 指定已经实现 `QApplication` 的类型；若省略，需要由宿主显式绑定架构。
普通 Controller 也支持 `--architecture`；这两个选项只用于 Controller。

Bevy 表现层推荐直接使用 ECS 系统；这里的 `system` 模板生成的是 QFramework 业务层 `ISystem`。

## 保护已有文件

```bash
qframework command PlayerAttack --force
qframework command PlayerAttack --no-mod
qframework --help
```

默认拒绝覆盖目标文件。`--force` / `-f` 只覆盖本次生成的源文件；父模块内容保留。
`--no-mod` 只生成源文件，不读取或修改父模块，适合手动管理模块或使用条件编译的工程。

## 验证

```bash
cargo test -p qframework-cli --offline
cargo clippy -p qframework-cli --all-targets --offline -- -D warnings
```
