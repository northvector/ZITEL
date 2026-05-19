use ratatui::widgets::ListState;

pub struct BandLockState {
    pub items: Vec<String>,
    pub state: ListState,
}

impl BandLockState {
    pub fn new() -> Self {
        Self {
            items: vec![
                "42490".to_string(),
                "42690".to_string(),
                "42890".to_string(),
            ],
            state: ListState::default().with_selected(Some(0)),
        }
    }
}