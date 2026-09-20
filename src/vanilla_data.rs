// Généré par scripts/gen_vanilla_data.py — NE PAS ÉDITER À LA MAIN.
// Données factuelles de registre Minecraft Java 1.20.5 (protocole 766),
// extraites de PrismarineJS/minecraft-data (data generator Mojang).

pub const PROTOCOL_VERSION: i32 = 766;
pub const MC_VERSION_NAME: &str = "1.20.5";
pub const MAX_STATE_ID: u32 = 26683;
pub const GLOBAL_BITS: u32 = 15; // bits palette directe

/// Table O(1) bloc -> etat vanilla (fallback pierre).
pub const STATE_OF: [u32; 224] = {
    let mut a = [1u32; 224]; // stone
    a[(crate::world::AIR) as usize] = 0;
    a[(crate::world::GRASS) as usize] = 8;
    a[(crate::world::DIRT) as usize] = 10;
    a[(crate::world::WOOL) as usize] = 2047;
    a[(crate::world::STONE) as usize] = 1;
    a[(crate::world::COBBLE) as usize] = 14;
    a[(crate::world::PLANKS) as usize] = 15;
    a[(crate::world::LOG) as usize] = 131;
    a[(crate::world::LEAVES) as usize] = 237;
    a[(crate::world::SAND) as usize] = 112;
    a[(crate::world::WATER) as usize] = 80;
    a[(crate::world::GLASS) as usize] = 519;
    a[(crate::world::BRICK) as usize] = 2093;
    a[(crate::world::SNOW) as usize] = 5781;
    a[(crate::world::BEDROCK) as usize] = 79;
    a[(crate::world::COAL) as usize] = 127;
    a[(crate::world::IRON) as usize] = 125;
    a[(crate::world::GRAVEL) as usize] = 118;
    a[(crate::world::SANDSTONE) as usize] = 535;
    a[(crate::world::BIRCH_LOG) as usize] = 137;
    a[(crate::world::BIRCH_LEAVES) as usize] = 293;
    a[(crate::world::SPRUCE_LOG) as usize] = 134;
    a[(crate::world::SPRUCE_LEAVES) as usize] = 265;
    a[(crate::world::CACTUS) as usize] = 5782;
    a[(crate::world::TALLGRASS) as usize] = 2005;
    a[(crate::world::FLOWER_RED) as usize] = 2077;
    a[(crate::world::FLOWER_YELLOW) as usize] = 2075;
    a[(crate::world::MUSHROOM) as usize] = 2090;
    a[(crate::world::DEADBUSH) as usize] = 2007;
    a[(crate::world::PUMPKIN) as usize] = 6811;
    a[(crate::world::GLOWSTONE) as usize] = 5863;
    a[(crate::world::WOOL_COLOR_BASE + 0) as usize] = 2047;
    a[(crate::world::CONCRETE_BASE + 0) as usize] = 12728;
    a[(crate::world::CUSHION_BASE + 0) as usize] = 10728;
    a[(crate::world::WOOL_SLAB_BASE + 0) as usize] = 2047;
    a[(crate::world::WOOL_STAIRS_BASE + 0) as usize] = 2047;
    a[(crate::world::CONC_SLAB_BASE + 0) as usize] = 12728;
    a[(crate::world::CONC_STAIRS_BASE + 0) as usize] = 12728;
    a[(crate::world::WOOL_COLOR_BASE + 1) as usize] = 2048;
    a[(crate::world::CONCRETE_BASE + 1) as usize] = 12729;
    a[(crate::world::CUSHION_BASE + 1) as usize] = 10729;
    a[(crate::world::WOOL_SLAB_BASE + 1) as usize] = 2048;
    a[(crate::world::WOOL_STAIRS_BASE + 1) as usize] = 2048;
    a[(crate::world::CONC_SLAB_BASE + 1) as usize] = 12729;
    a[(crate::world::CONC_STAIRS_BASE + 1) as usize] = 12729;
    a[(crate::world::WOOL_COLOR_BASE + 2) as usize] = 2049;
    a[(crate::world::CONCRETE_BASE + 2) as usize] = 12730;
    a[(crate::world::CUSHION_BASE + 2) as usize] = 10730;
    a[(crate::world::WOOL_SLAB_BASE + 2) as usize] = 2049;
    a[(crate::world::WOOL_STAIRS_BASE + 2) as usize] = 2049;
    a[(crate::world::CONC_SLAB_BASE + 2) as usize] = 12730;
    a[(crate::world::CONC_STAIRS_BASE + 2) as usize] = 12730;
    a[(crate::world::WOOL_COLOR_BASE + 3) as usize] = 2050;
    a[(crate::world::CONCRETE_BASE + 3) as usize] = 12731;
    a[(crate::world::CUSHION_BASE + 3) as usize] = 10731;
    a[(crate::world::WOOL_SLAB_BASE + 3) as usize] = 2050;
    a[(crate::world::WOOL_STAIRS_BASE + 3) as usize] = 2050;
    a[(crate::world::CONC_SLAB_BASE + 3) as usize] = 12731;
    a[(crate::world::CONC_STAIRS_BASE + 3) as usize] = 12731;
    a[(crate::world::WOOL_COLOR_BASE + 4) as usize] = 2051;
    a[(crate::world::CONCRETE_BASE + 4) as usize] = 12732;
    a[(crate::world::CUSHION_BASE + 4) as usize] = 10732;
    a[(crate::world::WOOL_SLAB_BASE + 4) as usize] = 2051;
    a[(crate::world::WOOL_STAIRS_BASE + 4) as usize] = 2051;
    a[(crate::world::CONC_SLAB_BASE + 4) as usize] = 12732;
    a[(crate::world::CONC_STAIRS_BASE + 4) as usize] = 12732;
    a[(crate::world::WOOL_COLOR_BASE + 5) as usize] = 2052;
    a[(crate::world::CONCRETE_BASE + 5) as usize] = 12733;
    a[(crate::world::CUSHION_BASE + 5) as usize] = 10733;
    a[(crate::world::WOOL_SLAB_BASE + 5) as usize] = 2052;
    a[(crate::world::WOOL_STAIRS_BASE + 5) as usize] = 2052;
    a[(crate::world::CONC_SLAB_BASE + 5) as usize] = 12733;
    a[(crate::world::CONC_STAIRS_BASE + 5) as usize] = 12733;
    a[(crate::world::WOOL_COLOR_BASE + 6) as usize] = 2053;
    a[(crate::world::CONCRETE_BASE + 6) as usize] = 12734;
    a[(crate::world::CUSHION_BASE + 6) as usize] = 10734;
    a[(crate::world::WOOL_SLAB_BASE + 6) as usize] = 2053;
    a[(crate::world::WOOL_STAIRS_BASE + 6) as usize] = 2053;
    a[(crate::world::CONC_SLAB_BASE + 6) as usize] = 12734;
    a[(crate::world::CONC_STAIRS_BASE + 6) as usize] = 12734;
    a[(crate::world::WOOL_COLOR_BASE + 7) as usize] = 2054;
    a[(crate::world::CONCRETE_BASE + 7) as usize] = 12735;
    a[(crate::world::CUSHION_BASE + 7) as usize] = 10735;
    a[(crate::world::WOOL_SLAB_BASE + 7) as usize] = 2054;
    a[(crate::world::WOOL_STAIRS_BASE + 7) as usize] = 2054;
    a[(crate::world::CONC_SLAB_BASE + 7) as usize] = 12735;
    a[(crate::world::CONC_STAIRS_BASE + 7) as usize] = 12735;
    a[(crate::world::WOOL_COLOR_BASE + 8) as usize] = 2055;
    a[(crate::world::CONCRETE_BASE + 8) as usize] = 12736;
    a[(crate::world::CUSHION_BASE + 8) as usize] = 10736;
    a[(crate::world::WOOL_SLAB_BASE + 8) as usize] = 2055;
    a[(crate::world::WOOL_STAIRS_BASE + 8) as usize] = 2055;
    a[(crate::world::CONC_SLAB_BASE + 8) as usize] = 12736;
    a[(crate::world::CONC_STAIRS_BASE + 8) as usize] = 12736;
    a[(crate::world::WOOL_COLOR_BASE + 9) as usize] = 2056;
    a[(crate::world::CONCRETE_BASE + 9) as usize] = 12737;
    a[(crate::world::CUSHION_BASE + 9) as usize] = 10737;
    a[(crate::world::WOOL_SLAB_BASE + 9) as usize] = 2056;
    a[(crate::world::WOOL_STAIRS_BASE + 9) as usize] = 2056;
    a[(crate::world::CONC_SLAB_BASE + 9) as usize] = 12737;
    a[(crate::world::CONC_STAIRS_BASE + 9) as usize] = 12737;
    a[(crate::world::WOOL_COLOR_BASE + 10) as usize] = 2057;
    a[(crate::world::CONCRETE_BASE + 10) as usize] = 12738;
    a[(crate::world::CUSHION_BASE + 10) as usize] = 10738;
    a[(crate::world::WOOL_SLAB_BASE + 10) as usize] = 2057;
    a[(crate::world::WOOL_STAIRS_BASE + 10) as usize] = 2057;
    a[(crate::world::CONC_SLAB_BASE + 10) as usize] = 12738;
    a[(crate::world::CONC_STAIRS_BASE + 10) as usize] = 12738;
    a[(crate::world::WOOL_COLOR_BASE + 11) as usize] = 2058;
    a[(crate::world::CONCRETE_BASE + 11) as usize] = 12739;
    a[(crate::world::CUSHION_BASE + 11) as usize] = 10739;
    a[(crate::world::WOOL_SLAB_BASE + 11) as usize] = 2058;
    a[(crate::world::WOOL_STAIRS_BASE + 11) as usize] = 2058;
    a[(crate::world::CONC_SLAB_BASE + 11) as usize] = 12739;
    a[(crate::world::CONC_STAIRS_BASE + 11) as usize] = 12739;
    a[(crate::world::WOOL_COLOR_BASE + 12) as usize] = 2059;
    a[(crate::world::CONCRETE_BASE + 12) as usize] = 12740;
    a[(crate::world::CUSHION_BASE + 12) as usize] = 10740;
    a[(crate::world::WOOL_SLAB_BASE + 12) as usize] = 2059;
    a[(crate::world::WOOL_STAIRS_BASE + 12) as usize] = 2059;
    a[(crate::world::CONC_SLAB_BASE + 12) as usize] = 12740;
    a[(crate::world::CONC_STAIRS_BASE + 12) as usize] = 12740;
    a[(crate::world::WOOL_COLOR_BASE + 13) as usize] = 2060;
    a[(crate::world::CONCRETE_BASE + 13) as usize] = 12741;
    a[(crate::world::CUSHION_BASE + 13) as usize] = 10741;
    a[(crate::world::WOOL_SLAB_BASE + 13) as usize] = 2060;
    a[(crate::world::WOOL_STAIRS_BASE + 13) as usize] = 2060;
    a[(crate::world::CONC_SLAB_BASE + 13) as usize] = 12741;
    a[(crate::world::CONC_STAIRS_BASE + 13) as usize] = 12741;
    a[(crate::world::WOOL_COLOR_BASE + 14) as usize] = 2061;
    a[(crate::world::CONCRETE_BASE + 14) as usize] = 12742;
    a[(crate::world::CUSHION_BASE + 14) as usize] = 10742;
    a[(crate::world::WOOL_SLAB_BASE + 14) as usize] = 2061;
    a[(crate::world::WOOL_STAIRS_BASE + 14) as usize] = 2061;
    a[(crate::world::CONC_SLAB_BASE + 14) as usize] = 12742;
    a[(crate::world::CONC_STAIRS_BASE + 14) as usize] = 12742;
    a[(crate::world::WOOL_COLOR_BASE + 15) as usize] = 2062;
    a[(crate::world::CONCRETE_BASE + 15) as usize] = 12743;
    a[(crate::world::CUSHION_BASE + 15) as usize] = 10743;
    a[(crate::world::WOOL_SLAB_BASE + 15) as usize] = 2062;
    a[(crate::world::WOOL_STAIRS_BASE + 15) as usize] = 2062;
    a[(crate::world::CONC_SLAB_BASE + 15) as usize] = 12743;
    a[(crate::world::CONC_STAIRS_BASE + 15) as usize] = 12743;
    a[(crate::world::POPLAR_LOG) as usize] = 137;
    a[(crate::world::POP_LEAVES_R) as usize] = 461;
    a[(crate::world::POP_LEAVES_O) as usize] = 237;
    a[(crate::world::POP_LEAVES_Y) as usize] = 293;
    a[(crate::world::POPLAR_PLANKS) as usize] = 17;
    a[(crate::world::RED_SHRUB) as usize] = 18578;
    a[(crate::world::SHELF_SM) as usize] = 18416;
    a[(crate::world::SHELF_LG) as usize] = 18416;
    a[(crate::world::HAY) as usize] = 10726;
    a[(crate::world::STRAW_BED) as usize] = 1689;
    a[(crate::world::CAMPFIRE) as usize] = 18515;
    a[(crate::world::BARREL) as usize] = 18416;
    a[(crate::world::GRASS_PALE) as usize] = 2005;
    a[(crate::world::RED_SAND) as usize] = 117;
    a[(crate::world::RED_SANDSTONE) as usize] = 11079;
    a[(crate::world::PODZOL) as usize] = 12;
    a[(crate::world::COARSE_DIRT) as usize] = 11;
    a[(crate::world::PACKED_ICE) as usize] = 10746;
    a[(crate::world::GRANITE) as usize] = 2;
    a[(crate::world::DIORITE) as usize] = 4;
    a[(crate::world::ANDESITE) as usize] = 6;
    a[(crate::world::SPONGE) as usize] = 517;
    a[(crate::world::MAGMA) as usize] = 12543;
    a[(crate::world::SEAGRASS) as usize] = 2008;
    a[(crate::world::KELP) as usize] = 12760;
    a[(crate::world::CORAL_PINK) as usize] = 12809;
    a[(crate::world::CORAL_BLUE) as usize] = 12808;
    a[(crate::world::CORAL_DEAD) as usize] = 12803;
    a[(crate::world::LILYPAD) as usize] = 7271;
    a[(crate::world::SUGARCANE) as usize] = 5799;
    a[(crate::world::BERRY_BUSH) as usize] = 18578;
    a[(crate::world::BAMBOO) as usize] = 12945;
    a[(crate::world::BAMBOO_BLOCK) as usize] = 160;
    a[(crate::world::HONEY) as usize] = 19445;
    a[(crate::world::BEEHIVE) as usize] = 19421;
    a[(crate::world::BASALT) as usize] = 5853;
    a[(crate::world::BLACKSTONE) as usize] = 19460;
    a[(crate::world::COPPER_ORE) as usize] = 22942;
    a[(crate::world::COPPER_BLOCK) as usize] = 22938;
    a[(crate::world::AMETHYST) as usize] = 21031;
    a[(crate::world::CALCITE) as usize] = 22316;
    a[(crate::world::TUFF) as usize] = 21081;
    a[(crate::world::DEEPSLATE) as usize] = 24905;
    a[(crate::world::DRIPSTONE) as usize] = 24768;
    a[(crate::world::MOSS) as usize] = 24843;
    a[(crate::world::AZALEA) as usize] = 24824;
    a[(crate::world::AZALEA_FLOWER) as usize] = 24825;
    a[(crate::world::GLOW_BERRIES) as usize] = 24822;
    a[(crate::world::MUD) as usize] = 24903;
    a[(crate::world::PACKED_MUD) as usize] = 6541;
    a[(crate::world::MUD_BRICKS) as usize] = 6542;
    a[(crate::world::SCULK) as usize] = 22799;
    a[(crate::world::MANGROVE_LOG) as usize] = 152;
    a[(crate::world::MANGROVE_LEAVES) as usize] = 433;
    a[(crate::world::MANGROVE_PLANKS) as usize] = 22;
    a[(crate::world::CHERRY_LOG) as usize] = 146;
    a[(crate::world::CHERRY_LEAVES) as usize] = 377;
    a[(crate::world::CHERRY_PLANKS) as usize] = 20;
    a[(crate::world::PINK_PETALS) as usize] = 24827;
    a[(crate::world::ACACIA_LOG) as usize] = 143;
    a[(crate::world::ACACIA_LEAVES) as usize] = 349;
    a[(crate::world::ACACIA_PLANKS) as usize] = 19;
    a[(crate::world::MELON) as usize] = 6812;
    a[(crate::world::TERRACOTTA) as usize] = 10744;
    a[(crate::world::TERRACOTTA_RED) as usize] = 9370;
    a[(crate::world::TERRACOTTA_ORANGE) as usize] = 9357;
    a[(crate::world::TERRACOTTA_YELLOW) as usize] = 9360;
    a[(crate::world::GOLD_ORE) as usize] = 123;
    a[(crate::world::DIAMOND_ORE) as usize] = 4274;
    a[(crate::world::COPPER_BULB) as usize] = 24692;
    a[(crate::world::OBSIDIAN) as usize] = 2354;
    a[(crate::world::CRYING_OBSIDIAN) as usize] = 19449;
    a[(crate::world::TORCH) as usize] = 2355;
    a[(crate::world::SNOW_LAYER) as usize] = 5772;
    a[(crate::world::DRIPSTONE_SPIKE) as usize] = 24752;
    a[(crate::world::OAK_FENCE) as usize] = 5817;
    a
};

