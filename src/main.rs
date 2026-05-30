use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    style::{Color, Stylize},
    terminal::{self, BeginSynchronizedUpdate, ClearType, EndSynchronizedUpdate},
};
use rand::Rng;
use std::collections::HashSet;
use std::io::{self, Write};
use std::time::{Duration, Instant};

const MIN_GAME_WIDTH: i16 = 20;
const MIN_GAME_HEIGHT: i16 = 10;

// ===== 类型定义 =====

#[derive(Debug, Clone, Copy, PartialEq)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    fn is_opposite(self, other: Direction) -> bool {
        matches!(
            (self, other),
            (Direction::Up, Direction::Down)
                | (Direction::Down, Direction::Up)
                | (Direction::Left, Direction::Right)
                | (Direction::Right, Direction::Left)
        )
    }

    fn from_keycode(code: KeyCode) -> Option<Direction> {
        match code {
            KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('W') => Some(Direction::Up),
            KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('S') => Some(Direction::Down),
            KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('A') => Some(Direction::Left),
            KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('D') => Some(Direction::Right),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum GameState {
    Playing,
    Paused,
    GameOver,
    Help,
    QuitConfirm,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Position {
    x: i16,
    y: i16,
}

struct Food {
    position: Position,
}

// ===== NPC蛇系统 =====

struct NpcSnake {
    body: Vec<Position>,
    direction: Direction,
    move_counter: u8,
    change_dir_counter: u8,
}

impl NpcSnake {
    fn new(width: i16, height: i16, player_length: usize) -> Self {
        let mut rng = rand::thread_rng();
        let body_len = rng.gen_range(1..=player_length).min((width - 4) as usize).max(1);
        let start_x = rng.gen_range((body_len as i16 + 1)..width - 1);
        let start_y = rng.gen_range(2..height - 1);
        let body: Vec<Position> = (0..body_len as i16)
            .map(|i| Position { x: start_x - i, y: start_y })
            .collect();
        let directions = [Direction::Up, Direction::Down, Direction::Left, Direction::Right];
        let direction = directions[rng.gen_range(0..4)];
        Self {
            body,
            direction,
            move_counter: 0,
            change_dir_counter: 0,
        }
    }

    fn new_avoiding(width: i16, height: i16, player_snake: &[Position], all_npc_bodies: &HashSet<(i16, i16)>, player_length: usize) -> Option<Self> {
        let mut rng = rand::thread_rng();
        let player_set: HashSet<(i16, i16)> = player_snake.iter().map(|p| (p.x, p.y)).collect();
        // 尝试多次找到一个不与玩家或其他NPC蛇重叠的位置
        for _ in 0..30 {
            let body_len = rng.gen_range(1..=player_length).min((width - 4) as usize).max(1);
            let start_x = rng.gen_range((body_len as i16 + 1)..width - 1);
            let start_y = rng.gen_range(2..height - 1);
            let body: Vec<Position> = (0..body_len as i16)
                .map(|i| Position { x: start_x - i, y: start_y })
                .collect();
            let overlaps = body.iter().any(|p| {
                player_set.contains(&(p.x, p.y)) || all_npc_bodies.contains(&(p.x, p.y))
            });
            if !overlaps {
                let directions = [Direction::Up, Direction::Down, Direction::Left, Direction::Right];
                let direction = directions[rng.gen_range(0..4)];
                return Some(Self {
                    body,
                    direction,
                    move_counter: 0,
                    change_dir_counter: 0,
                });
            }
        }
        None
    }

    fn update(
        &mut self,
        width: i16,
        height: i16,
        player_snake: &[Position],
        other_npc_bodies: &HashSet<(i16, i16)>,
        food_pos: &Position,
    ) {
        let mut rng = rand::thread_rng();
        self.move_counter += 1;
        self.change_dir_counter += 1;

        // NPC蛇比玩家慢一些，每3帧移动一次
        if self.move_counter < 3 {
            return;
        }
        self.move_counter = 0;

        // 随机改变方向（避免频繁转向）
        if self.change_dir_counter >= rng.gen_range(5..15) {
            self.change_dir_counter = 0;
            // 70%概率朝食物方向移动，30%随机
            if rng.gen_range(0..10) < 7 {
                let head = self.body[0];
                let dx = food_pos.x - head.x;
                let dy = food_pos.y - head.y;
                let preferred_dir = if dx.abs() > dy.abs() {
                    if dx > 0 { Direction::Right } else { Direction::Left }
                } else {
                    if dy > 0 { Direction::Down } else { Direction::Up }
                };
                if !preferred_dir.is_opposite(self.direction) {
                    self.direction = preferred_dir;
                }
            } else {
                let directions = [Direction::Up, Direction::Down, Direction::Left, Direction::Right];
                let new_dir = directions[rng.gen_range(0..4)];
                if !new_dir.is_opposite(self.direction) {
                    self.direction = new_dir;
                }
            }
        }

        let head = self.body[0];
        let new_head = match self.direction {
            Direction::Up => Position { x: head.x, y: head.y - 1 },
            Direction::Down => Position { x: head.x, y: head.y + 1 },
            Direction::Left => Position { x: head.x - 1, y: head.y },
            Direction::Right => Position { x: head.x + 1, y: head.y },
        };

        // 撞墙或撞到自己/玩家蛇/其他NPC蛇时改变方向
        let self_set: HashSet<(i16, i16)> = self.body.iter().map(|p| (p.x, p.y)).collect();
        let player_set: HashSet<(i16, i16)> = player_snake.iter().map(|p| (p.x, p.y)).collect();
        let will_collide = new_head.x <= 0
            || new_head.x >= width - 1
            || new_head.y <= 0
            || new_head.y >= height - 1
            || self_set.contains(&(new_head.x, new_head.y))
            || player_set.contains(&(new_head.x, new_head.y))
            || other_npc_bodies.contains(&(new_head.x, new_head.y));

        if will_collide {
            // 尝试其他方向
            let directions = [Direction::Up, Direction::Down, Direction::Left, Direction::Right];
            let mut moved = false;
            for dir in directions.iter() {
                if dir.is_opposite(self.direction) {
                    continue;
                }
                let alt_head = match dir {
                    Direction::Up => Position { x: head.x, y: head.y - 1 },
                    Direction::Down => Position { x: head.x, y: head.y + 1 },
                    Direction::Left => Position { x: head.x - 1, y: head.y },
                    Direction::Right => Position { x: head.x + 1, y: head.y },
                };
                let alt_ok = alt_head.x > 0
                    && alt_head.x < width - 1
                    && alt_head.y > 0
                    && alt_head.y < height - 1
                    && !self_set.contains(&(alt_head.x, alt_head.y))
                    && !player_set.contains(&(alt_head.x, alt_head.y))
                    && !other_npc_bodies.contains(&(alt_head.x, alt_head.y));
                if alt_ok {
                    self.direction = *dir;
                    self.body.insert(0, alt_head);
                    self.body.pop();
                    moved = true;
                    break;
                }
            }
            if !moved {
                // 无路可走，保持不动
                return;
            }
        } else {
            self.body.insert(0, new_head);
            self.body.pop();
        }

        // NPC蛇吃到食物时变长
        if self.body[0] == *food_pos {
            let tail = *self.body.last().unwrap();
            self.body.push(tail);
        }
    }
}

// ===== 粒子特效系统 =====

struct Particle {
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
    life: u8,
    max_life: u8,
    ch: char,
    color: Color,
}

struct FloatingText {
    text: String,
    x: f32,
    y: f32,
    life: u8,
    max_life: u8,
    color: Color,
}

struct ParticleSystem {
    particles: Vec<Particle>,
    floating_texts: Vec<FloatingText>,
}

impl ParticleSystem {
    fn new() -> Self {
        Self {
            particles: Vec::new(),
            floating_texts: Vec::new(),
        }
    }

    fn emit_food_effect(&mut self, x: i16, y: i16, score_added: u32) {
        if self.particles.len() > 200 {
            return;
        }
        let mut rng = rand::thread_rng();
        let chars = ['*', '+', 'o', '#', '@', '~'];
        let colors = [
            Color::Yellow,
            Color::Red,
            Color::Magenta,
            Color::Cyan,
            Color::White,
            Color::Green,
        ];

        // 粒子爆炸效果
        for _ in 0..15 {
            let angle = rng.gen_range(0.0..std::f32::consts::TAU);
            let speed = rng.gen_range(0.3..1.8);
            let ml = rng.gen_range(8..18);
            self.particles.push(Particle {
                x: x as f32,
                y: y as f32,
                dx: angle.cos() * speed,
                dy: angle.sin() * speed * 0.5,
                life: ml,
                max_life: ml,
                ch: chars[rng.gen_range(0..chars.len())],
                color: colors[rng.gen_range(0..colors.len())],
            });
        }

        // 分数飘字
        self.floating_texts.push(FloatingText {
            text: format!("+{}", score_added),
            x: x as f32,
            y: y as f32,
            life: 20,
            max_life: 20,
            color: Color::Yellow,
        });
    }

    fn emit_npc_death_effect(&mut self, x: i16, y: i16) {
        if self.particles.len() > 200 {
            return;
        }
        let mut rng = rand::thread_rng();
        let chars = ['×', '†', '‡', '†', '‡', '†'];
        let colors = [Color::Red, Color::DarkRed, Color::Magenta];

        for _ in 0..20 {
            let angle = rng.gen_range(0.0..std::f32::consts::TAU);
            let speed = rng.gen_range(0.5..2.0);
            let ml = rng.gen_range(10..22);
            self.particles.push(Particle {
                x: x as f32,
                y: y as f32,
                dx: angle.cos() * speed,
                dy: angle.sin() * speed * 0.5,
                life: ml,
                max_life: ml,
                ch: chars[rng.gen_range(0..chars.len())],
                color: colors[rng.gen_range(0..colors.len())],
            });
        }

        self.floating_texts.push(FloatingText {
            text: "💀DEAD!".to_string(),
            x: x as f32,
            y: y as f32,
            life: 25,
            max_life: 25,
            color: Color::Red,
        });
    }

    fn update(&mut self) {
        for p in &mut self.particles {
            p.x += p.dx;
            p.y += p.dy;
            p.dy += 0.03; // 微重力效果
            p.life = p.life.saturating_sub(1);
        }
        self.particles.retain(|p| p.life > 0);

        for t in &mut self.floating_texts {
            t.y -= 0.2; // 向上飘动
            t.life = t.life.saturating_sub(1);
        }
        self.floating_texts.retain(|t| t.life > 0);
    }

    fn draw(&self, stdout: &mut io::Stdout) -> io::Result<()> {
        let (term_w, term_h) = terminal::size().unwrap_or((80, 24));
        for p in &self.particles {
            let px = p.x.round() as i16;
            let py = p.y.round() as i16;
            if px >= 0 && py >= 0 && px < term_w as i16 && py < term_h as i16 {
                let _ = execute!(stdout, cursor::MoveTo(px as u16, py as u16));
                if p.life > p.max_life * 2 / 3 {
                    print!("{}", p.ch.to_string().with(p.color));
                } else if p.life > p.max_life / 3 {
                    print!("{}", p.ch.to_string().with(Color::DarkYellow));
                } else {
                    print!("{}", ' '.to_string().with(Color::DarkGrey));
                }
            }
        }
        for t in &self.floating_texts {
            let tx = t.x.round() as i16;
            let ty = t.y.round() as i16;
            if tx >= 0 && ty >= 0 && tx < term_w as i16 && ty < term_h as i16 {
                let _ = execute!(stdout, cursor::MoveTo(tx as u16, ty as u16));
                if t.life > t.max_life / 2 {
                    print!("{}", t.text.clone().with(t.color));
                } else {
                    print!("{}", t.text.clone().with(Color::DarkYellow));
                }
            }
        }
        Ok(())
    }
}

// ===== 游戏主体 =====

struct Game {
    snake: Vec<Position>,
    direction: Direction,
    food: Food,
    score: u32,
    high_score: u32,
    state: GameState,
    width: i16,
    height: i16,
    start_time: Instant,
    last_update: Instant,
    base_speed_ms: u64,
    // 多按键方向栈
    direction_stack: Vec<Direction>,
    // 按住同方向键检测（3倍加速）
    direction_key_held: bool,
    direction_held_since: Option<Instant>,
    // 粒子特效系统
    particles: ParticleSystem,
    // 分数闪烁效果
    score_flash: Option<Instant>,
    // NPC蛇（多条，随时间增加）
    npc_snakes: Vec<NpcSnake>,
    npc_spawn_timer: Instant,
    npc_spawn_interval_secs: u64,
    max_npc_snakes: usize,
    // 死亡原因
    death_reason: Option<String>,
}

impl Game {
    fn new(width: i16, height: i16, high_score: u32) -> Self {
        let mut rng = rand::thread_rng();
        let cx = width / 2;
        let cy = height / 2;
        let initial_snake = vec![
            Position { x: cx, y: cy },
            Position { x: cx - 1, y: cy },
            Position { x: cx - 2, y: cy },
        ];
        let food = Food {
            position: Position {
                x: rng.gen_range(2..width - 2),
                y: rng.gen_range(2..height - 2),
            },
        };
        let initial_length = initial_snake.len();
        Game {
            snake: initial_snake,
            direction: Direction::Right,
            food,
            score: 0,
            high_score,
            state: GameState::Playing,
            width,
            height,
            start_time: Instant::now(),
            last_update: Instant::now(),
            base_speed_ms: 150,
            direction_stack: Vec::new(),
            direction_key_held: false,
            direction_held_since: None,
            particles: ParticleSystem::new(),
            score_flash: None,
            npc_snakes: vec![NpcSnake::new(width, height, initial_length)],
            npc_spawn_timer: Instant::now(),
            npc_spawn_interval_secs: 15, // 每15秒生成一条新NPC蛇
            max_npc_snakes: 8,
            death_reason: None,
        }
    }

    /// 收集所有NPC蛇的身体位置集合（用于碰撞检测和生成）
    fn all_npc_bodies(&self) -> HashSet<(i16, i16)> {
        let mut set = HashSet::new();
        for npc in &self.npc_snakes {
            for p in &npc.body {
                set.insert((p.x, p.y));
            }
        }
        set
    }

    fn spawn_food(&mut self) {
        let npc_set = self.all_npc_bodies();
        let mut rng = rand::thread_rng();
        loop {
            let new_pos = Position {
                x: rng.gen_range(1..self.width - 1),
                y: rng.gen_range(1..self.height - 1),
            };
            if !self.snake.contains(&new_pos) && !npc_set.contains(&(new_pos.x, new_pos.y)) {
                self.food.position = new_pos;
                break;
            }
        }
    }

    fn update(&mut self) {
        if self.state != GameState::Playing {
            return;
        }

        // ===== 定时生成新的NPC蛇 =====
        if self.npc_snakes.len() < self.max_npc_snakes
            && self.npc_spawn_timer.elapsed() >= Duration::from_secs(self.npc_spawn_interval_secs)
        {
            let npc_set = self.all_npc_bodies();
            if let Some(new_snake) = NpcSnake::new_avoiding(self.width, self.height, &self.snake, &npc_set, self.snake.len()) {
                self.npc_snakes.push(new_snake);
            }
            self.npc_spawn_timer = Instant::now();
            // 每次成功生成后，缩短下次生成间隔（难度递增），最短5秒
            if self.npc_spawn_interval_secs > 5 {
                self.npc_spawn_interval_secs -= 1;
            }
        }

        // ===== 更新所有NPC蛇 =====
        let food_pos = self.food.position;
        // 为每条NPC蛇计算"其他NPC蛇"的身体集合
        for i in 0..self.npc_snakes.len() {
            let mut other_npc_bodies: HashSet<(i16, i16)> = HashSet::new();
            for j in 0..self.npc_snakes.len() {
                if i != j {
                    for p in &self.npc_snakes[j].body {
                        other_npc_bodies.insert((p.x, p.y));
                    }
                }
            }
            self.npc_snakes[i].update(self.width, self.height, &self.snake, &other_npc_bodies, &food_pos);
        }

        // ===== 玩家移动 =====
        let head = self.snake[0];
        let new_head = match self.direction {
            Direction::Up => Position { x: head.x, y: head.y - 1 },
            Direction::Down => Position { x: head.x, y: head.y + 1 },
            Direction::Left => Position { x: head.x - 1, y: head.y },
            Direction::Right => Position { x: head.x + 1, y: head.y },
        };

        // 撞墙检测
        if new_head.x <= 0
            || new_head.x >= self.width - 1
            || new_head.y <= 0
            || new_head.y >= self.height - 1
        {
            self.death_reason = Some("撞墙".to_string());
            self.state = GameState::GameOver;
            return;
        }
        // 撞自己检测
        if self.snake.contains(&new_head) {
            self.death_reason = Some("咬到自己".to_string());
            self.state = GameState::GameOver;
            return;
        }
        // 玩家蛇头碰到NPC蛇身 → 玩家死亡
        for npc in &self.npc_snakes {
            if npc.body.contains(&new_head) {
                self.death_reason = Some("碰到敌蛇".to_string());
                self.state = GameState::GameOver;
                return;
            }
        }
        // NPC蛇头碰到玩家蛇身 → NPC死亡消失
        let mut npc_indices_to_remove: Vec<usize> = Vec::new();
        let player_set: HashSet<(i16, i16)> = self.snake.iter().map(|p| (p.x, p.y)).collect();
        for (idx, npc) in self.npc_snakes.iter().enumerate() {
            if let Some(npc_head) = npc.body.first() {
                if player_set.contains(&(npc_head.x, npc_head.y)) {
                    npc_indices_to_remove.push(idx);
                }
            }
        }
        // 从后往前删除，避免索引偏移；播放死亡粒子效果
        for &idx in npc_indices_to_remove.iter().rev() {
            if let Some(dead_npc) = self.npc_snakes.get(idx) {
                let head_pos = dead_npc.body[0];
                self.particles.emit_npc_death_effect(head_pos.x, head_pos.y);
            }
            self.npc_snakes.remove(idx);
        }

        self.snake.insert(0, new_head);

        if new_head == self.food.position {
            self.score += 10;
            if self.score > self.high_score {
                self.high_score = self.score;
            }
            // 触发食物特效
            self.particles
                .emit_food_effect(self.food.position.x, self.food.position.y, 10);
            self.score_flash = Some(Instant::now());
            self.spawn_food();
        } else {
            self.snake.pop();
        }
    }

    fn handle_resize(&mut self, new_w: i16, new_h: i16) {
        if new_w < MIN_GAME_WIDTH || new_h < MIN_GAME_HEIGHT {
            return;
        }
        self.width = new_w;
        self.height = new_h;
        self.snake.retain(|p| p.x > 0 && p.x < new_w - 1 && p.y > 0 && p.y < new_h - 1);
        if self.snake.is_empty() {
            self.snake.push(Position { x: new_w / 2, y: new_h / 2 });
        }
        for npc in &mut self.npc_snakes {
            npc.body.retain(|p| p.x > 0 && p.x < new_w - 1 && p.y > 0 && p.y < new_h - 1);
            if npc.body.is_empty() {
                npc.body.push(Position { x: new_w / 2, y: new_h / 2 });
            }
        }
        self.npc_snakes.retain(|npc| !npc.body.is_empty());
        if self.food.position.x <= 0 || self.food.position.x >= new_w - 1
            || self.food.position.y <= 0 || self.food.position.y >= new_h - 1
        {
            self.spawn_food();
        }
    }

    /// 计算当前速度（毫秒/步），得分越高越快，按住同方向键3倍加速
    fn get_speed_ms(&self) -> u64 {
        let reduction = (self.score / 10) as u64 * 5;
        let base = if self.base_speed_ms > reduction + 50 {
            self.base_speed_ms - reduction
        } else {
            50
        };
        if self.direction_key_held {
            (base / 3).max(15)
        } else {
            base
        }
    }

    /// 压入新方向到方向栈（多按键支持）
    fn push_direction(&mut self, new_dir: Direction) {
        if new_dir.is_opposite(self.direction) {
            return; // 阻止反向移动
        }
        if !self.direction_stack.contains(&new_dir) {
            self.direction_stack.push(new_dir);
        }
        if new_dir != self.direction {
            // 方向改变时重置加速标记
            self.direction_key_held = false;
            self.direction_held_since = None;
        }
        self.direction = new_dir;
    }

    /// 从方向栈释放方向键（松开按键时调用）
    fn release_direction(&mut self, dir: Direction) {
        self.direction_stack.retain(|&d| d != dir);
        if let Some(&top) = self.direction_stack.last() {
            self.direction = top;
            self.direction_key_held = true;
            self.direction_held_since = Some(Instant::now() - Duration::from_millis(200));
        } else {
            self.direction_key_held = false;
            self.direction_held_since = None;
        }
    }
}

// ===== 绘制函数 =====

fn draw_game(stdout: &mut io::Stdout, game: &Game) -> io::Result<()> {
    execute!(stdout, cursor::MoveTo(0, 0))?;

    // 边框闪烁效果（吃到食物后300ms内边框变金色）
    let flash_active = game
        .score_flash
        .map(|t| t.elapsed() < Duration::from_millis(300))
        .unwrap_or(false);
    let border_color = if flash_active {
        Color::Yellow
    } else {
        Color::Cyan
    };

    // 构建玩家蛇位置集合
    let head_pos = game.snake.first().map(|p| (p.x, p.y));
    let body_set: HashSet<(i16, i16)> = game.snake.iter().skip(1).map(|p| (p.x, p.y)).collect();
    let food_pos = (game.food.position.x, game.food.position.y);
    let food_visible = game.start_time.elapsed().as_millis() % 600 < 400;

    // 构建所有NPC蛇位置集合
    let mut npc_head_positions: HashSet<(i16, i16)> = HashSet::new();
    let mut npc_body_set: HashSet<(i16, i16)> = HashSet::new();
    for npc in &game.npc_snakes {
        if let Some(h) = npc.body.first() {
            npc_head_positions.insert((h.x, h.y));
        }
        for p in npc.body.iter().skip(1) {
            npc_body_set.insert((p.x, p.y));
        }
    }

    // 逐行绘制游戏区域
    for y in 0..game.height {
        execute!(stdout, cursor::MoveTo(0, y as u16))?;
        if y == 0 {
            print!("{}", '╔'.to_string().with(border_color));
            for _ in 0..game.width - 2 {
                print!("{}", '═'.to_string().with(border_color));
            }
            print!("{}", '╗'.to_string().with(border_color));
        } else if y == game.height - 1 {
            print!("{}", '╚'.to_string().with(border_color));
            for _ in 0..game.width - 2 {
                print!("{}", '═'.to_string().with(border_color));
            }
            print!("{}", '╝'.to_string().with(border_color));
        } else {
            print!("{}", '║'.to_string().with(border_color));
            for x in 1..game.width - 1 {
                if head_pos == Some((x, y)) {
                    let head_color = if flash_active { Color::White } else { Color::Green };
                    print!("{}", '█'.to_string().with(head_color));
                } else if body_set.contains(&(x, y)) {
                    print!("{}", '▓'.to_string().with(Color::DarkGreen));
                } else if npc_head_positions.contains(&(x, y)) {
                    let npc_color = if food_visible { Color::Magenta } else { Color::DarkMagenta };
                    print!("{}", '█'.to_string().with(npc_color));
                } else if npc_body_set.contains(&(x, y)) {
                    print!("{}", '▒'.to_string().with(Color::Red));
                } else if food_pos == (x, y) {
                    if food_visible {
                        print!("{}", '●'.to_string().with(Color::Red));
                    } else {
                        print!("{}", '●'.to_string().with(Color::DarkRed));
                    }
                    let _ = execute!(stdout, cursor::MoveTo((x + 1) as u16, y as u16));
                } else {
                    print!(" ");
                }
            }
            print!("{}", '║'.to_string().with(border_color));
        }
    }

    // 绘制粒子特效和飘字
    game.particles.draw(stdout)?;

    // 绘制信息面板
    let info_y = game.height as u16 + 1;
    let (term_w, term_h) = terminal::size().unwrap_or((80, 24));
    let max_y = term_h.saturating_sub(1);

    let elapsed = game.start_time.elapsed().as_secs();
    let min = elapsed / 60;
    let sec = elapsed % 60;

    if info_y <= max_y {
        let score_color = if flash_active { Color::White } else { Color::Yellow };
        let _ = execute!(stdout, cursor::MoveTo(0, info_y));
        print!("{}", format!("分数: {:<6}", game.score).with(score_color));
        let _ = execute!(stdout, cursor::MoveTo(20.min(term_w.saturating_sub(1)), info_y));
        print!("{}", format!("最高分: {:<6}", game.high_score).with(Color::Magenta));
        let _ = execute!(stdout, cursor::MoveTo(40.min(term_w.saturating_sub(1)), info_y));
        print!("{}", format!("时间: {:02}:{:02}", min, sec).with(Color::White));
        let _ = execute!(stdout, cursor::MoveTo(60.min(term_w.saturating_sub(1)), info_y));
        let speed_text = if game.direction_key_held {
            format!("速度: {}ms [3x!] ", game.get_speed_ms())
        } else {
            format!("速度: {}ms        ", game.get_speed_ms())
        };
        let speed_color = if game.direction_key_held { Color::Green } else { Color::DarkGrey };
        print!("{}", speed_text.with(speed_color));
    }

    if info_y + 1 <= max_y {
        let _ = execute!(stdout, cursor::MoveTo(0, info_y + 1));
        let npc_count = game.npc_snakes.len();
        let npc_warn = if npc_count >= 5 { " ⚠危险!" } else { "" };
        print!("{}", format!("敌蛇: {} 条 (每15秒+1){:<10}", npc_count, npc_warn).with(
            if npc_count >= 5 { Color::Red } else { Color::DarkYellow }
        ));
    }

    if info_y + 2 <= max_y {
        let _ = execute!(stdout, cursor::MoveTo(0, info_y + 2));
        print!(
            "{}",
            "操作: ↑↓←→/WASD | P暂停 | R重开 | H帮助 | Q退出 | 按住同方向键3x加速"
                .with(Color::DarkGrey)
        );
    }

    if info_y + 3 <= max_y {
        let _ = execute!(stdout, cursor::MoveTo(0, info_y + 3));
        print!("{:<40}", "");
    }

    let _ = execute!(stdout, cursor::Hide, cursor::MoveTo(0, 0));
    stdout.flush()
}

fn draw_help(stdout: &mut io::Stdout, w: i16, h: i16) -> io::Result<()> {
    execute!(stdout, terminal::Clear(ClearType::All))?;

    let lines = vec![
        "╔══════════════════════════════════════════════════════════════╗",
        "║                      贪吃蛇游戏帮助                        ║",
        "╠══════════════════════════════════════════════════════════════╣",
        "║                                                            ║",
        "║  操作方式:                                                 ║",
        "║    ↑ ↓ ← →  方向键移动                                    ║",
        "║    W A S D  WASD键移动                                     ║",
        "║                                                            ║",
        "║  游戏控制:                                                 ║",
        "║    P  暂停/继续游戏                                        ║",
        "║    R  重新开始游戏                                         ║",
        "║    H  显示帮助                                             ║",
        "║    Q  退出游戏                                             ║",
        "║                                                            ║",
        "║  游戏规则:                                                 ║",
        "║    - 吃到食物(●)得分+10                                    ║",
        "║    - 撞墙、撞到自己游戏结束                                ║",
        "║    - 玩家蛇头碰到敌蛇身体 → 玩家死亡！                      ║",
        "║    - 敌蛇蛇头碰到玩家身体 → 敌蛇死亡消失(有特效!)          ║",
        "║    - 场上初始有1条敌蛇(NPC)，每隔15秒新增1条！             ║",
        "║    - 敌蛇最多8条，越往后越危险                             ║",
        "║    - 随分数增加，移动速度逐渐加快                          ║",
        "║                                                            ║",
        "║  新增特性:                                                 ║",
        "║    - 按住同方向键不放，移动速度变为3倍！                   ║",
        "║    - 多键组合:按住右再按上则向上，松开上恢复向右           ║",
        "║    - 吃到食物有粒子爆炸特效和分数飘字                     ║",
        "║                                                            ║",
        "║  按任意键返回游戏...                                       ║",
        "║                                                            ║",
        "╚══════════════════════════════════════════════════════════════╝",
    ];

    let start_y = ((h - lines.len() as i16) / 2).max(0) as u16;
    let start_x = ((w - 64) / 2).max(0) as u16;
    for (i, line) in lines.iter().enumerate() {
        execute!(stdout, cursor::MoveTo(start_x, start_y + i as u16))?;
        print!("{}", line.with(Color::Cyan));
    }

    stdout.flush()
}

fn draw_game_over(stdout: &mut io::Stdout, game: &Game) -> io::Result<()> {
    let elapsed = game.start_time.elapsed().as_secs();
    let min = elapsed / 60;
    let sec = elapsed % 60;

    let death_msg = match game.death_reason.as_deref() {
        Some("碰到敌蛇") => "你被敌蛇杀死了！".to_string(),
        Some("撞墙") => "你撞墙了！".to_string(),
        Some("咬到自己") => "你咬到了自己！".to_string(),
        _ => "游戏结束".to_string(),
    };

    let lines: Vec<String> = vec![
        "╔══════════════════════════════════════════════════════════════╗".to_string(),
        "║                       游戏结束！                           ║".to_string(),
        "╠══════════════════════════════════════════════════════════════╣".to_string(),
        "║                                                            ║".to_string(),
        format!("║    {}{:<width$}║", death_msg, "", width = 60 - death_msg.chars().count()),
        format!("║    最终分数: {:<44}║", game.score),
        format!("║    最高分数: {:<44}║", game.high_score),
        format!("║    游戏时间: {:02}:{:02}{:<40}║", min, sec, ""),
        format!("║    蛇的长度: {:<44}║", game.snake.len()),
        format!("║    敌蛇数量: {:<44}║", game.npc_snakes.len()),
        "║                                                            ║".to_string(),
        "║    按 R 重新开始                                           ║".to_string(),
        "║    按 Q 退出游戏                                           ║".to_string(),
        "║                                                            ║".to_string(),
        "╚══════════════════════════════════════════════════════════════╝".to_string(),
    ];

    let start_y = ((game.height - lines.len() as i16) / 2).max(0) as u16;
    let start_x = ((game.width - 64) / 2).max(0) as u16;
    for (i, line) in lines.iter().enumerate() {
        execute!(stdout, cursor::MoveTo(start_x, start_y + i as u16))?;
        if i == 1 {
            print!("{}", line.clone().with(Color::Red));
        } else {
            print!("{}", line.clone().with(Color::Yellow));
        }
    }

    stdout.flush()
}

fn draw_quit_confirm(stdout: &mut io::Stdout, game: &Game) -> io::Result<()> {
    execute!(stdout, terminal::Clear(ClearType::All))?;

    let lines = vec![
        "╔══════════════════════════════════════════════════════════════╗",
        "║                       确认退出？                           ║",
        "╠══════════════════════════════════════════════════════════════╣",
        "║                                                            ║",
        "║    您确定要退出游戏吗？                                    ║",
        "║                                                            ║",
        "║    按 Y 确认退出                                           ║",
        "║    按 N 取消返回游戏                                       ║",
        "║                                                            ║",
        "╚══════════════════════════════════════════════════════════════╝",
    ];

    let start_y = ((game.height - lines.len() as i16) / 2).max(0) as u16;
    let start_x = ((game.width - 64) / 2).max(0) as u16;
    for (i, line) in lines.iter().enumerate() {
        execute!(stdout, cursor::MoveTo(start_x, start_y + i as u16))?;
        print!("{}", line.with(Color::Yellow));
    }

    stdout.flush()
}

fn draw_paused(stdout: &mut io::Stdout, game: &Game) -> io::Result<()> {
    let info_y = game.height as u16 + 1;
    let (_, term_h) = terminal::size().unwrap_or((80, 24));
    if info_y < term_h {
        let _ = execute!(stdout, cursor::MoveTo(0, info_y));
        print!("{}", "游戏已暂停，按 P 继续...".with(Color::Yellow));
    }
    stdout.flush()
}

// ===== 主函数 =====

fn main() -> io::Result<()> {
    let mut stdout = io::stdout();

    terminal::enable_raw_mode()?;
    execute!(
        stdout,
        terminal::EnterAlternateScreen,
        cursor::Hide,
        terminal::Clear(ClearType::All)
    )?;

    let (term_w, term_h) = terminal::size()?;
    let mut game_w: i16 = (term_w as i16).max(MIN_GAME_WIDTH);
    let mut game_h: i16 = (term_h as i16).saturating_sub(6).max(MIN_GAME_HEIGHT);

    let mut game = Game::new(game_w, game_h, 0);
    let mut high_score: u32 = 0;

    loop {
        if let Err(e) = (|| -> io::Result<()> {
            execute!(stdout, BeginSynchronizedUpdate)?;

            match game.state {
                GameState::Playing => {
                    let now = Instant::now();
                    if now.duration_since(game.last_update) >= Duration::from_millis(game.get_speed_ms())
                    {
                        game.update();
                        game.last_update = now;
                    }

                    game.particles.update();

                    draw_game(&mut stdout, &game)?;

                    if game.state == GameState::GameOver {
                        high_score = game.high_score;
                        draw_game_over(&mut stdout, &game)?;
                    }
                }
                GameState::Paused => {
                    game.particles.update();
                    draw_game(&mut stdout, &game)?;
                    draw_paused(&mut stdout, &game)?;
                }
                GameState::Help => {
                    draw_help(&mut stdout, game_w, game_h)?;
                }
                GameState::GameOver => {
                    game.particles.update();
                    draw_game_over(&mut stdout, &game)?;
                }
                GameState::QuitConfirm => {
                    draw_quit_confirm(&mut stdout, &game)?;
                }
            }

            execute!(stdout, EndSynchronizedUpdate)?;
            stdout.flush()?;
            Ok(())
        })() {
            if e.kind() == io::ErrorKind::BrokenPipe {
                break;
            }
            let _ = execute!(stdout, terminal::Clear(ClearType::All));
            let _ = stdout.flush();
            std::thread::sleep(Duration::from_millis(50));
        }

        match event::poll(Duration::from_millis(50)) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(e) => {
                if e.kind() == io::ErrorKind::BrokenPipe {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
        }

        let ev = match event::read() {
            Ok(ev) => ev,
            Err(e) => {
                if e.kind() == io::ErrorKind::BrokenPipe {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
        };

        match ev {
            Event::Resize(w, h) => {
                let new_w = (w as i16).max(MIN_GAME_WIDTH);
                let new_h = (h as i16).saturating_sub(6).max(MIN_GAME_HEIGHT);
                game.handle_resize(new_w, new_h);
                game_w = new_w;
                game_h = new_h;
                let _ = execute!(stdout, terminal::Clear(ClearType::All));
            }
            Event::Key(KeyEvent {
                code, modifiers, kind, ..
            }) => {
                if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
                    break;
                }

                match game.state {
                    GameState::Playing => {
                        if let Some(dir) = Direction::from_keycode(code) {
                            match kind {
                                KeyEventKind::Press => {
                                    if dir == game.direction {
                                        if let Some(since) = game.direction_held_since {
                                            if since.elapsed() >= Duration::from_millis(150) {
                                                game.direction_key_held = true;
                                            }
                                        } else {
                                            game.direction_held_since = Some(Instant::now());
                                        }
                                        game.push_direction(dir);
                                    } else {
                                        game.direction_key_held = false;
                                        game.direction_held_since = Some(Instant::now());
                                        game.push_direction(dir);
                                    }
                                }
                                KeyEventKind::Repeat => {
                                    if dir == game.direction {
                                        game.direction_key_held = true;
                                    }
                                }
                                KeyEventKind::Release => {
                                    game.release_direction(dir);
                                }
                            }
                        } else if kind == KeyEventKind::Press {
                            match code {
                                KeyCode::Char('p') | KeyCode::Char('P') => {
                                    game.state = GameState::Paused;
                                }
                                KeyCode::Char('r') | KeyCode::Char('R') => {
                                    game = Game::new(game_w, game_h, high_score);
                                }
                                KeyCode::Char('h') | KeyCode::Char('H') => {
                                    game.state = GameState::Help;
                                }
                                KeyCode::Char('q') | KeyCode::Char('Q') => {
                                    game.state = GameState::QuitConfirm;
                                }
                                _ => {}
                            }
                        }
                    }
                    GameState::Paused => {
                        if let Some(dir) = Direction::from_keycode(code) {
                            if kind == KeyEventKind::Release {
                                game.release_direction(dir);
                            }
                        }
                        if kind == KeyEventKind::Press {
                            match code {
                                KeyCode::Char('p') | KeyCode::Char('P') => {
                                    game.state = GameState::Playing;
                                    game.last_update = Instant::now();
                                }
                                KeyCode::Char('r') | KeyCode::Char('R') => {
                                    game = Game::new(game_w, game_h, high_score);
                                }
                                KeyCode::Char('h') | KeyCode::Char('H') => {
                                    game.state = GameState::Help;
                                }
                                KeyCode::Char('q') | KeyCode::Char('Q') => {
                                    game.state = GameState::QuitConfirm;
                                }
                                _ => {}
                            }
                        }
                    }
                    GameState::Help => {
                        if let Some(dir) = Direction::from_keycode(code) {
                            if kind == KeyEventKind::Release {
                                game.release_direction(dir);
                            }
                        }
                        if kind == KeyEventKind::Press {
                            game.state = GameState::Playing;
                        }
                    }
                    GameState::GameOver => {
                        if kind == KeyEventKind::Press {
                            match code {
                                KeyCode::Char('r') | KeyCode::Char('R') => {
                                    game = Game::new(game_w, game_h, high_score);
                                }
                                KeyCode::Char('q') | KeyCode::Char('Q') => {
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    GameState::QuitConfirm => {
                        if kind == KeyEventKind::Press {
                            match code {
                                KeyCode::Char('y') | KeyCode::Char('Y') => {
                                    break;
                                }
                                KeyCode::Char('n') | KeyCode::Char('N') => {
                                    game.state = GameState::Playing;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    execute!(stdout, cursor::Show, terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;

    println!("感谢游玩！最终分数: {}", game.score);

    Ok(())
}
