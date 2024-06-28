#![allow(unused)]

mod tile;
mod grid;
mod money;
mod stock;
mod player;

use tile::Tile;
use std::fmt::{Debug, Display, Formatter, Write};
use ahash::HashMap;
use itertools::Itertools;
use rand::Rng;
use rand::seq::SliceRandom;
use player::Player;
use crate::grid::{Grid, PlaceTileResult, Slot};
use crate::stock::Stocks;

fn main() {
    println!("Hello, world!");
}

#[derive(Clone)]
struct Acquire {
    phase: Phase,
    players: Vec<Player>,
    tiles: Vec<Tile>,
    stocks: Stocks,
    grid: Grid,
    current_player_id: PlayerId,
    turn: u16,
}

struct Options {
    num_players: u8,
    num_tiles: u8,
    grid_width: u8,
    grid_height: u8,
    num_stock: u8,
    starting_money: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            num_players: 4,
            num_tiles: 6,
            grid_width: 12,
            grid_height: 9,
            num_stock: 25,
            starting_money: 6000,
        }
    }
}


impl Acquire {
    pub fn new<R: Rng>(mut rng: R, options: &Options) -> Self {
        let grid = Grid::new(options.grid_width, options.grid_height);

        let mut tiles = vec![];
        for y in 0..grid.height as i8 {
            for x in 0..grid.width as i8 {
                tiles.push(Tile::new(x, y));
            }
        }
        tiles.shuffle(&mut rng);

        let players = (0..options.num_players).into_iter().map(|id| Player {
            id: PlayerId(id),
            tiles: (0..options.num_tiles).into_iter().map(|_| tiles.remove(0)).collect(),
            stocks: Default::default(),
            money: options.starting_money,
        }).collect();

        let stocks: HashMap<Chain, u8> = CHAIN_ARRAY.iter().map(|chain| (*chain, options.num_stock)).collect();

        Self {
            phase: Phase::AwaitingTilePlacement,
            players,
            tiles,
            stocks: stocks.into(),
            grid,
            current_player_id: PlayerId(0),
            turn: 1,
        }
    }

    pub fn actions(&self) -> Vec<Action> {
        match &self.phase {
            Phase::AwaitingTilePlacement => {
                let player = self.get_player_by_id(self.current_player_id);
                player.tiles.iter().map(|tile| {
                    Action::PlaceTile(self.current_player_id, *tile)
                }).collect()
            }

            Phase::AwaitingChainCreationSelection => {
                self.grid.available_chains().into_iter().map(|chain| {
                    Action::SelectChainToCreate(self.current_player_id, chain)
                }).collect()
            }
            Phase::Merge { merging_player_id, phase: merge_phase, mergers_remaining } => {
                match merge_phase {
                    MergePhase::AwaitingTiebreakSelection { tied_chains } => {
                        tied_chains.iter().map(|chain| {
                            Action::SelectChainForTiebreak(*merging_player_id, *chain)
                        }).collect()
                    }
                    MergePhase::AwaitingMergeDecision => {
                        let current_merger = mergers_remaining[0];

                        self.merge_combinations(*merging_player_id, current_merger)
                            .iter()
                            .map(|decision| {
                                Action::DecideMerge {
                                    merging_player_id: *merging_player_id,
                                    decision: *decision,
                                }
                            })
                            .collect()
                    }
                }
            }

            Phase::AwaitingStockPurchase => {
                let player = self.get_player_by_id(self.current_player_id);

                let mut actions = self.purchasable_combinations(self.current_player_id)
                    .iter()
                    .map(|buy| {
                        Action::PurchaseStock(self.current_player_id, *buy)
                    })
                    .collect();

                actions
            }
        }
    }

