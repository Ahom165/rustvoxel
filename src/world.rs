// Chunked voxel world: storage, terrain generation, block properties.
// v0.4 "Legacy Update": content inspired by the great voxel-game updates
// (stone families, deepslate, copper, amethyst, badlands, savanna, jungle,
// cherry groves, mangrove swamps, shipwrecks...) - all original textures.
use crate::noise::{fbm2, fbm3, hash2i, hash3i, hash01};
use std::collections::HashMap;

pub const CX: usize = 16;
/// v0.5 "Caves & Cliffs": the world doubled in height (1.18 style),
/// mountains now reach y~115 and caves delve under y=14 into deepslate.
pub const CY: usize = 128;
pub const CZ: usize = 16;
pub const WATER_LEVEL: i32 = 26;
pub const SNOW_LINE: i32 = 68;

// Block ids (0..=16 are frozen: they exist in v1 save files)
pub const AIR: u16 = 0;
pub const GRASS: u16 = 1;
pub const DIRT: u16 = 2;
pub const STONE: u16 = 3;
pub const COBBLE: u16 = 4;
pub const PLANKS: u16 = 5;
pub const LOG: u16 = 6;
pub const LEAVES: u16 = 7;
pub const SAND: u16 = 8;
pub const WATER: u16 = 9;
pub const GLASS: u16 = 10;
pub const BRICK: u16 = 11;
pub const SNOW: u16 = 12;
pub const BEDROCK: u16 = 13;
pub const COAL: u16 = 14;
pub const IRON: u16 = 15;
pub const GRAVEL: u16 = 16;
// --- Wilderness Bound additions ---
pub const SANDSTONE: u16 = 17;
pub const BIRCH_LOG: u16 = 18;
pub const BIRCH_LEAVES: u16 = 19;
pub const SPRUCE_LOG: u16 = 20;
pub const SPRUCE_LEAVES: u16 = 21;
pub const CACTUS: u16 = 22;
pub const TALLGRASS: u16 = 23;
pub const FLOWER_RED: u16 = 24;
pub const FLOWER_YELLOW: u16 = 25;
pub const MUSHROOM: u16 = 26;
pub const DEADBUSH: u16 = 27;
pub const PUMPKIN: u16 = 28;
pub const GLOWSTONE: u16 = 29;
pub const WOOL: u16 = 30;
/// Item only (never generated in the world): heals when "eaten".
pub const MEAT: u16 = 31;

// --- v0.3 Wilderness Bound: dappled forest & camps ---
pub const POPLAR_LOG: u16 = 32;
pub const POP_LEAVES_R: u16 = 33;
pub const POP_LEAVES_O: u16 = 34;
pub const POP_LEAVES_Y: u16 = 35;
pub const POPLAR_PLANKS: u16 = 36;
pub const RED_SHRUB: u16 = 37;
pub const SHELF_SM: u16 = 38;
pub const SHELF_LG: u16 = 39;
pub const HAY: u16 = 40;
pub const STRAW_BED: u16 = 41;
pub const CAMPFIRE: u16 = 42;
pub const BARREL: u16 = 43;
pub const GRASS_PALE: u16 = 44;
// colored sets (c = dye color index 0..15, vanilla order white..black)
pub const CONCRETE_BASE: u16 = 48;    // 48..=63
pub const CUSHION_BASE: u16 = 64;     // 64..=79 (no collision)
pub const WOOL_COLOR_BASE: u16 = 80;  // 80..=95 (white = WOOL)
pub const WOOL_SLAB_BASE: u16 = 96;   // 96..=111
pub const WOOL_STAIRS_BASE: u16 = 112; // 112..=127
pub const CONC_SLAB_BASE: u16 = 128;  // 128..=143
pub const CONC_STAIRS_BASE: u16 = 144; // 144..=159

// --- v0.4 "Legacy Update" blocks (1.7..1.21 inspired, original everything) ---
pub const RED_SAND: u16 = 160;
pub const RED_SANDSTONE: u16 = 161;
pub const PODZOL: u16 = 162;
pub const COARSE_DIRT: u16 = 163;
pub const PACKED_ICE: u16 = 164;
pub const GRANITE: u16 = 165;
pub const DIORITE: u16 = 166;
pub const ANDESITE: u16 = 167;
pub const SPONGE: u16 = 168;
pub const MAGMA: u16 = 169;
pub const SEAGRASS: u16 = 170;
pub const KELP: u16 = 171;
pub const CORAL_PINK: u16 = 172;
pub const CORAL_BLUE: u16 = 173;
pub const CORAL_DEAD: u16 = 174;
pub const LILYPAD: u16 = 175;
pub const SUGARCANE: u16 = 176;
pub const BERRY_BUSH: u16 = 177;
pub const BAMBOO: u16 = 178;
pub const BAMBOO_BLOCK: u16 = 179;
pub const HONEY: u16 = 180;
pub const BEEHIVE: u16 = 181;
pub const BASALT: u16 = 182;
pub const BLACKSTONE: u16 = 183;
pub const COPPER_ORE: u16 = 184;
pub const COPPER_BLOCK: u16 = 185;
pub const AMETHYST: u16 = 186;
pub const CALCITE: u16 = 187;
pub const TUFF: u16 = 188;
pub const DEEPSLATE: u16 = 189;
pub const DRIPSTONE: u16 = 190;
pub const MOSS: u16 = 191;
pub const AZALEA: u16 = 192;
pub const AZALEA_FLOWER: u16 = 193;
pub const GLOW_BERRIES: u16 = 194;
pub const MUD: u16 = 195;
pub const PACKED_MUD: u16 = 196;
pub const MUD_BRICKS: u16 = 197;
pub const SCULK: u16 = 198;
pub const MANGROVE_LOG: u16 = 199;
pub const MANGROVE_LEAVES: u16 = 200;
pub const MANGROVE_PLANKS: u16 = 201;
pub const CHERRY_LOG: u16 = 202;
pub const CHERRY_LEAVES: u16 = 203;
pub const CHERRY_PLANKS: u16 = 204;
pub const PINK_PETALS: u16 = 205;
pub const ACACIA_LOG: u16 = 206;
pub const ACACIA_LEAVES: u16 = 207;
pub const ACACIA_PLANKS: u16 = 208;
pub const MELON: u16 = 209;
pub const TERRACOTTA: u16 = 210;
pub const TERRACOTTA_RED: u16 = 211;
pub const TERRACOTTA_ORANGE: u16 = 212;
pub const TERRACOTTA_YELLOW: u16 = 213;
pub const GOLD_ORE: u16 = 214;
pub const DIAMOND_ORE: u16 = 215;
pub const COPPER_BULB: u16 = 216;
pub const OBSIDIAN: u16 = 217;
pub const CRYING_OBSIDIAN: u16 = 218;
pub const TORCH: u16 = 219;
/// Item only: harvested from berry bushes, heals a little.
pub const BERRIES: u16 = 220;
// --- v0.5 "Caves & Cliffs" (1.18) ---
pub const SNOW_LAYER: u16 = 221;      // thin snow carpet (2/16) on cold ground
pub const DRIPSTONE_SPIKE: u16 = 222; // pointed dripstone stalactite/stalagmite
pub const OAK_FENCE: u16 = 223;       // post + arms (real fence model)
pub const MAX_BLOCK_ID: u16 = 223;

// --- Atlas layout (16 x 16 tiles of 16px = 256x256 texture; POT for old GPUs) ---
pub const ATLAS_COLS: u32 = 16;
pub const ATLAS_ROWS: u32 = 16;

// --- Atlas tile indices (single source of truth, used by renderer/pack/hud) ---
pub const T_GRASS_TOP: u32 = 0;
pub const T_GRASS_SIDE: u32 = 1;
pub const T_DIRT: u32 = 2;
pub const T_STONE: u32 = 3;
pub const T_COBBLE: u32 = 4;
pub const T_SAND: u32 = 5;
pub const T_LOG_SIDE: u32 = 6;
pub const T_LOG_TOP: u32 = 7;
pub const T_LEAVES: u32 = 8;
pub const T_PLANKS: u32 = 9;
pub const T_WATER: u32 = 10;
pub const T_GLASS: u32 = 11;
pub const T_BRICK: u32 = 12;
pub const T_SNOW: u32 = 13;
pub const T_BEDROCK: u32 = 14;
pub const T_COAL: u32 = 15;
pub const T_IRON: u32 = 16;
pub const T_GRAVEL: u32 = 17;
pub const T_SANDSTONE: u32 = 18;
pub const T_BIRCH_SIDE: u32 = 19;
pub const T_BIRCH_TOP: u32 = 20;
pub const T_BIRCH_LEAVES: u32 = 21;
pub const T_SPRUCE_SIDE: u32 = 22;
pub const T_SPRUCE_TOP: u32 = 23;
pub const T_SPRUCE_LEAVES: u32 = 24;
pub const T_CACTUS_SIDE: u32 = 25;
pub const T_CACTUS_TOP: u32 = 26;
pub const T_TALLGRASS: u32 = 27;
pub const T_FLOWER_RED: u32 = 28;
pub const T_FLOWER_YELLOW: u32 = 29;
pub const T_MUSHROOM: u32 = 30;
pub const T_DEADBUSH: u32 = 31;
pub const T_PUMPKIN_SIDE: u32 = 32;
pub const T_PUMPKIN_TOP: u32 = 33;
pub const T_GLOWSTONE: u32 = 34;
pub const T_WOOL: u32 = 35;
pub const T_SUN: u32 = 36;
pub const T_MOON: u32 = 37;
pub const T_ZOMBIE_FACE: u32 = 38;
pub const T_ZOMBIE_SKIN: u32 = 39;
pub const T_ZOMBIE_BODY: u32 = 40;
pub const T_ZOMBIE_LIMB: u32 = 41;
pub const T_SIFFLEUR_SKIN: u32 = 42;
pub const T_SIFFLEUR_FACE: u32 = 43;
pub const T_PIG_SKIN: u32 = 44;
pub const T_PIG_FACE: u32 = 45;
pub const T_SHEEP_WOOL: u32 = 46;
pub const T_SHEEP_FACE: u32 = 47;
pub const T_RABBIT_SKIN: u32 = 48;
pub const T_RABBIT_FACE: u32 = 49;
pub const T_MEAT: u32 = 50;
// --- v0.3 tiles ---
pub const T_POPLAR_SIDE: u32 = 51;
pub const T_POPLAR_TOP: u32 = 52;
pub const T_POP_LEAVES_R: u32 = 53;
pub const T_POP_LEAVES_O: u32 = 54;
pub const T_POP_LEAVES_Y: u32 = 55;
pub const T_POPLAR_PLANKS: u32 = 56;
pub const T_RED_SHRUB: u32 = 57;
pub const T_SHELF_SM: u32 = 58;
pub const T_SHELF_LG: u32 = 59;
pub const T_HAY_SIDE: u32 = 60;
pub const T_HAY_TOP: u32 = 61;
pub const T_BED_TOP: u32 = 62;
pub const T_BED_SIDE: u32 = 63;
pub const T_CAMPFIRE: u32 = 64;
pub const T_BARREL_SIDE: u32 = 65;
pub const T_BARREL_TOP: u32 = 66;
pub const T_PALE_GRASS_TOP: u32 = 67;
pub const T_PALE_GRASS_SIDE: u32 = 68;
pub const T_WOOL_COLORS: u32 = 69; // 69..=83 : orange..black (white = T_WOOL)
pub const T_CONCRETE: u32 = 84;    // 84..=99 : white..black
pub const T_CHICKEN_SKIN: u32 = 100;
pub const T_CHICKEN_FACE: u32 = 101;
pub const T_COW_SKIN: u32 = 102;
pub const T_COW_FACE: u32 = 103;
pub const T_FOX_SKIN: u32 = 104;
pub const T_FOX_FACE: u32 = 105;
// --- v0.4 tiles (atlas is 16x16 = 256 tiles now) ---
pub const T_RED_SAND: u32 = 106;
pub const T_RED_SANDSTONE: u32 = 107;
pub const T_PODZOL_TOP: u32 = 108;
pub const T_PODZOL_SIDE: u32 = 109;
pub const T_COARSE_DIRT: u32 = 110;
pub const T_PACKED_ICE: u32 = 111;
pub const T_GRANITE: u32 = 112;
pub const T_DIORITE: u32 = 113;
pub const T_ANDESITE: u32 = 114;
pub const T_SPONGE: u32 = 115;
pub const T_MAGMA: u32 = 116;
pub const T_SEAGRASS: u32 = 117;
pub const T_KELP: u32 = 118;
pub const T_CORAL_PINK: u32 = 119;
pub const T_CORAL_BLUE: u32 = 120;
pub const T_CORAL_DEAD: u32 = 121;
pub const T_LILYPAD: u32 = 122;
pub const T_SUGARCANE: u32 = 123;
pub const T_BERRY_BUSH: u32 = 124;
pub const T_BAMBOO: u32 = 125;
pub const T_BAMBOO_SIDE: u32 = 126;
pub const T_BAMBOO_TOP: u32 = 127;
pub const T_HONEY: u32 = 128;
pub const T_BEEHIVE_SIDE: u32 = 129;
pub const T_BEEHIVE_TOP: u32 = 130;
pub const T_BASALT_SIDE: u32 = 131;
pub const T_BASALT_TOP: u32 = 132;
pub const T_BLACKSTONE: u32 = 133;
pub const T_COPPER_ORE: u32 = 134;
pub const T_COPPER_BLOCK: u32 = 135;
pub const T_AMETHYST: u32 = 136;
pub const T_CALCITE: u32 = 137;
pub const T_TUFF: u32 = 138;
pub const T_DEEPSLATE: u32 = 139;
pub const T_DRIPSTONE: u32 = 140;
pub const T_MOSS: u32 = 141;
pub const T_AZALEA: u32 = 142;
pub const T_AZALEA_FLOWER: u32 = 143;
pub const T_GLOW_BERRIES: u32 = 144;
pub const T_MUD: u32 = 145;
pub const T_PACKED_MUD: u32 = 146;
pub const T_MUD_BRICKS: u32 = 147;
pub const T_SCULK: u32 = 148;
pub const T_MANGROVE_SIDE: u32 = 149;
pub const T_MANGROVE_TOP: u32 = 150;
pub const T_MANGROVE_LEAVES: u32 = 151;
pub const T_MANGROVE_PLANKS: u32 = 152;
pub const T_CHERRY_SIDE: u32 = 153;
pub const T_CHERRY_TOP: u32 = 154;
pub const T_CHERRY_LEAVES: u32 = 155;
pub const T_CHERRY_PLANKS: u32 = 156;
pub const T_PINK_PETALS: u32 = 157;
pub const T_ACACIA_SIDE: u32 = 158;
pub const T_ACACIA_TOP: u32 = 159;
pub const T_ACACIA_LEAVES: u32 = 160;
pub const T_ACACIA_PLANKS: u32 = 161;
pub const T_MELON_SIDE: u32 = 162;
pub const T_MELON_TOP: u32 = 163;
pub const T_TERRACOTTA: u32 = 164;
pub const T_TERRACOTTA_RED: u32 = 165;
pub const T_TERRACOTTA_ORANGE: u32 = 166;
pub const T_TERRACOTTA_YELLOW: u32 = 167;
pub const T_GOLD_ORE: u32 = 168;
pub const T_DIAMOND_ORE: u32 = 169;
pub const T_COPPER_BULB: u32 = 170;
pub const T_OBSIDIAN: u32 = 171;
pub const T_CRYING_OBSIDIAN: u32 = 172;
pub const T_TORCH: u32 = 173;
pub const T_SLIME: u32 = 174;
pub const T_SHADOW_SKIN: u32 = 175;
pub const T_SHADOW_FACE: u32 = 176;
pub const T_BEE_SKIN: u32 = 177;
pub const T_BEE_FACE: u32 = 178;
pub const T_PARROT_SKIN: u32 = 179;
pub const T_PARROT_FACE: u32 = 180;
pub const T_TURTLE_SKIN: u32 = 181;
pub const T_TURTLE_FACE: u32 = 182;
pub const T_DOLPHIN_SKIN: u32 = 183;
pub const T_DOLPHIN_FACE: u32 = 184;
pub const T_GOAT_SKIN: u32 = 185;
pub const T_GOAT_FACE: u32 = 186;
pub const T_FROG_SKIN: u32 = 187;
pub const T_FROG_FACE: u32 = 188;
pub const T_AXOLOTL_SKIN: u32 = 189;
pub const T_AXOLOTL_FACE: u32 = 190;
pub const T_BERRY_ITEM: u32 = 191;
// --- v0.5 tiles ---
pub const T_GRASS_SIDE_OVERLAY: u32 = 192; // grayscale grass fringe (tinted)
pub const T_DRIPSTONE_SPIKE: u32 = 193;
pub const T_GLOW_SQUID_SKIN: u32 = 194;
pub const T_GLOW_SQUID_FACE: u32 = 195;
// --- v0.6 (multijoueur): playable-skin tiles, 8 shirt colors
pub const T_PLAYER_SKIN: u32 = 196;
pub const T_PLAYER_FACE: u32 = 197;
pub const T_PLAYER_SHIRT: u32 = 198; // 198..=205 = 8 shirt colors
pub const T_PLAYER_SHIRTS: u32 = 8;
pub const T_PLAYER_PANTS: u32 = 206;