/// Id d'etat vanilla global pour un bloc rustvoxel.
pub fn block_state_id(b: u16) -> u32 {
    STATE_OF[(b as usize).min(223)]
}

/// Table (bloc, etat vanilla) des blocs placables - carte inverse.
pub const BLOCK_STATES: [(u16, u32); 219] = [
    (crate::world::AIR, 0),
    (crate::world::GRASS, 8),
    (crate::world::DIRT, 10),
    (crate::world::WOOL, 2047),
    (crate::world::STONE, 1),
    (crate::world::COBBLE, 14),
    (crate::world::PLANKS, 15),
    (crate::world::LOG, 131),
    (crate::world::LEAVES, 237),
    (crate::world::SAND, 112),
    (crate::world::WATER, 80),
    (crate::world::GLASS, 519),
    (crate::world::BRICK, 2093),
    (crate::world::SNOW, 5781),
    (crate::world::BEDROCK, 79),
    (crate::world::COAL, 127),
    (crate::world::IRON, 125),
    (crate::world::GRAVEL, 118),
    (crate::world::SANDSTONE, 535),
    (crate::world::BIRCH_LOG, 137),
    (crate::world::BIRCH_LEAVES, 293),
    (crate::world::SPRUCE_LOG, 134),
    (crate::world::SPRUCE_LEAVES, 265),
    (crate::world::CACTUS, 5782),
    (crate::world::TALLGRASS, 2005),
    (crate::world::FLOWER_RED, 2077),
    (crate::world::FLOWER_YELLOW, 2075),
    (crate::world::MUSHROOM, 2090),
    (crate::world::DEADBUSH, 2007),
    (crate::world::PUMPKIN, 6811),
    (crate::world::GLOWSTONE, 5863),
    (crate::world::WOOL_COLOR_BASE + 0, 2047),
    (crate::world::CONCRETE_BASE + 0, 12728),
    (crate::world::CUSHION_BASE + 0, 10728),
    (crate::world::WOOL_SLAB_BASE + 0, 2047),
    (crate::world::WOOL_STAIRS_BASE + 0, 2047),
    (crate::world::CONC_SLAB_BASE + 0, 12728),
    (crate::world::CONC_STAIRS_BASE + 0, 12728),
    (crate::world::WOOL_COLOR_BASE + 1, 2048),
    (crate::world::CONCRETE_BASE + 1, 12729),
    (crate::world::CUSHION_BASE + 1, 10729),
    (crate::world::WOOL_SLAB_BASE + 1, 2048),
    (crate::world::WOOL_STAIRS_BASE + 1, 2048),
    (crate::world::CONC_SLAB_BASE + 1, 12729),
    (crate::world::CONC_STAIRS_BASE + 1, 12729),
    (crate::world::WOOL_COLOR_BASE + 2, 2049),
    (crate::world::CONCRETE_BASE + 2, 12730),
    (crate::world::CUSHION_BASE + 2, 10730),
    (crate::world::WOOL_SLAB_BASE + 2, 2049),
    (crate::world::WOOL_STAIRS_BASE + 2, 2049),
    (crate::world::CONC_SLAB_BASE + 2, 12730),
    (crate::world::CONC_STAIRS_BASE + 2, 12730),
    (crate::world::WOOL_COLOR_BASE + 3, 2050),
    (crate::world::CONCRETE_BASE + 3, 12731),
    (crate::world::CUSHION_BASE + 3, 10731),
    (crate::world::WOOL_SLAB_BASE + 3, 2050),
    (crate::world::WOOL_STAIRS_BASE + 3, 2050),
    (crate::world::CONC_SLAB_BASE + 3, 12731),
    (crate::world::CONC_STAIRS_BASE + 3, 12731),
    (crate::world::WOOL_COLOR_BASE + 4, 2051),
    (crate::world::CONCRETE_BASE + 4, 12732),
    (crate::world::CUSHION_BASE + 4, 10732),
    (crate::world::WOOL_SLAB_BASE + 4, 2051),
    (crate::world::WOOL_STAIRS_BASE + 4, 2051),
    (crate::world::CONC_SLAB_BASE + 4, 12732),
    (crate::world::CONC_STAIRS_BASE + 4, 12732),
    (crate::world::WOOL_COLOR_BASE + 5, 2052),
    (crate::world::CONCRETE_BASE + 5, 12733),
    (crate::world::CUSHION_BASE + 5, 10733),
    (crate::world::WOOL_SLAB_BASE + 5, 2052),
    (crate::world::WOOL_STAIRS_BASE + 5, 2052),
    (crate::world::CONC_SLAB_BASE + 5, 12733),
    (crate::world::CONC_STAIRS_BASE + 5, 12733),
    (crate::world::WOOL_COLOR_BASE + 6, 2053),
    (crate::world::CONCRETE_BASE + 6, 12734),
    (crate::world::CUSHION_BASE + 6, 10734),
    (crate::world::WOOL_SLAB_BASE + 6, 2053),
    (crate::world::WOOL_STAIRS_BASE + 6, 2053),
    (crate::world::CONC_SLAB_BASE + 6, 12734),
    (crate::world::CONC_STAIRS_BASE + 6, 12734),
    (crate::world::WOOL_COLOR_BASE + 7, 2054),
    (crate::world::CONCRETE_BASE + 7, 12735),
    (crate::world::CUSHION_BASE + 7, 10735),
    (crate::world::WOOL_SLAB_BASE + 7, 2054),
    (crate::world::WOOL_STAIRS_BASE + 7, 2054),
    (crate::world::CONC_SLAB_BASE + 7, 12735),
    (crate::world::CONC_STAIRS_BASE + 7, 12735),
    (crate::world::WOOL_COLOR_BASE + 8, 2055),
    (crate::world::CONCRETE_BASE + 8, 12736),
    (crate::world::CUSHION_BASE + 8, 10736),
    (crate::world::WOOL_SLAB_BASE + 8, 2055),
    (crate::world::WOOL_STAIRS_BASE + 8, 2055),
    (crate::world::CONC_SLAB_BASE + 8, 12736),
    (crate::world::CONC_STAIRS_BASE + 8, 12736),
    (crate::world::WOOL_COLOR_BASE + 9, 2056),
    (crate::world::CONCRETE_BASE + 9, 12737),
    (crate::world::CUSHION_BASE + 9, 10737),
    (crate::world::WOOL_SLAB_BASE + 9, 2056),
    (crate::world::WOOL_STAIRS_BASE + 9, 2056),
    (crate::world::CONC_SLAB_BASE + 9, 12737),
    (crate::world::CONC_STAIRS_BASE + 9, 12737),
    (crate::world::WOOL_COLOR_BASE + 10, 2057),
    (crate::world::CONCRETE_BASE + 10, 12738),
    (crate::world::CUSHION_BASE + 10, 10738),
    (crate::world::WOOL_SLAB_BASE + 10, 2057),
    (crate::world::WOOL_STAIRS_BASE + 10, 2057),
    (crate::world::CONC_SLAB_BASE + 10, 12738),
    (crate::world::CONC_STAIRS_BASE + 10, 12738),
    (crate::world::WOOL_COLOR_BASE + 11, 2058),
    (crate::world::CONCRETE_BASE + 11, 12739),
    (crate::world::CUSHION_BASE + 11, 10739),
    (crate::world::WOOL_SLAB_BASE + 11, 2058),
    (crate::world::WOOL_STAIRS_BASE + 11, 2058),
    (crate::world::CONC_SLAB_BASE + 11, 12739),
    (crate::world::CONC_STAIRS_BASE + 11, 12739),
    (crate::world::WOOL_COLOR_BASE + 12, 2059),
    (crate::world::CONCRETE_BASE + 12, 12740),
    (crate::world::CUSHION_BASE + 12, 10740),
    (crate::world::WOOL_SLAB_BASE + 12, 2059),
    (crate::world::WOOL_STAIRS_BASE + 12, 2059),
    (crate::world::CONC_SLAB_BASE + 12, 12740),
    (crate::world::CONC_STAIRS_BASE + 12, 12740),
    (crate::world::WOOL_COLOR_BASE + 13, 2060),
    (crate::world::CONCRETE_BASE + 13, 12741),
    (crate::world::CUSHION_BASE + 13, 10741),
    (crate::world::WOOL_SLAB_BASE + 13, 2060),
    (crate::world::WOOL_STAIRS_BASE + 13, 2060),
    (crate::world::CONC_SLAB_BASE + 13, 12741),
    (crate::world::CONC_STAIRS_BASE + 13, 12741),
    (crate::world::WOOL_COLOR_BASE + 14, 2061),
    (crate::world::CONCRETE_BASE + 14, 12742),
    (crate::world::CUSHION_BASE + 14, 10742),
    (crate::world::WOOL_SLAB_BASE + 14, 2061),
    (crate::world::WOOL_STAIRS_BASE + 14, 2061),
    (crate::world::CONC_SLAB_BASE + 14, 12742),
    (crate::world::CONC_STAIRS_BASE + 14, 12742),
    (crate::world::WOOL_COLOR_BASE + 15, 2062),
    (crate::world::CONCRETE_BASE + 15, 12743),
    (crate::world::CUSHION_BASE + 15, 10743),
    (crate::world::WOOL_SLAB_BASE + 15, 2062),
    (crate::world::WOOL_STAIRS_BASE + 15, 2062),
    (crate::world::CONC_SLAB_BASE + 15, 12743),
    (crate::world::CONC_STAIRS_BASE + 15, 12743),
    (crate::world::POPLAR_LOG, 137),
    (crate::world::POP_LEAVES_R, 461),
    (crate::world::POP_LEAVES_O, 237),
    (crate::world::POP_LEAVES_Y, 293),
    (crate::world::POPLAR_PLANKS, 17),
    (crate::world::RED_SHRUB, 18578),
    (crate::world::SHELF_SM, 18416),
    (crate::world::SHELF_LG, 18416),
    (crate::world::HAY, 10726),
    (crate::world::STRAW_BED, 1689),
    (crate::world::CAMPFIRE, 18515),
    (crate::world::BARREL, 18416),
    (crate::world::GRASS_PALE, 2005),
    (crate::world::RED_SAND, 117),
    (crate::world::RED_SANDSTONE, 11079),
    (crate::world::PODZOL, 12),
    (crate::world::COARSE_DIRT, 11),
    (crate::world::PACKED_ICE, 10746),
    (crate::world::GRANITE, 2),
    (crate::world::DIORITE, 4),
    (crate::world::ANDESITE, 6),
    (crate::world::SPONGE, 517),
    (crate::world::MAGMA, 12543),
    (crate::world::SEAGRASS, 2008),
    (crate::world::KELP, 12760),
    (crate::world::CORAL_PINK, 12809),
    (crate::world::CORAL_BLUE, 12808),
    (crate::world::CORAL_DEAD, 12803),
    (crate::world::LILYPAD, 7271),
    (crate::world::SUGARCANE, 5799),
    (crate::world::BERRY_BUSH, 18578),
    (crate::world::BAMBOO, 12945),
    (crate::world::BAMBOO_BLOCK, 160),
    (crate::world::HONEY, 19445),
    (crate::world::BEEHIVE, 19421),
    (crate::world::BASALT, 5853),
    (crate::world::BLACKSTONE, 19460),
    (crate::world::COPPER_ORE, 22942),
    (crate::world::COPPER_BLOCK, 22938),
    (crate::world::AMETHYST, 21031),
    (crate::world::CALCITE, 22316),
    (crate::world::TUFF, 21081),
    (crate::world::DEEPSLATE, 24905),
    (crate::world::DRIPSTONE, 24768),
    (crate::world::MOSS, 24843),
    (crate::world::AZALEA, 24824),
    (crate::world::AZALEA_FLOWER, 24825),
    (crate::world::GLOW_BERRIES, 24822),
    (crate::world::MUD, 24903),
    (crate::world::PACKED_MUD, 6541),
    (crate::world::MUD_BRICKS, 6542),
    (crate::world::SCULK, 22799),
    (crate::world::MANGROVE_LOG, 152),
    (crate::world::MANGROVE_LEAVES, 433),
    (crate::world::MANGROVE_PLANKS, 22),
    (crate::world::CHERRY_LOG, 146),
    (crate::world::CHERRY_LEAVES, 377),
    (crate::world::CHERRY_PLANKS, 20),
    (crate::world::PINK_PETALS, 24827),
    (crate::world::ACACIA_LOG, 143),
    (crate::world::ACACIA_LEAVES, 349),
    (crate::world::ACACIA_PLANKS, 19),
    (crate::world::MELON, 6812),
    (crate::world::TERRACOTTA, 10744),
    (crate::world::TERRACOTTA_RED, 9370),
    (crate::world::TERRACOTTA_ORANGE, 9357),
    (crate::world::TERRACOTTA_YELLOW, 9360),
    (crate::world::GOLD_ORE, 123),
    (crate::world::DIAMOND_ORE, 4274),
    (crate::world::COPPER_BULB, 24692),
    (crate::world::OBSIDIAN, 2354),
    (crate::world::CRYING_OBSIDIAN, 19449),
    (crate::world::TORCH, 2355),
    (crate::world::SNOW_LAYER, 5772),
    (crate::world::DRIPSTONE_SPIKE, 24752),
    (crate::world::OAK_FENCE, 5817),
];

