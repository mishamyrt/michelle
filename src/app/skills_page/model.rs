//! Catalog data, background scans and mutations for the Skills component.
use super::*;

const SKILLS_RESCAN_AFTER: Duration = Duration::from_secs(60);

#[derive(Default)]
pub(in crate::app) struct SkillsModel {
    pub(super) catalog: Option<Rc<SkillsCatalog>>,
    generation: u64,
    pub(super) pending: bool,
    scanned_at: Option<Instant>,
    error: Option<String>,
}

impl SkillsModel {
    fn begin_scan(&mut self, force: bool) -> Option<u64> {
        if self.pending {
            return None;
        }
        let fresh = self.catalog.is_some()
            && self
                .scanned_at
                .is_some_and(|scanned| scanned.elapsed() < SKILLS_RESCAN_AFTER);
        if !force && fresh {
            return None;
        }
        self.pending = true;
        self.generation += 1;
        Some(self.generation)
    }

    fn invalidate(&mut self) {
        self.generation += 1;
        self.pending = false;
    }

    fn finish_scan(&mut self, generation: u64, result: anyhow::Result<SkillsCatalog>) -> bool {
        if self.generation != generation {
            return false;
        }
        self.pending = false;
        match result {
            Ok(catalog) => {
                self.catalog = Some(Rc::new(catalog));
                self.scanned_at = Some(Instant::now());
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        true
    }
}

impl Michelle {
    // ── Catalog ────────────────────────────────────────────────────────────

    /// Start a background library scan unless a current-enough catalog (or an
    /// in-flight scan) already covers it. Results from superseded scans are
    /// discarded by generation.
    pub(in crate::app) fn ensure_skills_catalog(&mut self, force: bool, cx: &mut Context<Self>) {
        let Some(generation) = self.skills.begin_scan(force) else {
            return;
        };
        let projects = self.skill_scan_projects();
        let daemon = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let catalog = cx
                .background_executor()
                .spawn(async move {
                    match daemon.request(
                        Uuid::nil(),
                        Uuid::nil(),
                        michelle_client::Command::LoadSkills { projects },
                    )? {
                        michelle_client::ResponsePayload::SkillsCatalog { catalog } => Ok(catalog),
                        _ => anyhow::bail!("the daemon returned an invalid skills response"),
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if !this.skills.finish_scan(generation, catalog) {
                    return;
                }
                if let Some(error) = this.skills.error.clone() {
                    this.show_toast(error);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Drop any in-flight scan's claim and rescan now. Called after every
    /// mutation, so the library on screen always re-reads the disk it just
    /// changed.
    fn invalidate_skills_catalog(&mut self, cx: &mut Context<Self>) {
        self.skills.invalidate();
        self.ensure_skills_catalog(true, cx);
    }

    /// `(display name, path)` per scannable project. Projectless workspaces
    /// are generated directories that never hold curated skills.
    fn skill_scan_projects(&self) -> Vec<(String, PathBuf)> {
        self.state
            .projects
            .iter()
            .filter(|project| !project.is_projectless())
            .map(|project| (project.display_name(), project.path.clone()))
            .collect()
    }

    // ── Mutations ──────────────────────────────────────────────────────────

    /// Flip every copy of the skill keyed by `primary_dir`. A skill installed
    /// into several roots is one skill; the switch converges all of them.
    pub(super) fn toggle_skill_enabled(
        &mut self,
        primary_dir: PathBuf,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        let dirs = self
            .skills
            .catalog
            .as_ref()
            .and_then(|catalog| {
                catalog
                    .skills
                    .iter()
                    .find(|skill| skill.primary().dir == primary_dir)
            })
            .map(|skill| {
                skill
                    .installs
                    .iter()
                    .map(|install| install.dir.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec![primary_dir.clone()]);
        // The switch answers immediately; the rescan confirms from disk.
        if let Some(catalog) = self.skills.catalog.as_ref() {
            let mut updated = catalog.as_ref().clone();
            for skill in &mut updated.skills {
                if skill.primary().dir == primary_dir {
                    skill.enabled = enabled;
                    skill.row_key = skill.row_key.wrapping_add(1);
                    for install in &mut skill.installs {
                        install.enabled = enabled;
                        install.skill_file = install.dir.join(if enabled {
                            crate::skills::SKILL_FILE
                        } else {
                            crate::skills::DISABLED_SKILL_FILE
                        });
                    }
                }
            }
            self.skills.catalog = Some(Rc::new(updated));
        }
        self.skills.invalidate();
        let daemon = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    daemon.request(
                        Uuid::nil(),
                        Uuid::nil(),
                        michelle_client::Command::SetSkillsEnabled { dirs, enabled },
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Err(error) = result {
                    this.show_toast(tr!("skills.toggle_failed", error = error));
                }
                this.invalidate_skills_catalog(cx);
            });
        })
        .detach();
        cx.notify();
    }

    /// Trash every copy of the skill keyed by `primary_dir`.
    pub(super) fn delete_skill(&mut self, primary_dir: PathBuf, cx: &mut Context<Self>) {
        let entry = self.skills.catalog.as_ref().and_then(|catalog| {
            catalog
                .skills
                .iter()
                .find(|skill| skill.primary().dir == primary_dir)
                .cloned()
        });
        let name = entry
            .as_ref()
            .map(|skill| skill.name.clone())
            .unwrap_or_else(|| {
                primary_dir
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            });
        let dirs = entry
            .map(|skill| {
                skill
                    .installs
                    .iter()
                    .map(|install| install.dir.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec![primary_dir.clone()]);
        if self.skills_ui.selected.as_ref() == Some(&primary_dir) {
            self.skills_ui.selected = None;
        }
        if let Some(catalog) = self.skills.catalog.as_ref() {
            let mut updated = catalog.as_ref().clone();
            updated
                .skills
                .retain(|skill| skill.primary().dir != primary_dir);
            self.skills.catalog = Some(Rc::new(updated));
        }
        self.skills_ui.delete_arming = None;
        self.skills.invalidate();
        let daemon = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    daemon.request(
                        Uuid::nil(),
                        Uuid::nil(),
                        michelle_client::Command::TrashSkills { dirs },
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(_) => this.show_success_toast(tr!("skills.deleted_toast", name = name)),
                    Err(error) => {
                        this.show_toast(tr!("skills.delete_failed", error = error));
                    }
                }
                this.invalidate_skills_catalog(cx);
            });
        })
        .detach();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalidated_scan_cannot_overwrite_catalog_or_clear_new_request() {
        let mut model = SkillsModel::default();
        let old = model.begin_scan(false).unwrap();
        assert_eq!(model.begin_scan(true), None);
        model.invalidate();
        let current = model.begin_scan(true).unwrap();
        assert!(!model.finish_scan(old, Ok(SkillsCatalog::default())));
        assert!(model.pending);
        assert!(model.catalog.is_none());
        assert!(model.finish_scan(current, Ok(SkillsCatalog::default())));
        assert!(!model.pending);
        assert!(model.catalog.is_some());
        assert_eq!(model.begin_scan(false), None);
        assert!(model.begin_scan(true).is_some());
    }

    #[test]
    fn failed_refresh_preserves_catalog_and_allows_retry() {
        let mut model = SkillsModel::default();
        let first = model.begin_scan(false).unwrap();
        model.finish_scan(first, Ok(SkillsCatalog::default()));
        let catalog = model.catalog.clone().unwrap();
        let refresh = model.begin_scan(true).unwrap();
        model.finish_scan(refresh, Err(anyhow::anyhow!("scan failed")));
        assert!(Rc::ptr_eq(&catalog, model.catalog.as_ref().unwrap()));
        assert_eq!(model.error.as_deref(), Some("scan failed"));
        assert!(!model.pending);
        assert!(model.begin_scan(true).is_some());
    }
}
