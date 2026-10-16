//! Read-only presentation of the audited converter inventory and its evidence.
use crate::progress_data::{ApiRow, Counts, Dashboard, FeatureRow, Group, Status};
use crate::{mono, pixel, GOLD, GREEN, LINE, MUTED, PANEL, PINK, TEXT};
use eframe::egui::{self, vec2, Color32, RichText, Sense, Stroke};

const CYAN: Color32 = Color32::from_rgb(133, 216, 238);
const DARK: Color32 = Color32::from_rgb(20, 17, 32);

#[derive(Default, PartialEq, Eq, Clone, Copy)]
enum Tab {
    #[default]
    Overview,
    Api,
    Games,
}

#[derive(Default)]
pub struct ProgressView {
    tab: Tab,
    search: String,
    category: String,
    status: Option<Status>,
    observed_only: bool,
    selected_api: Option<String>,
    #[cfg(test)]
    metric_rects: Vec<egui::Rect>,
}

impl ProgressView {
    pub fn select_tab(&mut self, name: &str) {
        self.tab = match name {
            "api" => Tab::Api,
            "games" => Tab::Games,
            _ => Tab::Overview,
        };
    }

    pub fn render(&mut self, ui: &mut egui::Ui, data: &Dashboard) {
        ui.add_space(6.);
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new("TRANSLATION STATUS")
                    .font(pixel(13.))
                    .color(TEXT),
            );
            ui.label(RichText::new(&data.baseline).color(MUTED));
        });
        ui.add_space(5.);
        ui.label(
            RichText::new(
                "Implementation, exercised behavior and working games are counted separately.",
            )
            .color(MUTED),
        );
        ui.add_space(7.);
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.tab, Tab::Overview, "OVERVIEW");
            ui.selectable_value(&mut self.tab, Tab::Api, "API MATRIX");
            ui.selectable_value(&mut self.tab, Tab::Games, "20-GAME SAMPLE");
            ui.separator();
            if data.freshness.fresh {
                ui.label(RichText::new("● AUDIT CURRENT").color(GREEN));
            } else {
                ui.label(RichText::new("● AUDIT NEEDS REFRESH").color(GOLD));
            }
            ui.label(RichText::new(format!("Assessed {}", data.assessed_date)).color(MUTED));
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("converter-progress-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if !data.freshness.fresh {
                    notice(ui, GOLD, "Implementation files changed after this audit. These are the last assessed counts.");
                    for path in &data.freshness.changed {
                        ui.label(RichText::new(path).font(mono(11.)).color(MUTED));
                    }
                    ui.add_space(10.);
                }
                match self.tab {
                    Tab::Overview => self.overview(ui, data),
                    Tab::Api => self.api_matrix(ui, data),
                    Tab::Games => self.games(ui, data),
                }
                ui.add_space(15.);
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("SOURCE").font(mono(11.)).color(MUTED));
                    ui.hyperlink_to("Official PICO-8 manual", &data.baseline_url);
                    ui.label(RichText::new("· Local capability inventory and test reports").color(MUTED));
                });
                ui.label(RichText::new("One audited snapshot. No historical completion trend has been measured.").font(mono(11.)).color(MUTED));
                ui.add_space(8.);
            });
    }

    fn overview(&mut self, ui: &mut egui::Ui, data: &Dashboard) {
        #[cfg(test)]
        self.metric_rects.clear();
        let width = ui.available_width();
        let columns = if width >= 950. { 4 } else { 2 };
        let gap = 10.;
        let card_width = ((width - gap * (columns - 1) as f32) / columns as f32).max(150.);
        let total = data.api_counts.total();
        let sample_total = data.games.len();
        let cards = [
            (
                "FUNCTIONAL API NAMES",
                format!("{}/{}", data.api_counts.functional(), total),
                format!(
                    "{:.1}% · supported or partial behavior",
                    data.api_counts.functional_percent()
                ),
                CYAN,
            ),
            (
                "REGRESSION EXERCISED",
                if data.runtime_coverage.present && data.runtime_coverage.fresh {
                    format!("{}/{}", data.runtime_coverage.api_names_observed, total)
                } else if data.runtime_coverage.present {
                    "STALE".into()
                } else {
                    "UNMEASURED".into()
                },
                if data.runtime_coverage.present && data.runtime_coverage.fresh {
                    "Functional names reached by host tests".into()
                } else if data.runtime_coverage.present {
                    "Re-run scenarios after source changes".into()
                } else {
                    "No runtime coverage report available".into()
                },
                GOLD,
            ),
            (
                "SAMPLE CONFIRMED",
                format!("{}/{}", data.sample_confirmed, sample_total),
                format!(
                    "{:.1}% · display + controls on Pico",
                    percent(data.sample_confirmed, sample_total)
                ),
                GREEN,
            ),
            (
                "REFERENCE CONFORMANCE",
                "UNMEASURED".into(),
                "Independent PICO-8 comparison pending".into(),
                MUTED,
            ),
        ];
        for chunk in cards.chunks(columns) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for (title, value, subtitle, color) in chunk {
                    let _rect = metric(ui, card_width, title, value, subtitle, *color);
                    #[cfg(test)]
                    self.metric_rects.push(_rect);
                }
            });
            ui.add_space(5.);
        }
        ui.add_space(6.);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("API IMPLEMENTATION").font(mono(15.)).strong());
            legend(ui);
        });
        ui.label(RichText::new("Each API name has one status. Partial implementations count as functional names, with their limitations visible in the matrix.").color(MUTED));
        ui.add_space(5.);
        group_table(
            ui,
            "api-subsystem-counts",
            &data.categories,
            Some(&data.apis),
            data.runtime_coverage.fresh,
        );
        ui.label(RichText::new("CALLED counts functional names reached by host tests; it is execution coverage, not reference correctness.").font(mono(11.)).color(MUTED));
        ui.add_space(5.);
        if ui.button("EXPLORE THE API MATRIX  →").clicked() {
            self.tab = Tab::Api;
        }
        ui.add_space(12.);
        ui.label(
            RichText::new("ENTIRE TRANSLATION PIPELINE")
                .font(mono(15.))
                .strong(),
        );
        ui.label(RichText::new("This full checklist includes the API names above, plus source-language, cartridge, device and validation capabilities.").color(MUTED));
        ui.add_space(5.);
        group_table(
            ui,
            "compiler-subsystem-counts",
            &data.components,
            None,
            false,
        );
        egui::CollapsingHeader::new("Browse subsystem features and evidence")
            .id_salt("progress-core-evidence")
            .show(ui, |ui| {
                for group in &data.components {
                    egui::CollapsingHeader::new(&group.name)
                        .id_salt(format!("core-category-{}", group.id))
                        .show(ui, |ui| {
                            for item in &group.members {
                                feature_detail(ui, item);
                            }
                        });
                }
            });
        ui.add_space(10.);
        egui::CollapsingHeader::new("Supporting tools and delivery")
            .id_salt("progress-tooling")
            .show(ui, |ui| {
                for item in &data.tooling {
                    feature_detail(ui, item);
                }
            });
        egui::CollapsingHeader::new(format!("Planning index: {:.1}%  ·  secondary estimate", data.weighted_score))
            .id_salt("progress-planning-index")
            .show(ui, |ui| {
                ui.label("The original weighted capability index gives 1 point for supported, ½ for partial, and 0 for missing. Subsystem weights are engineering judgments.");
                ui.label(RichText::new(format!("{} checklist items: {} supported · {} partial · {} missing", data.core_counts.total(), data.core_counts.supported, data.core_counts.partial, data.core_counts.missing)).font(mono(12.)));
                ui.label(RichText::new("This is a planning measure. It does not estimate development time or the share of games that will run.").color(MUTED));
            });
    }

    fn api_matrix(&mut self, ui: &mut egui::Ui, data: &Dashboard) {
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(format!("{} CANONICAL API NAMES", data.api_counts.total()))
                    .font(mono(15.))
                    .strong(),
            );
            legend(ui);
        });
        ui.add_space(6.);
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Search API or limitation…")
                    .desired_width(255.),
            );
            egui::ComboBox::from_id_salt("progress-api-category")
                .selected_text(if self.category.is_empty() {
                    "All subsystems"
                } else {
                    &self.category
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.category, String::new(), "All subsystems");
                    for category in &data.categories {
                        ui.selectable_value(
                            &mut self.category,
                            category.name.clone(),
                            &category.name,
                        );
                    }
                });
            egui::ComboBox::from_id_salt("progress-api-status")
                .selected_text(
                    self.status
                        .map(|status| status.label())
                        .unwrap_or("All statuses"),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.status, None, "All statuses");
                    for status in [Status::Supported, Status::Partial, Status::Missing] {
                        ui.selectable_value(&mut self.status, Some(status), status.label());
                    }
                });
        });
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut self.observed_only, "Observed in host regression only");
            ui.label(
                RichText::new(
                    "A filled dot means the function was called; it does not prove correctness.",
                )
                .font(mono(11.))
                .color(MUTED),
            );
        });
        if data.runtime_coverage.present && !data.runtime_coverage.fresh {
            notice(
                ui,
                GOLD,
                "Host-call coverage is from an older implementation snapshot.",
            );
        }
        let q = self.search.trim().to_lowercase();
        let rows: Vec<_> = data
            .apis
            .iter()
            .filter(|api| {
                let category_matches = self.category.is_empty()
                    || data.categories.iter().any(|category| {
                        category.name == self.category
                            && (category.id == api.category || category.name == api.category)
                    });
                category_matches
                    && self
                        .status
                        .map(|status| api.status == status)
                        .unwrap_or(true)
                    && (!self.observed_only || api.observed_in_regression)
                    && (q.is_empty()
                        || format!("{} {} {}", api.name, api.reason, api.aliases.join(" "))
                            .to_lowercase()
                            .contains(&q))
            })
            .collect();
        ui.label(
            RichText::new(format!(
                "{} of {} names · select a cell for limits and evidence",
                rows.len(),
                data.apis.len()
            ))
            .color(MUTED),
        );
        ui.add_space(4.);
        let columns = (ui.available_width() / 126.).floor().max(2.) as usize;
        let width = (ui.available_width() - (columns - 1) as f32 * 7.) / columns as f32;
        let mut selected_now = false;
        egui::Grid::new("api-coverage-cells")
            .spacing(vec2(7., 7.))
            .num_columns(columns)
            .show(ui, |ui| {
                for (index, api) in rows.iter().enumerate() {
                    let chosen = self.selected_api.as_deref() == Some(api.name.as_str());
                    if api_cell(ui, api, width, chosen) {
                        self.selected_api = Some(api.name.clone());
                        selected_now = true;
                    }
                    if (index + 1) % columns == 0 {
                        ui.end_row();
                    }
                }
            });
        if rows.is_empty() {
            ui.add_space(10.);
            ui.label(RichText::new("No API names match these filters.").color(MUTED));
        }
        if let Some(api) = self
            .selected_api
            .as_deref()
            .and_then(|name| data.apis.iter().find(|api| api.name == name))
        {
            ui.add_space(12.);
            let response = detail(ui, api, data.runtime_coverage.fresh);
            if selected_now {
                response.scroll_to_me(Some(egui::Align::Center));
            }
        } else {
            ui.add_space(12.);
            notice(ui, MUTED, "Select an API name to inspect its assessed behavior, known limits and test evidence.");
        }
    }

    fn games(&mut self, ui: &mut egui::Ui, data: &Dashboard) {
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new("FROZEN RANDOM SAMPLE")
                    .font(mono(15.))
                    .strong(),
            );
            ui.label(
                RichText::new(format!(
                    "{} built / {} · {} confirmed on Pico",
                    data.sample_built,
                    data.games.len(),
                    data.sample_confirmed
                ))
                .color(GREEN),
            );
        });
        ui.label(RichText::new("The same 20 games from the Top 200 list are retried without substitutions. A successful build and an owner-confirmed hardware check are separate stages.").color(MUTED));
        ui.add_space(8.);
        for game in &data.games {
            egui::Frame::new()
                .fill(PANEL)
                .stroke(Stroke::new(1_f32, LINE))
                .inner_margin(12.)
                .show(ui, |ui| {
                    let width = ui.available_width();
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                RichText::new(format!("#{:03}", game.rank))
                                    .font(mono(12.))
                                    .color(MUTED),
                            );
                            ui.label(RichText::new(&game.title).font(mono(16.)).strong());
                            ui.label(
                                RichText::new(if game.built { "BUILT" } else { "BLOCKED" })
                                    .color(if game.built { GREEN } else { GOLD }),
                            );
                            if game.hardware_confirmed {
                                ui.label(
                                    RichText::new("● DISPLAY + CONTROLS CONFIRMED")
                                        .font(mono(11.))
                                        .color(GREEN),
                                );
                            }
                        });
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                RichText::new(format!("Stage: {}", game.stage))
                                    .font(mono(12.))
                                    .color(MUTED),
                            );
                            if game.host_frames > 0 {
                                ui.label(
                                    RichText::new(format!(
                                        "Host smoke test: {} frames",
                                        game.host_frames
                                    ))
                                    .font(mono(12.))
                                    .color(CYAN),
                                );
                            }
                            ui.hyperlink_to("Cartridge source ↗", &game.source_url);
                        });
                        if !game.error.is_empty() {
                            ui.add_space(3.);
                            ui.label(RichText::new(&game.error).font(mono(12.)).color(GOLD));
                        }
                        if game.built && !game.hardware_confirmed {
                            ui.label(
                                RichText::new("Hardware gameplay check pending.").color(MUTED),
                            );
                        }
                        if !game.owner_confirmation.is_empty() && game.hardware_confirmed {
                            ui.label(RichText::new(&game.owner_confirmation).color(MUTED));
                        }
                    });
                });
            ui.add_space(6.);
        }
        ui.label(
            RichText::new(
                "Blocked games show the first detected failure. Later features may also need work.",
            )
            .color(MUTED),
        );
    }
}

