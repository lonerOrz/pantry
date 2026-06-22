use crate::services::preview::{PreviewPayload, ProdPreviewService, create_prod_preview_service};
use crate::ui::list::ListState;
use crate::ui::preview::PreviewArea;
use gtk4::{gio, glib};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

static PREVIEW_SERVICE: OnceLock<ProdPreviewService> = OnceLock::new();

pub struct PreviewManager;

impl PreviewManager {
    pub fn update_preview(
        list_state: &ListState,
        preview_area_rc_opt: &Option<Rc<RefCell<PreviewArea>>>,
    ) {
        let Some(preview_area_rc) = preview_area_rc_opt else {
            return;
        };
        let Some(item) = list_state.selected_item() else {
            return;
        };

        static NEXT_TASK_ID: OnceLock<AtomicU64> = OnceLock::new();
        static ACTIVE_TASK_ID: OnceLock<AtomicU64> = OnceLock::new();
        let next_id = NEXT_TASK_ID.get_or_init(|| AtomicU64::new(1));
        let active_id = ACTIVE_TASK_ID.get_or_init(|| AtomicU64::new(0));

        let task_id = next_id.fetch_add(1, Ordering::SeqCst);
        active_id.store(task_id, Ordering::SeqCst);

        preview_area_rc
            .borrow()
            .render(PreviewPayload::Text("Loading...".to_string()), &item);

        let preview_area = preview_area_rc.clone();
        let service = PREVIEW_SERVICE.get_or_init(create_prod_preview_service);
        let item_clone = item.clone();

        glib::spawn_future_local(async move {
            if active_id.load(Ordering::SeqCst) != task_id {
                return;
            }

            let payload_result =
                gio::spawn_blocking(move || service.resolve_payload(&item_clone)).await;

            if active_id.load(Ordering::SeqCst) != task_id {
                return;
            }

            match payload_result {
                Ok(payload) => {
                    preview_area.borrow().render(payload, &item);
                }
                Err(_) => {
                    preview_area.borrow().render(
                        PreviewPayload::Error("Preview generation panicked".to_string()),
                        &item,
                    );
                }
            }
        });
    }
}
