//! Sidebar projections, ordering, rename and daemon move operations.
use super::*;

pub(in crate::app) struct SidebarModel {
    pub(in crate::app) rows_fingerprint: Cell<Option<u64>>,
    pub(in crate::app) rows_snapshot: RefCell<Rc<Vec<SidebarRow>>>,
    pub(in crate::app) projectless_projects: RefCell<HashSet<Uuid>>,
    pub(in crate::app) session_moves: HashSet<Uuid>,
    pub(in crate::app) pending_session_drops: HashMap<Uuid, sidebar::SidebarSessionDrop>,
    pub(in crate::app) working_headers: RefCell<HashSet<SidebarRow>>,
}

impl Michelle {
    /// The sidebar row snapshot, rebuilt only when its inputs move.
    ///
    /// The sidebar re-renders at pulse cadence whenever one of its session
    /// rows shows a working spinner, and rebuilding the snapshot sorts every
    /// started session — far too much per
    /// tick for values that move at most once per stream commit. The
    /// fingerprint is an allocation-free scan of exactly what
    /// [`Self::sidebar_rows`] reads: started sessions with their project and
    /// recency and activity, the presentation preferences, and the collapsed
    /// groups and reveal counts.
    pub(in crate::app) fn sidebar_rows_cached(&self) -> Rc<Vec<SidebarRow>> {
        let mut fingerprint = 0x51de_ba5e_5eed_c0de;
        fingerprint = mix(
            fingerprint,
            match self.state.sidebar_ordering {
                SidebarOrdering::Newest => 1,
                SidebarOrdering::Oldest => 2,
                SidebarOrdering::Manual => 3,
            },
        );
        for session in &self.state.sessions {
            if !session.has_started() {
                continue;
            }
            fingerprint = mix_uuid(fingerprint, session.id);
            fingerprint = mix_uuid(fingerprint, session.project_id);
            fingerprint = mix(
                fingerprint,
                u64::from(matches!(
                    session.status,
                    SessionStatus::Connecting | SessionStatus::Working
                )),
            );
            if self.state.sidebar_ordering == SidebarOrdering::Manual {
                fingerprint = mix(fingerprint, session.created_at);
            }
            if self.state.sidebar_ordering != SidebarOrdering::Manual {
                fingerprint = mix(fingerprint, sidebar_session_timestamp(session));
            }
        }
        for project in &self.state.projects {
            fingerprint = mix_uuid(fingerprint, project.id);
            fingerprint = mix(fingerprint, project.created_at);
        }
        fingerprint = mix(
            fingerprint,
            u64::from(self.state.sidebar_projects_collapsed),
        );
        for group in &self.state.sidebar_project_groups {
            fingerprint = mix_uuid(fingerprint, group.id);
            fingerprint = mix(fingerprint, u64::from(group.collapsed));
            for project in &group.projects {
                fingerprint = mix_uuid(fingerprint, *project);
            }
        }
        // A map has no stable iteration order; combine order-independently.
        let revealed = self
            .sidebar_ui
            .project_reveal_counts
            .iter()
            .fold(0u64, |combined, (group, count)| {
                combined.wrapping_add(group.mix_fingerprint(*count as u64))
            });
        fingerprint = mix(
            mix(
                fingerprint,
                self.sidebar_ui.project_reveal_counts.len() as u64,
            ),
            revealed,
        );
        // A set has no stable iteration order; combine order-independently.
        let collapsed = self
            .sidebar_ui
            .collapsed_groups
            .iter()
            .fold(0u64, |combined, group| {
                combined.wrapping_add(group.mix_fingerprint(0))
            });
        fingerprint = mix(
            mix(fingerprint, self.sidebar_ui.collapsed_groups.len() as u64),
            collapsed,
        );
        if self.sidebar_model.rows_fingerprint.get() != Some(fingerprint) {
            *self.sidebar_model.rows_snapshot.borrow_mut() = Rc::new(self.sidebar_rows());
            *self.sidebar_model.working_headers.borrow_mut() = sidebar_working_headers(
                &self.state,
                crate::projectless::workspace_root().as_deref(),
            );
            self.sidebar_model.rows_fingerprint.set(Some(fingerprint));
        }
        self.sidebar_model.rows_snapshot.borrow().clone()
    }