fn percent(n: usize, total: usize) -> f64 {
    if total == 0 {
        0.
    } else {
        n as f64 * 100. / total as f64
    }
}

fn tone(status: Status) -> Color32 {
    match status {
        Status::Supported => GREEN,
        Status::Partial => GOLD,
        Status::Missing => PINK,
    }
}

fn status_pill(ui: &mut egui::Ui, status: Status) {
    ui.label(
        RichText::new(format!("● {}", status.label()))
            .font(mono(11.))
            .color(tone(status)),
    );
}

fn feature_detail(ui: &mut egui::Ui, item: &FeatureRow) {
    ui.horizontal_wrapped(|ui| {
        status_pill(ui, item.status);
        ui.label(RichText::new(&item.name).strong());
    });
    ui.label(RichText::new(&item.reason).color(MUTED));
    if !item.tests.is_empty() {
        ui.label(
            RichText::new(format!("Selected tests: {}", item.tests.join(", ")))
                .font(mono(11.))
                .color(MUTED),
        );
    }
    ui.label(
        RichText::new(if item.reference_validated {
            "Reference comparison recorded."
        } else {
            "Independent reference comparison: unmeasured."
        })
        .font(mono(11.))
        .color(MUTED),
    );
    if !item.evidence.is_empty() {
        egui::CollapsingHeader::new("Evidence")
            .id_salt(format!("feature-evidence-{}", item.id))
            .show(ui, |ui| {
                for evidence in &item.evidence {
                    ui.label(RichText::new(evidence).font(mono(11.)).color(MUTED));
                }
            });
    }
    ui.add_space(8.);
}

