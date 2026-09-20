# RustVoxel v0.7.3

Sandbox voxel original en Rust pur (**zéro dépendance**) : moteur, rendu
OpenGL, monde 1.18-style (grottes 3D, montagnes, lush/dripstone), biomes avec
teinture par biome, **multijoueur intégré** : client OpenGL + **serveur dédié
headless parlant le protocole vanilla Minecraft Java 1.20.5**, le tout
toujours **sans une seule dépendance externe**.

> Projet original inspiré du genre « voxel sandbox ». Aucun code, aucun asset,
> aucune texture ni aucun nom provenant d'un jeu existant n'est intégré.
> Le protocole réseau est une réimplémentation d'interopérabilité fondée sur la
> spécification publique (wiki.vg / données machine PrismarineJS) — le serveur
> est en mode offline (fonction vanilla). Non affilié à Mojang / Microsoft.

## Correctif v0.7.3 — souris, registres client Java récent, packs

**1) « On peut pas bouger la souris »** : `win32::mouse_delta()` (delta du
curseur re-centré) existait mais **n'était jamais appelé** — le yaw/pitch du
joueur restait figé sur ses valeurs initiales, le regard ne tournait jamais
(et le minage/la pose visaient une direction fixe). Correctif : le regard est
appliqué chaque frame hors interfaces (pause/inventaire/chat), avec la
sensibilité des options (0.3-2.0) et un pitch borné ±89°.

**2) Client Java officiel récent déconnecté pendant la configuration**
(log « Registry loading errors ... Failed to handle packet
ClientboundFinishConfigurationPacket ... Network Protocol Error »). Trois
déviations dans les registres envoyés en phase configuration :
- `dimension_type` : `monster_spawn_light_level` était au format wrappé
  `{type:uniform,value:{min,max}}` — les clients >= 1.21.5 veulent soit un
  **entier simple**, soit `{type,min,max}` à plat ; on envoie un entier
  (accepté par toutes les versions) ;
- `chat_type` : la décoration était wrappée `{chat:{decoration:{...}}}`
  (format 1.19.3-1.21.4) — les clients récents veulent les champs **en ligne**
  `{chat:{translation_key,parameters}}` ;
- `damage_type` : le registre ne contenait qu'**une entrée** (`in_fire`) — les
  tags par défaut du client référencent tous les types vanilla, d'où
  « Unbound values ... [minecraft:thorns] » ; le serveur envoie désormais les
  **40 types vanilla complets** (message_id/scaling/exhaustion/effects/
  death_message_type) dans l'ordre alphabétique.
Anti-régression : test `registry_payloads_are_recent_client_compatible` qui
lit les paquets registres comme un client et vérifie les trois points.