/// Proprietes canoniques d'un bloc (pour choisir la variante blockstate).
pub fn block_props(b: u16) -> &'static [(&'static str, &'static str)] {
    match b {
        b if b >= crate::world::WOOL_COLOR_BASE && b < crate::world::WOOL_COLOR_BASE + 16 => &[],
        b if b >= crate::world::CONCRETE_BASE && b < crate::world::CONCRETE_BASE + 16 => &[],
        b if b >= crate::world::CUSHION_BASE && b < crate::world::CUSHION_BASE + 16 => &[],
        b if b >= crate::world::WOOL_SLAB_BASE && b < crate::world::WOOL_SLAB_BASE + 16 => &[],
        b if b >= crate::world::WOOL_STAIRS_BASE && b < crate::world::WOOL_STAIRS_BASE + 16 => &[],
        b if b >= crate::world::CONC_SLAB_BASE && b < crate::world::CONC_SLAB_BASE + 16 => &[],
        b if b >= crate::world::CONC_STAIRS_BASE && b < crate::world::CONC_STAIRS_BASE + 16 => &[],
        crate::world::GRASS => &[("snowy", "false")],
        crate::world::LOG => &[("axis", "y")],
        crate::world::LEAVES => &[("distance", "1"), ("persistent", "false"), ("waterlogged", "false")],
        crate::world::WATER => &[("level", "0")],
        crate::world::BIRCH_LOG => &[("axis", "y")],
        crate::world::BIRCH_LEAVES => &[("distance", "1"), ("persistent", "false"), ("waterlogged", "false")],
        crate::world::SPRUCE_LOG => &[("axis", "y")],
        crate::world::SPRUCE_LEAVES => &[("distance", "1"), ("persistent", "false"), ("waterlogged", "false")],
        crate::world::CACTUS => &[("age", "0")],
        crate::world::PUMPKIN => &[("axis", "y")],
        crate::world::POPLAR_LOG => &[("axis", "y")],
        crate::world::POP_LEAVES_R => &[("distance", "1"), ("persistent", "false"), ("waterlogged", "false")],
        crate::world::POP_LEAVES_O => &[("distance", "1"), ("persistent", "false"), ("waterlogged", "false")],
        crate::world::POP_LEAVES_Y => &[("distance", "1"), ("persistent", "false"), ("waterlogged", "false")],
        crate::world::RED_SHRUB => &[("age", "3")],
        crate::world::SHELF_SM => &[("facing", "up"), ("open", "false")],
        crate::world::SHELF_LG => &[("facing", "up"), ("open", "false")],
        crate::world::HAY => &[("axis", "y")],
        crate::world::STRAW_BED => &[("facing", "north"), ("part", "foot"), ("occupied", "false")],
        crate::world::CAMPFIRE => &[("facing", "north"), ("lit", "true"), ("signal_fire", "false"), ("waterlogged", "false")],
        crate::world::BARREL => &[("facing", "up"), ("open", "false")],
        crate::world::PODZOL => &[("snowy", "false")],
        crate::world::KELP => &[("age", "0")],
        crate::world::SUGARCANE => &[("age", "0")],
        crate::world::BERRY_BUSH => &[("age", "3")],
        crate::world::BAMBOO => &[("age", "0"), ("bamboo_leaves", "none"), ("stage", "0"), ("thickness", "thin")],
        crate::world::BAMBOO_BLOCK => &[("axis", "y")],
        crate::world::BEEHIVE => &[("facing", "north"), ("honey_level", "0")],
        crate::world::BASALT => &[("axis", "y")],
        crate::world::DEEPSLATE => &[("axis", "y")],
        crate::world::GLOW_BERRIES => &[("berries", "true")],
        crate::world::MANGROVE_LOG => &[("axis", "y")],
        crate::world::MANGROVE_LEAVES => &[("distance", "1"), ("persistent", "false"), ("waterlogged", "false")],
        crate::world::CHERRY_LOG => &[("axis", "y")],
        crate::world::CHERRY_LEAVES => &[("distance", "1"), ("persistent", "false"), ("waterlogged", "false")],
        crate::world::PINK_PETALS => &[("facing", "north"), ("flower_amount", "1")],
        crate::world::ACACIA_LOG => &[("axis", "y")],
        crate::world::ACACIA_LEAVES => &[("distance", "1"), ("persistent", "false"), ("waterlogged", "false")],
        crate::world::COPPER_BULB => &[("lit", "false"), ("powered", "false")],
        crate::world::SNOW_LAYER => &[("layers", "1")],
        crate::world::DRIPSTONE_SPIKE => &[("thickness", "tip"), ("vertical_direction", "up"), ("waterlogged", "false")],
        crate::world::OAK_FENCE => &[("north", "false"), ("east", "false"), ("south", "false"), ("west", "false"), ("waterlogged", "false")],
        _ => &[],
    }
}