fn legend(ui: &mut egui::Ui) {
    for status in [Status::Supported, Status::Partial, Status::Missing] {
        status_pill(ui, status);
    }
}

fn notice(ui: &mut egui::Ui, color: Color32, text: &str) {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1_f32, color.gamma_multiply(0.45)))
        .inner_margin(10.)
        .show(ui, |ui| {
            let width = ui.available_width();
            ui.vertical(|ui| {
                ui.set_width(width);
                ui.add(egui::Label::new(RichText::new(text).color(color)).wrap());
            });
        });
}

fn metric(
    ui: &mut egui::Ui,
    width: f32,
    title: &str,
    value: &str,
    subtitle: &str,
    color: Color32,
) -> egui::Rect {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1_f32, LINE))
        .inner_margin(12.)
        .show(ui, |ui| {
            // Frames inherit their parent's layout. These cards live in a row,
            // so their contents must explicitly start a bounded vertical UI.
            ui.set_width(width - 26.);
            ui.vertical(|ui| {
                ui.set_width(width - 26.);
                ui.set_min_height(98.);
                ui.add(egui::Label::new(RichText::new(title).font(mono(10.)).color(MUTED)).wrap());
                ui.add_space(5.);
                ui.add(
                    egui::Label::new(
                        RichText::new(value)
                            .font(mono(if value == "UNMEASURED" { 20. } else { 29. }))
                            .strong()
                            .color(color),
                    )
                    .wrap(),
                );
                ui.add(
                    egui::Label::new(RichText::new(subtitle).font(mono(11.)).color(MUTED)).wrap(),
                );
            });
        })
        .response
        .rect
}

