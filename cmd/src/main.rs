use std::collections::HashMap;
use bg_ai::ismcts::MtAgent;
use rand_chacha::rand_core::SeedableRng;
use acquire::{Acquire, Options, PlayerId};


fn main() {
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(2);
    let initial_game_state = Acquire::new(&mut rng, &Options::default());
    let agents: HashMap<PlayerId, MtAgent<PlayerId>> = initial_game_state
        .players()
        .iter()
        .enumerate()
        .map(|(idx, player)| (
            player.id,
            MtAgent {
                player: player.id,
                num_simulations: 100 + 250 * idx as u32,
                num_determinations: 4 + 4 * idx as u32,
            }
        )).collect();

    println!("{:?}", agents);

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