    pub fn apply_action(&self, action: Action) -> Acquire {
        let mut game = self.clone();

        match action {
            Action::PlaceTile(player_id, tile) => {
                let mut player = game.get_player_by_id_mut(player_id);

                // remove tile from player inventory
                let tile_idx = player.tiles.iter().position(|t| *t == tile).unwrap();
                let tile = player.tiles.remove(tile_idx);

                // after the tile is placed, there are several branches to consider
                // which changes which phase the game moves to
                let result = game.grid.place(tile);
                match result {
                    // nothing special happens, the game proceeds to the next player
                    PlaceTileResult::Proceed => {
                        game.phase = Phase::AwaitingStockPurchase;
                        // shortcut the purchase of stock when
                        if game.grid.existing_chains().len() == 0 {
                            game.player_take_tile(player_id);
                            game.phase = Phase::AwaitingTilePlacement;
                            game.go_next_turn();
                        }
                    }
                    // the new tile created a chain, we need user input to select the hotel chain
                    PlaceTileResult::SelectAvailableChain => {
                        game.phase = Phase::AwaitingChainCreationSelection;
                    }
                    // the tile is going to merge two or more equal sized chains
                    // we require user input to break the tie
                    PlaceTileResult::DecideTieBreak { tied_chains, mergers } => {
                        game.phase = Phase::Merge {
                            merging_player_id: self.current_player_id,
                            phase: MergePhase::AwaitingTiebreakSelection {
                                tied_chains
                            },
                            mergers_remaining: mergers,
                        };
                    }
                    // the tile placed merged two chains together without the need for a tiebreak
                    PlaceTileResult::Merge { mergers } => {
                        game.phase = Phase::Merge {
                            merging_player_id: self.current_player_id,
                            phase: MergePhase::AwaitingMergeDecision,
                            mergers_remaining: mergers,
                        };
                    }
                    // the tile was placed illegally
                    PlaceTileResult::Illegal { .. } => {
                        panic!("an action should not have been created to place an illegal tile");
                    }
                }
            }

            Action::SelectChainToCreate(player_id, chain) => {
                let pt = game.grid.previously_placed_tile_pt.expect("last tile pt should be Some()");
                game.grid.fill_chain(pt, chain);
                game.phase = Phase::AwaitingStockPurchase;

                // free stock for creating a chain
                if game.stocks.withdraw(chain, 1).is_ok() {
                    game.get_player_by_id_mut(player_id).stocks.deposit(chain, 1);
                }
            }

            Action::PurchaseStock(player_id, buys) => {
                for buy in buys {
                    match buy {
                        BuyOption::None => {}
                        BuyOption::Chain(chain) => {
                            game.stocks.withdraw(chain, 1).expect("enough stock to withdraw");

                            let mut player = game.get_player_by_id_mut(player_id);
                            player.stocks.deposit(chain, 1);
                            player.money -= money::chain_value(chain, self.grid.chain_size(chain))
                        }
                    }
                }

                game.player_take_tile(player_id);

                // todo: illegal tile replacement

                game.phase = Phase::AwaitingTilePlacement;
                game.go_next_turn();
            }

            Action::SelectChainForTiebreak(player_id, chain) => {
                match &mut game.phase {
                    Phase::Merge { phase, .. } => {
                        *phase = MergePhase::AwaitingMergeDecision
                    }
                    _ => panic!("phase should be 'Merge' already")
                }
            }

            Action::DecideMerge { .. } => {}
        }

        game
    }

    fn player_take_tile(&mut self, player_id: PlayerId) {
        if self.tiles.len() > 0 {
            let tile = self.tiles.remove(self.tiles.len() - 1);
            let mut player = self.get_player_by_id_mut(player_id);
            player.tiles.push(tile);
        }
    }

    fn go_next_turn(&mut self) {
        self.current_player_id = self.next_player_id();
        self.turn += 1;
    }

    fn get_player_by_id(&self, player_id: PlayerId) -> &Player {
        self.players.iter().find(|player| player.id == player_id).unwrap()
    }