fn group_table(
    ui: &mut egui::Ui,
    id: &str,
    groups: &[Group],
    apis: Option<&[ApiRow]>,
    coverage_fresh: bool,
) {
    let available = ui.available_width();
    let name_width = (available * 0.36).clamp(210., 430.);
    let bars_width = (available - name_width - if apis.is_some() { 306. } else { 254. }).max(65.);
    egui::Grid::new(id)
        .num_columns(if apis.is_some() { 7 } else { 6 })
        .striped(true)
        .spacing(vec2(12., 10.))
        .show(ui, |ui| {
            for (name, color) in [
                ("SUBSYSTEM", MUTED),
                ("SUPPORTED", GREEN),
                ("PARTIAL", GOLD),
                ("MISSING", PINK),
                ("TOTAL", MUTED),
            ] {
                ui.label(RichText::new(name).font(mono(10.)).color(color));
            }
            if apis.is_some() {
                ui.label(RichText::new("CALLED").font(mono(10.)).color(CYAN));
            }
            ui.label(RichText::new("COVERAGE").font(mono(10.)).color(MUTED));
            ui.end_row();
            for group in groups {
                ui.add_sized(
                    [name_width, 30.],
                    egui::Label::new(RichText::new(&group.name).font(mono(12.)))
                        .wrap()
                        .halign(egui::Align::Min),
                );
                ui.label(
                    RichText::new(group.counts.supported.to_string())
                        .font(mono(14.))
                        .color(GREEN),
                );
                ui.label(
                    RichText::new(group.counts.partial.to_string())
                        .font(mono(14.))
                        .color(GOLD),
                );
                ui.label(
                    RichText::new(group.counts.missing.to_string())
                        .font(mono(14.))
                        .color(PINK),
                );
                ui.label(
                    RichText::new(group.counts.total().to_string())
                        .font(mono(12.))
                        .color(MUTED),
                );
                if let Some(apis) = apis {
                    let called = apis
                        .iter()
                        .filter(|api| api.category == group.id && api.observed_in_regression)
                        .count();
                    ui.label(
                        RichText::new(if coverage_fresh {
                            format!("{}/{}", called, group.counts.total())
                        } else {
                            "—".into()
                        })
                        .font(mono(12.))
                        .color(CYAN),
                    );
                }
                let response = stacked_bar(ui, &group.counts, bars_width);
                response.on_hover_ui(|ui| {
                    ui.label(&group.name);
                    ui.label(format!(
                        "{} supported · {} partial · {} missing",
                        group.counts.supported, group.counts.partial, group.counts.missing
                    ));
                    ui.separator();
                    for item in &group.members {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new("●").color(tone(item.status)));
                            ui.label(&item.name);
                        });
                    }
                });
                ui.end_row();
            }
        });
}

