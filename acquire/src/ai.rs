use ahash::HashMap;
use ai::{Determinable, Mcts, Outcome};
use rand::prelude::SliceRandom;
use rand::Rng;
use crate::{Acquire, Action, Phase, PlayerId};

impl Determinable<PlayerId, Action, Acquire> for Acquire {
    fn determine<R: Rng>(&self, rng: &mut R, perspective_player: PlayerId) -> Acquire {
        let mut game = self.clone();

        // store current player tiles counts, so we can reimburse them with the correct number of tiles
        let players_tile_counts: HashMap<PlayerId, usize> = game.players.iter().map(|p| (p.id, p.tiles.len())).collect();

        // put all player tiles back into the bank
        for p in &mut game.players {

            // perceiving player knows what they have, and so is unaffected
            if p.id == perspective_player {
                continue;
            }

            for _ in 0..players_tile_counts[&p.id] {
                game.tiles.push(p.tiles.remove(p.tiles.len() - 1));
            }
        }

        // shuffle the bank
        game.tiles.shuffle(rng);

        // draw new tiles
        for p in &mut game.players {
            if p.id == perspective_player {
                continue;
            }

            for _ in 0..players_tile_counts[&p.id] {
                p.tiles.push(game.tiles.remove(game.tiles.len() - 1));
            }
        }

        // result is a new game but with each player's tiles randomized, other than the perceiving player

        game
    }
}

impl Mcts<PlayerId, Action> for Acquire {
    type Error = ();

    fn actions(&self) -> Vec<Action> {
        self.actions()
    }

    fn apply_action<R: Rng + Sized>(&self, action: &Action, _: &mut R) -> Result<Self, Self::Error> where Self: Sized {
        Ok(self.apply_action(action.clone()))
    }

    fn outcome(&self) -> Option<Outcome<PlayerId>> {
        if !self.is_terminated() {
            return None;
        } else {
            let winners = self.winners();
            if winners.len() == 1 {
                return Some(Outcome::Winner(winners[0]));
            } else if winners.len() > 1 {
                return Some(Outcome::Draw(winners));
            } else {
                panic!("no winners");
            }

        }
    }

    fn current_player(&self) -> PlayerId {
        match self.phase {
            Phase::Merge { merging_player_id, .. } => merging_player_id,
            _ => self.current_player_id,
        }

    }

    fn players(&self) -> Vec<PlayerId> {
        self.players.iter().map(|p| p.id).collect()
    }
}

impl ai::Player for PlayerId {}
impl ai::Action for Action {}
