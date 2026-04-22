pub trait Ai {
    fn run(&self);
}

pub struct Manual;

impl Ai for Manual {
    fn run(&self) {
        //Todo
    }
}

pub struct Auto;

impl Ai for Auto {
    fn run(&self) {
        //Todo
    }
}
