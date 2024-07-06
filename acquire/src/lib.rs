mod tile;
mod grid;
mod money;
mod stock;
mod player;
mod chain;
mod ai;

use tile::Tile;
use std::fmt::{Debug, Display, Formatter};
use itertools::Itertools;
use rand::Rng;
use rand::seq::SliceRandom;
use chain::{Chain, CHAIN_ARRAY};
use player::Player;
use crate::chain::ChainTable;
use crate::grid::{Grid, Legality, PlaceTileResult, Slot};
use crate::stock::Stocks;


#[derive(Clone)]
pub struct Acquire {
    phase: Phase,
    players: Vec<Player>,
    tiles: Vec<Tile>,
    stocks: Stocks,
    grid: Grid,
    current_player_id: PlayerId,
    turn: u16,
    step: u16,
    terminated: bool,
}

pub struct Options {
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
    pub fn new<R: Rng>(rng: &mut R, options: &Options) -> Self {
        let grid = Grid::new(options.grid_width, options.grid_height);

        let mut tiles = vec![];
        for y in 0..grid.height as i8 {
            for x in 0..grid.width as i8 {
                tiles.push(Tile::new(x, y));
            }
        }

        tiles.shuffle(rng);

        let players = (0..options.num_players).map(|id| Player {
            id: PlayerId(id),
            tiles: (0..options.num_tiles).map(|_| tiles.remove(0)).collect(),
            stocks: Stocks::new(0),
            money: options.starting_money,
        }).collect();

        let stocks = Stocks::new(options.num_stock);

        Self {
            phase: Phase::AwaitingTilePlacement,
            players,
            tiles,
            stocks,
            grid,
            current_player_id: PlayerId(0),
            turn: 1,
            step: 0,
            terminated: false,
        }
    }

    pub fn actions(&self) -> Vec<Action> {
        match &self.phase {
            Phase::AwaitingTilePlacement => {
                self.tile_placement_actions()
            }

            Phase::AwaitingChainCreationSelection => {
                self.chain_selection_actions()
            }

            Phase::Merge { merging_player_id, phase: merge_phase, mergers_remaining } => {
                self.merge_actions(merging_player_id, merge_phase, mergers_remaining)
            }

            Phase::AwaitingStockPurchase => {
                self.stock_purchase_actions()
            }
            Phase::AwaitingGameTerminationDecision => {
                self.game_termination_actions()
            }
        }
    }

    #[inline(never)]
    fn tile_placement_actions(&self) -> Vec<Action> {
        let player = self.get_player_by_id(self.current_player_id);
        player.tiles.iter().filter_map(|tile| {
            match self.grid.get(tile.0) {
                Slot::Empty(legality) => {
                    match legality {
                        Legality::Legal => Some(Action::PlaceTile(self.current_player_id, *tile)),
                        Legality::TemporarilyIllegal |
                        Legality::PermanentIllegal => None,
                    }
                }

                _ => {
                    // theoretically the player should not have a tile that has already been placed,
                    // except i do that all the time during testing
                    #[cfg(not(test))]
                    panic!("player shouldn't have a tile that's already been placed");

                    #[cfg(test)]
                    None
                }
            }
        }).collect()
    }

    pub fn players(&self) -> &Vec<Player> {
        &self.players
    }


    #[inline(never)]
    fn chain_selection_actions(&self) -> Vec<Action> {
        self.grid.available_chains().into_iter().map(|chain| {
            Action::SelectChainToCreate(self.current_player_id, chain)
        }).collect()
    }

