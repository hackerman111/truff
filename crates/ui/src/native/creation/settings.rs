use super::super::{editor::color, icons, resolve_font_and_line_height, widgets};
use crate::app::Application;
use crate::components::follow_tab_focus;
use crate::theme::Theme;

pub struct SettingsDialog {
    pub open: bool,
    pub font_family: String,
    pub base_font_size: f32,
    pub line_height_multiplier: f32,
    pub show_inline_title: bool,
    pub max_content_width: f32,
    pub custom_font_path: String,
    pub active_theme: String,
    pub error: Option<String>,
}

impl Default for SettingsDialog {
    fn default() -> Self {
        Self {
            open: false,
            font_family: "proportional".into(),
            base_font_size: 15.0,
            line_height_multiplier: 1.55,
            show_inline_title: true,
            max_content_width: 720.0,
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
        self.show_inline_title = typography.show_inline_title;
        self.max_content_width = typography.max_content_width;
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
        let mut cancelled = false;

        crate::components::dialog_with_footer(
            context,
            theme,
            "settings_dialog",
            |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("Настройки")
                                .size(theme.geometry.ui_font_size + 3.0)
                                .strong()
                                .color(color(theme.text_primary)),
                        );
                        ui.label(
                            egui::RichText::new("Типографика редактора и внешний вид")
                                .size(theme.geometry.ui_font_size)
                                .color(color(theme.text_muted)),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if follow_tab_focus(
                            widgets::icon_button(ui, icons::Icon::Close, "Закрыть"),
                            ui,
                        )
                        .clicked()
                        {
                            close_dialog = true;
                        }
                    });
                });

                ui.add_space(14.0);
                ui.label(
                    egui::RichText::new("Семейство шрифта")
                        .size(theme.geometry.ui_font_size)
                        .strong()
                        .color(color(theme.text_primary)),
                );
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    let is_mono = self.font_family == "monospace";
                    let is_prop = self.font_family == "proportional";
                    let is_custom = self.font_family == "custom";

                    if follow_tab_focus(ui.selectable_label(is_mono, "Моноширинный"), ui).clicked()
                    {
                        self.font_family = "monospace".into();
                    }
                    if follow_tab_focus(
                        ui.selectable_label(is_prop, "Пропорциональный (по умолчанию)"),
                        ui,
                    )
                    .clicked()
                    {
                        self.font_family = "proportional".into();
                    }
                    if follow_tab_focus(ui.selectable_label(is_custom, "Пользовательский файл"), ui)
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
                                .size(theme.geometry.ui_font_size)
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
                        .size(theme.geometry.ui_font_size)
                        .strong()
                        .color(color(theme.text_primary)),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if follow_tab_focus(ui.button(" - "), ui).clicked()
                        && self.base_font_size > 10.0
                    {
                        self.base_font_size = (self.base_font_size - 1.0).max(10.0);
                    }
                    follow_tab_focus(
                        ui.add(
                            egui::Slider::new(&mut self.base_font_size, 10.0..=32.0)
                                .step_by(1.0)
                                .suffix(" px"),
                        ),
                        ui,
                    );
                    if follow_tab_focus(ui.button(" + "), ui).clicked()
                        && self.base_font_size < 32.0
                    {
                        self.base_font_size = (self.base_font_size + 1.0).min(32.0);
                    }
                    if follow_tab_focus(ui.button("Сброс (15px)"), ui).clicked() {
                        self.base_font_size = 15.0;
                    }
                });

                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("Предпросмотр")
                        .size(theme.geometry.ui_font_size)
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
                    show_inline_title: self.show_inline_title,
                    max_content_width: self.max_content_width,
                };

                let preview_frame = egui::Frame::new()
                    .fill(color(theme.bg_canvas))
                    .stroke(egui::Stroke::new(1.0, color(theme.border)))
                    .corner_radius(egui::CornerRadius::same(theme.geometry.corner_radius as u8))
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

                ui.add_space(10.0);
                follow_tab_focus(
                    ui.checkbox(
                        &mut self.show_inline_title,
                        "Показывать заголовок заметки над текстом",
                    ),
                    ui,
                );

                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new("Свободное пространство по бокам (ширина текста)")
                        .size(theme.geometry.ui_font_size)
                        .strong()
                        .color(color(theme.text_primary)),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    follow_tab_focus(
                        ui.add(
                            egui::Slider::new(&mut self.max_content_width, 500.0..=1600.0)
                                .step_by(20.0)
                                .text("px"),
                        ),
                        ui,
                    );
                });
                ui.label(
                    egui::RichText::new(
                        "Регулирует ширину текстовой колонки и свободные поля по краям редактора (по умолчанию 720 px)",
                    )
                    .size(theme.geometry.ui_font_size - 2.0)
                    .color(color(theme.text_muted)),
                );

                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("Тема оформления")
                        .size(theme.geometry.ui_font_size)
                        .strong()
                        .color(color(theme.text_primary)),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    for name in ["nord", "dark", "light"] {
                        let selected = self.active_theme == name
                            || name == "dark" && self.active_theme == "obsidian-dark";
                        if follow_tab_focus(ui.selectable_label(selected, name), ui).clicked() {
                            self.active_theme = name.into();
                        }
                    }
                });

                if let Some(err) = &self.error {
                    ui.add_space(6.0);
                    ui.colored_label(color(theme.state_error), err);
                }

                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.error = None;
                    close_dialog = true;
                }
            },
            |ui| {
                ui.add_space(16.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if widgets::primary_button(ui, "Применить и сохранить").clicked()
                    {
                        submit_settings = true;
                    }
                    if widgets::secondary_button(ui, "Отмена").clicked() {
                        cancelled = true;
                    }
                });
            },
        );
        if cancelled {
            self.error = None;
            close_dialog = true;
        }

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
                show_inline_title: self.show_inline_title,
                max_content_width: self.max_content_width,
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