    /// Snapshot the session history as a flat list of lightweight rows under
    /// projects and the current ordering preference.
    fn sidebar_rows(&self) -> Vec<SidebarRow> {
        let mut sorted_sessions = self
            .state
            .sessions
            .iter()
            .filter(|session| session.has_started())
            .collect::<Vec<_>>();
        sort_sidebar_sessions(
            &mut sorted_sessions,
            self.state.sidebar_ordering,
            &self.state.sidebar_session_order,
        );

        let mut rows = vec![SidebarRow::TopSpacer];
        let projectless_root = crate::projectless::workspace_root();
        let projectless_project_ids = self
            .state
            .projects
            .iter()
            .filter(|project| sidebar_project_is_projectless(project, projectless_root.as_deref()))
            .map(|project| project.id)
            .collect::<HashSet<_>>();
        *self.sidebar_model.projectless_projects.borrow_mut() = projectless_project_ids.clone();
        let mut groups = project_sidebar_groups(
            &sorted_sessions,
            &projectless_project_ids,
            &self.sidebar_model.pending_session_drops,
        );
        let mut known = groups
            .iter()
            .filter_map(|(group, _)| match group {
                SidebarGroup::Project(id) => Some(*id),
                _ => None,
            })
            .collect::<HashSet<_>>();
        for project in &self.state.projects {
            if !projectless_project_ids.contains(&project.id) && known.insert(project.id) {
                groups.push((SidebarGroup::Project(project.id), Vec::new()));
            }
        }
        let project_timestamps = sidebar_project_timestamps(&self.state);
        if self.state.sidebar_ordering != SidebarOrdering::Manual {
            groups.sort_by_key(|(group, _)| {
                let project = match group {
                    SidebarGroup::Project(id) => *id,
                    _ => Uuid::nil(),
                };
                let timestamp = project_timestamps
                    .get(&project)
                    .copied()
                    .unwrap_or_default();
                (
                    matches!(group, SidebarGroup::Projectless),
                    sidebar_project_sort_key(self.state.sidebar_ordering, timestamp, project),
                )
            });
        }
        if self.state.sidebar_ordering == SidebarOrdering::Manual {
            let ranks = self
                .state
                .sidebar_project_order
                .iter()
                .enumerate()
                .map(|(i, id)| (*id, i))
                .collect::<HashMap<_, _>>();
            groups.sort_by_key(|(group, _)| match group {
                SidebarGroup::Project(id) => (0, ranks.get(id).copied().unwrap_or(usize::MAX)),
                _ => (1, usize::MAX),
            });
        }
        let mut sections = Vec::with_capacity(groups.len());
        for (group, sessions) in groups {
            let mut section = Vec::new();
            let pending_drops = self
                .sidebar_model
                .pending_session_drops
                .values()
                .filter(|drop| drop.group == group)
                .count();
            let revealed_sessions = self
                .sidebar_ui
                .project_reveal_counts
                .get(&group)
                .copied()
                .unwrap_or_default()
                .saturating_add(pending_drops);
            let (visible_sessions, show_more) =
                visible_project_sessions(&sessions, revealed_sessions);
            if visible_sessions.is_empty() && !show_more {
                section.extend([SidebarRow::Header(group), SidebarRow::GroupSpacer]);
            } else {
                append_sidebar_group_rows(
                    &mut section,
                    group,
                    visible_sessions,
                    self.sidebar_ui.collapsed_groups.contains(&group) && pending_drops == 0,
                    show_more,
                );
            }
            sections.push(section);
        }
        append_sidebar_project_collections(
            &mut rows,
            sections,
            &self.state.sidebar_project_groups,
            self.state.sidebar_projects_collapsed,
        );
        rows
    }
    fn save_sidebar_presentation(&mut self, cx: &mut Context<Self>) {
        self.sidebar_model.rows_fingerprint.set(None);
        self.save();
        cx.notify();
    }

    pub(in crate::app) fn move_sidebar_project_to_collection(
        &mut self,
        project: Uuid,
        target: Option<Uuid>,
        cx: &mut Context<Self>,
    ) {
        if self.state.sidebar_group_for_project(project) == target {
            return;
        }
        let manual = self.state.sidebar_ordering == SidebarOrdering::Manual;
        let before = if manual {
            self.remember_sidebar_manual_order();
            let membership = self
                .state
                .sidebar_project_groups
                .iter()
                .flat_map(|group| group.projects.iter().map(move |id| (*id, group.id)))
                .collect::<HashMap<_, _>>();
            self.state
                .sidebar_project_order
                .iter()
                .copied()
                .find(|id| *id != project && membership.get(id).copied() == target)
        } else {
            None
        };
        if self.state.move_project_to_sidebar_group(project, target) {
            if manual {
                self.state.sidebar_project_order.retain(|id| *id != project);
                let index = before
                    .and_then(|id| {
                        self.state
                            .sidebar_project_order
                            .iter()
                            .position(|item| *item == id)
                    })
                    .unwrap_or(self.state.sidebar_project_order.len());
                self.state.sidebar_project_order.insert(index, project);
            }
            self.save_sidebar_presentation(cx);
        }
    }

