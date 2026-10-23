//! Clean-room project scheduling engine.
//!
//! The behavior implemented here is specified by black-box experiments: test projects were built
//! through Microsoft Project's documented COM object model, exported as MSPDI XML before and after
//! leveling, and the observed dates/slack/delays became the golden tests in `tests/golden`.
//! No vendor code was decompiled or consulted.
//!
//! Pipeline: [`mspdi::read_project`] -> [`calendar::Timeline`] -> [`cpm::schedule`] or
//! [`leveling::level`] -> [`cpm::TaskSchedule`] per task.

pub mod calendar;
pub mod cfb;
pub mod cost;
pub mod cpm;
pub mod edit;
pub mod leveling;
pub mod model;
pub mod mpp;
pub mod mspdi;
pub mod mspdi_write;
pub mod tracking;