/// Nom vanilla du bloc (logs/debug).
pub fn block_vanilla_name(b: u16) -> &'static str {
    match b {
        b if b >= crate::world::WOOL_SLAB_BASE && b < crate::world::WOOL_SLAB_BASE + 16 => "white_wool",
        b if b >= crate::world::WOOL_STAIRS_BASE && b < crate::world::WOOL_STAIRS_BASE + 16 => "white_wool",
        b if b >= crate::world::CONC_SLAB_BASE && b < crate::world::CONC_SLAB_BASE + 16 => "white_concrete",
        b if b >= crate::world::CONC_STAIRS_BASE && b < crate::world::CONC_STAIRS_BASE + 16 => "white_concrete",
        b if b >= crate::world::WOOL_COLOR_BASE && b < crate::world::WOOL_COLOR_BASE + 16 => "white_wool",
        b if b >= crate::world::CONCRETE_BASE && b < crate::world::CONCRETE_BASE + 16 => "white_concrete",
        b if b >= crate::world::CUSHION_BASE && b < crate::world::CUSHION_BASE + 16 => "white_carpet",
        crate::world::AIR => "air",
        crate::world::GRASS => "grass_block",
        crate::world::DIRT => "dirt",
        crate::world::WOOL => "white_wool",
        crate::world::STONE => "stone",
        crate::world::COBBLE => "cobblestone",
        crate::world::PLANKS => "oak_planks",
        crate::world::LOG => "oak_log",
        crate::world::LEAVES => "oak_leaves",
        crate::world::SAND => "sand",
        crate::world::WATER => "water",
        crate::world::GLASS => "glass",
        crate::world::BRICK => "bricks",
        crate::world::SNOW => "snow_block",
        crate::world::BEDROCK => "bedrock",
        crate::world::COAL => "coal_ore",
        crate::world::IRON => "iron_ore",
        crate::world::GRAVEL => "gravel",
        crate::world::SANDSTONE => "sandstone",
        crate::world::BIRCH_LOG => "birch_log",
        crate::world::BIRCH_LEAVES => "birch_leaves",
        crate::world::SPRUCE_LOG => "spruce_log",
        crate::world::SPRUCE_LEAVES => "spruce_leaves",
        crate::world::CACTUS => "cactus",
        crate::world::TALLGRASS => "short_grass",
        crate::world::FLOWER_RED => "poppy",
        crate::world::FLOWER_YELLOW => "dandelion",
        crate::world::MUSHROOM => "red_mushroom",
        crate::world::DEADBUSH => "dead_bush",
        crate::world::PUMPKIN => "pumpkin",
        crate::world::GLOWSTONE => "glowstone",
        crate::world::POPLAR_LOG => "birch_log",
        crate::world::POP_LEAVES_R => "azalea_leaves",
        crate::world::POP_LEAVES_O => "oak_leaves",
        crate::world::POP_LEAVES_Y => "birch_leaves",
        crate::world::POPLAR_PLANKS => "birch_planks",
        crate::world::RED_SHRUB => "sweet_berry_bush",
        crate::world::SHELF_SM => "barrel",
        crate::world::SHELF_LG => "barrel",
        crate::world::HAY => "hay_block",
        crate::world::STRAW_BED => "white_bed",
        crate::world::CAMPFIRE => "campfire",
        crate::world::BARREL => "barrel",
        crate::world::GRASS_PALE => "short_grass",
        crate::world::RED_SAND => "red_sand",
        crate::world::RED_SANDSTONE => "red_sandstone",
        crate::world::PODZOL => "podzol",
        crate::world::COARSE_DIRT => "coarse_dirt",
        crate::world::PACKED_ICE => "packed_ice",
        crate::world::GRANITE => "granite",
        crate::world::DIORITE => "diorite",
        crate::world::ANDESITE => "andesite",
        crate::world::SPONGE => "sponge",
        crate::world::MAGMA => "magma_block",
        crate::world::SEAGRASS => "seagrass",
        crate::world::KELP => "kelp",
        crate::world::CORAL_PINK => "brain_coral_block",
        crate::world::CORAL_BLUE => "tube_coral_block",
        crate::world::CORAL_DEAD => "dead_tube_coral_block",
        crate::world::LILYPAD => "lily_pad",
        crate::world::SUGARCANE => "sugar_cane",
        crate::world::BERRY_BUSH => "sweet_berry_bush",
        crate::world::BAMBOO => "bamboo",
        crate::world::BAMBOO_BLOCK => "bamboo_block",
        crate::world::HONEY => "honey_block",
        crate::world::BEEHIVE => "beehive",
        crate::world::BASALT => "basalt",
        crate::world::BLACKSTONE => "blackstone",
        crate::world::COPPER_ORE => "copper_ore",
        crate::world::COPPER_BLOCK => "copper_block",
        crate::world::AMETHYST => "amethyst_block",
        crate::world::CALCITE => "calcite",
        crate::world::TUFF => "tuff",
        crate::world::DEEPSLATE => "deepslate",
        crate::world::DRIPSTONE => "dripstone_block",
        crate::world::MOSS => "moss_block",
        crate::world::AZALEA => "azalea",
        crate::world::AZALEA_FLOWER => "flowering_azalea",
        crate::world::GLOW_BERRIES => "cave_vines_plant",
        crate::world::MUD => "mud",
        crate::world::PACKED_MUD => "packed_mud",
        crate::world::MUD_BRICKS => "mud_bricks",
        crate::world::SCULK => "sculk",
        crate::world::MANGROVE_LOG => "mangrove_log",
        crate::world::MANGROVE_LEAVES => "mangrove_leaves",
        crate::world::MANGROVE_PLANKS => "mangrove_planks",
        crate::world::CHERRY_LOG => "cherry_log",
        crate::world::CHERRY_LEAVES => "cherry_leaves",
        crate::world::CHERRY_PLANKS => "cherry_planks",
        crate::world::PINK_PETALS => "pink_petals",
        crate::world::ACACIA_LOG => "acacia_log",
        crate::world::ACACIA_LEAVES => "acacia_leaves",
        crate::world::ACACIA_PLANKS => "acacia_planks",
        crate::world::MELON => "melon",
        crate::world::TERRACOTTA => "terracotta",
        crate::world::TERRACOTTA_RED => "red_terracotta",
        crate::world::TERRACOTTA_ORANGE => "orange_terracotta",
        crate::world::TERRACOTTA_YELLOW => "yellow_terracotta",
        crate::world::GOLD_ORE => "gold_ore",
        crate::world::DIAMOND_ORE => "diamond_ore",
        crate::world::COPPER_BULB => "copper_bulb",
        crate::world::OBSIDIAN => "obsidian",
        crate::world::CRYING_OBSIDIAN => "crying_obsidian",
        crate::world::TORCH => "torch",
        crate::world::SNOW_LAYER => "snow",
        crate::world::DRIPSTONE_SPIKE => "pointed_dripstone",
        crate::world::OAK_FENCE => "oak_fence",
        _ => "stone",
    }
}