**3) « Mon inv les trucs s'appliquent pas » / packs invisibles** : l'écran
Options affichait **« 0 texture(s) » en dur**, même avec un pack chargé — le
jeu vous faisait croire qu'aucune texture n'était appliquée. Le compteur est
désormais réel (mis à jour au démarrage et au F4), et le menu principal
affiche un rappel quand aucun pack n'est détecté. Voir
texturepacks/README.txt : déposez vos .zip vanilla dans `texturepacks/`
(aucun atlas requis — le jeu assemble les PNG individuels à l'exécution),
F4 pour recharger.

Validation : 106 tests verts (dont 4 intégration solo end-to-end et le
diagnostic d'atlas `tests/diag_textures.rs` qui vérifie que chaque bloc
posable référence des tuiles vivantes avec ET sans pack).

## Correctif v0.7.2 — « Échec de la connexion » en solo réparé

**Symptôme** : en solo (« Nouveau monde » ou « Continuer le monde »), le jeu
restait sur « Connexion au serveur... » puis affichait une erreur de connexion
(`Connexion perdue`) — le monde ne se chargeait jamais. Même symptôme en
multijoueur depuis le client RustVoxel vers n'importe quel serveur.

**Cause racine** : `Client::send_packet` (tous les envois après le login :
teleport_confirm, position 20 Hz, keepalive echo, dig/place, chat...) écrivait
l'id + le payload **sans le préfixe de longueur varint** qui encadre chaque
trame du protocole. Le serveur lisait alors le premier octet (l'id de paquet,
par ex. `0x00`) comme une **longueur de trame = 0**, considérait la trame
illisible, fermait la connexion — et le client recevait un reset au moment
d'entrer dans le monde. Le login bloquant, lui, utilisait bien `write_frame`,
ce qui explique que la connexion passait les phases handshake/login/config
avant de mourir exactement au premier envoi de la phase play.

**Pourquoi les tests ne l'avaient pas vu** : les tests serveur parlaient via un
client de test qui cadre correctement ses trames, et le validateur Python ne
valide que le flux serveur→client. Aucun test ne couvrait les envois du vrai
client. `tests/solo_connect.rs` reproduit désormais le flux exact des menus
solo (nouveau monde, continuer, save corrompu/tronqué) avec le vrai
`Client::connect` : 4 tests d'intégration en plus (98 au total).

**Autres améliorations** : les erreurs de connexion affichent désormais la
description de l'erreur système (`Connexion perdue (Connection reset by peer)`,
etc.) au lieu d'un message nu ; trace réseau optionnelle côté serveur
(`RV_DEBUG=1 rustvoxel_server`) pour diagnostiquer les problèmes de connexion
paquet par paquet.

## Nouveauté v0.7.4 — Persistance du monde au format Anvil vanilla

Le monde du serveur dédié est désormais sauvegardé **au format Anvil de
Minecraft** (`world/region/r.X.Z.mca` + NBT), plus un `.sav` maison :