fn stacked_bar(ui: &mut egui::Ui, counts: &Counts, width: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(width, 13.), Sense::hover());
    ui.painter().rect_filled(rect, 2., DARK);
    let total = counts.total().max(1) as f32;
    let mut left = rect.left();
    for (count, color) in [
        (counts.supported, GREEN),
        (counts.partial, GOLD),
        (counts.missing, PINK),
    ] {
        let span = rect.width() * count as f32 / total;
        if span > 0. {
            let section =
                egui::Rect::from_min_size(egui::pos2(left, rect.top()), vec2(span, rect.height()));
            ui.painter().rect_filled(
                section.shrink2(vec2(0.5, 0.)),
                1.,
                color.gamma_multiply(0.85),
            );
            left += span;
        }
    }
    response
}

fn api_cell(ui: &mut egui::Ui, api: &ApiRow, width: f32, selected: bool) -> bool {
    let color = tone(api.status);
    let response = ui.add_sized(
        [width, 43.],
        egui::Button::new(
            RichText::new(format!(
                "{}\n{}{}",
                api.name,
                match api.status {
                    Status::Supported => "SUPPORTED",
                    Status::Partial => "PARTIAL",
                    Status::Missing => "MISSING",
                },
                if api.observed_in_regression {
                    " ●"
                } else {
                    ""
                }
            ))
            .font(mono(12.))
            .color(color),
        )
        .fill(if selected {
            color.gamma_multiply(0.22)
        } else {
            PANEL
        })
        .stroke(Stroke::new(
            if selected { 2_f32 } else { 1_f32 },
            if selected {
                color
            } else {
                color.gamma_multiply(0.40)
            },
        )),
    );
    let clicked = response.clicked();
    response.on_hover_ui(|ui| {
        ui.set_max_width(420.);
        ui.label(
            RichText::new(format!("{} — {}", api.name, api.status.label()))
                .color(color)
                .strong(),
        );
        ui.label(&api.reason);
        ui.label(
            RichText::new(if api.observed_in_regression {
                "Called in host regression; no reference equivalence claim."
            } else {
                "No current functional host-call coverage is credited."
            })
            .color(MUTED),
        );
    });
    clicked
}

