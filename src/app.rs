// Game loop, menu flow (solo/multi), and the in-game client.
// v0.6 "Multijoueur" - the UI is drawn through ui::Painter + renderer.
use crate::client::{Client, NetEvent, Phase};
use crate::gl;
use crate::math::{forward, ortho_pixels, Vec3};
use crate::mesher;
use crate::mobs::{self, Particle};
use crate::noise;
use crate::pack::Pack;
use crate::player::{Input, Player};
use crate::raycast;
use crate::renderer::{ChunkMesh, Renderer};
use crate::server::{self, ServerConfig};
use crate::ui::{self, Painter};
use crate::win32;
use crate::world::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

const SAVE_NAME: &str = "rustvoxel_world.sav";
const OPTIONS_NAME: &str = "options.txt";
const SERVERS_NAME: &str = "servers.txt";

fn offsets(radius: i32) -> Vec<(i32, i32)> {
    let mut v = Vec::new();
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            v.push((dx, dz));
        }
    }
    v.sort_by_key(|&(dx, dz)| dx * dx + dz * dz);
    v
}

fn neighbors8(cx: i32, cz: i32) -> [(i32, i32); 8] {
    [
        (cx - 1, cz),
        (cx + 1, cz),
        (cx, cz - 1),
        (cx, cz + 1),
        (cx - 1, cz - 1),
        (cx + 1, cz - 1),
        (cx - 1, cz + 1),
        (cx + 1, cz + 1),
    ]
}

/// Mark chunks whose mesh is affected by an edit at (x, y, z).
fn mark_edited(dirty: &mut HashSet<(i32, i32)>, x: i32, z: i32) {
    let cx = x.div_euclid(CX as i32);
    let cz = z.div_euclid(CZ as i32);
    let lx = x.rem_euclid(CX as i32);
    let lz = z.rem_euclid(CZ as i32);
    dirty.insert((cx, cz));
    for dcx in -1..=1i32 {
        for dcz in -1..=1i32 {
            if dcx == -1 && lx != 0 {
                continue;
            }
            if dcx == 1 && lx != CX as i32 - 1 {
                continue;
            }
            if dcz == -1 && lz != 0 {
                continue;
            }
            if dcz == 1 && lz != CZ as i32 - 1 {
                continue;
            }
            if dcx != 0 || dcz != 0 {
                dirty.insert((cx + dcx, cz + dcz));
            }
        }
    }
}

// ------------------------------------------------------------------ options
#[derive(Clone)]
pub struct Options {
    pub render_dist: i32,
    pub gui_scale: i32, // 0 = auto
    pub sensitivity: f32,
    pub fov: f32,
    pub fog: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            render_dist: 6,
            gui_scale: 0,
            sensitivity: 1.0,
            fov: 75.0,
            fog: true,
        }
    }
}

impl Options {
    fn load(base: &Path) -> Options {
        let mut o = Options::default();
        if let Ok(s) = std::fs::read_to_string(base.join(OPTIONS_NAME)) {
            for line in s.lines() {
                let mut it = line.split('=');
                let (k, v) = match (it.next(), it.next()) {
                    (Some(k), Some(v)) => (k.trim(), v.trim()),
                    _ => continue,
                };
                match k {
                    "render_dist" => o.render_dist = v.parse().unwrap_or(o.render_dist),
                    "gui_scale" => o.gui_scale = v.parse().unwrap_or(o.gui_scale),
                    "sensitivity" => o.sensitivity = v.parse().unwrap_or(o.sensitivity),
                    "fov" => o.fov = v.parse().unwrap_or(o.fov),
                    "fog" => o.fog = v.parse().unwrap_or(o.fog),
                    _ => {}
                }
            }
        }
        o
    }
    fn save(&self, base: &Path) {
        let s = format!(
            "render_dist={}\ngui_scale={}\nsensitivity={:.2}\nfov={:.0}\nfog={}\n",
            self.render_dist, self.gui_scale, self.sensitivity, self.fov, self.fog
        );
        let _ = std::fs::write(base.join(OPTIONS_NAME), s);
    }
    pub fn effective_scale(&self, win_h: i32) -> f32 {
        if self.gui_scale > 0 {
            return self.gui_scale as f32;
        }
        ((win_h / 260).clamp(1, 3)) as f32
    }
}

fn load_servers(base: &Path) -> Vec<(String, String)> {
    let mut v = Vec::new();
    if let Ok(s) = std::fs::read_to_string(base.join(SERVERS_NAME)) {
        for line in s.lines() {
            let mut it = line.splitn(2, '|');
            if let (Some(n), Some(a)) = (it.next(), it.next()) {
                v.push((n.to_string(), a.to_string()));
            }
        }
    }
    v
}

fn save_servers(base: &Path, v: &[(String, String)]) {
    let s = v.iter().map(|(n, a)| format!("{n}|{a}")).collect::<Vec<_>>().join("\n");
    let _ = std::fs::write(base.join(SERVERS_NAME), s);
}

const SPLASHES: [&str; 14] = [
    "100% logiciel !",
    "Aussi en Rust !",
    "Sans aucune dépendance !",
    "Zéro bloc Copyright !",
    "Multijoueur intégré !",
    "Essayez /time nuit !",
    "Gare aux Siffleurs !",
    "Grottes luxuriantes !",
    "Fait main, comme 2009 !",
    "Le vide est un choix !",
    "OpenGL 2.0 powered !",
    "128 blocs de haut !",
    "Non affilié à Mojang !",
    "Creusez profond !",
];

// ------------------------------------------------------------------ screens
#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Menu,
    SoloMenu,
    MultiMenu,
    Direct,
    AddServer,
    OptionsMenu,
    Connecting,
    Game,
}

// -------------------------------------------------------------------- game
struct Game {
    cli: Client,
    player: Player,
    meshes: HashMap<(i32, i32), ChunkMesh>,
    dirty: HashSet<(i32, i32)>,
    offs: Vec<(i32, i32)>,
    hotbar: [u16; 9],
    slot: usize,
    mine_target: Option<(i32, i32, i32)>,
    mine_progress: f32,
    swing: f32, // <0 idle
    inv_open: bool,
    inv_page: usize,
    dead_screen_t: f32,
    paused: bool,
    particles: Vec<Particle>,
    last_place: f32,
    attack_cd: f32,
    item_name_t: f32,
    chat_input: Option<String>,
    tab_on: bool,
    t_total: f32,
    day_t: f32,
    label: String, // server label (F3 / pause)
    solo: bool,
}

impl Game {
    fn new(
        addr: &str,
        name: &str,
        solo: bool,
        label: String,
        render_dist: i32,
    ) -> Result<Game, String> {
        let cli = Client::connect(addr, name)
            .map_err(|e| format!("connexion impossible: {e}"))?;
        Ok(Game {
            cli,
            player: Player::new(),
            meshes: HashMap::new(),
            dirty: HashSet::new(),
            offs: offsets(render_dist + 1),
            hotbar: [
                GRASS, TORCH, CHERRY_PLANKS, DEEPSLATE, COPPER_BLOCK, OAK_FENCE,
                SNOW_LAYER, WOOL_STAIRS_BASE, MEAT,
            ],
            slot: 0,
            mine_target: None,
            mine_progress: 0.0,
            swing: -1.0,
            inv_open: false,
            inv_page: 0,
            dead_screen_t: 0.0,
            paused: false,
            particles: Vec::new(),
            last_place: -1.0,
            attack_cd: 0.0,
            item_name_t: 0.0,
            chat_input: None,
            tab_on: false,
            t_total: 0.0,
            day_t: 0.06,
            label,
            solo,
        })
    }

