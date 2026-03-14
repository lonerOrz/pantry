use gtk4::ListBox;
use std::cell::RefCell;
use std::process::Command;
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
        if prev_time != 0
            && now.saturating_sub(prev_time) < crate::constants::PREVIEW_UPDATE_THROTTLE_MS
        {
            return;
        }

        if last_update
            .compare_exchange(prev_time, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            return;
        }

        if let Some(preview_area_rc) = preview_area_rc_opt {
            if let Some(selected_row) = listbox.selected_row() {
                if let Some(item_obj) = crate::app::item_object::ItemObject::from_row(&selected_row)
                {
                    if let Some(item) = item_obj.item() {
                        // Use preview_template if available (dynamic source)
                        if let Some(ref template) = item.preview_template {
                            let preview_content = execute_preview_command(template, &item.value);
                            let mut display_item = item.clone();
                            display_item.value = preview_content;
                            let preview_area = &*preview_area_rc.borrow();
                            preview_area.update_with_content(&display_item);
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

/// Execute preview command with the item value substituted for {}
fn execute_preview_command(template: &str, item_value: &str) -> String {
    let command = template.replace("{}", item_value);
    match Command::new("sh").arg("-c").arg(&command).output() {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).to_string()
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            format!("[Preview error: {}]", stderr.trim())
        }
        Err(e) => format!("[Preview error: {}]", e),
    }
}
