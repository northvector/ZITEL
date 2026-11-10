#[derive(Copy, Clone, PartialEq)]
pub enum Page {
    Dashboard,
    NeighborCells,
    BandLock,
    BandScan,
    Dmz,
}

impl Page {
    pub const ALL: [Page; 5] = [
        Page::Dashboard,
        Page::NeighborCells,
        Page::BandLock,
        Page::BandScan,
        Page::Dmz,
    ];

    pub const TITLES: [&'static str; 5] = ["Dashboard", "Neighbors", "BandLock", "Scan", "DMZ"];

    pub fn index(&self) -> usize {
        Page::ALL.iter().position(|p| p == self).unwrap_or(0)
    }
}
