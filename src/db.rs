//! Store the opening preparation we want to work over - might rename it in future but it is kind
//! of a mini stripped-down move database.
use petgraph::dot::Dot;
use petgraph::graph::{Graph, NodeIndex};
use petgraph::Direction;
use pgn_reader::{BufferedReader, SanPlus, Skip, Visitor};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub type OpeningGraph = Graph<SanPlus, ()>;

#[derive(Default)]
pub struct OpeningDatabase {
    white_openings: HashMap<PathBuf, OpeningGraph>,
    black_openings: HashMap<PathBuf, OpeningGraph>,
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
}

fn load_folder(folder: &Path) -> anyhow::Result<HashMap<PathBuf, OpeningGraph>> {
    let mut res = HashMap::new();
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
        let mut pgn_visitor = PgnVisitor::default();
        reader.read_game(&mut pgn_visitor)?;

        // debugging we can print the graphs and see they're right!
        // let pretty_graph = pgn_visitor.pgn.map(|_, node| node.to_string(), |_, _| ());
        // let dot = Dot::new(&pretty_graph);
        // println!("{:?}", dot);

        // We should attempt to simplify to the minimum number of graphs (I think each graph should
        // have a root of the same first move and then branch from there. Though in future if we
        // want to support transpositions then making it into one giant graph is probably the
        // smartest... hmmm graph merge algorithms.
        //
        // Until then returning each file in a map of PGN file -> Graph
        res.insert(entry.path().to_path_buf(), pgn_visitor.pgn);
    }
    Ok(res)
}

#[derive(Default, Debug)]
struct PgnVisitor {
    pgn: OpeningGraph,
    node_stack: Vec<NodeIndex>,
}

impl Visitor for PgnVisitor {
    type Result = ();

    fn end_game(&mut self) -> Self::Result {
        ()
    }

    fn san(&mut self, san_plus: SanPlus) {
        let node = self.pgn.add_node(san_plus);
        if let Some(old_node) = self.node_stack.last_mut() {
            self.pgn.add_edge(*old_node, node, ());
            *old_node = node;
        } else {
            self.node_stack.push(node);
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
        println!("Node stack: {:?}", self.node_stack);
        Skip(false)
    }

    fn end_variation(&mut self) {
        // We now want to add to last node before variation
        self.node_stack.pop();
    }
}