fn detail(ui: &mut egui::Ui, api: &ApiRow, coverage_fresh: bool) -> egui::Response {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1_f32, tone(api.status).gamma_multiply(0.5)))
        .inner_margin(14.)
        .show(ui, |ui| {
            let width = ui.available_width();
            ui.vertical(|ui| {
                ui.set_width(width);
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(&api.name).font(mono(22.)).color(TEXT));
                    status_pill(ui, api.status);
                    ui.label(RichText::new(&api.category).color(MUTED));
                });
                ui.label(&api.reason);
                if !api.aliases.is_empty() {
                    ui.label(
                        RichText::new(format!("Aliases: {}", api.aliases.join(", ")))
                            .font(mono(12.))
                            .color(MUTED),
                    );
                }
                ui.add_space(7.);
                if api.observed_in_regression {
                    ui.label(
                        RichText::new(format!(
                            "● Exercised in host regression · {} recorded calls",
                            api.host_calls
                        ))
                        .color(CYAN),
                    );
                } else if coverage_fresh && api.host_calls > 0 && api.status == Status::Missing {
                    ui.label(
                        RichText::new(format!(
                            "Stub called {} times; no functional behavior coverage credited.",
                            api.host_calls
                        ))
                        .color(GOLD),
                    );
                } else if coverage_fresh {
                    ui.label(
                        RichText::new("○ Not exercised in the recorded host regression scenarios")
                            .color(MUTED),
                    );
                } else {
                    ui.label(
                        RichText::new("○ No current host-call coverage available").color(MUTED),
                    );
                }
                ui.label(
                    RichText::new(if api.reference_validated {
                        "Reference comparison evidence recorded for this API."
                    } else {
                        "Independent reference correctness: unmeasured."
                    })
                    .color(MUTED),
                );
                if !api.selected_regression_tests.is_empty() {
                    ui.label(
                        RichText::new(format!(
                            "Selected tests: {}",
                            api.selected_regression_tests.join(", ")
                        ))
                        .font(mono(12.))
                        .color(MUTED),
                    );
                }
                if !api.evidence.is_empty() {
                    egui::CollapsingHeader::new("Implementation and assessment evidence")
                        .id_salt(format!("api-evidence-{}", api.name))
                        .show(ui, |ui| {
                            for evidence in &api.evidence {
                                ui.label(RichText::new(evidence).font(mono(11.)).color(MUTED));
                            }
                        });
                }
            });
        })
        .response
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn overview_metric_cards_fit_1100_and_1300_pixel_viewports() {
        let data = Dashboard::load(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        for width in [1100_f32, 1300_f32] {
            let ctx = egui::Context::default();
            let mut fonts = egui::FontDefinitions::default();
            fonts.families.insert(
                egui::FontFamily::Name("pixel".into()),
                fonts.families[&egui::FontFamily::Monospace].clone(),
            );
            ctx.set_fonts(fonts);
            let mut view = ProgressView::default();
            let mut bounds = egui::Rect::NOTHING;
            for _ in 0..2 {
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            vec2(width, 850.),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            bounds = ui.available_rect_before_wrap();
                            view.render(ui, &data);
                        });
                    },
                );
            }
            assert_eq!(view.metric_rects.len(), 4);
            for rect in &view.metric_rects {
                assert!(
                    rect.left() >= bounds.left() - 1.,
                    "card left {rect:?} outside {bounds:?}"
                );
                assert!(
                    rect.right() <= bounds.right() + 1.,
                    "card right {rect:?} outside {bounds:?}"
                );
                assert!(
                    rect.width() <= bounds.width() / 4.,
                    "card expanded beyond quarter viewport: {rect:?}"
                );
            }
            for pair in view.metric_rects.windows(2) {
                assert!(pair[0].right() <= pair[1].left(), "metric cards overlap");
            }
        }
    }
}
