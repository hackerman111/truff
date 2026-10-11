//! Procedural vector icons and canvas drawing primitives.
use eframe::egui::{Color32, Painter, Pos2, Rect, Stroke, vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    File,
    Folder,
    ChevronRight,
    ChevronDown,
    Plus,
    Close,
    Vault,
    MoreHorizontal,
    Gear,
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
            let y0 = b.top();
            let y1 = b.bottom();
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
        Icon::ChevronRight => {
            let x_left = center.x - 2.5;
            let x_right = center.x + 2.5;
            painter.line_segment(
                [
                    Pos2::new(x_left, center.y - 4.5),
                    Pos2::new(x_right, center.y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(x_right, center.y),
                    Pos2::new(x_left, center.y + 4.5),
                ],
                stroke,
            );
        }
        Icon::ChevronDown => {
            let y_top = center.y - 2.5;
            let y_bot = center.y + 2.5;
            painter.line_segment(
                [Pos2::new(center.x - 4.5, y_top), Pos2::new(center.x, y_bot)],
                stroke,
            );
            painter.line_segment(
                [Pos2::new(center.x, y_bot), Pos2::new(center.x + 4.5, y_top)],
                stroke,
            );
        }
        Icon::Plus => {
            let arm = 5.0;
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
            let arm = 4.0;
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
    }
}
