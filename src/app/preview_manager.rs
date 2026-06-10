use crate::ui::list::ListState;
use glib::clone;
use gtk4::{gio, glib};
use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;

pub struct PreviewManager;

impl PreviewManager {
    pub fn update_preview(
        list_state: &ListState,
        preview_area_rc_opt: &Option<Rc<RefCell<crate::ui::preview::PreviewArea>>>,
    ) {
        let Some(preview_area_rc) = preview_area_rc_opt else {
            return;
        };
        let Some(item) = list_state.selected_item() else {
            return;
        };

        if let Some(ref template) = item.preview_template {
            let preview_area = preview_area_rc.clone();
            let item_value = item.value.clone();
            let template_owned = template.clone();
            let item_owned = item.clone();

            glib::spawn_future_local(clone!(
                #[weak]
                preview_area,
                async move {
                    let result = gio::spawn_blocking(move || {
                        execute_preview_command_sync(&template_owned, &item_value)
                    })
                    .await;

                    let preview_content = result.unwrap_or_else(|_| "[Preview error]".to_string());

                    let mut display_item = item_owned;
                    display_item.value = preview_content;
                    let preview_area = &*preview_area.borrow();
                    preview_area.update_with_content(&display_item);
                }
            ));
        } else {
            let preview_area = &*preview_area_rc.borrow();
            preview_area.update_with_content(&item);
        }
    }
}

fn execute_preview_command_sync(template: &str, item_value: &str) -> String {
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
