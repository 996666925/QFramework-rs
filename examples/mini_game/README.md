# QFramework 迷你地牢

一个「从零开始、六种角色全部参与」的完整示例工程。无窗口运行，剧本由控制器驱动，
可以直接 `cargo run` 看到完整的一局游戏。

```bash
cargo run -p mini-game
```

---

## 它演示了什么

| 角色 | 目录 | 代表 |
|---|---|---|
| 工具层 `IUtility` | [`src/utility`](src/utility) | `LogUtility`（日志格式）、`SaveUtility`（真实落盘） |
| 数据层 `IModel` | [`src/model`](src/model) | `PlayerModel`、`InventoryModel`、`ShopModel`、`BattleModel`、`AchievementModel` |
| 命令 `ICommand` | [`src/command`](src/command) | `BuyItemCommand`、`PlayerAttackCommand`、`UseItemCommand`、`StartGameCommand` |
| 查询 `IQuery` | [`src/query`](src/query) | `GetPlayerSnapshotQuery`、`GetInventoryQuery` |
| 业务逻辑层 `ISystem` | [`src/system`](src/system) | `AchievementSystem`（跨 Model 协调）、`AutoSaveSystem`（Model + Utility） |
| 表现层 `IController` | [`src/controller`](src/controller) | `HudController`（状态 → 表现）、`ScriptController`（输入 → 命令） |

另外还演示了：

- **Bevy 集成**：`add_qframework` / `bridge_q_messages` / `add_q_controller`
- **事件桥接**：`EnemyDefeatedEvent` 同时是 QFramework 事件和 Bevy 消息
- **数据绑定**：`register_with_init_value` 一次搞定「初值 + 后续变化」
- **脏标记 + 内容比对**：状态栏不会因为意义相同的通知而重复输出
- **生命周期钩子**：`#[model(init = Self::stock)]`、`#[system(init = Self::subscribe)]`
- **分层约束**：`IController` 不能发事件、不能取 Utility；`IModel` 不能取别的 Model

---

## 目录结构

```
mini_game/
├── Cargo.toml
├── README.md
└── src/
    ├── main.rs              # Bevy App 组装 + 一个普通的 Bevy 系统 + 最终结算
    ├── app.rs               # QApplication：整个游戏的「目录」
    ├── event.rs             # 全局事件定义
    ├── model.rs
    ├── model/
    │   ├── item.rs          # ItemId
    │   ├── player.rs        # 血量 / 金币 / 等级 / 经验 / 攻击力
    │   ├── inventory.rs     # BindableDictionary<ItemId, u32>
    │   ├── shop.rs          # 商品价格表 + ShopError
    │   ├── battle.rs        # 战斗状态与敌人
    │   └── achievement.rs   # BindableList<&'static str>
    ├── command.rs
    ├── command/
    │   ├── session.rs       # StartGameCommand
    │   ├── shop.rs          # BuyItemCommand
    │   ├── item.rs          # UseItemCommand
    │   └── battle.rs        # StartBattleCommand、PlayerAttackCommand
    ├── query.rs
    ├── query/
    │   ├── player.rs        # GetPlayerSnapshotQuery
    │   └── inventory.rs     # GetInventoryQuery
    ├── system.rs
    ├── system/
    │   ├── achievement.rs   # 监听事件 → 跨 Model 解锁成就
    │   └── auto_save.rs     # 监听事件 → 用 Utility 落盘
    ├── controller.rs
    ├── controller/
    │   ├── hud.rs           # 状态绑定 + 脏标记
    │   └── script.rs        # 固定剧本模拟玩家输入
    └── utility.rs
    ├── utility/
    │   ├── log.rs           # LogUtility
    │   └── save.rs          # SaveUtility
```

