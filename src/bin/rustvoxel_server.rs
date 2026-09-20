// RustVoxel dedicated server (headless, cross-platform, pure std).
//
// Usage:
//   rustvoxel_server [--port N] [--motd "text"] [--seed N] [--max N]
//                    [--view N] [--world path] [--no-save-interval]
//
// Console commands: /help /list /time /tp /gamemode /kick /say /seed /save /stop
//
// Monde par défaut : <dossier de l'exécutable>/world/ — un DOSSIER au format
// Anvil vanilla (world/region/r.X.Z.mca). Le monde survit aux redémarrages et
// peut être un vrai monde de Minecraft : `--world "<.minecraft>/saves/Mon monde"`
// sert tel quel la sauvegarde du jeu (graine lue dans level.dat, chunks dans
// region/*.mca). Un ancien rustvoxel_world.sav voisin du dossier world/ est
// migré automatiquement à la première sauvegarde (le .sav reste en place).
// `--world fichier.sav` conserve le format historique du jeu solo.
// Ctrl+C / fermeture de la console (Windows) déclenchent une sauvegarde propre.
use rustvoxel::server::{serve, default_world_path, ServerConfig};
use std::path::PathBuf;
#[cfg(windows)]
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name || a == &format!("--{name}"))
        .and_then(|i| args.get(i + 1))
        .cloned()
}

// ---------------------------------------------------------------- Ctrl+C / fermeture console (Windows)
// Le handler signale l'arrêt puis attend (max ~4 s) que la boucle principale
// ait terminé sa sauvegarde finale — au retour du handler sur CTRL_CLOSE_EVENT
// le processus est tué par le système, il faut donc avoir fini avant.
#[cfg(windows)]
static RUNNING_PTR: AtomicPtr<AtomicBool> = AtomicPtr::new(std::ptr::null_mut());
#[cfg_attr(not(windows), allow(dead_code))]
static SAVED: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
extern "system" fn on_ctrl(_ctrl: u32) -> i32 {
    unsafe {
        if let Some(flag) = RUNNING_PTR.load(Ordering::SeqCst).as_ref() {
            flag.store(false, Ordering::SeqCst);
        }
    }
    // laisse la boucle serveur sauvegarder avant que Windows ne tue le process
    for _ in 0..400 {
        if SAVED.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    1 // TRUE = géré : pas de fin de force par le runtime
}

#[cfg(windows)]
fn install_ctrl_handler(running: &Arc<AtomicBool>) -> bool {
    use std::os::raw::c_int;
    #[link(name = "kernel32")]
    extern "system" {
        fn SetConsoleCtrlHandler(
            handler: Option<extern "system" fn(u32) -> c_int>,
            add: c_int,
        ) -> c_int;
    }
    RUNNING_PTR.store(
        Arc::into_raw(running.clone()) as *mut AtomicBool,
        Ordering::SeqCst,
    );
    unsafe { SetConsoleCtrlHandler(Some(on_ctrl), 1) == 1 }
}

fn main() {
    let port: u16 = arg("port").and_then(|v| v.parse().ok()).unwrap_or(25565);
    let cfg = ServerConfig {
        port,
        max_players: arg("max").and_then(|v| v.parse().ok()).unwrap_or(8),
        view: arg("view").and_then(|v| v.parse().ok()).unwrap_or(7).clamp(2, 12),
        motd: arg("motd").unwrap_or_else(|| "Serveur RustVoxel".into()),
        seed: arg("seed")
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(rustvoxel::noise::rand_seed),
        world_path: arg("world")
            .map(PathBuf::from)
            .unwrap_or_else(default_world_path),
        save_interval_s: 180,
        verbose: true,
        stop_when_empty: false,
    };

    println!("RustVoxel serveur dédié v{}", env!("CARGO_PKG_VERSION"));
    println!(
        "  port {} | motd \"{}\" | {} joueurs max",
        cfg.port, cfg.motd, cfg.max_players
    );
    let world_state = if cfg.world_path.is_file() {
        "chargé (.sav)"
    } else if rustvoxel::anvil::dir_has_regions(&cfg.world_path) {
        "chargé (Anvil)"
    } else {
        "nouveau (Anvil)"
    };
    let world_abs = std::fs::canonicalize(&cfg.world_path).unwrap_or_else(|_| cfg.world_path.clone());
    println!("  monde: {} ({})", world_abs.display(), world_state);
    println!("  console: /help pour les commandes, /stop pour arrêter");
    let running = Arc::new(AtomicBool::new(true));
    #[cfg(windows)]
    {
        if install_ctrl_handler(&running) {
            println!("  Ctrl+C / fermeture de fenêtre : sauvegarde automatique");
        }
    }
    if let Err(e) = serve(cfg, running) {
        eprintln!("erreur serveur: {e}");
        std::process::exit(1);
    }
    SAVED.store(true, Ordering::SeqCst);
    println!("serveur arrêté, monde sauvegardé.");
}