    fn get_player_by_id_mut(&mut self, player_id: PlayerId) -> &mut Player {
        self.players.iter_mut().find(|player| player.id == player_id).unwrap()
    }

    fn next_player_id(&self) -> PlayerId {
        PlayerId((self.current_player_id.0 + 1) % self.players.len() as u8)
    }

    fn purchasable_combinations(&self, purchasing_player_id: PlayerId) -> Vec<[BuyOption; 3]> {
        let player = self.get_player_by_id(purchasing_player_id);
        let remaining_money = player.money;

        let mut combinations = vec![];

        let buy_options = {
            let mut buy_option_chains: Vec<BuyOption> = self.grid.existing_chains()
                .iter()
                .map(|chain| BuyOption::Chain(*chain))
                .collect();

            buy_option_chains.push(BuyOption::None);

            buy_option_chains
        };

        // this anonymous function is reused to
        // simulate purchasing each stock to determine if it's
        // possible to purchase the combination of stocks at all
        let can_buy = |buy_options: &[BuyOption; 3]| -> bool {
            let mut money = remaining_money;
            let mut stock = self.stocks.clone();

            for buy_option in buy_options {
                if let BuyOption::Chain(chain) = buy_option {
                    // check if there's enough stock left to buy
                    if stock.has_any(*chain) == false {
                        return false;
                    }

                    let cost = money::chain_value(*chain, self.grid.chain_size(*chain));

                    // check if there's enough money left to buy
                    if money < cost {
                        return false;
                    }

                    money -= cost;
                }
            }

            true
        };

        let num_buy_options = buy_options.len();
        for i in 0..num_buy_options {
            for j in i..num_buy_options {
                for k in j..num_buy_options {
                    let combination = [
                        buy_options[i],
                        buy_options[j],
                        buy_options[k]
                    ];

                    if can_buy(&combination) {
                        combinations.push(combination);
                    }
                }
            }
        }

        combinations
    }

    fn merge_combinations(&self, merging_player_id: PlayerId, merging_chains: MergingChains) -> Vec<MergeDecision> {
        let num_defunct_stock = self
            .get_player_by_id(merging_player_id)
            .stocks
            .amount(merging_chains.defunct_chain);

        let num_merging_stock_remaining = self
            .stocks
            .amount(merging_chains.merging_chain);

        let mut combinations = vec![];

        for sell_amount in 0..=num_defunct_stock {
            let half_of_remaining_stock = (num_defunct_stock - sell_amount) / 2;
            let trade_ins_possible = u8::min(half_of_remaining_stock, num_merging_stock_remaining);

            for trade_in_num in 0..=trade_ins_possible {
                combinations.push(MergeDecision {
                    sell: sell_amount,
                    trade_in: trade_in_num * 2,
                });
            }
        }

        combinations
    }
}


#[derive(Debug)]
enum Action {
    PlaceTile(PlayerId, Tile),
    PurchaseStock(PlayerId, [BuyOption; 3]),
    SelectChainToCreate(PlayerId, Chain),
    SelectChainForTiebreak(PlayerId, Chain),
    DecideMerge {
        merging_player_id: PlayerId,
        decision: MergeDecision,
    },
}

#[derive(Copy, Clone, Debug)]
pub struct MergeDecision {
    sell: u8,
    trade_in: u8,
    // 'keep' is the fallback
}

#[derive(Debug, Clone)]
enum Phase {
    AwaitingTilePlacement,
    AwaitingChainCreationSelection,
    AwaitingStockPurchase,
    Merge {
        merging_player_id: PlayerId,
        phase: MergePhase,
        mergers_remaining: Vec<MergingChains>,
    },
}

#[derive(Clone, Debug)]
enum MergePhase {
    AwaitingTiebreakSelection {
        tied_chains: Vec<Chain>
    },
    AwaitingMergeDecision,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct MergingChains {
    merging_chain: Chain,
    defunct_chain: Chain,
}


impl Display for Acquire {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("  Acquire: Turn {}", self.turn));
        writeln!(f);

