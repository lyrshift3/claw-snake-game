# 贪吃蛇终端游戏 — AI 开发需求文档

## 一、项目概述

| 项目 | 说明 |
|------|------|
| 项目名称 | snake-game |
| 语言 | Rust (Edition 2021) |
| 终端库 | crossterm 0.27 |
| 随机数 | rand 0.8 |
| 运行环境 | Windows 终端 / 跨平台终端 |
| 入口文件 | src/main.rs（单文件项目） |

## 二、游戏状态机

```
Playing ←→ Paused
Playing  → Help
Help     → Playing
Playing  → GameOver
GameOver → Playing (R重开) / 退出 (Q)
Playing  → QuitConfirm
QuitConfirm → Playing (N) / 退出 (Y)
```

## 三、核心数据结构

### 3.1 Position
```rust
struct Position { x: i16, y: i16 }
```
- 坐标系：左上角 (0,0)，x 向右增大，y 向下增大
- 边框占据 x=0、x=width-1、y=0、y=height-1
- 可活动区域：1..width-1, 1..height-1

### 3.2 Direction
```rust
enum Direction { Up, Down, Left, Right }
```
- `is_opposite()`: 判断是否反向（禁止 180° 掉头）
- `from_keycode()`: 映射方向键/WASD

### 3.3 GameState
```rust
enum GameState { Playing, Paused, GameOver, Help, QuitConfirm }
```

### 3.4 Food
```rust
struct Food { position: Position }
```
- 食物闪烁：600ms 周期，400ms 亮 / 200ms 暗
- 字符：`●`（U+25CF，双宽度字符，需特殊处理光标归位）
- 亮色：Red，暗色：DarkRed

### 3.5 Game
核心字段：
- `snake: Vec<Position>` — 玩家蛇，snake[0] 为蛇头
- `direction: Direction` — 当前移动方向
- `food: Food` — 食物位置
- `score: u32` — 当前分数
- `high_score: u32` — 最高分
- `state: GameState` — 游戏状态
- `width: i16, height: i16` — 游戏区域尺寸（= 终端窗口尺寸）
- `base_speed_ms: u64` — 基础速度 150ms/步
- `direction_stack: Vec<Direction>` — 多按键方向栈
- `direction_key_held: bool` — 是否按住同方向键（3x 加速）
- `direction_held_since: Option<Instant>` — 按键按住起始时间
- `particles: ParticleSystem` — 粒子特效系统
- `score_flash: Option<Instant>` — 分数闪烁效果时间戳
- `npc_snakes: Vec<NpcSnake>` — NPC 蛇列表
- `npc_spawn_timer: Instant` — NPC 生成计时器
- `npc_spawn_interval_secs: u64` — NPC 生成间隔（初始 15 秒，递减至 5 秒）
- `max_npc_snakes: usize` — NPC 最大数量（8）
- `death_reason: Option<String>` — 死亡原因
- `invincible_until: Option<Instant>` — 无敌状态截止时间

## 四、玩家蛇行为

### 4.1 移动
- 每帧按当前方向移动一格
- 禁止反向移动（如正在向右不能直接向左）
- 速度公式：`base_speed_ms - (score/10)*5`，最低 50ms
- 按住同方向键 150ms 后触发 3x 加速，最低 15ms/步

### 4.2 碰撞检测（按优先级）
1. **撞墙**：new_head.x ≤ 0 || ≥ width-1 || y ≤ 0 || ≥ height-1 → 死亡
2. **撞自己**：new_head 在 snake 中 → 死亡
3. **碰 NPC 蛇身**：
   - 非无敌 → 玩家死亡
   - 无敌 → NPC 死亡（播放死亡粒子特效）
4. **NPC 蛇头碰玩家蛇身** → NPC 死亡
5. **无敌期间 NPC 蛇身碰玩家蛇身** → NPC 死亡

### 4.3 吃食物
- 分数 +10
- 触发粒子爆炸特效 + 分数飘字 "+10"
- 边框闪烁 300ms（变金色）
- **触发 3 秒无敌状态**
- 蛇身增长（不 pop 尾部）

### 4.4 无敌状态
- 持续时间：3 秒
- 视觉效果：蛇身 200ms 间隔闪烁（Yellow ↔ White/DarkYellow）
- 信息面板显示倒计时：`★无敌 2.5s`
- 无敌期间碰到玩家的 NPC 蛇死亡消失
- 无敌不保护撞墙和撞自己

### 4.5 多按键方向栈
- 按下方向键 → push_direction（入栈，去重）
- 松开方向键 → release_direction（出栈，回退到栈顶方向）
- 松开按键回退到仍按住的方向时，立即恢复加速状态（held_since 设为 now-200ms 跳过阈值）

## 五、NPC 蛇系统

### 5.1 NpcSnake 结构
```rust
struct NpcSnake {
    body: Vec<Position>,
    direction: Direction,
    move_counter: u8,
    change_dir_counter: u8,
}
```

### 5.2 生成规则
- 初始 1 条 NPC 蛇
- 每 15 秒新增 1 条（间隔递减，最短 5 秒）
- 最多 8 条
- 出生位置：避免与玩家蛇和其他 NPC 蛇重叠
- 出生长度：1 到玩家蛇长度之间随机

### 5.3 AI 行为
- 每 3 帧移动一次（比玩家慢）
- 70% 概率朝食物方向移动，30% 随机转向
- 随机改变方向间隔：5-15 帧
- 撞墙/撞蛇时尝试其他方向，无路可走则保持不动
- NPC 蛇吃到食物也会变长

