use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::{Display, Formatter};
use std::ptr::write;
use itertools::Itertools;
use crate::{Chain};
use crate::tile::{Tile, TileParseError};

#[derive(Clone)]
pub struct Grid {
    pub width: u8,
    pub height: u8,
    pub data: HashMap<Point, Slot>,
    pub chain_sizes: HashMap<Chain, u16>,
}

impl Display for Grid {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for y in 0..self.height as i8 {
            for x in 0..self.width as i8 {
                match self.get(Point { x, y }) {
                    Slot::Empty => {
                        write!(f, "☐", );
                    }
                    Slot::NoChain => {
                        write!(f, "☒", );
                    }
                    Slot::Chain(chain) => {
                        write!(f, "{}", chain.initial());
                    }
                }
                write!(f, "  ", );
            }
            writeln!(f);
        }

        Ok(())
    }
}

impl Default for Grid {
    fn default() -> Self {
        Self {
            width: 12,
            height: 9,
            data: Default::default(),
            chain_sizes: Default::default(),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PlaceTileResult {
    Proceed,
    ChainSelect {
        placed_tile_pt: Point
    },
}

impl Grid {
    pub fn is_pt_out_of_bounds(&self, pt: Point) -> bool {
        pt.x < 0 ||
            pt.y < 0 ||
            pt.x > self.width as i8 ||
            pt.y > self.height as i8
    }

    pub fn get(&self, pt: Point) -> Slot {
        if let Some(pt) = self.data.get(&pt) {
            *pt
        } else {
            Slot::Empty
        }
    }

    pub fn place(&mut self, tile: Tile) -> PlaceTileResult {
        if self.is_pt_out_of_bounds(tile.0) {
            panic!("setting invalid pt {:?}", tile.0);
        }

        let neighbours = self.neighbours(tile.0);
        let neighbouring_chains = self.chains_in_slots(&neighbours);

        if neighbouring_chains.len() == 0 {
            self.set_slot(tile.0, Slot::NoChain);

            let num_neighbouring_nochains = self.num_nochains_chains_in_slots(&neighbours);
            if num_neighbouring_nochains > 0 {
                return PlaceTileResult::ChainSelect {
                    placed_tile_pt: tile.0,
                };
            }
        }

        if neighbouring_chains.len() == 1 {
            let chain = neighbouring_chains[0];
            self.set_slot(tile.0, Slot::Chain(chain));
        }

        if neighbouring_chains.len() >= 2 {
            // merger
        }

        return PlaceTileResult::Proceed;
    }

    fn set_slot(&mut self, pt: Point, slot: Slot) {
        // if there was a chain in this slot,
        // update the count to reflect that it has been overwritten
        let existing_in_slot = self.get(pt);
        match existing_in_slot {
            Slot::Chain(chain) => {
                self.chain_sizes.entry(chain).and_modify(|n| *n -= 1);
            },
            _ => {}
        }

        // update the slot
        self.data.insert(pt, slot);

        // if the slot was a chain,
        // update the count to reflect that it has been added
        match slot {
            Slot::Chain(chain) => {
                self.chain_sizes.entry(chain).and_modify(|n| *n += 1).or_insert(1);
            }
            _ => {}
        }
    }

    /// Collects a vec of existing hotel chains in the slice of slots
    pub fn chains_in_slots(&self, slots: &[Slot]) -> Vec<Chain> {
        slots.iter().filter_map(|slot| {
            match slot {
                Slot::Empty |
                Slot::NoChain => None,
                Slot::Chain(chain) => Some(*chain)
            }
        }).unique().collect()
    }

    pub fn num_nochains_chains_in_slots(&self, slots: &[Slot]) -> u8 {
        slots.iter().fold(0u8, |acc, slot| {
            acc + {
                match slot {
                    Slot::Empty |
                    Slot::Chain(_) => 0,
                    Slot::NoChain => 1,
                }
            }
        })
    }

    /// Returns a \[North,West,South,East\] array of points which are orthogonal neighbours to
    /// the center point.
    pub fn neighbouring_points(&self, pt: Point) -> [Point; 4] {
        [
            Point { x: pt.x, y: pt.y + 1 },
            Point { x: pt.x + 1, y: pt.y },
            Point { x: pt.x, y: pt.y - 1 },
            Point { x: pt.x - 1, y: pt.y },
        ]
    }

    /// Returns a \[North,West,South,East\] array of grid slots which are orthogonal neighbours to
    /// the center point.
    pub fn neighbours(&self, pt: Point) -> [Slot; 4] {
        [
            self.get(Point { x: pt.x, y: pt.y + 1 }),
            self.get(Point { x: pt.x + 1, y: pt.y }),
            self.get(Point { x: pt.x, y: pt.y - 1 }),
            self.get(Point { x: pt.x - 1, y: pt.y }),
        ]
    }

    pub fn fill_chain(&mut self, pt: Point, chain: Chain) {
        let mut stack: VecDeque<Point> = Default::default();
        let mut visited: HashSet<Point> = Default::default();

        stack.push_back(pt);

        while let Some(pt) = stack.pop_front() {
            visited.insert(pt);

            match self.get(pt) {
                Slot::Empty => {
                    continue;
                }
                Slot::NoChain => {
                    self.set_slot(pt, Slot::Chain(chain));
                }
                Slot::Chain(existing_chain) => {
                    if existing_chain != chain {
                        self.set_slot(pt, Slot::Chain(chain));
                    }
                }
            }

            for valid_neighbour_pt in self.neighbouring_points(pt).iter().filter(|pt| {
                visited.contains(pt) == false && self.get(**pt) != Slot::Empty
            }) {
                stack.push_back(*valid_neighbour_pt);
            }
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Point {
    pub x: i8,
    pub y: i8,
}

impl TryFrom<&str> for Point {
    type Error = TileParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let tile: Tile = value.try_into()?;
        Ok(tile.0)
    }
}

impl From<Tile> for Point {
    fn from(value: Tile) -> Self {
        value.0
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Slot {
    Empty,
    NoChain,
    Chain(Chain),
}

#[cfg(test)]
mod test {
    use crate::Chain;
    use crate::grid::{Grid, PlaceTileResult, Point, Slot};
    use crate::tile::Tile;

    #[test]
    fn test_place_tile_empty_grid() {
        let mut grid = Grid::default();
        grid.place("A1".try_into().unwrap());

        assert_eq!(Slot::NoChain, grid.get(Point { x: 0, y: 0 }));
        assert_eq!(Slot::Empty, grid.get(Point { x: 1, y: 0 }));
        assert_eq!(Slot::Empty, grid.get(Point { x: -1, y: -1 }));

        assert_eq!(grid.chain_sizes.len(), 0);

    }

    #[test]
    fn test_form_chain() {
        let mut grid = Grid::default();

        assert_eq!(grid.place("A1".try_into().unwrap()), PlaceTileResult::Proceed);
        assert_eq!(grid.place("A2".try_into().unwrap()), PlaceTileResult::ChainSelect { placed_tile_pt: Point { x: 1, y: 0 } });

        // simulate player selects a chain and the game fills the chain
        let chain = Chain::American;
        grid.fill_chain("A1".try_into().unwrap(), chain);

        assert_eq!(grid.get("A1".try_into().unwrap()), Slot::Chain(Chain::American));
        assert_eq!(grid.get("A2".try_into().unwrap()), Slot::Chain(Chain::American));

        assert_eq!(grid.chain_sizes.len(), 1);
        assert_eq!(grid.chain_sizes[&Chain::American], 1);

    }

    #[test]
    fn test_form_chain_between_multiple_nochains() {
        let mut grid = Grid::default();


        assert_eq!(grid.place("A1".try_into().unwrap()), PlaceTileResult::Proceed);
        assert_eq!(grid.place("B2".try_into().unwrap()), PlaceTileResult::Proceed);

        assert_eq!(grid.place("A3".try_into().unwrap()), PlaceTileResult::Proceed);

        // ignore this, not filling
        assert_eq!(grid.place("A4".try_into().unwrap()), PlaceTileResult::ChainSelect { placed_tile_pt: Point { x: 3, y: 0 } });

        // isolated islands
        assert_eq!(grid.place("D1".try_into().unwrap()), PlaceTileResult::Proceed);
        assert_eq!(grid.place("F6".try_into().unwrap()), PlaceTileResult::Proceed);


        // merge the chunks of nochains
        assert_eq!(grid.place("A2".try_into().unwrap()), PlaceTileResult::ChainSelect { placed_tile_pt: Point { x: 1, y: 0 } });

        // simulate player selects a chain and the game fills the chain
        let chain = Chain::American;
        grid.fill_chain("A1".try_into().unwrap(), chain);

        assert_eq!(grid.get("A1".try_into().unwrap()), Slot::Chain(Chain::American));
        assert_eq!(grid.get("A2".try_into().unwrap()), Slot::Chain(Chain::American));
        assert_eq!(grid.get("A3".try_into().unwrap()), Slot::Chain(Chain::American));
        assert_eq!(grid.get("A4".try_into().unwrap()), Slot::Chain(Chain::American));
        assert_eq!(grid.get("B2".try_into().unwrap()), Slot::Chain(Chain::American));

        // make sure islands are untouched
        assert_eq!(grid.get("D1".try_into().unwrap()), Slot::NoChain);
        assert_eq!(grid.get("F6".try_into().unwrap()), Slot::NoChain);

        // make sure empties are untouched
        assert_eq!(grid.get("A5".try_into().unwrap()), Slot::Empty);
        assert_eq!(grid.get("F7".try_into().unwrap()), Slot::Empty);

        // make sure chain sizes are correct
        assert_eq!(grid.chain_sizes.len(), 1);
        assert_eq!(grid.chain_sizes[&Chain::American], 5);

        // simulate overriding a chain
        grid.set_slot("A1".try_into().unwrap(), Slot::Chain(Chain::Luxor));

        assert_eq!(grid.chain_sizes.len(), 2);
        assert_eq!(grid.chain_sizes[&Chain::American], 4);
        assert_eq!(grid.chain_sizes[&Chain::Luxor], 1);
    }
}