目录组织方式与[最佳实践](../../docs/best-practices.md#92-目录组织)中的建议一致。

---

## 剧本

`ScriptController` 每 4 帧执行一步：

1. **开始游戏** — `StartGameCommand` 广播 `GameStartedEvent`
2. **买两瓶药水** — 花费 60 金币（100 → 40），解锁「有备无患」「大客户」
3. **试图买剑** — 需要 80 金币，余额不足 → `BuyItemCommand` 返回 `Err`
4. **进入战斗** — 哥布林 30 HP / 8 攻击
5. **攻击三次** — 12 点伤害 ×3 击杀，期间被反击两次（100 → 84）
6. **结算** — 掉落 50 金币 / 60 经验 → 升到 2 级（最大血量 120，攻击力 15）
7. **喝药回血** — 恢复被上限截断，实际 +16
8. **中场报告** — 用 Query 读数据、用 System 取聚合信息

最终状态：等级 2、HP 120/120、金币 90、背包剩 1 瓶药水、5 项成就。

---

## 输出示例

```
════════════════ QFramework 迷你地牢 ════════════════
  [HUD] HP 100/100  |  金币 100  |  等级 1  |  背包 0 件

══ 剧本 2：商店买两瓶药水 ══
  · 本店在售 3 种商品
  [HUD·事件] 金币 -60 -> 40
  · 解锁成就「有备无患」
  · 解锁成就「大客户」
  · 买入 治疗药水 ×2，花费 60 金币
  [HUD] HP 100/100  |  金币 40  |  等级 1  |  背包 2 件

══ 剧本 3：试试买一把剑（金币不够）══
  · 购买 铁剑 失败：金币不足（需要 80，只有 40）

══ 剧本 4：进入战斗 ══
  [HUD·事件] 遭遇 哥布林（30 HP）
  [HUD·事件] 血量 -> 92/100
  · 命中！哥布林 还剩 18/30 点血
  ...
  [Bevy 系统] 收到战斗结算：哥布林 掉落 50 金币 / 60 经验

══ 剧本 5：喝药回血 ══
  [HUD·事件] 血量 -> 120/120
  · 喝下治疗药水，实际恢复 16 点血量

══ 剧本 6：中场报告 ══
  · 等级 2（10 exp），金币 90，HP 120/120，攻击力 15
  · 背包：治疗药水 ×1
  · 已解锁成就 5 项

════════════════════ 结算 ════════════════════
  等级      2（10 exp）
  生命      120/120
  攻击力    15
  金币      90
  背包      治疗药水 ×1
  成就      5 项
              · 有备无患
              · 大客户
              · 初露锋芒
              · 初战告捷
              · 赏金猎人
  存档目录  …/Temp/qframework_mini_game
  存档文件  …/Temp/qframework_mini_game/final.save
  存档内容  level=2,gold=90
══════════════════════════════════════════════
```

> 存档写在系统临时目录下的 `qframework_mini_game/`，不会污染工程目录。

---

## 几个值得单独看的地方

### 1. 表现层为什么不能「直接改数据」

```rust
// src/controller/script.rs
// 表现层只发命令；它甚至没有「发送事件」的能力
self.send_command(BuyItemCommand { item: ItemId::HealthPotion, quantity: 2 });
```

而「开始游戏时广播事件」这件事必须由一个命令完成（`StartGameCommand`）——
因为 `IController` 没有 `ICanSendEvent` 能力，写 `self.send_event(...)` 是编译错误。

### 2. 跨 Model 的规则放哪里

「买得起吗」需要同时看 `ShopModel` 和 `PlayerModel`，所以它在 `BuyItemCommand` 里。
「什么时候解锁成就」需要同时看事件和 `AchievementModel`，所以它在 `AchievementSystem` 里。
**数据层做不到这两件事**——`IModel` 拿不到别的 Model，这个约束本身就在提示你该放哪。

### 3. 一次状态变化只广播一次

```rust
// src/command/item.rs
// 不需要再广播 ItemUsedEvent —— PlayerModel::heal 已经发出了 HpChangedEvent
Ok(player.heal(heal))
```

### 4. 原子的余额判断

```rust
// src/model/player.rs
// 判断和扣减在同一个写锁内完成，多线程下也不会扣成负数
pub fn spend_gold(&self, amount: i32) -> bool {
    let mut paid = false;
    self.gold.modify(|gold| {
        if *gold >= amount {
            *gold -= amount;
            paid = true;
        }
    });
    ...
}
```

### 5. 状态栏为什么不重复输出

`set` / `modify` 不做相等判断，值没变也会通知，所以脏标记可能被无意义地置位。
`HudController` 除了脏标记之外还比对了渲染内容：

```rust
let mut last = self.last_rendered.lock().unwrap();
if *last == line {
    return;   // 内容一样就不重复打印
}
```

---

## 相关文档

- [架构总览](../../docs/architecture.md) — 为什么这样分层
- [最佳实践](../../docs/best-practices.md) — 本示例遵循的全部约定
- [Bevy 集成](../../docs/bevy.md) — 插件、消息桥接、系统排序
- [排错手册](../../docs/troubleshooting.md) — 遇到报错时
