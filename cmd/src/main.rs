use std::fs;
use rand_chacha::rand_core::SeedableRng;
use acquire::{Acquire, Options};
use ai;
use ai::DecisionTree;
use petgraph_graphml::GraphMl;

fn main() {

    let rng = rand_chacha::ChaCha8Rng::seed_from_u64(1);
    let mut game = Acquire::new(rand_chacha::ChaCha8Rng::seed_from_u64(1), &Options::default());

    loop {

        if game.is_terminated() {
            break;
        }

        let action = ai::ismcts_mt(&game, &rng, 1, 100);

        game = game.apply_action(action.clone());

        println!("{}", action);
        println!("{}", game);
    }
}


fn write_tree_to_file(){
    let mut game = Acquire::new(rand_chacha::ChaCha8Rng::seed_from_u64(1), &Options::default());

    let mut tree = DecisionTree::new(game.clone());

    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(1);

    tree.search_n(&mut rng, 10000);

    let graphml = GraphMl::new(tree.graph())
        .export_node_weights(Box::new(|node| {
            vec![
                ("num visits".into(), node.num_visits.to_string().into()),
                ("state".into(), format!("{}", node.state).into()),
                ("scores".into(), format!("{:?}", node.scores).into()),
            ]
        }))
        .export_edge_weights(Box::new(|edge| {
            vec![
                ("weight".into(), edge.num_visits.to_string().into()),
                ("action".into(), format!("{:?}", edge.action).to_string().into()),
            ]
        }));

    let file_writer = fs::File::create("output.graphml").unwrap();
    graphml.to_writer(file_writer).unwrap();
}