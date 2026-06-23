pub struct Service {
    name: &'static str,
}

impl Service {
    pub const fn new(name: &'static str) -> Self {
        Self { name }
    }

    pub const fn name(&self) -> &'static str {
        self.name
    }
}