    fn mark(&mut self, x: i32, z: i32) {
        mark_edited(&mut self.dirty, x, z);
    }

    fn push_chat_line(&mut self, s: String) {
        self.cli.chat.push_back((std::time::Instant::now(), s));
        if self.cli.chat.len() > 100 {
            self.cli.chat.pop_front();
        }
    }
}

// -------------------------------------------------------------------- app
pub struct App {
    screen: Screen,
    base: PathBuf,
    opts: Options,
    splash: &'static str,
    servers: Vec<(String, String)>,
    sel_server: usize,
    in_name: String,
    in_addr: String,
    focus: u8, // 0 name, 1 addr
    conn_err: Option<String>,
    game: Option<Game>,
    f3_on: bool,
    prev_keys: [bool; 32],
    prev_lmb: bool,
    mouse: (f32, f32),
    fps: u32,
    fps_n: u32,
    fps_t: f32,
    // options slider drag state
    drag: Option<u8>,
    /// Nombre de PNG chargés depuis texturepacks/ (affichage menu/options).
    pack_textures: usize,
}

pub fn run() -> Result<(), String> {
    let win = win32::init("RustVoxel", 1280, 720)?;
    gl::init()?;
    let base = win32::exe_dir();
    let mut pack = Pack::discover(&base);
    crate::models::install(&pack);
    let mut palette = BiomePalette::from_pack(&pack);
    let mut renderer = Renderer::new(Some(&pack))?;
    let mut ent_atlas = crate::entity_models::EntityAtlas::from_pack(&pack);
    if let Some(a) = &ent_atlas {
        renderer.upload_entity_atlas(a);
    }
    win32::swap(win.hdc);

    let opts = Options::load(&base);
    let splash = SPLASHES
        [(noise::hash2i(0, std::process::id() as i64, 0x5B7) % SPLASHES.len() as u64) as usize];

    let mut app = App {
        screen: Screen::Menu,
        base: base.clone(),
        opts,
        splash,
        servers: load_servers(&base),
        sel_server: 0,
        in_name: "Joueur".into(),
        in_addr: "127.0.0.1:25565".into(),
        focus: 1,
        conn_err: None,
        game: None,
        f3_on: false,
        prev_keys: [false; 32],
        prev_lmb: false,
        mouse: (0.0, 0.0),
        fps: 0,
        fps_n: 0,
        fps_t: 0.0,
        drag: None,
        pack_textures: pack.loaded_names().len(),
    };

    let mut painter = Painter::new();
    let mut qpc_prev = win32::qpc_counter();
    win32::set_captured(false);

    loop {
        if !win32::pump() {
            break;
        }
        let now = win32::qpc_counter();
        let dt = win32::qpc_dt(qpc_prev, now).min(0.05);
        qpc_prev = now;
        app.fps_n += 1;
        app.fps_t += dt;
        if app.fps_t >= 0.5 {
            app.fps = (app.fps_n as f32 / app.fps_t) as u32;
            app.fps_n = 0;
            app.fps_t = 0.0;
        }

        let chars = win32::take_chars();
        let wheel = win32::take_wheel();
        app.mouse = win32::cursor_pos();
        let (cw, ch) = win32::client_size();
        let scale = app.opts.effective_scale(ch);
        let mouse = (app.mouse.0 / scale, app.mouse.1 / scale);
        let lmb = win32::key_down(0x01);
        let clicked = lmb && !app.prev_lmb;
        app.prev_lmb = lmb;

        let keys = key_array();
        let prev_keys = app.prev_keys;
        let edge = |i: usize| keys[i] && !prev_keys[i];

        // F4 (index 6): reload texture packs, works everywhere
        if edge(6) {
            pack = Pack::discover(&base);
            crate::models::install(&pack);
            palette = BiomePalette::from_pack(&pack);
            renderer.reload_atlas(Some(&pack));
            ent_atlas = crate::entity_models::EntityAtlas::from_pack(&pack);
            if let Some(a) = &ent_atlas {
                renderer.upload_entity_atlas(a);
            }
            app.pack_textures = pack.loaded_names().len();
        }

        painter.clear();
        let gw = cw as f32 / scale;
        let gh = ch as f32 / scale;
        match app.screen {
            Screen::Menu => app.frame_menu(&mut painter, gw, gh, mouse, clicked),
            Screen::SoloMenu => app.frame_solo(&mut painter, gw, gh, mouse, clicked),
            Screen::MultiMenu => app.frame_multi(&mut painter, gw, gh, mouse, clicked),
            Screen::Direct => app.frame_direct(&mut painter, gw, gh, mouse, clicked, &chars),
            Screen::AddServer => app.frame_add(&mut painter, gw, gh, mouse, clicked, &chars),
            Screen::OptionsMenu => {
                app.frame_options(&mut painter, gw, gh, mouse, clicked);
            }
            Screen::Connecting => app.frame_connecting(&mut painter, gw, gh, mouse, clicked),
            Screen::Game => app.frame_game(
                &mut painter,
                &keys,
                &edge,
                mouse,
                clicked,
                wheel,
                &chars,
                &mut renderer,
                &mut ent_atlas,
                &mut palette,
                &mut pack,
                dt,
                (cw, ch),
                scale,
                lmb,
            ),
        }
        app.prev_keys = keys;

        // execute the painter in GUI coordinates
        let gw = cw as f32 / scale;
        let gh = ch as f32 / scale;
        let ortho = ortho_pixels(gw, gh);
        renderer.exec_painter(&painter, &ortho);
        win32::swap(win.hdc);
    }

    // leave cleanly: disconnect (the integrated server saves itself)
    app.game = None;
    Ok(())
}

// 32 polled keys: [ESC,F,E,T,TAB,F3,F4,P,1..9,LMB,RMB,MMB,SHIFT,CTRL,SPACE,ENTER,BACKSLASH]
fn key_array() -> [bool; 32] {
    let vk = [
        0x1B, 0x46, 0x45, 0x54, 0x09, 0x72, 0x73, 0x50, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36,
        0x37, 0x38, 0x39, 0x01, 0x02, 0x04, 0x10, 0x11, 0x20, 0x0D, 0xDC,
    ];
    let mut a = [false; 32];
    for (i, &k) in vk.iter().enumerate() {
        a[i] = win32::key_down(k);
    }
    a
}

// ------------------------------------------------------------------ menus
impl App {
    fn button(&self, p: &mut Painter, label: &str, cx: f32, y: f32, w: f32, mouse: (f32, f32)) -> bool {
        let x = cx - w * 0.5;
        let hov = ui::hover(mouse, x, y, w, 18.0);
        ui::button(p, label, x, y, w, 18.0, hov);
        hov
    }