// Biome ids
pub const B_PLAINS: u8 = 0;
pub const B_FOREST: u8 = 1;
pub const B_BIRCH: u8 = 2;
pub const B_DESERT: u8 = 3;
pub const B_SNOW: u8 = 4;
pub const B_SWAMP: u8 = 5;
pub const B_DAPPLED: u8 = 6;
pub const B_SAVANNA: u8 = 7;
pub const B_BADLANDS: u8 = 8;
pub const B_JUNGLE: u8 = 9;
pub const B_CHERRY: u8 = 10;
pub const B_PEAKS: u8 = 11;
// --- v0.5: 1.18 mountain biomes ---
pub const B_MEADOW: u8 = 12;      // flowery highland grassland
pub const B_GROVE: u8 = 13;       // snowy spruce forest on the flanks
pub const B_SNOWY_SLOPES: u8 = 14; // deep snow under the peaks
pub const B_STONY_PEAKS: u8 = 15; // bare stone summits (warm climate)

pub const BIOME_COUNT: usize = 16;
pub const BIOME_NAMES: [&str; BIOME_COUNT] = [
    "PLAINES", "FORET", "BOULEAUX", "DESERT", "TAIGA", "MARAIS", "FORET BIGARREE",
    "SAVANE", "BADLANDS", "JUNGLE", "VERGER", "PICS DENTELES", "PRAIRIES",
    "BOSQUET", "PENTES ENNEIGEES", "PICS ROCHEUX",
];

// ------------------------------------------------------------------- shapes
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Shape {
    Full,
    Cross,
    Slab,
    Stairs,
    Cushion,
    Bed,
    /// Lily pad: ultra-thin floating disc (1/16).
    Pad,
    /// Real torch model: a thin stick (2/16 wide, 10/16 tall).
    Torch,
    /// Thin snow carpet (2/16) that stacks on cold ground.
    SnowLayer,
    /// Post (4/16 wide) + rails towards solid neighbors.
    Fence,
    /// Inset box (14/16) like vanilla cactus.
    Cactus,
}

pub fn is_cross_id(b: u16) -> bool {
    matches!(
        b,
        TALLGRASS | FLOWER_RED | FLOWER_YELLOW | MUSHROOM | DEADBUSH | RED_SHRUB | SHELF_SM
            | SHELF_LG | CAMPFIRE | SEAGRASS | KELP | SUGARCANE | BERRY_BUSH | BAMBOO | AZALEA
            | AZALEA_FLOWER | GLOW_BERRIES | PINK_PETALS | DRIPSTONE_SPIKE
    )
}

pub fn in_range(b: u16, base: u16, n: u16) -> bool {
    b >= base && b < base + n
}

pub fn is_cushion_id(b: u16) -> bool {
    in_range(b, CUSHION_BASE, 16)
}

pub fn is_slab_id(b: u16) -> bool {
    in_range(b, WOOL_SLAB_BASE, 16) || in_range(b, CONC_SLAB_BASE, 16)
}

pub fn is_stairs_id(b: u16) -> bool {
    in_range(b, WOOL_STAIRS_BASE, 16) || in_range(b, CONC_STAIRS_BASE, 16)
}

pub fn shape_of(b: u16) -> Shape {
    if is_cross_id(b) {
        return Shape::Cross;
    }
    if is_slab_id(b) {
        Shape::Slab
    } else if is_stairs_id(b) {
        Shape::Stairs
    } else if is_cushion_id(b) {
        Shape::Cushion
    } else if b == STRAW_BED {
        Shape::Bed
    } else if b == LILYPAD {
        Shape::Pad
    } else if b == TORCH {
        Shape::Torch
    } else if b == SNOW_LAYER {
        Shape::SnowLayer
    } else if b == OAK_FENCE {
        Shape::Fence
    } else if b == CACTUS {
        Shape::Cactus
    } else {
        Shape::Full
    }
}

/// Dye color index (0..15) of a colored block, if any.
pub fn color_index_of(b: u16) -> Option<u16> {
    for (base, _) in [
        (CONCRETE_BASE, 16u16),
        (CUSHION_BASE, 16),
        (WOOL_COLOR_BASE, 16),
        (WOOL_SLAB_BASE, 16),
        (WOOL_STAIRS_BASE, 16),
        (CONC_SLAB_BASE, 16),
        (CONC_STAIRS_BASE, 16),
    ] {
        if in_range(b, base, 16) {
            return Some(b - base);
        }
    }
    None
}

pub fn is_solid_id(b: u16) -> bool {
    b != AIR
        && b != WATER
        && b != LILYPAD
        && b != SNOW_LAYER
        && !is_cross_id(b)
        && !is_cushion_id(b)
}

/// The stone family blobs (used by 1.18 cave decoration + tests).
pub fn is_stone_fam(b: u16) -> bool {
    matches!(
        b,
        STONE | DEEPSLATE | GRANITE | DIORITE | ANDESITE | TUFF | DRIPSTONE
    )
}

/// What the raycast can aim at (everything breakable except air/water).
pub fn is_targetable(b: u16) -> bool {
    b != AIR && b != WATER
}

pub fn is_opaque_id(b: u16) -> bool {
    if in_range(b, CONCRETE_BASE, 16)
        || in_range(b, WOOL_COLOR_BASE, 16)
        || in_range(b, POP_LEAVES_R, 3)
    {
        return true;
    }
    matches!(
        b,
        GRASS | DIRT
            | STONE
            | COBBLE
            | PLANKS
            | LOG
            | LEAVES
            | SAND
            | BRICK
            | SNOW
            | BEDROCK
            | COAL
            | IRON
            | GRAVEL
            | SANDSTONE
            | BIRCH_LOG
            | BIRCH_LEAVES
            | SPRUCE_LOG
            | SPRUCE_LEAVES
            | PUMPKIN
            | GLOWSTONE
            | WOOL
            | POPLAR_LOG
            | POPLAR_PLANKS
            | HAY
            | BARREL
            | GRASS_PALE
            // v0.4 full blocks
            | RED_SAND | RED_SANDSTONE | PODZOL | COARSE_DIRT | PACKED_ICE | GRANITE | DIORITE
            | ANDESITE | SPONGE | MAGMA | BAMBOO_BLOCK | BEEHIVE | BASALT | BLACKSTONE
            | COPPER_ORE | COPPER_BLOCK | AMETHYST | CALCITE | TUFF | DEEPSLATE | DRIPSTONE
            | MOSS | MUD | PACKED_MUD | MUD_BRICKS | SCULK | MANGROVE_LOG | MANGROVE_LEAVES
            | MANGROVE_PLANKS | CHERRY_LOG | CHERRY_LEAVES | CHERRY_PLANKS | ACACIA_LOG
            | ACACIA_LEAVES | ACACIA_PLANKS | MELON | TERRACOTTA | TERRACOTTA_RED
            | TERRACOTTA_ORANGE | TERRACOTTA_YELLOW | GOLD_ORE | DIAMOND_ORE | COPPER_BULB
            | OBSIDIAN | CRYING_OBSIDIAN | CORAL_PINK | CORAL_BLUE | CORAL_DEAD
    )
    // HONEY is translucent (like GLASS): deliberately not listed.
}

/// Seconds of continuous mining needed to break a block.
pub fn hardness(b: u16) -> f32 {
    match b {
        AIR | WATER => 0.0,
        TALLGRASS | FLOWER_RED | FLOWER_YELLOW | MUSHROOM | DEADBUSH | RED_SHRUB | SHELF_SM
        | SHELF_LG | CAMPFIRE | SEAGRASS | KELP | SUGARCANE | BAMBOO | AZALEA | AZALEA_FLOWER
        | GLOW_BERRIES | PINK_PETALS | TORCH | BERRY_BUSH | LILYPAD | DRIPSTONE_SPIKE => 0.05,
        SNOW_LAYER => 0.12,
        LEAVES | BIRCH_LEAVES | SPRUCE_LEAVES | POP_LEAVES_R | POP_LEAVES_O | POP_LEAVES_Y => 0.25,
        GRASS | DIRT | SAND | SNOW | GRAVEL | GRASS_PALE | RED_SAND | PODZOL | COARSE_DIRT
        | MUD | MOSS => 0.45,
        PUMPKIN | MELON => 0.7,
        HAY | STRAW_BED => 0.5,
        SPONGE | HONEY | MAGMA => 0.5,
        PACKED_ICE => 0.8,
        CORAL_PINK | CORAL_BLUE | CORAL_DEAD => 0.6,
        PLANKS | LOG | BIRCH_LOG | SPRUCE_LOG | WOOL | POPLAR_LOG | POPLAR_PLANKS | BAMBOO_BLOCK
        | MANGROVE_LOG | CHERRY_LOG | ACACIA_LOG | MANGROVE_PLANKS | CHERRY_PLANKS
        | ACACIA_PLANKS | BEEHIVE | OAK_FENCE => 0.85,
        BARREL => 0.9,
        COBBLE | SANDSTONE | RED_SANDSTONE | BRICK | GLASS | PACKED_MUD | MUD_BRICKS => 1.0,
        TERRACOTTA | TERRACOTTA_RED | TERRACOTTA_ORANGE | TERRACOTTA_YELLOW => 1.25,
        STONE => 1.1,
        GRANITE | DIORITE | ANDESITE | TUFF | CALCITE | DRIPSTONE | SCULK => 1.3,
        COAL => 1.4,
        AMETHYST => 1.5,
        COPPER_ORE | COPPER_BLOCK | COPPER_BULB => 1.8,
        IRON => 1.7,
        GOLD_ORE | DIAMOND_ORE => 2.0,
        DEEPSLATE | BLACKSTONE => 2.2,
        OBSIDIAN | CRYING_OBSIDIAN => 3.5,
        GLOWSTONE => 0.5,
        BEDROCK => f32::INFINITY,
        _ => {
            if in_range(b, CUSHION_BASE, 16) {
                0.4
            } else if in_range(b, WOOL_COLOR_BASE, 16)
                || is_slab_id(b)
                || is_stairs_id(b)
            {
                0.8
            } else if in_range(b, CONCRETE_BASE, 16) {
                1.3
            } else {
                1.0
            }
        }
    }
}

