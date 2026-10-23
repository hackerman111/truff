//! Procedural vector icons and canvas drawing primitives.
use egui::{Color32, Painter, Pos2, Rect, Stroke, vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    File,
    Folder,
    FolderOpen,
    ChevronRight,
    ChevronDown,
    Plus,
    Close,
    Vault,
    MoreHorizontal,
    Gear,
    Search,
    Command,
    Hash,
    CheckSquare,
    Square,
    Link,
    Pin,
    Star,
    Warning,
    Check,
}

/// Renders a crisp 5.5 px diameter dirty indicator circle without font dependencies.
pub fn draw_dirty_dot(painter: &Painter, center: Pos2, color: Color32, radius: f32) {
    painter.circle_filled(center, radius, color);
}

/// Renders a procedural icon centered inside `rect` with clean strokes.
pub fn render_icon(painter: &Painter, icon: Icon, rect: Rect, color: Color32, stroke_width: f32) {
    let stroke = Stroke::new(stroke_width, color);
    let center = rect.center();
    let s = 14.0;
    let b = Rect::from_center_size(center, vec2(s, s));

    match icon {
        Icon::File => {
            let x0 = b.left() + 2.0;
            let x1 = b.right() - 2.0;
            let y0 = b.top() + 0.5;
            let y1 = b.bottom() - 0.5;
            let fold = 3.5;

            painter.line_segment([Pos2::new(x0, y0), Pos2::new(x1 - fold, y0)], stroke);
            painter.line_segment([Pos2::new(x0, y0), Pos2::new(x0, y1)], stroke);
            painter.line_segment([Pos2::new(x0, y1), Pos2::new(x1, y1)], stroke);
            painter.line_segment([Pos2::new(x1, y1), Pos2::new(x1, y0 + fold)], stroke);
            painter.line_segment(
                [Pos2::new(x1 - fold, y0), Pos2::new(x1 - fold, y0 + fold)],
                stroke,
            );
            painter.line_segment(
                [Pos2::new(x1 - fold, y0 + fold), Pos2::new(x1, y0 + fold)],
                stroke,
            );
        }
        Icon::Folder => {
            let x0 = b.left() + 1.0;
            let x1 = b.right() - 1.0;
            let y0 = b.top() + 2.0;
            let y1 = b.bottom() - 1.0;
            let tab_w = 4.5;
            let tab_h = 2.0;

            painter.line_segment([Pos2::new(x0, y0), Pos2::new(x0 + tab_w, y0)], stroke);
            painter.line_segment(
                [
                    Pos2::new(x0 + tab_w, y0),
                    Pos2::new(x0 + tab_w + 1.5, y0 + tab_h),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(x0 + tab_w + 1.5, y0 + tab_h),
                    Pos2::new(x1, y0 + tab_h),
                ],
                stroke,
            );
            painter.line_segment([Pos2::new(x0, y0), Pos2::new(x0, y1)], stroke);
            painter.line_segment([Pos2::new(x0, y1), Pos2::new(x1, y1)], stroke);
            painter.line_segment([Pos2::new(x1, y0 + tab_h), Pos2::new(x1, y1)], stroke);
        }
        Icon::FolderOpen => {
            let x0 = b.left() + 1.0;
            let x1 = b.right() - 1.5;
            let y0 = b.top() + 2.0;
            let y1 = b.bottom() - 1.0;
            let tab_w = 4.2;
            let tab_h = 2.0;
            let flap_y = y0 + tab_h + 2.5;

            painter.line_segment([Pos2::new(x0, y0), Pos2::new(x0 + tab_w, y0)], stroke);
            painter.line_segment(
                [
                    Pos2::new(x0 + tab_w, y0),
                    Pos2::new(x0 + tab_w + 1.4, y0 + tab_h),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(x0 + tab_w + 1.4, y0 + tab_h),
                    Pos2::new(x1 - 1.0, y0 + tab_h),
                ],
                stroke,
            );
            painter.line_segment([Pos2::new(x0, y0), Pos2::new(x0, y1)], stroke);
            painter.line_segment([Pos2::new(x0, y1), Pos2::new(x1 - 1.0, y1)], stroke);
            painter.line_segment([Pos2::new(x0, y1), Pos2::new(x0 + 1.8, flap_y)], stroke);
            painter.line_segment(
                [Pos2::new(x0 + 1.8, flap_y), Pos2::new(x1 + 0.5, flap_y)],
                stroke,
            );
            painter.line_segment(
                [Pos2::new(x1 + 0.5, flap_y), Pos2::new(x1 - 1.0, y1)],
                stroke,
            );
        }
        Icon::ChevronRight => {
            let x_left = center.x - 2.2;
            let x_right = center.x + 2.2;
            painter.line_segment(
                [
                    Pos2::new(x_left, center.y - 4.0),
                    Pos2::new(x_right, center.y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(x_right, center.y),
                    Pos2::new(x_left, center.y + 4.0),
                ],
                stroke,
            );
        }
        Icon::ChevronDown => {
            let y_top = center.y - 2.2;
            let y_bot = center.y + 2.2;
            painter.line_segment(
                [Pos2::new(center.x - 4.0, y_top), Pos2::new(center.x, y_bot)],
                stroke,
            );
            painter.line_segment(
                [Pos2::new(center.x, y_bot), Pos2::new(center.x + 4.0, y_top)],
                stroke,
            );
        }
        Icon::Plus => {
            let arm = 4.5;
            painter.line_segment(
                [
                    Pos2::new(center.x - arm, center.y),
                    Pos2::new(center.x + arm, center.y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(center.x, center.y - arm),
                    Pos2::new(center.x, center.y + arm),
                ],
                stroke,
            );
        }
        Icon::Close => {
            let arm = 3.8;
            painter.line_segment(
                [
                    Pos2::new(center.x - arm, center.y - arm),
                    Pos2::new(center.x + arm, center.y + arm),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(center.x - arm, center.y + arm),
                    Pos2::new(center.x + arm, center.y - arm),
                ],
                stroke,
            );
        }
        Icon::Vault => {
            let x0 = b.left() + 1.5;
            let x1 = b.right() - 1.5;
            let y0 = b.top() + 1.0;
            let y1 = b.bottom() - 1.0;
            painter.line_segment([Pos2::new(x0, y0), Pos2::new(x1, y0)], stroke);
            painter.line_segment([Pos2::new(x0, y1), Pos2::new(x1, y1)], stroke);
            painter.line_segment([Pos2::new(x0, y0), Pos2::new(x0, y1)], stroke);
            painter.line_segment([Pos2::new(x1, y0), Pos2::new(x1, y1)], stroke);
            let y_mid = center.y;
            painter.line_segment([Pos2::new(x0, y_mid), Pos2::new(x1, y_mid)], stroke);
            painter.circle_filled(Pos2::new(center.x, (y0 + y_mid) / 2.0), 1.0, color);
            painter.circle_filled(Pos2::new(center.x, (y_mid + y1) / 2.0), 1.0, color);
        }
        Icon::MoreHorizontal => {
            let r = 1.25;
            painter.circle_filled(Pos2::new(center.x - 4.5, center.y), r, color);
            painter.circle_filled(Pos2::new(center.x, center.y), r, color);
            painter.circle_filled(Pos2::new(center.x + 4.5, center.y), r, color);
        }
        Icon::Gear => {
            painter.circle_stroke(center, 2.8, stroke);
            for i in 0..8 {
                let angle = (i as f32) * std::f32::consts::PI / 4.0;
                let dir = vec2(angle.cos(), angle.sin());
                let p1 = center + dir * 3.5;
                let p2 = center + dir * 5.8;
                painter.line_segment([p1, p2], stroke);
            }
        }
        Icon::Search => {
            let r = 3.5;
            let c = Pos2::new(center.x - 1.5, center.y - 1.5);
            painter.circle_stroke(c, r, stroke);
            painter.line_segment(
                [
                    Pos2::new(c.x + 2.5, c.y + 2.5),
                    Pos2::new(center.x + 4.5, center.y + 4.5),
                ],
                stroke,
            );
        }
        Icon::Command => {
            // Terminal / command prompt chevron `>_`
            let x0 = center.x - 4.5;
            let xm = center.x - 0.5;
            painter.line_segment(
                [Pos2::new(x0, center.y - 3.5), Pos2::new(xm, center.y)],
                stroke,
            );
            painter.line_segment(
                [Pos2::new(xm, center.y), Pos2::new(x0, center.y + 3.5)],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(center.x + 0.5, center.y + 3.5),
                    Pos2::new(center.x + 4.5, center.y + 3.5),
                ],
                stroke,
            );
        }
        Icon::Hash => {
            let d = 2.2;
            let ext = 4.5;
            painter.line_segment(
                [
                    Pos2::new(center.x - d + 0.5, center.y - ext),
                    Pos2::new(center.x - d - 0.5, center.y + ext),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(center.x + d + 0.5, center.y - ext),
                    Pos2::new(center.x + d - 0.5, center.y + ext),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(center.x - ext, center.y - d),
                    Pos2::new(center.x + ext, center.y - d),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(center.x - ext, center.y + d),
                    Pos2::new(center.x + ext, center.y + d),
                ],
                stroke,
            );
        }
        Icon::Square => {
            let box_rect = Rect::from_center_size(center, vec2(10.0, 10.0));
            painter.rect_stroke(box_rect, 2.0, stroke, egui::StrokeKind::Inside);
        }
        Icon::CheckSquare => {
            let box_rect = Rect::from_center_size(center, vec2(10.0, 10.0));
            painter.rect_stroke(box_rect, 2.0, stroke, egui::StrokeKind::Inside);
            painter.line_segment(
                [
                    Pos2::new(center.x - 2.6, center.y + 0.2),
                    Pos2::new(center.x - 0.6, center.y + 2.4),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(center.x - 0.6, center.y + 2.4),
                    Pos2::new(center.x + 3.0, center.y - 2.2),
                ],
                stroke,
            );
        }
        Icon::Link => {
            let p0 = Pos2::new(center.x - 3.5, center.y + 3.5);
            let p1 = Pos2::new(center.x + 3.5, center.y - 3.5);
            painter.line_segment([p0, p1], stroke);
            painter.line_segment([Pos2::new(p1.x - 4.0, p1.y), p1], stroke);
            painter.line_segment([Pos2::new(p1.x, p1.y + 4.0), p1], stroke);
        }
        Icon::Pin => {
            painter.circle_stroke(Pos2::new(center.x, center.y - 2.2), 2.5, stroke);
            painter.line_segment(
                [
                    Pos2::new(center.x, center.y + 0.3),
                    Pos2::new(center.x, center.y + 5.0),
                ],
                stroke,
            );
        }
        Icon::Star => {
            let r_outer = 5.0;
            let r_inner = 2.2;
            let mut pts = [Pos2::ZERO; 8];
            for (i, pt) in pts.iter_mut().enumerate() {
                let angle = (i as f32) * std::f32::consts::PI / 4.0 - std::f32::consts::FRAC_PI_2;
                let r = if i % 2 == 0 { r_outer } else { r_inner };
                *pt = center + vec2(angle.cos() * r, angle.sin() * r);
            }
            for i in 0..8 {
                painter.line_segment([pts[i], pts[(i + 1) % 8]], stroke);
            }
        }
        Icon::Warning => {
            let top = Pos2::new(center.x, center.y - 4.8);
            let bl = Pos2::new(center.x - 4.8, center.y + 4.2);
            let br = Pos2::new(center.x + 4.8, center.y + 4.2);
            painter.line_segment([top, bl], stroke);
            painter.line_segment([bl, br], stroke);
            painter.line_segment([br, top], stroke);
            painter.line_segment(
                [
                    Pos2::new(center.x, center.y - 1.5),
                    Pos2::new(center.x, center.y + 1.2),
                ],
                stroke,
            );
            painter.circle_filled(Pos2::new(center.x, center.y + 2.8), 0.9, color);
        }
        Icon::Check => {
            painter.line_segment(
                [
                    Pos2::new(center.x - 4.0, center.y + 0.2),
                    Pos2::new(center.x - 1.0, center.y + 3.2),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(center.x - 1.0, center.y + 3.2),
                    Pos2::new(center.x + 4.2, center.y - 3.0),
                ],
                stroke,
            );
        }
    }
}