- **persistance réelle** : chaque chunk modifié est réécrit dans sa région
  (sauvegarde toutes les minutes, à `/save` et à l'arrêt, Ctrl+C inclus) ;
  les chunks intacts d'une région sont préservés **octet pour octet** ;
- **les mondes vanilla sont servis tels quels** :
  `rustvoxel_server --world "<.minecraft>/saves/Mon monde"` charge les chunks
  de `region/*.mca` et la graine de `level.dat` — palette vanilla (Name +
  Properties), sections 1.18+, zlib (niveau quelconque, Huffman dynamique) et
  gzip lus par notre inflate maison ;
- **round-trip exact** : les palettes écrites portent les vrais noms colorés
  (`red_wool`, `light_blue_concrete`…) et, pour les blocs ambigus (dalles /
  escaliers de laine), une propriété `rvx` (id interne) que vanilla ignore
  silencieusement — le monde rechargé est bit à bit identique ;
- **chargement paresseux** par région avec cache LRU : un gros monde vanilla
  (des centaines de fichiers région) est exploré sans tout charger ;
- le monde par défaut devient le **dossier** `<exe>/world/` ; un ancien
  `rustvoxel_world.sav` voisin est migré automatiquement à la première
  sauvegarde (le .sav reste en place) ; `--world fichier.sav` garde le
  format historique du jeu solo ;
- **zéro dépendance conservée** : compresseur DEFLATE fixed-Huffman + LZ77
  (RFC 1951), conteneurs zlib (RFC 1950) et gzip (RFC 1952), lecteur NBT
  disque à racine nommée — réunis dans `src/anvil.rs` ;
- tests : round-trip région + slots préservés, palettes vanilla sans `rvx`,
  alias (tall_grass, ice, clay…), graine `level.dat` (vecteur réel zlib
  Python Huffman dynamique), migration .sav → Anvil ; E2E `scripts/
  e2e_anvil_check.py` (creuser → /save → redémarrer → recharger) et
  `scripts/e2e_vanilla_region.py` (région écrite par un outil tiers, relue
  et round-tripée par le serveur).

## Correctif v0.7.1 — Connexion client Java officielle (ViaVersion) réparée

Le client Java officiel (testé 26.3 via ViaFabricPlus) plantait dès le login.
Quatre déviations au protocole 1.20.5 (protocole 766) ont été trouvées en
comparant chaque paquet à la spécification machine (PrismarineJS minecraft-data
+ code de traduction ViaVersion) :

- **login_success** : le booléen `strictErrorHandling` (ajouté en 1.20.5)
  manquait — le traducteur de protocole lisait au-delà de la fin du paquet
  (`IndexOutOfBoundsException` côté ViaVersion) ;
- **map_chunk** : la longueur de `chunkData` était écrite comme un i32
  big-endian au lieu d'une **VarInt** — désynchronisation dès le premier chunk ;
- **map_chunk** : les block entities étaient écrits DANS le buffer alors que le
  protocole les attend dans un champ séparé juste après ;
- **map_chunk lumière** : les 4 masques de lumière n'avaient pas leur varint de
  comptage (monde noir + fin de paquet désynchronisée) ;
- **respawn** : champ `copyMetadata` manquant.

Tests anti-régression ajoutés : lecture du paquet `map_chunk` complet comme le
client vanilla (framing vérifié à l'octet près), structure exacte de
`login_success`, client Python de validation end-to-end durci (parse NBT +
masques + tableaux 2048 o). 94 tests purs + 100 avec fficheck au vert.

## Nouveauté v0.7.0 — Protocole vanilla 1.20.5, vrais modèles, serveur compatible client Java

### Le multi parle le protocole vanilla Minecraft Java (766)

- **`rustvoxel_server` est désormais joinable par un client Java officiel
  1.20.5/1.20.6** (mode offline, sans chiffrement ni compression — les deux
  supportés nativement par le client vanilla). Handshake → status (ping MOTD
  JSON) → login (UUID offline v3, MD5 maison) → configuration (registres
  dimension_type / biome / chat_type / damage_type en NBT) → play.
- **Chunks au format vanilla** : sections palettées (single-valued / indirecte
  4-8 bits / directe 15 bits), biomes 4x4x4, heightmaps NBT (9 bits), skylight
  par colonne — un client vanilla rend notre monde avec SES textures et SA
  teinture de biomes.
- **Entités vanilla** : spawn/move/teleport/head-rotation/destroy incrémentaux
  par joueur ; nos mobs sont mappés sur les types vanilla (zombie, creeper,
  enderman, etc.) — un client Java officiel voit les mobs avec ses modèles.
- Le client RustVoxel utilise **exactement le même protocole** (solo = serveur
  intégré en loopback) : décodage des chunks palettés, confirm de téléport,
  keepalive, chat système, health/xp, poses via set_creative_slot +
  use_item_on, creusage via player_action.
- Protocole implémenté d'après la spécification publique (wiki.vg /
  minecraft.wiki, données machine PrismarineJS). Aucun code/asset Mojang ;
  le serveur est en mode offline (fonction vanilla).

### Vrais modèles de blocs (blockstates + models JSON du pack)

- `src/models.rs` : lit `blockstates/*.json` + `models/block/*.json` du pack,
  résout les parents et variables `#texture`, cuit les `elements`
  (from/to/faces/uv/**tintindex**/rotation ±45/±22.5) en quads — la géométrie
  affichée devient celle des modèles vanilla (fleurs croisées, torche 2/16,
  dalles, etc.). Sans pack : formes procédurales habituelles.

### Vrais modèles de mobs + textures d'entités du pack

- `src/entity_data.rs` (généré) : rigs boîtes vanilla (bones/cubes/UV par
  face, inflate) pour 14 kinds — zombie (bras levés), creeper, vache, cochon,
  mouton (2 couches), lapin, poule, renard, slime, enderman, abeille, perroquet,
  tortue, dauphin.
- Les **textures d'entités sont des fichiers séparés** dans le pack
  (`textures/entity/zombie/zombie.png`, 64x64, etc.) : le jeu les charge en
  pleine résolution et les coud en un **atlas d'exécution** (comme le jeu de
  référence). Fallback procédural pour les mobs sans texture pack.