/// Atlas tile per face: [top, side, bottom].
pub fn tiles_of(b: u16) -> [u32; 3] {
    match b {
        GRASS => [0, 1, 2],
        DIRT => [2, 2, 2],
        STONE => [3, 3, 3],
        COBBLE => [4, 4, 4],
        SAND => [5, 5, 5],
        LOG => [7, 6, 7],
        LEAVES => [8, 8, 8],
        PLANKS => [9, 9, 9],
        WATER => [10, 10, 10],
        GLASS => [11, 11, 11],
        BRICK => [12, 12, 12],
        SNOW => [13, 13, 13],
        BEDROCK => [14, 14, 14],
        COAL => [15, 15, 15],
        IRON => [16, 16, 16],
        GRAVEL => [17, 17, 17],
        SANDSTONE => [18, 18, 18],
        BIRCH_LOG => [20, 19, 20],
        BIRCH_LEAVES => [21, 21, 21],
        SPRUCE_LOG => [23, 22, 23],
        SPRUCE_LEAVES => [24, 24, 24],
        CACTUS => [26, 25, 26],
        TALLGRASS => [27, 27, 27],
        FLOWER_RED => [28, 28, 28],
        FLOWER_YELLOW => [29, 29, 29],
        MUSHROOM => [30, 30, 30],
        DEADBUSH => [31, 31, 31],
        PUMPKIN => [33, 32, 33],
        GLOWSTONE => [34, 34, 34],
        WOOL => [35, 35, 35],
        MEAT => [50, 50, 50],
        // --- v0.3 ---
        GRASS_PALE => [T_PALE_GRASS_TOP, T_PALE_GRASS_SIDE, T_DIRT],
        POPLAR_LOG => [T_POPLAR_TOP, T_POPLAR_SIDE, T_POPLAR_TOP],
        POP_LEAVES_R => [T_POP_LEAVES_R, T_POP_LEAVES_R, T_POP_LEAVES_R],
        POP_LEAVES_O => [T_POP_LEAVES_O, T_POP_LEAVES_O, T_POP_LEAVES_O],
        POP_LEAVES_Y => [T_POP_LEAVES_Y, T_POP_LEAVES_Y, T_POP_LEAVES_Y],
        POPLAR_PLANKS => [T_POPLAR_PLANKS, T_POPLAR_PLANKS, T_POPLAR_PLANKS],
        RED_SHRUB => [T_RED_SHRUB, T_RED_SHRUB, T_RED_SHRUB],
        SHELF_SM => [T_SHELF_SM, T_SHELF_SM, T_SHELF_SM],
        SHELF_LG => [T_SHELF_LG, T_SHELF_LG, T_SHELF_LG],
        HAY => [T_HAY_TOP, T_HAY_SIDE, T_HAY_TOP],
        STRAW_BED => [T_BED_TOP, T_BED_SIDE, T_HAY_SIDE],
        CAMPFIRE => [T_CAMPFIRE, T_CAMPFIRE, T_CAMPFIRE],
        BARREL => [T_BARREL_TOP, T_BARREL_SIDE, T_BARREL_TOP],
        // --- v0.4 ---
        RED_SAND => [T_RED_SAND, T_RED_SAND, T_RED_SAND],
        RED_SANDSTONE => [T_RED_SANDSTONE, T_RED_SANDSTONE, T_RED_SANDSTONE],
        PODZOL => [T_PODZOL_TOP, T_PODZOL_SIDE, T_DIRT],
        COARSE_DIRT => [T_COARSE_DIRT, T_COARSE_DIRT, T_COARSE_DIRT],
        PACKED_ICE => [T_PACKED_ICE, T_PACKED_ICE, T_PACKED_ICE],
        GRANITE => [T_GRANITE, T_GRANITE, T_GRANITE],
        DIORITE => [T_DIORITE, T_DIORITE, T_DIORITE],
        ANDESITE => [T_ANDESITE, T_ANDESITE, T_ANDESITE],
        SPONGE => [T_SPONGE, T_SPONGE, T_SPONGE],
        MAGMA => [T_MAGMA, T_MAGMA, T_MAGMA],
        SEAGRASS => [T_SEAGRASS, T_SEAGRASS, T_SEAGRASS],
        KELP => [T_KELP, T_KELP, T_KELP],
        CORAL_PINK => [T_CORAL_PINK, T_CORAL_PINK, T_CORAL_PINK],
        CORAL_BLUE => [T_CORAL_BLUE, T_CORAL_BLUE, T_CORAL_BLUE],
        CORAL_DEAD => [T_CORAL_DEAD, T_CORAL_DEAD, T_CORAL_DEAD],
        LILYPAD => [T_LILYPAD, T_LILYPAD, T_LILYPAD],
        SUGARCANE => [T_SUGARCANE, T_SUGARCANE, T_SUGARCANE],
        BERRY_BUSH => [T_BERRY_BUSH, T_BERRY_BUSH, T_BERRY_BUSH],
        BAMBOO => [T_BAMBOO, T_BAMBOO, T_BAMBOO],
        BAMBOO_BLOCK => [T_BAMBOO_TOP, T_BAMBOO_SIDE, T_BAMBOO_TOP],
        HONEY => [T_HONEY, T_HONEY, T_HONEY],
        BEEHIVE => [T_BEEHIVE_TOP, T_BEEHIVE_SIDE, T_BEEHIVE_TOP],
        BASALT => [T_BASALT_TOP, T_BASALT_SIDE, T_BASALT_TOP],
        BLACKSTONE => [T_BLACKSTONE, T_BLACKSTONE, T_BLACKSTONE],
        COPPER_ORE => [T_COPPER_ORE, T_COPPER_ORE, T_COPPER_ORE],
        COPPER_BLOCK => [T_COPPER_BLOCK, T_COPPER_BLOCK, T_COPPER_BLOCK],
        AMETHYST => [T_AMETHYST, T_AMETHYST, T_AMETHYST],
        CALCITE => [T_CALCITE, T_CALCITE, T_CALCITE],
        TUFF => [T_TUFF, T_TUFF, T_TUFF],
        DEEPSLATE => [T_DEEPSLATE, T_DEEPSLATE, T_DEEPSLATE],
        DRIPSTONE => [T_DRIPSTONE, T_DRIPSTONE, T_DRIPSTONE],
        MOSS => [T_MOSS, T_MOSS, T_MOSS],
        AZALEA => [T_AZALEA, T_AZALEA, T_AZALEA],
        AZALEA_FLOWER => [T_AZALEA_FLOWER, T_AZALEA_FLOWER, T_AZALEA_FLOWER],
        GLOW_BERRIES => [T_GLOW_BERRIES, T_GLOW_BERRIES, T_GLOW_BERRIES],
        MUD => [T_MUD, T_MUD, T_MUD],
        PACKED_MUD => [T_PACKED_MUD, T_PACKED_MUD, T_PACKED_MUD],
        MUD_BRICKS => [T_MUD_BRICKS, T_MUD_BRICKS, T_MUD_BRICKS],
        SCULK => [T_SCULK, T_SCULK, T_SCULK],
        MANGROVE_LOG => [T_MANGROVE_TOP, T_MANGROVE_SIDE, T_MANGROVE_TOP],
        MANGROVE_LEAVES => [T_MANGROVE_LEAVES, T_MANGROVE_LEAVES, T_MANGROVE_LEAVES],
        MANGROVE_PLANKS => [T_MANGROVE_PLANKS, T_MANGROVE_PLANKS, T_MANGROVE_PLANKS],
        CHERRY_LOG => [T_CHERRY_TOP, T_CHERRY_SIDE, T_CHERRY_TOP],
        CHERRY_LEAVES => [T_CHERRY_LEAVES, T_CHERRY_LEAVES, T_CHERRY_LEAVES],
        CHERRY_PLANKS => [T_CHERRY_PLANKS, T_CHERRY_PLANKS, T_CHERRY_PLANKS],
        PINK_PETALS => [T_PINK_PETALS, T_PINK_PETALS, T_PINK_PETALS],
        ACACIA_LOG => [T_ACACIA_TOP, T_ACACIA_SIDE, T_ACACIA_TOP],
        ACACIA_LEAVES => [T_ACACIA_LEAVES, T_ACACIA_LEAVES, T_ACACIA_LEAVES],
        ACACIA_PLANKS => [T_ACACIA_PLANKS, T_ACACIA_PLANKS, T_ACACIA_PLANKS],
        MELON => [T_MELON_TOP, T_MELON_SIDE, T_MELON_TOP],
        TERRACOTTA => [T_TERRACOTTA, T_TERRACOTTA, T_TERRACOTTA],
        TERRACOTTA_RED => [T_TERRACOTTA_RED, T_TERRACOTTA_RED, T_TERRACOTTA_RED],
        TERRACOTTA_ORANGE => [T_TERRACOTTA_ORANGE, T_TERRACOTTA_ORANGE, T_TERRACOTTA_ORANGE],
        TERRACOTTA_YELLOW => [T_TERRACOTTA_YELLOW, T_TERRACOTTA_YELLOW, T_TERRACOTTA_YELLOW],
        GOLD_ORE => [T_GOLD_ORE, T_GOLD_ORE, T_GOLD_ORE],
        DIAMOND_ORE => [T_DIAMOND_ORE, T_DIAMOND_ORE, T_DIAMOND_ORE],
        COPPER_BULB => [T_COPPER_BULB, T_COPPER_BULB, T_COPPER_BULB],
        OBSIDIAN => [T_OBSIDIAN, T_OBSIDIAN, T_OBSIDIAN],
        CRYING_OBSIDIAN => [T_CRYING_OBSIDIAN, T_CRYING_OBSIDIAN, T_CRYING_OBSIDIAN],
        TORCH => [T_TORCH, T_TORCH, T_TORCH],
        SNOW_LAYER => [T_SNOW, T_SNOW, T_SNOW],
        DRIPSTONE_SPIKE => [T_DRIPSTONE_SPIKE, T_DRIPSTONE_SPIKE, T_DRIPSTONE_SPIKE],
        OAK_FENCE => [T_PLANKS, T_PLANKS, T_PLANKS],
        BERRIES => [T_BERRY_ITEM, T_BERRY_ITEM, T_BERRY_ITEM],
        _ => {
            // colored sets reuse wool/concrete tiles
            if let Some(c) = color_index_of(b) {
                let wt = if c == 0 { T_WOOL } else { T_WOOL_COLORS + c as u32 - 1 };
                let ct = T_CONCRETE + c as u32;
                let t = if in_range(b, CONCRETE_BASE, 16)
                    || is_stairs_id(b) && in_range(b, CONC_STAIRS_BASE, 16)
                    || is_slab_id(b) && in_range(b, CONC_SLAB_BASE, 16)
                {
                    ct
                } else {
                    wt
                };
                [t, t, t]
            } else {
                [0, 0, 0]
            }
        }
    }
}

// ------------------------------------------------------------- biome colors
// The reference-game coloring system: grass, leaves and water textures are
// stored GRAYSCALE and multiplied at mesh time by a biome-dependent color.
// Our procedural tiles follow the same rule, and texture packs may override
// them with the vanilla grayscale PNGs - plus optionally the vanilla
// colormap files (assets/minecraft/textures/colormap/{grass,foliage}.png).

/// How a tile picks its final color.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TintKind {
    None,
    /// Grass colormap: grass top, grass fringe overlay, tall grass, cane,
    /// lily pads.
    Grass,
    /// Foliage colormap: oak/acacia/mangrove leaves (birch/spruce fixed).
    Foliage,
    /// Fixed vanilla birch leaf tint.
    BirchLeaves,
    /// Fixed vanilla spruce leaf tint.
    SpruceLeaves,
    /// Water color per biome.
    Water,
}

pub fn tint_kind(t: u32) -> TintKind {
    match t {
        T_GRASS_TOP | T_GRASS_SIDE_OVERLAY | T_TALLGRASS | T_SUGARCANE | T_LILYPAD => {
            TintKind::Grass
        }
        // NOTE: seagrass and kelp are NOT tinted by modern vanilla (no
        // tintindex in their models, textures are pre-colored) - verified
        // against a real resource pack's model JSONs.
        T_LEAVES | T_ACACIA_LEAVES | T_MANGROVE_LEAVES => TintKind::Foliage,
        T_BIRCH_LEAVES => TintKind::BirchLeaves,
        T_SPRUCE_LEAVES => TintKind::SpruceLeaves,
        T_WATER => TintKind::Water,
        _ => TintKind::None,
    }
}

/// Fixed tint used for HUD icons and break particles (biome-agnostic).
pub fn icon_tint(t: u32) -> [f32; 3] {
    match tint_kind(t) {
        TintKind::None => [1.0, 1.0, 1.0],
        TintKind::Grass => c3(0x91BD59),
        TintKind::Foliage => c3(0x79C05A),
        TintKind::BirchLeaves => c3(0x80A755),
        TintKind::SpruceLeaves => c3(0x619961),
        TintKind::Water => c3(0x3F76E4),
    }
}

/// Rough climate of every biome (temperature, downfall), used to sample the
/// vanilla colormap PNGs exactly like the reference game does.
pub fn biome_climate(b: u8) -> (f32, f32) {
    match b {
        B_PLAINS => (0.8, 0.4),
        B_FOREST => (0.7, 0.5),
        B_BIRCH => (0.6, 0.6),
        B_DESERT => (1.0, 0.0),
        B_SNOW => (0.0, 0.5),
        B_SWAMP => (0.8, 0.9),
        B_DAPPLED => (0.35, 0.55),
        B_SAVANNA => (1.0, 0.0),
        B_BADLANDS => (1.0, 0.0),
        B_JUNGLE => (0.95, 0.9),
        B_CHERRY => (0.5, 0.5),
        B_PEAKS => (0.2, 0.3),
        B_MEADOW => (0.5, 0.6),
        B_GROVE => (0.25, 0.8),
        B_SNOWY_SLOPES => (0.1, 0.5),
        B_STONY_PEAKS => (1.0, 0.3),
        _ => (0.5, 0.5),
    }
}

const fn c3(hex: u32) -> [f32; 3] {
    [
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
    ]
}

/// Per-biome tint colors. `vanilla()` holds the hand-picked colors of the
/// reference game; `from_pack()` re-samples them from a pack's colormaps.
pub struct BiomePalette {
    pub grass: [[f32; 3]; BIOME_COUNT],
    pub foliage: [[f32; 3]; BIOME_COUNT],
    pub water: [[f32; 3]; BIOME_COUNT],
}

impl BiomePalette {
    pub fn vanilla() -> BiomePalette {
        let grass = [
            c3(0x91BD59), // plains
            c3(0x79C05A), // forest
            c3(0x88BB67), // birch forest
            c3(0xBFB755), // desert
            c3(0x80B497), // snowy taiga
            c3(0x6A7039), // swamp
            c3(0xA3BC6E), // dappled (pale)
            c3(0xBFB755), // savanna
            c3(0x90814D), // badlands
            c3(0x59C93C), // jungle
            c3(0xB6DB61), // cherry grove
            c3(0x80B497), // jagged peaks
            c3(0x83BB6D), // meadow
            c3(0x86B783), // grove
            c3(0x80B497), // snowy slopes
            c3(0x9ABE4B), // stony peaks
        ];
        let mut foliage = grass;
        foliage[B_BIRCH as usize] = c3(0x80A755);
        let mut water = [c3(0x3F76E4); BIOME_COUNT];
        water[B_SWAMP as usize] = c3(0x617B64);
        water[B_BADLANDS as usize] = c3(0x4E7F81);
        BiomePalette {
            grass,
            foliage,
            water,
        }
    }

    /// Sample the pack's colormaps (grass.png / foliage.png indexed by
    /// climate exactly like the reference game: u = 1-temp, v = 1-downfall*temp).
    /// Swamp & badlands keep their fixed overrides, as in the reference game.
    pub fn from_pack(p: &crate::pack::Pack) -> BiomePalette {
        let mut pal = BiomePalette::vanilla();
        let g = p.colormap_grass.as_ref();
        let f = p.colormap_foliage.as_ref();
        if g.is_none() && f.is_none() {
            return pal;
        }
        for b in 0u8..BIOME_COUNT as u8 {
            if b == B_SWAMP || b == B_BADLANDS {
                continue;
            }
            let (t, d) = biome_climate(b);
            let t = t.clamp(0.0, 1.0);
            let dd = (d * t).clamp(0.0, 1.0);
            let x = ((1.0 - t) * 255.0) as u32;
            let y = ((1.0 - dd) * 255.0) as u32;
            if let Some(img) = g {
                pal.grass[b as usize] = sample_cmap(img, x, y);
            }
            if let Some(img) = f {
                pal.foliage[b as usize] = sample_cmap(img, x, y);
            }
        }
        pal
    }
}

fn sample_cmap(img: &crate::pack::Rgba, x: u32, y: u32) -> [f32; 3] {
    let x = (x.min(img.w.saturating_sub(1))) as usize;
    let y = (y.min(img.h.saturating_sub(1))) as usize;
    let o = (y * img.w as usize + x) * 4;
    [
        img.px[o] as f32 / 255.0,
        img.px[o + 1] as f32 / 255.0,
        img.px[o + 2] as f32 / 255.0,
    ]
}

#[derive(Clone)]
pub struct Chunk {
    pub blocks: Vec<u16>, // CX*CY*CZ
    pub modified: bool,
}

impl Chunk {
    pub fn new() -> Chunk {
        Chunk {
            blocks: vec![AIR; CX * CY * CZ],
            modified: false,
        }
    }
    #[inline]
    pub fn get(&self, lx: usize, y: usize, lz: usize) -> u16 {
        self.blocks[lx + CX * (lz + CZ * y)]
    }
    #[inline]
    pub fn set(&mut self, lx: usize, y: usize, lz: usize, b: u16) {
        self.blocks[lx + CX * (lz + CZ * y)] = b;
    }
}

pub struct World {
    pub seed: u64,
    pub chunks: HashMap<(i32, i32), Chunk>,
}

#[inline]
fn idx_of(x: i32, _y: i32, z: i32) -> (i32, i32, usize, usize) {
    let cx = x.div_euclid(CX as i32);
    let cz = z.div_euclid(CZ as i32);
    let lx = x.rem_euclid(CX as i32) as usize;
    let lz = z.rem_euclid(CZ as i32) as usize;
    (cx, cz, lx, lz)
}

impl World {
    pub fn new(seed: u64) -> World {
        World {
            seed,
            chunks: HashMap::new(),
        }
    }

    pub fn has_chunk(&self, c: (i32, i32)) -> bool {
        self.chunks.contains_key(&c)
    }

    /// Terrain height (pure function of coordinates + seed).
    pub fn height_at(&self, x: i32, z: i32) -> i32 {
        let fx = x as f32;
        let fz = z as f32;
        let base = fbm2(fx * 0.0075, fz * 0.0075, self.seed, 4);
        let detail = fbm2(fx * 0.045, fz * 0.045, self.seed ^ 0xB00B, 2);
        let mut h = 22.0 + base * base * 30.0 + detail * 4.0 - 2.0;
        // 1.18-style mountain ranges: a low-frequency mask lifts whole areas
        // above the snow line, with a ridged detail on top.
        let mtn = fbm2(fx * 0.0035, fz * 0.0035, self.seed ^ 0x9EA7, 3);
        if mtn > 0.56 {
            // v0.5: 1.18 tall ranges - the same mask now climbs to y~115
            let rise = (mtn - 0.56) * 240.0;
            let ridge = fbm2(fx * 0.02, fz * 0.02, self.seed ^ 0x51D5, 2);
            h += rise * (0.55 + 0.45 * ridge);
        }
        (h as i32).clamp(2, CY as i32 - 8)
    }

