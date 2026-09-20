=== UTILISER LES TEXTURES VANILLA DANS RUSTVOXEL ===

1) Prenez un pack de textures au format vanilla (l'archive .zip que vous
   mettriez dans %APPDATA%\.minecraft\resourcepacks, ou un dossier extrait).
2) Déposez-le (sans le renommer) dans CE dossier :

   texturepacks\
     texture-pack-default1.20.5-26.2.zip    <- exemple

3) Lancez le jeu (ou appuyez sur F4 en jeu pour recharger à chaud).

C'est tout. Le jeu scanne au démarrage :
  - texturepacks/<n'importe quoi>.zip   (archive vanilla, format standard)
  - texturepacks/<dossier>/             (pack extrait)
et charge dedans :
  assets/minecraft/textures/block/*.png     -> tuiles des blocs
  assets/minecraft/textures/item/*.png      -> items
  assets/minecraft/textures/entity/*/....png-> peaux des créatures (fichiers séparés)
  assets/minecraft/textures/colormap/*.png  -> couleurs de biomes (teintures)
  assets/minecraft/blockstates/*.json       -> variantes
  assets/minecraft/models/block/*.json      -> VRAIE géométrie (croix, torches, dalles...)

L'atlas est reconstruit À L'EXÉCUTION à partir de ces fichiers individuels
(un pack vanilla ne contient jamais d'atlas).

Sans pack, le jeu utilise ses propres textures procédurales originales.
Vérification : menu Options -> « Packs: ... N texture(s) » (doit afficher
plusieurs milliers avec le pack vanilla par défaut).