    fn frame_menu(&mut self, p: &mut Painter, gw: f32, gh: f32, mouse: (f32, f32), clicked: bool) {
        win32::set_captured(false);
        ui::dirt_bg(p, T_DIRT, gw, gh);
        ui::title(p, gw, self.splash, self.t_of());
        let cx = gw * 0.5;
        let hit1 = self.button(p, "Jouer en solo", cx, 96.0, 200.0, mouse);
        let hit2 = self.button(p, "Multijoueur", cx, 118.0, 200.0, mouse);
        let hit3 = self.button(p, "Options...", cx, 140.0, 200.0, mouse);
        let hit4 = self.button(p, "Quitter", cx, 170.0, 200.0, mouse);
        ui::version_lines(p, gw, gh);
        if self.pack_textures == 0 {
            ui::center_text(
                p,
                "Aucun pack: déposez vos .zip vanilla dans texturepacks/ puis F4",
                gw * 0.5,
                gh - 14.0,
                ui::GRAY,
                true,
            );
        }
        if clicked {
            if hit1 {
                self.screen = Screen::SoloMenu;
            } else if hit2 {
                self.screen = Screen::MultiMenu;
            } else if hit3 {
                self.screen = Screen::OptionsMenu;
            } else if hit4 {
                std::process::exit(0);
            }
        }
    }