    /// Temperature field in [0, 1).
    fn temp_at(&self, x: i32, z: i32) -> f32 {
        fbm2(x as f32 * 0.0042, z as f32 * 0.0042, self.seed ^ 0x7E3B_1A, 3)
    }

    /// Moisture field in [0, 1).
    fn moist_at(&self, x: i32, z: i32) -> f32 {
        fbm2(x as f32 * 0.0051, z as f32 * 0.0051, self.seed ^ 0x51A7, 3)
    }

    pub fn biome_at(&self, x: i32, z: i32) -> u8 {
        let t = self.temp_at(x, z);
        let m = self.moist_at(x, z);
        let h = self.height_at(x, z);
        // --- v0.5: 1.18 altitude ladder -------------------------------
        // Jagged/frozen peaks on the summits, stony peaks where it is hot,
        // deep snow on the slopes, spruce groves and flowery meadows below.
        if h >= 86 {
            return if t > 0.62 { B_STONY_PEAKS } else { B_PEAKS };
        }
        if h >= 72 {
            return B_SNOWY_SLOPES;
        }
        if h >= 58 {
            return if t > 0.45 { B_MEADOW } else { B_GROVE };
        }
        if t < 0.30 {
            B_SNOW
        } else if t < 0.38 && m > 0.50 {
            // Wilderness Bound: dappled forests grow right next to cold biomes
            B_DAPPLED
        } else if t > 0.70 && m < 0.34 {
            // scorched mesas with terracotta strata
            B_BADLANDS
        } else if t > 0.58 && m > 0.70 {
            // dense hot jungle
            B_JUNGLE
        } else if t > 0.60 && m < 0.44 {
            B_DESERT
        } else if (0.52..0.58).contains(&t) && m > 0.46 {
            // mild cherry valleys between forest and desert climates
            B_CHERRY
        } else if t > 0.60 && m < 0.56 {
            // dry grassland dotted with acacias
            B_SAVANNA
        } else if m > 0.66 {
            if t > 0.45 && h <= WATER_LEVEL + 3 {
                B_SWAMP
            } else {
                B_FOREST
            }
        } else if m > 0.52 {
            B_FOREST
        } else if m > 0.42 {
            B_BIRCH
        } else {
            B_PLAINS
        }
    }

    fn cave_at(&self, x: i32, y: i32, z: i32) -> bool {
        let n = fbm3(
            x as f32 * 0.07,
            y as f32 * 0.11,
            z as f32 * 0.07,
            self.seed ^ 0xCA7E_1234,
            2,
        );
        n > 0.76
    }

    /// Generate and insert a chunk (no-op if already present).
    pub fn gen_chunk(&mut self, cx: i32, cz: i32) {
        if self.chunks.contains_key(&(cx, cz)) {
            return;
        }
        let mut ch = Chunk::new();
        let x0 = cx * CX as i32;
        let z0 = cz * CZ as i32;

        for lx in 0..CX {
            for lz in 0..CZ {
                let wx = x0 + lx as i32;
                let wz = z0 + lz as i32;
                let biome = self.biome_at(wx, wz);
                let mut h = self.height_at(wx, wz);
                if biome == B_SWAMP {
                    // flatten towards sea level so shallow pools appear
                    let d = fbm2(wx as f32 * 0.05, wz as f32 * 0.05, self.seed ^ 0x5EA9, 2);
                    h = WATER_LEVEL - 1 + if d > 0.58 { 2 } else if d > 0.35 { 1 } else { 0 };
                    h = h.min(WATER_LEVEL + 1);
                }
                let beach = h <= WATER_LEVEL + 1;
                let snowy = h >= SNOW_LINE;
                let desert = biome == B_DESERT;
                let badlands = biome == B_BADLANDS;
                // v0.5: thin snow carpets on taiga/grove/meadow ground
                let snow_layer_here = h > WATER_LEVEL
                    && !desert
                    && !badlands
                    && (biome == B_SNOW
                        || biome == B_GROVE
                        || (biome == B_MEADOW && h >= 66));
                for y in 0..CY as i32 {
                    let mut b = if y == 0 {
                        BEDROCK
                    } else if y < h - 3 {
                        STONE
                    } else if y < h {
                        // subsurface layers
                        if badlands {
                            // 1.7 mesas: colored terracotta strata + red sandstone
                            let layer = h - y;
                            if layer <= 3 {
                                let st =
                                    hash3i(wx as i64, y as i64, wz as i64, self.seed ^ 0x57AA) % 4;
                                match st {
                                    0 => TERRACOTTA_RED,
                                    1 => TERRACOTTA_ORANGE,
                                    2 => TERRACOTTA_YELLOW,
                                    _ => TERRACOTTA,
                                }
                            } else if layer <= 6 {
                                RED_SANDSTONE
                            } else {
                                DIRT
                            }
                        } else if desert {
                            SAND
                        } else {
                            DIRT
                        }
                    } else if y == h {
                        if badlands {
                            RED_SAND
                        } else if beach {
                            if h < WATER_LEVEL - 5 {
                                // deep sea floor: gravel with rare magma patches
                                let mr = hash01(hash3i(
                                    wx as i64,
                                    h as i64,
                                    wz as i64,
                                    self.seed ^ 0x4A6A,
                                )) * 256.0;
                                if mr > 249.0 {
                                    MAGMA
                                } else {
                                    GRAVEL
                                }
                            } else {
                                SAND
                            }
                        } else if desert {
                            SAND
                        } else if biome == B_STONY_PEAKS {
                            // v0.5: 1.18 stony peaks - bare stone summits
                            let pr = hash2i(wx as i64, wz as i64, self.seed ^ 0x9EA7_3);
                            if pr % 5 == 0 {
                                GRAVEL
                            } else if pr % 23 == 7 {
                                SNOW
                            } else {
                                STONE
                            }
                        } else if snowy {
                            SNOW
                        } else if biome == B_PEAKS {
                            let pr = hash2i(wx as i64, wz as i64, self.seed ^ 0x9EA7_2);
                            if pr % 7 == 0 {
                                STONE
                            } else if pr % 41 == 5 {
                                PACKED_ICE
                            } else {
                                SNOW
                            }
                        } else if biome == B_SWAMP && h < WATER_LEVEL {
                            // 1.19 mangrove swamps: mud on the swamp bottom
                            MUD
                        } else if biome == B_DAPPLED {
                            GRASS_PALE
                        } else {
                            GRASS
                        }
                    } else if y == h + 1 && snow_layer_here {
                        // 1.18: walkable snow carpets on cold ground
                        SNOW_LAYER
                    } else if y <= WATER_LEVEL {
                        WATER
                    } else {
                        AIR
                    };

                    if b == STONE {
                        // 1.17/1.18: deepslate takes over below y=14
                        if y < 14 {
                            b = DEEPSLATE;
                        } else {
                            // 1.8 stone family blobs + 1.17 tuff / dripstone
                            let r2 = hash01(hash3i(
                                wx as i64,
                                y as i64,
                                wz as i64,
                                self.seed ^ 0xB10B5,
                            )) * 256.0;
                            if r2 > 243.0 {
                                b = GRANITE;
                            } else if r2 > 236.0 {
                                b = DIORITE;
                            } else if r2 > 229.0 {
                                b = ANDESITE;
                            } else if r2 > 226.0 && y < 34 {
                                b = DRIPSTONE;
                            } else if r2 > 223.0 && y < 30 {
                                b = TUFF;
                            }
                        }
                    }
                    if b == STONE || b == DEEPSLATE {
                        // Ores + gravel pockets + buried glowstone (deterministic per block).
                        let r = hash01(hash3i(
                            wx as i64,
                            y as i64,
                            wz as i64,
                            self.seed ^ 0x05E5_A1,
                        )) * 256.0;
                        if r < 9.0 && y < 40 {
                            b = COAL;
                        } else if r < 13.0 && y < 24 {
                            b = IRON;
                        } else if r < 15.5 && y < 44 {
                            // 1.17 copper
                            b = COPPER_ORE;
                        } else if r < 17.0 && y < 16 {
                            b = GOLD_ORE;
                        } else if r < 18.0 && y < 12 {
                            b = DIAMOND_ORE;
                        } else if r > 246.0 && r < 249.0 && y < 20 {
                            b = GLOWSTONE;
                        } else if r > 249.0 && r < 251.0 {
                            b = GRAVEL;
                        } else if r > 251.0 && r < 252.0 && y < 18 {
                            b = BLACKSTONE;
                        } else if r > 252.0 && r < 253.0 && y < 18 {
                            b = OBSIDIAN;
                        } else if r > 253.0 && y < 14 {
                            // 1.19 deep dark ambience
                            b = SCULK;
                        }
                        // Caves
                        if y > 3 && y < h - 3 && self.cave_at(wx, y, wz) {
                            b = AIR;
                        }
                        // 1.17 glow berries dangle under cave ceilings
                        if b == AIR && y < 32 && y + 1 < h - 3 && !self.cave_at(wx, y + 1, wz) {
                            let r3 = hash01(hash3i(
                                wx as i64,
                                y as i64,
                                wz as i64,
                                self.seed ^ 0xBE33,
                            )) * 256.0;
                            if r3 > 253.5 {
                                b = GLOW_BERRIES;
                            }
                        }
                    }
                    ch.set(lx, y as usize, lz, b);
                }
            }
        }

        // helper: write into this chunk only, with bounds checks
        fn put(
            ch: &mut Chunk,
            x0: i32,
            z0: i32,
            x: i32,
            y: i32,
            z: i32,
            b: u16,
            only_air: bool,
        ) {
            let lx = x - x0;
            let lz = z - z0;
            if lx >= 0 && lx < CX as i32 && lz >= 0 && lz < CZ as i32 {
                let y = y as usize;
                if y < CY {
                    let cur = ch.get(lx as usize, y, lz as usize);
                    if !only_air || cur == AIR {
                        ch.set(lx as usize, y, lz as usize, b);
                    }
                }
            }
        }

        // Trees: deterministic per world column; scan a margin so canopies
        // crossing the border are consistent in every chunk.
        for tx in -3..(CX as i32 + 3) {
            for tz in -3..(CZ as i32 + 3) {
                let wx = x0 + tx;
                let wz = z0 + tz;
                let biome = self.biome_at(wx, wz);
                let r = hash2i(wx as i64, wz as i64, self.seed ^ 0x7EE3);
                let h = self.height_at(wx, wz);
                if h <= WATER_LEVEL + 1 || h >= CY as i32 - 8 {
                    continue;
                }
                let snowy_here = biome == B_SNOW || h >= SNOW_LINE;
                // tree kind + rarity per biome
                let kind: u8 = match biome {
                    B_FOREST => {
                        if r % 59 == 0 {
                            0
                        } else if r % 61 == 3 {
                            1
                        } else {
                            continue;
                        }
                    }
                    B_BIRCH => {
                        if r % 43 == 0 {
                            1
                        } else {
                            continue;
                        }
                    }
                    B_PLAINS => {
                        if r % 263 == 0 {
                            0
                        } else {
                            continue;
                        }
                    }
                    B_DAPPLED => {
                        if r % 37 == 0 {
                            3 // standing poplar
                        } else if r % 53 == 5 {
                            4 // fallen poplar log
                        } else {
                            continue
                        }
                    }
                    B_SAVANNA => {
                        if r % 97 == 0 {
                            5 // acacia
                        } else {
                            continue
                        }
                    }
                    B_JUNGLE => {
                        if r % 23 == 0 {
                            6 // tall jungle tree
                        } else {
                            continue
                        }
                    }
                    B_CHERRY => {
                        if r % 41 == 0 {
                            7 // cherry blossom
                        } else {
                            continue
                        }
                    }
                    B_SWAMP => {
                        if r % 89 == 0 {
                            8 // mangrove (1.19)
                        } else if r % 71 == 0 {
                            0
                        } else {
                            continue;
                        }
                    }
                    B_GROVE => {
                        if r % 29 == 0 {
                            2 // 1.18 grove: snowy spruce forest on the flanks
                        } else {
                            continue;
                        }
                    }
                    B_MEADOW => {
                        if r % 193 == 0 {
                            0 // the occasional lone oak in the meadows
                        } else {
                            continue;
                        }
                    }
                    B_SNOW => {
                        if snowy_here && r % 47 == 0 {
                            2
                        } else {
                            continue;
                        }
                    }
                    _ => continue,
                };

                let (log, leaf) = match kind {
                    0 => (LOG, LEAVES),
                    1 => (BIRCH_LOG, BIRCH_LEAVES),
                    3 => {
                        // poplar: one of the three leaf colors per tree
                        let leaf = match (r >> 5) % 3 {
                            0 => POP_LEAVES_R,
                            1 => POP_LEAVES_O,
                            _ => POP_LEAVES_Y,
                        };
                        (POPLAR_LOG, leaf)
                    }
                    _ => (SPRUCE_LOG, SPRUCE_LEAVES),
                };
                let th = match kind {
                    2 => 5 + ((r >> 8) % 3) as i32,       // spruce: tall
                    1 => 5 + ((r >> 8) % 2) as i32,       // birch: slim
                    3 => 6 + ((r >> 8) % 3) as i32,       // poplar: tall & slim
                    _ => 4 + ((r >> 8) % 3) as i32,       // oak
                };

                if kind == 4 {
                    // fallen poplar log lying on the ground
                    let along_x = (r >> 7) % 2 == 0;
                    let len = 3 + ((r >> 10) % 3) as i32;
                    for i in 0..len {
                        let (fx, fz) = if along_x {
                            (wx + i, wz)
                        } else {
                            (wx, wz + i)
                        };
                        put(&mut ch, x0, z0, fx, h + 1, fz, POPLAR_LOG, true);
                    }
                    // large shelf mushroom growing on the trunk
                    if (r >> 12) % 2 == 0 {
                        let (fx, fz) = if along_x {
                            (wx + len / 2, wz)
                        } else {
                            (wx, wz + len / 2)
                        };
                        put(&mut ch, x0, z0, fx, h + 2, fz, SHELF_LG, true);
                    }
                    continue;
                }

                if kind >= 5 {
                    // ---- v0.4 tree families (draw fully, then continue) ----
                    if kind == 5 {
                        // acacia: slim trunk + flat disc canopy
                        let th = 4 + ((r >> 8) % 2) as i32;
                        for dy in (th - 1)..=th {
                            let rad: i32 = if dy == th { 2 } else { 1 };
                            for dx in -rad..=rad {
                                for dz in -rad..=rad {
                                    if dx * dx + dz * dz > rad * rad + 1 {
                                        continue;
                                    }
                                    put(&mut ch, x0, z0, wx + dx, h + dy + 1, wz + dz, ACACIA_LEAVES, true);
                                }
                            }
                        }
                        for dy in 1..=th {
                            put(&mut ch, x0, z0, wx, h + dy, wz, ACACIA_LOG, false);
                        }
                        put(&mut ch, x0, z0, wx, h, wz, DIRT, false);
                        continue;
                    }
                    if kind == 6 {
                        // jungle: tall trunk with a wide shaggy crown
                        let th = 7 + ((r >> 8) % 3) as i32;
                        for dy in (th - 2)..=(th + 1) {
                            let rad: i32 = if dy >= th { 2 } else { 3 };
                            for dx in -rad..=rad {
                                for dz in -rad..=rad {
                                    let corner = dx.abs() == rad && dz.abs() == rad;
                                    let skip = corner
                                        && hash01(hash3i(
                                            (wx + dx) as i64,
                                            (h + dy) as i64,
                                            (wz + dz) as i64,
                                            self.seed ^ 0x1EAF,
                                        )) < 0.55;
                                    if skip {
                                        continue;
                                    }
                                    put(&mut ch, x0, z0, wx + dx, h + dy, wz + dz, LEAVES, true);
                                }
                            }
                        }
                        for dy in 1..=th {
                            put(&mut ch, x0, z0, wx, h + dy, wz, LOG, false);
                        }
                        put(&mut ch, x0, z0, wx, h, wz, DIRT, false);
                        continue;
                    }
                    if kind == 7 {
                        // cherry: short trunk + wide pink cloud + maybe a hive
                        let th = 4 + ((r >> 8) % 2) as i32;
                        for dy in (th - 2)..=(th + 1) {
                            let rad: i32 = match th - dy {
                                2 => 1,
                                1 => 2,
                                _ => 2,
                            };
                            for dx in -rad..=rad {
                                for dz in -rad..=rad {
                                    let corner = dx.abs() == rad && dz.abs() == rad;
                                    let skip = corner
                                        && hash01(hash3i(
                                            (wx + dx) as i64,
                                            (h + dy) as i64,
                                            (wz + dz) as i64,
                                            self.seed ^ 0x1EAF,
                                        )) < 0.5;
                                    if skip {
                                        continue;
                                    }
                                    put(&mut ch, x0, z0, wx + dx, h + dy, wz + dz, CHERRY_LEAVES, true);
                                }
                            }
                        }
                        for dy in 1..=th {
                            put(&mut ch, x0, z0, wx, h + dy, wz, CHERRY_LOG, false);
                        }
                        put(&mut ch, x0, z0, wx, h, wz, DIRT, false);
                        if (r >> 22) % 5 == 0 {
                            put(&mut ch, x0, z0, wx + 1, h + 2, wz, BEEHIVE, true);
                        }
                        continue;
                    }
                    // kind 8: mangrove — stilted roots in the mud
                    let th = 5 + ((r >> 8) % 3) as i32;
                    for (dx, dz) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                        put(&mut ch, x0, z0, wx + dx, h, wz + dz, MANGROVE_LOG, true);
                    }
                    for dy in (th - 1)..=(th + 1) {
                        let rad: i32 = if dy == th + 1 { 1 } else { 2 };
                        for dx in -rad..=rad {
                            for dz in -rad..=rad {
                                let corner = dx.abs() == rad && dz.abs() == rad;
                                let skip = corner
                                    && hash01(hash3i(
                                        (wx + dx) as i64,
                                        (h + dy) as i64,
                                        (wz + dz) as i64,
                                        self.seed ^ 0x1EAF,
                                    )) < 0.6;
                                if skip {
                                    continue;
                                }
                                put(&mut ch, x0, z0, wx + dx, h + dy, wz + dz, MANGROVE_LEAVES, true);
                            }
                        }
                    }
                    for dy in 1..=th {
                        put(&mut ch, x0, z0, wx, h + dy, wz, MANGROVE_LOG, false);
                    }
                    put(&mut ch, x0, z0, wx, h, wz, MUD, false);
                    continue;
                }

                // canopy first (only into air), trunk overwrites afterwards
                if kind == 2 {
                    // spruce: layered cone
                    for (dy, rad) in [(th - 3, 2i32), (th - 2, 2), (th - 1, 1), (th, 1), (th + 1, 0)] {
                        let y = h + dy;
                        if rad == 0 {
                            put(&mut ch, x0, z0, wx, y, wz, leaf, true);
                            continue;
                        }
                        for dx in -rad..=rad {
                            for dz in -rad..=rad {
                                if dx == 0 && dz == 0 && dy <= th {
                                    continue;
                                }
                                let corner = dx.abs() == rad && dz.abs() == rad;
                                if corner
                                    && hash01(hash3i(
                                        (wx + dx) as i64,
                                        y as i64,
                                        (wz + dz) as i64,
                                        self.seed ^ 0x1EAF,
                                    )) < 0.5
                                {
                                    continue;
                                }
                                put(&mut ch, x0, z0, wx + dx, y, wz + dz, leaf, true);
                            }
                        }
                    }
                } else if kind == 3 {
                    // poplar: slim canopy hugging the trunk
                    for dy in (th - 2)..=(th + 1) {
                        let rad: i32 = if dy == th + 1 { 0 } else { 1 };
                        for dx in -rad..=rad {
                            for dz in -rad..=rad {
                                let corner = dx.abs() == 1 && dz.abs() == 1;
                                let skip = corner
                                    && hash01(hash3i(
                                        (wx + dx) as i64,
                                        (h + dy) as i64,
                                        (wz + dz) as i64,
                                        self.seed ^ 0x1EAF,
                                    )) < 0.7;
                                if skip {
                                    continue;
                                }
                                put(&mut ch, x0, z0, wx + dx, h + dy, wz + dz, leaf, true);
                            }
                        }
                    }
                    // a shelf mushroom on the trunk now and then
                    if (r >> 14) % 3 == 0 {
                        let dy = 2 + ((r >> 16) % (th as u64 - 3).max(1)) as i32;
                        let side = (r >> 18) % 4;
                        let (dx, dz) = match side {
                            0 => (1, 0),
                            1 => (-1, 0),
                            2 => (0, 1),
                            _ => (0, -1),
                        };
                        put(&mut ch, x0, z0, wx + dx, h + dy, wz + dz, SHELF_SM, true);
                    }
                } else {
                    for dy in (th - 2)..=(th + 1) {
                        let rad: i32 = if dy >= th { 1 } else { 2 };
                        for dx in -rad..=rad {
                            for dz in -rad..=rad {
                                let corner = dx.abs() == rad && dz.abs() == rad;
                                let skip = corner
                                    && hash01(hash3i(
                                        (wx + dx) as i64,
                                        (h + dy) as i64,
                                        (wz + dz) as i64,
                                        self.seed ^ 0x1EAF,
                                    )) < 0.6;
                                if skip {
                                    continue;
                                }
                                put(&mut ch, x0, z0, wx + dx, h + dy, wz + dz, leaf, true);
                            }
                        }
                    }
                }
                for dy in 1..=th {
                    put(&mut ch, x0, z0, wx, h + dy, wz, log, false);
                }
                put(&mut ch, x0, z0, wx, h, wz, DIRT, false); // dirt under trunk
                // 1.15: oak & birch sometimes host a beehive
                if (kind == 0 || kind == 1) && (r >> 22) % 6 == 0 {
                    put(&mut ch, x0, z0, wx + 1, h + 2, wz, BEEHIVE, true);
                }
            }
        }

