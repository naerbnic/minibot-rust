#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Id(u64);

pub struct IdGen {
    next_id: u64,
    free_list: Vec<u64>,
}

impl IdGen {
    pub fn new() -> Self {
        IdGen {
            next_id: 0,
            free_list: Vec::new(),
        }
    }

    pub fn gen_id(&mut self) -> Id {
        if let Some(id) = self.free_list.pop() {
            Id(id)
        } else {
            assert!(self.next_id != u64::MAX);
            let new_id = self.next_id;
            self.next_id += 1;
            Id(new_id)
        }
    }
}
