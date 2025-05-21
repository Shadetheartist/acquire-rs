use std::collections::{HashMap};
use std::fmt::{ Debug, Display, Formatter};
use std::io;
use std::io::Write;
use std::str::FromStr;
use bg_ai::ismcts::{IsMctsMtAgent, MtAgent, MultithreadedInformationSetGame};
use rand_chacha::rand_core::SeedableRng;
use acquire::{Acquire, Action, BuyOption, Chain, Options, Phase, PlayerId, Tile};
use itertools::Itertools;
use rand::{thread_rng, RngCore};
use rand_chacha::ChaCha8Rng;

#[derive(Debug)]
struct HumanAgent {
    player_id: PlayerId,
}

impl IsMctsMtAgent<rand_chacha::ChaCha8Rng, Acquire, Action, PlayerId> for HumanAgent
where
{
    fn player(&self) -> PlayerId {
        self.player_id
    }

    fn decide(&self, _: &mut rand_chacha::ChaCha8Rng, state: &Acquire) -> Option<Action> {
        let actions = state.actions();
        let mut state = state.clone();

        for tile in state.get_player_by_id(self.player_id).clone().tiles {
            state.grid_mut().indicators.insert(tile.into());
        }


        println!("\n{}", state);


        loop {
            match state.phase() {
                Phase::AwaitingTilePlacement => {
                    if let Some(value) = Self::tile_placement_input(&actions) {
                        return value;
                    }
                }
                Phase::AwaitingStockPurchase => {
                    if let Some(value) = Self::purchase_stock_input(&state, &actions) {
                        return value;
                    }
                }
                Phase::AwaitingChainCreationSelection => {
                    if let Some(value) = Self::select_chain_to_create(&state, &actions) {
                        return value;
                    }
                }
                Phase::AwaitingGameTerminationDecision |
                Phase::Merge { .. } => {
                    println!("Choose Action to Take");
                    for (idx, action) in actions.iter().enumerate() {
                        let mut action_str = action.to_string();
                        action_str = action_str.replace("Player 0", "Human Player");
                        println!("  {} - {}", idx + 1, action_str);
                    }

                    let mut line = String::new();
                    io::stdin().read_line(&mut line).unwrap();

                    if let Ok(decision) = usize::from_str(line.trim()) {
                        return Some(actions.get(decision - 1).unwrap().clone());
                    }
                }
            }
        }
    }
}

impl HumanAgent {
    fn tile_placement_input(actions: &Vec<Action>) -> Option<Option<Action>> {

        println!("Choose Tile to Place");
        print!("{}: ", actions.iter().map(|a| {
            if let Action::PlaceTile(_, tile) = a {
                tile
            } else {
                panic!()
            }
        }).join(", "));

        io::stdout().flush().unwrap();

        let mut line = String::new();
        io::stdin().read_line(&mut line).unwrap();
        line = line.trim().to_uppercase();

        if let Ok(tile) = Tile::try_from(line.as_str()) {
            let action = actions.iter().find(|a| {
                if let Action::PlaceTile(_, t) = a {
                    *t == tile
                } else {
                    false
                }
            });

            if let Some(action) = action {
                return Some(Some(action.clone()));
            }
        }

        println!("Invalid Tile.");
        None
    }

    fn multiset<T: Eq + std::hash::Hash>(vec: &[T]) -> HashMap<&T, usize> {
        let mut counts = HashMap::new();
        for item in vec {
            *counts.entry(item).or_insert(0) += 1;
        }
        counts
    }