    pub(in crate::app) fn set_sidebar_collection_collapsed(
        &mut self,
        id: Option<Uuid>,
        collapsed: bool,
        cx: &mut Context<Self>,
    ) {
        let value = if let Some(id) = id {
            let Some(group) = self
                .state
                .sidebar_project_groups
                .iter_mut()
                .find(|group| group.id == id)
            else {
                return;
            };
            &mut group.collapsed
        } else {
            &mut self.state.sidebar_projects_collapsed
        };
        let collapse_changed = *value != collapsed;
        *value = collapsed;
        let reveal_reset = collapsed
            && reset_sidebar_collection_reveals(
                &mut self.sidebar_ui.project_reveal_counts,
                &self.state,
                id,
            );
        if collapse_changed || reveal_reset {
            self.save_sidebar_presentation(cx);
        }
    }

    fn update_sidebar_collapsed_projects(&mut self) -> bool {
        let collapsed_projects = self
            .sidebar_ui
            .collapsed_groups
            .iter()
            .filter_map(|group| match group {
                SidebarGroup::Project(id) => Some(*id),
                _ => None,
            })
            .collect();
        if self.state.sidebar_collapsed_projects != collapsed_projects {
            self.state.sidebar_collapsed_projects = collapsed_projects;
            true
        } else {
            false
        }
    }

    pub(in crate::app) fn set_sidebar_ordering(
        &mut self,
        ordering: SidebarOrdering,
        cx: &mut Context<Self>,
    ) {
        if self.state.sidebar_ordering == ordering {
            return;
        }
        if ordering == SidebarOrdering::Manual {
            self.remember_sidebar_manual_order();
        }
        self.state.sidebar_ordering = ordering;
        self.sidebar_model.rows_fingerprint.set(None);
        self.sidebar_ui.list_state.scroll_to(ListOffset {
            item_ix: 0,
            offset_in_item: Pixels::ZERO,
        });
        self.save();
        cx.notify();
    }

    fn remember_sidebar_manual_order(&mut self) {
        let mut sessions = self
            .state
            .sessions
            .iter()
            .filter(|session| session.has_started())
            .collect::<Vec<_>>();
        let ordering = if self.state.sidebar_session_order.is_empty() {
            self.state.sidebar_ordering
        } else {
            SidebarOrdering::Manual
        };
        sort_sidebar_sessions(&mut sessions, ordering, &self.state.sidebar_session_order);
        self.state.sidebar_session_order = sessions.iter().map(|session| session.id).collect();
        let mut project_order = self.state.sidebar_project_order.clone();
        let valid_projects = self
            .state
            .projects
            .iter()
            .map(|project| project.id)
            .collect::<HashSet<_>>();
        project_order.retain(|id| valid_projects.contains(id));
        let mut known = project_order.iter().copied().collect::<HashSet<_>>();
        for id in sessions
            .iter()
            .map(|session| session.project_id)
            .chain(self.state.projects.iter().map(|project| project.id))
        {
            if known.insert(id) {
                project_order.push(id);
            }
        }
        self.state.sidebar_project_order = project_order;
    }

    pub(in crate::app) fn reorder_sidebar_row(
        &mut self,
        source: SidebarRow,
        target: SidebarRow,
        cx: &mut Context<Self>,
    ) {
        match (source, target) {
            (SidebarRow::Collection(Some(source)), SidebarRow::Collection(target)) => {
                if self.state.reorder_sidebar_project_group(source, target) {
                    self.save_sidebar_presentation(cx);
                }
                return;
            }
            (SidebarRow::Header(SidebarGroup::Project(source)), SidebarRow::Collection(target)) => {
                self.move_sidebar_project_to_collection(source, target, cx);
                return;
            }
            _ => {}
        }
        let rows = self.sidebar_rows_cached();
        let Some(source_index) = rows.iter().position(|row| *row == source) else {
            return;
        };
        let Some(target_index) = rows.iter().position(|row| *row == target) else {
            return;
        };
        if !sidebar_reorder_siblings(&rows, source).contains(&target) {
            return;
        }
        if self.state.sidebar_ordering == SidebarOrdering::Manual {
            self.remember_sidebar_manual_order();
        }
        let changed = match (source, target) {
            (SidebarRow::Session(source), SidebarRow::Session(target))
                if self.state.sidebar_ordering == SidebarOrdering::Manual =>
            {
                move_sidebar_item(
                    &mut self.state.sidebar_session_order,
                    source,
                    target,
                    source_index < target_index,
                )
            }
            (
                SidebarRow::Header(SidebarGroup::Project(source)),
                SidebarRow::Header(SidebarGroup::Project(target)),
            ) => {
                let destination = self.state.sidebar_group_for_project(target);
                let transferred = self
                    .state
                    .move_project_to_sidebar_group(source, destination);
                let reordered = self.state.sidebar_ordering == SidebarOrdering::Manual
                    && move_sidebar_item(
                        &mut self.state.sidebar_project_order,
                        source,
                        target,
                        source_index < target_index,
                    );
                transferred || reordered
            }
            _ => false,
        };
        if changed {
            self.sidebar_model.rows_fingerprint.set(None);
            self.save();
            cx.notify();
        }
    }