        write!(f, "      ");
        for chain in &CHAIN_ARRAY {
            f.write_fmt(format_args!("{}", chain.initial()));
            write!(f, "  ");
        }
        write!(f, "Money");
        writeln!(f);

        for player in &self.players {
            if player.id == self.current_player_id {
                write!(f, "*");
            } else {
                write!(f, " ");
            }
            f.write_fmt(format_args!(" P{}: ", player.id.0));

            for chain in &CHAIN_ARRAY {
                f.write_fmt(format_args!("{: <3}", player.stocks.amount(*chain)));
            }
            f.write_fmt(format_args!("${}", player.money));

            writeln!(f);
        }

        f.write_fmt(format_args!("{}", self.grid));

        Ok(())
    }
}


#[derive(Copy, Clone, Eq, PartialEq)]
struct PlayerId(pub u8);

impl Debug for PlayerId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("P_{}", self.0))
    }
}

#[derive(Copy, Clone, Debug)]
enum BuyOption {
    None,
    Chain(Chain),
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


#[cfg(test)]
mod test {
    use rand::SeedableRng;
    use crate::{Acquire, Chain, Options, PlayerId, tile};
    use crate::grid::Slot;

    #[test]
    fn test_game() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(2);
        let game = Acquire::new(rng, &Options::default());

        println!("{}: {:?} | {:?}", game.turn, game.phase, game.actions());
        let game = game.apply_action(game.actions().remove(0));

        println!("{}: {:?} | {:?}", game.turn, game.phase, game.actions());
        let game = game.apply_action(game.actions().remove(0));

        println!("{}: {:?} | {:?}", game.turn, game.phase, game.actions());
        let game = game.apply_action(game.actions().remove(0));

        println!("{}: {:?} | {:?}", game.turn, game.phase, game.actions());
        let game = game.apply_action(game.actions().remove(0));

        println!("{}: {:?} | {:?}", game.turn, game.phase, game.actions());
    }

    #[test]
    fn test_game_up_to_merge() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(2);
        let game = Acquire::new(rng, &Options::default());

        // p1 place I11
        println!("{}: {:?} | {:?}", game.turn, game.phase, game.actions());
        let game = game.apply_action(game.actions().remove(0));

        assert_eq!(game.grid.get(tile!("I11")), Slot::NoChain);

        // p2 place H11
        println!("{}: {:?} | {:?}", game.turn, game.phase, game.actions());
        let game = game.apply_action(game.actions().remove(0));

        assert_eq!(game.grid.get(tile!("H11")), Slot::NoChain);

        // p2 Create Tower
        println!("{}: {:?} | {:?}", game.turn, game.phase, game.actions());
        let game = game.apply_action(game.actions().remove(0));

        // p2 purchase 3 Tower
        println!("{}: {:?} | {:?}", game.turn, game.phase, game.actions());
        let game = game.apply_action(game.actions().remove(0));

        // p3 place G12
        println!("{}: {:?} | {:?}", game.turn, game.phase, game.actions());
        let game = game.apply_action(game.actions().remove(4));

        // p3 purchase 3 Tower
        println!("{}: {:?} | {:?}", game.turn, game.phase, game.actions());
        let game = game.apply_action(game.actions().remove(0));

        // p3 place E12
        println!("{}: {:?} | {:?}", game.turn, game.phase, game.actions());
        let game = game.apply_action(game.actions().remove(2));

        // p4 purchase 3 Tower
        println!("{}: {:?} | {:?}", game.turn, game.phase, game.actions());
        let game = game.apply_action(game.actions().remove(0));

        // p1 place F12
        println!("{}: {:?} | {:?}", game.turn, game.phase, game.actions());
        let game = game.apply_action(game.actions().remove(1));

        // p1 create worldwide
        println!("{}: {:?} | {:?}", game.turn, game.phase, game.actions());
        let game = game.apply_action(game.actions().remove(2));

