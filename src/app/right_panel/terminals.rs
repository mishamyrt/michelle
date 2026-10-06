//! Right panel terminals operations.
use super::*;

fn terminal_index_to_focus(
    surfaces: &[RightPanelSurface],
    last_terminal_id: Option<Uuid>,
    terminal_focused: bool,
) -> Option<usize> {
    if terminal_focused {
        return None;
    }
    last_terminal_id
        .and_then(|id| {
            surfaces
                .iter()
                .position(|surface| surface.terminal_id() == Some(id))
        })
        .or_else(|| {
            surfaces
                .iter()
                .rposition(|surface| surface.terminal_id().is_some())
        })
}

impl Michelle {
    pub(in crate::app) fn open_terminal_action(
        &mut self,
        _: &OpenTerminal,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let terminal_focused = self.shell_ui.right_panel_visible
            && self
                .active_right_panel_surface()
                .and_then(RightPanelSurface::terminal_id)
                .and_then(|id| self.right_panel_model.terminals.get(&id))
                .is_some_and(|terminal| terminal.read(cx).focus_handle(cx).is_focused(window));
        self.settings_ui.page = None;
        if let Some(index) = terminal_index_to_focus(
            &self.right_panel_ui.surfaces,
            self.right_panel_ui.last_terminal_id,
            terminal_focused,
        ) {
            self.activate_right_panel_surface(index, cx);
            self.set_right_panel_visible(true, cx);
        } else {
            self.open_right_panel_surface(RightPanelSurface::new_terminal(), cx);
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_shortcut_creates_only_when_focused_or_missing() {
        let first = Uuid::from_u128(1);
        let second = Uuid::from_u128(2);
        let surfaces = [
            RightPanelSurface::Terminal(first),
            RightPanelSurface::Diff,
            RightPanelSurface::Terminal(second),
        ];
        assert_eq!(terminal_index_to_focus(&[], None, false), None);
        assert_eq!(
            terminal_index_to_focus(&[RightPanelSurface::Files], None, false),
            None
        );
        assert_eq!(
            terminal_index_to_focus(&surfaces, Some(first), false),
            Some(0)
        );
        assert_eq!(
            terminal_index_to_focus(&surfaces, Some(second), false),
            Some(2)
        );
        assert_eq!(terminal_index_to_focus(&surfaces, Some(first), true), None);
        assert_eq!(
            terminal_index_to_focus(&surfaces[1..], Some(first), false),
            Some(1)
        );
    }

    #[test]
    fn terminal_shortcut_uses_control_in_both_layouts() {
        let bindings = crate::terminal_key_bindings();
        for key in ["ctrl-`", "ctrl-ё", "ctrl-`->ё"] {
            let keystroke = gpui::Keystroke::parse(key).unwrap();
            assert!(
                bindings
                    .iter()
                    .any(|binding| binding.match_keystrokes(&[&keystroke]) == Some(false)),
                "{key}"
            );
        }
        for key in ["cmd-`", "cmd-ё", "`", "ё", "ctrl-shift-`"] {
            let keystroke = gpui::Keystroke::parse(key).unwrap();
            assert!(
                bindings
                    .iter()
                    .all(|binding| binding.match_keystrokes(&[&keystroke]).is_none()),
                "{key}"
            );
        }
    }
}
