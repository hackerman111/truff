use super::super::{editor::color, icons, resolve_font_and_line_height, widgets};
use crate::app::Application;
use crate::theme::Theme;
use eframe::egui;

pub struct SettingsDialog {
    pub open: bool,
    pub font_family: String,
    pub base_font_size: f32,
    pub line_height_multiplier: f32,
    pub custom_font_path: String,
    pub active_theme: String,
    pub error: Option<String>,
}

impl Default for SettingsDialog {
    fn default() -> Self {
        Self {
            open: false,
            font_family: "monospace".into(),
            base_font_size: 15.0,
            line_height_multiplier: 1.5,
            custom_font_path: "".into(),
            active_theme: "nord".into(),
            error: None,
        }
    }
}

impl SettingsDialog {
    pub fn init_from_app(
        &mut self,
        typography: &crate::app::global_config::TypographyConfig,
        theme_name: &str,
    ) {
        self.font_family =
            if typography.custom_font_path.is_some() || typography.font_family == "custom" {
                "custom".into()
            } else if typography.font_family == "proportional" {
                "proportional".into()
            } else {
                "monospace".into()
            };
        self.base_font_size = typography.base_font_size;
        self.line_height_multiplier = typography.line_height_multiplier;
        self.custom_font_path = typography
            .custom_font_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.active_theme = theme_name.to_string();
        self.error = None;
    }

    pub fn init_from_config(
        &mut self,
        typography: &crate::app::global_config::TypographyConfig,
        theme_name: &str,
    ) {
        self.init_from_app(typography, theme_name);
    }

    pub(super) fn show(
        &mut self,
        application: &mut Application,
        context: &egui::Context,
        theme: &Theme,
    ) {
        if !self.open {
            return;
        }

        let mut close_dialog = false;
        let mut submit_settings = false;

        let modal_frame = egui::Frame::new()
            .fill(color(theme.bg_secondary))
            .stroke(egui::Stroke::new(1.0, color(theme.border)))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::same(20));

