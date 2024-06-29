use std::collections::{VecDeque};
use std::fmt::{Display, Formatter};
use itertools::{chain, Itertools};
use crate::{Chain, CHAIN_ARRAY, MergingChains};
use crate::tile::{Tile, TileParseError};
use ahash::{HashMap, HashSet};

const SAFE_CHAIN_SIZE: u16 = 11;
const GAME_ENDING_CHAIN_SIZE: u16 = 41;

#[derive(Clone)]
pub struct Grid {
    pub width: u8,
    pub height: u8,
    pub data: HashMap<Point, Slot>,
    chain_sizes: HashMap<Chain, u16>,
    pub previously_placed_tile_pt: Option<Point>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum PlaceTileResult {
    Proceed,
    Illegal {
        allow_trade_in: bool
    },
    SelectAvailableChain,
    DecideTieBreak {
        tied_chains: Vec<Chain>,
        mergers: Vec<MergingChains>,
    },
    Merge {
        mergers: Vec<MergingChains>
    },
}

impl Grid {
    pub fn new(width: u8, height: u8) -> Self {
        Self {
            width,
            height,
            data: Default::default(),
            chain_sizes: Default::default(),
            previously_placed_tile_pt: None,
        }
    }

    pub fn previously_placed_slot(&self) -> Slot {
        if let Some(pt) = self.previously_placed_tile_pt {
            self.get(pt)
        } else {
            Slot::Empty
        }
    }

    pub fn all_chains_are_safe(&self) -> bool {
        self.chain_sizes.iter().all(|(_, size)| *size >= SAFE_CHAIN_SIZE)
    }

    pub fn game_ending_chain_exists(&self) -> bool {
        self.chain_sizes.iter().any(|(_, size)| *size >= GAME_ENDING_CHAIN_SIZE)
    }

    pub fn is_pt_out_of_bounds(&self, pt: Point) -> bool {
        pt.x < 0 ||
            pt.y < 0 ||
            pt.x > self.width as i8 ||
            pt.y > self.height as i8
    }

    pub fn get(&self, pt: Point) -> Slot {
        if let Some(slot) = self.data.get(&pt) {
            *slot
        } else {
            Slot::Empty
        }
    }


    pub fn place(&mut self, tile: Tile) -> PlaceTileResult {
        if self.is_pt_out_of_bounds(tile.0) {
            panic!("setting invalid pt {:?}", tile.0);
        }

        let (neighbours, neighbouring_chains, num_neighbouring_chains, illegal, allow_trade_in) = self._is_illegal_tile(tile);

        if illegal {
            return PlaceTileResult::Illegal {
                allow_trade_in
            };
        }

        match num_neighbouring_chains {
            // two or more neighbouring chains
            2.. => {

                // if the tile is place between two safe chains, it is an illegal action, and the tile can be traded in
                if neighbouring_chains.iter().filter(|chain| self.chain_size(**chain) >= SAFE_CHAIN_SIZE).count() > 1 {
                    return PlaceTileResult::Illegal {
                        allow_trade_in: true
                    };
                }

                // merger

                let largest_chain_size = neighbouring_chains
                    .iter()
                    .map(|chain| self.chain_size(*chain))
                    .max()
                    .unwrap();

                // smaller chains are dealt with, one at a time, from largest to smallest

                let largest_chains: Vec<Chain> = neighbouring_chains
                    .iter()
                    .filter(|chain| self.chain_size(**chain) == largest_chain_size)
                    .map(|chain| *chain)
                    .collect();

                let largest_chain = largest_chains[0];

                // sort non-largest chains into a list in descending chain size order - ties in defunct chains don't matter as far as I know
                // nor do I comprehend any advantage to sorting them in this way, it's just in the rules.
                let mut other_chains: Vec<Chain> = neighbouring_chains.into_iter().filter(|chain| *chain != largest_chain).collect();
                other_chains.sort_by(|a, b| self.chain_sizes[b].cmp(&self.chain_sizes[a]));

                let merger_list = other_chains
                    .iter()
                    .map(|chain| MergingChains {
                        merging_chain: largest_chain,
                        defunct_chain: *chain,
                        num_remaining_players_to_merge: None, // must be set by the caller
                    })
                    .collect();

                self.set_slot(tile.0, Slot::Limbo);
                self.previously_placed_tile_pt = Some(tile.0);

                // two or more chains are the same size and the merge-maker must make the tie-breaking decision
                if largest_chains.len() > 1 {
                    return PlaceTileResult::DecideTieBreak {
                        tied_chains: largest_chains,
                        mergers: merger_list,
                    };
                }

                return PlaceTileResult::Merge {
                    mergers: merger_list
                };
            }

            // no neighbouring chains
            0 => {
                let num_neighbouring_nochains = self.num_nochains_chains_in_slots(&neighbours);

                self.set_slot(tile.0, Slot::NoChain);
                self.previously_placed_tile_pt = Some(tile.0);

                // touching one or more tiles which do not form a chain (free real estate)
                return if num_neighbouring_nochains > 0 {
                    PlaceTileResult::SelectAvailableChain
                } else {
                    PlaceTileResult::Proceed
                };
            }

            1 => {
                let chain = neighbouring_chains[0];
                self.set_slot(tile.0, Slot::Chain(chain));

                let affected_neighbours_pts: Vec<Point> = self.neighbouring_points(tile.0).into_iter().filter(|pt| self.get(*pt) == Slot::NoChain).collect();
                for affected_neighbour_pt in affected_neighbours_pts {
                    self.set_slot(affected_neighbour_pt, Slot::Chain(chain));
                }

                self.previously_placed_tile_pt = Some(tile.0);
                return PlaceTileResult::Proceed;
            }
        }
    }

