//! Component assembly, subscriptions and startup tasks.
use super::*;

impl Michelle {
    pub fn new(
        window: &mut Window,
        cx: &mut App,
        daemon: michelle_client::DaemonSupervisor,
    ) -> Entity<Self> {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let store = StateStore::remote(daemon.clone());
        let daemon_hostname = crate::daemon::local_hostname().unwrap_or_else(|| "this-mac".into());
        let composer_draft_store = ComposerDraftStore::remote(daemon.clone());
        let composer_drafts = composer_draft_store.load().unwrap_or_default();
        let mut state = store.load_or_fresh(cwd);
        let home_directory = crate::projectless::home_directory();
        let projectless_root = crate::projectless::workspace_root();
        state.apply_daemon_settings(daemon.settings());
        if let Err(error) = daemon.update_settings(state.daemon_settings()) {
            eprintln!("could not normalize daemon settings after migration: {error:#}");
        }
        crate::i18n::set_language(state.language);
        // Chrome text is authored in `sp` rems against the default UI font
        // size, so the window's rem size *is* the UI font size setting.
        window.set_rem_size(px(michelle_client::persistence::sanitized_ui_font_size(
            state.ui_font_size,
        )));

        let composer = cx.new(|cx| {
            ComposerInput::new(window, cx)
                .attach_large_text_pastes(cx)
                .padding_x(px(16.0), cx)
        });
        let user_input_answer = cx
            .new(|cx| TextInput::new(window, cx).placeholder(tr!("user_input.other_placeholder")));
        let command_palette_search = cx.new(|cx| {
            TextInput::new(window, cx)
                .clear_on_escape()
                .placeholder(tr!("command_palette.placeholder"))
        });
        let model_search = cx.new(|cx| {
            TextInput::new(window, cx)
                .clear_on_escape()
                .placeholder(tr!("input.search_models"))
        });
        let branch_search = cx.new(|cx| {
            TextInput::new(window, cx)
                .clear_on_escape()
                .placeholder(tr!("input.search_branches"))
        });
        let branch_create_input = cx.new(|cx| {
            TextInput::new(window, cx)
                .clear_on_escape()
                .placeholder(tr!("input.new_branch_name"))
        });
        let project_search = cx.new(|cx| {
            TextInput::new(window, cx)
                .clear_on_escape()
                .placeholder(tr!("input.search_projects"))
        });
        let settings_search = cx.new(|cx| {
            TextInput::new(window, cx)
                .clear_on_escape()
                .placeholder(tr!("settings.search"))
        });
        let daemon_port = state.daemon_exposure.port.to_string();
        let daemon_origins = state.daemon_exposure.allowed_origins_text();
        let daemon_port_input = cx.new(|cx| {
            let mut input = TextInput::new(window, cx)
                .select_all_on_focus_click()
                .placeholder(tr!("daemon.port_placeholder"));
            input.set_content(daemon_port, cx);
            input
        });
        let daemon_origins_input = cx.new(|cx| {
            let mut input = TextInput::new(window, cx)
                .select_all_on_focus_click()
                .placeholder(tr!("daemon.allowed_origins_placeholder"));
            input.set_content(daemon_origins, cx);
            input
        });
        let skills_search = cx.new(|cx| {
            TextInput::new(window, cx)
                .clear_on_escape()
                .placeholder(tr!("skills.search"))
        });
        let session_rename_input = cx.new(|cx| TextInput::new(window, cx));
        let provider_path_input = cx.new(|cx| {
            TextInput::new(window, cx)
                .select_all_on_focus_click()
                .placeholder(tr!("input.detected_automatically"))
        });
        let usage_project_filter =
            cx.new(|cx| TextInput::new(window, cx).placeholder(tr!("input.filter_projects")));
        let right_panel_diff_filter =
            cx.new(|cx| TextInput::new(window, cx).placeholder(tr!("diff.filter_files")));
        let navigation_rail = cx.new(|_| ConversationNavigationRail::new());
        let sidebar_pane = MichellePane::new(Michelle::sidebar_pane_content, cx);
        let transcript_pane = MichellePane::new(Michelle::transcript_pane_content, cx);
        let right_panel_pane = MichellePane::new(Michelle::right_panel_pane_content, cx);
        let workspace_client = michelle_client::WorkspaceClient::new(daemon.client());
        let (projectless_migrated, projectless_migration_error) =
            migrate_legacy_projectless_projects(&mut state, &workspace_client);
        let projectless_save_error = projectless_migrated
            .then(|| store.save(&mut state).err())
            .flatten();
        let startup_toast = projectless_migration_error
            .map(|error| tr!("errors.move_projectless_task", error = error))
            .or_else(|| {
                projectless_save_error
                    .map(|error| tr!("errors.save_projectless_migration", error = error))
            });
        let sidebar_visible = state.sidebar_visible;
        let sidebar_collapsed_groups = state
            .sidebar_collapsed_projects
            .iter()
            .copied()
            .map(SidebarGroup::Project)
            .collect();
        let right_panel_visible = state.right_panel_visible;
        let sidebar_width = sanitize_panel_width(
            state.sidebar_width,
            DEFAULT_SIDEBAR_WIDTH,
            SIDEBAR_MIN_WIDTH,
            SIDEBAR_MAX_WIDTH,
        );
        let right_panel_width = sanitize_panel_width(
            state.right_panel_width,
            DEFAULT_RIGHT_PANEL_WIDTH,
            RIGHT_PANEL_MIN_WIDTH,
            RIGHT_PANEL_MAX_WIDTH,
        );
        state.sidebar_width = sidebar_width;
        state.right_panel_width = right_panel_width;
        // First launch has no persisted frame yet; seed from the freshly
        // opened window so an immediate zoom or fullscreen still has a
        // floating frame to restore to. The bounds observer keeps it current
        // from here.
        if state.window_state.is_none() {
            state.window_state = Some(persisted_window_state(
                window.bounds(),
                false,
                window.display(cx).and_then(|display| display.uuid().ok()),
            ));
        }
        crate::theme::apply_theme_preference(state.theme, &ThemeDefinition::default(), window, cx);
        crate::platform::set_sidebar_material_width(window, sidebar_width);
        let project_paths = state
            .projects
            .iter()
            .map(|project| (project.id, project.path.clone()))
            .collect::<HashMap<_, _>>();
        let mut startup_live_session_ids = state
            .sessions
            .iter()
            .filter(|session| session.status.is_busy())
            .map(|session| session.id)
            .collect::<Vec<_>>();
        if let Some(selected) = state.selected_session
            && state
                .sessions
                .iter()
                .find(|session| session.id == selected)
                .is_some_and(AgentSession::has_started)
            && !startup_live_session_ids.contains(&selected)
        {
            startup_live_session_ids.push(selected);
        }
        let mut interrupted_turn_checkpoints = Vec::new();
        for session in &mut state.sessions {
            session.migrate_legacy_state();
            // A provider runtime belongs to the daemon and may still be
            // streaming after this desktop process restarted. Leave its
            // persisted projection intact until the background attachment
            // check proves there is no live runtime to resume.
            if session.status.is_busy() {
                continue;
            }
            if session.status != SessionStatus::Idle {
                session.status = SessionStatus::Idle;
            }
            let interrupted_turn = if let Some(turn) = session
                .turns
                .last_mut()
                .filter(|turn| turn.status == TurnStatus::Running)
            {
                turn.status = TurnStatus::Interrupted;
                turn.completed_at = Some(unix_time());
                Some(turn.turn_count)
            } else {
                None
            };
            // A crash mid-turn leaves work in the tree worth checkpointing, but
            // one `capture_turn` per interrupted session is upwards of ten
            // `git` invocations each — paid here, before the window has drawn
            // once. Queue them and let the first frames go out first.
            if let Some(turn_count) = interrupted_turn
                && let Some(project_path) = session
                    .workspace
                    .path()
                    .map(std::path::Path::to_path_buf)
                    .or_else(|| project_paths.get(&session.project_id).cloned())
            {
                interrupted_turn_checkpoints.push(PendingCheckpointCapture {
                    session_id: session.id,
                    turn_count,
                    project_path,
                });
            }
            for message in &mut session.messages {
                message.streaming = false;
            }
            for block in &mut session.transcript_blocks {
                block.activities.retain(|activity| {
                    activity
                        .reasoning
                        .as_ref()
                        .is_none_or(|reasoning| !reasoning.content.trim().is_empty())
                });
                for activity in &mut block.activities {
                    activity.complete = true;
                }
            }
            session
                .transcript_blocks
                .retain(|block| !block.activities.is_empty());
        }
        let initial_composer_draft = state
            .selected_session
            .and_then(|selected| state.sessions.iter().find(|session| session.id == selected))
            .and_then(|session| composer_drafts.get_for(session))
            .cloned()
            .unwrap_or_default();
        let crate::persistence::ComposerDraft {
            text: initial_composer_text,
            attachments: initial_composer_attachments,
        } = initial_composer_draft;
        if !initial_composer_text.is_empty() {
            composer.update(cx, |input, cx| input.set_content(initial_composer_text, cx));
        }
        let composer_attachments = initial_composer_attachments
            .into_iter()
            .map(ComposerAttachment::from)
            .collect();
        let probes = ProviderKind::ALL
            .into_iter()
            .map(|provider| ProviderProbe {
                provider,
                installed: false,
                path: None,
                models: crate::model_catalog::fallback_models(provider),
                agent_presets: crate::model_catalog::fallback_agent_presets(provider),
            })
            .collect::<Vec<_>>();
        let (provider_probe_tx, provider_probe_events) = unbounded();
        let (provider_version_tx, provider_version_events) = unbounded();
        let (provider_detection_tx, provider_detection_events) = unbounded();
        let (computer_permission_tx, computer_permission_events) = unbounded();
        let (plan_usage_tx, plan_usage_events) = unbounded();
        let (event_wake_tx, event_wake_events) = smol::channel::bounded(1);
        let (task_state_sync_tx, task_state_sync_events) = unbounded();
        #[cfg(target_os = "macos")]
        if crate::computer_use::is_available() {
            let computer_permission_tx = computer_permission_tx.clone();
            let event_wake = event_wake_tx.clone();
            let daemon = daemon.client();
            std::thread::Builder::new()
                .name("michelle-computer-permission-probe".into())
                .spawn(move || {
                    let result = match daemon.request(
                        Uuid::nil(),
                        Uuid::nil(),
                        michelle_client::Command::ProbeComputerPermissions { prompt: false },
                    ) {
                        Ok(michelle_client::ResponsePayload::ComputerPermissions {
                            permissions,
                        }) => Ok(permissions),
                        Ok(_) => Err("the daemon returned an invalid permission response".into()),
                        Err(error) => Err(error.to_string()),
                    };
                    if computer_permission_tx.send(result).is_ok() {
                        signal_event_pump(&event_wake);
                    }
                })
                .ok();
        }
        let mut session_navigation = SessionNavigation::default();
        if let Some(session_id) = state.selected_session.filter(|session_id| {
            state
                .sessions
                .iter()
                .any(|session| session.id == *session_id && !session.has_started())
        }) {
            session_navigation.remember_new_task(session_id);
        }
        // Measure visible rows only, with a generous overdraw — the same shape
        // Zed's own agent chat uses. `measure_all` lays out every row in the
        // session on the first frame and again after any structural splice,
        // which a long transcript cannot afford.
        let transcript_rows = ListState::new(0, ListAlignment::Bottom, px(2048.0));
        let anchored_transcript_rows = ListState::new(0, ListAlignment::Top, px(2048.0));
        let sidebar_list_state = ListState::new(0, ListAlignment::Top, px(256.0));
        let usage_projects_list = ListState::new(0, ListAlignment::Top, px(256.0));
        let branch_picker_list_state = ListState::new(0, ListAlignment::Top, px(152.0));
        let project_picker_list_state = ListState::new(0, ListAlignment::Top, px(152.0));
        let transcript_is_scrolled = Rc::new(Cell::new(false));
        let transcript_anchor_following = Rc::new(Cell::new(false));
        let transcript_tail_recheck = Rc::new(Cell::new(false));
        // A wheel scroll drops tail following and asks the next measured frame
        // whether it landed back on the tail. GPUI re-engages its own tail pin
        // when a bottom-aligned list reaches the end — it represents that end as
        // no logical offset — but a turn renders through the top-aligned
        // anchored list, whose end is an ordinary offset, so only this can.
        transcript_rows.set_scroll_handler({
            let transcript_is_scrolled = transcript_is_scrolled.clone();
            let transcript_anchor_following = transcript_anchor_following.clone();
            let transcript_tail_recheck = transcript_tail_recheck.clone();
            move |event, window, _| {
                transcript_is_scrolled.set(event.is_scrolled);
                transcript_anchor_following.set(false);
                transcript_tail_recheck.set(true);
                window.refresh();
            }
        });
        anchored_transcript_rows.set_scroll_handler({
            let transcript_is_scrolled = transcript_is_scrolled.clone();
            let transcript_anchor_following = transcript_anchor_following.clone();
            let transcript_tail_recheck = transcript_tail_recheck.clone();
            move |event, window, _| {
                transcript_is_scrolled.set(event.is_scrolled);
                transcript_anchor_following.set(false);
                transcript_tail_recheck.set(true);
                window.refresh();
            }
        });
        let entity = cx.new(|cx| {
            let settings_focus = cx.focus_handle();
            let onboarding_add_project_focus = cx.focus_handle();
            let onboarding_projectless_focus = cx.focus_handle();
            let model_picker_empty_focus = cx.focus_handle();
            let model_picker_favorite_focus = cx.focus_handle();
            let task_switcher_focus = cx.focus_handle();
            cx.on_focus_out(
                &task_switcher_focus,
                window,
                |this: &mut Self, _, window, cx| {
                    this.cancel_task_switcher(window, cx);
                },
            )
            .detach();
            let mut task_switcher = task_switcher::TaskSwitcherUi::new(task_switcher_focus);
            if let Some(selected_session) = state.selected_session {
                task_switcher.record_access(selected_session);
            }

            cx.observe_window_appearance(window, |this: &mut Self, window, cx| {
                if this.state.theme == ThemePreference::System {
                    this.apply_theme(window, cx);
                    cx.notify();
                }
            })
            .detach();

            cx.observe_window_bounds(window, |this: &mut Self, window, cx| {
                this.capture_window_state(window, cx);
            })
            .detach();

            cx.observe_window_activation(window, |this: &mut Self, window, cx| {
                if window.is_window_active() {
                    this.reload_clean_right_panel_file_editors(cx);
                    // The working tree and branch may have moved while another
                    // app had focus — a checkout in a terminal, an edit in an
                    // editor. Coming back is the moment to re-check.
                    this.invalidate_workspace_queries(cx);
                    if this.settings_ui.page == Some(SettingsPage::ComputerUse) {
                        this.request_computer_permissions(false, cx);
                    }
                    // Skill files are routinely edited in another app; coming
                    // back to the window is the moment to re-read them.
                    if this.settings_ui.page == Some(SettingsPage::Skills) {
                        this.ensure_skills_catalog(true, cx);
                    }
                }
            })
            .detach();

            // A closed surface can take the window's focus down with it, and
            // with nothing focused, action availability walks only the root
            // dispatch node, so every app menu item greys out. When focus
            // dies with its element, send it home to the composer, the way
            // Zed's workspace refocuses itself.
            cx.on_focus_lost(window, |this: &mut Self, window, cx| {
                let focus = this.composer_focus(cx);
                window.focus(&focus, cx);
            })
            .detach();

            // Edits, not raw notifies: a field also notifies for caret blinks
            // and selection changes, and none of the app chrome depends on
            // those — re-rendering the window twice a second for a blinking
            // caret is exactly what the Performance guidance forbids.
            composer::subscribe_input(&composer, cx);

            cx.subscribe(
                &user_input_answer,
                |this: &mut Self, input, event: &InputEvent, cx| match event {
                    InputEvent::Submit(answer) => {
                        this.submit_user_input_custom_answer(answer.clone(), cx);
                    }
                    InputEvent::Edited => {
                        let answer = input.read(cx).content().to_owned();
                        this.update_user_input_custom_answer(answer, cx);
                    }
                    InputEvent::Focus | InputEvent::BackspaceOnEmpty => {}
                },
            )
            .detach();

            // Clipboard images, Finder files, and large text are attachment
            // payloads. The input owns representation priority; Michelle
            // owns durable staging and composer/session state.

            // A normal Cmd-Q waits briefly for this future, so even an edit
            // made inside the debounce window is durable before the process
            // exits. Filesystem work still stays off the UI thread.
            cx.on_app_quit(|this, cx| {
                this.capture_current_composer_draft(cx);
                this.composer_model.draft_save_generation =
                    this.composer_model.draft_save_generation.saturating_add(1);
                let generation = this.composer_model.draft_save_generation;
                let store = this.composer_model.draft_store.clone();
                let drafts = this.composer_model.drafts.clone();
                let save = cx
                    .background_executor()
                    .spawn(async move { store.save(drafts, generation) });
                async move {
                    let _ = save.await;
                }
            })
            .detach();

            // Window-frame changes are only mirrored in memory; the quit save
            // is what lands the final position and size on disk.
            cx.on_app_quit(|this, _| {
                this.save();
                async {}
            })
            .detach();

            // A changed query re-filters the picker rows and renumbers them,
            // so the drawn selection cannot carry over. While a filter is
            // active the cursor lands on the first match so `enter` has a
            // visible target; clearing the query returns to the opening
            // state — nothing highlighted, the current model's row in view.
            model_picker::subscribe_search(&model_search, cx);
            cx.subscribe(
                &command_palette_search,
                |this: &mut Self, search, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Edited) {
                        let query = search.read(cx).content().to_owned();
                        this.command_palette_query_edited(&query, cx);
                    }
                },
            )
            .detach();
            branch_picker::subscribe_search(&branch_search, cx);
            project_picker::subscribe_search(&project_search, cx);
            cx.subscribe(
                &branch_create_input,
                |_: &mut Self, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Edited) {
                        cx.notify();
                    }
                },
            )
            .detach();
            cx.subscribe(
                &settings_search,
                |_: &mut Self, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Edited) {
                        cx.notify();
                    }
                },
            )
            .detach();
            for input in [&daemon_port_input, &daemon_origins_input] {
                cx.subscribe(
                    input,
                    |this: &mut Self, _, event: &InputEvent, cx| match event {
                        InputEvent::Submit(_) => this.apply_daemon_exposure_fields(cx),
                        InputEvent::Edited => cx.notify(),
                        _ => {}
                    },
                )
                .detach();
            }
            cx.subscribe(&skills_search, |_: &mut Self, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Edited) {
                    cx.notify();
                }
            })
            .detach();
            cx.subscribe_in(
                &session_rename_input,
                window,
                |this: &mut Self, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Submit(_) => this.finish_session_rename(window, cx),
                    InputEvent::Edited
                        if this.sidebar_ui.session_rename.is_some()
                            || this.sidebar_ui.project_group_rename.is_some() =>
                    {
                        cx.notify()
                    }
                    _ => {}
                },
            )
            .detach();
            cx.subscribe(
                &usage_project_filter,
                |_: &mut Self, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Edited) {
                        cx.notify();
                    }
                },
            )
            .detach();
            cx.subscribe(
                &right_panel_diff_filter,
                |this: &mut Self, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Edited) {
                        this.sync_right_panel_diff_tree_rows(cx);
                        cx.notify();
                    }
                },
            )
            .detach();
            cx.subscribe(
                &provider_path_input,
                |this: &mut Self, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Submit(_)) {
                        this.apply_provider_path_override(cx);
                    }
                },
            )
            .detach();

            // Like T3 Code's adapter subscriptions feeding its ingestion
            // worker, provider threads push an edge into this bounded wake
            // channel. The UI does no standing scan: the short follow-up tick
            // exists only to remeasure Markdown after a changed text frame.
            cx.spawn(async move |this, cx| {
                while event_wake_events.recv().await.is_ok() {
                    loop {
                        // The typed queues are drained below, so all wake edges
                        // already represented by those payloads can coalesce.
                        while event_wake_events.try_recv().is_ok() {}
                        let schedule = match this.update(cx, |this, cx| this.drain_event_pump(cx)) {
                            Ok(schedule) => schedule,
                            Err(_) => return,
                        };
                        match schedule {
                            EventPumpSchedule::Idle => break,
                            EventPumpSchedule::StreamFrame => {
                                // Deliberately not raced against the wake
                                // channel: waking per chunk made the notify
                                // rate equal the provider's chunk rate, and
                                // every notify is a full re-render. Chunks
                                // queue during the sleep and fold into the
                                // next drain's single batch.
                                cx.background_executor().timer(STREAM_FRAME_INTERVAL).await;
                            }
                            EventPumpSchedule::BackgroundOutput(delay) => {
                                // A log cache has its own 100 ms batching
                                // cadence. A new provider edge interrupts that
                                // wait; it must not wait behind log rendering.
                                futures_lite::future::race(
                                    async {
                                        let _ = event_wake_events.recv().await;
                                    },
                                    async {
                                        cx.background_executor().timer(delay).await;
                                    },
                                )
                                .await;
                            }
                        }
                    }
                }
            })
            .detach();

            // Maintenance clocks are intentionally independent of provider
            // ingestion and run at the slowest cadence their UI requires.
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(BACKGROUND_WORK_TICK_INTERVAL)
                        .await;
                    if this
                        .update(cx, |this, cx| this.maybe_refresh_background_work(cx))
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .detach();

            cx.spawn(async move |this, cx| {
                loop {
                    if this
                        .update(cx, |this, cx| this.maybe_refresh_plan_usage(cx))
                        .is_err()
                    {
                        break;
                    }
                    cx.background_executor()
                        .timer(PLAN_USAGE_MAINTENANCE_INTERVAL)
                        .await;
                }
            })
            .detach();

            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(IDLE_SESSION_SWEEP_INTERVAL)
                        .await;
                    if this
                        .update(cx, |this, _| this.reap_idle_sessions())
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .detach();

            let markdown_link_handler: md::render::LinkHandler = {
                let michelle = cx.entity().downgrade();
                Rc::new(move |target, _, cx| {
                    let handled = michelle
                        .update(cx, |michelle, cx| michelle.open_transcript_link(target, cx))
                        .unwrap_or(false);
                    if !handled {
                        cx.open_url(target);
                    }
                })
            };

            Self {
                session_ui: sessions::interactions::SessionUi {
                    user_input_answer: user_input_answer,
                    escape_stop_confirmation: EscapeStopConfirmation::default(),
                    computer_use_preview_position: None,
                },
                shell_ui: shell::ShellUi {
                    sidebar_visible: sidebar_visible,
                    sidebar_width: sidebar_width,
                    right_panel_visible: right_panel_visible,
                    right_panel_width: right_panel_width,
                    sidebar_slide: None,
                    right_panel_slide: None,
                    sidebar_rendered_width: if sidebar_visible { sidebar_width } else { 0.0 },
                    right_panel_rendered_width: if right_panel_visible {
                        right_panel_width
                    } else {
                        0.0
                    },
                    fps_counter_visible: false,
                    panel_resize_drag: None,
                    header_drag_armed: false,
                    menus: RefCell::new(HashMap::new()),
                    sidebar_pane: sidebar_pane.clone(),
                    transcript_pane: transcript_pane.clone(),
                    right_panel_pane: right_panel_pane.clone(),
                    fps_last_frame: Instant::now(),
                    fps_frame_count: 0,
                    fps_value: 0,
                },
                shell_model: shell::model::ShellModel {
                    home_directory: home_directory,
                    projectless_root: projectless_root,
                    open_in_apps: Rc::new(Vec::new()),
                    time_label_wake: Cell::new(None),
                    time_label_wake_generation: Cell::new(0),
                },
                notifications: notifications::model::NotificationsModel::new(startup_toast),
                notifications_ui: notifications::NotificationsUi {
                    selection: TranscriptSelection::default(),
                },
                goal_model: goal_dialog::model::GoalModel {
                    pending_operations: HashMap::new(),
                    runtime_starts: HashSet::new(),
                    observed_at: HashMap::new(),
                },
                goal_ui: goal_dialog::GoalUi {
                    dialog: None,
                    request: None,
                },
                image_model: image_preview::model::ImageModel::default(),
                image_ui: image_preview::ImageUi {
                    dialog: None,
                    generation: 0,
                },
                right_panel_model: right_panel::model::RightPanelModel {
                    session_states: HashMap::new(),
                    file_editors: HashMap::new(),
                    diff_source: ReviewDiffSource::default(),
                    diff_snapshot: None,
                    diff_loading: false,
                    diff_error: None,
                    diff_generation: 0,
                    working_tree: Vec::new(),
                    working_trees: QueryCache::new(MAX_CACHED_WORKSPACES),
                    workspace_queries_stale: false,
                    terminals: HashMap::new(),
                },
                right_panel_ui: right_panel::RightPanelUi {
                    surfaces: Vec::new(),
                    active_surface: None,
                    tabs_scroll_handle: ScrollHandle::new(),
                    files_scroll_handle: ScrollHandle::new(),
                    files_scrollbar: ScrollbarState::new(),
                    diff_filter: right_panel_diff_filter,
                    diff_list_state: ListState::new(0, ListAlignment::Top, px(512.0)),
                    diff_scrollbar: ScrollbarState::new(),
                    diff_selection: TranscriptSelection::default(),
                    diff_tree_list_state: ListState::new(0, ListAlignment::Top, px(180.0))
                        .with_uniform_item_height(px(30.0)),
                    diff_tree_scrollbar: ScrollbarState::new(),
                    editor_scroll_handle: ScrollHandle::new(),
                    editor_scrollbar: ScrollbarState::new(),
                    file_preview_markdown: RefCell::new(None),
                    file_preview_selection: TranscriptSelection::default(),
                    file_preview_scroll_handle: ScrollHandle::new(),
                    file_preview_scrollbar: ScrollbarState::new(),
                    pending_tab_reveal: None,
                    pending_terminal_focus: None,
                    expanded_paths: HashSet::new(),
                    files_selected_path: None,
                    file_tree_width: DEFAULT_FILE_TREE_WIDTH,
                    file_search: None,
                    diff_selected_file: None,
                    diff_expanded_paths: HashSet::new(),
                    diff_tree_rows: RefCell::new(Vec::new()),
                    diff_tree_cursor: None,
                },
                transcript_model: transcript_view::model::TranscriptModel {
                    row_kinds: RefCell::new(Vec::new()),
                    row_kinds_fingerprint: Cell::new(None),
                    navigation_turns: RefCell::new(Rc::new(Vec::new())),
                    navigation_turns_fingerprint: Cell::new(None),
                    assistant_footer_cache: RefCell::new(HashMap::new()),
                    assistant_footer_fingerprint: Cell::new(None),
                    message_markdown: RefCell::new(HashMap::new()),
                    activity_markdown: RefCell::new(HashMap::new()),
                    reasoning_window_starts: RefCell::new(HashMap::new()),
                    activity_diffs: RefCell::new(HashMap::new()),
                },
                transcript_ui: transcript_view::TranscriptUi {
                    activities_expanded: HashMap::new(),
                    expanded_activity_items: HashMap::new(),
                    expanded_turns: HashSet::new(),
                    expanded_changed_files: HashSet::new(),
                    control_focuses: RefCell::new(HashMap::new()),
                    copied_message_feedback: HashMap::new(),
                    copied_message_generation: 0,
                    copied_activity_feedback: HashMap::new(),
                    copied_activity_generation: 0,
                    message_edit: None,
                    rows: transcript_rows,
                    anchored_transcript_rows: anchored_transcript_rows,
                    hovered_response_row: None,
                    anchor: Cell::new(None),
                    anchor_end_space: Rc::new(Cell::new(Pixels::ZERO)),
                    anchor_following: transcript_anchor_following,
                    tail_recheck: transcript_tail_recheck,
                    is_scrolled: transcript_is_scrolled,
                    scroll_to_bottom_visible: Cell::new(false),
                    scrollbar_dragging: Cell::new(false),
                    layout_width: Cell::new(Pixels::ZERO),
                    user_message_viewports: RefCell::new(HashMap::new()),
                    activity_scroll_viewports: RefCell::new(HashMap::new()),
                    activity_diff_viewports: RefCell::new(HashMap::new()),
                    markdown_link_handler: markdown_link_handler,
                    selection: TranscriptSelection::default(),
                    focus: cx.focus_handle(),
                    search: None,
                    scrollbar: ScrollbarState::new(),
                    navigation_rail: navigation_rail.clone(),
                    navigation_rail_reset_generation: Cell::new(0),
                },
                sidebar_model: sidebar::model::SidebarModel {
                    rows_fingerprint: Cell::new(None),
                    rows_snapshot: RefCell::new(Rc::new(Vec::new())),
                    projectless_projects: RefCell::new(HashSet::new()),
                    session_moves: HashSet::new(),
                    pending_session_drops: HashMap::new(),
                    working_headers: RefCell::new(HashSet::new()),
                },
                sidebar_ui: sidebar::SidebarUi {
                    onboarding_add_project_focus: onboarding_add_project_focus,
                    onboarding_projectless_focus: onboarding_projectless_focus,

                    session_rename: None,
                    project_group_rename: None,
                    session_rename_input: session_rename_input,
                    collapsed_groups: sidebar_collapsed_groups,
                    project_reveal_counts: HashMap::new(),
                    group_header_focuses: RefCell::new(HashMap::new()),
                    show_more_focuses: RefCell::new(HashMap::new()),
                    list_state: sidebar_list_state,
                    scrollbar: ScrollbarState::new(),
                    row_cache: RefCell::new(Vec::new()),
                    drag_preview: Rc::default(),
                    reorder_animation: RefCell::new(None),
                },
                composer_model: composer::model::ComposerModel {
                    drafts: composer_drafts,
                    draft_store: composer_draft_store,
                    draft_save_generation: 0,
                    attachments: composer_attachments,
                    sources: composer::sources::ComposerSources {
                        slash_commands: QueryCache::new(2 * MAX_CACHED_WORKSPACES),
                        slash_command_index: Rc::new(Vec::new()),
                        slash_command_index_key: None,
                        slash_command_index_loading: false,
                        mention_files: QueryCache::new(MAX_CACHED_WORKSPACES),
                        mention_file_index: Rc::new(Vec::new()),
                        mention_file_index_path: None,
                        mention_file_index_loading: false,
                        stale: false,
                    },
                },
                composer_ui: composer::ComposerUi {
                    input: composer,
                    autocomplete: autocomplete::AutocompleteUi::new(),
                },
                sessions: sessions::model::SessionModel {
                    activation: sessions::activation::SessionActivation {
                        hydrations: HashSet::new(),
                        pending: None,
                        navigation: session_navigation,
                    },
                    runtime: runtime::model::RuntimeModel {
                        last_idle_session_sweep: Instant::now(),
                        event_wake_tx,
                        task_state_sync_tx,
                        task_state_sync_events,
                        runtimes: HashMap::new(),
                        runtime_attach_pending: HashSet::new(),
                        runtime_attach_misses: HashMap::new(),
                        background_work: HashMap::new(),
                        last_background_work_tick: Instant::now(),
                        submission_preparations: HashSet::new(),
                        response_fork_preparations: HashMap::new(),
                        pending_queue_drains: Vec::new(),
                        stream_state_dirty: false,
                        last_stream_save: Instant::now(),
                    },
                    checkpoints: sessions::checkpoints::CheckpointModel {
                        refs: RefCell::new(HashMap::new()),
                        generation: Cell::new(0),
                        prefetch: Cell::new(None),
                        pending: interrupted_turn_checkpoints,
                        in_flight: HashSet::new(),
                    },
                },
                model_picker_ui: model_picker::ModelPickerUi::new(
                    model_search,
                    model_picker_favorite_focus,
                    model_picker_empty_focus,
                ),
                branch_picker_ui: branch_picker::BranchPickerUi::new(
                    branch_search,
                    branch_create_input,
                    branch_picker_list_state,
                ),
                branches: branch_picker::model::BranchModel::new(),
                project_picker_ui: project_picker::ProjectPickerUi::new(
                    project_search,
                    project_picker_list_state,
                ),
                project_picker: project_picker::model::ProjectPickerModel::new(),
                daemon,
                settings: settings::model::SettingsModel {
                    daemon_hostname,
                    themes: Rc::new(vec![ThemeDefinition::default()]),
                    daemon_reconfigure_pending: false,
                    providers: settings::providers::ProviderCatalog {
                        probes,
                        probe_tx: provider_probe_tx,
                        probe_events: provider_probe_events,
                        model_discoveries: HashSet::new(),
                        model_discoveries_pending: HashSet::new(),
                        versions: HashMap::new(),
                        version_tx: provider_version_tx,
                        version_events: provider_version_events,
                        version_probes_pending: HashSet::new(),
                        detection_tx: provider_detection_tx,
                        detection_events: provider_detection_events,
                        detection_remaining: 0,
                        detection_checked_at: None,
                    },
                    permissions: settings::permissions::PermissionState {
                        snapshot: ComputerPermissions::default(),
                        tx: computer_permission_tx,
                        events: computer_permission_events,
                        request_pending: false,
                        app_icons: RefCell::new(HashMap::new()),
                        app_icon_loads: RefCell::new(HashSet::new()),
                    },
                    plan: settings::plan_usage::PlanUsageState {
                        snapshots: HashMap::new(),
                        errors: HashMap::new(),
                        tx: plan_usage_tx,
                        events: plan_usage_events,
                        pending: HashSet::new(),
                        unconfigured: HashSet::new(),
                        checked_at: HashMap::new(),
                        stale: HashSet::new(),
                    },
                },
                settings_ui: settings::SettingsUi {
                    page: None,
                    search: settings_search,
                    daemon_port_input,
                    daemon_origins_input,
                    daemon_token_revealed: false,
                    focus: settings_focus,
                    sidebar_focuses: RefCell::new(HashMap::new()),
                    expanded_provider: None,
                    provider_path_input,
                    scroll: ScrollHandle::new(),
                    scrollbar: ScrollbarState::new(),
                },

                state,
                store,
                command_palette: command_palette::CommandPaletteUi::new(command_palette_search),
                task_switcher,
                usage: usage_page::model::UsageModel::default(),
                usage_ui: usage_page::UsageUi::new(usage_project_filter, usage_projects_list),
                commit_dialog: None,
                commit: commit_dialog::model::CommitModel::default(),
                // Providers × workspaces; both scans are small, the cache
                // only exists to keep them off the frame path.
                skills: skills_page::model::SkillsModel::default(),
                skills_ui: skills_page::SkillsUi::new(skills_search),
            }
        });
        navigation_rail.update(cx, |rail, _| rail.set_michelle(entity.downgrade()));
        for pane in [&sidebar_pane, &transcript_pane, &right_panel_pane] {
            pane.update(cx, |pane, cx| pane.bind(&entity, cx));
        }
        let initial_row_count = entity.read(cx).transcript_row_count();
        entity.read(cx).reset_transcript_rows(initial_row_count);
        // Everything launch needs from `git` or the filesystem, started now
        // that there is an entity to notify and deliberately not before the
        // first frame.
        entity.update(cx, |this, cx| {
            this.load_themes(window, cx);
            this.restart_task_state_sync();
            for session_id in startup_live_session_ids {
                this.start_runtime_attachment(session_id, cx);
            }
            this.start_pending_checkpoint_captures(cx);
            // The autocomplete indexes prefetch alongside, so typing `/` or
            // `@` into the very first prompt already has data to draw.
            this.refresh_composer_sources(cx);
            // Re-detect providers after resolving the user's login-shell
            // environment off-thread. Detection then starts model and version
            // discovery for every CLI it finds, including nvm/fnm-managed
            // installs.
            this.refresh_provider_detection(None);
            // The skill library too: the Skills settings page must open onto
            // data, not a scan.
            this.ensure_skills_catalog(false, cx);
            // And the header's "open project in app" targets, so its menu
            // lists installed apps and icons without ever probing on a frame.
            this.detect_open_in_apps(cx);
        });
        entity
    }
}

fn migrate_legacy_projectless_projects(
    state: &mut PersistedState,
    workspace: &michelle_client::WorkspaceClient,
) -> (bool, Option<anyhow::Error>) {
    let legacy_indices = state
        .projects
        .iter()
        .enumerate()
        .filter_map(|(index, project)| {
            crate::projectless::needs_migration(&project.path).then_some(index)
        })
        .collect::<Vec<_>>();
    if legacy_indices.is_empty() {
        return (false, None);
    }

    let mut changed = false;
    for index in legacy_indices {
        let path = state.projects[index].path.clone();
        let response = workspace
            .request(michelle_client::WorkspaceOperation::MigrateProjectlessWorkspace { path });
        let cwd = match response {
            Ok(michelle_client::WorkspaceResult::ProjectlessWorkspace { cwd }) => cwd,
            Ok(_) => {
                return (
                    changed,
                    Some(anyhow::anyhow!(
                        "the daemon returned an invalid projectless response"
                    )),
                );
            }
            Err(error) => return (changed, Some(error)),
        };
        state.projects[index].name = Project::PROJECTLESS_NAME.to_owned();
        state.projects[index].path = cwd;
        changed = true;
    }
    (changed, None)
}
