//! Store the opening preparation we want to work over - might rename it in future but it is kind
//! of a mini stripped-down move database.
use petgraph::dot::Dot;
use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::Bfs;
use petgraph::Direction;
use pgn_reader::{BufferedReader, SanPlus, Skip, Visitor};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub type OpeningGraph = Graph<SanPlus, ()>;

#[derive(Default)]
pub struct OpeningDatabase {
    white_openings: OpeningGraph,
    black_openings: OpeningGraph,
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
    let pretty_graph = graph.map(|_, node| node.to_string(), |_, _| ());
    let dot = Dot::new(&pretty_graph);
    println!("{:?}", dot);
    Ok(graph)
}

fn merge_graphs(graphs: HashMap<PathBuf, OpeningGraph>) -> OpeningGraph {
    let mut master_graph = OpeningGraph::default();
    let mut master_roots = HashMap::new();
    println!("Merging graphs");
    for (k, v) in &graphs {
        println!("Merging {} into master graph", k.display());
        if master_graph.node_count() == 0 {
            master_graph = v.clone();
            for node in v.node_indices() {
                if v.neighbors_directed(node, Direction::Incoming).count() == 0 {
                    master_roots.insert(master_graph[node].clone(), node);
                }
            }
        } else {
            // Keep track of non-root nodes we've seen when merging so we don't check them for
            // neighbors
            let mut visited = HashSet::new();
            // a PGN might contain variations from move 1 meaning a lot of root nodes
            let mut roots_to_merge_in = vec![];
            for node in v.node_indices() {
                if visited.contains(&node) {
                    continue;
                }
                if v.neighbors_directed(node, Direction::Incoming).count() == 0 {
                    // Now we have a root node we want to traverse along it merging into the graph.
                    // There's two ways to do this: just add to graph and then prune duplicate
                    // children, or try to traverse both graphs finding merge points. I'm going for
                    // the latter.
                    roots_to_merge_in.push(node);
                    if let Some(root) = master_roots.get(&v[node]) {
                        // Okay time to merge. follow the graph up until we hit our prep limit and
                        // then start inserting.
                        let mut bfs = Bfs::new(v, node);
                        // lets skip the root
                        bfs.next(v);
                        while let Some(next_node) = bfs.next(v) {
                            visited.insert(next_node);


                        }
                    } else {
                        // No shared root in master graph, we can just add all of this in!
                        let mut node_map = HashMap::new();
                        let mut bfs = Bfs::new(v, node);
                        while let Some(n) = bfs.next(v) {
                            visited.insert(n);
                            let new_node = master_graph.add_node(v[n].clone());
                            node_map.insert(n, new_node);
                        }
                        for edge in v.raw_edges() {
                            if node_map.contains_key(&edge.source())
                                && node_map.contains_key(&edge.target())
                            {
                                master_graph.add_edge(
                                    node_map[&edge.source()],
                                    node_map[&edge.target()],
                                    (),
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    master_graph
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
            self.first = false;
            for node in self.pgn.node_indices() {
                if self.pgn.neighbors_directed(node, Direction::Incoming).count() == 0 {
                    if self.pgn[node] == san_plus {
                        self.node_stack.push(node);
                    }
                }
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
        println!("Node stack: {:?}", self.node_stack);
        Skip(false)
    }

    fn end_variation(&mut self) {
        // We now want to add to last node before variation
        self.node_stack.pop();
    }
}
