use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;
use bg_ai::ismcts::{IsMctsMtAgent, MtAgent};
use rand_chacha::rand_core::SeedableRng;
use acquire::{Acquire, Action, Options, PlayerId};

#[derive(Debug)]
struct HumanAgent {
    player_id: PlayerId,
}

impl IsMctsMtAgent<rand_chacha::ChaCha8Rng, Acquire, Action, PlayerId> for HumanAgent where
{
    fn player(&self) -> PlayerId {
        self.player_id
    }

    fn decide(&self, _: &mut rand_chacha::ChaCha8Rng, state: &Acquire) -> Option<Action> {

        let mut actions = state.actions();

        let mut grid = state.grid().clone();

        print!("Your Tiles: ");
        for tile in &state.get_player_by_id(self.player_id).tiles {
            grid.indicators.insert((*tile).into());
            print!("{}, ", tile);
        }
        println!();
        println!("{}", grid);
        println!();


        println!("Choose an action");

        loop {
            for (idx, action) in actions.iter().enumerate() {
                println!("\t{} - {}", idx + 1, action);
            }

            let mut line = String::new();
            std::io::stdin().read_line(&mut line).unwrap();

            if let Ok(decision) = usize::from_str(line.trim()) {
                return Some(actions.remove(decision - 1))
            } else {
                println!("Invalid action.");
            }
        }
    }
}

impl Display for HumanAgent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}", self.player_id))
    }
}


fn main() {
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(2);
    let initial_game_state = Acquire::new(&mut rng, &Options::default());
    let agents: HashMap<PlayerId, Box<dyn IsMctsMtAgent<rand_chacha::ChaCha8Rng, Acquire, Action, PlayerId>>> = initial_game_state
        .players()
        .iter()
        .enumerate()
        .map(|(idx, player)| {
            let agent = {
                if idx == 0 {
                    (
                        || Box::new(HumanAgent {
                            player_id: PlayerId(0)
                        }) as _
                    )()
                } else {
                    (
                        || Box::new(MtAgent {
                            player: player.id,
                            num_simulations: 100 + 250 * idx as u32,
                            num_determinations: 4 + 4 * idx as u32,
                        }) as _
                    )()
                }
            };

            (
                player.id, agent
            )
        }).collect();

    let mut game = bg_ai::ismcts::MultithreadedInformationSetGame::new(rng, initial_game_state, agents);

    loop {
        if game.is_terminated() {
            break;
        }

        let action = game.step().unwrap();

        println!("{}", action);
        println!("{}", game.state);
    }
}
