use std::sync::{Arc, Condvar, Mutex};


#[derive(Clone)]
pub struct NotifyFlag {
    cvar: Arc<(Mutex<bool>, Condvar)>,
}

impl NotifyFlag {
    pub fn new() -> Self {
        Self {
            cvar: Arc::new((Mutex::new(false), Condvar::new()))
        }
    }

    pub fn wait(&self) {
        let mut flag = self.cvar.0.lock().unwrap();
        while !*flag {
            flag = self.cvar.1.wait(flag).unwrap();
        }
    }

    pub fn notify_all(&self) {
        let mut flag = self.cvar.0.lock().unwrap();
        *flag = true;
        self.cvar.1.notify_all();
    }
}