        // Ground cover: plants, cacti, pumpkins (1x1, fully deterministic).
        for lx in 0..CX {
            for lz in 0..CZ {
                let wx = x0 + lx as i32;
                let wz = z0 + lz as i32;
                let biome = self.biome_at(wx, wz);
                let h = self.height_at(wx, wz);
                if h + 1 >= CY as i32 {
                    continue;
                }
                let surf = ch.get(lx, h as usize, lz);
                let above = ch.get(lx, (h + 1) as usize, lz);
                if above != AIR {
                    continue; // under a tree / in water
                }
                let r = hash2i(wx as i64, wz as i64, self.seed ^ 0x60CA);
                match biome {
                    B_PLAINS | B_FOREST | B_BIRCH => {
                        if surf != GRASS {
                            continue;
                        }
                        let m = r % 100;
                        if m < 14 {
                            ch.set(lx, (h + 1) as usize, lz, TALLGRASS);
                        } else if m < 18 {
                            ch.set(lx, (h + 1) as usize, lz, if r % 2 == 0 { FLOWER_RED } else { FLOWER_YELLOW });
                        } else if m < 19 && biome != B_PLAINS {
                            ch.set(lx, (h + 1) as usize, lz, MUSHROOM);
                        } else if biome == B_PLAINS && r % 431 == 7 {
                            ch.set(lx, (h + 1) as usize, lz, PUMPKIN);
                        } else if r % 271 == 9 {
                            // 1.14: sweet berry bushes in the temperate lands
                            ch.set(lx, (h + 1) as usize, lz, BERRY_BUSH);
                        } else if biome == B_FOREST {
                            // 1.17: moss carpets with azalea bushes
                            let patch = hash2i(
                                (wx as i64).div_euclid(7),
                                (wz as i64).div_euclid(7),
                                self.seed ^ 0x402A,
                            );
                            if patch % 5 == 0 {
                                let m2 = r % 13;
                                if m2 == 0 {
                                    ch.set(lx, (h + 1) as usize, lz, AZALEA);
                                } else if m2 == 1 {
                                    ch.set(lx, (h + 1) as usize, lz, AZALEA_FLOWER);
                                } else if m2 < 5 {
                                    ch.set(lx, h as usize, lz, MOSS);
                                }
                            }
                        } else if (h == WATER_LEVEL || h == WATER_LEVEL + 1) && r % 23 == 7 {
                            // 1.13: sugarcane on shorelines
                            let sh = 2 + ((r >> 8) % 2) as i32;
                            for dy in 0..sh {
                                if h + 1 + dy < CY as i32
                                    && ch.get(lx, (h + 1 + dy) as usize, lz) == AIR
                                {
                                    ch.set(lx, (h + 1 + dy) as usize, lz, SUGARCANE);
                                }
                            }
                        }
                    }
                    B_SAVANNA => {
                        if surf != GRASS {
                            continue;
                        }
                        let m = r % 100;
                        if m < 30 {
                            ch.set(lx, (h + 1) as usize, lz, TALLGRASS);
                        } else if m < 33 {
                            ch.set(lx, (h + 1) as usize, lz, DEADBUSH);
                        } else if (h == WATER_LEVEL || h == WATER_LEVEL + 1) && r % 23 == 7 {
                            let sh = 2 + ((r >> 8) % 2) as i32;
                            for dy in 0..sh {
                                if h + 1 + dy < CY as i32
                                    && ch.get(lx, (h + 1 + dy) as usize, lz) == AIR
                                {
                                    ch.set(lx, (h + 1 + dy) as usize, lz, SUGARCANE);
                                }
                            }
                        }
                    }
                    B_JUNGLE => {
                        if surf != GRASS {
                            continue;
                        }
                        let m = r % 100;
                        if m < 40 {
                            ch.set(lx, (h + 1) as usize, lz, TALLGRASS);
                        } else if m < 44 {
                            ch.set(lx, (h + 1) as usize, lz, if r % 2 == 0 { FLOWER_RED } else { FLOWER_YELLOW });
                        } else if r % 291 == 3 {
                            // 1.14: wild melons
                            ch.set(lx, (h + 1) as usize, lz, MELON);
                        } else {
                            // bamboo groves in patchy clusters
                            let patch = hash2i(
                                (wx as i64).div_euclid(6),
                                (wz as i64).div_euclid(6),
                                self.seed ^ 0xB00B0,
                            );
                            if patch % 3 == 0 && r % 3 != 0 {
                                let bh = 2 + ((r >> 9) % 3) as i32;
                                for dy in 0..bh {
                                    let y = h + 1 + dy;
                                    if y < CY as i32 && ch.get(lx, y as usize, lz) == AIR {
                                        ch.set(lx, y as usize, lz, BAMBOO);
                                    }
                                }
                            }
                        }
                    }
                    B_CHERRY => {
                        if surf != GRASS {
                            continue;
                        }
                        let m = r % 100;
                        if m < 25 {
                            // 1.20: pink petal carpets under the blossom trees
                            ch.set(lx, (h + 1) as usize, lz, PINK_PETALS);
                        } else if m < 33 {
                            ch.set(lx, (h + 1) as usize, lz, TALLGRASS);
                        } else if m < 38 {
                            ch.set(lx, (h + 1) as usize, lz, if r % 2 == 0 { FLOWER_RED } else { FLOWER_YELLOW });
                        }
                    }
                    B_BADLANDS => {
                        if surf != RED_SAND {
                            continue;
                        }
                        let m = r % 100;
                        if m < 6 {
                            ch.set(lx, (h + 1) as usize, lz, DEADBUSH);
                        } else if m < 8 {
                            let chh = 2 + ((r >> 8) % 2) as i32;
                            for dy in 1..=chh {
                                if h + dy < CY as i32 {
                                    ch.set(lx, (h + dy) as usize, lz, CACTUS);
                                }
                            }
                        } else if r % 97 == 11 {
                            // 1.7: exposed gold nuggets in the mesa surface
                            ch.set(lx, h as usize, lz, GOLD_ORE);
                        }
                    }
                    B_SWAMP => {
                        if surf == GRASS && r % 100 < 16 {
                            ch.set(lx, (h + 1) as usize, lz, TALLGRASS);
                        } else if surf == GRASS && r % 100 < 20 {
                            ch.set(lx, (h + 1) as usize, lz, MUSHROOM);
                        } else if surf == GRASS && r % 37 == 5 {
                            // exposed mud flats
                            ch.set(lx, h as usize, lz, MUD);
                        }
                    }
                    B_DAPPLED => {
                        if surf != GRASS_PALE {
                            continue;
                        }
                        let m = r % 100;
                        if m < 9 {
                            // red shrubs grow in small patches
                            let patch = hash2i(
                                (wx as i64).div_euclid(5),
                                (wz as i64).div_euclid(5),
                                self.seed ^ 0x5E4D,
                            );
                            if patch % 5 == 0 {
                                ch.set(lx, (h + 1) as usize, lz, RED_SHRUB);
                            }
                        } else if m < 12 {
                            ch.set(lx, (h + 1) as usize, lz, SHELF_SM);
                        } else if m < 20 {
                            ch.set(lx, (h + 1) as usize, lz, TALLGRASS);
                        } else if m < 22 {
                            ch.set(
                                lx,
                                (h + 1) as usize,
                                lz,
                                if r % 2 == 0 { FLOWER_RED } else { FLOWER_YELLOW },
                            );
                        }
                    }
                    B_DESERT => {
                        if surf != SAND {
                            continue;
                        }
                        let m = r % 100;
                        if m < 2 {
                            // cactus, 2..3 tall
                            let chh = 2 + ((r >> 8) % 2) as i32;
                            for dy in 1..=chh {
                                if h + dy < CY as i32 {
                                    ch.set(lx, (h + dy) as usize, lz, CACTUS);
                                }
                            }
                        } else if m < 5 {
                            ch.set(lx, (h + 1) as usize, lz, DEADBUSH);
                        }
                    }
                    B_MEADOW => {
                        // 1.18 meadows: flower fields in the high grass
                        if surf != GRASS {
                            continue;
                        }
                        let m = r % 100;
                        if m < 26 {
                            ch.set(lx, (h + 1) as usize, lz, TALLGRASS);
                        } else if m < 46 {
                            ch.set(
                                lx,
                                (h + 1) as usize,
                                lz,
                                if r % 2 == 0 { FLOWER_RED } else { FLOWER_YELLOW },
                            );
                        } else if m < 50 {
                            ch.set(lx, (h + 1) as usize, lz, PINK_PETALS);
                        }
                    }
                    _ => {}
                }
            }
        }