    fn purchase_stock_input(state: &Acquire, actions: &Vec<Action>) -> Option<Option<Action>> {
        print!("Choose up to Three Stock to Buy of [{}] (initials): ", state.grid().available_chains().into_iter().join(", "));
        io::stdout().flush().unwrap();

        let mut line = String::new();
        io::stdin().read_line(&mut line).unwrap();
        line = line.trim().to_uppercase();

        let chains: Option<Vec<_>> = line
            .chars()
            .into_iter()
            .filter(|c| *c != ' ')
            .map(|c| Chain::from_initial(&c.to_string()))
            .collect();

        let Some(chains) = chains else {
            println!("Invalid input.");
            return None;
        };

        if chains.len() > 3 {
            println!("Invalid Input, Too Many Choices.");
            return None;
        }

        let mut buys: Vec<_> = chains.into_iter().map(|c| BuyOption::Chain(c)).collect();
        for _ in 0..3-buys.len() {
            buys.push(BuyOption::None);
        }

        let buys_set = Self::multiset(&buys);

        let has_match = {
            let mut m = false;
            for a in actions {
                if let Action::PurchaseStock(_, buys) = a {
                    if buys_set == Self::multiset(buys) {
                        m = true;
                        break;
                    }
                }
            }
            m
        };

        if !has_match {
            println!("Invalid Input, Can you afford these? Are there enough in stock?");
            return None;
        }


        let buys: [BuyOption; 3] = [buys.pop().unwrap(), buys.pop().unwrap(), buys.pop().unwrap()];
        Some(Some(Action::PurchaseStock(PlayerId(0), buys)))
    }

    fn select_chain_to_create(state: &Acquire, actions: &Vec<Action>) -> Option<Option<Action>> {
        print!("Choose Chain to Create of [{}] (initial): ", state.grid().available_chains().into_iter().join(", "));
        io::stdout().flush().unwrap();

        let mut line = String::new();
        io::stdin().read_line(&mut line).unwrap();
        line = line.trim().to_uppercase();

        let chains: Option<Vec<_>> = line
            .chars()
            .into_iter()
            .filter(|c| *c != ' ')
            .map(|c| Chain::from_initial(&c.to_string()))
            .collect();

        let Some(chains) = chains else {
            println!("Invalid input.");
            return None;
        };

        if chains.len() != 1 {
            println!("Invalid Input, choose one chain by entering it's initial.");
            return None;
        }

        let selected_chain = chains[0];

        let action = actions.iter().find(|a| {
            if let Action::SelectChainToCreate(_, chain) = a {
                chain == &selected_chain
            } else {
                false
            }
        });

        if action.is_none() {
            println!("Invalid Input, matching action not found");
            return None;
        }

        Some(Some(action.unwrap().clone()))
    }
}

impl Display for HumanAgent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}", self.player_id))
    }
}

#[derive(Debug, Clone, Copy)]
enum CpuStrength {
    Garbage,
    Childlike,
    RegularShmegular,
    Decent,
    Hardge,
    Spooky,
    Immortal,
    Bezos
}

impl CpuStrength {
    fn strength(&self) -> (u32, u32) {
        match self {
            CpuStrength::Garbage => (1, 1),
            CpuStrength::Childlike => (4, 10),
            CpuStrength::RegularShmegular => (8, 100),
            CpuStrength::Decent => (8, 400),
            CpuStrength::Hardge => (10, 700),
            CpuStrength::Spooky => (12, 2000),
            CpuStrength::Immortal => (24, 4000),
            CpuStrength::Bezos => (48, 8000),
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
            Immortal,
            Bezos,
        ]
    }
}

enum Mode {
    Human,
    CpuExpo
}

struct SetupData {
    seed: u64,
    cpus: Vec<CpuStrength>,
    mode: Mode,
}

