pub mod wonderlamp {
    pub mod v1 {
        include!(concat!(env!("OUT_DIR"), "/wonderlamp.v1.rs"));
    }
}

pub use wonderlamp::v1::*;
