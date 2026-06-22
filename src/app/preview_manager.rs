use crate::services::preview::{PreviewPayload, PreviewService};
use crate::ui::list::ListState;
use crate::ui::preview::PreviewArea;
use gtk4::{gio, glib};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::services::preview::{CacheAdapter, CommandExecutor, ImageDecoder};

pub struct PreviewManager<
    C: CacheAdapter + Clone,
    E: CommandExecutor + Clone,
    D: ImageDecoder + Clone,
> {
    service: PreviewService<C, E, D>,
    next_task_id: Cell<u64>,
    active_task_id: Cell<u64>,
}
impl<
    C: CacheAdapter + Clone + 'static,
    E: CommandExecutor + Clone + 'static,
    D: ImageDecoder + Clone + 'static,
> Clone for PreviewManager<C, E, D>
{
    fn clone(&self) -> Self {
        Self {
            service: self.service.clone(),
            next_task_id: Cell::new(self.next_task_id.get()),
            active_task_id: Cell::new(self.active_task_id.get()),
        }
    }
}

impl<
    C: CacheAdapter + Clone + 'static,
    E: CommandExecutor + Clone + 'static,
    D: ImageDecoder + Clone + 'static,
> PreviewManager<C, E, D>
{
    pub fn new(service: PreviewService<C, E, D>) -> Self {
        Self {
            service,
            next_task_id: Cell::new(1),
            active_task_id: Cell::new(0),
        }
    }

    pub fn update_preview(
        &self,
        list_state: &ListState,
        preview_area_rc_opt: &Option<Rc<RefCell<PreviewArea>>>,
    ) {
        let Some(preview_area_rc) = preview_area_rc_opt else {
            return;
        };
        let Some(item) = list_state.selected_item() else {
            return;
        };

        if let Some(cached) = self.service.try_cache(&item) {
            preview_area_rc.borrow().render(cached, &item);
            return;
        }

        let task_id = self.next_task_id.get();
        self.next_task_id.set(task_id + 1);
        self.active_task_id.set(task_id);

        let service = self.service.clone();
        let active_task_id = self.active_task_id.clone();

        preview_area_rc
            .borrow()
            .render(PreviewPayload::Text("Loading...".to_string()), &item);

        let preview_area = preview_area_rc.clone();
        let item_clone = item.clone();

        glib::spawn_future_local(async move {
            if active_task_id.get() != task_id {
                return;
            }

            let payload_result =
                gio::spawn_blocking(move || service.resolve_payload(&item_clone)).await;

            if active_task_id.get() != task_id {
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
