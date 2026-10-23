use std::ops::Range;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Alignment {
    Nearest,
    Start,
    End,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct EnsureVisible {
    pub item_index: usize,
    pub alignment: Alignment,
}

/// Selection and viewport changes trigger following; idle frames preserve wheel input.
#[derive(Clone, Default, Debug)]
pub(crate) struct RowScroll {
    pub offset: f32,
    pub viewport_height: f32,
    pub visible_range: Range<usize>,
    selection: Option<usize>,
    count: usize,
    stride: f32,
    request: Option<EnsureVisible>,
}

impl RowScroll {
    pub fn request(&mut self, item_index: usize, alignment: Alignment) {
        self.request = Some(EnsureVisible {
            item_index,
            alignment,
        });
    }

    pub fn page_rows(&self) -> usize {
        (self.viewport_height / self.stride.max(1.0))
            .floor()
            .max(1.0) as usize
    }

    pub fn prepare(
        &mut self,
        selected: usize,
        count: usize,
        stride: f32,
        height: f32,
    ) -> Option<f32> {
        let changed = self.selection != Some(selected)
            || self.count != count
            || self.stride != stride
            || (self.viewport_height - height).abs() > 1.0;
        self.selection = Some(selected);
        self.count = count;
        self.stride = stride;
        self.viewport_height = height;
        let request = self.request.take().or_else(|| {
            changed.then_some(EnsureVisible {
                item_index: selected,
                alignment: Alignment::Nearest,
            })
        })?;
        let index = request.item_index.min(count.saturating_sub(1));
        let top = index as f32 * stride;
        let bottom = top + stride;
        let margin = stride.min((height - stride).max(0.0) / 2.0);
        let target = match request.alignment {
            Alignment::Start => top,
            Alignment::End => bottom - height,
            Alignment::Nearest if top < self.offset + margin => top - margin,
            Alignment::Nearest if bottom > self.offset + height - margin => {
                bottom + margin - height
            }
            Alignment::Nearest => self.offset,
        };
        self.offset = target.clamp(0.0, (count as f32 * stride - height).max(0.0));
        Some(self.offset)
    }

    pub fn observe<R>(&mut self, output: &egui::scroll_area::ScrollAreaOutput<R>) {
        self.offset = output.state.offset.y;
        self.viewport_height = output.inner_rect.height();
        self.visible_range = (self.offset / self.stride.max(1.0)).floor() as usize
            ..((self.offset + self.viewport_height) / self.stride.max(1.0)).ceil() as usize;
        self.visible_range.end = self.visible_range.end.min(self.count);
    }
}

/// Adapter for dock lists whose model is owned by Core or Application.
pub(crate) fn selected_rows<R>(
    ui: &mut egui::Ui,
    salt: &'static str,
    row_height: f32,
    count: usize,
    selected: usize,
    render: impl FnOnce(&mut egui::Ui, Range<usize>) -> R,
) -> egui::scroll_area::ScrollAreaOutput<R> {
    let id = ui.make_persistent_id(salt);
    let mut scroll = ui
        .ctx()
        .data_mut(|data| data.get_temp::<RowScroll>(id))
        .unwrap_or_default();
    let mut area = egui::ScrollArea::vertical().id_salt(salt);
    if let Some(offset) = scroll.prepare(
        selected,
        count,
        row_height + ui.spacing().item_spacing.y,
        ui.available_height(),
    ) {
        area = area.vertical_scroll_offset(offset);
    }
    let output = area.show_rows(ui, row_height, count, render);
    scroll.observe(&output);
    ui.ctx().data_mut(|data| data.insert_temp(id, scroll));
    output
}

#[cfg(test)]
mod tests {
    use super::{Alignment, RowScroll};

    fn assert_visible(scroll: &RowScroll, index: usize, stride: f32, height: f32) {
        let top = index as f32 * stride;
        assert!(
            top >= scroll.offset && top + stride <= scroll.offset + height,
            "item {index}, offset {}, height {height}",
            scroll.offset
        );
    }

    #[test]
    fn thousand_rows_twelve_visible_follow_then_preserve_manual_scroll() {
        for stride in [24.0, 40.0, 64.0] {
            let height = 12.0 * stride;
            let mut scroll = RowScroll::default();
            for index in 0..=100 {
                assert!(scroll.prepare(index, 1000, stride, height).is_some());
                assert_visible(&scroll, index, stride, height);
            }
            for (index, alignment) in [
                (999, Alignment::End),
                (0, Alignment::Start),
                (50, Alignment::Nearest),
                (49, Alignment::Nearest),
            ] {
                scroll.request(index, alignment);
                scroll.prepare(index, 1000, stride, height);
                assert_visible(&scroll, index, stride, height);
            }
            scroll.offset += height * 3.0;
            let manual = scroll.offset;
            assert_eq!(scroll.prepare(49, 1000, stride, height), None);
            assert_eq!(scroll.offset, manual);
            scroll.prepare(49, 1000, stride, height / 2.0);
            assert_visible(&scroll, 49, stride, height / 2.0);
            scroll.prepare(2, 3, stride, height);
            assert_eq!(scroll.offset, 0.0);
            scroll.prepare(0, 0, stride, height);
            assert_eq!(scroll.offset, 0.0);
        }
    }
}