fn init() -> SetupData {
    println!(r#########"
      .o.                                         o8o
     .888.                                        `"'
    .8"888.      .ooooo.   .ooooo oo oooo  oooo  oooo  oooo d8b  .ooooo.
   .8' `888.    d88' `"Y8 d88' `888  `888  `888  `888  `888""8P d88' `88b
  .88ooo8888.   888       888   888   888   888   888   888     888ooo888
 .8'     `888.  888   .o8 888   888   888   888   888   888     888    .o
o88o     o8888o `Y8bod8P' `V8bod888   `V88V"V8P' o888o d888b    `Y8bod8P'
                                888.
                                8P'              CLI by Derek H.
                                "
    "#########);

    println!("\nHelp:\nWhen the game asks you for initials when buying stock, you can type up to three. For example:\n\tcci = buy two continental and one imperial.\n\tttt = buy three tower\n\n");


    println!("Game Setup");

    let mut line = String::new();

    print!("Default or Custom? ([D] or C): ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut line).unwrap();
    let custom = line.trim().to_lowercase();
    if custom != "c" {
        return SetupData { mode: Mode::Human, seed: thread_rng().next_u64(), cpus: vec![CpuStrength::Hardge, CpuStrength::Decent, CpuStrength::RegularShmegular] };
    }
    line.clear();

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


    let mut mode = Mode::CpuExpo;
    print!("Will you be playing? ([y] or n): ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut line).unwrap();
    let mode_str = line.trim();
    if mode_str == "y" {
        mode = Mode::Human;
    }
    line.clear();

    let mut cpus: Vec<CpuStrength> = Vec::with_capacity(num_players - 1);

    let start = match mode {
        Mode::Human => 1,
        Mode::CpuExpo => 0
    };

    for i in start..num_players {
        println!("Choose cpu {} strength: ", i);

        for (idx, s) in CpuStrength::all().iter().enumerate() {
            println!("\t{idx}: {:?}", s);
        }

        print!("Select one (0-{}): ", CpuStrength::all().len() - start);
        io::stdout().flush().unwrap();
        io::stdin().read_line(&mut line).unwrap();
        let strength_idx = line.trim().parse::<usize>().unwrap();
        line.clear();

        let strength = CpuStrength::all()[strength_idx];
        cpus.push(strength);
    }

    SetupData {
        mode,
        seed,
        cpus,
    }
}

fn main() {
    let setup_data = init();

    let mut options = Options::default();

    options.num_players = match setup_data.mode {
        Mode::Human => (setup_data.cpus.len() + 1) as u8,
        Mode::CpuExpo => (setup_data.cpus.len()) as u8
    };

    println!("Starting Game");

    let mut rng = ChaCha8Rng::seed_from_u64(setup_data.seed);
    let initial_game_state = Acquire::new(&mut rng, &options);

    let game = match setup_data.mode {
        Mode::Human => human_play(setup_data, rng, initial_game_state),
        Mode::CpuExpo => cpu_expo(setup_data, rng, initial_game_state)
    };
    println!("{}", game.state);
    println!("{:?}", game.outcome());
    println!("Game Over!");
}

fn human_play(setup_data: SetupData, rng: ChaCha8Rng, initial_game_state: Acquire) -> MultithreadedInformationSetGame<ChaCha8Rng, Acquire, Action, PlayerId> {
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
                            num_simulations: setup_data.cpus[idx - 1].strength().0,
                            num_determinations: setup_data.cpus[idx - 1].strength().1,
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
    }

    game
}


fn cpu_expo(setup_data: SetupData, rng: ChaCha8Rng, initial_game_state: Acquire) -> MultithreadedInformationSetGame<ChaCha8Rng, Acquire, Action, PlayerId> {
    let agents: HashMap<PlayerId, Box<dyn IsMctsMtAgent<rand_chacha::ChaCha8Rng, Acquire, Action, PlayerId>>> = initial_game_state
        .players()
        .iter()
        .enumerate()
        .map(|(idx, player)| {
            let agent = {

                (
                    || Box::new(MtAgent {
                        player: player.id,
                        num_simulations: setup_data.cpus[idx].strength().0,
                        num_determinations: setup_data.cpus[idx].strength().1,
                    }) as _
                )()
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

        println!("{}", game.state);
        println!("{}", action);
    }
    game
}