/// Item vanilla (id) placé par ce bloc, pour le mode créatif.
pub fn item_id_for_block(b: u16) -> Option<i32> {
    let name = match b {
        crate::world::GRASS => "grass_block",
        crate::world::DIRT => "dirt",
        crate::world::STONE => "stone",
        crate::world::COBBLE => "cobblestone",
        crate::world::PLANKS => "oak_planks",
        crate::world::LOG => "oak_log",
        crate::world::LEAVES => "oak_leaves",
        crate::world::SAND => "sand",
        crate::world::GLASS => "glass",
        crate::world::BRICK => "bricks",
        crate::world::SNOW => "snow_block",
        crate::world::BEDROCK => "bedrock",
        crate::world::COAL => "coal_ore",
        crate::world::IRON => "iron_ore",
        crate::world::GRAVEL => "gravel",
        crate::world::SANDSTONE => "sandstone",
        crate::world::TORCH => "torch",
        crate::world::OAK_FENCE => "oak_fence",
        crate::world::SNOW_LAYER => "snow",
        crate::world::GLOWSTONE => "glowstone",
        crate::world::PUMPKIN => "pumpkin",
        crate::world::MELON => "melon",
        crate::world::HAY => "hay_block",
        crate::world::CHERRY_PLANKS => "cherry_planks",
        crate::world::DEEPSLATE => "deepslate",
        crate::world::COPPER_BLOCK => "copper_block",
        crate::world::AMETHYST => "amethyst_block",
        crate::world::OBSIDIAN => "obsidian",
        crate::world::MAGMA => "magma_block",
        crate::world::SPONGE => "sponge",
        crate::world::WOOL => "white_wool",
        crate::world::MOSS => "moss_block",
        crate::world::SCULK => "sculk",
        crate::world::TERRACOTTA => "terracotta",
        _ => return None,
    };
    item_id_by_name(name)
}

