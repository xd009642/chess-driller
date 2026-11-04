//! A prefix tree of legal moves. Sources belong to nodes so shared prefixes retain
//! every contributing chapter without allowing an unselected file to supply replies.
use anyhow::{anyhow, bail, Context};
use pgn_reader::{RawTag, Reader, SanPlus, Skip, Visitor};
use serde::{Deserialize, Serialize};
use shakmaty::{uci::UciMove, CastlingMode, Chess, Position};
use std::ops::ControlFlow;
use std::{fs, path::PathBuf};

#[derive(Clone, Serialize, Deserialize)]
pub struct PrepFile {
    pub id: u64,
    pub name: String,
    pub color: String,
    pub pgn: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Source {
    pub file_id: u64,
    pub chapter_number: usize,
    pub file: String,
    pub chapter: String,
}

#[derive(Default, Clone)]
pub struct Node {
    pub children: Vec<(String, usize)>,
    pub sources: Vec<Source>,
}

#[derive(Clone)]
pub struct Repertoire {
    pub nodes: Vec<Node>,
}

impl Repertoire {
    pub fn from_files(files: &[PrepFile]) -> anyhow::Result<Self> {
        let mut tree = Self {
            nodes: vec![Node::default()],
        };
        for file in files {
            let mut visitor = Importer {
                tree: &mut tree,
                file,
                position: Chess::default(),
                node: 0,
                before: (Chess::default(), 0),
                variations: Vec::new(),
                chapter: String::new(),
                game_number: 0,
                error: None,
            };
            Reader::new(file.pgn.as_bytes())
                .read_games(&mut visitor)
                .try_for_each(|result| result)
                .with_context(|| format!("Cannot read {}", file.name))?;
            if let Some(error) = visitor.error {
                bail!("{}: {error}", file.name);
            }
        }
        if tree.nodes.len() == 1 {
            bail!("The selected PGN files contain no moves");
        }
        Ok(tree)
    }
}

struct Importer<'a> {
    tree: &'a mut Repertoire,
    file: &'a PrepFile,
    position: Chess,
    node: usize,
    before: (Chess, usize),
    variations: Vec<((Chess, usize), (Chess, usize))>,
    chapter: String,
    game_number: usize,
    error: Option<String>,
}

impl Visitor for Importer<'_> {
    type Tags = ();
    type Movetext = ();
    type Output = ();

    fn begin_tags(&mut self) -> ControlFlow<()> {
        self.game_number += 1;
        self.position = Chess::default();
        self.node = 0;
        self.before = (self.position.clone(), 0);
        self.variations.clear();
        self.chapter = format!("Chapter {}", self.game_number);
        ControlFlow::Continue(())
    }

    fn tag(&mut self, _: &mut (), key: &[u8], value: RawTag<'_>) -> ControlFlow<()> {
        let value = value.decode_utf8_lossy().to_string();
        match key {
            b"Event" if !value.is_empty() && value != "?" => self.chapter = value,
            b"FEN" => {
                self.error =
                    Some("PGNs with custom starting positions are not supported yet".into())
            }
            b"Variant" if !matches!(value.as_str(), "Standard" | "Chess" | "From Position") => {
                self.error = Some(format!("Unsupported chess variant: {value}"));
            }
            _ => {}
        }
        ControlFlow::Continue(())
    }

    fn san(&mut self, _: &mut (), san: SanPlus) -> ControlFlow<()> {
        if self.error.is_some() {
            return ControlFlow::Continue(());
        }
        let mv = match san.san.to_move(&self.position) {
            Ok(mv) => mv,
            Err(error) => {
                self.error = Some(format!("{}: illegal move {san}: {error}", self.chapter));
                return ControlFlow::Continue(());
            }
        };
        self.before = (self.position.clone(), self.node);
        let uci = UciMove::from_move(mv, CastlingMode::Standard).to_string();
        let next = self.tree.nodes[self.node]
            .children
            .iter()
            .find(|(m, _)| *m == uci)
            .map(|(_, n)| *n);
        let next = next.unwrap_or_else(|| {
            let next = self.tree.nodes.len();
            self.tree.nodes.push(Node::default());
            self.tree.nodes[self.node].children.push((uci, next));
            next
        });
        let source = Source {
            file_id: self.file.id,
            chapter_number: self.game_number,
            file: self.file.name.clone(),
            chapter: self.chapter.clone(),
        };
        for index in [self.node, next] {
            if !self.tree.nodes[index].sources.contains(&source) {
                self.tree.nodes[index].sources.push(source.clone());
            }
        }
        self.node = next;
        self.position.play_unchecked(mv);
        ControlFlow::Continue(())
    }

    fn begin_variation(&mut self, _: &mut ()) -> ControlFlow<(), Skip> {
        self.variations
            .push(((self.position.clone(), self.node), self.before.clone()));
        (self.position, self.node) = self.before.clone();
        ControlFlow::Continue(Skip(false))
    }

    fn end_variation(&mut self, _: &mut ()) -> ControlFlow<()> {
        if let Some((current, before)) = self.variations.pop() {
            (self.position, self.node) = current;
            self.before = before;
        } else {
            self.error = Some("Unbalanced PGN variation".into());
        }
        ControlFlow::Continue(())
    }

    fn end_game(&mut self, _: ()) {
        if !self.variations.is_empty() {
            self.error = Some("Unclosed PGN variation".into());
        }
    }
    fn begin_movetext(&mut self, _: ()) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }
}

