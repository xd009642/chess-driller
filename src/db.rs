//! Store the opening preparation we want to work over - might rename it in future but it is kind
//! of a mini stripped-down move database.
use std::path::Path;

pub struct OpeningDatabase {
    white_openings: (),
    black_openings: (),
}

pub fn load_default_database() -> anyhow::Result<OpeningDatabase> {
    load_database(Path::new("prep"))
}

pub fn load_database(root: &Path) -> anyhow::Result<OpeningDatabase> {
    todo!()
}