/// Bloc rustvoxel posé par un item vanilla (mode créatif).
pub fn block_from_item_id(item: i32) -> Option<u16> {
    let b = match item {
        1 => crate::world::STONE,
        8 => crate::world::DEEPSLATE,
        27 => crate::world::GRASS,
        28 => crate::world::DIRT,
        35 => crate::world::COBBLE,
        36 => crate::world::PLANKS,
        41 => crate::world::CHERRY_PLANKS,
        56 => crate::world::BEDROCK,
        57 => crate::world::SAND,
        61 => crate::world::GRAVEL,
        62 => crate::world::COAL,
        64 => crate::world::IRON,
        86 => crate::world::AMETHYST,
        89 => crate::world::COPPER_BLOCK,
        132 => crate::world::LOG,
        176 => crate::world::LEAVES,
        186 => crate::world::SPONGE,
        188 => crate::world::GLASS,
        191 => crate::world::SANDSTONE,
        202 => crate::world::WOOL_COLOR_BASE + 0,
        203 => crate::world::WOOL_COLOR_BASE + 1,
        204 => crate::world::WOOL_COLOR_BASE + 2,
        205 => crate::world::WOOL_COLOR_BASE + 3,
        206 => crate::world::WOOL_COLOR_BASE + 4,
        207 => crate::world::WOOL_COLOR_BASE + 5,
        208 => crate::world::WOOL_COLOR_BASE + 6,
        209 => crate::world::WOOL_COLOR_BASE + 7,
        210 => crate::world::WOOL_COLOR_BASE + 8,
        211 => crate::world::WOOL_COLOR_BASE + 9,
        212 => crate::world::WOOL_COLOR_BASE + 10,
        213 => crate::world::WOOL_COLOR_BASE + 11,
        214 => crate::world::WOOL_COLOR_BASE + 12,
        215 => crate::world::WOOL_COLOR_BASE + 13,
        216 => crate::world::WOOL_COLOR_BASE + 14,
        217 => crate::world::WOOL_COLOR_BASE + 15,
        247 => crate::world::MOSS,
        285 => crate::world::BRICK,
        290 => crate::world::OBSIDIAN,
        291 => crate::world::TORCH,
        305 => crate::world::SNOW_LAYER,
        307 => crate::world::SNOW,
        311 => crate::world::OAK_FENCE,
        322 => crate::world::PUMPKIN,
        332 => crate::world::GLOWSTONE,
        358 => crate::world::MELON,
        371 => crate::world::SCULK,
        445 => crate::world::HAY,
        446 => crate::world::CUSHION_BASE + 0,
        447 => crate::world::CUSHION_BASE + 1,
        448 => crate::world::CUSHION_BASE + 2,
        449 => crate::world::CUSHION_BASE + 3,
        450 => crate::world::CUSHION_BASE + 4,
        451 => crate::world::CUSHION_BASE + 5,
        452 => crate::world::CUSHION_BASE + 6,
        453 => crate::world::CUSHION_BASE + 7,
        454 => crate::world::CUSHION_BASE + 8,
        455 => crate::world::CUSHION_BASE + 9,
        456 => crate::world::CUSHION_BASE + 10,
        457 => crate::world::CUSHION_BASE + 11,
        458 => crate::world::CUSHION_BASE + 12,
        459 => crate::world::CUSHION_BASE + 13,
        460 => crate::world::CUSHION_BASE + 14,
        461 => crate::world::CUSHION_BASE + 15,
        462 => crate::world::TERRACOTTA,
        516 => crate::world::MAGMA,
        555 => crate::world::CONCRETE_BASE + 0,
        556 => crate::world::CONCRETE_BASE + 1,
        557 => crate::world::CONCRETE_BASE + 2,
        558 => crate::world::CONCRETE_BASE + 3,
        559 => crate::world::CONCRETE_BASE + 4,
        560 => crate::world::CONCRETE_BASE + 5,
        561 => crate::world::CONCRETE_BASE + 6,
        562 => crate::world::CONCRETE_BASE + 7,
        563 => crate::world::CONCRETE_BASE + 8,
        564 => crate::world::CONCRETE_BASE + 9,
        565 => crate::world::CONCRETE_BASE + 10,
        566 => crate::world::CONCRETE_BASE + 11,
        567 => crate::world::CONCRETE_BASE + 12,
        568 => crate::world::CONCRETE_BASE + 13,
        569 => crate::world::CONCRETE_BASE + 14,
        570 => crate::world::CONCRETE_BASE + 15,
        _ => return None,
    };
    Some(b)
}

