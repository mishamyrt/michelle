//! Virtual list state, anchoring and row remeasurement.
use super::*;

impl Michelle {
    pub(in crate::app) fn active_transcript_rows(&self) -> &ListState {
        if self.transcript_ui.anchor.get().is_some() {
            &self.transcript_ui.anchored_transcript_rows
        } else {
            &self.transcript_ui.rows
        }
    }

    /// Turn a tail-pinned list into an explicit scroll position before a
    /// disclosure changes the document height. Otherwise GPUI keeps the
    /// bottom edge fixed and makes the disclosure header jump upward while
    /// its newly visible content is inserted.
    pub(in crate::app) fn pin_transcript_for_disclosure(&self) {
        self.sync_transcript_rows();
        let transcript_rows = self.active_transcript_rows();
        let count = transcript_rows.item_count();
        let scroll_top = transcript_rows.logical_scroll_top();

        if scroll_top.item_ix >= count && count > 0 {
            let viewport_height = transcript_rows.viewport_bounds().size.height;
            let actual_max = transcript_rows.max_offset_for_scrollbar().y;
            if actual_max > px(0.5) {
                // GPUI represents the exact bottom as an implicit tail anchor.
                // Resolve the corresponding item just above the bottom, then
                // restore the final half pixel with scroll_to so the same
                // position remains explicit while rows below it grow.
                transcript_rows
                    .set_offset_from_scrollbar(point(Pixels::ZERO, -(actual_max - px(0.5))));
                let mut explicit_bottom = transcript_rows.logical_scroll_top();
                explicit_bottom.offset_in_item += px(0.5);
                transcript_rows.scroll_to(explicit_bottom);
            } else if viewport_height > Pixels::ZERO {
                // A short bottom-aligned transcript has leading empty space.
                // A negative item offset preserves that space so expanding a
                // row still grows downward from its current screen position.
                // `scroll_px_offset_for_scrollbar` is zero for a short list in
                // Zed's GPUI, so derive the actual content height from its
                // rendered row bounds instead of treating the list as empty.
                //
                // Only when those bounds actually exist. Rows that have not
                // been measured yet report `None`, and treating that as a
                // zero-height document asks for a leading space of the whole
                // viewport — which pushes every row off screen and leaves the
                // transcript blank until the reader scrolls it back.
                let measured_content_height = transcript_rows
                    .bounds_for_item(0)
                    .zip(transcript_rows.bounds_for_item(count - 1))
                    .map(|(first, last)| (last.bottom() - first.top()).max(Pixels::ZERO));
                if let Some(leading_space) =
                    disclosure_leading_space(viewport_height, measured_content_height)
                {
                    transcript_rows.scroll_to(ListOffset {
                        item_ix: 0,
                        offset_in_item: -leading_space,
                    });
                }
            }
        }

        self.transcript_ui.anchor_following.set(false);
        self.transcript_ui.is_scrolled.set(true);
        // The position above is deliberate, so a wheel scroll still waiting to
        // be classified must not re-engage following on top of it.
        self.transcript_ui.tail_recheck.set(false);
    }

    /// Bulk-reset the transcript. Used for session/document replacement.
    pub(in crate::app) fn reset_transcript_rows(&self, count: usize) {
        self.transcript_ui.is_scrolled.set(false);
        self.transcript_ui.rows.reset(count);
        self.transcript_ui.anchored_transcript_rows.reset(count);
    }

    /// Apply a local disclosure change without replacing unchanged transcript
    /// rows.
    pub(in crate::app) fn splice_transcript_rows_after_visibility_change(
        &self,
        previous_kinds: &[TranscriptRowKind],
    ) {
        self.refresh_transcript_row_kinds();
        let splice = {
            let next_kinds = self.transcript_model.row_kinds.borrow();
            transcript_row_splice(previous_kinds, &next_kinds)
        };
        self.splice_transcript_rows(splice);
    }

    /// Snapshot the rows currently shown for `session_id` before a local state
    /// transition changes their visibility. Synchronizing first makes the
    /// snapshot describe the active list even when several provider events
    /// arrived in the same drain pass.
    pub(in crate::app) fn snapshot_selected_transcript_rows(
        &self,
        session_id: Uuid,
    ) -> Option<Vec<TranscriptRowKind>> {
        if self.state.selected_session != Some(session_id) {
            return None;
        }
        self.sync_transcript_rows();
        Some(self.transcript_model.row_kinds.borrow().clone())
    }

