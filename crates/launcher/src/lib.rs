//! Development tooling shared by the launcher's bins.
//!
//! This crate is the game binary; the library target exists only so that
//! `feral-processes`, `savetool` and `arena` can share `dev_template` — and
//! so that it can be unit-tested, which a module reached by `#[path]` from
//! three bins could not be without compiling its tests three times. Nothing
//! about the game itself lives here.

pub mod dev_template;
