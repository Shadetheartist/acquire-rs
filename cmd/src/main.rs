use rand_chacha::rand_core::SeedableRng;
use acquire::{Acquire, Options};
use rand::seq::SliceRandom;

fn main() {
    for n in 0..4 {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(n);
        let mut game = Acquire::new(rand_chacha::ChaCha8Rng::seed_from_u64(n), &Options::default());

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

        println!("{}", game);

    }

}
