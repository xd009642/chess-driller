//! Store the opening preparation we want to work over - might rename it in future but it is kind
//! of a mini stripped-down move database.
use petgraph::graph::{Graph, NodeIndex};
use petgraph::Direction;
use pgn_reader::{BufferedReader, SanPlus, Skip, Visitor};
use std::fs;
use std::path::Path;
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
        };

        for m in moves {
            let prep = state.apply_move(m);
            if prep != MoveAssessment::InPrep {
                return None;
            }
        }
        Some(state)
    }
}

impl<'a> GameState<'a> {
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
        if let Some(index) = self.current_move {
            for next in self.openings.neighbors_directed(index, Direction::Outgoing) {
                has_neighbors = true;
                if &self.openings[next] == san {
                    self.current_move = Some(next);
                    self.player_turn = !self.player_turn;
                    return MoveAssessment::InPrep;
                }
            }
        } else {
            for root in self.find_roots().iter() {
                has_neighbors = true;
                if &self.openings[*root] == san {
                    self.current_move = Some(*root);
                    self.player_turn = !self.player_turn;
                    return MoveAssessment::InPrep;
                }
            }
        }
        if has_neighbors {
            MoveAssessment::OutOfPrep
        } else {
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
    pgn: OpeningGraph,
    node_stack: Vec<NodeIndex>,
    first: bool,
}

impl PgnVisitor {
    pub fn new_with_graph(pgn: OpeningGraph) -> Self {
        Self {
            pgn,
            node_stack: vec![],
            first: true,
        }
    }
}

impl Visitor for PgnVisitor {
    type Result = ();

    fn end_game(&mut self) -> Self::Result {
        ()
    }

    fn san(&mut self, san_plus: SanPlus) {
        if self.first {
            assert!(self.node_stack.is_empty());
            self.first = false;
            for node in self.pgn.node_indices() {
                if self
                    .pgn
                    .neighbors_directed(node, Direction::Incoming)
                    .count()
                    == 0
                {
                    if self.pgn[node] == san_plus {
                        self.node_stack.push(node);
                    }
                }
            }
            if self.node_stack.is_empty() {
                let node = self.pgn.add_node(san_plus);
                self.node_stack.push(node);
            }
        } else {
            if let Some(old_node) = self.node_stack.last_mut() {
                for neighbor in self.pgn.neighbors_directed(*old_node, Direction::Outgoing) {
                    if self.pgn[neighbor] == san_plus {
                        self.node_stack.push(neighbor);
                        return;
                    }
                }
                let node = self.pgn.add_node(san_plus);
                self.pgn.add_edge(*old_node, node, ());
                *old_node = node;
            } else {
                let node = self.pgn.add_node(san_plus);
                self.node_stack.push(node);
            }
        }
    }

    fn begin_variation(&mut self) -> Skip {
        // Variation is an alternative for last move pushed so we want to join to two moves back
        // This won't work well with variations right at the start (should probably be
        // `Vec<Option<NodeStack>>` to create new tree roots...
        if let Some(last) = self.node_stack.last() {
            let parents = self
                .pgn
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
