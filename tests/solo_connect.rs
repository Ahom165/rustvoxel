// Reproduit EXACTEMENT le flux solo de app.rs::start_connecting :
//   1. "Nouveau monde"     : remove_file(save) -> spawn_local(local(save, Some(seed))) -> Client::connect
//   2. "Continuer le monde": spawn_local(local(save, None)) -> Client::connect (le save existe)
// puis pump() jusqu'à Phase::Playing — comme frame_connecting.
use rustvoxel::client::{Client, Phase};
use rustvoxel::noise;
use rustvoxel::server::{self, ServerConfig};

fn pump_until_playing(c: &mut Client, max_ms: u64) -> bool {
    let t0 = std::time::Instant::now();
    while (t0.elapsed().as_millis() as u64) < max_ms {
        c.pump();
        if let Some(k) = &c.kick {
            panic!("kick pendant la connexion : {k}");
        }
        if c.phase == Phase::Playing {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    false
}

#[test]
fn solo_new_world_flow() {
    let dir = std::env::temp_dir().join(format!("rvx_solo_new_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let save = dir.join("rustvoxel_world.sav");
    std::fs::remove_file(&save).ok();

    // start_connecting(true, Some(seed))
    let addr = server::spawn_local(ServerConfig::local(save.clone(), Some(noise::rand_seed())))
        .expect("spawn_local doit réussir");
    let mut g = Client::connect(&addr.to_string(), "Joueur")
        .expect("Client::connect (solo nouveau monde) doit réussir");
    g.tick();
    assert!(
        pump_until_playing(&mut g, 60_000),
        "phase Playing jamais atteinte (chunks {}/{}), kick={:?}",
        g.chunks_have,
        g.chunks_needed,
        g.kick
    );
    // le serveur intégré a sauvegardé au moins une fois -> le save doit exister
    drop(g);
    std::thread::sleep(std::time::Duration::from_secs(10));
}

#[test]
fn solo_continue_world_flow() {
    let dir = std::env::temp_dir().join(format!("rvx_solo_cont_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let save = dir.join("rustvoxel_world.sav");

    // 1re session : créer le monde et laisser le serveur sauvegarder
    {
        let addr = server::spawn_local(ServerConfig::local(save.clone(), Some(4242)))
            .expect("spawn_local #1");
        let mut g = Client::connect(&addr.to_string(), "Joueur").expect("connect #1");
        assert!(pump_until_playing(&mut g, 60_000), "1re session: pas Playing");
        // pose un bloc pour marquer le chunk modifié
        g.send_dig(0, 100, 0);
        std::thread::sleep(std::time::Duration::from_millis(500));
        g.pump();
    }
    // attend l'arrêt du serveur intégré (stop_when_empty = 8 s max) + save final
    std::thread::sleep(std::time::Duration::from_secs(10));
    assert!(save.exists(), "le save doit exister après la 1re session");

    // 2e session : "Continuer le monde" (seed None -> le save est chargé)
    let addr = server::spawn_local(ServerConfig::local(save.clone(), None))
        .expect("spawn_local #2");
    let mut g = Client::connect(&addr.to_string(), "Joueur")
        .expect("Client::connect (continuer le monde) doit réussir");
    assert!(
        pump_until_playing(&mut g, 60_000),
        "continuer: phase Playing jamais atteinte, kick={:?}",
        g.kick
    );
    assert_eq!(
        g.world.get_block(0, 100, 0),
        rustvoxel::world::AIR,
        "le bloc creusé en session 1 doit avoir persisté"
    );
}

#[test]
fn solo_continue_with_corrupt_save_still_connects() {
    let dir = std::env::temp_dir().join(format!("rvx_solo_corrupt_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let save = dir.join("rustvoxel_world.sav");
    std::fs::write(&save, b"RVX2gayageincomplet").unwrap();

    // "Continuer le monde" avec un save corrompu : le serveur doit retomber
    // sur un monde neuf AU LIEU de planter (le client doit quand même entrer).
    let addr = server::spawn_local(ServerConfig::local(save.clone(), None))
        .expect("spawn_local (save corrompu)");
    let mut g = Client::connect(&addr.to_string(), "Joueur")
        .expect("connect avec save corrompu doit réussir (monde neuf)");
    assert!(pump_until_playing(&mut g, 60_000), "pas Playing, kick={:?}", g.kick);
}

#[test]
fn solo_continue_with_truncated_rvx2_save_still_connects() {
    // save RVX2 valide mais coupé en plein RLE (simule une sauvegarde avortée)
    let dir = std::env::temp_dir().join(format!("rvx_solo_trunc_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let save = dir.join("rustvoxel_world.sav");

    let mut w = rustvoxel::world::World::new(7);
    w.gen_chunk(0, 0);
    // marque un chunk modifié pour que le save contienne réellement des données
    w.set_block(2, 70, 2, rustvoxel::world::COBBLE);
    rustvoxel::server::save_world(&w, &save).unwrap();
    let full = std::fs::read(&save).unwrap();
    assert!(full.len() > 64, "le save doit contenir un chunk");
    std::fs::write(&save, &full[..full.len() - 40]).unwrap();

    let addr = server::spawn_local(ServerConfig::local(save.clone(), None)).expect("spawn_local");
    let mut g = Client::connect(&addr.to_string(), "Joueur").expect("connect");
    assert!(pump_until_playing(&mut g, 60_000), "pas Playing, kick={:?}", g.kick);
}
