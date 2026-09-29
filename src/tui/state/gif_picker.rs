use super::{
    DashboardState,
    popups::{ModalPopup, SelectablePopupState},
};
use crate::config::KlipyOptions;
use crate::klipy::{Gif, GifPage, KlipyClient};
use crate::tui::keybindings::SelectionAction;
use crate::tui::text_input::{TextEditAction, TextInputState};
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Default)]
pub(super) struct KlipyState {
    pub options: KlipyOptions,
    generation: u64,
    selected: Vec<(String, String, String)>,
    shares: Vec<(String, String)>,
}

#[derive(Debug)]
pub(in crate::tui) struct GifPickerState {
    pub query: TextInputState,
    pub query_selected: bool,
    pub query_editing: bool,
    pub page: u32,
    pub results: Vec<Gif>,
    pub horizontal_scroll: usize,
    pub(super) selection: SelectablePopupState,
    pub has_next: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub generation: u64,
    pub preview_generation: u64,
    advance_on_load: bool,
}

impl GifPickerState {
    pub(in crate::tui) fn selected_result_index(&self) -> Option<usize> {
        (!self.query_selected && !self.results.is_empty())
            .then(|| self.selection.selected_for_len(self.results.len()))
    }

    pub(in crate::tui) fn selected_gif(&self) -> Option<&Gif> {
        self.results.get(self.selected_result_index()?)
    }
}

impl DashboardState {
    pub(in crate::tui) fn apply_klipy_options(&mut self, options: KlipyOptions) {
        self.klipy.options = options;
    }

    pub(in crate::tui) fn open_gif_picker(&mut self) {
        if !self.is_composing() || self.composer.edit_target_message.is_some() {
            return;
        }
        if let Err(error) = KlipyClient::new(&self.klipy.options) {
            self.show_error_toast(error, std::time::Instant::now());
            return;
        }
        self.cancel_active_composer_picker();
        self.klipy.generation = self.klipy.generation.wrapping_add(1);
        self.popups.set_modal(ModalPopup::GifPicker(GifPickerState {
            query: TextInputState::default(),
            query_selected: true,
            query_editing: false,
            page: 1,
            results: Vec::new(),
            horizontal_scroll: 0,
            selection: SelectablePopupState::default(),
            has_next: false,
            loading: true,
            error: None,
            generation: self.klipy.generation,
            preview_generation: self.klipy.generation,
            advance_on_load: false,
        }));
    }

    pub(in crate::tui) fn gif_picker(&self) -> Option<&GifPickerState> {
        match self.popups.modal.as_ref()? {
            ModalPopup::GifPicker(picker) => Some(picker),
            _ => None,
        }
    }

    pub(in crate::tui::state) fn gif_picker_mut(&mut self) -> Option<&mut GifPickerState> {
        match self.popups.modal.as_mut()? {
            ModalPopup::GifPicker(picker) => Some(picker),
            _ => None,
        }
    }

    fn refresh_gif_query(&mut self) {
        self.klipy.generation = self.klipy.generation.wrapping_add(1);
        let generation = self.klipy.generation;
        if let Some(picker) = self.gif_picker_mut() {
            picker.page = 1;
            picker.results.clear();
            picker.horizontal_scroll = 0;
            picker.selection = SelectablePopupState::default();
            picker.query_selected = true;
            picker.has_next = false;
            picker.loading = true;
            picker.error = None;
            picker.generation = generation;
            picker.preview_generation = generation;
            picker.advance_on_load = false;
        }
    }

    fn request_gif_page(&mut self, page: u32, advance_on_load: bool) {
        self.klipy.generation = self.klipy.generation.wrapping_add(1);
        let generation = self.klipy.generation;
        if let Some(picker) = self.gif_picker_mut() {
            picker.page = page;
            picker.loading = true;
            picker.error = None;
            picker.generation = generation;
            picker.advance_on_load = advance_on_load;
        }
    }

    pub(in crate::tui) fn is_gif_query_editing(&self) -> bool {
        self.gif_picker().is_some_and(|picker| picker.query_editing)
    }

    pub(in crate::tui) fn select_gif_query(&mut self) {
        if let Some(picker) = self.gif_picker_mut() {
            picker.query_selected = true;
            picker.query_editing = true;
            picker.advance_on_load = false;
        }
    }

    pub(in crate::tui) fn stop_gif_query_editing(&mut self) {
        if let Some(picker) = self.gif_picker_mut() {
            picker.query_editing = false;
        }
    }

    pub(in crate::tui) fn focus_gif_result(&mut self) {
        if let Some(picker) = self.gif_picker_mut() {
            picker.query_selected = false;
            picker.query_editing = false;
            picker.advance_on_load = false;
        }
    }

    pub(in crate::tui) fn insert_gif_query(&mut self, text: &str) {
        if let Some(picker) = self.gif_picker_mut().filter(|picker| picker.query_editing) {
            let text: String = text
                .chars()
                .filter(|c| !c.is_control())
                .take(256usize.saturating_sub(picker.query.value().chars().count()))
                .collect();
            if text.is_empty() {
                return;
            }
            picker.query.insert_str(&text);
            self.refresh_gif_query();
        }
    }