    #[inline(never)]
    fn merge_actions(&self, merging_player_id: &PlayerId, merge_phase: &MergePhase, mergers_remaining: &[MergingChains]) -> Vec<Action> {
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

    #[inline(never)]
    fn game_termination_actions(&self) -> Vec<Action> {
        if !self.may_terminate() {
            panic!("shouldn't be able to terminate");
        }

        vec![Action::Terminate(self.current_player_id, true), Action::Terminate(self.current_player_id, false)]
    }

    #[inline(never)]
    fn stock_purchase_actions(&self) -> Vec<Action> {
        self.purchasable_combinations(self.current_player_id)
            .iter()
            .map(|buy| {
                Action::PurchaseStock(self.current_player_id, *buy)
            })
            .collect()
    }

    pub fn apply_action(&self, action: Action) -> Acquire {
        let mut game = self.clone();


        #[cfg(test)]
        println!("S{}: {}", game.step, action);

        match action {
            Action::PlaceTile(player_id, tile) => {
                let player = game.get_player_by_id_mut(player_id);

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
                        // shortcut the purchase of stock when there are no chains to buy
                        if game.grid.existing_chains().is_empty() {
                            game.player_take_tile(player_id);
                            game.move_to_next_player_who_can_play_a_tile();
                        }
                    }
                    // the new tile created a chain, we need user input to select the hotel chain
                    PlaceTileResult::SelectAvailableChain => {
                        game.phase = Phase::AwaitingChainCreationSelection;
                    }
                    // the tile is going to merge two or more equal sized chains
                    // we require user input to break the tie
                    PlaceTileResult::DecideTieBreak { tied_chains } => {
                        game.phase = Phase::Merge {
                            merging_player_id: self.current_player_id,
                            mergers_remaining: vec![],
                            phase: MergePhase::AwaitingTiebreakSelection {
                                tied_chains
                            },
                        };
                    }
                    // the tile placed merged two chains together without the need for a tiebreak
                    PlaceTileResult::Merge { mut mergers } => {
                        for merger in &mut mergers {
                            let num = self.num_players_with_stock_in_chain(merger.defunct_chain);
                            merger.num_remaining_players_to_merge = Some(num);
                        }

                        mergers.retain(|merger| merger.num_remaining_players_to_merge != Some(0));

                        // apparently nobody benefits from any of the mergers
                        if mergers.is_empty() {
                            game.phase = Phase::AwaitingStockPurchase;
                        } else {
                            let first_defunct_chain = mergers[0].defunct_chain;

                            if let Some(next_merging_player_id) = self.next_merging_player_id(first_defunct_chain) {
                                game.provide_bonuses(first_defunct_chain);

                                game.phase = Phase::Merge {
                                    merging_player_id: next_merging_player_id,
                                    phase: MergePhase::AwaitingMergeDecision,
                                    mergers_remaining: mergers,
                                };
                            } else {
                                // somehow no one has any stake in the hotel.
                                // only possible with house rules allowing sale of stock
                                game.phase = Phase::AwaitingStockPurchase;
                            }
                        }
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

                            let player = game.get_player_by_id_mut(player_id);
                            player.stocks.deposit(chain, 1);
                            player.money -= money::chain_value(chain, self.grid.chain_size(chain))
                        }
                    }
                }

                game.player_take_tile(player_id);
                game.player_trade_in_illegal_tiles(player_id);

                if game.may_terminate() {
                    game.phase = Phase::AwaitingGameTerminationDecision;
                } else {
                    game.move_to_next_player_who_can_play_a_tile();
                }
            }

            Action::SelectChainForTiebreak(_, tiebreak_chain) => {
                match &mut game.phase {
                    Phase::Merge { phase: merge_phase, mergers_remaining, .. } => {
                        if let MergePhase::AwaitingTiebreakSelection { tied_chains } = merge_phase {
                            for defunct_chain in tied_chains.iter().filter(|chain| **chain != tiebreak_chain) {
                                // use self here to avoid interior mutability issues
                                let num = self.num_players_with_stock_in_chain(*defunct_chain);
                                mergers_remaining.push(MergingChains {
                                    merging_chain: tiebreak_chain,
                                    defunct_chain: *defunct_chain,
                                    num_remaining_players_to_merge: Some(num),
                                });
                            }

                            *merge_phase = MergePhase::AwaitingMergeDecision;

                            let first_defunct_chain = mergers_remaining[0].defunct_chain;
                            game.provide_bonuses(first_defunct_chain);
                        } else {
                            panic!("supposed to be awaiting a tiebreak")
                        }
                    }
                    _ => panic!("phase should be 'Merge' already")
                }
            }

