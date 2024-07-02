use rand_chacha::rand_core::SeedableRng;
use acquire::{Acquire, Options};
use ai;

fn main() {

    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(2);
    let mut game = Acquire::new(rand_chacha::ChaCha8Rng::seed_from_u64(2), &Options::default());

    loop {
        if game.is_terminated() {
            break;
        }

        let action = ai::ismcts_mt(&game, &rng, 12, 500);

        game = game.apply_action(action.clone());

        println!("{}", action);
        println!("{}", game);
    }
}
