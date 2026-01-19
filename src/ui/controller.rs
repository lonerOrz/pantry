use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{ApplicationWindow, EventControllerKey, Label, ListBox, ListBoxRow};
use std::cell::RefCell;
use std::rc::Rc;

use crate::app::item_object::ItemObject;
use crate::ui::preview::PreviewArea;

/// 键盘控制器
pub struct KeyboardController {
    window: ApplicationWindow,
    listbox: ListBox,
    search_state: crate::ui::search::SearchState,
    search_label: Label,
    preview_enabled: bool,
    preview_area: Option<Rc<RefCell<PreviewArea>>>,
}

impl KeyboardController {
    pub fn new(
        window: &ApplicationWindow,
        listbox: &ListBox,
        search_state: crate::ui::search::SearchState,
        search_label: &Label,
        preview_enabled: bool,
        preview_area: Option<Rc<RefCell<PreviewArea>>>,
    ) -> Self {
        KeyboardController {
            window: window.clone(),
            listbox: listbox.clone(),
            search_state,
            search_label: search_label.clone(),
            preview_enabled,
            preview_area,
        }
    }

    /// 设置并返回控制器
    pub fn setup(self) -> EventControllerKey {
        let controller = EventControllerKey::new();
        controller.set_propagation_phase(gtk4::PropagationPhase::Capture);

        let listbox = self.listbox.clone();
        let search_label = self.search_label.clone();
        let preview_enabled = self.preview_enabled;
        let preview_area_rc = self.preview_area;
        let query_state = self.search_state.clone();

        controller.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == gtk4::gdk::Key::Return || keyval == gtk4::gdk::Key::KP_Enter {
                Self::handle_selection(&listbox);
                return glib::Propagation::Stop;
            }
            if keyval == gtk4::gdk::Key::Escape {
                Self::clear_search(&query_state, &listbox, &search_label, &preview_area_rc);
                return glib::Propagation::Stop;
            }

            // 如果在图片模式，处理选择变化以更新预览
            if preview_enabled
                && (keyval == gtk4::gdk::Key::Down
                    || keyval == gtk4::gdk::Key::Up
                    || keyval == gtk4::gdk::Key::Tab
                    || keyval == gtk4::gdk::Key::ISO_Left_Tab)
            {
                // 延迟预览更新，等待选择更新完成
                let listbox_clone = listbox.clone();
                let preview_area_rc_clone = preview_area_rc.clone();
                glib::timeout_add_local(
                    std::time::Duration::from_millis(crate::constants::SELECTION_UPDATE_DELAY_MS),
                    move || {
                        Self::update_preview(&listbox_clone, &preview_area_rc_clone);
                        glib::ControlFlow::Break
                    },
                );
            }

            Self::handle_search_input(
                keyval,
                &query_state,
                &listbox,
                &search_label,
                &preview_area_rc,
            )
        });

        controller
    }

    /// 处理搜索输入
    fn handle_search_input(
        keyval: gtk4::gdk::Key,
        query_state: &crate::ui::search::SearchState,
        listbox: &ListBox,
        label: &Label,
        preview_area_rc_opt: &Option<Rc<RefCell<PreviewArea>>>,
    ) -> glib::Propagation {
        let (should_invalidate, current_text) = {
            let mut query = query_state.borrow_mut();
            let mut updated = false;
            if keyval == gtk4::gdk::Key::BackSpace {
                query.pop();
                updated = true;
            } else if let Some(c) = keyval.to_unicode() {
                if c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | '@') {
                    query.push(c);
                    updated = true;
                }
            }
            (updated, query.clone())
        };
        if should_invalidate {
            if current_text.is_empty() {
                label.add_css_class("hidden");
            } else {
                label.set_text(&format!("Search: {}", current_text));
                label.remove_css_class("hidden");
            }
            listbox.invalidate_filter();
            Self::update_selection_after_filter(listbox, preview_area_rc_opt);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    }

    /// 清除搜索
    fn clear_search(
        query_state: &crate::ui::search::SearchState,
        listbox: &ListBox,
        label: &Label,
        preview_area_rc_opt: &Option<Rc<RefCell<PreviewArea>>>,
    ) {
        query_state.borrow_mut().clear();
        label.add_css_class("hidden");
        listbox.invalidate_filter();
        Self::update_selection_after_filter(listbox, preview_area_rc_opt);
    }
}