        // Water world (1.13/1.19): lily pads, seagrass, kelp forests, coral.
        for lx in 0..CX {
            for lz in 0..CZ {
                let wx = x0 + lx as i32;
                let wz = z0 + lz as i32;
                let h = self.height_at(wx, wz);
                if h < 1 || h + 1 >= CY as i32 {
                    continue;
                }
                let r = hash2i(wx as i64, wz as i64, self.seed ^ 0x0CEA);
                // lily pads float on swamp water
                if self.biome_at(wx, wz) == B_SWAMP
                    && h < WATER_LEVEL
                    && ch.get(lx, WATER_LEVEL as usize, lz) == WATER
                    && ch.get(lx, (WATER_LEVEL + 1) as usize, lz) == AIR
                    && r % 19 == 4
                {
                    ch.set(lx, (WATER_LEVEL + 1) as usize, lz, LILYPAD);
                    continue;
                }
                if h >= WATER_LEVEL {
                    continue; // column is above water
                }
                let surf = ch.get(lx, h as usize, lz);
                let floor_ok = surf == SAND || surf == GRAVEL || surf == MUD || surf == RED_SAND;
                if !floor_ok {
                    continue;
                }
                if r % 9 == 0 {
                    ch.set(lx, (h + 1) as usize, lz, SEAGRASS);
                } else if r % 37 == 1 {
                    // kelp forest stalk
                    let kh = 2 + ((r >> 6) % 4) as i32;
                    for dy in 1..=kh {
                        let y = h + dy;
                        if y < WATER_LEVEL && ch.get(lx, y as usize, lz) == WATER {
                            ch.set(lx, y as usize, lz, KELP);
                        }
                    }
                } else if self.temp_at(wx, wz) > 0.52 && r % 71 == 5 {
                    // warm-water coral gardens
                    let c = match (r >> 8) % 5 {
                        0 | 2 => CORAL_PINK,
                        1 => CORAL_BLUE,
                        _ => CORAL_DEAD,
                    };
                    ch.set(lx, (h + 1) as usize, lz, c);
                }
            }
        }

        // --- v0.5: 1.18 lush caves & dripstone caves -------------------
        let mut hts = [[0i32; CZ]; CX];
        for lx in 0..CX {
            for lz in 0..CZ {
                hts[lx][lz] = self.height_at(x0 + lx as i32, z0 + lz as i32);
            }
        }
        decorate_caves(&mut ch, x0, z0, self.seed, &hts);

        // Amethyst geodes (1.17): rare chunk-local underground spheres.
        let gr = hash2i(cx as i64, cz as i64, self.seed ^ 0x6E0D_E5);
        if gr % 29 == 0 {
            gen_geode(&mut ch, gr);
        }

        // Abandoned camp: ~1/13 of eligible chunks, fully deterministic and
        // chunk-local (Wilderness Bound structure).
        let cr = hash2i(cx as i64, cz as i64, self.seed ^ 0xCA3F_0000);
        if cr % 13 == 0 {
            gen_camp(&mut ch, x0, z0, cr, self);
        }

        // Shipwrecks (1.13): broken wooden hulls resting in deep water.
        let sr = hash2i(cx as i64, cz as i64, self.seed ^ 0x5EE_A0);
        if sr % 21 == 0 {
            gen_shipwreck(&mut ch, x0, z0, sr, self);
        }

        self.chunks.insert((cx, cz), ch);
    }

    /// World-space block lookup. Missing chunks read as AIR; below the world
    /// reads as BEDROCK so nothing can fall out.
    pub fn get_block(&self, x: i32, y: i32, z: i32) -> u16 {
        if y < 0 {
            return BEDROCK;
        }
        if y >= CY as i32 {
            return AIR;
        }
        let (cx, cz, lx, lz) = idx_of(x, y, z);
        match self.chunks.get(&(cx, cz)) {
            Some(c) => c.get(lx, y as usize, lz),
            None => AIR,
        }
    }

    pub fn set_block(&mut self, x: i32, y: i32, z: i32, b: u16) {
        if y < 0 || y >= CY as i32 {
            return;
        }
        let (cx, cz, lx, lz) = idx_of(x, y, z);
        let ch = self.chunks.entry((cx, cz)).or_insert_with(Chunk::new);
        ch.set(lx, y as usize, lz, b);
        ch.modified = true;
    }

    pub fn is_solid(&self, x: i32, y: i32, z: i32) -> bool {
        is_solid_id(self.get_block(x, y, z))
    }

    pub fn is_opaque(&self, x: i32, y: i32, z: i32) -> bool {
        is_opaque_id(self.get_block(x, y, z))
    }

    /// Topmost solid y at (x, z) within loaded chunks (else None).
    pub fn surface_y(&self, x: i32, z: i32) -> Option<i32> {
        let mut y = CY as i32 - 1;
        while y > 0 {
            let b = self.get_block(x, y, z);
            if is_solid_id(b) {
                return Some(y);
            }
            y -= 1;
        }
        None
    }
}

// ---------------------------------------------------------------- geode
/// v0.5: 1.18 lush caves & dripstone caves. Underground caverns get their
/// own biome: region noise (only evaluated inside carved cells) picks the
/// style — moss floors with glow berries and azalea bushes, or dripstone
/// with stone spikes. `hts` holds the precomputed terrain heights.
pub(crate) fn decorate_caves(ch: &mut Chunk, x0: i32, z0: i32, seed: u64, hts: &[[i32; CZ]; CX]) {
    for lx in 0..CX {
        for lz in 0..CZ {
            let wx = x0 + lx as i32;
            let wz = z0 + lz as i32;
            let hmax = hts[lx][lz].min(58);
            for y in 5..hmax.max(6) {
                if ch.get(lx, y as usize, lz) != AIR {
                    continue;
                }
                let lush = fbm3(
                    wx as f32 * 0.021,
                    y as f32 * 0.05,
                    wz as f32 * 0.021,
                    seed ^ 0x1CA7,
                    2,
                );
                let dri = if lush <= 0.60 {
                    fbm3(
                        wx as f32 * 0.024,
                        y as f32 * 0.05,
                        wz as f32 * 0.024,
                        seed ^ 0x3D1F,
                        2,
                    )
                } else {
                    0.0
                };
                if lush <= 0.60 && dri <= 0.60 {
                    continue;
                }
                let below = ch.get(lx, (y - 1) as usize, lz);
                let above = ch.get(lx, (y + 1) as usize, lz);
                let r = hash3i(wx as i64, y as i64, wz as i64, seed ^ 0xDEE0);
                let floor_stone = is_stone_fam(below);
                let ceil_solid = is_stone_fam(above);
                if lush > 0.60 {
                    if floor_stone {
                        ch.set(lx, (y - 1) as usize, lz, MOSS);
                    }
                    if ceil_solid && r % 19 == 0 {
                        ch.set(lx, y as usize, lz, GLOW_BERRIES);
                    } else if floor_stone && r % 43 == 3 {
                        ch.set(
                            lx,
                            y as usize,
                            lz,
                            if r % 2 == 0 { AZALEA } else { AZALEA_FLOWER },
                        );
                    } else if floor_stone && r % 97 == 11 {
                        // small puddles of water in lush caves
                        ch.set(lx, y as usize, lz, WATER);
                    }
                } else {
                    if ceil_solid && r % 15 == 0 {
                        ch.set(lx, y as usize, lz, DRIPSTONE_SPIKE);
                    } else if floor_stone && r % 17 == 5 {
                        ch.set(lx, y as usize, lz, DRIPSTONE_SPIKE);
                    } else if floor_stone && r % 29 == 7 {
                        ch.set(lx, (y - 1) as usize, lz, DRIPSTONE);
                    }
                }
            }
        }
    }
}

/// Amethyst geode (1.17): a calcite shell around an amethyst ring with a
/// hollow heart, sometimes holding a crying-obsidian core. Only replaces
/// stone-family blocks so caves and dirt stay intact.
fn gen_geode(ch: &mut Chunk, gr: u64) {
    let gx = 4 + ((gr >> 5) % 8) as i32; // 4..11 keeps the sphere chunk-local
    let gz = 4 + ((gr >> 9) % 8) as i32;
    let gy = 8 + ((gr >> 13) % 7) as i32; // 8..14
    let heart = (gr >> 17) % 3 == 0;
    for dx in -4..=4 {
        for dy in -4..=4 {
            for dz in -4..=4 {
                let d2 = dx * dx + dy * dy + dz * dz;
                let (xx, yy, zz) = (gx + dx, gy + dy, gz + dz);
                if xx < 0 || xx >= CX as i32 || zz < 0 || zz >= CZ as i32 || yy < 1 || yy >= CY as i32 {
                    continue;
                }
                let (lx, lz) = (xx as usize, zz as usize);
                let cur = ch.get(lx, yy as usize, lz);
                if !matches!(cur, STONE | DEEPSLATE | COAL | IRON | GRAVEL | COPPER_ORE) {
                    continue;
                }
                if d2 <= 1 {
                    ch.set(lx, yy as usize, lz, if heart { CRYING_OBSIDIAN } else { AMETHYST });
                } else if d2 <= 10 {
                    ch.set(lx, yy as usize, lz, AMETHYST);
                } else if d2 <= 18 {
                    ch.set(lx, yy as usize, lz, CALCITE);
                }
            }
        }
    }
}

// -------------------------------------------------------------- shipwreck
/// Sunken shipwreck (1.13-style, original design): a broken plank hull with
/// a snapped mast resting on the seabed, with barrels, sponges and hay.
fn gen_shipwreck(ch: &mut Chunk, x0: i32, z0: i32, sr: u64, w: &World) {
    let lx = 4 + ((sr >> 4) % 8) as i32;
    let lz = 4 + ((sr >> 6) % 8) as i32;
    let wx = x0 + lx;
    let wz = z0 + lz;
    let h0 = w.height_at(wx, wz);
    if h0 > WATER_LEVEL - 2 || h0 < 3 {
        return; // needs water above the seabed (partially sunk hulls are fine)
    }
    let yb = h0 + 1;
    let put_at = |ch: &mut Chunk, x: i32, y: i32, z: i32, b: u16| {
        let lx2 = x - x0;
        let lz2 = z - z0;
        if lx2 >= 0 && lx2 < CX as i32 && lz2 >= 0 && lz2 < CZ as i32 && y > 0 && y < CY as i32 {
            ch.set(lx2 as usize, y as usize, lz2 as usize, b);
        }
    };
    // hull floor 7x3
    for dx in -3..=3 {
        for dz in -1..=1 {
            put_at(ch, wx + dx, yb, wz + dz, PLANKS);
        }
    }
    // hull sides with rotted-out gaps
    for dx in -3..=3 {
        for dz in [-1i32, 1] {
            let hole = hash01(hash3i((wx + dx) as i64, 7, (wz + dz) as i64, 0x7007)) < 0.25;
            if hole {
                continue;
            }
            let hh = if dx.abs() == 3 { 2 } else { 1 };
            for dy in 1..=hh {
                put_at(
                    ch,
                    wx + dx,
                    yb + dy,
                    wz + dz,
                    if dy == 2 { LOG } else { PLANKS },
                );
            }
        }
    }
    // bow & stern posts
    put_at(ch, wx - 4, yb + 1, wz, LOG);
    put_at(ch, wx + 4, yb + 1, wz, LOG);
    // snapped mast
    let mast = 2 + ((sr >> 9) % 3) as i32;
    for dy in 1..=mast {
        put_at(ch, wx, yb + dy, wz, LOG);
    }
    // cargo in the hold
    put_at(ch, wx - 2, yb + 1, wz, BARREL);
    put_at(ch, wx + 2, yb + 1, wz + 1, BARREL);
    put_at(ch, wx + 1, yb + 1, wz, SPONGE);
    put_at(ch, wx + 2, yb + 1, wz - 1, HAY);
    put_at(ch, wx - 1, yb + 1, wz, MOSS);
}