        // p1 purchase 2 tower 1 worldwide
        println!("{}: {:?} | {:?}", game.turn, game.phase, game.actions());
        let game = game.apply_action(game.actions().remove(1));

        // p2 place E11
        println!("{}: {:?} | {:?}", game.turn, game.phase, game.actions());
        let game = game.apply_action(game.actions().remove(5));
        // buy nothing
        let game = game.apply_action(game.actions().remove(game.actions().len() - 1));

        // p3 place E5
        println!("{}: {:?} | {:?}", game.turn, game.phase, game.actions());
        let game = game.apply_action(game.actions().remove(0));
        // buy nothing
        let game = game.apply_action(game.actions().remove(game.actions().len() - 1));

        // p4 place I6
        println!("{}: {:?} | {:?}", game.turn, game.phase, game.actions());
        let game = game.apply_action(game.actions().remove(0));
        // buy nothing
        let game = game.apply_action(game.actions().remove(game.actions().len() - 1));

        // p1 place H12
        println!("{}: {:?} | {:?}", game.turn, game.phase, game.actions());
        println!("{}", game);
        let game = game.apply_action(game.actions().remove(4));

        //merge!

        println!();
        println!("{}: {:?} | {:?}", game.turn, game.phase, game.actions());
        println!("{}", game);
    }

    #[test]
    fn test_purchase() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(2);
        let mut game = Acquire::new(rng, &Options::default());

        game.grid.place(tile!("A1"));
        game.grid.place(tile!("A2"));
        game.grid.fill_chain(tile!("A1"), Chain::American);
    }

    #[test]
    fn test_purchase_combinations() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(2);
        let mut game = Acquire::new(rng, &Options::default());

        game.grid.place(tile!("A1"));
        game.grid.place(tile!("A2"));
        game.grid.fill_chain(tile!("A1"), Chain::American);

        assert_eq!(game.purchasable_combinations(PlayerId(0)).len(), 4);

        game.players[0].money = 0;
        assert_eq!(game.purchasable_combinations(PlayerId(0)).len(), 1);

        game.players[0].money = 300;
        assert_eq!(game.purchasable_combinations(PlayerId(0)).len(), 2);

        game.players[0].money = 600;
        assert_eq!(game.purchasable_combinations(PlayerId(0)).len(), 3);

        game.players[0].money = 900;
        assert_eq!(game.purchasable_combinations(PlayerId(0)).len(), 4);

        game.players[0].money = 6000;

        game.grid.place(tile!("D1"));
        game.grid.place(tile!("D2"));
        game.grid.fill_chain(tile!("D1"), Chain::Luxor);

        assert_eq!(game.purchasable_combinations(PlayerId(0)).len(), 10);

        game.grid.place(tile!("F1"));
        game.grid.place(tile!("F2"));
        game.grid.fill_chain(tile!("F1"), Chain::Continental);

        assert_eq!(game.purchasable_combinations(PlayerId(0)).len(), 20);

        game.grid.place(tile!("H1"));
        game.grid.place(tile!("H2"));
        game.grid.fill_chain(tile!("H1"), Chain::Festival);
        assert_eq!(game.purchasable_combinations(PlayerId(0)).len(), 35);

        game.grid.place(tile!("A4"));
        game.grid.place(tile!("A5"));
        game.grid.fill_chain(tile!("A4"), Chain::Imperial);
        assert_eq!(game.purchasable_combinations(PlayerId(0)).len(), 56);

        game.grid.place(tile!("C4"));
        game.grid.place(tile!("C5"));
        game.grid.fill_chain(tile!("C4"), Chain::Tower);
        assert_eq!(game.purchasable_combinations(PlayerId(0)).len(), 84);

        game.players[0].money = 700;
        assert_eq!(game.purchasable_combinations(PlayerId(0)).len(), 35);
    }
}
