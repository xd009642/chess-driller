//! Store the opening preparation we want to work over - might rename it in future but it is kind
//! of a mini stripped-down move database.
use petgraph::graph::{Graph, NodeIndex};
use petgraph::Direction;
use pgn_reader::{BufferedReader, SanPlus, Skip, Visitor};
use std::path::Path;
use std::{fs, io};
use walkdir::WalkDir;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MoveAssessment {
    /// You're still in prep
    InPrep,
    /// You've gone wrong somewhere
    OutOfPrep,
    /// Mission completed!
    PrepEnded,
}

pub type OpeningGraph = Graph<SanPlus, ()>;

#[derive(Default)]
pub struct OpeningDatabase {
    white_openings: OpeningGraph,
    black_openings: OpeningGraph,
}

pub struct GameState<'a> {
    openings: &'a OpeningGraph,
    pub current_move: Option<NodeIndex>,
    player_turn: bool,
    still_running: bool,
}

impl OpeningDatabase {
    pub fn load_default() -> anyhow::Result<Self> {
        Self::load(Path::new("prep"))
    }

    pub fn load(root: &Path) -> anyhow::Result<Self> {
        let white_openings = load_folder(&root.join("white"))?;
        let black_openings = load_folder(&root.join("black"))?;

        Ok(Self {
            white_openings,
            black_openings,
        })
    }

    pub fn start_drill(&self, player: chess::Color, moves: &[SanPlus]) -> Option<GameState> {
        let openings = match player {
            chess::Color::White => &self.white_openings,
            _ => &self.black_openings,
        };
        let mut state = GameState {
            openings,
            player_turn: player == chess::Color::White,
            current_move: None,
            still_running: true,
        };

        for m in moves {
            let prep = state.apply_move(m);
            if prep != MoveAssessment::InPrep {
                return None;
            }
        }
        Some(state)
    }

    pub fn load_multigame_pgn(pgns: impl io::Read, player: String) -> anyhow::Result<Self> {
        let mut this = Self::default();
        this.add_multigame_pgn(pgns, player)?;
        Ok(this)
    }

    pub fn add_multigame_pgn(&mut self, pgns: impl io::Read, player: String) -> anyhow::Result<()> {
        let white = self.white_openings.clone();
        let black = self.black_openings.clone();
        let mut reader = BufferedReader::new(pgns);

        let mut visitor = PgnVisitor::new_game_recorder(white, black, player);
        while reader.has_more()? {
            reader.read_game(&mut visitor)?;
        }
        self.white_openings = visitor.pgn;
        self.black_openings = visitor.backup_pgn.unwrap();
        Ok(())
    }
}

impl<'a> GameState<'a> {
    pub fn still_running(&self) -> bool {
        self.still_running
    }

    pub fn check_move(&self) -> MoveAssessment {
        if let Some(current) = self.current_move {
            if self
                .openings
                .neighbors_directed(current, Direction::Outgoing)
                .count()
                > 0
            {
                MoveAssessment::InPrep
            } else {
                MoveAssessment::PrepEnded
            }
        } else {
            MoveAssessment::OutOfPrep
        }
    }

    pub fn make_move(&mut self) -> Option<SanPlus> {
        let candidates = if let Some(index) = self.current_move {
            self.openings
                .neighbors_directed(index, Direction::Outgoing)
                .collect::<Vec<NodeIndex>>()
        } else {
            self.find_roots()
        };
        let choice = fastrand::choice(candidates.iter())?;
        self.current_move = Some(*choice);
        self.player_turn = !self.player_turn;
        Some(self.openings[*choice].clone())
    }

    pub fn apply_move(&mut self, san: &SanPlus) -> MoveAssessment {
        let mut has_neighbors = false;
        let mut possible_moves = vec![];
        if let Some(index) = self.current_move {
            for next in self.openings.neighbors_directed(index, Direction::Outgoing) {
                has_neighbors = true;
                possible_moves.push(&self.openings[next]);
                if &self.openings[next] == san {
                    self.current_move = Some(next);
                    self.player_turn = !self.player_turn;
                    return MoveAssessment::InPrep;
                }
            }
        } else {
            for root in self.find_roots().iter() {
                has_neighbors = true;
                possible_moves.push(&self.openings[*root]);
                if &self.openings[*root] == san {
                    self.current_move = Some(*root);
                    self.player_turn = !self.player_turn;
                    return MoveAssessment::InPrep;
                }
            }
        }
        if has_neighbors {
            let possible_moves = possible_moves
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            println!(
                "You chose: {}. Instead you should have chose one of: {}",
                san, possible_moves
            );
            self.still_running = false;
            MoveAssessment::OutOfPrep
        } else {
            self.still_running = false;
            MoveAssessment::PrepEnded
        }
    }

    pub fn is_player_turn(&self) -> bool {
        self.player_turn
    }

    fn find_roots(&self) -> Vec<NodeIndex> {
        let mut res = vec![];
        for node in self.openings.node_indices() {
            if self
                .openings
                .neighbors_directed(node, Direction::Incoming)
                .count()
                == 0
            {
                res.push(node);
            }
        }
        res
    }
}

