/// Name of the artifact.
pub(crate) const NAME: &str = env!("CARGO_PKG_NAME");

/// Current semver version.
pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");

/// GIT revision.
pub(crate) const REVISION: &str = env!("GIT_HASH");

/// Version string combining [`VERSION`] and [`REVISION`].
pub(crate) const VERSION_STRING: &str =
    concat!(env!("CARGO_PKG_VERSION"), ", revision ", env!("GIT_HASH"));
