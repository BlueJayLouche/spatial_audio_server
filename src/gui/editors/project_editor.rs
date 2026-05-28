use egui::Ui;
use std::path::PathBuf;

/// Transient UI state for the project panel.
#[derive(Default)]
pub struct State {
    pub new_project_name: String,
}

pub enum Action {
    LoadProject(PathBuf),
    SaveProject,
    NewProject(String),
}

pub fn show(ui: &mut Ui, state: &mut State, current_name: Option<&str>) -> Option<Action> {
    let mut action = None;

    ui.heading("Project");
    ui.separator();

    if let Some(name) = current_name {
        ui.label(format!("Current: {}", name));
        if ui.button("Save").clicked() {
            action = Some(Action::SaveProject);
        }
    } else {
        ui.label("No project loaded.");
    }

    ui.separator();
    ui.label("New project");
    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut state.new_project_name);
        if ui.button("Create").clicked() && !state.new_project_name.is_empty() {
            let name = std::mem::take(&mut state.new_project_name);
            action = Some(Action::NewProject(name));
        }
    });

    action
}