            Action::DecideMerge { decision, merging_player_id: action_merging_player_id } => {
                let next_merging_player_id = match &game.phase {
                    Phase::Merge { mergers_remaining, merging_player_id, .. } => {
                        assert_eq!(action_merging_player_id, *merging_player_id);

                        let merging_chains = mergers_remaining[0];
                        let defunct_chain_size = game.grid.chain_size(merging_chains.defunct_chain);

                        let player = game.get_player_by_id_mut(*merging_player_id);
                        player.stocks.withdraw(merging_chains.defunct_chain, decision.sell + decision.trade_in).expect("enough stock to sell & trade-in");
                        player.money += money::chain_value(merging_chains.defunct_chain, defunct_chain_size) * decision.sell as u32;
                        player.stocks.deposit(merging_chains.merging_chain, decision.trade_in / 2);

                        game.stocks.withdraw(merging_chains.merging_chain, decision.trade_in / 2).expect("enough stock to trade-in for");
                        game.stocks.deposit(merging_chains.defunct_chain, decision.sell + decision.trade_in);

                        game.next_merging_player_id(merging_chains.defunct_chain)
                    }
                    _ => panic!("should not be able to decide to merge when the game phase is not a merger")
                };

                // need to do this in a second step due to borrowing rules
                if let Phase::Merge { merging_player_id, mergers_remaining, .. } = &mut game.phase {
                    if let Some(next_merge_player_id) = next_merging_player_id {
                        *merging_player_id = next_merge_player_id;

                        let current_merger = &mut mergers_remaining[0];
                        let num_remaining_players_to_merge = current_merger.num_remaining_players_to_merge.as_mut().unwrap();
                        *num_remaining_players_to_merge -= 1;

                        // finished the merge
                        if *num_remaining_players_to_merge == 0 {
                            // strike off this merge, if there's another then we continue,
                            // everything should work the same for merge 2+
                            let merger = mergers_remaining.remove(0);

                            *merging_player_id = self.current_player_id;

                            // if there are no more mergers left to do,
                            // we can move on to the stock purchase phase
                            if mergers_remaining.is_empty() {
                                game.phase = Phase::AwaitingStockPurchase;
                                game.grid.fill_chain(game.grid.previously_placed_tile_pt.expect("a previously placed tile"), merger.merging_chain);
                            }
                        }
                    } else {
                        let merger = mergers_remaining.remove(0);

                        *merging_player_id = self.current_player_id;

                        // if there are no more mergers left to do,
                        // we can move on to the stock purchase phase
                        if mergers_remaining.is_empty() {
                            game.phase = Phase::AwaitingStockPurchase;
                            game.grid.fill_chain(game.grid.previously_placed_tile_pt.expect("a previously placed tile"), merger.merging_chain);
                        } else {
                            let first_defunct_chain = mergers_remaining[0].defunct_chain;
                            game.provide_bonuses(first_defunct_chain);
                        }
                    }
                }
            }
            Action::Terminate(_, terminate) => {
                game.terminated = terminate;

                if game.terminated {
                    game.provide_final_bonuses();
                } else {
                    game.move_to_next_player_who_can_play_a_tile();
                }
            }
        }

        if game.terminated {
            return game;
        }

        game.step += 1;

        game
    }

    pub fn is_terminated(&self) -> bool {
        self.terminated
    }

    pub fn winners(&self) -> Vec<PlayerId> {
        let most_money = self.players.iter().map(|player| player.money).max().unwrap();

        self.players.iter().filter_map(|player| {
            if player.money == most_money {
                Some(player.id)
            } else {
                None
            }
        }).collect()
    }

    fn provide_final_bonuses(&mut self) {
        for chain in &CHAIN_ARRAY {
            self.provide_bonuses(*chain);
        }
    }

    fn move_to_next_player_who_can_play_a_tile(&mut self) {
        let mut count = 0;
        loop {
            self.phase = Phase::AwaitingTilePlacement;
            self.go_next_turn();

            if self.player_has_any_valid_tiles(self.current_player_id) {
                break;
            }

            self.player_trade_in_illegal_tiles(self.current_player_id);

            count += 1;

            if count == self.players.len() * 2 {
                self.terminated = true;
                self.provide_final_bonuses();
                break;
            }
        }
    }

    fn may_terminate(&self) -> bool {
        self.grid.all_chains_are_safe() || self.grid.game_ending_chain_exists()
    }

    fn player_has_any_valid_tiles(&mut self, player_id: PlayerId) -> bool {
        let player = self.get_player_by_id(player_id);
        player.tiles.iter().any(|tile| {
            match self.grid.get(tile.0) {
                Slot::Empty(legality) => {
                    match legality {
                        Legality::Legal => true,
                        Legality::TemporarilyIllegal |
                        Legality::PermanentIllegal => false,
                    }
                }
                _ => panic!("player shouldn't have any tiles that are already placed"),
            }
        })
    }

    fn provide_bonuses(&mut self, chain: Chain) {
        let bonuses = self.chain_bonus(chain);
        for (player_id, bonus) in bonuses {
            #[cfg(test)]
            println!("Player {} received a bonus of ${bonus}", player_id.0);
            self.get_player_by_id_mut(player_id).money += bonus;
        }
    }

    fn player_take_tile(&mut self, player_id: PlayerId) {
        if !self.tiles.is_empty() {
            let tile = self.tiles.remove(self.tiles.len() - 1);
            let player = self.get_player_by_id_mut(player_id);
            player.tiles.push(tile);
        }
    }

    fn player_trade_in_illegal_tiles(&mut self, player_id: PlayerId) {
        let grid = self.grid.clone();
        let num_remaining_tiles = self.tiles.len();

        let tiles_to_draw = {
            let player = self.get_player_by_id_mut(player_id);
            player.tiles = player.tiles
                .iter()
                .filter(|tile| {
                    match grid.get(tile.0) {
                        Slot::Empty(legality) => {
                            match legality {
                                Legality::Legal |
                                Legality::TemporarilyIllegal => true,
                                Legality::PermanentIllegal => false,
                            }
                        }
                        _ => {
                            // theoretically the player should not have a tile that has already been placed,
                            // except i do that all the time during testing
                            #[cfg(not(test))]
                            panic!("player shouldn't have a tile that's already been placed");

                            #[cfg(test)]
                            true
                        }
                    }

                })
                .copied()
                .collect();

            let required_tiles: usize = 6 - player.tiles.len();
            required_tiles.min(num_remaining_tiles)
        };

        #[cfg(test)]
        if tiles_to_draw > 0 {
            println!("Player {} replaces {} of their illegal tiles.", player_id.0, tiles_to_draw);
        }

        // have to do some weird shit in here to deal with interior mutability
        for _ in 0..tiles_to_draw {
            let tile = self.tiles.remove(self.tiles.len() - 1);
            let player = self.get_player_by_id_mut(player_id);
            player.tiles.push(tile);
        }
    }

    fn go_next_turn(&mut self) {
        self.current_player_id = self.next_player_id();
        self.turn += 1;
    }

    pub fn get_player_by_id(&self, player_id: PlayerId) -> &Player {
        &self.players[player_id.0 as usize]
    }

    fn get_player_by_id_mut(&mut self, player_id: PlayerId) -> &mut Player {
        &mut self.players[player_id.0 as usize]
    }

    fn next_player_id(&self) -> PlayerId {
        PlayerId((self.current_player_id.0 + 1) % self.players.len() as u8)
    }

    fn num_players_with_stock_in_chain(&self, chain: Chain) -> u8 {
        self.players.iter().filter(|player| player.stocks.has_any(chain)).count() as u8
    }

    fn next_merging_player_id(&self, chain: Chain) -> Option<PlayerId> {
        match self.phase {
            Phase::AwaitingTilePlacement => {
                // the last action was to enter a merge phase, so the first merging player is the
                // first player with stock in the defunct chain, starting from the current player

                self.player_ids_in_order(self.current_player_id).into_iter().find(|player_id| {
                    self.get_player_by_id(*player_id).stocks.has_any(chain)
                })
            }
            Phase::Merge { merging_player_id, .. } => {
                self.player_ids_in_order(merging_player_id).into_iter().find(|player_id| {
                    *player_id != merging_player_id &&
                        *player_id != self.current_player_id &&
                        self.get_player_by_id(*player_id).stocks.has_any(chain)
                })
            }
            _ => panic!("invalid phase to call this fn in this phase")
        }
    }

    fn player_ids_in_order(&self, starting_player_id: PlayerId) -> Vec<PlayerId> {
        (0..self.players.len() as u8).map(|n| {
            PlayerId((starting_player_id.0 + n) % self.players.len() as u8)
        }).collect()
    }


    fn purchasable_combinations(&self, purchasing_player_id: PlayerId) -> Vec<[BuyOption; 3]> {
        let player = self.get_player_by_id(purchasing_player_id);
        let remaining_money = player.money;

        let mut combinations = vec![];

        let buy_options = {
            let mut buy_option_chains: Vec<BuyOption> = self.grid.existing_chains()
                .iter()
                .sorted()
                .map(|chain| BuyOption::Chain(*chain))
                .collect();

            buy_option_chains.push(BuyOption::None);

            buy_option_chains
        };

        let mut chain_values: ChainTable<u32> = ChainTable::default();
        for chain in &CHAIN_ARRAY {
            chain_values.set(chain, money::chain_value(*chain, self.grid.chain_size(*chain)))
        }

        // this anonymous function is reused to
        // simulate purchasing each stock to determine if it's
        // possible to purchase the combination of stocks at all
        let can_buy = |buy_options: &[BuyOption; 3]| -> bool {
            let mut money = remaining_money;
            let mut stock = self.stocks.clone();

            for buy_option in buy_options {
                if let BuyOption::Chain(chain) = buy_option {
                    // check if there's enough stock left to buy
                    if !stock.has_any(*chain) {
                        return false;
                    }

                    let cost = chain_values.get(chain);

                    // check if there's enough money left to buy
                    if money < cost {
                        return false;
                    }

                    money -= cost;
                    stock.withdraw(*chain, 1).expect("a stock");
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
                    merging_chains,
                    sell: sell_amount,
                    trade_in: trade_in_num * 2,
                });
            }
        }

        combinations
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Action {
    PlaceTile(PlayerId, Tile),
    PurchaseStock(PlayerId, [BuyOption; 3]),
    SelectChainToCreate(PlayerId, Chain),
    SelectChainForTiebreak(PlayerId, Chain),
    DecideMerge {
        merging_player_id: PlayerId,
        decision: MergeDecision,
    },
    Terminate(PlayerId, bool),
}