    fn t_of(&self) -> f64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
            .fract()
            * 100.0
    }

    fn frame_solo(&mut self, p: &mut Painter, gw: f32, gh: f32, mouse: (f32, f32), clicked: bool) {
        win32::set_captured(false);
        ui::dirt_bg(p, T_DIRT, gw, gh);
        ui::center_text(p, "Solo", gw * 0.5, 40.0, ui::WHITE, true);
        let save = self.base.join(SAVE_NAME);
        let cx = gw * 0.5;
        let b1 = save.exists();
        let mut y = 96.0;
        let hit_cont = if b1 {
            let h = self.button(p, "Continuer le monde", cx, y, 200.0, mouse);
            y += 22.0;
            h
        } else {
            false
        };
        let hit_new = self.button(p, "Nouveau monde", cx, y, 200.0, mouse);
        let hit_back = self.button(p, "Retour", cx, y + 50.0, 200.0, mouse);
        if clicked {
            if hit_cont {
                self.start_connecting(true, None);
            } else if hit_new {
                std::fs::remove_file(&save).ok();
                self.start_connecting(true, Some(noise::rand_seed()));
            } else if hit_back {
                self.screen = Screen::Menu;
            }
        }
    }

    fn frame_multi(&mut self, p: &mut Painter, gw: f32, gh: f32, mouse: (f32, f32), clicked: bool) {
        win32::set_captured(false);
        ui::dirt_bg(p, T_DIRT, gw, gh);
        ui::center_text(p, "Multijoueur", gw * 0.5, 24.0, ui::WHITE, true);
        let cx = gw * 0.5;
        let mut y = 60.0;
        let mut joined: Option<(String, String)> = None;
        let n = self.servers.len().min(6);
        for i in 0..n {
            let (name, addr) = self.servers[i].clone();
            let label = format!("{}  [{}]", name, addr);
            let h = self.button(p, &label, cx, y, 340.0, mouse);
            if h && clicked {
                joined = Some((name, addr));
            }
            y += 22.0;
        }
        let hit_add = self.button(p, "Ajouter un serveur", cx, y + 8.0, 200.0, mouse);
        let hit_direct = self.button(p, "Connexion directe...", cx, y + 30.0, 200.0, mouse);
        let hit_back = self.button(p, "Retour", cx, y + 52.0, 200.0, mouse);
        if clicked {
            if let Some((_, addr)) = joined {
                self.in_addr = addr.clone();
                self.start_connecting(false, None);
            } else if hit_add {
                self.in_name.clear();
                self.in_addr = "127.0.0.1:".into();
                self.focus = 0;
                self.screen = Screen::AddServer;
            } else if hit_direct {
                self.screen = Screen::Direct;
            } else if hit_back {
                self.screen = Screen::Menu;
            }
        }
    }

    fn text_field(p: &mut Painter, label: &str, value: &str, cx: f32, y: f32, focused: bool, w: f32) {
        ui::text(p, label, cx - w * 0.5, y - 9.0, ui::GRAY, true);
        let x0 = cx - w * 0.5;
        p.rect(x0, y, x0 + w, y + 16.0, [0.0, 0.0, 0.0, 0.75]);
        p.rect_outline(x0, y, x0 + w, y + 16.0, 1.0, if focused { [1.0, 1.0, 1.0, 0.9] } else { [0.4; 4] });
        let cursor = if focused && (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() / 500 % 2 == 0)
            .unwrap_or(false))
        {
            "_"
        } else {
            " "
        };
        ui::text(p, &format!("{}{}", value, cursor), x0 + 3.0, y + 4.0, ui::WHITE, true);
    }

    fn frame_direct(&mut self, p: &mut Painter, gw: f32, gh: f32, mouse: (f32, f32), clicked: bool, chars: &[char]) {
        win32::set_captured(false);
        ui::dirt_bg(p, T_DIRT, gw, gh);
        ui::center_text(p, "Connexion directe", gw * 0.5, 30.0, ui::WHITE, true);
        let cx = gw * 0.5;
        if clicked {
            let (mx, my) = mouse;
            self.focus = if (gh * 0.11..=gh * 0.17).contains(&my) { 0 } else { self.focus };
            let _ = mx;
        }
        for &c in chars {
            match c {
                '\r' | '\n' => {
                    if !self.in_addr.trim().is_empty() {
                        self.start_connecting(false, None);
                    }
                }
                '\u{8}' => {
                    if self.focus == 0 && !self.in_name.is_empty() {
                        self.in_name.pop();
                    } else if self.focus == 1 && !self.in_addr.is_empty() {
                        self.in_addr.pop();
                    }
                }
                c if !c.is_control() => {
                    if self.focus == 0 && self.in_name.chars().count() < 16 {
                        self.in_name.push(c);
                    } else if self.focus == 1 && self.in_addr.chars().count() < 40 {
                        self.in_addr.push(c);
                    }
                }
                _ => {}
            }
        }
        Self::text_field(p, "Adresse (ip:port)", &self.in_addr, cx, gh * 0.24, self.focus == 1, 240.0);
        let hit_join = self.button(p, "Rejoindre", cx, 80.0, 140.0, mouse);
        let hit_back = self.button(p, "Retour", cx, 104.0, 140.0, mouse);
        if clicked {
            if hit_join {
                self.start_connecting(false, None);
            } else if hit_back {
                self.screen = Screen::MultiMenu;
            }
        }
    }

    fn frame_add(&mut self, p: &mut Painter, gw: f32, gh: f32, mouse: (f32, f32), clicked: bool, chars: &[char]) {
        win32::set_captured(false);
        ui::dirt_bg(p, T_DIRT, gw, gh);
        ui::center_text(p, "Ajouter un serveur", gw * 0.5, 30.0, ui::WHITE, true);
        let cx = gw * 0.5;
        if clicked {
            let (_, my) = mouse;
            self.focus = if (gh * 0.11..=gh * 0.17).contains(&my) { 0 } else { 1 };
        }
        for &c in chars {
            match c {
                '\r' | '\n' => {
                    if !self.in_addr.trim().is_empty() {
                        self.servers.push((self.in_name.clone(), self.in_addr.clone()));
                        save_servers(&self.base, &self.servers);
                        self.screen = Screen::MultiMenu;
                    }
                }
                '\u{8}' => {
                    if self.focus == 0 && !self.in_name.is_empty() {
                        self.in_name.pop();
                    } else if self.focus == 1 && !self.in_addr.is_empty() {
                        self.in_addr.pop();
                    }
                }
                c if !c.is_control() => {
                    if self.focus == 0 && self.in_name.chars().count() < 16 {
                        self.in_name.push(c);
                    } else if self.focus == 1 && self.in_addr.chars().count() < 40 {
                        self.in_addr.push(c);
                    }
                }
                _ => {}
            }
        }
        Self::text_field(p, "Nom", &self.in_name, cx, gh * 0.24, self.focus == 0, 240.0);
        Self::text_field(p, "Adresse (ip:port)", &self.in_addr, cx, gh * 0.4, self.focus == 1, 240.0);
        let hit_add = self.button(p, "Ajouter", cx, 110.0, 140.0, mouse);
        let hit_back = self.button(p, "Retour", cx, 134.0, 140.0, mouse);
        if clicked {
            if hit_add && !self.in_addr.trim().is_empty() {
                self.servers.push((self.in_name.clone(), self.in_addr.clone()));
                save_servers(&self.base, &self.servers);
                self.screen = Screen::MultiMenu;
            } else if hit_back {
                self.screen = Screen::MultiMenu;
            }
        }
    }

    fn slider(p: &mut Painter, label: &str, cx: f32, y: f32, frac: f32, mouse: (f32, f32), w: f32) -> bool {
        let x0 = cx - w * 0.5;
        ui::button(p, label, x0, y, w, 18.0, false);
        // groove + handle
        p.rect(x0 + 6.0, y + 8.0, x0 + w - 6.0, y + 10.0, [0.2, 0.2, 0.22, 1.0]);
        let hx = x0 + 8.0 + frac * (w - 16.0);
        p.rect(hx - 3.0, y + 3.0, hx + 3.0, y + 15.0, [0.8, 0.8, 0.82, 1.0]);
        ui::hover(mouse, x0, y, w, 18.0)
    }

    fn frame_options(&mut self, p: &mut Painter, gw: f32, gh: f32, mouse: (f32, f32), clicked: bool) {
        win32::set_captured(false);
        ui::dirt_bg(p, T_DIRT, gw, gh);
        ui::center_text(p, "Options", gw * 0.5, 24.0, ui::WHITE, true);
        let cx = gw * 0.5;
        let w = 240.0;
        let mut y = 56.0;
        // render distance
        {
            let frac = (self.opts.render_dist - 3) as f32 / 7.0;
            let l = format!("Distance de rendu: {} chunks", self.opts.render_dist);
            let _h = Self::slider(p, &l, cx, y, frac, mouse, w);
            if clicked && ui::hover(mouse, cx - w * 0.5, y, w, 18.0) {
                self.drag = Some(0);
            }
        }
        y += 26.0;
        // sensitivity
        {
            let frac = (self.opts.sensitivity - 0.3) / 1.7;
            let l = format!("Sensibilité: {:.0}%", self.opts.sensitivity * 100.0);
            let _h = Self::slider(p, &l, cx, y, frac.clamp(0.0, 1.0), mouse, w);
            if clicked && ui::hover(mouse, cx - w * 0.5, y, w, 18.0) {
                self.drag = Some(1);
            }
        }
        y += 26.0;
        // fov
        {
            let frac = (self.opts.fov - 60.0) / 50.0;
            let l = format!("Champ de vision: {:.0}°", self.opts.fov);
            let _h = Self::slider(p, &l, cx, y, frac, mouse, w);
            if clicked && ui::hover(mouse, cx - w * 0.5, y, w, 18.0) {
                self.drag = Some(2);
            }
        }
        y += 26.0;
        let hit_scale = self.button(p, &format!("Échelle d'interface: {}", match self.opts.gui_scale {
            0 => "Auto".to_string(),
            n => n.to_string(),
        }), cx, y, w, mouse);
        y += 22.0;
        let hit_fog = self.button(p, &format!("Brouillard: {}", if self.opts.fog { "activé" } else { "désactivé" }), cx, y, w, mouse);
        y += 30.0;
        let pack_note = format!(
            "Packs: dossier texturepacks/ (F4 recharge) - {} texture(s)",
            self.pack_textures
        );
        ui::center_text(p, pack_note.trim_end(), cx, y, ui::GRAY, true);
        y += 16.0;
        let hit_back = self.button(p, "Terminé", cx, y + 4.0, 160.0, mouse);

        // live dragging
        if self.drag.is_some() && !win32::key_down(0x01) {
            self.drag = None;
        }
        if let Some(d) = self.drag {
            let (mx, _) = mouse;
            let x0 = cx - w * 0.5;
            let frac = ((mx - x0 - 8.0) / (w - 16.0)).clamp(0.0, 1.0);
            match d {
                0 => self.opts.render_dist = 3 + (frac * 7.0).round() as i32,
                1 => self.opts.sensitivity = 0.3 + frac * 1.7,
                2 => self.opts.fov = 60.0 + frac * 50.0,
                _ => {}
            }
        }
        if clicked {
            if hit_scale {
                self.opts.gui_scale = (self.opts.gui_scale + 1) % 4;
            } else if hit_fog {
                self.opts.fog = !self.opts.fog;
            } else if hit_back {
                self.opts.save(&self.base);
                self.screen = Screen::Menu;
            }
        }
    }

    fn start_connecting(&mut self, solo: bool, fresh_seed: Option<u64>) {
        self.conn_err = None;
        let label = if solo {
            "Monde local".to_string()
        } else {
            format!("Serveur {}", self.in_addr)
        };
        match if solo {
            let save = self.base.join(SAVE_NAME);
            if let Some(seed) = fresh_seed {
                // fresh world: the server will not find the (deleted) save
                std::fs::remove_file(&save).ok();
                let cfg = ServerConfig::local(save, Some(seed));
                server::spawn_local(cfg)
            } else {
                server::spawn_local(ServerConfig::local(save, None))
            }
            .map(|a| a.to_string())
        } else {
            Ok(self.in_addr.trim().to_string())
        } {
            Ok(addr) => {
                let name = self.in_name.trim().is_empty().then(|| "Joueur".to_string()).unwrap_or_else(|| self.in_name.clone());
                match Game::new(&addr, &name, solo, label, self.opts.render_dist) {
                    Ok(g) => {
                        self.game = Some(g);
                        self.screen = Screen::Connecting;
                    }
                    Err(e) => self.conn_err = Some(e),
                }
            }
            Err(e) => self.conn_err = Some(format!("serveur local: {e}")),
        }
    }

    fn frame_connecting(&mut self, p: &mut Painter, gw: f32, gh: f32, mouse: (f32, f32), clicked: bool) {
        win32::set_captured(false);
        ui::dirt_bg(p, T_DIRT, gw, gh);
        if let Some(g) = self.game.as_mut() {
            g.cli.pump();
            let (phase, have, need) = (
                g.cli.phase,
                g.cli.chunks_have.min(g.cli.chunks_needed),
                g.cli.chunks_needed,
            );
            if let Some(k) = g.cli.kick.clone() {
                self.conn_err = Some(k);
                self.game = None;
            } else {
                let label = match phase {
                    Phase::Connecting => "Connexion au serveur...".to_string(),
                    Phase::Downloading => format!("Téléchargement du terrain... {have}/{need}"),
                    Phase::Playing => "Préparation...".to_string(),
                };
                ui::center_text(p, &label, gw * 0.5, gh * 0.42, ui::WHITE, true);
                let frac = if need > 0 { have as f32 / need as f32 } else { 1.0 };
                let bw = 200.0;
                p.rect(gw * 0.5 - bw * 0.5, gh * 0.47, gw * 0.5 + bw * 0.5, gh * 0.47 + 8.0, [0.1, 0.1, 0.1, 0.9]);
                p.rect(gw * 0.5 - bw * 0.5, gh * 0.47, gw * 0.5 - bw * 0.5 + bw * frac, gh * 0.47 + 8.0, [0.35, 0.85, 0.3, 1.0]);
                if phase == Phase::Playing {
                    self.screen = Screen::Game;
                }
            }
        } else if let Some(e) = &self.conn_err.clone() {
            ui::center_text(p, e, gw * 0.5, gh * 0.42, ui::RED, true);
        }
        if self.conn_err.is_some() || self.game.is_none() {
            let hit = self.button(p, "Retour", gw * 0.5, gh * 0.58, 160.0, mouse);
            if clicked && hit {
                self.game = None;
                self.screen = Screen::Menu;
            }
        } else {
            let hit = self.button(p, "Annuler", gw * 0.5, gh * 0.58, 160.0, mouse);
            if clicked && hit {
                self.game = None;
                self.screen = Screen::Menu;
            }
        }
    }
}

