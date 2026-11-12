//! The Bench's ERP card: a quick search of the ERP, the identity picked, and
//! the Positions Test so far with Send, at the top of the Bench when the ERP
//! is set up. It shows the same identity, search and results as the ERP view
//! (one state), and the table is the Positions Test pane's own
//! (`guided::free_grid`).

use super::erp::show_note;
use super::*;
use timegrapher_erp::client::Identity;
use timegrapher_erp::operator;
use timegrapher_erp::payload::PositionResult;

/// How wide the card's identity column is.
const IDENTITY_W: f32 = 360.0;
/// How wide the quick search field is.
const SEARCH_W: f32 = 230.0;
/// How wide the quick sign-in's Operator ID and PIN fields are.
const OPERATOR_W: f32 = 120.0;
const PIN_W: f32 = 90.0;
/// How many matches the quick search lists at most.
const MATCHES_SHOWN: usize = 6;
/// Below this much room the card folds to its one row by itself.
const FOLD_BELOW_H: f32 = 520.0;

/// What the card's controls asked for.
#[derive(Default)]
struct Asked {
    typed: bool,
    pick: Option<String>,
    send: bool,
    fold: bool,
    open_view: bool,
    sign_in: bool,
}

/// The identity's lines for the card, as the ERP view shows them.
fn identity_rows(id: &Identity) -> Vec<(&'static str, String)> {
    let status = [&id.lifecycle_status, &id.functional_grade, &id.qa_status]
        .iter()
        .filter_map(|s| s.as_deref())
        .collect::<Vec<_>>()
        .join(" · ");
    let part = id.ln_part.clone().or_else(|| id.item.clone());
    [
        ("Identity", id.identity_id.clone()),
        ("Movement S/N", id.movement_serial.clone()),
        ("Part", part),
        ("Watch Unit", id.current_watch_unit.clone()),
        ("Work Order", id.current_work_order.clone()),
        ("Status", (!status.is_empty()).then_some(status)),
        ("Recommended", id.recommended_measurement_type.clone()),
    ]
    .into_iter()
    .filter_map(|(label, value)| Some((label, value.filter(|v| !v.is_empty())?)))
    .collect()
}

impl TimegrapherApp {
    /// The ERP card at the top of the Bench; nothing when the ERP is not
    /// set up.
    pub(super) fn erp_card(&mut self, ui: &mut egui::Ui) {
        if !self.erp.live() {
            return;
        }
        let ctx = ui.ctx().clone();
        let results = self.erp_results_now(&ctx);
        let blocker = self.erp_send_blocker(&results);
        let folded = self.erp.card_closed || ui.available_height() < FOLD_BELOW_H;
        let mut asked = Asked::default();
        watches::panel(ui, |ui| {
            self.erp_card_row(ui, &results, blocker, folded, &mut asked);
            self.erp_card_signin(ui, &mut asked);
            self.erp_card_matches(ui, &mut asked);
            if !folded {
                self.erp_card_body(ui, &ctx);
            }
            show_note(ui, &self.erp.view_note.clone());
            for line in &self.erp.sent {
                ui.label(RichText::new(line).small());
            }
        });
        ui.add_space(6.0);
        if asked.typed {
            self.erp.typed_at = Some(std::time::Instant::now());
            self.erp.search_open = true;
        }
        if let Some(id) = asked.pick.filter(|_| !self.erp.busy()) {
            self.erp.look_up(&ctx, id);
        }
        if asked.fold {
            self.erp.card_closed = !self.erp.card_closed;
        }
        if asked.open_view {
            self.view = View::Erp;
        }
        if asked.sign_in {
            self.erp.sign_in(&ctx);
        }
        if asked.send {
            self.erp_send_now(&ctx, results);
        }
    }

