#[derive(Copy, Clone)]
pub enum Page {
    Dashboard,
    NeighborCells,
    BandLock,
    Dmz,
}

impl Page {
    pub fn index(&self) -> usize {
        match self {
            Page::Dashboard => 0,
            Page::NeighborCells => 1,
            Page::BandLock => 2,
            Page::Dmz => 3,
        }
    }
}