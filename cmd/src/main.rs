use std::fs;
use rand_chacha::rand_core::SeedableRng;
use acquire::{Acquire, Options};
use ai;
use ai::DecisionTree;

fn main() {

    let rng = rand_chacha::ChaCha8Rng::seed_from_u64(1);
    let mut game = Acquire::new(rand_chacha::ChaCha8Rng::seed_from_u64(1), &Options::default());

    loop {

        if game.is_terminated() {
            break;
        }

        let action = ai::ismcts_mt(&game, &rng, 100, 1000);

        game = game.apply_action(action.clone());

        println!("{}", action);
        println!("{}", game);
    }
}

#[allow(dead_code)]
fn write_tree_to_file(game: &Acquire){
    let mut tree = DecisionTree::new(game.clone());

    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(1);

    tree.search_n(&mut rng, 40000);

    let mut file_writer = fs::File::create("output.graphml").unwrap();
    tree.write_graphml(&mut file_writer).unwrap();
}