    /// The card's one row: the name, the quick search, who is signed in, the
    /// identity in a few words, and at the right Send, the ERP view and the
    /// fold.
    fn erp_card_row(
        &mut self,
        ui: &mut egui::Ui,
        results: &[PositionResult],
        blocker: Option<&'static str>,
        folded: bool,
        asked: &mut Asked,
    ) {
        let pal = theme::pal(ui);
        ui.horizontal(|ui| {
            ui.label(RichText::new("ERP").font(theme::semibold(13.0)));
            let field = ui
                .add(
                    egui::TextEdit::singleline(&mut self.erp.query)
                        .hint_text("Quick search the ERP")
                        .desired_width(SEARCH_W),
                )
                .on_hover_text("Find a movement or watch by serial, label, part or work order");
            asked.typed |= field.changed();
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if enter && self.erp.search_open {
                asked.pick = self.erp.results.first().and_then(|r| r.identity_id.clone());
            }
            if field.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.erp.search_open = false;
            }
            if self.erp.busy() {
                ui.spinner();
            }
            if let Some(id) = &self.erp.picked {
                let name = id.identity_id.as_deref().unwrap_or("");
                let part = id.ln_part.as_deref().or(id.item.as_deref()).unwrap_or("");
                ui.label(RichText::new(format!("{name}  {part}")).strong());
            }
            let who = match (&self.erp.operator, self.erp.signed_in()) {
                (Some(o), true) => format!("Signed in: {}", o.operator_name),
                (Some(_), false) => "Sign-in ended".to_string(),
                (None, _) => "Not signed in".to_string(),
            };
            ui.label(RichText::new(who).small().color(pal.text_secondary))
                .on_hover_text("The operator the ERP vouches for. Sign in in Settings > ERP.");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let icon = if folded {
                    Icon::ArrowRight
                } else {
                    Icon::ArrowDown
                };
                if theme::icon_tool(ui, icon)
                    .on_hover_text("Show or hide the identity and the Positions Test")
                    .clicked()
                {
                    asked.fold = true;
                }
                if ui
                    .add(theme::secondary(ui, "ERP View"))
                    .on_hover_text("The ERP's own view: search results, the note, what to send")
                    .clicked()
                {
                    asked.open_view = true;
                }
                let text = format!(
                    "Send {} Position{}",
                    results.len(),
                    if results.len() == 1 { "" } else { "s" }
                );
                if ui
                    .add_enabled(blocker.is_none(), theme::primary(ui, &text))
                    .on_hover_text("Sign each position and send it to the ERP")
                    .on_disabled_hover_text(blocker.unwrap_or(""))
                    .clicked()
                {
                    asked.send = true;
                }
            });
        });
    }

    /// A quick sign-in under the row while nobody is signed in: the Operator
    /// ID and PIN, and Sign In. Enter in the PIN field signs in too.
    fn erp_card_signin(&mut self, ui: &mut egui::Ui, asked: &mut Asked) {
        if self.erp.signed_in() {
            return;
        }
        let pal = theme::pal(ui);
        let ready = !self.erp.busy();
        let id_ok = !operator::clean_operator_id(&self.erp.operator_id).is_empty();
        let pin_ok = operator::pin_format_ok(&self.erp.pin);
        let ended = self.erp.operator.is_some();
        ui.horizontal(|ui| {
            let words = if ended {
                "Sign-in ended. Sign in again:"
            } else {
                "Sign in so the ERP attests your results:"
            };
            ui.label(RichText::new(words).small().color(pal.text_secondary));
            ui.add_sized(
                [OPERATOR_W, theme::CONTROL_H],
                egui::TextEdit::singleline(&mut self.erp.operator_id).hint_text("Operator ID"),
            );
            let pin = ui.add_sized(
                [PIN_W, theme::CONTROL_H],
                egui::TextEdit::singleline(&mut *self.erp.pin)
                    .password(true)
                    .hint_text("PIN"),
            );
            let can = ready && id_ok && pin_ok;
            let why = if !id_ok {
                "Type the Operator ID"
            } else if !pin_ok {
                "The PIN is 6 to 8 digits"
            } else {
                "The ERP is answering"
            };
            let clicked = ui
                .add_enabled(can, theme::primary(ui, "Sign In"))
                .on_hover_text("Sign in so the ERP can attest your results")
                .on_disabled_hover_text(why)
                .clicked();
            let enter = pin.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            asked.sign_in |= can && (clicked || enter);
        });
        show_note(ui, &self.erp.operator_note.clone());
    }

    /// The quick search's matches, under the row while it is open.
    fn erp_card_matches(&mut self, ui: &mut egui::Ui, asked: &mut Asked) {
        if !self.erp.search_open || self.erp.results.is_empty() {
            return;
        }
        for item in self.erp.results.iter().take(MATCHES_SHOWN) {
            let id = item.identity_id.clone().unwrap_or_default();
            let text = item.display.clone().unwrap_or_else(|| id.clone());
            if ui.selectable_label(false, text).clicked() {
                asked.pick = Some(id);
            }
        }
    }

    /// The identity's lines at the left and the Positions Test table at the
    /// right.
    fn erp_card_body(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let pal = theme::pal(ui);
        let rows = self.erp.picked.as_ref().map(identity_rows);
        let table = self.free_table_shown(ctx);
        ui.add_space(4.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(IDENTITY_W);
                match &rows {
                    Some(rows) => {
                        for (label, value) in rows {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(*label).color(pal.text_secondary));
                                ui.label(value);
                            });
                        }
                    }
                    None => {
                        ui.label(
                            RichText::new("Search above to pick the watch being measured.")
                                .small()
                                .color(pal.text_secondary),
                        );
                    }
                }
            });
            ui.add_space(16.0);
            ui.vertical(|ui| match &table {
                Some(t) => guided::free_grid(ui, t),
                None => {
                    ui.label(
                        RichText::new("The Positions Test on the Bench shows here.")
                            .small()
                            .color(pal.text_secondary),
                    );
                }
            });
        });
    }
}
