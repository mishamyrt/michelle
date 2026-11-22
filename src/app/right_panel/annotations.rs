//! Discussions belong to the captured diff, independently of the live worktree.
use super::*;
use crate::app::runtime::workspace_ack;
use michelle_client::workspace::{ReviewDiffData, ReviewFixGuard};
use serde::{Deserialize, Serialize};

mod view;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Side {
    Old,
    New,
}

impl Side {
    fn number(self, line: &crate::review_diff::Line) -> Option<u32> {
        match self {
            Self::Old => line.old_line,
            Self::New => line.new_line,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Anchor {
    path: String,
    side: Side,
    start: u32,
    end: u32,
}

#[derive(Clone, Deserialize, Serialize)]
struct ReviewMessage {
    agent: bool,
    body: String,
}

#[derive(Clone, Deserialize, Serialize)]
struct Discussion {
    id: Uuid,
    anchor: Option<Anchor>,
    messages: Vec<ReviewMessage>,
    draft: bool,
    dismissed: bool,
    fix_turn: Option<Uuid>,
}

impl Discussion {
    fn open(&self) -> bool {
        !self.draft && !self.dismissed && self.fix_turn.is_none()
    }
}

#[derive(Clone, Deserialize, Serialize)]
struct SavedReview {
    id: Uuid,
    cwd: PathBuf,
    data: Arc<ReviewDiffData>,
    discussions: Vec<Discussion>,
    #[serde(default)]
    agent_finished: bool,
}

#[derive(Deserialize)]
struct ReviewSnapshot {
    id: Uuid,
    cwd: PathBuf,
}

impl ReviewSnapshot {
    fn read(path: &Path) -> anyhow::Result<Option<Self>> {
        match std::fs::File::open(path) {
            Ok(file) => Ok(Some(serde_json::from_reader(std::io::BufReader::new(
                file,
            ))?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn set_refs(
        self,
        workspace: &michelle_client::WorkspaceClient,
        revisions: Option<[String; 2]>,
    ) -> anyhow::Result<()> {
        workspace_ack(
            workspace,
            michelle_client::WorkspaceOperation::SetReviewSnapshotRefs {
                cwd: self.cwd,
                review_id: self.id,
                revisions,
            },
        )
    }
}

#[derive(Default)]
struct ReviewWriteState {
    version: u64,
    pinned_review: Option<Uuid>,
}

#[derive(Default)]
struct ReviewIndex {
    lines: HashMap<String, HashMap<(Side, u32), Vec<usize>>>,
    summary: Vec<usize>,
    open: usize,
    has_agent_findings: bool,
    open_per_file: HashMap<String, usize>,
}

pub(in crate::app) struct ReviewDocument {
    saved: SavedReview,
    index: ReviewIndex,
    request_pending: bool,
    bulk: Option<HashSet<Uuid>>,
    save_version: u64,
    write_state: Arc<parking_lot::Mutex<ReviewWriteState>>,
}

impl ReviewDocument {
    fn new(saved: SavedReview) -> Self {
        let mut document = Self {
            saved,
            index: ReviewIndex::default(),
            request_pending: false,
            bulk: None,
            save_version: 0,
            write_state: Arc::default(),
        };
        document.reindex();
        document
    }

    fn reindex(&mut self) {
        let mut index = ReviewIndex::default();
        for (i, discussion) in self.saved.discussions.iter().enumerate() {
            index.has_agent_findings |= discussion
                .messages
                .first()
                .is_some_and(|message| message.agent);
            if discussion.open() {
                index.open += 1;
                if let Some(anchor) = &discussion.anchor {
                    *index.open_per_file.entry(anchor.path.clone()).or_default() += 1;
                }
            }
            if let Some(anchor) = &discussion.anchor {
                index
                    .lines
                    .entry(anchor.path.clone())
                    .or_default()
                    .entry((anchor.side, anchor.end))
                    .or_default()
                    .push(i);
            } else {
                index.summary.push(i);
            }
        }
        self.index = index;
    }
}

pub(in crate::app) struct ReviewEditor {
    anchor: Option<Anchor>,
    discussion: Option<Uuid>,
    input: Entity<TextInput>,
}

#[derive(Deserialize)]
struct Finding {
    #[serde(flatten)]
    anchor: Anchor,
    body: String,
}

#[derive(Deserialize)]
struct Findings {
    findings: Vec<Finding>,
}

#[derive(Deserialize)]
struct Answer {
    answer: String,
}

enum AgentReviewResult {
    Findings(Vec<Discussion>),
    Answer(String),
}

#[derive(Clone, Debug)]
pub(in crate::app) struct ReviewFix {
    review: Uuid,
    discussions: Vec<Uuid>,
    pub(in crate::app) guard: ReviewFixGuard,
}

impl Michelle {
    pub(super) fn review_open_count(&self, path: &str) -> usize {
        self.active_review()
            .and_then(|review| review.index.open_per_file.get(path))
            .copied()
            .unwrap_or_default()
    }

    pub(super) fn first_open_review_row(
        &self,
        snapshot: &ReviewDiffSnapshot,
        file: &crate::review_diff::File,
    ) -> Option<usize> {
        let review = self.active_review()?;
        let anchors = review.index.lines.get(&file.path)?;
        let start = file.diff_line? + 1;
        snapshot
            .lines
            .iter()
            .enumerate()
            .skip(start)
            .take_while(|(_, line)| !matches!(line.kind, crate::review_diff::LineKind::FileHeader))
            .find_map(|(index, line)| {
                let found = [(Side::Old, line.old_line), (Side::New, line.new_line)]
                    .into_iter()
                    .filter_map(|(side, number)| anchors.get(&(side, number?)))
                    .flatten()
                    .any(|&discussion| review.saved.discussions[discussion].open());
                found.then_some(index)
            })
    }

    fn active_review(&self) -> Option<&ReviewDocument> {
        let session = self.state.selected_session?;
        self.right_panel_model.review_open.then_some(())?;
        self.right_panel_model.reviews.get(&session)
    }

    fn active_review_mut(&mut self) -> Option<&mut ReviewDocument> {
        let session = self.state.selected_session?;
        self.right_panel_model.review_open.then_some(())?;
        self.right_panel_model.reviews.get_mut(&session)
    }

    fn review_path(&self, session: Uuid) -> PathBuf {
        self.store
            .path()
            .with_file_name("reviews")
            .join(format!("{session}.json"))
    }

    fn persist_review(&mut self, session: Uuid, cx: &mut Context<Self>) {
        let path = self.review_path(session);
        let Some(document) = self.right_panel_model.reviews.get_mut(&session) else {
            return;
        };
        document.reindex();
        document.save_version += 1;
        let version = document.save_version;
        let write_state = document.write_state.clone();
        let saved = document.saved.clone();
        let workspace = michelle_client::WorkspaceClient::new(self.daemon.client());
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut written = write_state.lock();
                    if written.version >= version {
                        return Ok(());
                    }
                    let previous = if written.pinned_review != Some(saved.id) {
                        let previous = ReviewSnapshot::read(&path)?;
                        ReviewSnapshot {
                            id: saved.id,
                            cwd: saved.cwd.clone(),
                        }
                        .set_refs(
                            &workspace,
                            Some([saved.data.from.clone(), saved.data.to.clone()]),
                        )?;
                        previous.filter(|previous| previous.id != saved.id)
                    } else {
                        None
                    };
                    michelle_client::persistence::write_json_atomically(&path, &saved)?;
                    written.version = version;
                    written.pinned_review = Some(saved.id);
                    if let Some(previous) = previous {
                        previous.set_refs(&workspace, None)?;
                    }
                    Ok::<_, anyhow::Error>(())
                })
                .await;
            if let Err(error) = result {
                let _ = this.update(cx, |this, cx| {
                    this.show_toast(tr!("review.save_failed", error = error));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(super) fn remove_saved_review(&mut self, session: Uuid, cx: &mut Context<Self>) {
        let path = self.review_path(session);
        let write_state = self
            .right_panel_model
            .reviews
            .remove(&session)
            .map(|review| review.write_state)
            .unwrap_or_default();
        let workspace = michelle_client::WorkspaceClient::new(self.daemon.client());
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut written = write_state.lock();
                    written.version = u64::MAX;
                    if let Some(snapshot) = ReviewSnapshot::read(&path)? {
                        snapshot.set_refs(&workspace, None)?;
                        std::fs::remove_file(path)?;
                    }
                    Ok::<_, anyhow::Error>(())
                })
                .await;
            if let Err(error) = result {
                let _ = this.update(cx, |this, cx| {
                    this.show_toast(tr!("review.save_failed", error = error));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn persist_active_review(&mut self, cx: &mut Context<Self>) {
        if let Some(session) = self.state.selected_session {
            self.persist_review(session, cx);
        }
        cx.notify();
    }

    fn start_review(&mut self, cx: &mut Context<Self>) {
        if self.right_panel_model.diff_loading {
            return;
        }
        let Some(session) = self.state.selected_session else {
            return;
        };
        let Some(cwd) = self.selected_workspace_path().map(Path::to_path_buf) else {
            return;
        };
        let Some(data) = self
            .right_panel_model
            .diff_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.data.clone())
        else {
            return;
        };
        let mut document = ReviewDocument::new(SavedReview {
            id: Uuid::new_v4(),
            cwd,
            data,
            discussions: Vec::new(),
            agent_finished: false,
        });
        if let Some(previous) = self.right_panel_model.reviews.get(&session) {
            document.save_version = previous.save_version;
            document.write_state = previous.write_state.clone();
        }
        self.right_panel_model.reviews.insert(session, document);
        self.right_panel_model.review_open = true;
        self.persist_active_review(cx);
    }

    fn resume_review(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.state.selected_session else {
            return;
        };
        let Some(cwd) = self.selected_workspace_path().map(Path::to_path_buf) else {
            return;
        };
        let expected = self.right_panel_model.diff_generation;
        let path = self.review_path(session);
        let memory = self
            .right_panel_model
            .reviews
            .get(&session)
            .map(|review| review.saved.clone());
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let saved = match memory {
                        Some(saved) => saved,
                        None => serde_json::from_slice::<SavedReview>(&std::fs::read(path)?)?,
                    };
                    anyhow::ensure!(saved.cwd == cwd, "review belongs to another workspace");
                    let snapshot = Arc::new(crate::review_diff::from_workspace(saved.data.clone()));
                    Ok::<_, anyhow::Error>((saved, snapshot))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.state.selected_session != Some(session)
                    || this.right_panel_model.diff_generation != expected
                    || this.right_panel_model.review_open
                {
                    return;
                }
                match result {
                    Ok((saved, snapshot)) => {
                        let loaded_from_disk =
                            !this.right_panel_model.reviews.contains_key(&session);
                        this.right_panel_model
                            .reviews
                            .entry(session)
                            .or_insert_with(|| ReviewDocument::new(saved));
                        this.right_panel_model.review_open = true;
                        this.right_panel_model.diff_generation =
                            this.right_panel_model.diff_generation.wrapping_add(1);
                        this.right_panel_model.diff_source = snapshot.source.clone();
                        this.right_panel_model.diff_loading = false;
                        this.right_panel_model.diff_error = None;
                        this.right_panel_ui.diff_selection.clear();
                        this.right_panel_ui.diff_selected_file =
                            (!snapshot.files.is_empty()).then_some(0);
                        this.right_panel_ui.diff_expanded_paths =
                            super::review::review_diff_directory_paths(&snapshot.files);
                        this.right_panel_ui
                            .diff_list_state
                            .reset(snapshot.lines.len());
                        this.right_panel_model.diff_snapshot = Some(snapshot);
                        this.sync_right_panel_diff_tree_rows(cx);
                        if loaded_from_disk {
                            this.persist_review(session, cx);
                        }
                    }
                    Err(_) => this.show_toast(tr!("review.unavailable")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn close_review(&mut self, cx: &mut Context<Self>) {
        self.save_review_editor(cx);
        self.right_panel_model.review_open = false;
        self.right_panel_model.review_selection = None;
    }

    fn select_review_line(&mut self, index: usize, extend: bool, cx: &mut Context<Self>) {
        let start = self
            .right_panel_model
            .review_selection
            .filter(|_| extend)
            .map_or(index, |(start, _)| start);
        let Some(snapshot) = &self.right_panel_model.diff_snapshot else {
            return;
        };
        if selection_anchor(snapshot, start, index).is_none() {
            return;
        }
        self.right_panel_model.review_selection = Some((start, index));
        cx.notify();
    }

    fn edit_review_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((start, end)) = self.right_panel_model.review_selection else {
            return;
        };
        let Some(anchor) = self
            .right_panel_model
            .diff_snapshot
            .as_ref()
            .and_then(|snapshot| selection_anchor(snapshot, start, end))
        else {
            return;
        };
        self.right_panel_ui
            .diff_list_state
            .scroll_to_reveal_item(start.max(end));
        self.edit_review_comment(Some(anchor), None, window, cx);
    }

    fn edit_review_comment(
        &mut self,
        anchor: Option<Anchor>,
        discussion: Option<Uuid>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.save_review_editor(cx);
        let input = cx.new(|cx| {
            TextInput::new(window, cx)
                .multi_line()
                .auto_height()
                .placeholder(tr!("review.comment_placeholder"))
        });
        input.read(cx).focus().focus(window, cx);
        self.right_panel_model.review_editor = Some(ReviewEditor {
            anchor,
            discussion,
            input,
        });
        self.right_panel_ui.diff_list_state.remeasure();
        cx.notify();
    }

    pub(in crate::app) fn save_review_editor(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.right_panel_model.review_editor.take() else {
            return;
        };
        let body = editor.input.read(cx).content().trim().to_owned();
        if !body.is_empty()
            && let Some(review) = self.active_review_mut()
        {
            let message = ReviewMessage { agent: false, body };
            if let Some(id) = editor.discussion {
                if let Some(thread) = review
                    .saved
                    .discussions
                    .iter_mut()
                    .find(|item| item.id == id)
                {
                    thread.messages.push(message);
                    thread.draft = true;
                }
            } else {
                review.saved.discussions.push(Discussion {
                    id: Uuid::new_v4(),
                    anchor: editor.anchor,
                    messages: vec![message],
                    draft: true,
                    dismissed: false,
                    fix_turn: None,
                });
            }
            self.persist_active_review(cx);
        }
        self.right_panel_ui.diff_list_state.remeasure();
        cx.notify();
    }

    fn edit_existing_discussion(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let Some(anchor) = self
            .active_review()
            .and_then(|review| {
                review
                    .saved
                    .discussions
                    .iter()
                    .find(|thread| thread.id == id)
            })
            .map(|thread| thread.anchor.clone())
        else {
            return;
        };
        self.edit_review_comment(anchor, Some(id), window, cx);
    }

    fn update_review_discussion(
        &mut self,
        id: Uuid,
        update: impl FnOnce(&mut Discussion),
        cx: &mut Context<Self>,
    ) {
        if let Some(item) = self.active_review_mut().and_then(|review| {
            review
                .saved
                .discussions
                .iter_mut()
                .find(|item| item.id == id)
        }) {
            update(item);
            self.right_panel_ui.diff_list_state.remeasure();
            self.persist_active_review(cx);
        }
    }

    fn toggle_review_bulk(&mut self, cx: &mut Context<Self>) {
        if let Some(review) = self.active_review_mut() {
            review.bulk = review.bulk.is_none().then(|| {
                review
                    .saved
                    .discussions
                    .iter()
                    .filter(|thread| thread.open())
                    .map(|thread| thread.id)
                    .collect()
            });
            self.right_panel_ui.diff_list_state.remeasure();
            cx.notify();
        }
    }

    fn toggle_review_bulk_item(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if let Some(selected) = self
            .active_review_mut()
            .and_then(|review| review.bulk.as_mut())
        {
            if !selected.remove(&id) {
                selected.insert(id);
            }
            cx.notify();
        }
    }

    fn request_agent_review(&mut self, discussion: Option<Uuid>, cx: &mut Context<Self>) {
        self.save_review_editor(cx);
        let Some(session) = self.selected_session() else {
            return;
        };
        let session_id = session.id;
        let model = (session.provider == ProviderKind::Codex)
            .then(|| session.model.clone())
            .flatten();
        let binary_override = self
            .state
            .provider_binary_overrides
            .get(&ProviderKind::Codex)
            .cloned();
        let Some(snapshot) = self.right_panel_model.diff_snapshot.clone() else {
            return;
        };
        let Some(review) = self.active_review_mut() else {
            return;
        };
        if review.request_pending {
            return;
        }
        let (question, answered_messages) = if let Some(id) = discussion {
            let Some(thread) = review
                .saved
                .discussions
                .iter()
                .find(|thread| thread.id == id && !thread.dismissed)
            else {
                return;
            };
            let mut text = String::from(
                "Answer the user's latest message in this review discussion. Reply in the user's language.\n",
            );
            append_discussion(&mut text, thread, &snapshot);
            (Some(text), thread.messages.len())
        } else {
            (None, 0)
        };
        let review_id = review.saved.id;
        review.request_pending = true;
        let operation = michelle_client::WorkspaceOperation::ReviewWithCodex {
            cwd: review.saved.cwd.clone(),
            binary_override,
            model,
            from: review.saved.data.from.clone(),
            to: review.saved.data.to.clone(),
            question,
        };
        let workspace = michelle_client::WorkspaceClient::new(self.daemon.client());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let michelle_client::WorkspaceResult::ReviewResponse { response } =
                        workspace.request(operation)?
                    else {
                        anyhow::bail!("invalid review response");
                    };
                    if discussion.is_some() {
                        let answer: Answer = serde_json::from_str(&response)?;
                        anyhow::ensure!(!answer.answer.trim().is_empty(), "empty review reply");
                        Ok(AgentReviewResult::Answer(answer.answer))
                    } else {
                        let findings: Findings = serde_json::from_str(&response)?;
                        Ok(AgentReviewResult::Findings(
                            findings
                                .findings
                                .into_iter()
                                .filter(|finding| !finding.body.trim().is_empty())
                                .map(|finding| model_discussion(finding, &snapshot))
                                .collect(),
                        ))
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let Some(review) = this
                    .right_panel_model
                    .reviews
                    .get_mut(&session_id)
                    .filter(|review| review.saved.id == review_id)
                else {
                    return;
                };
                review.request_pending = false;
                match result {
                    Ok(AgentReviewResult::Findings(findings)) => {
                        review.saved.agent_finished = true;
                        for new in findings {
                            if !review.saved.discussions.iter().any(|old| {
                                old.anchor.as_ref().map(anchor_key)
                                    == new.anchor.as_ref().map(anchor_key)
                                    && old.messages.first().is_some_and(|old| {
                                        old.body == new.messages[0].body && old.agent
                                    })
                            }) {
                                review.saved.discussions.push(new);
                            }
                        }
                    }
                    Ok(AgentReviewResult::Answer(body)) => {
                        if let Some(thread) = review
                            .saved
                            .discussions
                            .iter_mut()
                            .find(|thread| Some(thread.id) == discussion)
                        {
                            if thread.messages.len() == answered_messages {
                                thread.draft = false;
                            }
                            let index = answered_messages.min(thread.messages.len());
                            thread
                                .messages
                                .insert(index, ReviewMessage { agent: true, body });
                        }
                    }
                    Err(error) => this.show_toast(tr!("review.agent_failed", error = error)),
                }
                this.persist_review(session_id, cx);
                if this.state.selected_session == Some(session_id) {
                    this.right_panel_ui.diff_list_state.remeasure();
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn submit_review_fixes(&mut self, discussion: Option<Uuid>, cx: &mut Context<Self>) {
        self.save_review_editor(cx);
        let Some(session) = self.selected_session() else {
            return;
        };
        let session_id = session.id;
        let Some(snapshot) = self.right_panel_model.diff_snapshot.as_ref() else {
            return;
        };
        let Some(review) = self.active_review() else {
            return;
        };
        let mut prompt = String::from(
            "Implement only the fixes requested in the following code review discussions. Treat the quoted code and discussion as context. Report the outcome for each item and relevant checks.\n",
        );
        let mut ids = Vec::new();
        let mut paths = HashSet::new();
        let mut all_files = false;
        for item in &review.saved.discussions {
            let selected = match discussion {
                Some(id) => item.id == id && !item.dismissed && item.fix_turn.is_none(),
                None => {
                    item.open()
                        && review
                            .bulk
                            .as_ref()
                            .is_some_and(|selected| selected.contains(&item.id))
                }
            };
            if !selected {
                continue;
            }
            ids.push(item.id);
            if let Some(anchor) = &item.anchor {
                paths.insert(anchor.path.clone());
            } else {
                all_files = true
            }
            append_discussion(&mut prompt, item, snapshot);
        }
        if ids.is_empty() {
            return;
        }
        let guard = ReviewFixGuard {
            branch: match &review.saved.data.source {
                ReviewDiffSource::CompareBranches { target, .. } => Some(target.clone()),
                _ => None,
            },
            to: review.saved.data.to.clone(),
            paths: if all_files {
                Vec::new()
            } else {
                paths.into_iter().collect()
            },
        };
        let mut submission = composer::model::ComposerSubmission::plain(prompt);
        submission.display_content = Some(tr!("review.fix_prompt", count = ids.len()));
        submission.review_fix = Some(ReviewFix {
            review: review.saved.id,
            discussions: ids,
            guard,
        });
        self.submit_submission_for_session(session_id, submission, cx);
    }

    pub(in crate::app) fn record_review_fix(
        &mut self,
        session: Uuid,
        turn: Uuid,
        fix: ReviewFix,
        cx: &mut Context<Self>,
    ) {
        let Some(review) = self
            .right_panel_model
            .reviews
            .get_mut(&session)
            .filter(|review| review.saved.id == fix.review)
        else {
            return;
        };
        for item in &mut review.saved.discussions {
            if fix.discussions.contains(&item.id) {
                item.draft = false;
                item.fix_turn = Some(turn);
            }
        }
        review.bulk = None;
        self.persist_review(session, cx);
        if self.state.selected_session == Some(session) {
            self.right_panel_ui.diff_list_state.remeasure();
        }
    }

    fn open_review_fix_diff(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if self.selected_session().is_some_and(|session| {
            session.turns.iter().any(|turn| {
                turn.id == id
                    && turn
                        .checkpoint
                        .as_ref()
                        .is_some_and(|checkpoint| checkpoint.status == CheckpointStatus::Ready)
            })
        }) {
            self.open_turn_diff(id, cx);
        } else {
            self.show_toast(tr!("review.fix_not_ready"));
            cx.notify();
        }
    }
}

fn anchor_key(anchor: &Anchor) -> (&str, Side, u32, u32) {
    (&anchor.path, anchor.side, anchor.start, anchor.end)
}

fn selection_anchor(snapshot: &ReviewDiffSnapshot, a: usize, b: usize) -> Option<Anchor> {
    let first = snapshot.lines.get(a.min(b))?;
    let last = snapshot.lines.get(a.max(b))?;
    if first.file_index != last.file_index {
        return None;
    }
    let side = if first.new_line.is_some() && last.new_line.is_some() {
        Side::New
    } else {
        Side::Old
    };
    Some(Anchor {
        path: snapshot.files.get(first.file_index)?.path.clone(),
        side,
        start: side.number(first)?,
        end: side.number(last)?,
    })
}

fn model_discussion(finding: Finding, snapshot: &ReviewDiffSnapshot) -> Discussion {
    let Finding { anchor, body } = finding;
    let file = snapshot.files.iter().find(|file| file.path == anchor.path);
    let lines = file.and_then(|file| file.diff_line).map(|start| {
        snapshot
            .lines
            .iter()
            .skip(start + 1)
            .take_while(|line| !matches!(line.kind, crate::review_diff::LineKind::FileHeader))
    });
    let valid = anchor.start != 0
        && anchor.start <= anchor.end
        && lines.is_some_and(|lines| {
            let mut start = false;
            let mut end = false;
            for line in lines {
                let number = anchor.side.number(line);
                start |= number == Some(anchor.start);
                end |= number == Some(anchor.end);
                if start && end {
                    return true;
                }
            }
            false
        });
    let (anchor, body) = if valid {
        (Some(anchor), body)
    } else {
        (
            None,
            format!("{}:{}–{}\n\n{body}", anchor.path, anchor.start, anchor.end),
        )
    };
    Discussion {
        id: Uuid::new_v4(),
        anchor,
        messages: vec![ReviewMessage { agent: true, body }],
        draft: false,
        dismissed: false,
        fix_turn: None,
    }
}

fn append_discussion(out: &mut String, discussion: &Discussion, snapshot: &ReviewDiffSnapshot) {
    use std::fmt::Write as _;
    if let Some(anchor) = &discussion.anchor {
        let side = match anchor.side {
            Side::Old => "old",
            Side::New => "new",
        };
        let _ = writeln!(
            out,
            "\n{}:{}–{} ({side}):",
            anchor.path, anchor.start, anchor.end
        );
        if let Some(start) = snapshot
            .files
            .iter()
            .find(|file| file.path == anchor.path)
            .and_then(|file| file.diff_line)
        {
            for line in snapshot
                .lines
                .iter()
                .skip(start + 1)
                .take_while(|line| !matches!(line.kind, crate::review_diff::LineKind::FileHeader))
                .filter(|line| {
                    anchor
                        .side
                        .number(line)
                        .is_some_and(|number| (anchor.start..=anchor.end).contains(&number))
                })
                .take(80)
            {
                let _ = writeln!(out, "    {}", line.content);
            }
        }
    }
    for message in &discussion.messages {
        let speaker = if message.agent { "Reviewer" } else { "User" };
        let _ = writeln!(out, "{speaker}: {}", message.body);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_anchors_distinguish_added_and_deleted_lines() {
        let snapshot = crate::review_diff::parse_collected(
            ReviewDiffSource::Uncommitted,
            "2\t1\tfile.rs\n",
            "diff --git a/file.rs b/file.rs\n--- a/file.rs\n+++ b/file.rs\n@@ -1 +1,2 @@\n-old\n+first\n+second\n",
            true,
        );
        let row = |text| {
            snapshot
                .lines
                .iter()
                .position(|line| line.content == text)
                .unwrap()
        };
        let old = selection_anchor(&snapshot, row("old"), row("old")).unwrap();
        let new = selection_anchor(&snapshot, row("second"), row("first")).unwrap();
        assert_eq!(anchor_key(&old), ("file.rs", Side::Old, 1, 1));
        assert_eq!(anchor_key(&new), ("file.rs", Side::New, 1, 2));
        assert!(selection_anchor(&snapshot, row("old"), row("first")).is_none());

        let response: Findings = serde_json::from_str(
            r#"{"findings":[{"path":"file.rs","side":"new","start":1,"end":2,"body":"Valid"},{"path":"file.rs","side":"new","start":2,"end":99,"body":"Unmatched"}]}"#,
        ).unwrap();
        let mut findings = response
            .findings
            .into_iter()
            .map(|finding| model_discussion(finding, &snapshot));
        assert_eq!(
            findings.next().unwrap().anchor.as_ref().map(anchor_key),
            Some(anchor_key(&new))
        );
        assert!(findings.next().unwrap().anchor.is_none());
    }
}
