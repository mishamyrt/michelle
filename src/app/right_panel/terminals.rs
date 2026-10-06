//! Right panel terminals operations.
use super::*;

impl Michelle {
    pub(in crate::app) fn ensure_right_panel_terminal(
        &mut self,
        terminal_id: Uuid,
        cx: &mut Context<Self>,
    ) {
        if self.daemon.is_remote() {
            // A desktop PTY would interpret the daemon's cwd on the wrong
            // machine. Keep the surface unavailable until the protocol grows
            // a daemon-owned streaming terminal.
            self.right_panel_model.terminals.remove(&terminal_id);
            return;
        }
        let Some(working_directory) = self
            .selected_workspace_path()
            .map(std::path::Path::to_path_buf)
        else {
            self.right_panel_model.terminals.remove(&terminal_id);
            return;
        };
        let matches_project = self
            .right_panel_model
            .terminals
            .get(&terminal_id)
            .is_some_and(|terminal| terminal.read(cx).working_directory() == working_directory);
        if !matches_project {
            self.right_panel_model.terminals.insert(
                terminal_id,
                cx.new(|cx| TerminalView::new(working_directory.clone(), cx)),
            );
        }
    }

    pub(in crate::app) fn ensure_right_panel_terminals(&mut self, cx: &mut Context<Self>) {
        let active_terminal_ids = self
            .right_panel_ui
            .surfaces
            .iter()
            .filter_map(RightPanelSurface::terminal_id)
            .collect::<Vec<_>>();
        let retained_terminal_ids = active_terminal_ids
            .iter()
            .copied()
            .chain(
                self.right_panel_model
                    .session_states
                    .values()
                    .flat_map(|state| {
                        state
                            .surfaces
                            .iter()
                            .filter_map(RightPanelSurface::terminal_id)
                    }),
            )
            .collect::<HashSet<_>>();
        self.right_panel_model
            .terminals
            .retain(|terminal_id, _| retained_terminal_ids.contains(terminal_id));
        for terminal_id in active_terminal_ids {
            self.ensure_right_panel_terminal(terminal_id, cx);
        }
    }
}