## Nouveauté v0.6.2 — Pack vanilla « Default 1.20.5 » validé de bout en bout

Le jeu a été **testé contre un vrai pack de textures vanilla** (11 000
entrées, ~3 900 PNG) téléchargé depuis CurseForge. Résultats et
correctifs engendrés :

- **PNG palette 1/2/4 bits supportés** : les textures vanilla modernes ne
  sont presque toutes plus en 8 bits (`water_still`, `oak_leaves`,
  `birch/acacia/mangrove_leaves`, `kelp_plant`, le feu… sont en palette
  4 bits) — elles étaient rejetées et silencieusement remplacées par le
  fallback procédural. Corrigé + tests (PNG 4/2/1 bits générés avec les
  5 filtres, indices de palette non-scalés, niveaux de gris scalés).
- **Couverture atlas : 145/256 tuiles** prises dans le pack (les 39
  restantes sont du contenu original : peupliers, lits de paille, skins
  de mobs…). Décodage complet du pack en **~180 ms**, atlas en 2 ms.
- **Candidats corrigés contre les vrais noms vanilla** :
  `sweet_berry_bush_stage3` (le nom simple n'existe pas),
  `flowering_azalea_side`, `campfire_log`, `azalea_side`.
- **Tints corrigés contre les vrais modèles vanilla** (lu dans les
  `models/block/*.json` du pack) : acacia/mangrove leaves = teinte
  foliage (grayscale 4 bits) ; **seagrass et kelp ne sont PAS teintés**
  en vanilla moderne (pas de `tintindex`, textures pré-colorées) — nos
  tuiles procédurales sont désormais pré-colorées comme les leurs ;
  sugar_cane reste teintée (`tinted_cross`) mais est pré-colorée comme
  en vanilla.
- Colormaps `grass.png`/`foliage.png`/`dry_foliage.png` (256×256) du
  pack chargées et échantillonnées par climat.
- Planche de revue : `docs/atlas_compare_pack.png` (procédural vs pack +
  démo de teinte biome), `docs/atlas_vanilla_pack.png`,
  `docs/atlas_procedural(_x2).png`.
- Nouveau test (ignoré par défaut, nécessite le zip dans
  `texturepacks/`) : `cargo test --release --features fficheck
  real_vanilla_default_pack -- --ignored --nocapture`.

## Nouveauté v0.6.1 — Corrections d'affichage (capture d'écran)

- **Icônes des blocs réparées** : l'échantillonneur de l'aperçu logiciel
  lisait l'atlas avec un mauvais index (octets par pixel et layout
  ligne-major) — les captures `docs/*.png` montraient des icônes
  arc-en-ciel identiques. Corrigé + tests anti-régression : les icônes de
  l'inventaire doivent être distinctes, les tuiles affichées doivent
  correspondre à l'atlas exact uploadé sur le GPU.
- **Chat replacé au-dessus des cœurs** (empilement vanilla : hotbar → XP
  → cœurs/air → chat) — il écrasait la rangée de santé.
- `docs/ui_{menu,hud,inventory}_preview.png` régénérés **avec le vrai
  atlas** et l'échelle GUI ×2 : ce que vous voyez = ce que le jeu dessine.
- Nouveau test `referenced_tiles_are_defined` : aucun bloc ne peut
  référencer une tuile indéfinie (magenta) de l'atlas.

## Nouveauté v0.6.0 — Multijoueur, UI refaite, serveur dédié

### Le multijoueur (et un vrai serveur « software »)

