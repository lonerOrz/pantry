use gtk4::prelude::ObjectExt;
use gtk4::ListBox;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct PreviewManager;

impl PreviewManager {
    pub fn update_preview(
        listbox: &ListBox,
        preview_area_rc_opt: &Option<Rc<RefCell<crate::ui::preview::PreviewArea>>>,
    ) {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::sync::OnceLock;

        static LAST_UPDATE_TIME: OnceLock<AtomicU64> = OnceLock::new();
        let last_update = LAST_UPDATE_TIME.get_or_init(|| AtomicU64::new(0));

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let prev_time = last_update.load(Ordering::Relaxed);
        // Skip throttling for initial update (when prev_time is 0) or if enough time has passed
        if prev_time != 0
            && now.saturating_sub(prev_time) < crate::constants::PREVIEW_UPDATE_THROTTLE_MS
        {
            return;
        }

        // Attempt to update the timestamp atomically
        if !last_update
            .compare_exchange(prev_time, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            // Another thread updated the time, skip this update
            return;
        }

        if let Some(preview_area_rc) = preview_area_rc_opt {
            if let Some(selected_row) = listbox.selected_row() {
                if let Some(item_obj_ptr) =
                    unsafe { selected_row.data::<crate::app::item_object::ItemObject>("item") }
                {
                    let item_obj = unsafe { &*item_obj_ptr.as_ptr() };
                    if let Some(item) = item_obj.item() {
                        // Handle dynamic source differently
                        if matches!(item.source, crate::config::SourceMode::Dynamic) {
                            // For dynamic source, we need to execute the preview command
                            // The item.value contains the ID to use in the command
                            // The command template would have been stored somewhere
                            // For now, we'll treat it as a regular item
                            let preview_area = &*preview_area_rc.borrow();
                            preview_area.update_with_content(&item);
                        } else {
                            let preview_area = &*preview_area_rc.borrow();
                            preview_area.update_with_content(&item);
                        }
                    }
                }
            }
        }
    }
}