    pub(in crate::tui) fn edit_gif_query(&mut self, action: TextEditAction) {
        if self
            .gif_picker_mut()
            .filter(|picker| picker.query_editing)
            .is_some_and(|p| p.query.apply_edit_action(action))
        {
            self.refresh_gif_query();
        }
    }

    pub(in crate::tui) fn clear_gif_query(&mut self) {
        if let Some(picker) = self.gif_picker_mut().filter(|picker| picker.query_editing) {
            picker.query.clear();
            self.refresh_gif_query();
        }
    }

    pub(in crate::tui) fn move_gif_selection(&mut self, action: SelectionAction) {
        let Some(picker) = self.gif_picker_mut() else {
            return;
        };
        match action {
            SelectionAction::Next if picker.query_selected => {
                if !picker.results.is_empty() {
                    picker.query_selected = false;
                    picker.query_editing = false;
                }
            }
            SelectionAction::Next if !picker.results.is_empty() => {
                let at_end = picker.selection.selected() >= picker.results.len() - 1;
                if at_end && picker.loading {
                    picker.advance_on_load = true;
                } else if at_end && picker.error.is_some() {
                    self.retry_gif_page();
                } else if at_end && picker.has_next {
                    let page = picker.page.saturating_add(1);
                    self.request_gif_page(page, true);
                } else {
                    picker.selection.move_down(picker.results.len());
                }
            }
            SelectionAction::Previous if !picker.query_selected => {
                if picker.selection.selected() == 0 {
                    picker.query_selected = true;
                } else {
                    picker.selection.move_up();
                }
                picker.advance_on_load = false;
            }
            SelectionAction::Next | SelectionAction::Previous => {}
        }
    }

    pub(in crate::tui) fn scroll_gif_titles(&mut self, delta: i8) {
        let Some(picker) = self
            .gif_picker_mut()
            .filter(|picker| !picker.query_selected)
        else {
            return;
        };
        let max = picker
            .results
            .iter()
            .map(|gif| gif.title.as_str().width().saturating_sub(1))
            .max()
            .unwrap_or_default();
        picker.horizontal_scroll = picker
            .horizontal_scroll
            .saturating_add_signed(isize::from(delta))
            .min(max);
    }

    pub(in crate::tui) fn page_gif_selection(&mut self, action: SelectionAction) {
        let Some(picker) = self.gif_picker_mut() else {
            return;
        };
        if picker.query_selected && action == SelectionAction::Next && !picker.results.is_empty() {
            picker.query_selected = false;
            picker.query_editing = false;
            return;
        }
        if picker.query_selected {
            return;
        }
        let before = picker.selection.selected();
        picker.selection.page(picker.results.len(), action);
        if action == SelectionAction::Next && before == picker.selection.selected() {
            self.move_gif_selection(action);
        }
    }

    pub(in crate::tui) fn retry_gif_page(&mut self) {
        if let Some(picker) = self
            .gif_picker()
            .filter(|picker| !picker.loading && picker.error.is_some())
        {
            self.request_gif_page(picker.page, false);
        }
    }

    pub(in crate::tui) fn store_gif_results(
        &mut self,
        generation: u64,
        result: Result<GifPage, String>,
    ) -> bool {
        let Some(picker) = self.gif_picker_mut().filter(|p| p.generation == generation) else {
            return false;
        };
        picker.loading = false;
        match result {
            Ok(page) => {
                let next_result = picker.results.len();
                if picker.page == 1 {
                    picker.results = page.data;
                } else {
                    picker.results.extend(page.data);
                }
                picker.has_next = page.has_next;
                picker.error = None;
                if picker.advance_on_load && picker.results.len() > next_result {
                    picker.selection.select(next_result);
                    picker.query_selected = false;
                }
            }
            Err(error) => picker.error = Some(error),
        }
        picker.advance_on_load = false;
        true
    }

    pub(in crate::tui) fn confirm_gif_selection(&mut self) {
        if !self.is_composing() || !self.can_send_in_selected_channel() {
            self.popups.clear_modal();
            return;
        }
        let Some(picker) = self.gif_picker() else {
            return;
        };
        let Some(gif) = picker.selected_gif() else {
            return;
        };
        let Some(url) = gif.media_url(false).map(str::to_owned) else {
            return;
        };
        let selection = (
            url.clone(),
            gif.slug.clone(),
            picker.query.value().to_owned(),
        );
        self.popups.clear_modal();
        // Append on its own line so links survive mid-word cursors and markdown drafts.
        self.move_composer_cursor_end();
        if !self.composer_input().is_empty() && !self.composer_input().ends_with('\n') {
            self.insert_composer_text_at_cursor("\n");
        }
        self.insert_composer_text_at_cursor(&url);
        self.klipy.selected.retain(|(selected, _, _)| {
            selected != &url && self.composer.composer_input.value().contains(selected)
        });
        self.klipy.selected.push(selection);
    }

    pub(in crate::tui::state) fn clear_klipy_selections(&mut self) {
        self.klipy.selected.clear();
    }

    pub(in crate::tui::state) fn queue_klipy_shares(&mut self, content: &str) {
        for (url, slug, query) in self.klipy.selected.drain(..) {
            if content.split_whitespace().any(|word| word == url) {
                self.klipy.shares.push((slug, query));
            }
        }
    }

    pub(in crate::tui) fn take_klipy_shares(&mut self) -> Vec<(String, String)> {
        std::mem::take(&mut self.klipy.shares)
    }
}