#[allow(unused_must_use)]
impl Display for Action {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::PlaceTile(player_id, tile) => {
                f.write_fmt(format_args!("Player {} places tile {}", player_id.0, tile))
            }

            Action::PurchaseStock(player_id, buys) => {
                if buys.iter().all(|buy| matches!(buy, BuyOption::None)) {
                    return f.write_fmt(format_args!("Player {} does not buy any stocks.", player_id.0));
                }

                f.write_fmt(format_args!("Player {} buys ", player_id.0));

                let counts = buys.iter().counts();
                for (idx, (chain, count)) in counts.iter().enumerate() {
                    if let BuyOption::Chain(chain) = chain {
                        f.write_fmt(format_args!("{} {:?}", count, chain));
                        if idx < counts.len() - 1 {
                            f.write_fmt(format_args!(", "));
                        }
                    }
                }

                Ok(())
            }

            Action::SelectChainToCreate(player_id, chain) => {
                f.write_fmt(format_args!("Player {} chooses to create {:?}", player_id.0, chain))
            }

            Action::SelectChainForTiebreak(player_id, chain) => {
                f.write_fmt(format_args!("Player {} chooses {:?} as the merge winner.", player_id.0, chain))
            }

            Action::DecideMerge { merging_player_id, decision } => {
                return if decision.sell == 0 && decision.trade_in == 0 {
                    f.write_fmt(format_args!("Player {} decides to keep their stock in {:?}.", merging_player_id.0, decision.merging_chains.defunct_chain))
                } else if decision.sell != 0 && decision.trade_in == 0 {
                    f.write_fmt(format_args!("Player {} sells {} {:?}.", merging_player_id.0, decision.sell, decision.merging_chains.defunct_chain))
                } else if decision.sell == 0 && decision.trade_in != 0 {
                    f.write_fmt(format_args!(
                        "Player {} trades in {} {:?} for {} {:?}.",
                        merging_player_id.0,
                        decision.trade_in,
                        decision.merging_chains.defunct_chain,
                        decision.trade_in / 2,
                        decision.merging_chains.merging_chain
                    ))
                } else {
                    f.write_fmt(format_args!(
                        "Player {} sells {} {:?} and trades in {} {:?} for {} {:?}.",
                        merging_player_id.0,
                        decision.sell,
                        decision.merging_chains.defunct_chain,
                        decision.trade_in,
                        decision.merging_chains.defunct_chain,
                        decision.trade_in / 2,
                        decision.merging_chains.merging_chain
                    ))
                };
            }
            Action::Terminate(player_id, terminate) => {
                if *terminate {
                    f.write_fmt(format_args!("Player {} chooses to terminate the game.", player_id.0))
                } else {
                    f.write_fmt(format_args!("Player {} chooses to prolong the game.", player_id.0))
                }
            }
        }
    }
}

