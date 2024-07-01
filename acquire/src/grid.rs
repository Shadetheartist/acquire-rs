use std::collections::VecDeque;
use std::fmt::{Display, Formatter};
use itertools::Itertools;
use crate::MergingChains;
use crate::tile::{Tile, TileParseError};
use ahash::{HashMap, HashSet};
use crate::chain::{Chain, ChainTable};

const SAFE_CHAIN_SIZE: u16 = 11;
const GAME_ENDING_CHAIN_SIZE: u16 = 41;

#[derive(Clone)]
pub struct Grid {
    pub width: u8,
    pub height: u8,
    pub data: HashMap<Point, Slot>,
    chain_sizes: ChainTable<u16>,
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

    pub fn all_chains_are_safe(&self) -> bool {
        self.chain_sizes.0.iter().all(|size| *size >= SAFE_CHAIN_SIZE)
    }

    fn num_safe_chains(&self) -> usize {
        self.chain_sizes.0.iter().filter(|size| **size >= SAFE_CHAIN_SIZE).count()
    }

    pub fn game_ending_chain_exists(&self) -> bool {
        self.chain_sizes.0.iter().any(|size| *size >= GAME_ENDING_CHAIN_SIZE)
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
            Slot::Empty(Legality::Legal)
        }
    }


    pub fn place(&mut self, tile: Tile) -> PlaceTileResult {
        if self.is_pt_out_of_bounds(tile.0) {
            panic!("setting invalid pt {:?}", tile.0);
        }

        let neighbours = self.neighbours(tile.0);
        let neighbouring_chains = self.chains_in_slots(&neighbours);
        let num_neighbouring_chains = neighbouring_chains.len();

        if let Slot::Empty(legality) = self.get(tile.0) {
            match legality {
                Legality::Legal => {}
                Legality::TemporarilyIllegal => {
                    return PlaceTileResult::Illegal { allow_trade_in: false };
                }
                Legality::PermanentIllegal => {
                    return PlaceTileResult::Illegal { allow_trade_in: true };
                }
            }
        }

        match num_neighbouring_chains {
            // two or more neighbouring chains
            2.. => {
                // merger

                let largest_chain_size = neighbouring_chains
                    .iter()
                    .map(|chain| self.chain_size(*chain))
                    .max()
                    .unwrap();

                // smaller chains are dealt with, one at a time, from largest to smallest

                let largest_chains: Vec<Chain> = neighbouring_chains
                    .iter()
                    .filter(|chain| self.chain_size(**chain) == largest_chain_size).copied()
                    .collect();

                let largest_chain = largest_chains[0];

                // sort non-largest chains into a list in descending chain size order - ties in defunct chains don't matter as far as I know
                // nor do I comprehend any advantage to sorting them in this way, it's just in the rules.
                let mut other_chains: Vec<Chain> = neighbouring_chains.into_iter().filter(|chain| *chain != largest_chain).collect();
                other_chains.sort_by_key(|chain|self.chain_sizes.get(chain));

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
                    };
                }

                PlaceTileResult::Merge {
                    mergers: merger_list
                }
            }

            // no neighbouring chains
            0 => {
                let num_neighbouring_nochains = self.num_nochains_chains_in_slots(&neighbours);

                self.set_slot(tile.0, Slot::NoChain);
                self.previously_placed_tile_pt = Some(tile.0);

                self.update_legality_of_neighbours(tile.0);

                // touching one or more tiles which do not form a chain (free real estate)
                if num_neighbouring_nochains > 0 {
                    PlaceTileResult::SelectAvailableChain
                } else {
                    PlaceTileResult::Proceed
                }
            }

            1 => {
                let chain = neighbouring_chains[0];
                self.set_slot(tile.0, Slot::Chain(chain));

                self.update_legality_of_neighbours(tile.0);
                self.update_chain_of_neighbours(tile.0, chain);

                self.previously_placed_tile_pt = Some(tile.0);
                PlaceTileResult::Proceed
            }
        }
    }

    fn update_chain_of_neighbours(&mut self, pt: Point, chain: Chain){
        for neighbouring_pt in self.neighbouring_points(pt) {
            match self.get(neighbouring_pt) {
                Slot::Limbo |
                Slot::NoChain => self.set_slot(neighbouring_pt, Slot::Chain(chain)),
                _ => {}
            };
        }
    }

    fn update_legality_of_slot(&mut self, pt: Point) {
        match self.get(pt) {
            // update the legality of neighbouring empty slots
            Slot::Empty(legality) => {
                match legality {
                    Legality::Legal |
                    Legality::TemporarilyIllegal => {
                        let (illegal, permanent) = self._is_illegal_tile(Tile(pt));
                        if illegal {
                            if permanent {
                                self.set_slot(pt, Slot::Empty(Legality::PermanentIllegal))
                            } else {
                                self.set_slot(pt, Slot::Empty(Legality::TemporarilyIllegal))
                            }
                        }
                    }
                    // this won't ever change once set to permanently illegal
                    Legality::PermanentIllegal => {}
                }
            }
            _ => {}
        };
    }

    fn update_legality_of_neighbours(&mut self, pt: Point){
        for neighbouring_pt in self.neighbouring_points(pt) {
            self.update_legality_of_slot(neighbouring_pt);
        }
    }

    fn set_slot(&mut self, pt: Point, slot: Slot) {
        // if there was a chain in this slot,
        // update the count to reflect that it has been overwritten
        let existing_in_slot = self.get(pt);
        if let Slot::Chain(chain) = existing_in_slot {
            let new_value = self.chain_sizes.get(&chain) - 1;
            self.chain_sizes.set(&chain, new_value);
        }

        // update the slot
        self.data.insert(pt, slot);

        // if the slot was a chain,
        // update the count to reflect that it has been added
        if let Slot::Chain(chain) = slot {
            let new_value = self.chain_sizes.get(&chain) + 1;
            self.chain_sizes.set(&chain, new_value);
        }
    }

    /// Collects a vec of existing hotel chains in the slice of slots
    pub fn chains_in_slots(&self, slots: &[Slot]) -> Vec<Chain> {
        slots.iter().filter_map(|slot| {
            match slot {
                Slot::Empty(_) |
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
                    Slot::Empty(_) |
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

    fn update_legality_of_all_nochains(&mut self) {
        let nochain_pts: Vec<Point> = self.data.iter().filter(|(_, slot)| matches!(**slot, Slot::NoChain | Slot::Limbo)).map(|(pt, slot)| *pt).collect();
        for pt in nochain_pts {
            self.update_legality_of_neighbours(pt);
        }
    }

    pub fn fill_chain(&mut self, pt: Point, chain: Chain) {
        let mut stack: VecDeque<Point> = Default::default();
        let mut visited: HashSet<Point> = Default::default();
        let mut empty_surrounding_pts: Vec<Point> = Default::default();

        stack.push_back(pt);

        while let Some(pt) = stack.pop_front() {
            visited.insert(pt);

            match self.get(pt) {
                Slot::Empty(legality) => {
                    match legality {
                        Legality::Legal |
                        Legality::TemporarilyIllegal => { empty_surrounding_pts.push(pt); }
                        Legality::PermanentIllegal => {}
                    };

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

            // add valid neighbours to the stack
            for valid_neighbour_pt in self.neighbouring_points(pt).iter().filter(|pt| {
                !visited.contains(pt)
            }) {
                stack.push_back(*valid_neighbour_pt);
            }
        }

        if self.permanently_illegal_possible() {
            for pt in empty_surrounding_pts {
                self.update_legality_of_slot(pt);
            }
        }

        if self.temporary_illegal_possible () {
            self.update_legality_of_all_nochains();
        }

    }

    pub fn existing_chains(&self) -> Vec<Chain> {
        self.chain_sizes.0
            .iter()
            .enumerate()
            .filter(|(_, size)| **size > 0)
            .map(|(chain_idx, _)| Chain::from_index(chain_idx))
            .collect()
    }

    pub fn available_chains(&self) -> Vec<Chain> {
        self.chain_sizes.0
            .iter()
            .enumerate()
            .filter(|(_, size)| **size == 0)
            .map(|(chain_idx, _)| Chain::from_index(chain_idx))
            .collect()
    }

    pub fn num_available_chains(&self) -> usize {
        self.chain_sizes.0
            .iter()
            .enumerate()
            .filter(|(_, size)| **size == 0)
            .map(|(chain_idx, _)| Chain::from_index(chain_idx))
            .count()
    }

    pub fn chain_size(&self, chain: Chain) -> u16 {
        self.chain_sizes.get(&chain)
    }

    fn permanently_illegal_possible(&self) -> bool {
        self.num_safe_chains() > 1
    }

    fn temporary_illegal_possible(&self) -> bool {
        self.num_available_chains() == 0
    }

    fn _is_illegal_tile(&self, tile: Tile) -> (bool, bool) {

        let permanently_illegal_possible = self.permanently_illegal_possible();
        let temporary_illegal_possible = self.temporary_illegal_possible();

        // can shortcut knowing that no tiles are illegal if there is less than two safe chains and there are chains available to create
        if !permanently_illegal_possible && !temporary_illegal_possible {
            return (false, false)
        }

        let neighbours = self.neighbours(tile.0);
        let neighbouring_chains = self.chains_in_slots(&neighbours);
        let num_neighbouring_chains = neighbouring_chains.len();

        match num_neighbouring_chains {
            2.. => {
                if !permanently_illegal_possible {
                    return (false, false)
                }

                if neighbouring_chains.iter().filter(|chain| self.chain_size(**chain) >= SAFE_CHAIN_SIZE).count() > 1 {
                    return (true, true);
                }
            }

            0 => {
                if !temporary_illegal_possible {
                    return (false, false)
                }

                let num_neighbouring_nochains = self.num_nochains_chains_in_slots(&neighbours);
                if num_neighbouring_nochains > 0 {

                    // illegal to form an 8th chain
                    // but also this specific form of illegal tile cannot be traded in
                    if self.num_available_chains() == 0 {
                        return (true, false);
                    }
                }
            }
            _ => {}
        };

        (false, false)
    }

}


#[allow(unused_must_use)]
impl Display for Grid {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for y in 0..self.height as i8 {
            for x in 0..self.width as i8 {
                let pt = Point { x, y };
                match self.get(pt) {
                    Slot::Empty(legality) => {
                        match legality {
                            Legality::Legal => write!(f, "□", ),
                            Legality::TemporarilyIllegal => write!(f, "▫", ),
                            Legality::PermanentIllegal => write!(f, "▪", ),
                        };
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
pub enum Legality {
    Legal,
    TemporarilyIllegal,
    PermanentIllegal
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Slot {
    Empty(Legality),
    NoChain,
    Limbo,
    Chain(Chain),
}

#[cfg(test)]
mod test {
    use crate::tile;
    use crate::chain::Chain;
    use crate::grid::{Grid, Legality, PlaceTileResult, Slot};
    

    #[test]
    fn test_place_tile_empty_grid() {
        let mut grid = Grid::default();
        grid.place(tile!("A1"));

        assert_eq!(Slot::NoChain, grid.get(tile!("A1")));
        assert_eq!(Slot::Empty(Legality::Legal), grid.get(tile!("A2")));
    }

    #[test]
    fn test_form_chain() {
        let mut grid = Grid::default();

        assert_eq!(grid.place(tile!("A1")), PlaceTileResult::Proceed);
        assert_eq!(grid.place(tile!("A2")), PlaceTileResult::SelectAvailableChain);

        // simulate player selects a chain and the game fills the chain
        let chain = Chain::American;
        grid.fill_chain(tile!("A1"), chain);

        assert_eq!(grid.get(tile!("A1")), Slot::Chain(chain));
        assert_eq!(grid.get(tile!("A2")), Slot::Chain(chain));

        assert_eq!(grid.chain_sizes[&chain], 2);
    }

    #[test]
    fn test_permanent_illegal_tile() {
        let mut grid = Grid::default();

        grid.place(tile!("A1"));
        grid.place(tile!("A2"));
        grid.place(tile!("A3"));
        grid.place(tile!("A4"));
        grid.place(tile!("A5"));
        grid.place(tile!("A6"));
        grid.place(tile!("A7"));
        grid.place(tile!("A8"));
        grid.place(tile!("A9"));
        grid.place(tile!("A10"));
        grid.place(tile!("A11"));
        grid.place(tile!("A12"));
        grid.fill_chain(tile!("A12"), Chain::American);

        grid.place(tile!("D1"));
        grid.place(tile!("D2"));
        grid.place(tile!("D3"));
        grid.place(tile!("D4"));
        grid.place(tile!("D5"));
        grid.place(tile!("D6"));
        grid.place(tile!("D7"));
        grid.place(tile!("D8"));
        grid.place(tile!("D9"));
        grid.place(tile!("D10"));
        grid.place(tile!("D11"));
        grid.place(tile!("D12"));
        grid.place(tile!("C12"));
        grid.fill_chain(tile!("C12"), Chain::Tower);
        println!("{}", grid);

        assert_eq!(grid.get(tile!("B12")), Slot::Empty(Legality::PermanentIllegal));

        grid.place(tile!("F1"));
        grid.place(tile!("F2"));
        grid.fill_chain(tile!("F2"), Chain::Festival);

        grid.place(tile!("B1"));
        assert_eq!(grid.get(tile!("C1")), Slot::Empty(Legality::PermanentIllegal));

        grid.place(tile!("F3"));
        assert_eq!(grid.get(tile!("E1")), Slot::Empty(Legality::Legal));
        assert_eq!(grid.get(tile!("E2")), Slot::Empty(Legality::Legal));
        assert_eq!(grid.get(tile!("E3")), Slot::Empty(Legality::Legal));

        println!("{}", grid);

    }

    #[test]
    fn test_temporary_illegal_tile() {
        let mut grid = Grid::default();

        grid.place(tile!("A1"));
        grid.place(tile!("A2"));
        grid.fill_chain(tile!("A2"), Chain::Tower);

        grid.place(tile!("C1"));
        grid.place(tile!("C2"));
        grid.fill_chain(tile!("C2"), Chain::Luxor);

        grid.place(tile!("E1"));
        grid.place(tile!("E2"));
        grid.fill_chain(tile!("E2"), Chain::American);

        grid.place(tile!("G1"));
        grid.place(tile!("G2"));
        grid.fill_chain(tile!("G2"), Chain::Festival);

        grid.place(tile!("I1"));
        grid.place(tile!("I2"));
        grid.fill_chain(tile!("I2"), Chain::Worldwide);

        grid.place(tile!("A4"));
        grid.place(tile!("A5"));
        grid.fill_chain(tile!("A5"), Chain::Imperial);

        grid.place(tile!("C4"));
        grid.place(tile!("C5"));
        grid.fill_chain(tile!("C5"), Chain::Continental);

        grid.place(tile!("E4"));

        assert_eq!(grid.get(tile!("E3")), Slot::Empty(Legality::Legal));
        assert_eq!(grid.get(tile!("D4")), Slot::Empty(Legality::Legal));
        assert_eq!(grid.get(tile!("E5")), Slot::Empty(Legality::TemporarilyIllegal));
        assert_eq!(grid.get(tile!("F4")), Slot::Empty(Legality::TemporarilyIllegal));
    }

    #[test]
    fn test_temporary_illegal_tile_2() {
        let mut grid = Grid::default();

        grid.place(tile!("A1"));
        grid.place(tile!("A2"));
        grid.fill_chain(tile!("A2"), Chain::Tower);

        grid.place(tile!("C1"));
        grid.place(tile!("C2"));
        grid.fill_chain(tile!("C2"), Chain::Luxor);

        grid.place(tile!("E1"));
        grid.place(tile!("E2"));
        grid.fill_chain(tile!("E2"), Chain::American);

        grid.place(tile!("G1"));
        grid.place(tile!("G2"));
        grid.fill_chain(tile!("G2"), Chain::Festival);

        grid.place(tile!("I1"));
        grid.place(tile!("I2"));
        grid.fill_chain(tile!("I2"), Chain::Worldwide);

        grid.place(tile!("A4"));
        grid.place(tile!("A5"));
        grid.fill_chain(tile!("A5"), Chain::Imperial);

        grid.place(tile!("E4"));

        grid.place(tile!("C4"));
        grid.place(tile!("C5"));
        grid.fill_chain(tile!("C5"), Chain::Continental);

        println!("{}", grid);

        assert_eq!(grid.get(tile!("E3")), Slot::Empty(Legality::Legal));
        assert_eq!(grid.get(tile!("D4")), Slot::Empty(Legality::Legal));
        assert_eq!(grid.get(tile!("E5")), Slot::Empty(Legality::TemporarilyIllegal));
        assert_eq!(grid.get(tile!("F4")), Slot::Empty(Legality::TemporarilyIllegal));
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
        assert_eq!(grid.get(tile!("A5")), Slot::Empty(Legality::Legal));
        assert_eq!(grid.get(tile!("F7")), Slot::Empty(Legality::Legal));

        // make sure chain sizes are correct
        assert_eq!(grid.chain_sizes[&Chain::American], 5);

        // simulate overriding a chain
        grid.set_slot(tile!("A1"), Slot::Chain(Chain::Luxor));

        assert_eq!(grid.chain_sizes[&Chain::American], 4);
        assert_eq!(grid.chain_sizes[&Chain::Luxor], 1);

        // refill with american
        grid.fill_chain(tile!("A1"), chain);

        // should only have one chain, luxor should be removed from map
        assert_eq!(grid.chain_sizes[&Chain::American], 5);
    }
}