use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::io;
use std::io::Write;
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

#[derive(Debug, Clone, Copy)]
enum CpuStrength{
    Garbage,
    Childlike,
    RegularShmegular,
    Decent,
    Hardge,
    Spooky,
    Immortal
}

impl CpuStrength {
   fn strength(&self) -> (u32, u32) {
       match self {
           CpuStrength::Garbage => (1, 1),
           CpuStrength::Childlike => (4, 10),
           CpuStrength::RegularShmegular => (8, 100),
           CpuStrength::Decent => (8, 400),
           CpuStrength::Hardge => (12, 1000),
           CpuStrength::Spooky => (24, 4000),
           CpuStrength::Immortal => (32, 10000),
       }
   }

    fn all() -> Vec<Self> {
        use CpuStrength::*;
        vec![
            Garbage,
            Childlike,
            RegularShmegular,
            Decent,
            Hardge,
            Spooky,
            Immortal
        ]
    }
}

struct SetupData {
    seed: u64,
    cpus: Vec<CpuStrength>,
}

fn init() -> SetupData {
    println!("Initial Setup");

    let mut line = String::new();

    print!("Seed (number): ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut line).unwrap();
    let seed = line.trim().parse::<u64>().unwrap();
    line.clear();

    print!("Number of Players (2-6): ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut line).unwrap();
    let num_players = line.trim().parse::<usize>().unwrap();
    line.clear();

    if num_players < 2 || num_players > 6 {
        panic!("Invalid number of players");
    }

    let mut cpus: Vec<CpuStrength> = Vec::with_capacity(num_players-1);

    for i in 1..num_players {
        println!("Choose player {} strength: ", i + 1);

        for (idx, s) in CpuStrength::all().iter().enumerate() {
            println!("\t{idx}: {:?}", s);
        }

        print!("Select one (0-{}): ", CpuStrength::all().len()-1);
        io::stdout().flush().unwrap();
        io::stdin().read_line(&mut line).unwrap();
        let strength_idx = line.trim().parse::<usize>().unwrap();
        line.clear();

        let strength = CpuStrength::all()[strength_idx];
        cpus.push(strength);
    }

    SetupData {
        seed,
        cpus,
    }


}

fn main() {

    let setup_data = init();

    let mut options = Options::default();
    options.num_players = (setup_data.cpus.len() + 1) as u8;

    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(setup_data.seed);
    let initial_game_state = Acquire::new(&mut rng, &options);
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
                            num_simulations: setup_data.cpus[idx-1].strength().0,
                            num_determinations: setup_data.cpus[idx-1].strength().1,
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
        match action {
            Action::PlaceTile(_, _) => {
                println!("{}", game.state);
            }
            Action::PurchaseStock(_, _) => {}
            Action::SelectChainToCreate(_, _) => {}
            Action::SelectChainForTiebreak(_, _) => {}
            Action::DecideMerge { .. } => {}
            Action::Terminate(_, _) => {}
        }

    }
}