#[derive(Copy, Clone, Debug,  Eq, PartialEq, Hash)]
pub struct MergeDecision {
    merging_chains: MergingChains,
    sell: u8,
    trade_in: u8,
    // 'keep' is the fallback
}

#[derive(Debug, Clone)]
enum Phase {
    AwaitingTilePlacement,
    AwaitingChainCreationSelection,
    AwaitingStockPurchase,
    AwaitingGameTerminationDecision,
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

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct MergingChains {
    merging_chain: Chain,
    defunct_chain: Chain,
    num_remaining_players_to_merge: Option<u8>,
}

#[allow(unused_must_use)]
impl Display for Acquire {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("  Acquire: Turn {} | Tiles Left {}", self.turn, self.tiles.len()));
        writeln!(f);

        write!(f, "        ");
        for chain in &CHAIN_ARRAY {
            f.write_fmt(format_args!("{}", chain.initial()));
            write!(f, "   ");
        }
        writeln!(f);

        write!(f, " Size:  ");
        for chain in &CHAIN_ARRAY {
            f.write_fmt(format_args!("{: <4}", self.grid.chain_size(*chain)));
        }
        writeln!(f);

        write!(f, "Stock:  ");
        for chain in &CHAIN_ARRAY {
            f.write_fmt(format_args!("{: <4}", self.stocks.amount(*chain)));
        }