        egui::Window::new("settings_window")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .default_width(480.0)
            .max_width(480.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .frame(modal_frame)
            .show(context, |ui| {
                ui.visuals_mut().extreme_bg_color =
                    color(crate::theme::Color::from_hex("#20242c").unwrap());
                ui.visuals_mut().widgets.inactive.bg_stroke =
                    egui::Stroke::new(1.0, color(theme.border));
                ui.visuals_mut().widgets.hovered.bg_stroke =
                    egui::Stroke::new(1.0, color(theme.accent));

                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("Настройки")
                                .size(16.0)
                                .strong()
                                .color(color(theme.text_primary)),
                        );
                        ui.label(
                            egui::RichText::new("Типографика редактора и внешний вид")
                                .size(12.0)
                                .color(color(theme.text_muted)),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if widgets::icon_button(ui, icons::Icon::Close, "Закрыть").clicked()
                        {
                            close_dialog = true;
                        }
                    });
                });

                ui.add_space(14.0);
                ui.label(
                    egui::RichText::new("Семейство шрифта")
                        .size(12.0)
                        .strong()
                        .color(color(theme.text_primary)),
                );
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    let is_mono = self.font_family == "monospace";
                    let is_prop = self.font_family == "proportional";
                    let is_custom = self.font_family == "custom";

                    if ui
                        .selectable_label(is_mono, "Моноширинный (по умолчанию)")
                        .clicked()
                    {
                        self.font_family = "monospace".into();
                    }
                    if ui.selectable_label(is_prop, "Пропорциональный").clicked() {
                        self.font_family = "proportional".into();
                    }
                    if ui
                        .selectable_label(is_custom, "Пользовательский файл")
                        .clicked()
                    {
                        self.font_family = "custom".into();
                    }
                });

                if self.font_family == "custom" {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Путь к TTF/OTF:")
                                .size(11.5)
                                .color(color(theme.text_muted)),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut self.custom_font_path)
                                .desired_width(320.0)
                                .hint_text("/path/to/font.ttf")
                                .margin(egui::Margin::symmetric(8, 6)),
                        );
                    });
                }

                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("Базовый размер шрифта")
                        .size(12.0)
                        .strong()
                        .color(color(theme.text_primary)),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button(" - ").clicked() && self.base_font_size > 10.0 {
                        self.base_font_size = (self.base_font_size - 1.0).max(10.0);
                    }
                    ui.add(
                        egui::Slider::new(&mut self.base_font_size, 10.0..=32.0)
                            .step_by(1.0)
                            .suffix(" px"),
                    );
                    if ui.button(" + ").clicked() && self.base_font_size < 32.0 {
                        self.base_font_size = (self.base_font_size + 1.0).min(32.0);
                    }
                    if ui.button("Сброс (15px)").clicked() {
                        self.base_font_size = 15.0;
                    }
                });

                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("Предпросмотр")
                        .size(12.0)
                        .strong()
                        .color(color(theme.text_primary)),
                );
                ui.add_space(4.0);

                let temp_custom_path =
                    if self.font_family == "custom" && !self.custom_font_path.trim().is_empty() {
                        let trimmed = self.custom_font_path.trim();
                        let expanded = if trimmed.starts_with('~') {
                            crate::app::path_completion::expand_path(trimmed)
                        } else {
                            std::path::PathBuf::from(trimmed)
                        };
                        if expanded.exists() {
                            Some(expanded)
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                let temp_typo = crate::app::global_config::TypographyConfig {
                    font_family: self.font_family.clone(),
                    base_font_size: self.base_font_size,
                    line_height_multiplier: self.line_height_multiplier,
                    custom_font_path: temp_custom_path,
                };

                let preview_frame = egui::Frame::new()
                    .fill(color(
                        crate::theme::Color::from_hex("#1c2028").unwrap_or(theme.bg_primary),
                    ))
                    .stroke(egui::Stroke::new(1.0, color(theme.border)))
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::same(12));

                preview_frame.show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    let (h1_font, _) = resolve_font_and_line_height("# Заголовок", &temp_typo);
                    ui.label(
                        egui::RichText::new("# Заголовок")
                            .font(h1_font)
                            .color(color(theme.text_primary)),
                    );

                    let (code_font, _) =
                        resolve_font_and_line_height("```fn hello() -> &str", &temp_typo);
                    ui.label(
                        egui::RichText::new("fn hello() -> &str")
                            .font(code_font)
                            .color(color(theme.accent)),
                    );

                    let (body_font, _) =
                        resolve_font_and_line_height("Основной текст заметки", &temp_typo);
                    ui.label(
                        egui::RichText::new("Основной текст заметки")
                            .font(body_font)
                            .color(color(theme.text_primary)),
                    );
                });

                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("Тема оформления")
                        .size(12.0)
                        .strong()
                        .color(color(theme.text_primary)),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(self.active_theme == "nord", "nord")
                        .clicked()
                    {
                        self.active_theme = "nord".into();
                    }
                    if ui
                        .selectable_label(
                            self.active_theme == "dark" || self.active_theme == "obsidian-dark",
                            "dark",
                        )
                        .clicked()
                    {
                        self.active_theme = "dark".into();
                    }
                    if ui
                        .selectable_label(self.active_theme == "light", "light")
                        .clicked()
                    {
                        self.active_theme = "light".into();
                    }
                });

                if let Some(err) = &self.error {
                    ui.add_space(6.0);
                    ui.colored_label(egui::Color32::from_rgb(255, 100, 100), err);
                }

                ui.add_space(16.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if widgets::primary_button(ui, "Применить и сохранить").clicked()
                    {
                        submit_settings = true;
                    }
                    if widgets::secondary_button(ui, "Отмена").clicked() {
                        self.error = None;
                        close_dialog = true;
                    }
                });

                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.error = None;
                    close_dialog = true;
                }
            });

        if submit_settings {
            let custom_path =
                if self.font_family == "custom" && !self.custom_font_path.trim().is_empty() {
                    let trimmed = self.custom_font_path.trim();
                    let expanded = if trimmed.starts_with('~') {
                        crate::app::path_completion::expand_path(trimmed)
                    } else {
                        std::path::PathBuf::from(trimmed)
                    };
                    if !expanded.exists() {
                        self.error = Some(format!("Файл шрифта не найден: {}", trimmed));
                        return;
                    }
                    Some(expanded)
                } else {
                    None
                };

            let new_typography = crate::app::global_config::TypographyConfig {
                font_family: self.font_family.clone(),
                base_font_size: self.base_font_size,
                line_height_multiplier: self.line_height_multiplier,
                custom_font_path: custom_path,
            };

            let mut config = crate::app::global_config::GlobalConfig::load();
            config.typography = new_typography.clone();
            let theme_changed = config.theme != self.active_theme;
            config.theme = self.active_theme.clone();
            if let Err(e) = config.save() {
                self.error = Some(format!("Ошибка сохранения: {}", e));
                return;
            }

            application.shell.set_typography(new_typography);
            super::super::configure_fonts(context, application.shell.typography());
            if theme_changed {
                let new_theme = config.resolve_theme();
                application.shell.set_theme(new_theme);
            }

            self.error = None;
            close_dialog = true;
        }

        if close_dialog {
            self.open = false;
        }
    }
}
