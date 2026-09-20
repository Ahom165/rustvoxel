// RustVoxel dedicated server (headless, cross-platform, pure std).
//
// Usage:
//   rustvoxel_server [--port N] [--motd "text"] [--seed N] [--max N]
//                    [--view N] [--world path] [--no-save-interval]
//
// Console commands: /help /list /time /tp /gamemode /kick /say /seed /save /stop
use rustvoxel::server::{serve, ServerConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name || a == &format!("--{name}"))
        .and_then(|i| args.get(i + 1))
        .cloned()
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
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("rustvoxel_world.sav")),
        save_interval_s: 180,
        verbose: true,
        stop_when_empty: false,
    };

    println!("RustVoxel serveur dédié v{}", env!("CARGO_PKG_VERSION"));
    println!("  port {} | motd \"{}\" | {} joueurs max", cfg.port, cfg.motd, cfg.max_players);
    println!("  console: /help pour les commandes, /stop pour arrêter");
    let running = Arc::new(AtomicBool::new(true));
    let r2 = running.clone();
    // Ctrl+C on unix: default behavior kills the process; save best-effort
    // happens on /stop. Keep a parked thread so the flag stays meaningful.
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if !r2.load(Ordering::SeqCst) {
                break;
            }
        }
    });
    if let Err(e) = serve(cfg, running) {
        eprintln!("erreur serveur: {e}");
        std::process::exit(1);
    }
    println!("serveur arrêté, monde sauvegardé.");
}

use std::path::PathBuf;
