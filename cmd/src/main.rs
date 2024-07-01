use rand_chacha::rand_core::SeedableRng;
use acquire::{Acquire, Options};
use rand::seq::SliceRandom;
use rand::{RngCore, thread_rng};

fn main() {
    for n in 0..10_000 {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(thread_rng().next_u64());
        let mut game = Acquire::new(rand_chacha::ChaCha8Rng::seed_from_u64(thread_rng().next_u64()), &Options::default());

        loop {
            if game.is_terminated() {
                break;
            }

            let actions = game.actions();
            let Some(action) = actions.choose(&mut rng) else {
                println!("{}", game);
                panic!("out of stuff {n}")
            };

            game = game.apply_action(action.clone());
        }

        game.calculate_winners();

        //println!("{}", game);
    }

}
