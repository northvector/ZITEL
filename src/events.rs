use crossterm::event::{KeyCode, KeyEvent};

use tokio::sync::mpsc;

use crate::{
    app::{App, DEFAULT_DMZ_IP},
    models::requests::{Request, Response},
};

pub fn send_request(
    worker_tx: &mpsc::UnboundedSender<(Request, mpsc::UnboundedSender<Response>)>,
    response_tx: &mpsc::UnboundedSender<Response>,
    request: Request,
) {
    let _ = worker_tx.send((request, response_tx.clone()));
}

pub fn handle_key_event(
    key: KeyEvent,
    app: &mut App,
    response_tx: &mpsc::UnboundedSender<Response>,
) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => {
            return true;
        }

        KeyCode::Char('i') | KeyCode::Char('I') => {
            let current_earfcn = app.index_data["EARFCN"]
                .as_str()
                .unwrap_or("")
                .trim()
                .to_string();

            if current_earfcn.is_empty() {
                app.status_message = "Unable to detect current EARFCN".to_string();

                return false;
            }

            send_request(
                &app.request_tx,
                response_tx,
                Request::SetBandLock {
                    earfcn: current_earfcn.clone(),
                },
            );

            app.band_lock_response = Some(format!(
                "Refreshing IP using active EARFCN {}...",
                current_earfcn
            ));

            app.status_message = format!("Reapplying current EARFCN {}", current_earfcn);
        }

        KeyCode::Tab => {
            app.next_page();
        }

        KeyCode::BackTab => {
            app.previous_page();
        }

        KeyCode::Char('1') => app.go_to_page(0),
        KeyCode::Char('2') => app.go_to_page(1),
        KeyCode::Char('3') => app.go_to_page(2),
        KeyCode::Char('4') => app.go_to_page(3),

        KeyCode::Up => {
            let current = app.band_lock_state.state.selected().unwrap_or(0);

            let new = current.saturating_sub(1);

            app.band_lock_state.state.select(Some(new));
        }

        KeyCode::Down => {
            let current = app.band_lock_state.state.selected().unwrap_or(0);

            let max = app.band_lock_state.items.len() - 1;

            let new = (current + 1).min(max);

            app.band_lock_state.state.select(Some(new));
        }

        KeyCode::Enter => match app.page {
            crate::models::page::Page::NeighborCells => {
                if !app.neighbour_fetching {
                    app.neighbour_fetching = true;

                    send_request(&app.request_tx, response_tx, Request::FetchNeighbors);
                }
            }

            crate::models::page::Page::BandLock => {
                let selected = app.band_lock_state.state.selected().unwrap_or(0);

                let earfcn = app.band_lock_state.items[selected].clone();

                send_request(
                    &app.request_tx,
                    response_tx,
                    Request::SetBandLock { earfcn },
                );

                app.band_lock_response = Some("Sending...".to_string());
            }

            crate::models::page::Page::Dmz => {
                let ip = if app.dmz_ip_input.is_empty() {
                    DEFAULT_DMZ_IP.to_string()
                } else {
                    app.dmz_ip_input.clone()
                };

                send_request(&app.request_tx, response_tx, Request::SetDmz { ip });

                app.dmz_response = Some("Sending...".to_string());

                app.dmz_ip_input.clear();
            }

            _ => {}
        },

        KeyCode::Backspace => {
            app.dmz_ip_input.pop();
        }

        KeyCode::Char(c) => {
            if matches!(app.page, crate::models::page::Page::Dmz) {
                if c.is_ascii_digit() || c == '.' {
                    app.dmz_ip_input.push(c);
                }
            }
        }

        _ => {}
    }

    false
}
