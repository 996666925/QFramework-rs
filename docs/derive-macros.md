# 派生宏

四个派生宏把「手写 4~6 个 trait 实现」压缩成一行：

| 派生宏 | 生成的能力 trait | 生成的层 trait | 需要 `arch` 字段 |
|---|---|---|:---:|
| `#[derive(IController)]` | `HasArchRef` `ICanGetArchitecture` `ICanGetModel` `ICanGetSystem` `ICanSendCommand` `ICanSendQuery` `ICanRegisterEvent` | `IController` | ✓ |
| `#[derive(ISystem)]` | `HasArchRef` `ICanGetArchitecture` `ICanGetModel` `ICanGetSystem` `ICanGetUtility` `ICanSendEvent` `ICanRegisterEvent` | `ISystem` | ✓ |
| `#[derive(IModel)]` | `HasArchRef` `ICanGetArchitecture` `ICanGetUtility` `ICanSendEvent` | `IModel` | ✓ |
| `#[derive(IUtility)]` | （无） | `IUtility` | ✗ |

宏名和同名 trait 位于**不同命名空间**，所以 `#[derive(IModel)]` 和 `trait IModel` 可以共存——和 serde 的 `Serialize` 是一个机制。`use qframework_core::prelude::*;` 会同时把两者引入。

---

## 展开后是什么

```rust
#[derive(Default, IModel)]
struct CounterModel {
    arch: ArchRef,
    pub count: BindableProperty<i32>,
}
```

等价于手写：

```rust
impl HasArchRef for CounterModel {
    fn arch_ref(&self) -> &ArchRef { &self.arch }
}

impl ICanGetArchitecture for CounterModel {
    fn architecture(&self) -> Arc<Architecture> { self.arch.get() }
}

impl ICanGetUtility for CounterModel {}
impl ICanSendEvent for CounterModel {}

impl IModel for CounterModel {}
```

想完全不用过程宏？把上面这段抄下来就行——`qframework-core` 的运行时部分不依赖宏，只有便利性依赖它。

---

## `arch` 字段

Controller / System / Model 必须有一个用于注入架构引用的字段。宏按这个顺序查找：

1. 带 `#[arch]` 属性的字段；
2. 名为 `arch` 的字段。

```rust
// 方式一：约定名
#[derive(Default, IModel)]
struct PlayerModel {
    arch: ArchRef,
    pub hp: BindableProperty<i32>,
}

// 方式二：自定义名 + #[arch] 标注
#[derive(Default, ISystem)]
struct BattleSystem {
    #[arch]
    context: ArchRef,
    turn: AtomicI32,
}
```

字段可以是私有的（生成的 `impl` 位于同一模块）。字段类型必须是 `ArchRef`。

**Utility 不需要 `arch` 字段**，因为它拿不到架构：

```rust
#[derive(Default, IUtility)]
struct SaveUtility;

impl SaveUtility {
    pub fn save(&self, bytes: &[u8]) -> std::io::Result<()> { /* ... */ }
}
```

---

## 生命周期钩子

用层专属的辅助属性指定 `init` / `deinit`。值可以是**闭包**，也可以是**函数或方法路径**。

```rust
// 闭包
#[derive(Default, ISystem)]
#[system(init = |this: &AchievementSystem| { this.subscribe(); })]
struct AchievementSystem {
    arch: ArchRef,
}

// 方法路径（推荐：逻辑本身放在 impl 里，可读性更好）
#[derive(Default, IModel)]
#[model(init = Self::on_init, deinit = Self::on_deinit)]
struct PlayerModel {
    arch: ArchRef,
    pub hp: BindableProperty<i32>,
}

impl PlayerModel {
    fn on_init(&self) {
        // 此时架构已完成注册，可以取 Model / Utility
        self.hp.set(100);
    }

    fn on_deinit(&self) {
        self.send_event(PlayerUnloadedEvent);
    }
}
```

辅助属性名与派生宏一一对应：`#[controller(..)]`、`#[system(..)]`、`#[model(..)]`、`#[utility(..)]`。

两个钩子都是可选的，可以只写一个：

```rust
#[derive(Default, IController)]
#[controller(init = Self::start)]
struct HudController { arch: ArchRef }
```

钩子接收的是 `&Self`（即 `&self`），因为层对象注册后就以 `Arc<Self>` 共享，不可能拿到 `&mut Self`。**所有可变状态必须用内部可变性**（`BindableProperty`、`Atomic*`、`Mutex`、`RwLock`）。

---

## 泛型结构体

支持带类型参数、生命周期参数和 const 参数的结构体。宏会为层 trait 的 `impl` 追加一条约束，把检查推迟到使用处：

```rust
#[derive(Default, IModel)]
struct RegistryModel<T>
where
    T: Send + Sync + 'static,
{
    arch: ArchRef,
    payload: std::marker::PhantomData<T>,
}
```

展开后会带上 `where RegistryModel<T>: Send + Sync + 'static`，所以 `T` 不满足时错误会指向你的使用点，而不是宏展开处。

---

## 限制

### 只能用于具名字段的结构体

```rust
#[derive(IModel)]
struct Bad(i32);        // ❌ 编译错误
```

### 一个类型只能是一个层

```rust
#[derive(IModel, ISystem)]
struct Bad { arch: ArchRef }   // ❌ HasArchRef 被实现了两次
```

### 不能与手写的同名 impl 共存

```rust
#[derive(IModel)]
struct CounterModel { arch: ArchRef }

impl IModel for CounterModel { }   // ❌ conflicting implementations
```

如果某个层需要完全自定义的 trait 实现（比如要覆盖 `deinit` 之外的行为），就**不要用派生宏**，改为手写展开后的那几行。需要 `init` / `deinit` 的话，用辅助属性就够了，不必手写。

---

## 常见编译错误

### 缺少 `arch` 字段

```
error: #[derive(IModel)] 找不到架构引用字段：请添加 `arch: ArchRef`，或用 `#[arch]` 标记已有字段
 --> src/model.rs:4:8
  |
4 | struct PlayerModel {
  |        ^^^^^^^^^^^
```

### 用在了枚举或 union 上

```
error: #[derive(IModel)] 只能用于结构体
```

### 元组结构体

```
error: #[derive(IModel)] 需要具名字段（包含 `arch: ArchRef`）
```

### 辅助属性写错

```
error: `#[model(...)]` 只支持 `init` 与 `deinit`
error: `#[model(...)]` 只支持 `init = <表达式>` 与 `deinit = <表达式>`
```

---

## 与层 trait 的关系

派生宏生成的层 `impl` 是**空的**（只填了钩子），所以 trait 的默认方法都会生效。`IController` / `ISystem` / `IModel` / `IUtility` 的 `init` 与 `deinit` 默认都是空实现，写了钩子才会被调用。

想在宏之外扩展行为，就在 `impl 你的类型` 里加普通方法——它们和 trait 无关，随便写：

```rust
#[derive(Default, IModel)]
struct PlayerModel { arch: ArchRef, pub hp: BindableProperty<i32> }

impl PlayerModel {
    /// 普通业务方法：不需要派生宏参与
    pub fn heal(&self, amount: i32) {
        self.hp.modify(|hp| *hp += amount);
        self.send_event(HpChangedEvent { hp: self.hp.get() });
    }
}
```

---

## 下一步

- 这些 trait 的完整能力边界 → [核心概念](core-concepts.md#四层对象)
- 怎么组织业务代码 → [最佳实践](best-practices.md)