// ------------------------------------------------------------------- camp
/// Abandoned camp: flatten a 7x7 patch, pitch a wool tent whose color depends
/// on the biome, campfire, barrels, hay, straw beds and cushions.
fn gen_camp(ch: &mut Chunk, x0: i32, z0: i32, cr: u64, w: &World) {
    // center of the camp, inside the chunk with a 3-block margin
    let lx = 4 + ((cr >> 4) % 8) as i32; // 4..11
    let lz = 4 + ((cr >> 6) % 8) as i32;
    let wx = x0 + lx;
    let wz = z0 + lz;
    let biome = w.biome_at(wx, wz);
    let eligible = matches!(
        biome,
        B_PLAINS | B_FOREST | B_BIRCH | B_DAPPLED | B_SNOW | B_SWAMP
    );
    if !eligible {
        return;
    }
    let h0 = w.height_at(wx, wz);
    if h0 <= WATER_LEVEL + 1 || h0 >= CY as i32 - 8 {
        return;
    }
    // needs a reasonably flat 7x7 spot
    for (dx, dz) in [(-3i32, 0i32), (3, 0), (0, -3), (0, 3), (-3, -3), (3, 3), (3, -3), (-3, 3)] {
        if (w.height_at(wx + dx, wz + dz) - h0).abs() > 1 {
            return;
        }
    }

    // tent wool color depends on the biome (dye index)
    let dye: u16 = match biome {
        B_PLAINS => 4,    // yellow
        B_FOREST => 13,   // green
        B_BIRCH => 0,     // white
        B_DAPPLED => 1,   // orange
        B_SNOW => 3,      // light blue
        _ => 12,          // brown (swamp)
    };
    let tent = WOOL_COLOR_BASE + dye;
    let floor = if biome == B_DAPPLED { POPLAR_PLANKS } else { PLANKS };

    // flatten + clear 7x7
    for dx in -3..=3 {
        for dz in -3..=3 {
            let px = wx + dx;
            let pz = wz + dz;
            let lx2 = px - x0;
            let lz2 = pz - z0;
            if lx2 < 0 || lx2 >= CX as i32 || lz2 < 0 || lz2 >= CZ as i32 {
                continue;
            }
            for y in (h0 + 1)..(h0 + 10).min(CY as i32) {
                ch.set(lx2 as usize, y as usize, lz2 as usize, AIR);
            }
            // sturdy ground under the camp
            let top = ch.get(lx2 as usize, h0 as usize, lz2 as usize);
            if top != STONE && top != DIRT {
                ch.set(lx2 as usize, h0 as usize, lz2 as usize, DIRT);
            }
        }
    }

    let put_at = |ch: &mut Chunk, x: i32, y: i32, z: i32, b: u16| {
        let lx2 = x - x0;
        let lz2 = z - z0;
        if lx2 >= 0 && lx2 < CX as i32 && lz2 >= 0 && lz2 < CZ as i32 && y > 0 && y < CY as i32 {
            ch.set(lx2 as usize, y as usize, lz2 as usize, b);
        }
    };

    // tent: 3x3 footprint, ring of wool, ridge roof, open front facing camp
    let tx = wx - 1;
    let tz = wz - 1;
    for dz in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dz == -1 {
                continue; // entrance
            }
            if dx == 0 {
                put_at(ch, tx + dx, h0 + 2, tz + dz, tent); // ridge
            } else {
                put_at(ch, tx + dx, h0 + 1, tz + dz, tent);
            }
        }
    }
    // interior: straw bed on a plank floor
    put_at(ch, tx, h0, tz, floor);
    put_at(ch, tx, h0 + 1, tz, STRAW_BED);

    // campfire + benches
    put_at(ch, wx + 1, h0 + 1, wz + 1, CAMPFIRE);
    put_at(ch, wx + 1, h0 + 1, wz, LOG);
    put_at(ch, wx, h0 + 1, wz + 1, floor);

    // barrels with "loot" (decorative)
    put_at(ch, wx + 2, h0 + 1, wz - 1, BARREL);
    put_at(ch, wx - 2, h0 + 1, wz + 1, BARREL);

    // hay bales
    put_at(ch, wx + 2, h0 + 1, wz + 2, HAY);
    if (cr >> 8) % 2 == 0 {
        put_at(ch, wx + 2, h0 + 2, wz + 2, HAY);
    }

    // cushions (random colors, no collision)
    put_at(ch, wx + 1, h0 + 1, wz - 2, CUSHION_BASE + ((cr >> 10) % 16) as u16);
    put_at(ch, wx - 2, h0 + 1, wz, CUSHION_BASE + ((cr >> 14) % 16) as u16);

    // lantern pole: log with glowstone on top
    put_at(ch, wx - 2, h0 + 1, wz - 2, POPLAR_LOG);
    put_at(ch, wx - 2, h0 + 2, wz - 2, GLOWSTONE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_is_sane() {
        let w = World::new(12345);
        for x in -200..=200 {
            for z in (-200..=200).step_by(7) {
                let h = w.height_at(x, z);
                assert!((2..=CY as i32 - 10).contains(&h));
            }
        }
    }

    #[test]
    fn chunk_gen_has_bedrock_floor_and_ground() {
        let mut w = World::new(777);
        w.gen_chunk(0, 0);
        let ch = w.chunks.get(&(0, 0)).unwrap();
        for lx in 0..CX {
            for lz in 0..CZ {
                assert_eq!(ch.get(lx, 0, lz), BEDROCK);
            }
        }
        let h = w.height_at(5, 5);
        let b = w.get_block(5, h, 5);
        assert!(is_solid_id(b) || is_cross_id(b), "top of terrain should be solid");
        assert_eq!(w.get_block(5, h + 6, 5), AIR);
    }

    #[test]
    fn negative_coords_work() {
        let mut w = World::new(9);
        w.gen_chunk(-1, -1);
        assert_eq!(w.get_block(-1, 0, -1), BEDROCK);
        let h = w.height_at(-1, -1);
        assert!(is_solid_id(w.get_block(-1, h, -1)));
    }

    #[test]
    fn set_block_roundtrip_and_flag() {
        let mut w = World::new(3);
        w.gen_chunk(0, 0);
        w.set_block(2, 40, 3, BRICK);
        assert_eq!(w.get_block(2, 40, 3), BRICK);
        assert!(w.chunks.get(&(0, 0)).unwrap().modified);
    }

    #[test]
    fn trees_are_deterministic_across_chunks() {
        // Generate chunk A, then A+B; the shared border cells of A must not change.
        let mut w = World::new(4242);
        w.gen_chunk(0, 0);
        let before = w.chunks.get(&(0, 0)).unwrap().blocks.clone();
        w.gen_chunk(1, 0);
        w.gen_chunk(0, 1);
        w.gen_chunk(1, 1);
        let after = w.chunks.get(&(0, 0)).unwrap().blocks.clone();
        assert_eq!(before, after, "chunk content must not depend on neighbors");
    }

    #[test]
    fn biomes_are_deterministic_and_plants_legal() {
        let mut w = World::new(2024);
        for cx in -2..=2 {
            for cz in -2..=2 {
                w.gen_chunk(cx, cz);
            }
        }
        // every cross plant must sit on a legal support
        for (k, ch) in &w.chunks {
            for y in 1..CY {
                for lz in 0..CZ {
                    for lx in 0..CX {
                        let b = ch.get(lx, y, lz);
                        if is_cross_id(b) {
                            let under = ch.get(lx, y - 1, lz);
                            let ok = match b {
                                GLOW_BERRIES => true, // hangs from cave ceilings
                                DRIPSTONE_SPIKE => true, // hangs or stands in caves
                                TORCH => true,        // player-placed only
                                KELP | SEAGRASS => matches!(
                                    under,
                                    KELP | SEAGRASS | SAND | GRAVEL | MUD | RED_SAND
                                ),
                                SUGARCANE => matches!(under, SUGARCANE | GRASS | SAND),
                                BAMBOO => matches!(under, BAMBOO | GRASS),
                                _ => matches!(
                                    under,
                                    GRASS | SAND | GRASS_PALE | RED_SAND | PODZOL
                                        | COARSE_DIRT | MUD | MOSS
                                ),
                            };
                            assert!(
                                ok,
                                "plant {:?} at {:?} on {:?}",
                                b, k, under
                            );
                        }
                    }
                }
            }
        }
        // cacti must sit on sand
        for (k, ch) in &w.chunks {
            for y in 1..CY {
                for lz in 0..CZ {
                    for lx in 0..CX {
                        if ch.get(lx, y, lz) == CACTUS {
                            let under = ch.get(lx, y - 1, lz);
                            assert!(under == SAND || under == CACTUS, "cactus on {:?}", k);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn hardness_is_positive_for_breakables() {
        for b in [
            GRASS, DIRT, STONE, COBBLE, PLANKS, LOG, LEAVES, SAND, GLASS, BRICK, SNOW, COAL,
            IRON, GRAVEL, SANDSTONE, TALLGRASS, FLOWER_RED, PUMPKIN, GLOWSTONE, WOOL,
        ] {
            assert!(hardness(b) > 0.0 && hardness(b).is_finite(), "b={b}");
        }
        assert_eq!(hardness(BEDROCK), f32::INFINITY);
    }

    #[test]
    fn cross_blocks_are_not_solid_but_targetable() {
        assert!(!is_solid_id(TALLGRASS));
        assert!(is_targetable(TALLGRASS));
        assert!(!is_opaque_id(TALLGRASS));
        assert!(is_solid_id(CACTUS));
        assert!(is_targetable(PUMPKIN));
        // Wilderness Bound: cushions have NO collision but are targetable
        assert!(!is_solid_id(CUSHION_BASE + 3));
        assert!(is_targetable(CUSHION_BASE + 3));
        assert!(shape_of(CUSHION_BASE + 3) == Shape::Cushion);
        assert!(shape_of(WOOL_SLAB_BASE) == Shape::Slab);
        assert!(shape_of(CONC_STAIRS_BASE + 7) == Shape::Stairs);
        assert!(shape_of(STRAW_BED) == Shape::Bed);
        assert!(shape_of(POPLAR_LOG) == Shape::Full);
        assert!(shape_of(RED_SHRUB) == Shape::Cross);
        // slabs/stairs/cushions must not hide neighbor faces
        assert!(!is_opaque_id(WOOL_SLAB_BASE));
        assert!(!is_opaque_id(STRAW_BED));
        assert!(is_opaque_id(CONCRETE_BASE));
        assert!(is_opaque_id(WOOL_COLOR_BASE + 14));
    }

    #[test]
    fn dappled_forest_biome_exists_with_poplars() {
        let mut w = World::new(20260915);
        let mut found = None;
        'scan: for x in (-600..600).step_by(13) {
            for z in (-600..600).step_by(13) {
                if w.biome_at(x, z) == B_DAPPLED && w.height_at(x, z) > WATER_LEVEL + 1 {
                    found = Some((x, z));
                    break 'scan;
                }
            }
        }
        let (x, z) = found.expect("a dappled forest must exist");
        let cx = x.div_euclid(16);
        let cz = z.div_euclid(16);
        for dx in -1..=1 {
            for dz in -1..=1 {
                w.gen_chunk(cx + dx, cz + dz);
            }
        }
        // pale grass + poplar wood must appear around the sample point
        let mut pale = 0;
        let mut poplar = 0;
        for dx in -8..=8 {
            for dz in -8..=8 {
                for y in 0..CY {
                    let b = w.get_block(x + dx, y as i32, z + dz);
                    if b == GRASS_PALE {
                        pale += 1;
                    }
                    if b == POPLAR_LOG {
                        poplar += 1;
                    }
                }
            }
        }
        assert!(pale > 50, "pale grass missing ({pale})");
        assert!(poplar >= 6, "poplar logs missing ({poplar})");
    }

    #[test]
    fn abandoned_camps_generate() {
        // find a seed that produces at least one camp in a 6x6 chunk area
        for seed in 1..40u64 {
            let mut w = World::new(seed * 7919);
            for cx in -3..=2 {
                for cz in -3..=2 {
                    w.gen_chunk(cx, cz);
                }
            }
            let mut campfire = 0;
            let mut bed = 0;
            let mut barrel = 0;
            let mut cushion = 0;
            for ch in w.chunks.values() {
                for &b in &ch.blocks {
                    if b == CAMPFIRE {
                        campfire += 1;
                    }
                    if b == STRAW_BED {
                        bed += 1;
                    }
                    if b == BARREL {
                        barrel += 1;
                    }
                    if is_cushion_id(b) {
                        cushion += 1;
                    }
                }
            }
            if campfire > 0 {
                assert!(bed >= 1 && barrel >= 2 && cushion >= 1,
                    "camp incomplete: fire {campfire} bed {bed} barrel {barrel} cushion {cushion}");
                return;
            }
        }
        panic!("no abandoned camp found in 39 seeds");
    }

    #[test]
    fn all_blocks_map_to_valid_tiles() {
        let max_tile = ATLAS_COLS * ATLAS_ROWS;
        for b in 0..=MAX_BLOCK_ID {
            if b > 44 && !(48..=MAX_BLOCK_ID).contains(&b) {
                continue;
            }
            if (45..=47).contains(&b) {
                continue;
            }
            let t = tiles_of(b);
            for tv in t {
                assert!(tv < max_tile, "block {b} tile {tv} out of atlas");
            }
        }
    }

    #[test]
    fn every_biome_exists() {
        let w = World::new(2026);
        let mut found = [0u32; BIOME_COUNT];
        for x in (-1500..1500).step_by(9) {
            for z in (-1500..1500).step_by(9) {
                found[w.biome_at(x, z) as usize] += 1;
            }
        }
        for (bi, &n) in found.iter().enumerate() {
            assert!(n > 0, "biome {} never generated", BIOME_NAMES[bi]);
        }
    }

    #[test]
    fn biome_palette_matches_vanilla_colors() {
        let pal = BiomePalette::vanilla();
        let g = pal.grass[B_PLAINS as usize];
        assert!((g[0] - 0x91 as f32 / 255.0).abs() < 1e-4);
        assert!((g[1] - 0xBD as f32 / 255.0).abs() < 1e-4);
        assert!((g[2] - 0x59 as f32 / 255.0).abs() < 1e-4);
        // swamp grass & water keep their murky overrides
        assert!(pal.grass[B_SWAMP as usize] != pal.grass[B_PLAINS as usize]);
        assert!(pal.water[B_SWAMP as usize] != pal.water[B_PLAINS as usize]);
        assert!(pal.water[B_BADLANDS as usize] != pal.water[B_PLAINS as usize]);
        // jungle is hot & wet
        let (t, d) = biome_climate(B_JUNGLE);
        assert!(t > 0.8 && d > 0.7);
        // snowy slopes are cold
        let (t2, _) = biome_climate(B_SNOWY_SLOPES);
        assert!(t2 < 0.2);
    }

    #[test]
    fn tint_kinds_follow_the_vanilla_colormap_system() {
        assert_eq!(tint_kind(T_GRASS_TOP), TintKind::Grass);
        assert_eq!(tint_kind(T_GRASS_SIDE_OVERLAY), TintKind::Grass);
        assert_eq!(tint_kind(T_TALLGRASS), TintKind::Grass);
        assert_eq!(tint_kind(T_LILYPAD), TintKind::Grass);
        assert_eq!(tint_kind(T_LEAVES), TintKind::Foliage);
        assert_eq!(tint_kind(T_ACACIA_LEAVES), TintKind::Foliage);
        assert_eq!(tint_kind(T_MANGROVE_LEAVES), TintKind::Foliage);
        // modern vanilla ships seagrass/kelp pre-colored (no tintindex)
        assert_eq!(tint_kind(T_SEAGRASS), TintKind::None);
        assert_eq!(tint_kind(T_KELP), TintKind::None);
        assert_eq!(tint_kind(T_BIRCH_LEAVES), TintKind::BirchLeaves);
        assert_eq!(tint_kind(T_SPRUCE_LEAVES), TintKind::SpruceLeaves);
        assert_eq!(tint_kind(T_WATER), TintKind::Water);
        assert_eq!(tint_kind(T_STONE), TintKind::None);
        assert_eq!(tint_kind(T_DIRT), TintKind::None);
        // fixed vanilla leaf tints
        let b = icon_tint(T_BIRCH_LEAVES);
        assert!((b[0] - 0x80 as f32 / 255.0).abs() < 1e-4);
        let s = icon_tint(T_SPRUCE_LEAVES);
        assert!((s[1] - 0x99 as f32 / 255.0).abs() < 1e-4);
    }

    #[test]
    fn lush_and_dripstone_caves_generate() {
        // sweep seeds; the region noise (threshold 0.60) must produce both
        // decoration styles somewhere
        let mut moss_found = false;
        let mut spike_found = false;
        for s in 0..24u64 {
            let mut ch = Chunk::new();
            for i in 0..ch.blocks.len() {
                ch.blocks[i] = STONE;
            }
            // carve a cavern slab to decorate
            for lx in 0..CX {
                for lz in 0..CZ {
                    for y in 10..20 {
                        ch.set(lx, y, lz, AIR);
                    }
                }
            }
            let hts = [[40i32; CZ]; CX];
            decorate_caves(&mut ch, 0, 0, s * 7919 + 13, &hts);
            for y in 9..21 {
                for lz in 0..CZ {
                    for lx in 0..CX {
                        match ch.get(lx, y, lz) {
                            MOSS => moss_found = true,
                            DRIPSTONE_SPIKE => spike_found = true,
                            _ => {}
                        }
                    }
                }
            }
        }
        assert!(moss_found, "lush caves must generate moss floors");
        assert!(spike_found, "dripstone caves must generate spikes");
    }

    #[test]
    fn new_blocks_have_sane_properties() {
        assert!(!is_solid_id(SNOW_LAYER), "snow layer is walked through");
        assert!(is_solid_id(OAK_FENCE));
        assert!(!is_opaque_id(CACTUS), "inset cactus is not opaque");
        assert!(!is_opaque_id(OAK_FENCE));
        assert!(!is_opaque_id(SNOW_LAYER));
        assert!(is_cross_id(DRIPSTONE_SPIKE));
        assert!(!is_cross_id(TORCH), "torch is a real box now");
        assert_eq!(shape_of(TORCH), Shape::Torch);
        assert_eq!(shape_of(SNOW_LAYER), Shape::SnowLayer);
        assert_eq!(shape_of(OAK_FENCE), Shape::Fence);
        assert_eq!(shape_of(CACTUS), Shape::Cactus);
        assert!(hardness(SNOW_LAYER) > 0.0 && hardness(SNOW_LAYER) < 0.3);
        assert!(hardness(OAK_FENCE) > 0.5);
        // tiles resolve
        assert_eq!(tiles_of(SNOW_LAYER)[0], T_SNOW);
        assert_eq!(tiles_of(OAK_FENCE)[1], T_PLANKS);
        assert_eq!(tiles_of(DRIPSTONE_SPIKE)[0], T_DRIPSTONE_SPIKE);
    }

    #[test]
    fn mountains_reach_the_new_biomes() {
        let w = World::new(2026);
        let mut highest = 0;
        for x in (-2000..2000).step_by(5) {
            for z in (-2000..2000).step_by(5) {
                highest = highest.max(w.height_at(x, z));
            }
        }
        assert!(
            highest >= 86,
            "1.18 mountains must climb into the peak band, max {highest}"
        );
    }

    #[test]
    fn deepslate_copper_and_diamonds_generate() {
        let mut w = World::new(99);
        for cx in -2..=1 {
            for cz in -2..=1 {
                w.gen_chunk(cx, cz);
            }
        }
        let mut deep = 0;
        let mut dia = 0;
        let mut cop = 0;
        for ch in w.chunks.values() {
            for (i, &b) in ch.blocks.iter().enumerate() {
                let y = (i / (CX * CZ)) as i32;
                if b == DEEPSLATE && y < 14 {
                    deep += 1;
                }
                if b == DIAMOND_ORE {
                    dia += 1;
                }
                if b == COPPER_ORE {
                    cop += 1;
                }
            }
        }
        assert!(deep > 2000, "deepslate layer missing ({deep})");
        assert!(cop > 0, "copper ore missing");
        assert!(dia > 0, "diamond ore missing");
    }

    #[test]
    fn geodes_generate() {
        for seed in 1..40u64 {
            let mut w = World::new(seed * 7919);
            for cx in -3..=2 {
                for cz in -3..=2 {
                    w.gen_chunk(cx, cz);
                }
            }
            let mut am = 0;
            let mut cal = 0;
            for ch in w.chunks.values() {
                for &b in &ch.blocks {
                    if b == AMETHYST {
                        am += 1;
                    }
                    if b == CALCITE {
                        cal += 1;
                    }
                }
            }
            if am > 0 {
                assert!(cal > 0, "geode without its calcite shell");
                return;
            }
        }
        panic!("no amethyst geode found in 39 seeds");
    }

    #[test]
    fn shipwrecks_generate() {
        for seed in 1..60u64 {
            let mut w = World::new(seed * 104729);
            for cx in -3..=2 {
                for cz in -3..=2 {
                    w.gen_chunk(cx, cz);
                }
            }
            let mut sponge = 0;
            for ch in w.chunks.values() {
                for &b in &ch.blocks {
                    if b == SPONGE {
                        sponge += 1;
                    }
                }
            }
            if sponge > 0 {
                return;
            }
        }
        panic!("no shipwreck found in 59 seeds");
    }

    #[test]
    fn badlands_have_terraccotta_strata() {
        let w = World::new(2026);
        let mut spot = None;
        'scan: for x in (-1200..1200).step_by(11) {
            for z in (-1200..1200).step_by(11) {
                if w.biome_at(x, z) == B_BADLANDS && w.height_at(x, z) > WATER_LEVEL + 2 {
                    spot = Some((x, z));
                    break 'scan;
                }
            }
        }
        let (x, z) = spot.expect("no badlands found");
        let mut w2 = World::new(w.seed);
        let (cx, cz) = (x.div_euclid(16), z.div_euclid(16));
        for dx in -1..=1 {
            for dz in -1..=1 {
                w2.gen_chunk(cx + dx, cz + dz);
            }
        }
        let mut red = 0;
        let mut strata = 0;
        for dx in -8..=8 {
            for dz in -8..=8 {
                for y in 0..CY {
                    let b = w2.get_block(x + dx, y as i32, z + dz);
                    if b == RED_SAND {
                        red += 1;
                    }
                    if matches!(
                        b,
                        TERRACOTTA | TERRACOTTA_RED | TERRACOTTA_ORANGE | TERRACOTTA_YELLOW
                            | RED_SANDSTONE
                    ) {
                        strata += 1;
                    }
                }
            }
        }
        assert!(red > 30, "red sand missing ({red})");
        assert!(strata > 60, "terracotta strata missing ({strata})");
    }

    #[test]
    fn cherry_grove_has_blossoms() {
        let w = World::new(2026);
        let mut spot = None;
        'scan: for x in (-1200..1200).step_by(7) {
            for z in (-1200..1200).step_by(7) {
                if w.biome_at(x, z) == B_CHERRY && w.height_at(x, z) > WATER_LEVEL + 2 {
                    spot = Some((x, z));
                    break 'scan;
                }
            }
        }
        let (x, z) = spot.expect("no cherry grove found");
        let mut w2 = World::new(w.seed);
        let (cx, cz) = (x.div_euclid(16), z.div_euclid(16));
        for dx in -1..=1 {
            for dz in -1..=1 {
                w2.gen_chunk(cx + dx, cz + dz);
            }
        }
        let mut logs = 0;
        let mut petals = 0;
        for dx in -8..=8 {
            for dz in -8..=8 {
                for y in 0..CY {
                    let b = w2.get_block(x + dx, y as i32, z + dz);
                    if b == CHERRY_LOG {
                        logs += 1;
                    }
                    if b == PINK_PETALS {
                        petals += 1;
                    }
                }
            }
        }
        assert!(logs >= 3, "cherry trees missing ({logs})");
        assert!(petals >= 8, "pink petals missing ({petals})");
    }

    #[test]
    fn jungle_and_savanna_grow_their_trees() {
        let w = World::new(2026);
        let mut jungle = None;
        let mut savanna = None;
        'scan: for x in (-1500..1500).step_by(7) {
            for z in (-1500..1500).step_by(7) {
                let b = w.biome_at(x, z);
                let h = w.height_at(x, z);
                if h <= WATER_LEVEL + 2 {
                    continue;
                }
                if jungle.is_none() && b == B_JUNGLE {
                    jungle = Some((x, z));
                }
                if savanna.is_none() && b == B_SAVANNA {
                    savanna = Some((x, z));
                }
                if jungle.is_some() && savanna.is_some() {
                    break 'scan;
                }
            }
        }
        let count_logs = |w: &World, x: i32, z: i32, log_id: u16| -> u32 {
            let mut w2 = World::new(w.seed);
            let (cx, cz) = (x.div_euclid(16), z.div_euclid(16));
            for dx in -1..=1 {
                for dz in -1..=1 {
                    w2.gen_chunk(cx + dx, cz + dz);
                }
            }
            let mut n = 0;
            for dx in -8..=8 {
                for dz in -8..=8 {
                    for y in 0..CY {
                        if w2.get_block(x + dx, y as i32, z + dz) == log_id {
                            n += 1;
                        }
                    }
                }
            }
            n
        };
        let (jx, jz) = jungle.expect("no jungle found");
        assert!(count_logs(&w, jx, jz, LOG) > 10, "jungle trees missing");
        let (sx, sz) = savanna.expect("no savanna found");
        assert!(count_logs(&w, sx, sz, ACACIA_LOG) > 3, "acacias missing");
    }
}

// --------------------------------------------------- v0.6: UI helpers (FR)
/// French display names for the inventory / tooltips / death screen.
pub fn block_name(b: u16) -> String {
    const DYES: [&str; 16] = [
        "blanc", "orange", "magenta", "bleu clair", "jaune", "vert citron",
        "rose", "gris", "gris clair", "cyan", "violet", "bleu", "marron",
        "vert", "rouge", "noir",
    ];
    let s = match b {
        AIR => "Air",
        GRASS => "Bloc d'herbe",
        DIRT => "Terre",
        STONE => "Pierre",
        COBBLE => "Pierres taillées",
        PLANKS => "Planches de chêne",
        LOG => "Bûche de chêne",
        LEAVES => "Feuilles de chêne",
        SAND => "Sable",
        WATER => "Eau",
        GLASS => "Verre",
        BRICK => "Briques",
        SNOW => "Neige",
        BEDROCK => "Bedrock",
        COAL => "Minerai de charbon",
        IRON => "Minerai de fer",
        GRAVEL => "Gravier",
        SANDSTONE => "Grès",
        BIRCH_LOG => "Bûche de bouleau",
        BIRCH_LEAVES => "Feuilles de bouleau",
        SPRUCE_LOG => "Bûche de sapin",
        SPRUCE_LEAVES => "Feuilles de sapin",
        CACTUS => "Cactus",
        TALLGRASS => "Hautes herbes",
        FLOWER_RED => "Coquelicot",
        FLOWER_YELLOW => "Pissenlit",
        MUSHROOM => "Champignon",
        DEADBUSH => "Buisson mort",
        PUMPKIN => "Citrouille",
        GLOWSTONE => "Pierre lumineuse",
        WOOL => "Laine",
        MEAT => "Viande",
        POPLAR_LOG => "Bûche de peuplier",
        POP_LEAVES_R => "Feuilles rouges",
        POP_LEAVES_O => "Feuilles orangées",
        POP_LEAVES_Y => "Feuilles jaunes",
        POPLAR_PLANKS => "Planches de peuplier",
        RED_SHRUB => "Buisson rouge",
        SHELF_SM | SHELF_LG => "Polypore",
        HAY => "Botte de paille",
        STRAW_BED => "Lit de paille",
        CAMPFIRE => "Feu de camp",
        BARREL => "Tonneau",
        GRASS_PALE => "Herbe pâle",
        RED_SAND => "Sable rouge",
        RED_SANDSTONE => "Grès rouge",
        PODZOL => "Podzol",
        COARSE_DIRT => "Terre stérile",
        PACKED_ICE => "Glace compactée",
        GRANITE => "Granite",
        DIORITE => "Diorite",
        ANDESITE => "Andésite",
        SPONGE => "Éponge",
        MAGMA => "Bloc de magma",
        SEAGRASS => "Herbe marine",
        KELP => "Algues",
        CORAL_PINK => "Corail rose",
        CORAL_BLUE => "Corail bleu",
        CORAL_DEAD => "Corail mort",
        LILYPAD => "Nénuphar",
        SUGARCANE => "Canne à sucre",
        BERRY_BUSH => "Buisson à baies",
        BAMBOO => "Bambou",
        BAMBOO_BLOCK => "Bloc de bambou",
        HONEY => "Bloc de miel",
        BEEHIVE => "Ruche",
        BASALT => "Basalte",
        BLACKSTONE => "Roche noire",
        COPPER_ORE => "Minerai de cuivre",
        COPPER_BLOCK => "Bloc de cuivre",
        AMETHYST => "Améthyste",
        CALCITE => "Calcite",
        TUFF => "Tuf",
        DEEPSLATE => "Ardoise des abîmes",
        DRIPSTONE => "Bloc de stalactite",
        MOSS => "Mousse",
        AZALEA => "Azalée",
        AZALEA_FLOWER => "Azalée en fleurs",
        GLOW_BERRIES => "Baies lumineuses",
        MUD => "Boue",
        PACKED_MUD => "Boue tassée",
        MUD_BRICKS => "Briques de boue",
        SCULK => "Sculk",
        MANGROVE_LOG => "Bûche de palétuvier",
        MANGROVE_LEAVES => "Feuilles de palétuvier",
        MANGROVE_PLANKS => "Planches de palétuvier",
        CHERRY_LOG => "Bûche de cerisier",
        CHERRY_LEAVES => "Feuilles de cerisier",
        CHERRY_PLANKS => "Planches de cerisier",
        PINK_PETALS => "Pétales roses",
        ACACIA_LOG => "Bûche d'acacia",
        ACACIA_LEAVES => "Feuilles d'acacia",
        ACACIA_PLANKS => "Planches d'acacia",
        MELON => "Pastèque",
        TERRACOTTA => "Terre cuite",
        TERRACOTTA_RED => "Terre cuite rouge",
        TERRACOTTA_ORANGE => "Terre cuite orange",
        TERRACOTTA_YELLOW => "Terre cuite jaune",
        GOLD_ORE => "Minerai d'or",
        DIAMOND_ORE => "Minerai de diamant",
        COPPER_BULB => "Ampoule de cuivre",
        OBSIDIAN => "Obsidienne",
        CRYING_OBSIDIAN => "Obsidienne pleureuse",
        TORCH => "Torche",
        BERRIES => "Baies sucrées",
        SNOW_LAYER => "Fine couche de neige",
        DRIPSTONE_SPIKE => "Stalactite pointue",
        _ => "",
    };
    if !s.is_empty() {
        return s.to_string();
    }
    if in_range(b, WOOL_COLOR_BASE, 16) {
        return format!("Laine {}", DYES[(b - WOOL_COLOR_BASE) as usize]);
    }
    if in_range(b, CONCRETE_BASE, 16) {
        return format!("Béton {}", DYES[(b - CONCRETE_BASE) as usize]);
    }
    if in_range(b, CUSHION_BASE, 16) {
        return format!("Coussin {}", DYES[(b - CUSHION_BASE) as usize]);
    }
    if in_range(b, WOOL_SLAB_BASE, 16) {
        return format!("Dalle en laine {}", DYES[(b - WOOL_SLAB_BASE) as usize]);
    }
    if in_range(b, WOOL_STAIRS_BASE, 16) {
        return format!("Escaliers en laine {}", DYES[(b - WOOL_STAIRS_BASE) as usize]);
    }
    if in_range(b, CONC_SLAB_BASE, 16) {
        return format!("Dalle en béton {}", DYES[(b - CONC_SLAB_BASE) as usize]);
    }
    if in_range(b, CONC_STAIRS_BASE, 16) {
        return format!("Escaliers en béton {}", DYES[(b - CONC_STAIRS_BASE) as usize]);
    }
    format!("Bloc #{b}")
}

/// Curated creative catalogue for the inventory screen (pages of 54).
pub const PLACEABLE: &[u16] = &[
    // basics
    GRASS, DIRT, STONE, COBBLE, PLANKS, LOG, LEAVES, SAND, SANDSTONE,
    GRAVEL, GLASS, BRICK, SNOW, SNOW_LAYER, PACKED_ICE, BEDROCK,
    COAL, IRON, COPPER_ORE, GOLD_ORE, DIAMOND_ORE, COPPER_BLOCK, COPPER_BULB,
    DEEPSLATE, BLACKSTONE, BASALT, TUFF, CALCITE, GRANITE, DIORITE, ANDESITE,
    AMETHYST, DRIPSTONE, DRIPSTONE_SPIKE, OBSIDIAN, CRYING_OBSIDIAN, SCULK,
    // woodlands
    BIRCH_LOG, BIRCH_LEAVES, SPRUCE_LOG, SPRUCE_LEAVES, POPLAR_LOG,
    POP_LEAVES_R, POP_LEAVES_O, POP_LEAVES_Y, POPLAR_PLANKS,
    ACACIA_LOG, ACACIA_LEAVES, ACACIA_PLANKS, MANGROVE_LOG, MANGROVE_LEAVES,
    MANGROVE_PLANKS, CHERRY_LOG, CHERRY_LEAVES, CHERRY_PLANKS, PINK_PETALS,
    // flora & farms
    TALLGRASS, FLOWER_RED, FLOWER_YELLOW, MUSHROOM, DEADBUSH, RED_SHRUB,
    CACTUS, SUGARCANE, BAMBOO, BAMBOO_BLOCK, LILYPAD, SEAGRASS, KELP,
    CORAL_PINK, CORAL_BLUE, BERRY_BUSH, BERRIES, GLOW_BERRIES, PUMPKIN,
    MELON, HONEY, BEEHIVE, HAY, STRAW_BED, CAMPFIRE, BARREL, TORCH,
    GLOWSTONE, MAGMA, SPONGE, MEAT,
    // wetlands & wastes
    PODZOL, COARSE_DIRT, MUD, PACKED_MUD, MUD_BRICKS, GRASS_PALE, MOSS,
    AZALEA, AZALEA_FLOWER, RED_SAND, RED_SANDSTONE, TERRACOTTA,
    TERRACOTTA_RED, TERRACOTTA_ORANGE, TERRACOTTA_YELLOW,
    // 16 wools close the catalogue
    WOOL_COLOR_BASE, WOOL_COLOR_BASE + 1, WOOL_COLOR_BASE + 2,
    WOOL_COLOR_BASE + 3, WOOL_COLOR_BASE + 4, WOOL_COLOR_BASE + 5,
    WOOL_COLOR_BASE + 6, WOOL_COLOR_BASE + 7, WOOL_COLOR_BASE + 8,
    WOOL_COLOR_BASE + 9, WOOL_COLOR_BASE + 10, WOOL_COLOR_BASE + 11,
    WOOL_COLOR_BASE + 12, WOOL_COLOR_BASE + 13, WOOL_COLOR_BASE + 14,
    WOOL_COLOR_BASE + 15,
];

#[cfg(test)]
mod ui_tests {
    use super::*;

    #[test]
    fn every_placeable_has_a_french_name() {
        for &b in PLACEABLE {
            let n = block_name(b);
            assert!(!n.is_empty(), "block {b} has no name");
            assert!(!n.contains("#") || b > 222, "uncatalogued block {b}: {n}");
        }
    }

    #[test]
    fn catalogue_is_unique() {
        let mut v = PLACEABLE.to_vec();
        v.sort_unstable();
        v.dedup();
        assert_eq!(v.len(), PLACEABLE.len(), "duplicate entries in PLACEABLE");
    }
}
