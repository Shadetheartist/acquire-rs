use criterion::{criterion_group, criterion_main, Criterion};
use rand::prelude::SliceRandom;
use rand::{RngCore, SeedableRng, thread_rng};
use acquire::{Acquire, Options};

fn run_game() {
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(thread_rng().next_u64());
    let mut game = Acquire::new(rand_chacha::ChaCha8Rng::seed_from_u64(thread_rng().next_u64()), &Options::default());

    loop {
        if game.is_terminated() {
            break;
        }

        let actions = game.actions();
        let action = actions.choose(&mut rng).expect("an action");

        game = game.apply_action(action.clone());
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("fib 20", |b| b.iter(run_game));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);