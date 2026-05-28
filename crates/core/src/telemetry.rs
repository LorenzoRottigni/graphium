#[cfg(any(feature = "metrics", feature = "trace", feature = "logs"))]
mod imp {
    pub use crate::telemetry_real::*;
}

#[cfg(not(any(feature = "metrics", feature = "trace", feature = "logs")))]
mod imp {
    pub use crate::telemetry_stub::*;
}

pub use imp::*;