// ------------------------------------------------------------------- game
impl App {
    #[allow(clippy::too_many_arguments)]
    fn frame_game(
        &mut self,
        p: &mut Painter,
        keys: &[bool; 32],
        edge: &dyn Fn(usize) -> bool,
        mouse: (f32, f32),
        clicked: bool,
        wheel: f32,
        chars: &[char],
        renderer: &mut Renderer,
        ent_atlas: &mut Option<crate::entity_models::EntityAtlas>,
        palette: &mut BiomePalette,
        _pack: &Pack,
        dt: f32,
        (cw, ch): (i32, i32),
        scale: f32,
        _lmb: bool,
    ) {
        let gw = cw as f32 / scale;
        let gh = ch as f32 / scale;
        let mut g = match self.game.take() {
            Some(g) => g,
            None => {
                self.screen = Screen::Menu;
                return;
            }
        };

        // network receive + kick handling
        g.cli.pump();
        if let Some(k) = g.cli.kick.clone() {
            self.conn_err = Some(format!("Déconnecté: {k}"));
            self.screen = Screen::Menu;
            return;
        }

        g.t_total += dt;
        // local day clock, softly synced with the server clock
        g.day_t = (g.day_t + dt / server::DAY_CYCLE) % 1.0;
        let d = g.day_t - g.cli.day_t;
        if d.abs() > 0.5 {
            g.day_t = g.cli.day_t;
        } else {
            g.day_t -= d * 0.05;
        }
        g.cli.animate_ghosts(dt);

        // ui open states
        if edge(0) {
            if g.inv_open {
                g.inv_open = false;
            } else if g.chat_input.is_some() {
                g.chat_input = None;
            } else {
                g.paused = !g.paused;
            }
        }
        if edge(2) && !g.paused && g.chat_input.is_none() {
            g.inv_open = !g.inv_open;
        }
        if edge(5) {
            self.f3_on = !self.f3_on;
        }
        let ui_open = g.paused || g.inv_open || g.chat_input.is_some() || g.cli.dead;
        win32::set_captured(!ui_open);

        // ---------------- regard souris (yaw/pitch) — disponible hors UI
        if !ui_open {
            let (dx, dy) = win32::mouse_delta();
            if dx != 0.0 || dy != 0.0 {
                let s = 0.0025 * self.opts.sensitivity;
                g.player.yaw = (g.player.yaw + dx * s).rem_euclid(std::f32::consts::TAU);
                let lim = std::f32::consts::FRAC_PI_2 - 0.01;
                g.player.pitch = (g.player.pitch - dy * s).clamp(-lim, lim);
            }
        }

        // ---------------- chat input
        if g.chat_input.is_some() {
            let mut done: Option<String> = None;
            if let Some(input) = &mut g.chat_input {
                for &c in chars {
                    match c {
                        '\r' | '\n' => {
                            done = Some(input.clone());
                        }
                        '\u{8}' => {
                            input.pop();
                        }
                        c if !c.is_control() && input.chars().count() < 100 => input.push(c),
                        _ => {}
                    }
                }
            }
            if let Some(m) = done {
                g.chat_input = None;
                if !m.trim().is_empty() {
                    g.cli.say(&m);
                }
            }
        } else if !ui_open {
            for c in chars {
                if *c == 't' || *c == 'T' {
                    g.chat_input = Some(String::new());
                } else if *c == '/' {
                    g.chat_input = Some("/".into());
                }
            }
        }

        // ---------------- movement prediction
        let input = if ui_open {
            Input::default()
        } else {
            // ZQSD (AZERTY) et WASD (QWERTY) supportés simultanément
            Input {
                fwd: ((win32::key_down(0x57) || win32::key_down(0x5A)) as i32
                    - win32::key_down(0x53) as i32) as f32,
                side: (win32::key_down(0x44) as i32
                    - (win32::key_down(0x41) || win32::key_down(0x51)) as i32) as f32,
                jump: win32::key_down(0x20),
                sneak: win32::key_down(0x10),
                sprint: win32::key_down(0x11),
            }
        };
        if edge(1) && !ui_open {
            g.player.flying = !g.player.flying;
            g.player.vel.y = 0.0;
        }
        let hp_before = g.player.hp;
        g.player.update(dt, &input, &g.cli.world);
        // dégâts calculés côté serveur (autoritaire, comme vanilla)
        let _ = hp_before;
        g.player.hp = g.cli.hp;
        g.player.dead = g.cli.dead;

        // ---------------- interactions
        let eye = g.player.eye();
        let fwd = forward(g.player.yaw, g.player.pitch);
        let sel = raycast::raycast(&g.cli.world, eye, fwd, 5.5);
        let ghost_list = g.cli.ghost_mobs();
        let ghost_uids: Vec<u32> = {
            let mut v: Vec<(u32, &crate::client::Ghost)> = g.cli.ghosts.iter().map(|(k, v)| (*k, v)).collect();
            v.sort_by_key(|(k, _)| *k);
            v.iter().map(|(k, _)| *k).collect()
        };
        let (mobs_sorted, ghost_uids) = {
            let mut pairs: Vec<(u32, crate::mobs::Mob)> = ghost_list
                .into_iter()
                .zip(ghost_uids.into_iter().map(Some))
                .map(|(m, id)| (id.unwrap_or(0), m))
                .collect();
            pairs.sort_by_key(|(id, _)| *id);
            let ids: Vec<u32> = pairs.iter().map(|(id, _)| *id).collect();
            (pairs.into_iter().map(|(_, m)| m).collect::<Vec<_>>(), ids)
        };
        let mob_hit = mobs::pick_mob(&mobs_sorted, eye, fwd, 3.8);
        let mob_first = match (mob_hit, sel) {
            (Some((_, mt)), Some(bh)) => mt < bh.t,
            (Some(_), None) => true,
            _ => false,
        };
        g.attack_cd = (g.attack_cd - dt).max(0.0);

        let can_act = !ui_open && !g.cli.dead;
        // attack / mine (LMB)
        if keys[17] && can_act {
            if mob_first {
                g.mine_target = None;
                g.mine_progress = 0.0;
                if edge(17) && g.attack_cd <= 0.0 {
                    g.attack_cd = 0.4;
                    g.swing = 0.0;
                    if let Some((mi, _)) = mob_hit {
                        g.cli.send_attack(ghost_uids[mi]);
                    }
                }
            } else if let Some(h) = sel {
                let key = (h.x, h.y, h.z);
                if g.mine_target != Some(key) {
                    g.mine_target = Some(key);
                    g.mine_progress = 0.0;
                }
                let b = g.cli.world.get_block(h.x, h.y, h.z);
                let hard = if g.cli.creative { 0.3 } else { hardness(b) };
                g.mine_progress += dt / hard.max(0.05);
                if g.mine_progress >= 1.0 {
                    g.mine_progress = 0.0;
                    g.mine_target = None;
                    g.cli.send_dig(h.x, h.y, h.z);
                    g.cli.world.set_block(h.x, h.y, h.z, AIR);
                    g.mark(h.x, h.z);
                    g.swing = 0.0;
                    let tile = tiles_of(b)[1];
                    let avg = renderer.tile_avg.get(tile as usize).copied().unwrap_or([0.5, 0.5, 0.5]);
                    for i in 0..8 {
                        let a = i as f32 * 0.785;
                        g.particles.push(Particle::new(
                            Vec3::new(h.x as f32 + 0.5, h.y as f32 + 0.5, h.z as f32 + 0.5),
                            Vec3::new(a.cos() * 2.2, 2.2 + (i % 3) as f32, a.sin() * 2.2),
                            0.5,
                            [avg[0], avg[1], avg[2], 1.0],
                            true,
                        ));
                    }
                }
            } else {
                g.mine_target = None;
                g.mine_progress = 0.0;
            }
        } else {
            g.mine_target = None;
            g.mine_progress = 0.0;
        }

        // place / eat / sleep (RMB, 0.22 s repeat)
        if keys[18] && can_act && g.t_total - g.last_place > 0.22 {
            g.last_place = g.t_total;
            let held = g.hotbar[g.slot];
            if held == MEAT {
                if g.attack_cd <= 0.0 && g.cli.hunger < 20 {
                    g.attack_cd = 0.5;
                    g.swing = 0.0;
                    g.cli.send_eat();
                }
            } else if let Some(h) = sel {
                if g.cli.world.get_block(h.x, h.y, h.z) == STRAW_BED {
                    // Wilderness Bound beds: skip the night (server command)
                    g.cli.say("/time jour");
                } else {
                    let (px, py, pz) = (h.x + h.nx, h.y + h.ny, h.z + h.nz);
                    let cur_b = g.cli.world.get_block(px, py, pz);
                    if (cur_b == AIR || cur_b == WATER || is_cross_id(cur_b))
                        && py >= 0
                        && py < CY as i32
                        && !g.player.aabb_overlaps(px, py, pz)
                    {
                        let below = g.cli.world.get_block(px, py - 1, pz);
                        let ok_soil = if is_cross_id(held) {
                            matches!(
                                below,
                                GRASS | DIRT | GRASS_PALE | SAND | RED_SAND | PODZOL
                                    | COARSE_DIRT | MUD | MOSS | GRAVEL
                            )
                        } else if held == CACTUS {
                            below == SAND || below == RED_SAND || below == CACTUS
                        } else {
                            true
                        };
                        if ok_soil {
                            g.cli.send_place(px, py, pz, held);
                            g.cli.world.set_block(px, py, pz, held);
                            g.mark(px, pz);
                            g.swing = 0.0;
                        }
                    }
                }
            }
        }

        // pick block (MMB)
        if edge(19) && !ui_open {
            if let Some(h) = sel {
                let b = g.cli.world.get_block(h.x, h.y, h.z);
                if b != AIR {
                    if let Some(i) = g.hotbar.iter().position(|&x| x == b) {
                        g.slot = i;
                        g.item_name_t = 1.6;
                    } else {
                        g.hotbar[g.slot] = b;
                        g.item_name_t = 1.6;
                    }
                }
            }
        }

        // hotbar select: keys + wheel
        for s in 0..9 {
            if edge(8 + s) {
                g.slot = s;
                g.item_name_t = 1.6;
                g.cli.set_held(s);
            }
        }
        if wheel != 0.0 && !ui_open {
            let dir = if wheel > 0.0 { -1i32 } else { 1 };
            g.slot = (g.slot as i32 + dir).rem_euclid(9) as usize;
            g.item_name_t = 1.6;
            g.cli.set_held(g.slot);
        }
        g.item_name_t = (g.item_name_t - dt).max(0.0);

        // swing animation
        if g.swing >= 0.0 {
            g.swing += dt * 3.2;
            if g.swing > 1.0 {
                g.swing = -1.0;
            }
        }

        // send positions / pings at 20 Hz
        g.cli.tick();

        // ---------------- events -> particles
        for ev in std::mem::take(&mut g.cli.events) {
            match ev {
                NetEvent::Break(pos, b) => {
                    let tile = tiles_of(b)[1];
                    let avg = renderer.tile_avg.get(tile as usize).copied().unwrap_or([0.5, 0.5, 0.5]);
                    for i in 0..8 {
                        let a = i as f32 * 0.785;
                        g.particles.push(Particle::new(
                            pos,
                            Vec3::new(a.cos() * 2.4, 2.4 + (i % 3) as f32, a.sin() * 2.4),
                            0.55,
                            [avg[0], avg[1], avg[2], 1.0],
                            true,
                        ));
                    }
                }
                NetEvent::Explosion(pos) => {
                    for i in 0..30 {
                        let a = i as f32 * 0.21;
                        let sp = 3.0 + (i % 5) as f32 * 1.6;
                        g.particles.push(Particle::new(
                            pos,
                            Vec3::new(a.cos() * sp, 2.0 + (i % 4) as f32 * 1.8, a.sin() * sp),
                            0.8,
                            if i % 3 == 0 { [1.0, 0.6, 0.2, 0.95] } else { [0.4, 0.38, 0.36, 0.9] },
                            i % 3 != 0,
                        ));
                    }
                }
                NetEvent::HurtMob(pos) => {
                    for _ in 0..6 {
                        g.particles.push(Particle::new(
                            pos,
                            Vec3::new(
                                (noise::rand01() - 0.5) * 3.0,
                                noise::rand01() * 2.5,
                                (noise::rand01() - 0.5) * 3.0,
                            ),
                            0.5,
                            [0.75, 0.1, 0.1, 0.9],
                            true,
                        ));
                    }
                }
                NetEvent::Eat(pos) => {
                    for _ in 0..6 {
                        g.particles.push(Particle::new(
                            pos,
                            Vec3::new(0.0, 1.2, 0.0),
                            0.4,
                            [0.85, 0.2, 0.2, 0.9],
                            false,
                        ));
                    }
                }
            }
        }
        mobs::update_particles(&mut g.particles, &g.cli.world, g.t_total, dt);

        // ---------------- meshing
        let pcx = (g.player.pos.x / CX as f32).floor() as i32;
        let pcz = (g.player.pos.z / CZ as f32).floor() as i32;
        let mut mesh_budget = 3;
        for &(dx, dz) in g.offs.iter() {
            if mesh_budget == 0 {
                break;
            }
            if dx.abs() > self.opts.render_dist || dz.abs() > self.opts.render_dist {
                continue;
            }
            let c = (pcx + dx, pcz + dz);
            if g.dirty.contains(&c) && g.cli.world.has_chunk(c) {
                let ready = neighbors8(c.0, c.1).iter().all(|&n| g.cli.world.has_chunk(n));
                if !ready {
                    continue;
                }
                let data = mesher::build_mesh(&g.cli.world, c.0, c.1, palette);
                let m = g.meshes.entry(c).or_insert_with(ChunkMesh::empty);
                renderer.upload_mesh(m, &data);
                g.dirty.remove(&c);
                mesh_budget -= 1;
            }
        }
        let far: Vec<(i32, i32)> = g
            .meshes
            .keys()
            .filter(|&&(cx, cz)| {
                (cx - pcx).abs() > self.opts.render_dist + 2 || (cz - pcz).abs() > self.opts.render_dist + 2
            })
            .copied()
            .collect();
        for c in far {
            if let Some(mut m) = g.meshes.remove(&c) {
                renderer.free_mesh(&mut m);
            }
        }
        // any received chunk inside the render distance without a mesh yet
        // must be queued (the server streams them, the client meshes them)
        for &(dx, dz) in g.offs.iter() {
            if dx.abs() > self.opts.render_dist || dz.abs() > self.opts.render_dist {
                continue;
            }
            let c = (pcx + dx, pcz + dz);
            if g.cli.world.has_chunk(c) && !g.meshes.contains_key(&c) {
                g.dirty.insert(c);
            }
        }

        // ---------------- world render
        let (light, night, fog_col_sky, sun_dir, _elev) = server::sky_state(g.day_t);
        let eye_in_water = g
            .cli
            .world
            .get_block(eye.x.floor() as i32, eye.y.floor() as i32, eye.z.floor() as i32)
            == WATER;
        let (fog_near, fog_far, fog_col) = if eye_in_water {
            (
                2.0,
                22.0,
                [
                    fog_col_sky[0] * 0.35 + 0.02,
                    fog_col_sky[1] * 0.4 + 0.06,
                    fog_col_sky[2] * 0.5 + 0.12,
                ],
            )
        } else if self.opts.fog {
            let far = self.opts.render_dist as f32 * 16.0 - 6.0;
            (far * 0.55, far, fog_col_sky)
        } else {
            (1.0e6, 2.0e6, fog_col_sky)
        };
        let mut mob_verts: Vec<f32> = Vec::with_capacity(mobs_sorted.len() * 216 * 8);
        let mut mob_verts_pack: Vec<f32> = Vec::new();
        match ent_atlas {
            Some(a) => {
                // rigs vanilla (textures d'entités du pack) + fallback procédural
                let rendered = crate::entity_models::build_verts(&mobs_sorted, a, &mut mob_verts_pack);
                mobs::build_verts_filtered(&mobs_sorted, &mut mob_verts, &|k| {
                    !rendered.contains(&k.id())
                });
            }
            None => {
                mobs::build_verts(&mobs_sorted, &mut mob_verts);
            }
        }
        let mut packed: Vec<f32> = Vec::with_capacity(g.particles.len() * 7);
        for pa in &g.particles {
            pa.pack(&mut packed);
        }
        // nametags for remote players
        let tags: Vec<(Vec3, String)> = mobs_sorted
            .iter()
            .zip(ghost_uids.iter())
            .filter(|(m, _)| m.kind == crate::mobs::MobKind::Player)
            .filter_map(|(m, uid)| {
                g.cli
                    .players
                    .iter()
                    .find(|(id, _)| id == uid)
                    .map(|(_, name)| (m.pos, name.clone()))
            })
            .collect();
        let mine_overlay = g
            .mine_target
            .map(|(x, y, z)| (x, y, z, g.mine_progress));
        renderer.draw_frame(
            &g.meshes,
            &g.player,
            sel.as_ref(),
            cw,
            ch,
            self.opts.fov,
            fog_near,
            fog_far,
            fog_col,
            light,
            sun_dir,
            night,
            g.t_total,
            &mob_verts,
            &mob_verts_pack,
            &packed,
            mine_overlay,
            p,
            scale,
            &tags,
        );

        // ---------------- HUD
        let chat: Vec<(f64, String)> = g
            .cli
            .chat
            .iter()
            .map(|(t, s)| (t.elapsed().as_secs_f64(), s.clone()))
            .collect();
        let tab = if keys[4] {
            let mut v: Vec<(String, u32)> = g
                .cli
                .players
                .iter()
                .map(|(_, n)| (n.clone(), g.cli.ping_ms))
                .collect();
            v.push((format!("{} (vous)", g.cli.name), g.cli.ping_ms));
            v.sort();
            Some(v)
        } else {
            None
        };
        let f3 = if self.f3_on {
            Some(self.f3_lines(&g, mobs_sorted.len()))
        } else {
            None
        };
        let held = g.hotbar[g.slot];
        let hud = ui::HudData {
            hotbar: g.hotbar,
            slot: g.slot,
            hp: g.cli.hp,
            hunger: g.cli.hunger,
            air: g.cli.air,
            xp: ((g.cli.xp % 10) as f32) / 10.0,
            level: (g.cli.xp / 10) as i32,
            dead: g.cli.dead,
            creative: g.cli.creative,
            item_name_t: g.item_name_t,
            chat,
            chat_input: g.chat_input.clone(),
            f3,
            tab,
            swing_t: g.swing,
            hurt_shake: 0.0,
        };
        ui::draw_hud(p, &hud, gw, gh, g.t_total as f64);

        // ---------------- overlays
        if g.cli.dead {
            p.rect(0.0, 0.0, gw, gh, [0.45, 0.02, 0.02, 0.4]);
            ui::center_text(p, "Vous êtes mort !", gw * 0.5, gh * 0.4, ui::WHITE, true);
            let hit1 = self.button(p, "Réapparaître", gw * 0.5, gh * 0.55, 180.0, mouse);
            let hit2 = self.button(p, "Menu principal", gw * 0.5, gh * 0.55 + 24.0, 180.0, mouse);
            if clicked {
                if hit1 {
                    g.cli.send_respawn();
                } else if hit2 {
                    self.screen = Screen::Menu;
                    return;
                }
            }
        } else if g.paused {
            p.rect(0.0, 0.0, gw, gh, [0.05, 0.05, 0.08, 0.55]);
            ui::center_text(p, "Jeu en pause", gw * 0.5, gh * 0.32, ui::WHITE, true);
            let hit1 = self.button(p, "Reprendre", gw * 0.5, gh * 0.45, 200.0, mouse);
            let hit3 = self.button(p, "Quitter au menu", gw * 0.5, gh * 0.45 + 46.0, 200.0, mouse);
            let hint = if g.solo {
                "le monde local est sauvegardé à la sortie"
            } else {
                &g.label
            };
            ui::center_text(p, hint, gw * 0.5, gh * 0.45 + 78.0, ui::GRAY, true);
            if clicked {
                if hit1 {
                    g.paused = false;
                } else if hit3 {
                    self.screen = Screen::Menu;
                    return;
                }
            }
        } else if g.inv_open {
            self.draw_inventory(p, &mut g, gw, gh, mouse, clicked);
        }

        let _ = (held, edge);
        // keep the game alive for the next frame
        self.game = Some(g);
    }