        write!(f, "Money    ");
        write!(f, "Tiles");

        writeln!(f);

        for player in &self.players {
            if player.id == self.current_player_id {
                write!(f, "*");
            } else {
                write!(f, " ");
            }
            f.write_fmt(format_args!("  P{}:  ", player.id.0));

            for chain in &CHAIN_ARRAY {
                f.write_fmt(format_args!("{: <4}", player.stocks.amount(*chain)));
            }
            f.write_fmt(format_args!("${: <8}", player.money));
            f.write_fmt(format_args!("{}", player.tiles.len()));

            writeln!(f);
        }

        f.write_fmt(format_args!("{}", self.grid));

        Ok(())
    }
}


#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct PlayerId(pub u8);

impl Debug for PlayerId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("P_{}", self.0))
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum BuyOption {
    None,
    Chain(Chain),
}

#[cfg(test)]
mod test {
    use rand::SeedableRng;
    use rand::seq::SliceRandom;
    use crate::{Acquire, Options, Phase, PlayerId, tile};
    use crate::chain::Chain;
    use crate::grid::Slot;

    fn game_test_instance() -> Acquire {
        let rng = rand_chacha::ChaCha8Rng::seed_from_u64(2);
        Acquire::new(rng, &Options::default())
    }

    #[test]
    fn test_game_up_to_merge() {
        let game = game_test_instance();

        let game = game.apply_action(game.actions().remove(0));
        assert_eq!(game.grid.get(tile!("I11")), Slot::NoChain);

        let game = game.apply_action(game.actions().remove(0));
        assert_eq!(game.grid.get(tile!("H11")), Slot::NoChain);

        println!("{}", game);
    }