fn item_id_by_name(name: &str) -> Option<i32> {
    Some(match name {
        "amethyst_block" => 86,
        "bedrock" => 56,
        "black_carpet" => 461,
        "black_concrete" => 570,
        "black_wool" => 217,
        "blue_carpet" => 457,
        "blue_concrete" => 566,
        "blue_wool" => 213,
        "bricks" => 285,
        "brown_carpet" => 458,
        "brown_concrete" => 567,
        "brown_wool" => 214,
        "cherry_planks" => 41,
        "coal_ore" => 62,
        "cobblestone" => 35,
        "copper_block" => 89,
        "cyan_carpet" => 455,
        "cyan_concrete" => 564,
        "cyan_wool" => 211,
        "deepslate" => 8,
        "dirt" => 28,
        "glass" => 188,
        "glowstone" => 332,
        "grass_block" => 27,
        "gravel" => 61,
        "gray_carpet" => 453,
        "gray_concrete" => 562,
        "gray_wool" => 209,
        "green_carpet" => 459,
        "green_concrete" => 568,
        "green_wool" => 215,
        "hay_block" => 445,
        "iron_ore" => 64,
        "light_blue_carpet" => 449,
        "light_blue_concrete" => 558,
        "light_blue_wool" => 205,
        "light_gray_carpet" => 454,
        "light_gray_concrete" => 563,
        "light_gray_wool" => 210,
        "lime_carpet" => 451,
        "lime_concrete" => 560,
        "lime_wool" => 207,
        "magenta_carpet" => 448,
        "magenta_concrete" => 557,
        "magenta_wool" => 204,
        "magma_block" => 516,
        "melon" => 358,
        "moss_block" => 247,
        "oak_fence" => 311,
        "oak_leaves" => 176,
        "oak_log" => 132,
        "oak_planks" => 36,
        "obsidian" => 290,
        "orange_carpet" => 447,
        "orange_concrete" => 556,
        "orange_wool" => 203,
        "pink_carpet" => 452,
        "pink_concrete" => 561,
        "pink_wool" => 208,
        "pumpkin" => 322,
        "purple_carpet" => 456,
        "purple_concrete" => 565,
        "purple_wool" => 212,
        "red_carpet" => 460,
        "red_concrete" => 569,
        "red_wool" => 216,
        "sand" => 57,
        "sandstone" => 191,
        "sculk" => 371,
        "snow" => 305,
        "snow_block" => 307,
        "sponge" => 186,
        "stone" => 1,
        "terracotta" => 462,
        "torch" => 291,
        "white_carpet" => 446,
        "white_concrete" => 555,
        "white_wool" => 202,
        "yellow_carpet" => 450,
        "yellow_concrete" => 559,
        "yellow_wool" => 206,
        _ => return None,
    })
}

/// Id de registre d'entité vanilla (spawn_entity).
pub fn entity_type_id(kind: u8) -> i32 {
    match kind {
        0 => 128, // player
        1 => 124, // zombie
        2 => 23, // creeper
        3 => 77, // pig
        4 => 87, // sheep
        5 => 84, // rabbit
        6 => 19, // chicken
        7 => 22, // cow
        8 => 42, // fox
        9 => 93, // slime
        10 => 33, // enderman
        11 => 7, // bee
        12 => 75, // parrot
        13 => 111, // turtle
        14 => 24, // dolphin
        15 => 49, // goat
        16 => 43, // frog
        17 => 5, // axolotl
        18 => 48, // glow_squid
        _ => 23, // creeper
    }
}

/// MobKind::id() rustvoxel pour un type d'entité vanilla (None = inconnu -> zombie).
pub fn mob_kind_from_entity_type(id: i32) -> Option<u8> {
    Some(match id {
        128 => 0, // player
        124 => 1, // zombie
        23 => 2, // creeper
        77 => 3, // pig
        87 => 4, // sheep
        84 => 5, // rabbit
        19 => 6, // chicken
        22 => 7, // cow
        42 => 8, // fox
        93 => 9, // slime
        33 => 10, // enderman
        7 => 11, // bee
        75 => 12, // parrot
        111 => 13, // turtle
        24 => 14, // dolphin
        49 => 15, // goat
        43 => 16, // frog
        5 => 17, // axolotl
        48 => 18, // glow_squid
        _ => return None,
    })
}