    fn f3_lines(&self, g: &Game, mob_count: usize) -> Vec<String> {
        let pos = &g.player.pos;
        let facing = match ((g.player.yaw.rem_euclid(std::f32::consts::TAU)
            / std::f32::consts::FRAC_PI_2)
            .round()
            as i32)
            .rem_euclid(4)
        {
            0 => "nord (-Z)",
            1 => "est (+X)",
            2 => "sud (+Z)",
            _ => "ouest (-X)",
        };
        let biome = BIOME_NAMES
            [g.cli.world.biome_at(pos.x as i32, pos.z as i32) as usize];
        let phase = if g.day_t < 0.22 || g.day_t > 0.78 {
            "jour"
        } else if g.day_t < 0.3 || g.day_t > 0.7 {
            "aube/crépuscule"
        } else {
            "nuit"
        };
        vec![
            format!("RustVoxel {} ({} fps)", env!("CARGO_PKG_VERSION"), self.fps),
            format!(
                "XYZ: {:.3} / {:.3} / {:.3}",
                pos.x, pos.y, pos.z
            ),
            format!(
                "Chunk: {} {} dans {} {}",
                (pos.x as i32).rem_euclid(16),
                (pos.z as i32).rem_euclid(16),
                pos.x as i32 / 16,
                pos.z as i32 / 16
            ),
            format!("Regard: {} biome: {}", facing, biome),
            format!(
                "Heure: {:.2} ({}) entités: {} chunks: {}",
                g.day_t,
                phase,
                mob_count,
                g.cli.world.chunks.len()
            ),
            format!(
                "PV: {} faim: {} air: {} XP: {} (niv {})",
                g.cli.hp, g.cli.hunger, g.cli.air, g.cli.xp, g.cli.level
            ),
            format!(
                "Serveur: {} ping: {} ms mode: {}",
                g.label,
                g.cli.ping_ms,
                if g.cli.creative { "créatif" } else { "survie" }
            ),
            format!(
                "Souris: capture={} inv={} pause={} chat={}",
                crate::win32::cursor_captured(),
                g.inv_open,
                g.paused,
                g.chat_input.is_some()
            ),
        ]
    }