    #[test]
    fn test_purchase_combinations() {
        let rng = rand_chacha::ChaCha8Rng::seed_from_u64(2);
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

    #[test]
    fn test_player_ids_in_order() {
        let rng = rand_chacha::ChaCha8Rng::seed_from_u64(2);
        let game = Acquire::new(rng, &Options::default());

        assert_eq!(game.player_ids_in_order(PlayerId(0)), vec![
            PlayerId(0),
            PlayerId(1),
            PlayerId(2),
            PlayerId(3),
        ]);

        assert_eq!(game.player_ids_in_order(PlayerId(1)), vec![
            PlayerId(1),
            PlayerId(2),
            PlayerId(3),
            PlayerId(0),
        ]);

        assert_eq!(game.player_ids_in_order(PlayerId(3)), vec![
            PlayerId(3),
            PlayerId(0),
            PlayerId(1),
            PlayerId(2),
        ]);
    }

    #[test]
    fn test_four_way_merge() {
        let rng = rand_chacha::ChaCha8Rng::seed_from_u64(2);
        let mut game = Acquire::new(rng, &Options::default());

        game.grid.place(tile!("D1"));
        game.grid.place(tile!("D2"));
        game.grid.fill_chain(tile!("D2"), Chain::American);

        game.grid.place(tile!("D4"));
        game.grid.place(tile!("D5"));
        game.grid.fill_chain(tile!("D5"), Chain::Festival);

        game.grid.place(tile!("B3"));
        game.grid.place(tile!("C3"));
        game.grid.fill_chain(tile!("C3"), Chain::Continental);

        game.grid.place(tile!("E3"));
        game.grid.place(tile!("F3"));
        game.grid.fill_chain(tile!("F3"), Chain::Tower);

        game.players[0].tiles[0] = tile!("D3");

        game = game.apply_action(game.actions().remove(0));

        let a = game.actions();
        // should be one action for each way we can merge the chains together
        assert_eq!(a.len(), 4);
        game = game.apply_action(game.actions().remove(1));

        game = game.apply_action(game.actions().remove(0));

        game = game.apply_action(game.actions().remove(0));

        game = game.apply_action(game.actions().remove(0));

        game = game.apply_action(game.actions().remove(0));

        game.apply_action(game.actions().remove(0));
    }

    #[test]
    fn test_four_way_merge_with_stakes() {
        let rng = rand_chacha::ChaCha8Rng::seed_from_u64(2);
        let mut game = Acquire::new(rng, &Options::default());

        game.grid.place(tile!("D1"));
        game.grid.place(tile!("D2"));
        game.grid.fill_chain(tile!("D2"), Chain::American);

        game.grid.place(tile!("D4"));
        game.grid.place(tile!("D5"));
        game.grid.fill_chain(tile!("D5"), Chain::Festival);

        game.grid.place(tile!("B3"));
        game.grid.place(tile!("C3"));
        game.grid.fill_chain(tile!("C3"), Chain::Continental);

        game.grid.place(tile!("E3"));
        game.grid.place(tile!("F3"));
        game.grid.fill_chain(tile!("F3"), Chain::Tower);

        game.players[0].stocks.deposit(Chain::American, 3);
        game.players[0].stocks.deposit(Chain::Festival, 3);
        game.players[0].stocks.deposit(Chain::Continental, 3);
        game.players[0].stocks.deposit(Chain::Tower, 3);

        game.players[1].stocks.deposit(Chain::American, 1);
        game.players[1].stocks.deposit(Chain::Festival, 2);
        game.players[1].stocks.deposit(Chain::Continental, 3);
        game.players[1].stocks.deposit(Chain::Tower, 4);

        game.players[2].stocks.deposit(Chain::American, 5);
        game.players[2].stocks.deposit(Chain::Festival, 3);
        game.players[2].stocks.deposit(Chain::Continental, 2);
        game.players[2].stocks.deposit(Chain::Tower, 0);

        game.players[3].stocks.deposit(Chain::American, 8);
        game.players[3].stocks.deposit(Chain::Festival, 0);
        game.players[3].stocks.deposit(Chain::Continental, 2);
        game.players[3].stocks.deposit(Chain::Tower, 1);


        game.players[0].tiles[0] = tile!("D3");

        game = game.apply_action(game.actions().remove(0));

        // should be one action for each way we can merge the chains together
        assert_eq!(game.actions().len(), 4);
        game = game.apply_action(game.actions().remove(0));


        assert_eq!(game.players[0].stocks.amount(Chain::Festival), 3);
        assert_eq!(game.players[0].stocks.amount(Chain::Tower), 3);
        assert_eq!(game.players[0].money, 7500);

        // Player 0 sells 1 and trades-in 2 for 1. (Festival)
        game = game.apply_action(game.actions().remove(3));

        assert_eq!(game.players[0].stocks.amount(Chain::Festival), 0);
        assert_eq!(game.players[0].stocks.amount(Chain::Tower), 4);
        assert_eq!(game.players[0].money, 7800);


        assert_eq!(game.players[1].stocks.amount(Chain::Festival), 2);
        assert_eq!(game.players[1].money, 6000);

        // Player 1 sells 2. (Festival)
        game = game.apply_action(game.actions().remove(3));


        assert_eq!(game.players[2].stocks.amount(Chain::Festival), 3);
        assert_eq!(game.players[2].money, 7500);

        // Player 2 sells 3.
        game = game.apply_action(game.actions().remove(5));

        assert_eq!(game.players[2].stocks.amount(Chain::Festival), 0);
        assert_eq!(game.players[2].money, 8400);

        // Player 3 has no stake in fesitval


        match game.phase {
            Phase::Merge { merging_player_id, .. } => {
                assert_eq!(merging_player_id, PlayerId(0))
            }
            _ => panic!("game not in correct state")
        }

        game.apply_action(game.actions().remove(2));
    }

    #[test]
    fn test_growth() {
        let mut game = Acquire::new(rand_chacha::ChaCha8Rng::seed_from_u64(2), &Options::default());

        game.grid.place(tile!("A4"));
        game.grid.place(tile!("B3"));

        game.grid.place(tile!("A1"));
        game.grid.place(tile!("A2"));
        game.grid.fill_chain(tile!("A2"), Chain::Festival);

        game.grid.place(tile!("A3"));

        assert_eq!(game.grid.get(tile!("A3")), Slot::Chain(Chain::Festival));
        assert_eq!(game.grid.get(tile!("A4")), Slot::Chain(Chain::Festival));
        assert_eq!(game.grid.get(tile!("B3")), Slot::Chain(Chain::Festival));
    }

    #[test]
    fn test_random_games() {
        for n in 0..100 {
            let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(n);
            let mut game = Acquire::new(rand_chacha::ChaCha8Rng::seed_from_u64(n), &Options::default());

            for _ in 0..200 {
                if game.is_terminated() {
                    break;
                }

                let actions = game.actions();
                let action = actions.choose(&mut rng).expect("an action");

                game = game.apply_action(action.clone());
            }

            let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(2);
            let mut game = Acquire::new(rand_chacha::ChaCha8Rng::seed_from_u64(2), &Options::default());

            loop {
                if game.is_terminated() {
                    break;
                }

                let actions = game.actions();
                if actions.len() == 0 {
                    println!("{}", game);
                    #[allow(unused)]
                    let actions = game.actions();
                }
                let action = actions.choose(&mut rng).expect("an action");

                game = game.apply_action(action.clone());
            }
        }
    }
}