### 5.4 NPC 碰撞
- NPC 蛇头碰玩家蛇身 → NPC 死亡
- NPC 蛇身碰玩家蛇头 → 玩家死亡（无敌时 NPC 死亡）
- 无敌期间 NPC 任何部位碰玩家 → NPC 死亡

## 六、粒子特效系统

### 6.1 Particle
```rust
struct Particle {
    x: f32, y: f32,       // 浮点坐标
    dx: f32, dy: f32,     // 速度
    life: u8, max_life: u8,
    ch: char, color: Color,
}
```

### 6.2 FloatingText
```rust
struct FloatingText {
    text: String,
    x: f32, y: f32,
    life: u8, max_life: u8,
    color: Color,
}
```

### 6.3 特效类型
| 特效 | 触发 | 粒子数 | 飘字 | 粒子字符 | 粒子颜色 |
|------|------|--------|------|----------|----------|
| 食物特效 | 吃到食物 | 15 | "+10" | *,+,o,#,@,~ | Yellow/Red/Magenta/Cyan/White/Green |
| NPC 死亡特效 | NPC 被消灭 | 20 | "💀DEAD!" | ×,†,‡ | Red/DarkRed/Magenta |

### 6.4 粒子行为
- 向四周爆炸扩散，速度 0.3-2.0
- Y 轴有 0.03 微重力效果
- 飘字向上飘动（y -= 0.2/帧）
- 生命周期：食物 8-18 帧，NPC 死亡 10-22 帧
- 颜色渐变：原色 → DarkYellow → 空格（最终消失）
- 粒子上限 200 个，超出不再生成
- 绘制时需终端边界检查，防止越界

## 七、渲染系统

### 7.1 游戏区域
- 边框：╔═╗║╚═╝，颜色 Cyan（吃到食物闪 Yellow 300ms）
- 玩家蛇头：`█`，颜色 Green（无敌闪 Yellow/White，吃食物闪 White）
- 玩家蛇身：`▓`，颜色 DarkGreen（无敌闪 Yellow/DarkYellow）
- NPC 蛇头：`█`，颜色 Magenta/DarkMagenta（随食物闪烁）
- NPC 蛇身：`▒`，颜色 Red
- 食物：`●`，颜色 Red/DarkRed（闪烁）
- 空位：空格
- **重要**：食物字符 `●` 是双宽度字符，打印后需 `cursor::MoveTo((x+1), y)` 归位光标

### 7.2 信息面板（游戏区域下方）
- 第 1 行：分数 | 最高分 | 时间 | 速度
- 第 2 行：敌蛇数量 | 无敌倒计时（无敌时显示）
- 第 3 行：操作提示
- 第 4 行：清空行

### 7.3 渲染细节
- 使用 `BeginSynchronizedUpdate` / `EndSynchronizedUpdate` 防止闪烁
- 每帧末尾 `cursor::Hide` + `MoveTo(0,0)` 隐藏光标
- 粒子绘制使用 `let _ = execute!` 防止错误中断
- 窗口缩放时调用 `handle_resize` 裁剪越界元素

## 八、操作控制

| 按键 | 功能 | 适用状态 |
|------|------|----------|
| ↑↓←→ / WASD | 移动 | Playing, Paused(仅Release) |
| P | 暂停/继续 | Playing ↔ Paused |
| R | 重新开始 | Playing, Paused, GameOver |
| H | 帮助 | Playing, Paused |
| Q | 退出确认 | Playing, Paused |
| Y/N | 确认/取消退出 | QuitConfirm |
| Ctrl+C | 强制退出 | 任何状态 |

### 按键加速机制
- 按下同方向键 150ms 后触发 3x 加速
- `KeyEventKind::Repeat` 事件立即触发加速
- 松开按键回退到仍按住的方向时，立即恢复加速（跳过 150ms 阈值）

## 九、窗口自适应

- 游戏区域 = 终端窗口大小（高度减 6 行留给信息面板）
- 最小窗口：20×10
- 缩放时裁剪越界的蛇身/NPC/食物
- 蛇被完全裁剪时保留头部在中心
- 食物越界时重新生成

## 十、已知技术要点（AI 开发必读）

1. **双宽度字符**：`●` 在 Windows 终端占 2 列，打印后必须 `MoveTo` 归位光标，否则同行后续字符右偏
2. **光标残留**：粒子/飘字绘制后光标停留在打印位置，每帧末尾必须 `cursor::Hide` + `MoveTo(0,0)`
3. **粒子消失**：粒子最后阶段打印空格而非 `.`，确保视觉上完全消失
4. **粒子越界**：粒子绘制需检查终端边界 `px < term_w && py < term_h`，防止越界残影
5. **碰墙距离**：边框在 x=0/width-1，可活动区域 1..width-1，碰撞检测用 `≤0` / `≥width-1`
6. **方向栈加速恢复**：`release_direction` 回退到栈中仍有按键的方向时，必须恢复加速状态
7. **BrokenPipe 处理**：渲染和事件读取均需捕获 BrokenPipe 错误，避免崩溃
8. **SynchronizedUpdate**：渲染使用同步更新包裹，防止终端闪烁
9. **NPC 生成避让**：`new_avoiding` 尝试 30 次找到不重叠位置，失败返回 None
10. **无敌期间 NPC 全身碰撞**：不仅 NPC 蛇头，NPC 任何部位碰玩家蛇身都导致 NPC 死亡
