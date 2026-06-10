use crate::domain::item::Item;
use std::io::{self, Write};

pub struct OutputService;

impl OutputService {
    pub fn write_output(item: &Item) {
        print!("{}", item.value);
        let _ = io::stdout().flush();
    }
}