    /// Reconcile a visibility change against only the list on screen.
    ///
    /// Settling a turn folds its live work and removes the working indicator,
    /// so the visible row count usually shrinks. The generic count-based sync
    /// handles a shrink with `ListState::reset`, which clears the logical
    /// scroll position and exposes row zero for one frame before the sent-row
    /// anchor is restored. An exact splice retains the unchanged measurements
    /// and GPUI's logical scroll anchor throughout the fold.
    pub(in crate::app) fn splice_active_transcript_rows_after_visibility_change(
        &self,
        previous_kinds: &[TranscriptRowKind],
    ) {
        self.refresh_transcript_row_kinds();
        let splice = {
            let next_kinds = self.transcript_model.row_kinds.borrow();
            transcript_row_splice(previous_kinds, &next_kinds)
        };
        if let Some((range, new_count)) = splice {
            self.active_transcript_rows().splice(range, new_count);
        }
    }

    pub(in crate::app) fn selected_transcript_anchor_row(&self) -> Option<usize> {
        let anchor = self.transcript_ui.anchor.get()?;
        let session = self.selected_session()?;
        if session.id != anchor.session_id {
            return None;
        }
        let message_index = session.messages.iter().position(|message| {
            message.role == MessageRole::User && message.turn_id == Some(anchor.turn_id)
        })?;
        self.transcript_model
            .row_kinds
            .borrow()
            .iter()
            .position(|kind| *kind == TranscriptRowKind::Message(message_index))
    }

    pub(in crate::app) fn scroll_transcript_to_anchor(&self) {
        let Some(item_ix) = self.selected_transcript_anchor_row() else {
            return;
        };
        self.active_transcript_rows().scroll_to(ListOffset {
            item_ix,
            offset_in_item: Pixels::ZERO,
        });
        self.transcript_ui.is_scrolled.set(true);
    }

    pub(in crate::app) fn update_transcript_anchor_end_space(&self, window: &Window) -> Pixels {
        let Some(anchor_row) = self.selected_transcript_anchor_row() else {
            self.transcript_ui.anchor_end_space.set(Pixels::ZERO);
            self.transcript_ui.anchor_following.set(false);
            return Pixels::ZERO;
        };

        let viewport_height = {
            let measured = self.active_transcript_rows().viewport_bounds().size.height;
            if measured > Pixels::ZERO {
                measured
            } else {
                // The first sent message replaces the empty state, so the list
                // has no prior bounds yet. The full window is a conservative
                // first-frame fallback that still guarantees a top anchor.
                window.viewport_size().height
            }
        };
        let transcript_rows = self.active_transcript_rows();
        let last_row = transcript_rows.item_count().checked_sub(1);
        let anchored_tail_height = last_row.and_then(|last_row| {
            let anchor = transcript_rows.bounds_for_item(anchor_row)?;
            let last = transcript_rows.bounds_for_item(last_row)?;
            Some((last.bottom() - anchor.top()).max(Pixels::ZERO))
        });
        // Tail rows report no bounds for a frame whenever they are remeasured —
        // which the stream pump does on every commit — and report none at all
        // before the anchored list's first paint. Missing bounds mean unknown,
        // not zero: a zero end space reads as "the reply filled the viewport",
        // so the render's follow branch pins the list to its end, the next
        // measured frame snaps back to the anchor, and the two alternate at
        // stream cadence for the whole turn. Let the previous end space stand
        // (the send path seeds a provisional full-viewport reservation) and
        // keep asserting the anchor straight through the unmeasured frame —
        // scroll_to is bounds-independent.
        let end_space = match anchored_tail_height {
            Some(height) => transcript_anchor_end_space(viewport_height, height),
            None => self.transcript_ui.anchor_end_space.get(),
        };
        self.transcript_ui.anchor_end_space.set(end_space);
        if maintain_transcript_anchor(
            transcript_rows,
            anchor_row,
            self.transcript_ui.anchor_following.get(),
            end_space,
        ) {
            self.transcript_ui.is_scrolled.set(true);
        }
        end_space
    }

