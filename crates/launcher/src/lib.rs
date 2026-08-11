//! Development tooling shared by the launcher's bins.
//!
//! This crate is the game binary; the library target exists only so that
//! `feral-processes`, `savetool` and `arena` can share `dev_template` — and
//! so that it can be unit-tested, which a module reached by `#[path]` from
//! three bins could not be without compiling its tests three times. Nothing
//! about the game itself lives here.
//!
//! `tuner` is here for the second half of that reason rather than the first:
//! only one bin runs it, but it is the part of that bin worth testing, and a
//! dev tool that proposes changes to shipped game content is exactly the
//! kind of thing that should not live untested inside a `main`.

pub mod cem;
pub mod dev_template;
pub mod tuner;