    fn set_slot(&mut self, pt: Point, slot: Slot) {
        // if there was a chain in this slot,
        // update the count to reflect that it has been overwritten
        let existing_in_slot = self.get(pt);
        match existing_in_slot {
            Slot::Chain(chain) => {
                self.chain_sizes.entry(chain).and_modify(|n| *n -= 1);

                // remove chain from map if it is size zero
                if self.chain_sizes[&chain] == 0 {
                    self.chain_sizes.remove(&chain);
                }
            }
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
                Slot::Limbo |
                Slot::NoChain => None,
                Slot::Chain(chain) => Some(*chain),
            }
        }).unique().collect()
    }

    pub fn num_nochains_chains_in_slots(&self, slots: &[Slot]) -> u8 {
        slots.iter().fold(0u8, |acc, slot| {
            acc + {
                match slot {
                    Slot::Empty |
                    Slot::Limbo |
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
                Slot::Limbo |
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

    pub fn existing_chains(&self) -> Vec<Chain> {
        self.chain_sizes.clone().into_keys().collect()
    }

    pub fn available_chains(&self) -> Vec<Chain> {
        CHAIN_ARRAY
            .iter()
            .filter(|chain| !self.chain_sizes.contains_key(&chain))
            .map(|chain| *chain)
            .collect()
    }

    pub fn chain_size(&self, chain: Chain) -> u16 {
        if self.chain_sizes.contains_key(&chain) {
            self.chain_sizes[&chain]
        } else {
            0
        }
    }

    pub fn is_illegal_tile(&self, tile: Tile) -> (bool, bool) {
        let (_, _, _, illegal, allow_trade_in) = self._is_illegal_tile(tile);
        (illegal, allow_trade_in)
    }

    fn _is_illegal_tile(&self, tile: Tile) -> ([Slot; 4], Vec<Chain>, usize, bool, bool) {
        let neighbours = self.neighbours(tile.0);
        let neighbouring_chains = self.chains_in_slots(&neighbours);
        let num_neighbouring_chains = neighbouring_chains.len();

        match num_neighbouring_chains {
            2.. => {
                if neighbouring_chains.iter().filter(|chain| self.chain_size(**chain) >= SAFE_CHAIN_SIZE).count() > 1 {
                    return (neighbours, neighbouring_chains, num_neighbouring_chains, true, true);
                }
            }

            0 => {
                let num_neighbouring_nochains = self.num_nochains_chains_in_slots(&neighbours);
                if num_neighbouring_nochains > 0 {

                    // illegal to form an 8th chain
                    // but also this specific form of illegal tile cannot be traded in
                    if self.available_chains().len() == 0 {
                        return (neighbours, neighbouring_chains, num_neighbouring_chains, true, false);
                    }
                }
            }
            _ => {}
        };

        (neighbours, neighbouring_chains, num_neighbouring_chains, false, false)
    }
}


impl Display for Grid {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for y in 0..self.height as i8 {
            for x in 0..self.width as i8 {
                let pt = Point { x, y };
                let (is_illegal, is_replaceable) = self.is_illegal_tile(Tile(pt));
                match self.get(pt) {
                    Slot::Empty => {
                        if is_illegal {
                            if is_replaceable {
                                write!(f, "▪", );
                            } else {
                                write!(f, "▫", );
                            }
                        } else {
                            write!(f, "□", );
                        }
                    }
                    Slot::NoChain => {
                        write!(f, "■", );
                    }
                    Slot::Limbo => {
                        write!(f, "○", );
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
            previously_placed_tile_pt: None,
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
    Limbo,
    Chain(Chain),
}

#[cfg(test)]
mod test {
    use crate::{Chain, tile};
    use crate::grid::{Grid, PlaceTileResult, Point, Slot};
    use crate::tile::Tile;

    #[test]
    fn test_place_tile_empty_grid() {
        let mut grid = Grid::default();
        grid.place(tile!("A1"));

        assert_eq!(Slot::NoChain, grid.get(Point { x: 0, y: 0 }));
        assert_eq!(Slot::Empty, grid.get(Point { x: 1, y: 0 }));
        assert_eq!(Slot::Empty, grid.get(Point { x: -1, y: -1 }));

        assert_eq!(grid.chain_sizes.len(), 0);
    }

    #[test]
    fn test_form_chain() {
        let mut grid = Grid::default();

        assert_eq!(grid.place(tile!("A1")), PlaceTileResult::Proceed);
        assert_eq!(grid.place(tile!("A2")), PlaceTileResult::SelectAvailableChain);

        // simulate player selects a chain and the game fills the chain
        let chain = Chain::American;
        grid.fill_chain(tile!("A1"), chain);

        assert_eq!(grid.get(tile!("A1")), Slot::Chain(Chain::American));
        assert_eq!(grid.get(tile!("A2")), Slot::Chain(Chain::American));

        assert_eq!(grid.chain_sizes.len(), 1);
        assert_eq!(grid.chain_sizes[&Chain::American], 2);
    }

    #[test]
    fn test_form_chain_between_multiple_nochains() {
        let mut grid = Grid::default();


        assert_eq!(grid.place(tile!("A1")), PlaceTileResult::Proceed);
        assert_eq!(grid.place(tile!("B2")), PlaceTileResult::Proceed);

        assert_eq!(grid.place(tile!("A3")), PlaceTileResult::Proceed);

        // ignore this, not filling
        assert_eq!(grid.place(tile!("A4")), PlaceTileResult::SelectAvailableChain);

        // isolated islands
        assert_eq!(grid.place(tile!("D1")), PlaceTileResult::Proceed);
        assert_eq!(grid.place(tile!("F6")), PlaceTileResult::Proceed);


        // merge the chunks of nochains
        assert_eq!(grid.place(tile!("A2")), PlaceTileResult::SelectAvailableChain);

        // simulate player selects a chain and the game fills the chain
        let chain = Chain::American;
        grid.fill_chain(tile!("A1"), chain);

        assert_eq!(grid.get(tile!("A1")), Slot::Chain(Chain::American));
        assert_eq!(grid.get(tile!("A2")), Slot::Chain(Chain::American));
        assert_eq!(grid.get(tile!("A3")), Slot::Chain(Chain::American));
        assert_eq!(grid.get(tile!("A4")), Slot::Chain(Chain::American));
        assert_eq!(grid.get(tile!("B2")), Slot::Chain(Chain::American));

        // make sure islands are untouched
        assert_eq!(grid.get(tile!("D1")), Slot::NoChain);
        assert_eq!(grid.get(tile!("F6")), Slot::NoChain);

        // make sure empties are untouched
        assert_eq!(grid.get(tile!("A5")), Slot::Empty);
        assert_eq!(grid.get(tile!("F7")), Slot::Empty);

        // make sure chain sizes are correct
        assert_eq!(grid.chain_sizes.len(), 1);
        assert_eq!(grid.chain_sizes[&Chain::American], 5);

        // simulate overriding a chain
        grid.set_slot(tile!("A1"), Slot::Chain(Chain::Luxor));

        assert_eq!(grid.chain_sizes.len(), 2);
        assert_eq!(grid.chain_sizes[&Chain::American], 4);
        assert_eq!(grid.chain_sizes[&Chain::Luxor], 1);

        // refill with american
        grid.fill_chain(tile!("A1"), chain);

        // should only have one chain, luxor should be removed from map
        assert_eq!(grid.chain_sizes.len(), 1);
        assert_eq!(grid.chain_sizes[&Chain::American], 5);
    }
}