    /// Invalidate the measurement of rows whose *content* changed, without
    /// touching the row structure.
    ///
    /// `splice` would also work, but it re-arms GPUI's whole-list measuring
    /// behaviour, so every disclosure toggle and every streamed batch would
    /// re-measure the entire transcript. `remeasure_items` invalidates just the
    /// range — and restores the reader's scroll position across the height
    /// change on its own. This is what Zed's own agent chat uses.
    fn remeasure_transcript_rows(&self, range: Range<usize>) {
        if range.is_empty() {
            return;
        }
        self.transcript_ui.rows.remeasure_items(range.clone());
        if self.transcript_ui.anchor.get().is_some() {
            self.transcript_ui
                .anchored_transcript_rows
                .remeasure_items(range);
        }
    }

    /// Re-render rows whose content changed in place so GPUI re-measures them.
    fn splice_transcript_rows(&self, splice: Option<(Range<usize>, usize)>) {
        let Some((range, new_count)) = splice else {
            return;
        };
        self.transcript_ui.rows.splice(range.clone(), new_count);
        if self.transcript_ui.anchor.get().is_some() {
            self.transcript_ui
                .anchored_transcript_rows
                .splice(range, new_count);
        }
    }

    pub(in crate::app) fn sync_transcript_layout_width(&self, window: &Window) -> bool {
        let (sidebar_width, right_panel_width) = self.effective_panel_widths(window);
        let sidebar_width = px(sidebar_width);
        let right_panel_width = px(right_panel_width);
        let content_width =
            (window.viewport_size().width - sidebar_width - right_panel_width - px(40.0))
                .clamp(px(1.0), px(CONTENT_MAX_WIDTH));
        let previous = self.transcript_ui.layout_width.replace(content_width);
        if previous > Pixels::ZERO && (previous - content_width).abs() < px(1.0) {
            return false;
        }

        // Reflow every row at the new wrap width. The row set is unchanged, so
        // this is a re-measure, not a splice.
        let count = self.active_transcript_rows().item_count();
        self.remeasure_transcript_rows(0..count);
        true
    }

    /// Keep the list's row count *and its row kinds* in sync with the
    /// transcript.
    ///
    /// The kinds cache is what tells `transcript_row` whether row `n` is a
    /// message, a reasoning block, a tool-activity cluster or a turn fold.
    /// Leaving it stale makes every row fall back to `Message(n)`, which
    /// silently drops all reasoning and activity from the transcript.
    ///
    /// Appends keep the reader's place (or the pinned tail); shrinking resets
    /// the view.
    pub(in crate::app) fn sync_transcript_rows(&self) {
        // The fold is cached; the list-state reconciliation below is not. It
        // has to run every time because `active_transcript_rows` can switch
        // lists under an unchanged transcript.
        let count = self.refresh_transcript_row_kinds();

        let transcript_rows = self.active_transcript_rows();
        let current = transcript_rows.item_count();
        if count > current {
            transcript_rows.splice(current..current, count - current);
        } else if count < current {
            self.reset_transcript_rows(count);
        }
    }

    pub(in crate::app) fn remeasure_transcript_tail(&self) {
        self.sync_transcript_rows();
        let count = self.active_transcript_rows().item_count();
        let from = count.saturating_sub(STREAM_REMEASURE_TAIL_ROWS);
        self.remeasure_transcript_rows(from..count);
    }

    pub(in crate::app) fn remeasure_transcript_block(&self, block_index: usize) {
        self.remeasure_transcript_row(TranscriptRowKind::TurnBlock(block_index));
    }

    pub(in crate::app) fn remeasure_transcript_message(&self, message_index: usize) {
        self.remeasure_transcript_row(TranscriptRowKind::Message(message_index));
    }

    pub(in crate::app) fn remeasure_changed_files(&self, turn_id: Uuid) {
        let target = self
            .selected_session()
            .and_then(|session| response_footer_message_index(session, turn_id))
            .map(|message_index| TranscriptRowKind::ResponseFooter(turn_id, message_index))
            .unwrap_or(TranscriptRowKind::ChangedFiles(turn_id));
        self.remeasure_transcript_row(target);
    }

    fn remeasure_transcript_row(&self, target: TranscriptRowKind) {
        self.sync_transcript_rows();
        let row = self
            .transcript_model
            .row_kinds
            .borrow()
            .iter()
            .position(|kind| *kind == target);
        if let Some(row) = row {
            self.remeasure_transcript_rows(row..row + 1);
        }
    }
}

// ── Shared pieces ──────────────────────────────────────────────────────────