fn load_folder(folder: &Path) -> anyhow::Result<OpeningGraph> {
    let mut graph = OpeningGraph::default();
    for entry in WalkDir::new(folder)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        println!("Loading: {}", entry.path().display());
        let load = fs::File::open(entry.path());
        let load = match load {
            Ok(s) => s,
            Err(e) => {
                println!("Failed to load {}. Error: {}", entry.path().display(), e);
                continue;
            }
        };
        let mut reader = BufferedReader::new(load);
        let mut pgn_visitor = PgnVisitor::new_with_graph(graph);
        reader.read_game(&mut pgn_visitor)?;

        graph = pgn_visitor.pgn;
    }

    // debugging we can print the graphs and see they're right!
    //let pretty_graph = graph.map(|_, node| node.to_string(), |_, _| ());
    //let dot = Dot::new(&pretty_graph);
    //println!("{:?}", dot);
    Ok(graph)
}

#[derive(Default, Debug)]
struct PgnVisitor {
    /// If backup is present this is white!
    pgn: OpeningGraph,
    /// Stores black
    backup_pgn: Option<OpeningGraph>,
    node_stack: Vec<NodeIndex>,
    first: bool,
    /// Used to show if we want to filter on player
    player: Option<String>,
    store_in_backup: bool,
}

impl PgnVisitor {
    pub fn new_with_graph(pgn: OpeningGraph) -> Self {
        Self {
            pgn,
            backup_pgn: None,
            node_stack: vec![],
            first: true,
            player: None,
            store_in_backup: false,
        }
    }

    pub fn new_game_recorder(white: OpeningGraph, black: OpeningGraph, player: String) -> Self {
        Self {
            pgn: white,
            backup_pgn: Some(black),
            player: Some(player),
            first: true,
            node_stack: vec![],
            store_in_backup: false,
        }
    }
}

impl Visitor for PgnVisitor {
    type Result = ();

    fn header(&mut self, key: &[u8], value: pgn_reader::RawHeader) {
        if let Some(player) = self.player.as_ref() {
            let color_key = match std::str::from_utf8(key) {
                Ok("White") => chess::Color::White,
                Ok("Black") => chess::Color::Black,
                _ => return,
            };

            if let Ok(pgn_player) = std::str::from_utf8(value.0) {
                if pgn_player == player {
                    println!("Recording: {:?} game for player", color_key);
                    self.store_in_backup = color_key == chess::Color::Black;
                }
            }
        }
    }

    fn end_game(&mut self) -> Self::Result {
        self.node_stack.clear();
        self.first = true;
        ()
    }

    fn san(&mut self, san_plus: SanPlus) {
        let pgn = if self.store_in_backup {
            match self.backup_pgn.as_mut() {
                Some(s) => s,
                None => {
                    eprintln!("Trying to filter on player but no black graph!?");
                    return;
                }
            }
        } else {
            &mut self.pgn
        };
        if self.first {
            assert!(self.node_stack.is_empty());
            self.first = false;
            for node in pgn.node_indices() {
                if pgn.neighbors_directed(node, Direction::Incoming).count() == 0 {
                    if pgn[node] == san_plus {
                        self.node_stack.push(node);
                    }
                }
            }
            if self.node_stack.is_empty() {
                let node = pgn.add_node(san_plus);
                self.node_stack.push(node);
            }
        } else {
            if let Some(old_node) = self.node_stack.last_mut() {
                for neighbor in pgn.neighbors_directed(*old_node, Direction::Outgoing) {
                    if pgn[neighbor] == san_plus {
                        self.node_stack.push(neighbor);
                        return;
                    }
                }
                let node = pgn.add_node(san_plus);
                pgn.add_edge(*old_node, node, ());
                *old_node = node;
            } else {
                let node = pgn.add_node(san_plus);
                self.node_stack.push(node);
            }
        }
    }

    fn begin_variation(&mut self) -> Skip {
        let pgn = if self.store_in_backup {
            match self.backup_pgn.as_mut() {
                Some(s) => s,
                None => {
                    eprintln!("Trying to filter on player but no black graph!?");
                    return Skip(true);
                }
            }
        } else {
            &mut self.pgn
        };
        // Variation is an alternative for last move pushed so we want to join to two moves back
        // This won't work well with variations right at the start (should probably be
        // `Vec<Option<NodeStack>>` to create new tree roots...
        if let Some(last) = self.node_stack.last() {
            let parents = pgn
                .neighbors_directed(*last, Direction::Incoming)
                .collect::<Vec<NodeIndex>>();
            if parents.len() > 1 {
                println!("Unexpected extra parents!?");
            }
            if !parents.is_empty() {
                self.node_stack.push(parents[0]);
            }
        } else {
            println!("No root?");
        }
        Skip(false)
    }

    fn end_variation(&mut self) {
        // We now want to add to last node before variation
        self.node_stack.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_multiline_pgn() {
        let load = fs::File::open("tests/resources/games.pgn");
        let load = match load {
            Ok(s) => s,
            Err(e) => {
                panic!("Failed to load Error: {}", e);
            }
        };

        let db = OpeningDatabase::load_multigame_pgn(load, "xd009642".to_string()).unwrap();

        // Lets make sure for white we have a QGD and for black a caro kann

        let caro_kann = &[
            SanPlus::from_ascii(b"e4").unwrap(),
            SanPlus::from_ascii(b"c6").unwrap(),
        ];
        let _state = db.start_drill(chess::Color::Black, caro_kann).unwrap();

        let qgd = &[
            SanPlus::from_ascii(b"d4").unwrap(),
            SanPlus::from_ascii(b"d5").unwrap(),
            SanPlus::from_ascii(b"c4").unwrap(),
            SanPlus::from_ascii(b"e6").unwrap(),
        ];
        let _state = db.start_drill(chess::Color::White, qgd).unwrap();
    }
}