impl KeyboardController {
    /// 处理用户选择
    fn handle_selection(listbox: &ListBox) {
        if let Some(selected_row) = listbox.selected_row() {
            if let Some(item_obj_ptr) = unsafe { selected_row.data::<ItemObject>("item") } {
                let item_obj = unsafe { &*item_obj_ptr.as_ptr() };
                if let Some(item) = item_obj.item() {
                    print!("{}", item.value);

                    use std::io::{self, Write};
                    let _ = io::stdout().flush();

                    if let Some(window) = listbox.root().and_downcast::<ApplicationWindow>() {
                        Self::save_current_window_state(&window);
                        window.close();
                    }
                }
            }
        }
    }

    /// 保存当前窗口状态
    fn save_current_window_state(window: &ApplicationWindow) {
        let maximized = window.is_maximized();
        let (width, height) = window.default_size();
        let state = crate::window_state::WindowState {
            width,
            height,
            maximized,
        };
        state.save();
    }

    /// 更新选择后的过滤
    fn update_selection_after_filter(
        listbox: &ListBox,
        preview_area_rc_opt: &Option<Rc<RefCell<PreviewArea>>>,
    ) {
        let mut needs_reselect = true;
        if let Some(selected) = listbox.selected_row() {
            if selected.is_child_visible() {
                selected.grab_focus();
                needs_reselect = false;
            }
        }
        if needs_reselect {
            if let Some(row) = Self::first_visible_row_after_filter(listbox) {
                listbox.select_row(Some(&row));
                row.grab_focus();
            } else {
                listbox.select_row(None::<&ListBoxRow>);
            }
        }
        // 触发预览更新
        Self::update_preview(listbox, preview_area_rc_opt);
    }

    /// 获取过滤后的第一个可见行
    fn first_visible_row_after_filter(listbox: &ListBox) -> Option<ListBoxRow> {
        let mut i = 0;
        while let Some(row) = listbox.row_at_index(i) {
            if row.is_child_visible() {
                return Some(row);
            }
            i += 1;
        }
        None
    }

    /// 更新预览
    pub fn update_preview(
        listbox: &ListBox,
        preview_area_rc_opt: &Option<Rc<RefCell<PreviewArea>>>,
    ) {
        use std::sync::{Mutex, OnceLock};
        use std::time::{SystemTime, UNIX_EPOCH};

        static LAST_PREVIEW_UPDATE: OnceLock<Mutex<u128>> = OnceLock::new();

        let mutex = LAST_PREVIEW_UPDATE.get_or_init(|| Mutex::new(0));

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);

        {
            let mut last_update = mutex.lock().unwrap();
            if now - *last_update < crate::constants::PREVIEW_UPDATE_THROTTLE_MS as u128 {
                return;
            }
            *last_update = now;
        }

        if let Some(preview_area_rc) = preview_area_rc_opt {
            if let Some(selected_row) = listbox.selected_row() {
                if let Some(item_obj_ptr) = unsafe { selected_row.data::<ItemObject>("item") } {
                    let item_obj = unsafe { &*item_obj_ptr.as_ptr() };
                    if let Some(item) = item_obj.item() {
                        if matches!(item.display, crate::config::DisplayMode::Picture) {
                            let preview_area = &*preview_area_rc.borrow();
                            preview_area.update_with_content(&item);
                        } else {
                            let preview_area = &*preview_area_rc.borrow();
                            preview_area.clear();
                        }
                    }
                }
            }
        }
    }
}
