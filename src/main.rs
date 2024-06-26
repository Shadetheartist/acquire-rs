#![allow(unused)]

mod tile;
mod grid;

use tile::Tile;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use itertools::Itertools;
use rand::Rng;
use rand::seq::SliceRandom;
use crate::grid::Grid;

fn main() {
    println!("Hello, world!");
}

#[derive(Clone)]
struct Acquire {
    phase: Phase,
    players: Vec<Player>,
    tiles: Vec<Tile>,
    grid: Grid,
    current_player_id: PlayerId,
    current_merge_player: Option<PlayerId>,
}

#[derive(Debug)]
enum Action {
    PlaceTile(PlayerId, Tile),
    PurchaseStock([Option<Chain>; 3]),
    SelectMergingChain(Chain),
    StockDecision {
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
            Phase::AwaitingStockPurchase => unimplemented!(),
            Phase::AwaitingMergeSelection => unimplemented!(),
            Phase::AwaitingMergeStockDecision => unimplemented!(),
        }
    }

    pub fn apply_action(&self, action: Action) -> Acquire {
        let mut game = self.clone();

        match action {
            Action::PlaceTile(_, tile) => {
                game.grid.place(tile);
            }
            Action::PurchaseStock(_) => {}
            Action::SelectMergingChain(_) => {}
            Action::StockDecision { .. } => {}
        }

        game
    }

    fn get_player_by_id(&self, player_id: PlayerId) -> &Player {
        self.players.iter().find(|player| player.id == player_id).unwrap()
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


#[derive(Clone)]
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

        println!("{:?}", game.actions());
    }
}