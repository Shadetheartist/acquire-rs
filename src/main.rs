#![allow(unused)]

mod tile;
mod grid;

use tile::Tile;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use itertools::Itertools;
use rand::Rng;
use rand::seq::SliceRandom;
use crate::grid::{Grid, PlaceTileResult, Slot};

fn main() {
    println!("Hello, world!");
}

#[derive(Clone)]
struct Acquire {
    phase: Phase,
    players: Vec<Player>,
    tiles: Vec<Tile>,
    stocks: HashMap<Chain, u16>,
    grid: Grid,
    current_player_id: PlayerId,
    current_merge_player: Option<PlayerId>,
}

#[derive(Debug)]
enum Action {
    PlaceTile(PlayerId, Tile),
    PurchaseStock(PlayerId, [BuyOption; 3]),
    SelectMergingChain(PlayerId, Chain),
    StockDecision {
        player_id: PlayerId,
        num_sell: u8,
        num_trade_in: u8,
        num_keep: u8,
    },
}

impl Acquire {
    pub fn new<R: Rng>(mut rng: R, num_players: u8) -> Self {
        let grid = Grid::default();
        let mut tiles = vec![];
        for y in 0..grid.height as i8 {
            for x in 0..grid.width as i8 {
                tiles.push(Tile::new(x, y));
            }
        }
        tiles.shuffle(&mut rng);

        let players = (0..num_players).into_iter().map(|id| Player {
            id: PlayerId(id),
            tiles: (0..6).into_iter().map(|_| tiles.remove(0)).collect(),
        }).collect();

        Self {
            phase: Phase::AwaitingTilePlacement,
            players,
            tiles,
            stocks: Default::default(),
            grid,
            current_player_id: PlayerId(0),
            current_merge_player: None,
        }
    }

    pub fn actions(&self) -> Vec<Action> {
        match self.phase {
            Phase::AwaitingTilePlacement => {
                let player = self.get_player_by_id(self.current_player_id);
                player.tiles.iter().map(|tile| {
                    Action::PlaceTile(self.current_player_id, *tile)
                }).collect()
            }
            Phase::AwaitingStockPurchase => {

                let player = self.get_player_by_id(self.current_player_id);
                let buy_options = {
                    let mut buy_option_chains: Vec<BuyOption> = self.get_existing_chains()
                        .iter()
                        .map(|chain| BuyOption::Chain(*chain))
                        .collect()
                        ;

                    buy_option_chains.push(BuyOption::None);

                    buy_option_chains
                };

                let mut actions = vec![];

                for buy_option_1 in &buy_options {
                    for buy_option_2 in &buy_options {
                        for buy_option_3 in &buy_options {
                            actions.push(Action::PurchaseStock(self.current_player_id, [
                                *buy_option_1,
                                *buy_option_2,
                                *buy_option_3
                            ]));
                        }
                    }
                }

                actions
            },
            Phase::AwaitingMergeSelection => unimplemented!(),
            Phase::AwaitingMergeStockDecision => unimplemented!(),
        }
    }

    pub fn apply_action(&self, action: Action) -> Acquire {
        let mut game = self.clone();

        match action {
            Action::PlaceTile(_, tile) => {
                let result = game.grid.place(tile);
                match result {
                    PlaceTileResult::Proceed => {
                        game.phase = Phase::AwaitingStockPurchase;
                        // shortcut the purchase of stock when
                        if game.get_existing_chains().len() == 0 {
                            game.phase = Phase::AwaitingTilePlacement;
                            game.current_player_id = self.next_player_id();
                        }
                    }
                    PlaceTileResult::ChainSelect { .. } => {}
                }
            }
            Action::PurchaseStock(_, _) => {}
            Action::SelectMergingChain(_, _) => {}
            Action::StockDecision { .. } => {}
        }

        game
    }

    fn get_player_by_id(&self, player_id: PlayerId) -> &Player {
        self.players.iter().find(|player| player.id == player_id).unwrap()
    }

    fn get_existing_chains(&self) -> Vec<Chain> {
        self.grid.data.iter().filter_map(|(_, slot)|{
            match slot {
                Slot::Chain(chain) => Some(*chain),
                _ => None
            }
        }).unique().collect()
    }

    fn get_purchasable_chains(&self) -> Vec<Chain> {
        self.grid.data.iter().filter_map(|(_, slot)|{
            match slot {
                Slot::Chain(chain) => Some(*chain),
                _ => None
            }
        }).unique().collect()
    }

    fn next_player_id(&self) -> PlayerId {
        PlayerId((self.current_player_id.0 + 1) % self.players.len() as u8)
    }
}




#[derive(Copy, Clone, Eq, PartialEq)]
struct PlayerId(pub u8);
impl Debug for PlayerId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("P_{}", self.0))
    }
}

#[derive(Clone)]
struct Player {
    id: PlayerId,
    tiles: Vec<Tile>,
}

#[derive(Copy, Clone, Debug)]
enum BuyOption {
    None,
    Chain(Chain)
}


#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
enum Chain {
    Tower,
    Luxor,
    American,
    Worldwide,
    Festival,
    Continental,
    Imperial,
}

const CHAIN_ARRAY: [Chain; 7] = [
    Chain::Tower,
    Chain::Luxor,
    Chain::American,
    Chain::Worldwide,
    Chain::Festival,
    Chain::Continental,
    Chain::Imperial,
];

impl Chain {
    pub fn initial(&self) -> char {
        match self {
            Chain::Tower => 'T',
            Chain::Luxor => 'L',
            Chain::American => 'A',
            Chain::Worldwide => 'W',
            Chain::Festival => 'F',
            Chain::Continental => 'C',
            Chain::Imperial => 'I',
        }
    }
}


#[derive(Debug, Clone)]
enum Phase {
    AwaitingTilePlacement,
    AwaitingStockPurchase,
    AwaitingMergeSelection,
    AwaitingMergeStockDecision,
}

#[cfg(test)]
mod test {
    use rand::thread_rng;
    use crate::Acquire;

    #[test]
    fn test_simple() {
        let game = Acquire::new(thread_rng(), 4);

        let mut actions = game.actions();
        println!("{:?}: {:?}", game.phase, game.actions());

        let game = game.apply_action(actions.remove(0));
        println!("{:?}: {:?}", game.phase, game.actions());


    }
}