- **`rustvoxel_server.exe` : serveur dédié headless** (100 % std, aucun GPU,
  tourne sur n'importe quel vieux PC, Linux comme Windows) :
  - monde **autoritaire** à **20 TPS** (blocs, mobs, heure, faim, PV),
  - streaming de chunks compressés (RLE) par joueur, budget par tick,
  - **commandes console** : `/help /list /time /tp /gamemode /kick /say
    /seed /save /stop`,
  - **sauvegarde** du monde (format RVX2 compatible v0.5) à l'arrêt et
    toutes les 3 minutes, options `--port --motd --seed --max --view --world`.
- **Serveur intégré (solo)** : le client lance le même serveur en tâche de
  fond sur `127.0.0.1` et s'y connecte — **le solo passe par le vrai chemin
  réseau**, exactement comme le jeu de référence.
- **Protocole maison** (TCP std::net, frames `[len][opcode][payload]`,
  RLE pour les chunks) : handshake, position 20 Hz, casse/pose de blocs,
  attaques, chat + commandes, snapshots d'entités 10 Hz avec **interpolation
  côté client**, sync de l'heure, santé/faim/XP, status ping pour la liste
  de serveurs.
- **En jeu à plusieurs** : les autres joueurs apparaissent avec un **modèle
  humanoïde articulé** (tête, torse, bras, jambes animés), 8 couleurs de
  chemise dérivées du pseudo, **nametag** au-dessus de la tête, liste
  **Tab** avec ping. Le PvP fonctionne (coup de poing), les dégâts de mobs
  et d'explosions sont appliqués côté serveur.
- **Faim** : se vide en sprintant, se remplit avec la viande (clic droit) ;
  la régénération exige une faim ≥ 9 cœurs, comme le vrai système.

### L'interface, refaite « comme l'originale »

- **Écran-titre** : fond de terre, logo pixel avec splash jaune animé,
  boutons gris biseautés qui bleuissent au survol, version + mention légale.
- **HUD complet** : **hotbar 9 cases** (sélecteur blanc, icônes de blocs en
  **vrai rendu isométrique**), **barre d'XP + niveau**, **cœurs**,
  **faim** (côté droit, inversé), **bulles d'air** sous l'eau, **nom de
  l'objet** affiché au changement de slot, **item tenu** en bas à droite
  avec animation de coup.
- **Chat** (`T` ou `/`) : historique qui s'estompe, couleurs `§`, curseur
  clignotant, commandes serveur.
- **F3** : fps, XYZ, chunk, orientation, biome, heure, entités, chunks,
  PV/faim/XP, serveur + ping, mode de jeu.
- **Inventaire créatif** (`E`) : panneau gris biseauté, catalogue paginé
  (~120 blocs), tooltips, clic pour assigner à la case sélectionnée, molette
  pour la hotbar.
- **Menus** : liste de **serveurs** persistée (`servers.txt`), connexion
  directe, ajout de serveur, écran de **connexion** avec progression du
  téléchargement du terrain, **pause**, **écran de mort** (« Vous êtes
  mort ! ») avec réapparition.
- **Fonte pixel 5×7 maison** avec accents français composés (é, è, ç, à…),
  ombre portée comme il faut, couleurs de chat.

### Toujours là (v0.1 → v0.5)

- Rendu OpenGL 2.0 via FFI Win32 écrite à la main (aucune crate).
- Monde 16×128×16, 16 biomes (dont forêt bigarrée « Wilderness Bound »,
  cerisiers, badlands, jungle, pics gelés), grottes 3D, lush caves,
  dripstone, géodes d'améthyste, deepslate, épaves, camps abandonnés.
- **Coloration par biome** : textures d'herbe/feuilles en **niveaux de
  gris**, couleur appliquée par sommet selon le biome (colormaps des packs
  supportées).
- 18 espèces de mobs (zombies nocturnes, **Siffleur** explosif, slimes,
  ombres téléportantes, basse-courière, aquatiques, calmar lumineux…).
- **Packs de textures au format vanilla** (dossiers ou .zip, chemins
  `assets/minecraft/textures/block/*.png`), décodeur PNG/inflate/ZIP écrit
  à la main ; `F4` recharge à chaud.

## Compiler et lancer

```bash
cargo build --release
# Windows : target/release/rustvoxel.exe (client)
#          target/release/rustvoxel_server.exe (serveur)
```

### Jouer

| Écran | Action |
|---|---|
| Écran-titre | **Jouer en solo** (monde local, sauvegardé à la sortie) ou **Multijoueur** |
| Multijoueur | choisir un serveur de la liste, **Connexion directe…** (`ip:port`), ou l'ajouter |
| Serveur dédié | `rustvoxel_server.exe --port 25565 --motd "Mon serveur"` puis les autres rejoignent avec l'IP |

### Contrôles (AZERTY et QWERTY pris en charge)

| Touche | Action |
|---|---|
| `Z`/`W`, `Q`/`A`, `S`, `D` | se déplacer |
| `Espace` / `Maj` / `Ctrl` | sauter / s'accroupir / sprinter (double rôle en vol) |
| `F` | vol créatif |
| Clic gauche | miner (maintenir) / attaquer |
| Clic droit | poser / manger la viande / lit de paille (passer la nuit) |
| Clic molette | pipette |
| `1-9` / molette | hotbar |
| `E` | inventaire |
| `T` ou `/` | chat / commandes |
| `Tab` | liste des joueurs |
| `F3` / `F4` / `Échap` | debug / recharger les packs / pause |

### Texture packs

Déposez vos packs (dossier ou .zip, layout vanilla
`assets/<ns>/textures/block/*.png` + `pack.mcmeta`) dans `texturepacks/`
puis `F4` en jeu (ou relancez). Le pack vanilla « Default 1.20.5+ » est
validé par test automatisé. PNG palette 1/2/4/8 bits, strips d'animation
(cadrés sur la 1re frame), colormaps 256×256 : tout est géré par le
décodeur maison. `P` (au menu) exporte un modèle prêt à peindre. Les
tuiles en niveaux de gris (herbe, feuillage, eau) sont teintées par
biome au rendu ; les colormaps `grass.png`/`foliage.png` du pack sont
échantillonnées si présentes.

## Qualité / tests

`cargo test` : **82 tests** (logique pure, multiplateforme) et
`cargo test --features fficheck` : **82** (+ FFI Windows type-checkée)
+ 6 ignorés (pack vanilla réel, PNG externes, revues visuelles),
dont :

- protocole : framing TCP (echo sur vraies sockets, paquets fragmentés),
  RLE round-trip, codage des angles ;
- **serveur** : status ping, **2 clients réels** (blocs relayés, chat,
  commandes, départ), serveur intégré auto-stop, place→save→load ;
- packs : ZIP vanilla end-to-end, colormaps, PNG (5 filtres, palette,
  strips) ;
- monde : biomes, grottes lush/dripstone, géodes, arbres déterministes,
  migration RVX1→RVX2.

## Architecture

```
src/
  lib.rs      crate partagée client+serveur
  world.rs    blocs (222), biomes, génération 1.18, colormaps, noms FR
  mesher.rs   mesh par chunk (cubes, croix, dalles, torches, clôtures…)
  mobs.rs     IA + rigs articulés (18 espèces + joueurs) — trait MobTargets
  player.rs   physique AABB partagée (client = prédiction, serveur = mobs)
  net.rs      protocole TCP zéro-dépôt + RLE
  server.rs   serveur autoritaire 20 TPS (dédié + intégré)
  client.rs   client réseau : chunks, ghosts interpolés, santé, chat
  font.rs     fonte pixel 5×7 + accents
  ui.rs       Painter pur (hotbar iso, cœurs, chat, F3, menus…) + rasterizer
  renderer.rs exécution GL du Painter + monde (ciel, fog, eau, particules)
  app.rs      boucle client, machine à états des menus
  win32.rs / gl.rs   FFI Win32 + OpenGL 1.1/2.0 écrites à la main
  bin/rustvoxel_server.rs   serveur dédié headless
```
