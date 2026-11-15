#![doc = include_str!("../README.md")]

pub mod amm;
pub mod bridge;
pub mod connect_four;
pub mod exact_cover;
pub mod lending;
pub mod logistics;
pub mod marketplace;
pub mod maze;
pub mod mission;
pub mod perishables;
pub mod rescue;
pub mod scheduling;
pub mod sokoban;
pub mod work_league;
pub mod workshop;

#[cfg(feature = "packages")]
mod package;

/// What writes a sample's package into an engine's store, everything named
/// under the sample's name.
#[cfg(feature = "packages")]
pub type WritePackage = fn(&axionomy_engine::Engine) -> Result<(), axionomy_engine::EngineError>;

/// The samples written as packages, by name, in the order Studio lists them.
#[cfg(feature = "packages")]
pub fn packages() -> Vec<(axionomy_engine::PackagePath, WritePackage)> {
    let packages: [(&str, WritePackage); 15] = [
        ("maze", maze::package::write),
        ("sokoban", sokoban::package::write),
        ("exact-cover", exact_cover::package::write),
        ("workshop", workshop::package::write),
        ("bridge", bridge::package::write),
        ("rescue", rescue::package::write),
        ("scheduling", scheduling::package::write),
        ("amm", amm::package::write),
        ("lending", lending::package::write),
        ("marketplace", marketplace::package::write),
        ("logistics", logistics::package::write),
        ("connect-four", connect_four::package::write),
        ("work-league", work_league::package::write),
        ("mission", mission::package::write),
        ("perishables", perishables::package::write),
    ];
    packages
        .into_iter()
        .map(|(name, write)| (name.parse().expect("a sample's name is a path"), write))
        .collect()
}
