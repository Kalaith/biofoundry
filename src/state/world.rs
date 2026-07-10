//! Tile map generation for the warren.
//!
//! The map is an underground cavern: solid rock with a carved central
//! chamber, wandering tunnels, water pools, mushroom patches near water,
//! and ore veins in the rock walls. Generation is fully deterministic for
//! a given seed (state-owned `SeededRng`).

use macroquad_toolkit::grid::{FlatGrid, TilePos};
use macroquad_toolkit::rng::SeededRng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tile {
    Rock,
    Floor,
    Water,
    MushroomPatch,
    OreVein,
}

impl Tile {
    pub fn walkable(self) -> bool {
        matches!(self, Tile::Floor | Tile::MushroomPatch)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldMap {
    pub tiles: FlatGrid<Tile>,
    pub spawn: TilePos,
}

impl WorldMap {
    pub fn generate(width: usize, height: usize, rng: &mut SeededRng) -> Self {
        let mut tiles = FlatGrid::new(width, height, Tile::Rock);
        let center = TilePos::new(width as i32 / 2, height as i32 / 2);

        carve_chamber(&mut tiles, center, 6, 4);
        carve_tunnels(&mut tiles, center, rng);
        place_water_pools(&mut tiles, rng);
        place_mushroom_patches(&mut tiles, rng);
        place_ore_veins(&mut tiles, rng);

        Self {
            tiles,
            spawn: center,
        }
    }
}

/// Carve an elliptical open chamber around `center`.
fn carve_chamber(tiles: &mut FlatGrid<Tile>, center: TilePos, rx: i32, ry: i32) {
    for dy in -ry..=ry {
        for dx in -rx..=rx {
            let nx = dx as f32 / rx as f32;
            let ny = dy as f32 / ry as f32;
            if nx * nx + ny * ny <= 1.0 {
                tiles.set(TilePos::new(center.x + dx, center.y + dy), Tile::Floor);
            }
        }
    }
}

/// Random walks outward from the chamber carve winding tunnels.
fn carve_tunnels(tiles: &mut FlatGrid<Tile>, start: TilePos, rng: &mut SeededRng) {
    const DIRS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    let tunnel_count = 5 + rng.below(3);

    for _ in 0..tunnel_count {
        let mut pos = start;
        let mut dir = DIRS[rng.below(4)];
        let steps = 30 + rng.below(30);

        for _ in 0..steps {
            // Bias toward continuing straight so tunnels read as tunnels.
            if rng.chance(0.3) {
                dir = DIRS[rng.below(4)];
            }
            let next = TilePos::new(pos.x + dir.0, pos.y + dir.1);
            // Keep a 1-tile rock border so the map edge stays sealed.
            if next.x < 1
                || next.y < 1
                || next.x >= tiles.width as i32 - 1
                || next.y >= tiles.height as i32 - 1
            {
                dir = DIRS[rng.below(4)];
                continue;
            }
            pos = next;
            tiles.set(pos, Tile::Floor);
            // Occasionally widen so tunnels aren't single-file everywhere.
            if rng.chance(0.35) {
                let side = DIRS[rng.below(4)];
                let wide = TilePos::new(pos.x + side.0, pos.y + side.1);
                if wide.x >= 1
                    && wide.y >= 1
                    && wide.x < tiles.width as i32 - 1
                    && wide.y < tiles.height as i32 - 1
                {
                    tiles.set(wide, Tile::Floor);
                }
            }
        }
    }
}

/// Small water pools carved into floor areas.
fn place_water_pools(tiles: &mut FlatGrid<Tile>, rng: &mut SeededRng) {
    let pool_count = 2 + rng.below(2);
    let floors = floor_positions(tiles);

    for _ in 0..pool_count {
        let Some(&seed) = pick(&floors, rng) else {
            return;
        };
        tiles.set(seed, Tile::Water);
        for neighbor in seed.neighbors_4way() {
            if rng.chance(0.6) && tiles.get(neighbor) == Some(&Tile::Floor) {
                tiles.set(neighbor, Tile::Water);
            }
        }
    }
}

/// Mushroom patches grow on floor tiles, preferring spots near water.
fn place_mushroom_patches(tiles: &mut FlatGrid<Tile>, rng: &mut SeededRng) {
    let floors = floor_positions(tiles);
    let mut placed = 0;
    let target = 10 + rng.below(6);
    let mut attempts = 0;

    while placed < target && attempts < 400 {
        attempts += 1;
        let Some(&pos) = pick(&floors, rng) else {
            return;
        };
        if tiles.get(pos) != Some(&Tile::Floor) {
            continue;
        }
        let near_water = pos
            .neighbors_8way()
            .iter()
            .any(|n| tiles.get(*n) == Some(&Tile::Water));
        if near_water || rng.chance(0.25) {
            tiles.set(pos, Tile::MushroomPatch);
            placed += 1;
        }
    }
}

/// Ore veins embed in rock that touches an open floor tile, so miners can
/// reach them from day one.
fn place_ore_veins(tiles: &mut FlatGrid<Tile>, rng: &mut SeededRng) {
    let mut candidates: Vec<TilePos> = Vec::new();
    for (pos, tile) in tiles.iter_with_pos() {
        if *tile != Tile::Rock {
            continue;
        }
        // Keep the sealed 1-tile map border pure rock.
        if pos.x < 1
            || pos.y < 1
            || pos.x >= tiles.width as i32 - 1
            || pos.y >= tiles.height as i32 - 1
        {
            continue;
        }
        let touches_floor = pos
            .neighbors_4way()
            .iter()
            .any(|n| tiles.get(*n).is_some_and(|t| t.walkable()));
        if touches_floor {
            candidates.push(pos);
        }
    }

    let target = 12 + rng.below(6);
    for _ in 0..target {
        if let Some(&pos) = pick(&candidates, rng) {
            tiles.set(pos, Tile::OreVein);
        }
    }
}

fn floor_positions(tiles: &FlatGrid<Tile>) -> Vec<TilePos> {
    tiles
        .iter_with_pos()
        .filter(|(_, t)| **t == Tile::Floor)
        .map(|(pos, _)| pos)
        .collect()
}

fn pick<'a, T>(slice: &'a [T], rng: &mut SeededRng) -> Option<&'a T> {
    rng.choose(slice)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_is_deterministic() {
        let mut a = SeededRng::new(99);
        let mut b = SeededRng::new(99);
        let map_a = WorldMap::generate(48, 32, &mut a);
        let map_b = WorldMap::generate(48, 32, &mut b);

        assert_eq!(map_a.spawn, map_b.spawn);
        for (pos, tile) in map_a.tiles.iter_with_pos() {
            assert_eq!(map_b.tiles.get(pos), Some(tile));
        }
    }

