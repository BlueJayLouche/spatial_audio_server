#![allow(dead_code, unused_variables, unused_imports, unused_mut)]

pub mod app;
pub mod audio;
pub mod camera;
pub mod config;
pub mod geom;
pub mod gui;
pub mod installation;
pub mod master;
pub mod metres;
pub mod osc;
pub mod project;
pub mod soundscape;
pub mod utils;

pub fn run() {
    app::run();
}