pub struct Library {
    pub files: Vec<PrepFile>,
    path: PathBuf,
}

impl Library {
    pub fn load(directory: PathBuf) -> anyhow::Result<Self> {
        fs::create_dir_all(&directory)?;
        let path = directory.join("prep.json");
        let files = if path.exists() {
            serde_json::from_slice(&fs::read(&path)?)?
        } else {
            Vec::new()
        };
        Ok(Self { files, path })
    }

    pub fn import(&mut self, name: String, color: String, pgn: String) -> anyhow::Result<()> {
        if !matches!(color.as_str(), "white" | "black") {
            bail!("Choose white or black");
        }
        if name.trim().is_empty() {
            bail!("The PGN needs a filename");
        }
        let id = self
            .files
            .iter()
            .map(|f| f.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| anyhow!("Too many prep files"))?;
        let file = PrepFile {
            id,
            name,
            color,
            pgn,
        };
        Repertoire::from_files(std::slice::from_ref(&file))?;
        let mut files = self.files.clone();
        files.push(file);
        let temporary = self.path.with_extension("tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(&files)?)?;
        fs::rename(&temporary, &self.path)?;
        self.files = files;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn file(id: u64, pgn: &str) -> PrepFile {
        PrepFile {
            id,
            name: format!("study-{id}.pgn"),
            color: "white".into(),
            pgn: pgn.into(),
        }
    }

    fn follow(tree: &Repertoire, moves: &[&str]) -> usize {
        moves.iter().fold(0, |node, mv| {
            tree.nodes[node]
                .children
                .iter()
                .find(|(m, _)| m == mv)
                .expect("Expected PGN branch is missing")
                .1
        })
    }

    /// PGN RAVs replace the preceding move. Nested and first-move alternatives
    /// must return to their own mainlines, regardless of branch insertion order.
    #[test]
    fn nested_variations_and_multiple_chapters() {
        let pgn = "[Event \"Open games\"]\n\n1. e4 (1. d4 d5) e5 (1... c5 2. Nf3 (2. Nc3) d6) 2. Nf3 Nc6 *\n\n[Event \"French\"]\n\n1. e4 e6 2. d4 d5 *";
        let tree = Repertoire::from_files(&[file(1, pgn)]).expect("Valid study");
        for line in [
            vec!["e2e4", "e7e5", "g1f3", "b8c6"],
            vec!["d2d4", "d7d5"],
            vec!["e2e4", "c7c5", "g1f3", "d7d6"],
            vec!["e2e4", "c7c5", "b1c3"],
            vec!["e2e4", "e7e6", "d2d4", "d7d5"],
        ] {
            let node = follow(&tree, &line);
            assert!(
                tree.nodes[node].children.is_empty(),
                "Unexpected continuation for {line:?}"
            );
        }
        let e4 = follow(&tree, &["e2e4"]);
        let mut chapters: Vec<_> = tree.nodes[e4]
            .sources
            .iter()
            .map(|s| s.chapter.as_str())
            .collect();
        chapters.sort();
        assert_eq!(chapters, ["French", "Open games"]);
    }

    /// Shared moves retain both identities; selecting one file must remove the
    /// other's replies. Reordering the files must not change available moves.
    #[test]
    fn selected_files_bound_replies_and_preserve_identity() {
        let a = file(1, "1. e4 e5 *");
        let mut b = file(2, "1. e4 c5 *");
        b.name = a.name.clone(); // Same filename does not imply the same source.
        for files in [vec![a.clone(), b.clone()], vec![b, a.clone()]] {
            let tree = Repertoire::from_files(&files).expect("Valid files");
            let node = follow(&tree, &["e2e4"]);
            let mut ids: Vec<_> = tree.nodes[node].sources.iter().map(|s| s.file_id).collect();
            ids.sort();
            assert_eq!(ids, [1, 2]);
            let mut moves: Vec<_> = tree.nodes[node]
                .children
                .iter()
                .map(|(m, _)| m.as_str())
                .collect();
            moves.sort();
            assert_eq!(moves, ["c7c5", "e7e5"]);
        }
        let tree = Repertoire::from_files(&[a]).expect("Valid selection");
        let node = follow(&tree, &["e2e4"]);
        assert_eq!(tree.nodes[node].children.len(), 1);
        assert_eq!(tree.nodes[node].children[0].0, "e7e5");
    }

    /// Bad imports must not become partially playable prep. Standard annotated
    /// PGNs remain accepted while unsupported starting positions fail explicitly.
    #[test]
    fn validates_imports() {
        for invalid in [
            "",
            "1. e5 *",
            "1. e4 (1. d4 *",
            "[FEN \"8/8/8/8/8/8/8/8 w - - 0 1\"]\n\n*",
            "[Variant \"Atomic\"]\n\n1. e4 *",
        ] {
            assert!(
                Repertoire::from_files(&[file(1, invalid)]).is_err(),
                "Accepted {invalid}"
            );
        }
        Repertoire::from_files(&[file(1, "1. e4! {comment} e5 $1 *")])
            .expect("Annotations are supported");
    }

    /// Persistence survives restart, and a rejected import leaves both memory
    /// and the saved library unchanged. No database or filesystem is mocked.
    #[test]
    fn library_persists_only_valid_imports() {
        let directory = std::env::temp_dir().join(format!(
            "chess-driller-{}-{}",
            std::process::id(),
            fastrand::u64(..)
        ));
        let mut library = Library::load(directory.clone()).expect("Create library");
        library
            .import("study.pgn".into(), "black".into(), "1. e4 c6 *".into())
            .expect("Import study");
        assert!(library
            .import("bad.pgn".into(), "white".into(), "1. e5 *".into())
            .is_err());
        assert_eq!(library.files.len(), 1);
        let loaded = Library::load(directory.clone()).expect("Reload library");
        assert_eq!(loaded.files.len(), 1);
        assert_eq!(loaded.files[0].color, "black");
        assert_eq!(loaded.files[0].pgn, "1. e4 c6 *");
        fs::remove_dir_all(directory).expect("Remove test library");
    }

    /// The repository's actual study exports must remain importable.
    #[test]
    fn supplied_studies() {
        for entry in walkdir::WalkDir::new("prep") {
            let entry = entry.expect("Read fixture directory");
            if entry.file_type().is_file() {
                let pgn = fs::read_to_string(entry.path()).expect("Read study");
                Repertoire::from_files(&[file(1, &pgn)]).expect("Import existing study");
            }
        }
    }
}