    #[test]
    fn map_has_all_starting_resources() {
        let mut rng = SeededRng::new(20260710);
        let map = WorldMap::generate(48, 32, &mut rng);

        let mut floors = 0;
        let mut water = 0;
        let mut mushrooms = 0;
        let mut ore = 0;
        for (_, tile) in map.tiles.iter_with_pos() {
            match tile {
                Tile::Floor => floors += 1,
                Tile::Water => water += 1,
                Tile::MushroomPatch => mushrooms += 1,
                Tile::OreVein => ore += 1,
                Tile::Rock => {}
            }
        }

        assert!(
            floors > 100,
            "expected a carved warren, got {floors} floors"
        );
        assert!(water > 0, "expected at least one water pool");
        assert!(mushrooms > 0, "expected mushroom patches");
        assert!(ore > 0, "expected reachable ore veins");
    }

    #[test]
    fn spawn_is_walkable_and_edges_are_sealed() {
        let mut rng = SeededRng::new(7);
        let map = WorldMap::generate(48, 32, &mut rng);

        assert!(map.tiles.get(map.spawn).unwrap().walkable());
        for x in 0..48 {
            assert_eq!(map.tiles.get(TilePos::new(x, 0)), Some(&Tile::Rock));
            assert_eq!(map.tiles.get(TilePos::new(x, 31)), Some(&Tile::Rock));
        }
        for y in 0..32 {
            assert_eq!(map.tiles.get(TilePos::new(0, y)), Some(&Tile::Rock));
            assert_eq!(map.tiles.get(TilePos::new(47, y)), Some(&Tile::Rock));
        }
    }
}