/// Entrées du registre worldgen/biome (index = id de biome rustvoxel).
pub const BIOME_ENTRIES: [(&str, f32, f32, bool, i32, i32, i32, i32); 16] = [
    ("plains", 0.8, 0.4, true, 0x78A7FF, 0xC0D8FF, 0x44AFF5, 0x50533),
    ("forest", 0.7, 0.8, true, 0x78A7FF, 0xC0D8FF, 0x3F76E4, 0x50533),
    ("birch_forest", 0.6, 0.6, true, 0x78A7FF, 0xC0D8FF, 0x3F76E4, 0x50533),
    ("desert", 2.0, 0.0, false, 0x6EB1FF, 0xC0D8FF, 0x32A598, 0x50533),
    ("snowy_plains", 0.0, 0.5, true, 0x78A7FF, 0xC0D8FF, 0x3938C9, 0x50533),
    ("swamp", 0.8, 0.9, true, 0x78A7FF, 0xC0D8FF, 0x617B64, 0x232317),
    ("flower_forest", 0.7, 0.8, true, 0x78A7FF, 0xC0D8FF, 0x3F76E4, 0x50533),
    ("savanna", 1.2, 0.0, false, 0x78A7FF, 0xC0D8FF, 0x3F76E4, 0x50533),
    ("badlands", 2.0, 0.0, false, 0x6EB1FF, 0xC0D8FF, 0x4E7F81, 0x50533),
    ("jungle", 0.95, 0.9, true, 0x78A7FF, 0xC0D8FF, 0x14A2C5, 0x50533),
    ("cherry_grove", 0.5, 0.8, true, 0x78A7FF, 0xC0D8FF, 0x5DB7EF, 0x50533),
    ("jagged_peaks", -0.7, 0.9, true, 0x78A7FF, 0xC0D8FF, 0x3F76E4, 0x50533),
    ("meadow", 0.5, 0.8, true, 0x78A7FF, 0xC0D8FF, 0xE4ECF, 0x50533),
    ("grove", -0.2, 0.8, true, 0x78A7FF, 0xC0D8FF, 0x3F76E4, 0x50533),
    ("snowy_slopes", -0.3, 0.9, true, 0x78A7FF, 0xC0D8FF, 0x3938C9, 0x50533),
    ("stony_peaks", 1.0, 0.3, true, 0x78A7FF, 0xC0D8FF, 0x3F76E4, 0x50533),
];

/// Élément NBT du dimension_type overworld (utilisé par server.rs).
pub const DIMENSION_TYPE_NAME: &str = "minecraft:overworld";
pub const DIMENSION_NAME: &str = "minecraft:overworld";
pub const WORLD_HEIGHT: i32 = 384;
pub const WORLD_MIN_Y: i32 = -64;

/// Registre vanilla damage_type 1.20.5 EXACT (45 entrées copiées du client.jar
/// officiel : data/minecraft/damage_type/*.json) + wind_charge (1.21+) gardé en
/// extra de sécurité pour les tags des clients récents. Ordre alphabétique =
/// ordre d'id réseau.
/// (name, message_id, scaling, exhaustion, effects, death_message_type)
/// Champ vide = omis.
/// ⚠ IMPORTANT vérifié sur crash reports du client officiel :
/// - une CLÉ vanilla manquante fait crasher ClientLevel à l'entrée dans le
///   monde (« Missing element ResourceKey[minecraft:damage_type /
///   minecraft:on_fire] » via DamageSources.<init>) — d'où la table exacte ;
/// - une valeur d'enum hors palette (ex: effects="burn") fait échouer le gel
///   du registre (« Failed to parse value », Network Protocol Error).
/// Palettes client : effects ∈ {hurt, thorns, drowning, burning, freezing,
/// poking, knockback} ; scaling ∈ {never, when_caused_by_living_non_player,
/// always, falling_variations} ; death_message_type ∈ {default, fall_variants,
/// intentional_game_design}.
pub const DAMAGE_TYPES: &[(&str, &str, &str, f32, &str, &str)] = &[
    ("arrow", "arrow", "when_caused_by_living_non_player", 0.1, "", ""),
    ("bad_respawn_point", "badRespawnPoint", "always", 0.1, "", "intentional_game_design"),
    ("cactus", "cactus", "when_caused_by_living_non_player", 0.1, "", ""),
    ("cramming", "cramming", "when_caused_by_living_non_player", 0.0, "", ""),
    ("dragon_breath", "dragonBreath", "when_caused_by_living_non_player", 0.0, "", ""),
    ("drown", "drown", "when_caused_by_living_non_player", 0.0, "drowning", ""),
    ("dry_out", "dryout", "when_caused_by_living_non_player", 0.1, "", ""),
    ("explosion", "explosion", "always", 0.1, "", ""),
    ("fall", "fall", "when_caused_by_living_non_player", 0.0, "", "fall_variants"),
    ("falling_anvil", "anvil", "when_caused_by_living_non_player", 0.1, "", ""),
    ("falling_block", "fallingBlock", "when_caused_by_living_non_player", 0.1, "", ""),
    ("falling_stalactite", "fallingStalactite", "when_caused_by_living_non_player", 0.1, "", ""),
    ("fireball", "fireball", "when_caused_by_living_non_player", 0.1, "burning", ""),
    ("fireworks", "fireworks", "when_caused_by_living_non_player", 0.1, "", ""),
    ("fly_into_wall", "flyIntoWall", "when_caused_by_living_non_player", 0.0, "", ""),
    ("freeze", "freeze", "when_caused_by_living_non_player", 0.0, "freezing", ""),
    ("generic", "generic", "when_caused_by_living_non_player", 0.0, "", ""),
    ("generic_kill", "genericKill", "when_caused_by_living_non_player", 0.0, "", ""),
    ("hot_floor", "hotFloor", "when_caused_by_living_non_player", 0.1, "burning", ""),
    ("in_fire", "inFire", "when_caused_by_living_non_player", 0.1, "burning", ""),
    ("in_wall", "inWall", "when_caused_by_living_non_player", 0.0, "", ""),
    ("indirect_magic", "indirectMagic", "when_caused_by_living_non_player", 0.0, "", ""),
    ("lava", "lava", "when_caused_by_living_non_player", 0.1, "burning", ""),
    ("lightning_bolt", "lightningBolt", "when_caused_by_living_non_player", 0.1, "", ""),
    ("magic", "magic", "when_caused_by_living_non_player", 0.0, "", ""),
    ("mob_attack", "mob", "when_caused_by_living_non_player", 0.1, "", ""),
    ("mob_attack_no_aggro", "mob", "when_caused_by_living_non_player", 0.1, "", ""),
    ("mob_projectile", "mob", "when_caused_by_living_non_player", 0.1, "", ""),
    ("on_fire", "onFire", "when_caused_by_living_non_player", 0.0, "burning", ""),
    ("out_of_world", "outOfWorld", "when_caused_by_living_non_player", 0.0, "", ""),
    ("outside_border", "outsideBorder", "when_caused_by_living_non_player", 0.0, "", ""),
    ("player_attack", "player", "when_caused_by_living_non_player", 0.1, "", ""),
    ("player_explosion", "explosion.player", "always", 0.1, "", ""),
    ("sonic_boom", "sonic_boom", "always", 0.0, "", ""),
    ("spit", "mob", "when_caused_by_living_non_player", 0.1, "", ""),
    ("stalagmite", "stalagmite", "when_caused_by_living_non_player", 0.0, "", ""),
    ("starve", "starve", "when_caused_by_living_non_player", 0.0, "", ""),
    ("sting", "sting", "when_caused_by_living_non_player", 0.1, "", ""),
    ("sweet_berry_bush", "sweetBerryBush", "when_caused_by_living_non_player", 0.1, "poking", ""),
    ("thorns", "thorns", "when_caused_by_living_non_player", 0.1, "thorns", ""),
    ("thrown", "thrown", "when_caused_by_living_non_player", 0.1, "", ""),
    ("trident", "trident", "when_caused_by_living_non_player", 0.1, "", ""),
    ("unattributed_fireball", "onFire", "when_caused_by_living_non_player", 0.1, "burning", ""),
    ("wind_charge", "windCharge", "when_caused_by_living_non_player", 0.1, "", ""),
    ("wither", "wither", "when_caused_by_living_non_player", 0.0, "", ""),
    ("wither_skull", "witherSkull", "when_caused_by_living_non_player", 0.1, "", ""),
];