    fn draw_inventory(&mut self, p: &mut Painter, g: &mut Game, gw: f32, gh: f32, mouse: (f32, f32), clicked: bool) {
        let w = 200.0;
        let h = 158.0;
        let x0 = (gw - w) * 0.5;
        let y0 = (gh - h) * 0.5;
        p.rect(0.0, 0.0, gw, gh, [0.0, 0.0, 0.0, 0.45]);
        ui::panel(p, x0, y0, w, h);
        ui::center_text(p, "Inventaire", gw * 0.5, y0 + 5.0, [0.3, 0.3, 0.35, 1.0], false);
        // grid: 9 x 4 = 36 slots per page
        const PER: usize = 36;
        let pages = (PLACEABLE.len() + PER - 1) / PER;
        let page = g.inv_page.min(pages - 1);
        let mut tooltip: Option<(f32, f32, u16)> = None;
        for i in 0..PER {
            let idx = page * PER + i;
            if idx >= PLACEABLE.len() {
                break;
            }
            let b = PLACEABLE[idx];
            let col = (i % 9) as f32;
            let row = (i / 9) as f32;
            let sx = x0 + 10.0 + col * 20.0;
            let sy = y0 + 16.0 + row * 20.0;
            ui::slot_inset(p, sx, sy, 18.0);
            ui::block_icon(p, b, sx + 9.0, sy + 9.0, 13.0);
            if ui::hover(mouse, sx, sy, 18.0, 18.0) {
                p.rect_outline(sx, sy, sx + 18.0, sy + 18.0, 1.0, [1.0, 1.0, 1.0, 0.8]);
                tooltip = Some((mouse.0, mouse.1, b));
            }
            if clicked && ui::hover(mouse, sx, sy, 18.0, 18.0) {
                g.hotbar[g.slot] = b;
                g.item_name_t = 1.6;
            }
        }
        // hotbar row (pick target slot)
        for i in 0..9 {
            let sx = x0 + 10.0 + i as f32 * 20.0;
            let sy = y0 + h - 28.0;
            ui::slot_inset(p, sx, sy, 18.0);
            ui::block_icon(p, g.hotbar[i], sx + 9.0, sy + 9.0, 13.0);
            if i == g.slot {
                p.rect_outline(sx - 1.0, sy - 1.0, sx + 19.0, sy + 19.0, 1.0, [1.0, 1.0, 1.0, 0.95]);
            }
            if clicked && ui::hover(mouse, sx, sy, 18.0, 18.0) {
                g.slot = i;
            }
        }
        // page buttons
        let py = y0 + 100.0;
        let hit_prev = self.button(p, "<", x0 + 30.0, py, 30.0, mouse);
        let hit_next = self.button(p, ">", x0 + w - 60.0, py, 30.0, mouse);
        ui::center_text(p, &format!("page {}/{}", page + 1, pages), gw * 0.5, py + 5.0, ui::GRAY, true);
        if clicked {
            if hit_prev {
                g.inv_page = (page + pages - 1) % pages;
            } else if hit_next {
                g.inv_page = (page + 1) % pages;
            }
        }
        // tooltip
        if let Some((mx, my, b)) = tooltip {
            let name = block_name(b);
            let tw = ui::text_w(&name) + 6.0;
            let tx = (mx + 8.0).min(gw - tw - 2.0);
            let ty = (my - 16.0).max(2.0);
            p.rect(tx, ty, tx + tw, ty + 12.0, [0.05, 0.0, 0.06, 0.92]);
            p.rect_outline(tx, ty, tx + tw, ty + 12.0, 1.0, [0.35, 0.1, 0.5, 0.95]);
            ui::text(p, &name, tx + 3.0, ty + 2.0, ui::WHITE, true);
        }
        ui::center_text(p, "clic: mettre dans la case sélectionnée - E pour fermer", gw * 0.5, y0 + h + 6.0, ui::GRAY, true);
    }
}
