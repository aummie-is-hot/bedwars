use macroquad::prelude::*;
use std::collections::HashMap;

const TILE_SIZE: f32 = 36.0;
const MAP_W: usize = 44;
const MAP_H: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Team {
    Red,
    Blue,
    Green,
    Yellow,
}

impl Team {
    fn color(self) -> Color {
        match self {
            Team::Red => Color::new(0.92, 0.22, 0.24, 1.0),
            Team::Blue => Color::new(0.20, 0.52, 0.95, 1.0),
            Team::Green => Color::new(0.20, 0.80, 0.38, 1.0),
            Team::Yellow => Color::new(0.96, 0.82, 0.18, 1.0),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Team::Red => "RED",
            Team::Blue => "BLUE",
            Team::Green => "GREEN",
            Team::Yellow => "YELLOW",
        }
    }

    fn all() -> [Team; 4] {
        [Team::Red, Team::Blue, Team::Green, Team::Yellow]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tile {
    Void,
    BaseFloor(Team),
    MidFloor,
    PlacedWool(Team),
    PlacedEndstone(Team),
    Bed(Team),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ResourceKind {
    Iron,
    Gold,
    Emerald,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ProjectileKind {
    Arrow,
    Fireball,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum AiRole {
    Rusher,
    Defender,
    Gatherer,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum AiGoal {
    DefendBed,
    BreakBed(Team),
    EngageEnemy(usize),
    GatherResources(Vec2),
    RetreatHeal,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct TeamUpgrades {
    sharpness: u8,    // +12% melee damage per level (max 3)
    protection: u8,   // -12% incoming damage per level (max 3)
    forge_level: u8,  // faster generator interval (max 3)
    heal_pool: bool,  // regenerates HP near own base
}

impl Default for TeamUpgrades {
    fn default() -> Self {
        Self {
            sharpness: 0,
            protection: 0,
            forge_level: 0,
            heal_pool: false,
        }
    }
}

struct Generator {
    pos: Vec2,
    kind: ResourceKind,
    base_interval: f32,
    timer: f32,
    home_team: Option<Team>,
}

struct Pickup {
    pos: Vec2,
    kind: ResourceKind,
    amount: u32,
    bob: f32,
}

struct Projectile {
    pos: Vec2,
    vel: Vec2,
    kind: ProjectileKind,
    owner_id: usize,
    team: Team,
    damage: f32,
    knockback: f32,
    life: f32,
}

struct TntEntity {
    pos: Vec2,
    vel: Vec2,
    fuse: f32,
    owner_team: Team,
}

struct Particle {
    pos: Vec2,
    vel: Vec2,
    color: Color,
    size: f32,
    life: f32,
    max_life: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum WeaponTier {
    Fist,
    WoodSword,
    StoneSword,
    IronSword,
    DiamondSword,
}

impl WeaponTier {
    fn damage(self) -> f32 {
        match self {
            WeaponTier::Fist => 16.0,
            WeaponTier::WoodSword => 28.0,
            WeaponTier::StoneSword => 40.0,
            WeaponTier::IronSword => 54.0,
            WeaponTier::DiamondSword => 68.0,
        }
    }

    fn label(self) -> &'static str {
        match self {
            WeaponTier::Fist => "Fist",
            WeaponTier::WoodSword => "Wood Sword",
            WeaponTier::StoneSword => "Stone Sword",
            WeaponTier::IronSword => "Iron Sword",
            WeaponTier::DiamondSword => "Diamond Sword",
        }
    }
}

struct Player {
    id: usize,
    name: String,
    team: Team,
    is_human: bool,
    pos: Vec2,
    vel: Vec2,
    aim_dir: Vec2,
    hp: f32,
    max_hp: f32,
    iron: u32,
    gold: u32,
    emerald: u32,
    wool: u32,
    endstone: u32,
    tnt: u32,
    fireballs: u32,
    arrows: u32,
    has_knockback_stick: bool,
    weapon: WeaponTier,
    attack_cooldown: f32,
    hurt_flash: f32,
    respawn_timer: f32,
    alive: bool,
    kills: u32,
    beds_broken: u32,
    // Advanced AI fields
    ai_role: AiRole,
    ai_goal: AiGoal,
    ai_decision_timer: f32,
    ai_strafe_dir: f32,
    ai_strafe_timer: f32,
    ai_block_break_progress: f32,
    ai_last_break_tile: Option<(isize, isize)>,
}

impl Player {
    fn new(id: usize, name: &str, team: Team, is_human: bool, spawn: Vec2, role: AiRole) -> Self {
        Self {
            id,
            name: name.to_string(),
            team,
            is_human,
            pos: spawn,
            vel: Vec2::ZERO,
            aim_dir: vec2(1.0, 0.0),
            hp: 100.0,
            max_hp: 100.0,
            iron: 0,
            gold: 0,
            emerald: 0,
            wool: 16,
            endstone: 0,
            tnt: 0,
            fireballs: 0,
            arrows: 0,
            has_knockback_stick: false,
            weapon: WeaponTier::Fist,
            attack_cooldown: 0.0,
            hurt_flash: 0.0,
            respawn_timer: 0.0,
            alive: true,
            kills: 0,
            beds_broken: 0,
            ai_role: role,
            ai_goal: AiGoal::GatherResources(spawn),
            ai_decision_timer: 0.15,
            ai_strafe_dir: 1.0,
            ai_strafe_timer: 0.6,
            ai_block_break_progress: 0.0,
            ai_last_break_tile: None,
        }
    }
}

struct GameState {
    grid: Vec<Vec<Tile>>,
    spawns: HashMap<Team, Vec2>,
    bed_positions: HashMap<Team, (usize, usize)>,
    bed_alive: HashMap<Team, bool>,
    team_upgrades: HashMap<Team, TeamUpgrades>,
    generators: Vec<Generator>,
    pickups: Vec<Pickup>,
    players: Vec<Player>,
    projectiles: Vec<Projectile>,
    tnts: Vec<TntEntity>,
    particles: Vec<Particle>,
    shop_open: bool,
    upgrades_open: bool,
    paused: bool,
    status_banner: String,
    status_timer: f32,
    winner: Option<Team>,
    elapsed: f32,
}

impl GameState {
    fn new() -> Self {
        let mut grid = vec![vec![Tile::Void; MAP_W]; MAP_H];
        let mut spawns = HashMap::new();
        let mut bed_positions = HashMap::new();
        let mut bed_alive = HashMap::new();
        let mut team_upgrades = HashMap::new();
        let mut generators = Vec::new();

        // Four Corner Base Islands: (Team, top-left x, top-left y, bed_x, bed_y, spawn_x, spawn_y, gen_x, gen_y, role)
        let bases = [
            (Team::Red, 3, 3, 6, 6, 5.5, 5.5, 4.5, 4.5, AiRole::Rusher),
            (Team::Blue, 33, 3, 37, 6, 38.5, 5.5, 39.5, 4.5, AiRole::Rusher),
            (Team::Green, 3, 22, 6, 25, 5.5, 26.5, 4.5, 27.5, AiRole::Defender),
            (Team::Yellow, 33, 22, 37, 25, 38.5, 26.5, 39.5, 27.5, AiRole::Gatherer),
        ];

        {
            let mut fill_rect = |x0: usize, y0: usize, w: usize, h: usize, tile: Tile| {
                for y in y0..(y0 + h).min(MAP_H) {
                    for x in x0..(x0 + w).min(MAP_W) {
                        grid[y][x] = tile;
                    }
                }
            };

            // Center Mid Island + Side Emerald Platforms
            fill_rect(18, 12, 8, 8, Tile::MidFloor);
            fill_rect(21, 5, 3, 3, Tile::MidFloor);
            fill_rect(21, 24, 3, 3, Tile::MidFloor);

            for (team, bx, by, _bed_x, _bed_y, _sx, _sy, _gx, _gy, _role) in bases {
                fill_rect(bx, by, 8, 7, Tile::BaseFloor(team));
            }
        }

        // Mid & Emerald Generators
        generators.push(Generator {
            pos: vec2(21.5 * TILE_SIZE, 15.5 * TILE_SIZE),
            kind: ResourceKind::Gold,
            base_interval: 4.0,
            timer: 1.0,
            home_team: None,
        });
        generators.push(Generator {
            pos: vec2(22.5 * TILE_SIZE, 16.5 * TILE_SIZE),
            kind: ResourceKind::Iron,
            base_interval: 1.8,
            timer: 0.4,
            home_team: None,
        });
        generators.push(Generator {
            pos: vec2(22.5 * TILE_SIZE, 6.5 * TILE_SIZE),
            kind: ResourceKind::Emerald,
            base_interval: 8.5,
            timer: 3.0,
            home_team: None,
        });
        generators.push(Generator {
            pos: vec2(22.5 * TILE_SIZE, 25.5 * TILE_SIZE),
            kind: ResourceKind::Emerald,
            base_interval: 8.5,
            timer: 3.0,
            home_team: None,
        });

        for (team, _bx, _by, bed_x, bed_y, sx, sy, gx, gy, _role) in bases {
            grid[bed_y][bed_x] = Tile::Bed(team);
            bed_positions.insert(team, (bed_x, bed_y));
            bed_alive.insert(team, true);
            team_upgrades.insert(team, TeamUpgrades::default());
            spawns.insert(team, vec2(sx * TILE_SIZE, sy * TILE_SIZE));
            generators.push(Generator {
                pos: vec2(gx * TILE_SIZE, gy * TILE_SIZE),
                kind: ResourceKind::Iron,
                base_interval: 1.35,
                timer: 0.3,
                home_team: Some(team),
            });
            generators.push(Generator {
                pos: vec2((gx + 1.0) * TILE_SIZE, (gy + 1.0) * TILE_SIZE),
                kind: ResourceKind::Gold,
                base_interval: 5.8,
                timer: 1.5,
                home_team: Some(team),
            });
        }

        let players = vec![
            Player::new(0, "You", Team::Red, true, spawns[&Team::Red], AiRole::Rusher),
            Player::new(1, "Vortex", Team::Blue, false, spawns[&Team::Blue], AiRole::Rusher),
            Player::new(2, "Bastion", Team::Green, false, spawns[&Team::Green], AiRole::Defender),
            Player::new(3, "Scavenger", Team::Yellow, false, spawns[&Team::Yellow], AiRole::Gatherer),
        ];

        Self {
            grid,
            spawns,
            bed_positions,
            bed_alive,
            team_upgrades,
            generators,
            pickups: Vec::new(),
            players,
            projectiles: Vec::new(),
            tnts: Vec::new(),
            particles: Vec::new(),
            shop_open: false,
            upgrades_open: false,
            paused: false,
            status_banner: "Protect your BED! Use TNT (F), Fireballs (G), Endstone & Team Upgrades (U).".to_string(),
            status_timer: 5.0,
            winner: None,
            elapsed: 0.0,
        }
    }

    fn set_banner(&mut self, msg: impl Into<String>, duration: f32) {
        self.status_banner = msg.into();
        self.status_timer = duration;
    }

    fn world_to_tile(pos: Vec2) -> (isize, isize) {
        (
            (pos.x / TILE_SIZE).floor() as isize,
            (pos.y / TILE_SIZE).floor() as isize,
        )
    }

    fn tile_center(tx: isize, ty: isize) -> Vec2 {
        vec2((tx as f32 + 0.5) * TILE_SIZE, (ty as f32 + 0.5) * TILE_SIZE)
    }

    fn tile_at(&self, tx: isize, ty: isize) -> Tile {
        if tx < 0 || ty < 0 || tx >= MAP_W as isize || ty >= MAP_H as isize {
            Tile::Void
        } else {
            self.grid[ty as usize][tx as usize]
        }
    }

    fn is_walkable_tile(tile: Tile) -> bool {
        !matches!(tile, Tile::Void)
    }

    fn is_pos_on_ground(&self, pos: Vec2) -> bool {
        let offsets = [
            vec2(-7.0, -7.0),
            vec2(7.0, -7.0),
            vec2(-7.0, 7.0),
            vec2(7.0, 7.0),
            vec2(0.0, 0.0),
        ];
        offsets.iter().any(|&off| {
            let (tx, ty) = Self::world_to_tile(pos + off);
            Self::is_walkable_tile(self.tile_at(tx, ty))
        })
    }

    fn spawn_particles(&mut self, pos: Vec2, color: Color, count: usize, speed: f32) {
        for _ in 0..count {
            let angle = rand::gen_range(0.0, std::f32::consts::TAU);
            let spd = rand::gen_range(speed * 0.25, speed);
            self.particles.push(Particle {
                pos,
                vel: vec2(angle.cos() * spd, angle.sin() * spd),
                color,
                size: rand::gen_range(3.0, 7.0),
                life: rand::gen_range(0.25, 0.6),
                max_life: 0.6,
            });
        }
    }

    fn apply_damage(&mut self, victim_idx: usize, attacker_idx: Option<usize>, raw_dmg: f32, kb: Vec2, reason: &str) {
        if !self.players[victim_idx].alive {
            return;
        }
        let v_team = self.players[victim_idx].team;
        let prot_lvl = self.team_upgrades.get(&v_team).map(|u| u.protection).unwrap_or(0);
        let mult = (1.0 - 0.12 * prot_lvl as f32).max(0.4);
        let final_dmg = raw_dmg * mult;

        self.players[victim_idx].hp -= final_dmg;
        self.players[victim_idx].hurt_flash = 0.16;
        self.players[victim_idx].vel += kb;
        self.spawn_particles(self.players[victim_idx].pos, v_team.color(), 9, 125.0);

        if self.players[victim_idx].hp <= 0.0 {
            if let Some(a_idx) = attacker_idx {
                if a_idx < self.players.len() && self.players[a_idx].team != v_team {
                    self.players[a_idx].kills += 1;
                }
            }
            self.kill_player(victim_idx, reason);
        }
    }

    fn kill_player(&mut self, idx: usize, reason: &str) {
        let (team, name, pos, bed_intact) = {
            let p = &mut self.players[idx];
            if !p.alive {
                return;
            }
            p.alive = false;
            p.vel = Vec2::ZERO;
            let intact = self.bed_alive.get(&p.team).copied().unwrap_or(false);
            p.respawn_timer = if intact { 3.2 } else { -1.0 };
            (p.team, p.name.clone(), p.pos, intact)
        };

        self.spawn_particles(pos, team.color(), 24, 175.0);

        if bed_intact {
            self.set_banner(format!("{} {} ({}) — Respawning...", name, reason, team.name()), 2.4);
        } else {
            self.set_banner(format!("FINAL KILL! {} {} ({}) — Eliminated!", name, reason, team.name()), 3.4);
        }
    }

    fn try_place_block_at(&mut self, player_idx: usize, world_pos: Vec2, prefer_endstone: bool) -> bool {
        let (tx, ty) = Self::world_to_tile(world_pos);
        if tx < 1 || ty < 1 || tx >= (MAP_W - 1) as isize || ty >= (MAP_H - 1) as isize {
            return false;
        }
        if self.tile_at(tx, ty) != Tile::Void {
            return false;
        }
        let center = Self::tile_center(tx, ty);
        for p in &self.players {
            if p.alive && p.pos.distance(center) < TILE_SIZE * 0.65 {
                return false;
            }
        }

        let team = self.players[player_idx].team;
        if prefer_endstone && self.players[player_idx].endstone > 0 {
            self.players[player_idx].endstone -= 1;
            self.grid[ty as usize][tx as usize] = Tile::PlacedEndstone(team);
            self.spawn_particles(center, Color::new(0.85, 0.82, 0.62, 1.0), 7, 75.0);
            return true;
        }
        if self.players[player_idx].wool > 0 {
            self.players[player_idx].wool -= 1;
            self.grid[ty as usize][tx as usize] = Tile::PlacedWool(team);
            self.spawn_particles(center, team.color(), 6, 70.0);
            return true;
        }
        if self.players[player_idx].endstone > 0 {
            self.players[player_idx].endstone -= 1;
            self.grid[ty as usize][tx as usize] = Tile::PlacedEndstone(team);
            self.spawn_particles(center, Color::new(0.85, 0.82, 0.62, 1.0), 7, 75.0);
            return true;
        }
        false
    }

    fn try_break_tile_at(&mut self, player_idx: usize, world_pos: Vec2) -> bool {
        let (tx, ty) = Self::world_to_tile(world_pos);
        let tile = self.tile_at(tx, ty);
        let p_team = self.players[player_idx].team;
        let p_name = self.players[player_idx].name.clone();
        let center = Self::tile_center(tx, ty);

        match tile {
            Tile::PlacedWool(b_team) => {
                self.grid[ty as usize][tx as usize] = Tile::Void;
                if b_team == p_team {
                    self.players[player_idx].wool += 1;
                }
                self.spawn_particles(center, b_team.color(), 8, 85.0);
                true
            }
            Tile::PlacedEndstone(b_team) => {
                self.grid[ty as usize][tx as usize] = Tile::Void;
                if b_team == p_team {
                    self.players[player_idx].endstone += 1;
                }
                self.spawn_particles(center, Color::new(0.85, 0.82, 0.62, 1.0), 10, 95.0);
                true
            }
            Tile::Bed(bed_team) => {
                if bed_team == p_team {
                    self.set_banner("You can't break your own bed!", 1.5);
                    return false;
                }
                self.grid[ty as usize][tx as usize] = Tile::BaseFloor(bed_team);
                self.bed_alive.insert(bed_team, false);
                self.players[player_idx].beds_broken += 1;
                self.spawn_particles(center, bed_team.color(), 34, 210.0);
                self.set_banner(
                    format!("{} DESTROYED {}'S BED!", p_name, bed_team.name()),
                    4.0,
                );
                true
            }
            _ => false,
        }
    }

    fn place_tnt(&mut self, player_idx: usize) -> bool {
        let p = &mut self.players[player_idx];
        if !p.alive || p.tnt == 0 {
            return false;
        }
        p.tnt -= 1;
        let pos = p.pos + p.aim_dir.normalize_or_zero() * 16.0;
        let vel = p.aim_dir.normalize_or_zero() * 90.0;
        let team = p.team;
        self.tnts.push(TntEntity {
            pos,
            vel,
            fuse: 1.5,
            owner_team: team,
        });
        true
    }

    fn shoot_fireball(&mut self, shooter_idx: usize) -> bool {
        let p = &mut self.players[shooter_idx];
        if !p.alive || p.fireballs == 0 || p.attack_cooldown > 0.0 {
            return false;
        }
        p.fireballs -= 1;
        p.attack_cooldown = 0.4;
        let dir = if p.aim_dir.length_squared() > 0.01 {
            p.aim_dir.normalize()
        } else {
            vec2(1.0, 0.0)
        };
        let pos = p.pos + dir * 16.0;
        let id = p.id;
        let team = p.team;

        self.projectiles.push(Projectile {
            pos,
            vel: dir * 430.0,
            kind: ProjectileKind::Fireball,
            owner_id: id,
            team,
            damage: 34.0,
            knockback: 420.0,
            life: 1.8,
        });
        true
    }

    fn trigger_explosion(&mut self, center: Vec2, radius_tiles: f32, owner_team: Team) {
        self.spawn_particles(center, ORANGE, 26, 220.0);
        self.spawn_particles(center, RED, 14, 150.0);

        let r_px = radius_tiles * TILE_SIZE;
        let min_tx = ((center.x - r_px) / TILE_SIZE).floor() as isize;
        let max_tx = ((center.x + r_px) / TILE_SIZE).floor() as isize;
        let min_ty = ((center.y - r_px) / TILE_SIZE).floor() as isize;
        let max_ty = ((center.y + r_px) / TILE_SIZE).floor() as isize;

        for ty in min_ty..=max_ty {
            for tx in min_tx..=max_tx {
                if tx < 1 || ty < 1 || tx >= (MAP_W - 1) as isize || ty >= (MAP_H - 1) as isize {
                    continue;
                }
                let t_pos = Self::tile_center(tx, ty);
                if t_pos.distance(center) <= r_px {
                    match self.tile_at(tx, ty) {
                        Tile::PlacedWool(_) => {
                            self.grid[ty as usize][tx as usize] = Tile::Void;
                        }
                        Tile::PlacedEndstone(_) => {
                            if t_pos.distance(center) <= r_px * 0.55 {
                                self.grid[ty as usize][tx as usize] = Tile::Void;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        let attacker_idx = self.players.iter().position(|p| p.team == owner_team && p.is_human).or_else(|| {
            self.players.iter().position(|p| p.team == owner_team)
        });

        let mut hits = Vec::new();
        for (i, pl) in self.players.iter().enumerate() {
            if pl.alive {
                let d = pl.pos.distance(center);
                if d <= r_px * 1.15 {
                    let falloff = (1.0 - (d / (r_px * 1.15))).clamp(0.2, 1.0);
                    let push = (pl.pos - center).normalize_or_zero() * (560.0 * falloff);
                    let dmg = if pl.team == owner_team { 14.0 * falloff } else { 46.0 * falloff };
                    hits.push((i, dmg, push));
                }
            }
        }

        for (i, dmg, push) in hits {
            self.apply_damage(i, attacker_idx, dmg, push, "was blown up");
        }
    }

    fn melee_attack(&mut self, attacker_idx: usize) {
        let (pos, aim, team, dmg, kb_stick) = {
            let a = &mut self.players[attacker_idx];
            if !a.alive || a.attack_cooldown > 0.0 {
                return;
            }
            a.attack_cooldown = 0.34;
            let dir = if a.aim_dir.length_squared() > 0.01 {
                a.aim_dir.normalize()
            } else {
                vec2(1.0, 0.0)
            };
            let sharp = self.team_upgrades.get(&a.team).map(|u| u.sharpness).unwrap_or(0);
            let total_dmg = a.weapon.damage() * (1.0 + 0.12 * sharp as f32);
            (a.pos, dir, a.team, total_dmg, a.has_knockback_stick)
        };

        let reach = 44.0;
        let hit_center = pos + aim * 22.0;
        self.spawn_particles(hit_center, WHITE, 5, 65.0);

        let mut hits = Vec::new();
        for (i, pl) in self.players.iter().enumerate() {
            if i == attacker_idx || !pl.alive || pl.team == team {
                continue;
            }
            if pl.pos.distance(hit_center) <= reach {
                let kb_strength = if kb_stick { 540.0 } else { 245.0 };
                let push_dir = (pl.pos - pos).normalize_or_zero() * kb_strength;
                hits.push((i, dmg, push_dir));
            }
        }

        for (v, d, kb) in hits {
            self.apply_damage(v, Some(attacker_idx), d, kb, "was eliminated in combat");
        }
    }

    fn shoot_bow(&mut self, shooter_idx: usize) {
        let (pos, aim, id, team) = {
            let p = &mut self.players[shooter_idx];
            if !p.alive || p.arrows == 0 || p.attack_cooldown > 0.0 {
                return;
            }
            p.arrows -= 1;
            p.attack_cooldown = 0.42;
            let dir = if p.aim_dir.length_squared() > 0.01 {
                p.aim_dir.normalize()
            } else {
                vec2(1.0, 0.0)
            };
            (p.pos + dir * 14.0, dir, p.id, p.team)
        };

        self.projectiles.push(Projectile {
            pos,
            vel: aim * 580.0,
            kind: ProjectileKind::Arrow,
            owner_id: id,
            team,
            damage: 26.0,
            knockback: 270.0,
            life: 1.4,
        });
    }

    fn buy_item(&mut self, idx: usize, item_id: u8) -> bool {
        let p = &mut self.players[idx];
        match item_id {
            1 => {
                if p.iron >= 4 {
                    p.iron -= 4;
                    p.wool += 16;
                    true
                } else {
                    false
                }
            }
            2 => {
                if p.iron >= 10 && (p.weapon as u8) < (WeaponTier::StoneSword as u8) {
                    p.iron -= 10;
                    p.weapon = WeaponTier::StoneSword;
                    true
                } else {
                    false
                }
            }
            3 => {
                if p.gold >= 7 && (p.weapon as u8) < (WeaponTier::IronSword as u8) {
                    p.gold -= 7;
                    p.weapon = WeaponTier::IronSword;
                    true
                } else {
                    false
                }
            }
            4 => {
                if p.gold >= 5 && !p.has_knockback_stick {
                    p.gold -= 5;
                    p.has_knockback_stick = true;
                    true
                } else {
                    false
                }
            }
            5 => {
                if p.gold >= 6 {
                    p.gold -= 6;
                    p.arrows += 6;
                    true
                } else {
                    false
                }
            }
            6 => {
                if p.iron >= 8 {
                    p.iron -= 8;
                    p.max_hp = (p.max_hp + 20.0).min(160.0);
                    p.hp = p.max_hp;
                    true
                } else {
                    false
                }
            }
            7 => {
                if p.gold >= 8 {
                    p.gold -= 8;
                    p.tnt += 1;
                    true
                } else {
                    false
                }
            }
            8 => {
                if p.gold >= 6 {
                    p.gold -= 6;
                    p.fireballs += 1;
                    true
                } else {
                    false
                }
            }
            9 => {
                if p.iron >= 12 {
                    p.iron -= 12;
                    p.endstone += 8;
                    true
                } else {
                    false
                }
            }
            10 => {
                if p.emerald >= 4 && (p.weapon as u8) < (WeaponTier::DiamondSword as u8) {
                    p.emerald -= 4;
                    p.weapon = WeaponTier::DiamondSword;
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn buy_team_upgrade(&mut self, player_idx: usize, upgrade_id: u8) -> bool {
        let (team, gold, emerald) = {
            let p = &self.players[player_idx];
            (p.team, p.gold, p.emerald)
        };
        let upg = self.team_upgrades.entry(team).or_default();

        match upgrade_id {
            1 => {
                if upg.sharpness < 3 && gold >= 4 {
                    self.players[player_idx].gold -= 4;
                    upg.sharpness += 1;
                    true
                } else {
                    false
                }
            }
            2 => {
                if upg.protection < 3 && gold >= 4 {
                    self.players[player_idx].gold -= 4;
                    upg.protection += 1;
                    true
                } else {
                    false
                }
            }
            3 => {
                if upg.forge_level < 3 && emerald >= 2 {
                    self.players[player_idx].emerald -= 2;
                    upg.forge_level += 1;
                    true
                } else {
                    false
                }
            }
            4 => {
                if !upg.heal_pool && emerald >= 3 {
                    self.players[player_idx].emerald -= 3;
                    upg.heal_pool = true;
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn update_ai(&mut self, idx: usize, dt: f32) {
        let (pos, team, hp, max_hp, wool, endstone, iron, gold, emerald, weapon, has_kb, arrows, tnt, role) = {
            let p = &self.players[idx];
            if !p.alive {
                return;
            }
            (
                p.pos,
                p.team,
                p.hp,
                p.max_hp,
                p.wool,
                p.endstone,
                p.iron,
                p.gold,
                p.emerald,
                p.weapon,
                p.has_knockback_stick,
                p.arrows,
                p.tnt,
                p.ai_role,
            )
        };

        let home = self.spawns[&team];

        if pos.distance(home) < TILE_SIZE * 5.5 {
            if emerald >= 4 && (weapon as u8) < (WeaponTier::DiamondSword as u8) {
                self.buy_item(idx, 10);
            } else if emerald >= 2 && self.team_upgrades[&team].forge_level < 3 {
                self.buy_team_upgrade(idx, 3);
            } else if gold >= 7 && (weapon as u8) < (WeaponTier::IronSword as u8) {
                self.buy_item(idx, 3);
            } else if iron >= 10 && (weapon as u8) < (WeaponTier::StoneSword as u8) {
                self.buy_item(idx, 2);
            } else if gold >= 5 && !has_kb {
                self.buy_item(idx, 4);
            } else if matches!(role, AiRole::Defender) && iron >= 12 && endstone < 8 {
                self.buy_item(idx, 9);
            } else if gold >= 8 && tnt == 0 && matches!(role, AiRole::Rusher) {
                self.buy_item(idx, 7);
            } else if gold >= 6 && arrows < 4 {
                self.buy_item(idx, 5);
            } else if iron >= 4 && wool < 10 {
                self.buy_item(idx, 1);
            }
        }

        self.players[idx].ai_decision_timer -= dt;
        if self.players[idx].ai_decision_timer <= 0.0 {
            self.players[idx].ai_decision_timer = rand::gen_range(0.22, 0.45);

            if hp < max_hp * 0.34 {
                self.players[idx].ai_goal = AiGoal::RetreatHeal;
            } else {
                let mut nearest_enemy: Option<(usize, f32)> = None;
                for (i, other) in self.players.iter().enumerate() {
                    if i != idx && other.alive && other.team != team {
                        let d = pos.distance(other.pos);
                        if nearest_enemy.map(|(_, bd)| d < bd).unwrap_or(true) {
                            nearest_enemy = Some((i, d));
                        }
                    }
                }

                let my_bed_pos = self.bed_positions.get(&team).map(|&(bx, by)| Self::tile_center(bx as isize, by as isize));
                let my_bed_alive = self.bed_alive.get(&team).copied().unwrap_or(false);
                let enemy_near_bed = my_bed_pos.and_then(|bp| {
                    self.players.iter().enumerate().find(|(i, o)| {
                        *i != idx && o.alive && o.team != team && o.pos.distance(bp) < TILE_SIZE * 5.5
                    }).map(|(i, _)| i)
                });

                let mut best_bed: Option<(Team, f32)> = None;
                for t in Team::all() {
                    if t != team && self.bed_alive.get(&t).copied().unwrap_or(false) {
                        if let Some(&(bx, by)) = self.bed_positions.get(&t) {
                            let b_pos = Self::tile_center(bx as isize, by as isize);
                            let d = pos.distance(b_pos);
                            if best_bed.map(|(_, bd)| d < bd).unwrap_or(true) {
                                best_bed = Some((t, d));
                            }
                        }
                    }
                }

                let mut best_goal = AiGoal::GatherResources(home + vec2(18.0, 18.0));
                let mut best_score = -1.0f32;

                let mut gather_score = if wool < 6 { 75.0 } else { 35.0 };
                if matches!(role, AiRole::Gatherer) && (gold < 7 || emerald < 2) {
                    gather_score += 35.0;
                }
                if let Some(pk) = self.pickups.iter().min_by(|a, b| {
                    a.pos.distance(pos).partial_cmp(&b.pos.distance(pos)).unwrap_or(std::cmp::Ordering::Equal)
                }) {
                    if pk.pos.distance(pos) < TILE_SIZE * 8.0 {
                        best_goal = AiGoal::GatherResources(pk.pos);
                        best_score = gather_score + 15.0;
                    }
                }
                if gather_score > best_score {
                    best_score = gather_score;
                    best_goal = AiGoal::GatherResources(vec2(22.0 * TILE_SIZE, 16.0 * TILE_SIZE));
                }

                if my_bed_alive {
                    let mut def_score = if matches!(role, AiRole::Defender) { 68.0 } else { 30.0 };
                    if let Some(intruder_idx) = enemy_near_bed {
                        def_score += 55.0;
                        if def_score > best_score {
                            best_score = def_score;
                            best_goal = AiGoal::EngageEnemy(intruder_idx);
                        }
                    } else if def_score > best_score {
                        best_score = def_score;
                        best_goal = AiGoal::DefendBed;
                    }
                }

                if let Some((b_team, dist)) = best_bed {
                    let mut break_score = if wool >= 8 { 85.0 } else { 42.0 };
                    if matches!(role, AiRole::Rusher) {
                        break_score += 22.0;
                    }
                    break_score -= (dist / TILE_SIZE) * 1.1;
                    if break_score > best_score {
                        best_score = break_score;
                        best_goal = AiGoal::BreakBed(b_team);
                    }
                }

                if let Some((e_idx, dist)) = nearest_enemy {
                    if dist < TILE_SIZE * 6.5 {
                        let mut fight_score = 72.0 - (dist / TILE_SIZE) * 4.0;
                        if hp > self.players[e_idx].hp {
                            fight_score += 15.0;
                        }
                        if fight_score > best_score {
                            best_goal = AiGoal::EngageEnemy(e_idx);
                        }
                    }
                }

                self.players[idx].ai_goal = best_goal;
            }
        }

        let goal = self.players[idx].ai_goal;
        let target_pos = match goal {
            AiGoal::RetreatHeal => home,
            AiGoal::DefendBed => {
                if let Some(&(bx, by)) = self.bed_positions.get(&team) {
                    Self::tile_center(bx as isize, by as isize) + vec2(26.0, 26.0)
                } else {
                    home
                }
            }
            AiGoal::BreakBed(b_team) => {
                if let Some(&(bx, by)) = self.bed_positions.get(&b_team) {
                    Self::tile_center(bx as isize, by as isize)
                } else {
                    vec2(22.0 * TILE_SIZE, 16.0 * TILE_SIZE)
                }
            }
            AiGoal::EngageEnemy(e_idx) => {
                if e_idx < self.players.len() && self.players[e_idx].alive {
                    self.players[e_idx].pos
                } else {
                    home
                }
            }
            AiGoal::GatherResources(g_pos) => g_pos,
        };

        if matches!(goal, AiGoal::DefendBed) && (wool > 0 || endstone > 0) {
            if let Some(&(bx, by)) = self.bed_positions.get(&team) {
                let bxi = bx as isize;
                let byi = by as isize;
                let ring = [(bxi - 1, byi), (bxi + 1, byi), (bxi, byi - 1), (bxi, byi + 1)];
                for (rx, ry) in ring {
                    if self.tile_at(rx, ry) == Tile::Void {
                        let r_center = Self::tile_center(rx, ry);
                        if pos.distance(r_center) < TILE_SIZE * 2.4 {
                            self.try_place_block_at(idx, r_center, true);
                            break;
                        }
                    }
                }
            }
        }

        self.players[idx].ai_strafe_timer -= dt;
        if self.players[idx].ai_strafe_timer <= 0.0 {
            self.players[idx].ai_strafe_timer = rand::gen_range(0.35, 0.85);
            self.players[idx].ai_strafe_dir = if rand::gen_range(0.0, 1.0) < 0.5 { -1.0 } else { 1.0 };
        }

        let to_target = target_pos - pos;
        let dist_to_target = to_target.length();
        let mut dir = if dist_to_target > 1.0 {
            to_target / dist_to_target
        } else {
            Vec2::ZERO
        };

        if matches!(goal, AiGoal::EngageEnemy(_)) && dist_to_target < TILE_SIZE * 3.0 && dist_to_target > 24.0 {
            let perp = vec2(-dir.y, dir.x) * self.players[idx].ai_strafe_dir * 0.55;
            dir = (dir + perp).normalize_or_zero();
        }

        if dir.length_squared() > 0.01 {
            self.players[idx].aim_dir = dir;
        }

        let lookahead = pos + dir * (TILE_SIZE * 0.85);
        let (ltx, lty) = Self::world_to_tile(lookahead);
        if self.tile_at(ltx, lty) == Tile::Void && (self.players[idx].wool > 0 || self.players[idx].endstone > 0) {
            self.try_place_block_at(idx, lookahead, false);
        }

        let front_pt = pos + dir * 28.0;
        let (ftx, fty) = Self::world_to_tile(front_pt);
        let front_tile = self.tile_at(ftx, fty);
        match front_tile {
            Tile::Bed(bt) if bt != team => {
                self.try_break_tile_at(idx, front_pt);
            }
            Tile::PlacedWool(_) | Tile::PlacedEndstone(_) => {
                if self.players[idx].ai_last_break_tile == Some((ftx, fty)) {
                    self.players[idx].ai_block_break_progress += dt;
                    let req = if matches!(front_tile, Tile::PlacedEndstone(_)) { 0.85 } else { 0.35 };
                    if self.players[idx].ai_block_break_progress >= req {
                        self.try_break_tile_at(idx, front_pt);
                        self.players[idx].ai_block_break_progress = 0.0;
                    }
                } else {
                    self.players[idx].ai_last_break_tile = Some((ftx, fty));
                    self.players[idx].ai_block_break_progress = 0.0;
                }
            }
            _ => {
                self.players[idx].ai_last_break_tile = None;
                self.players[idx].ai_block_break_progress = 0.0;
            }
        }

        if matches!(goal, AiGoal::BreakBed(_)) && self.players[idx].tnt > 0 && dist_to_target < TILE_SIZE * 2.5 {
            self.place_tnt(idx);
        }

        if let AiGoal::EngageEnemy(e_idx) = goal {
            if e_idx < self.players.len() && self.players[e_idx].alive {
                let edist = pos.distance(self.players[e_idx].pos);
                if edist > TILE_SIZE * 2.8 && edist < TILE_SIZE * 8.0 && self.players[idx].arrows > 0 {
                    self.players[idx].aim_dir = (self.players[e_idx].pos - pos).normalize_or_zero();
                    self.shoot_bow(idx);
                }
            }
        }

        let should_swing = self.players.iter().enumerate().any(|(i, o)| {
            i != idx && o.alive && o.team != team && o.pos.distance(pos) < 44.0
        });
        if should_swing {
            self.melee_attack(idx);
        }

        let move_speed = 172.0;
        self.players[idx].vel = self.players[idx].vel.lerp(dir * move_speed, (dt * 11.0).min(1.0));
    }

    fn update_human_input(&mut self, idx: usize, dt: f32) {
        if !self.players[idx].alive {
            return;
        }

        if is_key_pressed(KeyCode::B) || is_key_pressed(KeyCode::Tab) {
            self.shop_open = !self.shop_open;
            if self.shop_open {
                self.upgrades_open = false;
            }
        }
        if is_key_pressed(KeyCode::U) {
            self.upgrades_open = !self.upgrades_open;
            if self.upgrades_open {
                self.shop_open = false;
            }
        }

        let mouse_screen = mouse_position();
        let world_mouse = vec2(mouse_screen.0, mouse_screen.1);
        let to_mouse = world_mouse - self.players[idx].pos;
        if to_mouse.length_squared() > 1.0 {
            self.players[idx].aim_dir = to_mouse.normalize();
        }

        if self.shop_open || self.upgrades_open {
            self.players[idx].vel = self.players[idx].vel.lerp(Vec2::ZERO, (dt * 12.0).min(1.0));
            return;
        }

        let mut move_dir = Vec2::ZERO;
        if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) {
            move_dir.y -= 1.0;
        }
        if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) {
            move_dir.y += 1.0;
        }
        if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) {
            move_dir.x -= 1.0;
        }
        if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) {
            move_dir.x += 1.0;
        }
        if move_dir.length_squared() > 0.0 {
            move_dir = move_dir.normalize();
        }

        let sprint_mult = if is_key_down(KeyCode::LeftShift) { 1.22 } else { 1.0 };
        let speed = 198.0 * sprint_mult;
        self.players[idx].vel = self.players[idx].vel.lerp(move_dir * speed, (dt * 12.0).min(1.0));

        if is_mouse_button_pressed(MouseButton::Left) || is_key_pressed(KeyCode::Space) {
            let reach_point = self.players[idx].pos + self.players[idx].aim_dir * 32.0;
            let (tx, ty) = Self::world_to_tile(reach_point);
            let tile = self.tile_at(tx, ty);
            let team = self.players[idx].team;
            let broke_special = match tile {
                Tile::Bed(bt) if bt != team => self.try_break_tile_at(idx, reach_point),
                Tile::PlacedWool(_) | Tile::PlacedEndstone(_) => self.try_break_tile_at(idx, reach_point),
                _ => false,
            };
            if !broke_special {
                self.melee_attack(idx);
            }
        }

        if is_mouse_button_pressed(MouseButton::Right) || is_key_pressed(KeyCode::E) {
            let reach_pos = if world_mouse.distance(self.players[idx].pos) <= TILE_SIZE * 2.4 {
                world_mouse
            } else {
                self.players[idx].pos + self.players[idx].aim_dir * (TILE_SIZE * 1.1)
            };
            let prefer_endstone = is_key_down(KeyCode::LeftShift);
            self.try_place_block_at(idx, reach_pos, prefer_endstone);
        }

        if is_key_pressed(KeyCode::Q) {
            self.shoot_bow(idx);
        }
        if is_key_pressed(KeyCode::F) {
            if self.place_tnt(idx) {
                self.set_banner("TNT planted! Fuse: 1.5s", 1.2);
            }
        }
        if is_key_pressed(KeyCode::G) {
            self.shoot_fireball(idx);
        }

        let num_keys = [
            (1u8, KeyCode::Key1),
            (2, KeyCode::Key2),
            (3, KeyCode::Key3),
            (4, KeyCode::Key4),
            (5, KeyCode::Key5),
            (6, KeyCode::Key6),
            (7, KeyCode::Key7),
            (8, KeyCode::Key8),
            (9, KeyCode::Key9),
            (10, KeyCode::Key0),
        ];
        for (item_id, key) in num_keys {
            if is_key_pressed(key) {
                if self.buy_item(idx, item_id) {
                    self.set_banner(format!("Purchased item #{}!", item_id), 1.2);
                }
            }
        }
    }

    fn update(&mut self, dt: f32) {
        if is_key_pressed(KeyCode::P) || is_key_pressed(KeyCode::Escape) {
            if self.shop_open {
                self.shop_open = false;
            } else if self.upgrades_open {
                self.upgrades_open = false;
            } else if self.winner.is_none() {
                self.paused = !self.paused;
            }
        }

        if self.paused {
            return;
        }

        self.elapsed += dt;
        if self.status_timer > 0.0 {
            self.status_timer -= dt;
        }

        if self.winner.is_some() {
            if is_key_pressed(KeyCode::R) {
                *self = GameState::new();
            }
            return;
        }

        for g in &mut self.generators {
            let forge_lvl = g
                .home_team
                .and_then(|t| self.team_upgrades.get(&t))
                .map(|u| u.forge_level)
                .unwrap_or(0);
            let speed_mult = 1.0 + 0.25 * forge_lvl as f32;
            g.timer -= dt * speed_mult;
            if g.timer <= 0.0 {
                g.timer = g.base_interval;
                let jitter = vec2(rand::gen_range(-10.0, 10.0), rand::gen_range(-10.0, 10.0));
                let new_pos = g.pos + jitter;
                if let Some(existing) = self.pickups.iter_mut().find(|pk| {
                    pk.kind == g.kind && pk.pos.distance(new_pos) < 18.0 && pk.amount < 16
                }) {
                    existing.amount += 1;
                } else {
                    self.pickups.push(Pickup {
                        pos: new_pos,
                        kind: g.kind,
                        amount: 1,
                        bob: rand::gen_range(0.0, 6.28),
                    });
                }
            }
        }

        let player_count = self.players.len();
        for i in 0..player_count {
            if self.players[i].attack_cooldown > 0.0 {
                self.players[i].attack_cooldown -= dt;
            }
            if self.players[i].hurt_flash > 0.0 {
                self.players[i].hurt_flash -= dt;
            }

            if !self.players[i].alive {
                if self.players[i].respawn_timer > 0.0 {
                    self.players[i].respawn_timer -= dt;
                    if self.players[i].respawn_timer <= 0.0 {
                        let team = self.players[i].team;
                        let spawn = self.spawns[&team];
                        self.players[i].alive = true;
                        self.players[i].pos = spawn;
                        self.players[i].vel = Vec2::ZERO;
                        self.players[i].hp = self.players[i].max_hp;
                        self.spawn_particles(spawn, team.color(), 16, 120.0);
                    }
                }
                continue;
            }

            let p_team = self.players[i].team;
            if self.team_upgrades.get(&p_team).map(|u| u.heal_pool).unwrap_or(false) {
                if let Some(&home) = self.spawns.get(&p_team) {
                    if self.players[i].pos.distance(home) < TILE_SIZE * 4.5 && self.players[i].hp < self.players[i].max_hp {
                        self.players[i].hp = (self.players[i].hp + 12.0 * dt).min(self.players[i].max_hp);
                    }
                }
            }

            if self.players[i].is_human {
                self.update_human_input(i, dt);
            } else {
                self.update_ai(i, dt);
            }

            let delta = self.players[i].vel * dt;
            self.players[i].pos += delta;
            self.players[i].pos.x = self.players[i].pos.x.clamp(8.0, MAP_W as f32 * TILE_SIZE - 8.0);
            self.players[i].pos.y = self.players[i].pos.y.clamp(8.0, MAP_H as f32 * TILE_SIZE - 8.0);

            if !self.is_pos_on_ground(self.players[i].pos) {
                self.kill_player(i, "fell into the void");
                continue;
            }

            let p_pos = self.players[i].pos;
            self.pickups.retain(|pk| {
                if pk.pos.distance(p_pos) < 22.0 {
                    match pk.kind {
                        ResourceKind::Iron => self.players[i].iron += pk.amount,
                        ResourceKind::Gold => self.players[i].gold += pk.amount,
                        ResourceKind::Emerald => self.players[i].emerald += pk.amount,
                    }
                    false
                } else {
                    true
                }
            });
        }

        let mut detonations: Vec<(Vec2, f32, Team)> = Vec::new();
        self.tnts.retain_mut(|t| {
            t.fuse -= dt;
            t.pos += t.vel * dt;
            t.vel *= 0.90;
            if t.fuse <= 0.0 {
                detonations.push((t.pos, 2.7, t.owner_team));
                false
            } else {
                true
            }
        });
        for (pos, rad, team) in detonations {
            self.trigger_explosion(pos, rad, team);
        }

        let mut proj_hits: Vec<(usize, Option<usize>, f32, Vec2, &'static str)> = Vec::new();
        let mut fireball_explosions: Vec<(Vec2, Team)> = Vec::new();
        let mut deferred_particles: Vec<(Vec2, Color, usize, f32)> = Vec::new();
        let mut projectiles = std::mem::take(&mut self.projectiles);
        let mut remaining_projectiles = Vec::with_capacity(projectiles.len());

        for mut pr in projectiles.drain(..) {
            pr.life -= dt;
            if pr.life <= 0.0 {
                continue;
            }
            pr.pos += pr.vel * dt;
            let (tx, ty) = Self::world_to_tile(pr.pos);
            let hit_block = matches!(self.tile_at(tx, ty), Tile::PlacedWool(_) | Tile::PlacedEndstone(_));

            if hit_block {
                if pr.kind == ProjectileKind::Fireball {
                    fireball_explosions.push((pr.pos, pr.team));
                } else {
                    deferred_particles.push((pr.pos, LIGHTGRAY, 5, 70.0));
                }
                continue;
            }

            let mut hit_player = false;
            let attacker_idx = self.players.iter().position(|p| p.id == pr.owner_id);
            for (i, pl) in self.players.iter().enumerate() {
                if pl.alive && pl.id != pr.owner_id && pl.team != pr.team && pl.pos.distance(pr.pos) < 16.0 {
                    if pr.kind == ProjectileKind::Fireball {
                        fireball_explosions.push((pr.pos, pr.team));
                    } else {
                        let kb = pr.vel.normalize_or_zero() * pr.knockback;
                        proj_hits.push((i, attacker_idx, pr.damage, kb, "was sniped by a bow"));
                    }
                    hit_player = true;
                    break;
                }
            }
            if !hit_player {
                remaining_projectiles.push(pr);
            }
        }
        self.projectiles = remaining_projectiles;

        for (p_pos, p_col, p_cnt, p_spd) in deferred_particles {
            self.spawn_particles(p_pos, p_col, p_cnt, p_spd);
        }
        for (f_pos, f_team) in fireball_explosions {
            self.trigger_explosion(f_pos, 2.2, f_team);
        }
        for (v_idx, a_idx, dmg, kb, reason) in proj_hits {
            self.apply_damage(v_idx, a_idx, dmg, kb, reason);
        }

        self.particles.retain_mut(|pt| {
            pt.life -= dt;
            pt.pos += pt.vel * dt;
            pt.vel *= 0.92;
            pt.life > 0.0
        });

        let mut active_teams = Vec::new();
        for team in Team::all() {
            let has_bed = self.bed_alive.get(&team).copied().unwrap_or(false);
            let has_alive_player = self.players.iter().any(|p| p.team == team && (p.alive || p.respawn_timer > 0.0));
            if has_bed || has_alive_player {
                active_teams.push(team);
            }
        }
        if active_teams.len() == 1 {
            self.winner = Some(active_teams[0]);
        } else if active_teams.is_empty() {
            self.winner = Some(Team::Red);
        }
    }

    fn resource_color(kind: ResourceKind) -> Color {
        match kind {
            ResourceKind::Iron => Color::new(0.78, 0.82, 0.88, 1.0),
            ResourceKind::Gold => Color::new(1.0, 0.82, 0.15, 1.0),
            ResourceKind::Emerald => Color::new(0.15, 0.92, 0.52, 1.0),
        }
    }

    fn draw_world(&self) {
        clear_background(Color::new(0.05, 0.06, 0.11, 1.0));

        for y in 0..MAP_H {
            for x in 0..MAP_W {
                let wx = x as f32 * TILE_SIZE;
                let wy = y as f32 * TILE_SIZE;
                let tile = self.grid[y][x];
                match tile {
                    Tile::Void => {
                        draw_rectangle_lines(wx, wy, TILE_SIZE, TILE_SIZE, 1.0, Color::new(0.10, 0.12, 0.20, 0.28));
                    }
                    Tile::BaseFloor(team) => {
                        draw_rectangle(wx, wy, TILE_SIZE, TILE_SIZE, Color::new(0.24, 0.26, 0.32, 1.0));
                        draw_rectangle(wx + 2.0, wy + 2.0, TILE_SIZE - 4.0, TILE_SIZE - 4.0, Color::new(0.28, 0.31, 0.38, 1.0));
                        draw_rectangle_lines(wx, wy, TILE_SIZE, TILE_SIZE, 1.0, team.color().with_alpha(0.35));
                    }
                    Tile::MidFloor => {
                        draw_rectangle(wx, wy, TILE_SIZE, TILE_SIZE, Color::new(0.22, 0.25, 0.33, 1.0));
                        draw_rectangle(wx + 2.0, wy + 2.0, TILE_SIZE - 4.0, TILE_SIZE - 4.0, Color::new(0.27, 0.31, 0.40, 1.0));
                        draw_rectangle_lines(wx, wy, TILE_SIZE, TILE_SIZE, 1.0, GOLD.with_alpha(0.22));
                    }
                    Tile::PlacedWool(team) => {
                        let base_c = team.color();
                        draw_rectangle(wx + 1.0, wy + 1.0, TILE_SIZE - 2.0, TILE_SIZE - 2.0, base_c.with_alpha(0.88));
                        draw_rectangle_lines(wx + 1.0, wy + 1.0, TILE_SIZE - 2.0, TILE_SIZE - 2.0, 2.0, WHITE.with_alpha(0.45));
                    }
                    Tile::PlacedEndstone(team) => {
                        let end_col = Color::new(0.82, 0.78, 0.56, 0.95);
                        draw_rectangle(wx + 1.0, wy + 1.0, TILE_SIZE - 2.0, TILE_SIZE - 2.0, end_col);
                        draw_rectangle_lines(wx + 1.0, wy + 1.0, TILE_SIZE - 2.0, TILE_SIZE - 2.0, 2.0, team.color());
                    }
                    Tile::Bed(team) => {
                        draw_rectangle(wx, wy, TILE_SIZE, TILE_SIZE, Color::new(0.24, 0.26, 0.32, 1.0));
                        draw_rectangle(wx + 4.0, wy + 6.0, TILE_SIZE - 8.0, TILE_SIZE - 12.0, team.color());
                        draw_rectangle(wx + 6.0, wy + 8.0, 9.0, TILE_SIZE - 16.0, WHITE);
                        draw_rectangle_lines(wx + 4.0, wy + 6.0, TILE_SIZE - 8.0, TILE_SIZE - 12.0, 2.0, GOLD);
                    }
                }
            }
        }

        for g in &self.generators {
            let pulse = ((self.elapsed * 3.0).sin() * 0.5 + 0.5) * 4.0;
            let col = Self::resource_color(g.kind);
            draw_circle(g.pos.x, g.pos.y, 12.0 + pulse, col.with_alpha(0.22));
            draw_circle(g.pos.x, g.pos.y, 8.0, col);
            draw_circle_lines(g.pos.x, g.pos.y, 8.0, 2.0, BLACK);
        }

        for pk in &self.pickups {
            let bob_y = (self.elapsed * 4.0 + pk.bob).sin() * 3.0;
            let col = Self::resource_color(pk.kind);
            draw_rectangle(pk.pos.x - 6.0, pk.pos.y - 6.0 + bob_y, 12.0, 12.0, col);
            draw_rectangle_lines(pk.pos.x - 6.0, pk.pos.y - 6.0 + bob_y, 12.0, 12.0, 1.5, BLACK);
            if pk.amount > 1 {
                draw_text(&format!("x{}", pk.amount), pk.pos.x + 8.0, pk.pos.y + 4.0, 14.0, WHITE);
            }
        }

        for t in &self.tnts {
            let blink = ((self.elapsed * 18.0).sin() * 0.5 + 0.5) > 0.5;
            let col = if blink { RED } else { WHITE };
            draw_circle(t.pos.x, t.pos.y, 2.7 * TILE_SIZE, RED.with_alpha(0.14));
            draw_rectangle(t.pos.x - 8.0, t.pos.y - 8.0, 16.0, 16.0, col);
            draw_rectangle_lines(t.pos.x - 8.0, t.pos.y - 8.0, 16.0, 16.0, 2.0, BLACK);
        }

        for pr in &self.projectiles {
            match pr.kind {
                ProjectileKind::Arrow => {
                    let end = pr.pos - pr.vel.normalize_or_zero() * 14.0;
                    draw_line(pr.pos.x, pr.pos.y, end.x, end.y, 3.0, YELLOW);
                }
                ProjectileKind::Fireball => {
                    draw_circle(pr.pos.x, pr.pos.y, 9.0, ORANGE);
                    draw_circle(pr.pos.x, pr.pos.y, 5.0, YELLOW);
                }
            }
        }

        for p in &self.players {
            if !p.alive {
                continue;
            }
            let body_col = if p.hurt_flash > 0.0 {
                WHITE
            } else {
                p.team.color()
            };

            let tip = p.pos + p.aim_dir * 20.0;
            draw_line(p.pos.x, p.pos.y, tip.x, tip.y, 3.0, WHITE.with_alpha(0.8));

            draw_circle(p.pos.x, p.pos.y, 12.0, body_col);
            draw_circle_lines(p.pos.x, p.pos.y, 12.0, 2.0, BLACK);

            let bw = 28.0;
            let bh = 5.0;
            let bx = p.pos.x - bw * 0.5;
            let by = p.pos.y - 22.0;
            draw_rectangle(bx, by, bw, bh, Color::new(0.1, 0.1, 0.1, 0.85));
            let hp_ratio = (p.hp / p.max_hp).clamp(0.0, 1.0);
            draw_rectangle(bx, by, bw * hp_ratio, bh, Color::new(0.2, 0.9, 0.35, 1.0));

            let tag = if p.is_human {
                p.name.clone()
            } else {
                let goal_short = match p.ai_goal {
                    AiGoal::DefendBed => "DEF",
                    AiGoal::BreakBed(_) => "BED",
                    AiGoal::EngageEnemy(_) => "ATK",
                    AiGoal::GatherResources(_) => "GEN",
                    AiGoal::RetreatHeal => "HEAL",
                };
                format!("{} [{}]", p.name, goal_short)
            };
            draw_text(&tag, p.pos.x - 24.0, p.pos.y - 26.0, 13.0, WHITE);
        }

        for pt in &self.particles {
            let alpha = (pt.life / pt.max_life).clamp(0.0, 1.0);
            draw_rectangle(
                pt.pos.x - pt.size * 0.5,
                pt.pos.y - pt.size * 0.5,
                pt.size,
                pt.size,
                pt.color.with_alpha(alpha),
            );
        }
    }

    fn draw_minimap(&self) {
        let scale = 3.6;
        let mw = MAP_W as f32 * scale;
        let mh = MAP_H as f32 * scale;
        let mx = screen_width() - mw - 12.0;
        let my = screen_height() - mh - 12.0;

        draw_rectangle(mx - 2.0, my - 2.0, mw + 4.0, mh + 4.0, Color::new(0.05, 0.06, 0.10, 0.90));
        draw_rectangle_lines(mx - 2.0, my - 2.0, mw + 4.0, mh + 4.0, 1.5, GRAY);

        for y in 0..MAP_H {
            for x in 0..MAP_W {
                let rx = mx + x as f32 * scale;
                let ry = my + y as f32 * scale;
                match self.grid[y][x] {
                    Tile::Void => {}
                    Tile::BaseFloor(t) => draw_rectangle(rx, ry, scale, scale, t.color().with_alpha(0.45)),
                    Tile::MidFloor => draw_rectangle(rx, ry, scale, scale, Color::new(0.35, 0.38, 0.48, 0.75)),
                    Tile::PlacedWool(t) => draw_rectangle(rx, ry, scale, scale, t.color().with_alpha(0.85)),
                    Tile::PlacedEndstone(_) => draw_rectangle(rx, ry, scale, scale, Color::new(0.85, 0.80, 0.55, 0.9)),
                    Tile::Bed(_t) => draw_rectangle(rx - 1.0, ry - 1.0, scale + 2.0, scale + 2.0, WHITE),
                }
            }
        }

        for p in &self.players {
            if p.alive {
                let px = mx + (p.pos.x / TILE_SIZE) * scale;
                let py = my + (p.pos.y / TILE_SIZE) * scale;
                draw_circle(px, py, 3.2, p.team.color());
            }
        }
    }

    fn draw_hud(&self) {
        let human = &self.players[0];

        draw_rectangle(12.0, 12.0, 355.0, 136.0, Color::new(0.08, 0.10, 0.16, 0.88));
        draw_rectangle_lines(12.0, 12.0, 355.0, 136.0, 1.5, GRAY);

        draw_text(
            &format!(
                "{} ({}) HP:{:.0}/{:.0}  K:{} B:{}",
                human.name,
                human.team.name(),
                human.hp,
                human.max_hp,
                human.kills,
                human.beds_broken
            ),
            22.0,
            34.0,
            17.0,
            human.team.color(),
        );
        draw_text(
            &format!(
                "Iron:{}  Gold:{}  Em:{}  Wool:{}  End:{}",
                human.iron, human.gold, human.emerald, human.wool, human.endstone
            ),
            22.0,
            58.0,
            16.0,
            WHITE,
        );
        draw_text(
            &format!(
                "Wpn:{}  TNT:{}  FB:{}  Arr:{}  KB:{}",
                human.weapon.label(),
                human.tnt,
                human.fireballs,
                human.arrows,
                if human.has_knockback_stick { "Y" } else { "N" }
            ),
            22.0,
            82.0,
            15.0,
            LIGHTGRAY,
        );
        draw_text(
            "WASD | LMB Atk/Brk | RMB Pl (+Shift End) | Q Bow | F TNT | G FB",
            22.0,
            106.0,
            12.5,
            SKYBLUE,
        );
        draw_text(
            "B Shop | U Team Upgrades | P Pause",
            22.0,
            128.0,
            13.0,
            GOLD,
        );

        let board_x = screen_width() - 245.0;
        draw_rectangle(board_x, 12.0, 233.0, 132.0, Color::new(0.08, 0.10, 0.16, 0.88));
        draw_rectangle_lines(board_x, 12.0, 233.0, 132.0, 1.5, GRAY);
        draw_text("TEAM BEDS", board_x + 12.0, 32.0, 17.0, GOLD);

        for (idx, team) in Team::all().iter().enumerate() {
            let y = 54.0 + idx as f32 * 20.0;
            let bed_ok = self.bed_alive.get(team).copied().unwrap_or(false);
            let alive_cnt = self.players.iter().filter(|p| p.team == *team && p.alive).count();
            let status_txt = if bed_ok {
                format!("BED OK ({} alive)", alive_cnt)
            } else if alive_cnt > 0 {
                format!("NO BED ({} alive)", alive_cnt)
            } else {
                "ELIMINATED".to_string()
            };
            draw_text(
                &format!("{}: {}", team.name(), status_txt),
                board_x + 12.0,
                y,
                15.0,
                team.color(),
            );
        }

        if self.status_timer > 0.0 {
            let bw = 560.0;
            let bx = (screen_width() - bw) * 0.5;
            draw_rectangle(bx, 16.0, bw, 34.0, Color::new(0.05, 0.06, 0.10, 0.92));
            draw_rectangle_lines(bx, 16.0, bw, 34.0, 1.5, GOLD);
            draw_text(&self.status_banner, bx + 16.0, 38.0, 16.0, WHITE);
        }

        if self.shop_open {
            let sw = 480.0;
            let sh = 385.0;
            let sx = (screen_width() - sw) * 0.5;
            let sy = (screen_height() - sh) * 0.5;
            draw_rectangle(sx, sy, sw, sh, Color::new(0.07, 0.09, 0.15, 0.96));
            draw_rectangle_lines(sx, sy, sw, sh, 2.0, GOLD);
            draw_text("ITEM SHOP (Click or press 1-9, 0)", sx + 18.0, sy + 28.0, 18.0, GOLD);

            let items = [
                ("[1] 16 Wool Blocks", "4 Iron"),
                ("[2] Stone Sword", "10 Iron"),
                ("[3] Iron Sword", "7 Gold"),
                ("[4] Knockback Stick", "5 Gold"),
                ("[5] Bow + 6 Arrows", "6 Gold"),
                ("[6] +20 Max HP & Heal", "8 Iron"),
                ("[7] TNT (Blast)", "8 Gold"),
                ("[8] Fireball", "6 Gold"),
                ("[9] 8 Endstone Blocks", "12 Iron"),
                ("[0] Diamond Sword", "4 Em"),
            ];

            let mouse = mouse_position();
            for row in 0..10 {
                let (label, cost) = items[row];
                let iy = sy + 52.0 + row as f32 * 31.0;
                let row_rect = Rect::new(sx + 16.0, iy - 15.0, sw - 32.0, 25.0);
                let hovered = row_rect.contains(vec2(mouse.0, mouse.1));

                draw_rectangle(
                    row_rect.x,
                    row_rect.y,
                    row_rect.w,
                    row_rect.h,
                    if hovered { Color::new(0.22, 0.28, 0.42, 0.92) } else { Color::new(0.14, 0.17, 0.26, 0.82) },
                );
                draw_text(label, row_rect.x + 10.0, iy + 2.0, 15.0, WHITE);
                draw_text(cost, row_rect.x + row_rect.w - 95.0, iy + 2.0, 15.0, YELLOW);
            }
        }

        if self.upgrades_open {
            let upg = self.team_upgrades.get(&human.team).copied().unwrap_or_default();
            let uw = 460.0;
            let uh = 215.0;
            let ux = (screen_width() - uw) * 0.5;
            let uy = (screen_height() - uh) * 0.5;
            draw_rectangle(ux, uy, uw, uh, Color::new(0.07, 0.10, 0.16, 0.96));
            draw_rectangle_lines(ux, uy, uw, uh, 2.0, SKYBLUE);
            draw_text("TEAM UPGRADES (Click row to buy, U to close)", ux + 18.0, uy + 28.0, 17.0, SKYBLUE);

            let rows = [
                (format!("Sharpness (Lv {}/3)", upg.sharpness), "4 Gold"),
                (format!("Protection (Lv {}/3)", upg.protection), "4 Gold"),
                (format!("Forge Speed (Lv {}/3)", upg.forge_level), "2 Em"),
                (format!("Base Heal Pool ({})", if upg.heal_pool { "Active" } else { "Off" }), "3 Em"),
            ];

            let mouse = mouse_position();
            for (i, (label, cost)) in rows.iter().enumerate() {
                let ry_pos = uy + 54.0 + i as f32 * 36.0;
                let rect = Rect::new(ux + 16.0, ry_pos - 16.0, uw - 32.0, 28.0);
                let hovered = rect.contains(vec2(mouse.0, mouse.1));
                draw_rectangle(
                    rect.x,
                    rect.y,
                    rect.w,
                    rect.h,
                    if hovered { Color::new(0.20, 0.30, 0.45, 0.92) } else { Color::new(0.13, 0.17, 0.25, 0.85) },
                );
                draw_text(label, rect.x + 10.0, ry_pos + 3.0, 15.0, WHITE);
                draw_text(cost, rect.x + rect.w - 90.0, ry_pos + 3.0, 15.0, YELLOW);
            }
        }

        if self.paused && self.winner.is_none() {
            let pw = 340.0;
            let ph = 90.0;
            let px = (screen_width() - pw) * 0.5;
            let py = (screen_height() - ph) * 0.5;
            draw_rectangle(px, py, pw, ph, Color::new(0.05, 0.06, 0.10, 0.94));
            draw_rectangle_lines(px, py, pw, ph, 2.0, GOLD);
            draw_text("PAUSED — Press P or Esc to resume", px + 26.0, py + 50.0, 18.0, WHITE);
        }

        if let Some(win_team) = self.winner {
            let gw = 440.0;
            let gh = 120.0;
            let gx = (screen_width() - gw) * 0.5;
            let gy = (screen_height() - gh) * 0.5;
            draw_rectangle(gx, gy, gw, gh, Color::new(0.05, 0.06, 0.10, 0.96));
            draw_rectangle_lines(gx, gy, gw, gh, 2.5, win_team.color());
            draw_text(
                &format!("{} TEAM WINS!", win_team.name()),
                gx + 105.0,
                gy + 48.0,
                28.0,
                win_team.color(),
            );
            draw_text("Press R to play again", gx + 135.0, gy + 88.0, 18.0, WHITE);
        }

        self.draw_minimap();
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Rust Macroquad Bedwars".to_owned(),
        window_width: (MAP_W as f32 * TILE_SIZE) as i32,
        window_height: (MAP_H as f32 * TILE_SIZE) as i32,
        window_resizable: false,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut game = GameState::new();

    loop {
        let dt = get_frame_time().min(0.05);

        if game.shop_open && is_mouse_button_pressed(MouseButton::Left) {
            let sw = 480.0;
            let sh = 385.0;
            let sx = (screen_width() - sw) * 0.5;
            let sy = (screen_height() - sh) * 0.5;
            let mouse = mouse_position();
            for row in 0..10u8 {
                let iy = sy + 52.0 + row as f32 * 31.0;
                let row_rect = Rect::new(sx + 16.0, iy - 15.0, sw - 32.0, 25.0);
                if row_rect.contains(vec2(mouse.0, mouse.1)) {
                    let item_id = if row == 9 { 10 } else { row + 1 };
                    if game.buy_item(0, item_id) {
                        game.set_banner(format!("Purchased item #{}!", item_id), 1.2);
                    }
                }
            }
        }

        if game.upgrades_open && is_mouse_button_pressed(MouseButton::Left) {
            let uw = 460.0;
            let uh = 215.0;
            let ux = (screen_width() - uw) * 0.5;
            let uy = (screen_height() - uh) * 0.5;
            let mouse = mouse_position();
            for i in 0..4u8 {
                let ry_pos = uy + 54.0 + i as f32 * 36.0;
                let rect = Rect::new(ux + 16.0, ry_pos - 16.0, uw - 32.0, 28.0);
                if rect.contains(vec2(mouse.0, mouse.1)) {
                    if game.buy_team_upgrade(0, i + 1) {
                        game.set_banner(format!("Upgraded team perk #{}!", i + 1), 1.4);
                    }
                }
            }
        }

        game.update(dt);
        game.draw_world();
        game.draw_hud();

        next_frame().await
    }
}