    pub(in crate::app) fn sidebar_session_can_move_to_project(
        &self,
        session: &AgentSession,
    ) -> bool {
        !self.sidebar_model.session_moves.contains(&session.id)
            && !self
                .sessions
                .runtime
                .submission_preparations
                .contains(&session.id)
            && !self.goal_model.runtime_starts.contains(&session.id)
            && !self
                .sessions
                .runtime
                .response_fork_preparations
                .contains_key(&session.id)
            && !session.is_busy()
    }

    pub(in crate::app) fn move_sidebar_session_to_project(
        &mut self,
        session_id: Uuid,
        project_id: Uuid,
        cx: &mut Context<Self>,
    ) {
        self.move_sidebar_session(session_id, Some(project_id), None, cx);
    }

    pub(in crate::app) fn move_sidebar_session(
        &mut self,
        session_id: Uuid,
        project_id: Option<Uuid>,
        placement: Option<SidebarSessionDrop>,
        cx: &mut Context<Self>,
    ) {
        if !self
            .state
            .sessions
            .iter()
            .find(|session| session.id == session_id)
            .is_some_and(|session| {
                Some(session.project_id) != project_id
                    && (project_id.is_some()
                        || !self
                            .sidebar_model
                            .projectless_projects
                            .borrow()
                            .contains(&session.project_id))
                    && self.sidebar_session_can_move_to_project(session)
            })
            || project_id.is_some_and(|project_id| {
                !self.state.projects.iter().any(|project| {
                    project.id == project_id
                        && !self
                            .sidebar_model
                            .projectless_projects
                            .borrow()
                            .contains(&project.id)
                })
            })
        {
            return;
        }
        let old_project = self
            .state
            .sessions
            .iter()
            .find(|session| session.id == session_id)
            .unwrap()
            .project_id;
        self.sidebar_model.session_moves.insert(session_id);
        if let Some(placement) = placement {
            self.sidebar_model
                .pending_session_drops
                .insert(session_id, placement);
            self.sidebar_model.rows_fingerprint.set(None);
        }
        cx.notify();
        let daemon = self.daemon.clone();
        cx.spawn(async move |michelle, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    match daemon.client().request(
                        session_id,
                        Uuid::nil(),
                        michelle_protocol::Command::MoveSessionToProject { project_id },
                    )? {
                        michelle_protocol::ResponsePayload::Session {
                            session: Some(session),
                            project: Some(project),
                        } => Ok((session, project)),
                        _ => anyhow::bail!("the daemon did not return the moved chat"),
                    }
                })
                .await;
            let _ = michelle.update(cx, |this, cx| {
                this.sidebar_model.session_moves.remove(&session_id);
                if this
                    .sidebar_model
                    .pending_session_drops
                    .remove(&session_id)
                    .is_some()
                {
                    this.sidebar_model.rows_fingerprint.set(None);
                }
                match result {
                    Ok((session, project)) => {
                        let Some(index) = this
                            .state
                            .sessions
                            .iter()
                            .position(|item| item.id == session_id)
                        else {
                            return;
                        };
                        this.sessions.runtime.runtimes.remove(&session_id);
                        this.sessions
                            .runtime
                            .runtime_attach_pending
                            .remove(&session_id);
                        this.sessions.activation.hydrations.remove(&session_id);
                        this.state.sessions[index] = session;
                        let project_id = project.id;
                        if !this
                            .state
                            .projects
                            .iter()
                            .any(|existing| existing.id == project_id)
                        {
                            this.state.projects.push(project.clone());
                        }
                        if this
                            .sidebar_model
                            .projectless_projects
                            .borrow()
                            .contains(&old_project)
                            && !this
                                .state
                                .sessions
                                .iter()
                                .any(|session| session.project_id == old_project)
                        {
                            this.state
                                .projects
                                .retain(|project| project.id != old_project);
                        }
                        let group = if project.is_projectless() {
                            SidebarGroup::Projectless
                        } else {
                            let collection = this.state.sidebar_group_for_project(project_id);
                            this.set_sidebar_collection_collapsed(collection, false, cx);
                            SidebarGroup::Project(project_id)
                        };
                        this.set_sidebar_group_collapsed(group, false, cx);
                        this.sidebar_ui
                            .project_reveal_counts
                            .insert(group, this.state.sessions.len());
                        if let Some(placement) = placement {
                            this.place_sidebar_session(session_id, placement.relative_to);
                        }
                        this.sidebar_model.rows_fingerprint.set(None);
                        if this.state.selected_session == Some(session_id)
                            || this
                                .sessions
                                .activation
                                .pending
                                .is_some_and(|pending| pending.session_id == session_id)
                        {
                            this.select_session(session_id, cx);
                            this.invalidate_workspace_queries(cx);
                        }
                        this.reveal_sidebar_session(session_id);
                        this.save();
                    }
                    Err(error) => this.show_toast(tr!("errors.move_chat", error = error)),
                }
                if this.goal_model.pending_operations.contains_key(&session_id) {
                    this.start_goal_runtime(session_id, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn place_sidebar_session(&mut self, session_id: Uuid, relative_to: Option<(Uuid, bool)>) {
        if self.state.sidebar_ordering != SidebarOrdering::Manual {
            let rows = self.sidebar_rows_cached();
            self.state.sidebar_project_order = rows
                .iter()
                .filter_map(|row| match row {
                    SidebarRow::Header(SidebarGroup::Project(id)) => Some(*id),
                    _ => None,
                })
                .collect();
            let mut sessions = self
                .state
                .sessions
                .iter()
                .filter(|session| session.has_started())
                .collect();
            sort_sidebar_sessions(&mut sessions, self.state.sidebar_ordering, &[]);
            self.state.sidebar_session_order = sessions.iter().map(|session| session.id).collect();
        }
        self.remember_sidebar_manual_order();
        self.state.sidebar_ordering = SidebarOrdering::Manual;
        self.state
            .sidebar_session_order
            .retain(|id| *id != session_id);
        let index = relative_to
            .and_then(|(target, after)| {
                self.state
                    .sidebar_session_order
                    .iter()
                    .position(|id| *id == target)
                    .map(|index| index + usize::from(after))
            })
            .unwrap_or(0);
        self.state.sidebar_session_order.insert(index, session_id);
        self.sidebar_model.rows_fingerprint.set(None);
    }

    pub(in crate::app) fn apply_sidebar_drop(
        &mut self,
        row: SidebarRow,
        preview: Option<&SidebarDragPreview>,
        cx: &mut Context<Self>,
    ) {
        if matches!(row, SidebarRow::Header(SidebarGroup::Project(_))) {
            self.apply_sidebar_project_drop(
                row,
                preview.and_then(|preview| preview.destination),
                cx,
            );
            return;
        }
        let Some(preview) = preview else {
            return;
        };
        if let SidebarRow::Session(source) = row
            && let Some(destination) = preview.session_destination
        {
            let Some(session) = self
                .state
                .sessions
                .iter()
                .find(|session| session.id == source)
            else {
                return;
            };
            let source_group = if self
                .sidebar_model
                .projectless_projects
                .borrow()
                .contains(&session.project_id)
            {
                SidebarGroup::Projectless
            } else {
                SidebarGroup::Project(session.project_id)
            };
            if source_group == destination.group {
                self.place_sidebar_session(source, destination.relative_to);
                self.save_sidebar_presentation(cx);
            } else {
                let project = match destination.group {
                    SidebarGroup::Project(id) => Some(id),
                    SidebarGroup::Projectless => None,
                };
                self.move_sidebar_session(source, project, Some(destination), cx);
            }
            return;
        }
        let changed = match row {
            SidebarRow::Collection(Some(_)) => {
                let ranks = preview
                    .rows
                    .iter()
                    .filter_map(|row| match row {
                        SidebarRow::Collection(Some(id)) => Some(*id),
                        _ => None,
                    })
                    .enumerate()
                    .map(|(index, id)| (id, index))
                    .collect::<HashMap<_, _>>();
                let old = self
                    .state
                    .sidebar_project_groups
                    .iter()
                    .map(|group| group.id)
                    .collect::<Vec<_>>();
                self.state
                    .sidebar_project_groups
                    .sort_by_key(|group| ranks.get(&group.id).copied().unwrap_or(usize::MAX));
                old.into_iter().ne(self
                    .state
                    .sidebar_project_groups
                    .iter()
                    .map(|group| group.id))
            }
            SidebarRow::Session(source)
                if self.state.sidebar_ordering == SidebarOrdering::Manual =>
            {
                let siblings = sidebar_reorder_siblings(&preview.rows, row);
                if siblings == preview.siblings {
                    return;
                }
                let Some(index) = siblings.iter().position(|candidate| *candidate == row) else {
                    return;
                };
                self.remember_sidebar_manual_order();
                let target = siblings
                    .get(index + 1)
                    .map(|row| (*row, false))
                    .or_else(|| index.checked_sub(1).map(|index| (siblings[index], true)));
                match target {
                    Some((SidebarRow::Session(target), after)) => move_sidebar_item(
                        &mut self.state.sidebar_session_order,
                        source,
                        target,
                        after,
                    ),
                    _ => false,
                }
            }
            _ => false,
        };
        if changed {
            self.save_sidebar_presentation(cx);
        }
    }

    fn apply_sidebar_project_drop(
        &mut self,
        row: SidebarRow,
        destination: Option<(Option<Uuid>, Option<Uuid>)>,
        cx: &mut Context<Self>,
    ) {
        let SidebarRow::Header(SidebarGroup::Project(project)) = row else {
            return;
        };
        let Some((collection, before)) = destination else {
            return;
        };
        let transferred = self
            .state
            .move_project_to_sidebar_group(project, collection);
        let mut reordered = false;
        if self.state.sidebar_ordering == SidebarOrdering::Manual {
            self.remember_sidebar_manual_order();
            let old_order = self.state.sidebar_project_order.clone();
            self.state.sidebar_project_order.retain(|id| *id != project);
            let index = before
                .and_then(|before| {
                    self.state
                        .sidebar_project_order
                        .iter()
                        .position(|id| *id == before)
                })
                .unwrap_or(self.state.sidebar_project_order.len());
            self.state.sidebar_project_order.insert(index, project);
            reordered = self.state.sidebar_project_order != old_order;
        }
        if transferred || reordered {
            self.save_sidebar_presentation(cx);
        }
    }

    pub(in crate::app) fn rename_sidebar_collection(
        &mut self,
        id: Uuid,
        name: &str,
        cx: &mut Context<Self>,
    ) {
        if self.state.rename_sidebar_project_group(id, name) {
            self.save_sidebar_presentation(cx);
        }
        cx.notify();
    }

    pub(in crate::app) fn rename_sidebar_session(
        &mut self,
        session_id: Uuid,
        title: &str,
        cx: &mut Context<Self>,
    ) {
        let title = title.trim().to_owned();
        let should_update = !title.is_empty()
            && self
                .state
                .sessions
                .iter()
                .find(|session| session.id == session_id)
                .is_some_and(|session| session.title != title);
        if should_update
            && self
                .state
                .session_mut(session_id)
                .is_some_and(|session| session.set_title(&title))
        {
            self.save();
        }
        cx.notify();
    }
}
pub(in crate::app) fn append_sidebar_group_rows(
    rows: &mut Vec<SidebarRow>,
    group: SidebarGroup,
    sessions: &[Uuid],
    collapsed: bool,
    show_more: bool,
) {
    if sessions.is_empty() && !show_more {
        return;
    }

    rows.push(SidebarRow::Header(group));
    if !collapsed {
        rows.extend(sessions.iter().copied().map(SidebarRow::Session));
        if show_more {
            rows.push(SidebarRow::ShowMore(group));
        }
    }
    rows.push(SidebarRow::GroupSpacer);
}

/// Recency for sidebar ordering. A submitted turn promotes the
/// task immediately, while metadata edits such as a rename do not; a task with
/// no turns stays anchored to when it was created.
pub(in crate::app) fn sidebar_session_timestamp(session: &AgentSession) -> u64 {
    session.last_reply_at.unwrap_or(session.created_at)
}

pub(in crate::app) fn sort_sidebar_sessions(
    sessions: &mut Vec<&AgentSession>,
    ordering: SidebarOrdering,
    manual_order: &[Uuid],
) {
    match ordering {
        SidebarOrdering::Newest => {
            sessions.sort_by_key(|session| std::cmp::Reverse(sidebar_session_timestamp(session)))
        }
        SidebarOrdering::Oldest => {
            sessions.sort_by_key(|session| sidebar_session_timestamp(session))
        }
        SidebarOrdering::Manual => {
            let ranks = manual_order
                .iter()
                .enumerate()
                .map(|(i, id)| (*id, i))
                .collect::<HashMap<_, _>>();
            sessions.sort_by_key(|session| {
                (
                    ranks.get(&session.id).copied(),
                    std::cmp::Reverse(session.created_at),
                    session.id,
                )
            });
        }
    }
}

pub(in crate::app) fn move_sidebar_item(
    order: &mut Vec<Uuid>,
    source: Uuid,
    target: Uuid,
    after_target: bool,
) -> bool {
    if source == target {
        return false;
    }
    let Some(from) = order.iter().position(|id| *id == source) else {
        return false;
    };
    let Some(target_index) = order.iter().position(|id| *id == target) else {
        return false;
    };
    let to = target_index - usize::from(from < target_index) + usize::from(after_target);
    if from == to {
        return false;
    }
    order.remove(from);
    order.insert(to, source);
    true
}

pub(in crate::app) fn project_sidebar_groups(
    sessions: &[&AgentSession],
    projectless_project_ids: &HashSet<Uuid>,
    pending_drops: &HashMap<Uuid, SidebarSessionDrop>,
) -> Vec<(SidebarGroup, Vec<Uuid>)> {
    let mut groups: Vec<(SidebarGroup, Vec<Uuid>)> = Vec::new();
    let mut indexes = HashMap::new();
    let mut projectless_sessions = Vec::new();
    for session in sessions {
        let group = pending_drops.get(&session.id).map_or_else(
            || {
                if projectless_project_ids.contains(&session.project_id) {
                    SidebarGroup::Projectless
                } else {
                    SidebarGroup::Project(session.project_id)
                }
            },
            |drop| drop.group,
        );
        let SidebarGroup::Project(project_id) = group else {
            projectless_sessions.push(session.id);
            continue;
        };
        let index = *indexes.entry(project_id).or_insert_with(|| {
            let index = groups.len();
            groups.push((SidebarGroup::Project(project_id), Vec::new()));
            index
        });
        groups[index].1.push(session.id);
    }
    if !projectless_sessions.is_empty() {
        groups.push((SidebarGroup::Projectless, projectless_sessions));
    }
    for (group, sessions) in &mut groups {
        for (session_id, drop) in pending_drops {
            if drop.group != *group {
                continue;
            }
            if let Some((target, after)) = drop.relative_to {
                move_sidebar_item(sessions, *session_id, target, after);
            } else if let Some(index) = sessions.iter().position(|id| id == session_id) {
                sessions.remove(index);
                sessions.insert(0, *session_id);
            }
        }
    }
    groups
}

pub(in crate::app) fn sidebar_project_timestamps(state: &PersistedState) -> HashMap<Uuid, u64> {
    let mut timestamps: HashMap<Uuid, u64> = HashMap::new();
    for session in state
        .sessions
        .iter()
        .filter(|session| session.has_started())
    {
        let timestamp = sidebar_session_timestamp(session);
        timestamps
            .entry(session.project_id)
            .and_modify(|current| {
                *current = if state.sidebar_ordering == SidebarOrdering::Oldest {
                    (*current).min(timestamp)
                } else {
                    (*current).max(timestamp)
                };
            })
            .or_insert(timestamp);
    }
    for project in &state.projects {
        timestamps.entry(project.id).or_insert(project.created_at);
    }
    timestamps
}

pub(in crate::app) fn sidebar_project_sort_key(
    ordering: SidebarOrdering,
    timestamp: u64,
    project: Uuid,
) -> (u64, Uuid) {
    (
        if ordering == SidebarOrdering::Newest {
            u64::MAX - timestamp
        } else {
            timestamp
        },
        project,
    )
}

pub(in crate::app) fn visible_project_sessions(
    sessions: &[Uuid],
    revealed_sessions: usize,
) -> (&[Uuid], bool) {
    if sessions.len() <= SIDEBAR_PROJECT_INITIAL_LIMIT + 1 {
        return (sessions, false);
    }
    let visible_count = SIDEBAR_PROJECT_INITIAL_LIMIT
        .saturating_add(revealed_sessions)
        .min(sessions.len());
    (&sessions[..visible_count], visible_count < sessions.len())
}

pub(in crate::app) fn reset_sidebar_collection_reveals(
    revealed: &mut HashMap<SidebarGroup, usize>,
    state: &PersistedState,
    collection: Option<Uuid>,
) -> bool {
    let previous_count = revealed.len();
    revealed.retain(|group, _| match group {
        SidebarGroup::Project(project) => state.sidebar_group_for_project(*project) != collection,
        SidebarGroup::Projectless => true,
    });
    revealed.len() != previous_count
}

pub(in crate::app) fn sidebar_project_is_projectless(
    project: &Project,
    projectless_root: Option<&Path>,
) -> bool {
    projectless_root.is_some_and(|root| project.path.starts_with(root))
}

pub(in crate::app) fn sidebar_working_headers(
    state: &PersistedState,
    projectless_root: Option<&Path>,
) -> HashSet<SidebarRow> {
    let projectless_projects = state
        .projects
        .iter()
        .filter(|project| sidebar_project_is_projectless(project, projectless_root))
        .map(|project| project.id)
        .collect::<HashSet<_>>();
    let mut collections = HashMap::new();
    for collection in &state.sidebar_project_groups {
        for project in &collection.projects {
            collections.entry(*project).or_insert(collection.id);
        }
    }
    let mut headers = HashSet::new();
    for session in state.sessions.iter().filter(|session| {
        session.has_started()
            && matches!(
                session.status,
                SessionStatus::Connecting | SessionStatus::Working
            )
    }) {
        let projectless = projectless_projects.contains(&session.project_id);
        if !projectless {
            headers.insert(SidebarRow::Collection(
                collections.get(&session.project_id).copied(),
            ));
        }
        let group = if projectless {
            SidebarGroup::Projectless
        } else {
            SidebarGroup::Project(session.project_id)
        };
        headers.insert(SidebarRow::Header(group));
    }
    headers
}

pub(in crate::app) fn append_sidebar_project_collections(
    rows: &mut Vec<SidebarRow>,
    sections: Vec<Vec<SidebarRow>>,
    groups: &[SidebarProjectGroup],
    projects_collapsed: bool,
) {
    let mut membership = HashMap::new();
    for (index, group) in groups.iter().enumerate() {
        for project in &group.projects {
            membership.entry(*project).or_insert(index);
        }
    }
    let mut buckets = vec![Vec::new(); groups.len() + 1];
    let mut chats = Vec::new();
    for section in sections {
        if section.first() == Some(&SidebarRow::Header(SidebarGroup::Projectless)) {
            chats = section;
            continue;
        }
        let index = match section.first() {
            Some(SidebarRow::Header(SidebarGroup::Project(id))) => {
                membership.get(id).copied().unwrap_or(groups.len())
            }
            _ => groups.len(),
        };
        buckets[index].extend(
            section
                .into_iter()
                .filter(|row| *row != SidebarRow::GroupSpacer),
        );
    }
    for (index, mut bucket) in buckets.into_iter().enumerate() {
        let group = groups.get(index);
        rows.push(SidebarRow::Collection(group.map(|group| group.id)));
        if !group.map_or(projects_collapsed, |group| group.collapsed) {
            rows.append(&mut bucket);
        }
        rows.push(SidebarRow::GroupSpacer);
    }
    rows.append(&mut chats);
}

/// Only peers may be reordered: projects, or sessions within one section.
pub(in crate::app) fn sidebar_reorder_siblings(
    rows: &[SidebarRow],
    row: SidebarRow,
) -> Vec<SidebarRow> {
    match row {
        SidebarRow::Collection(Some(_)) => rows
            .iter()
            .copied()
            .filter(|row| matches!(row, SidebarRow::Collection(Some(_))))
            .collect(),
        SidebarRow::Header(SidebarGroup::Project(_)) => rows
            .iter()
            .copied()
            .filter(|row| matches!(row, SidebarRow::Header(SidebarGroup::Project(_))))
            .collect(),
        SidebarRow::Session(_) => {
            let Some(index) = rows.iter().position(|candidate| *candidate == row) else {
                return Vec::new();
            };
            let start = rows[..index]
                .iter()
                .rposition(|row| matches!(row, SidebarRow::Header(_)))
                .map_or(0, |i| i + 1);
            rows[start..]
                .iter()
                .copied()
                .take_while(|row| !matches!(row, SidebarRow::Header(_) | SidebarRow::Collection(_)))
                .filter(|row| matches!(row, SidebarRow::Session(_)))
                .collect()
        }
        _ => Vec::new(),
    }
}

impl Michelle {
    pub(in crate::app) fn collapse_all_sidebar_groups(&mut self, cx: &mut Context<Self>) {
        let groups = self
            .sidebar_rows_cached()
            .iter()
            .filter_map(|row| match row {
                SidebarRow::Header(group) => Some(*group),
                _ => None,
            })
            .collect::<Vec<_>>();
        let collections_changed = !self.state.sidebar_projects_collapsed
            || self
                .state
                .sidebar_project_groups
                .iter()
                .any(|group| !group.collapsed);
        self.state.sidebar_projects_collapsed = true;
        for group in &mut self.state.sidebar_project_groups {
            group.collapsed = true;
        }
        let mut changed = !self.sidebar_ui.project_reveal_counts.is_empty();
        self.sidebar_ui.project_reveal_counts.clear();
        for group in groups {
            changed |= self.sidebar_ui.collapsed_groups.insert(group);
        }
        if changed || collections_changed {
            self.sidebar_model.rows_fingerprint.set(None);
            if self.update_sidebar_collapsed_projects() || collections_changed {
                self.save();
            }
            cx.notify();
        }
    }

    pub(in crate::app) fn set_sidebar_group_collapsed(
        &mut self,
        group: SidebarGroup,
        collapsed: bool,
        cx: &mut Context<Self>,
    ) {
        let collapse_changed = if collapsed {
            self.sidebar_ui.collapsed_groups.insert(group)
        } else {
            self.sidebar_ui.collapsed_groups.remove(&group)
        };
        let reveal_reset = collapsed
            && self
                .sidebar_ui
                .project_reveal_counts
                .remove(&group)
                .is_some();
        if collapse_changed || reveal_reset {
            self.sidebar_model.rows_fingerprint.set(None);
            if self.update_sidebar_collapsed_projects() {
                self.save();
            }
            cx.notify();
        }
    }
    pub(in crate::app) fn add_sidebar_collection(
        &mut self,
        name: &str,
        cx: &mut Context<Self>,
    ) -> Option<Uuid> {
        let id = self.state.create_sidebar_project_group(name)?;
        self.save_sidebar_presentation(cx);
        Some(id)
    }

    pub(in crate::app) fn remove_sidebar_collection(
        &mut self,
        id: Uuid,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.state.remove_sidebar_project_group(id) {
            return false;
        }
        if self.sidebar_ui.project_group_rename == Some(id) {
            self.sidebar_ui.project_group_rename = None;
        }
        self.save_sidebar_presentation(cx);
        true
    }
}

pub(in crate::app) fn sidebar_session_selected(
    selected_session: Option<Uuid>,
    pending_session: Option<Uuid>,
    session_id: Uuid,
) -> bool {
    pending_session.map_or(selected_session == Some(session_id), |pending| {
        pending == session_id
    })
}
