//! Computer-use permissions and asynchronously loaded application icons.
use super::*;

pub(in crate::app) struct PermissionState {
    pub(in crate::app) snapshot: ComputerPermissions,
    pub(in crate::app) tx: Sender<Result<ComputerPermissions, String>>,
    pub(in crate::app) events: Receiver<Result<ComputerPermissions, String>>,
    pub(in crate::app) request_pending: bool,
    pub(in crate::app) app_icons: RefCell<HashMap<String, Option<std::sync::Arc<gpui::Image>>>>,
    pub(in crate::app) app_icon_loads: RefCell<HashSet<String>>,
}

impl Michelle {
    pub(super) fn set_computer_use_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.state.computer_use_enabled = enabled;
        self.save();
        if enabled {
            self.request_computer_permissions(true, cx);
        }
        cx.notify();
    }

    pub(in crate::app) fn request_computer_permissions(
        &mut self,
        prompt: bool,
        cx: &mut Context<Self>,
    ) {
        if !cfg!(target_os = "macos")
            || !crate::computer_use::is_available()
            || self.settings.permissions.request_pending
        {
            return;
        }
        self.settings.permissions.request_pending = true;
        let tx = self.settings.permissions.tx.clone();
        let event_wake = self.sessions.runtime.event_wake_tx.clone();
        let daemon = self.daemon.client();
        std::thread::Builder::new()
            .name("michelle-computer-permission-request".into())
            .spawn(move || {
                let result = match daemon.request(
                    Uuid::nil(),
                    Uuid::nil(),
                    michelle_client::Command::ProbeComputerPermissions { prompt },
                ) {
                    Ok(michelle_client::ResponsePayload::ComputerPermissions { permissions }) => {
                        Ok(permissions)
                    }
                    Ok(_) => Err("the daemon returned an invalid permission response".into()),
                    Err(error) => Err(error.to_string()),
                };
                if tx.send(result).is_ok() {
                    signal_event_pump(&event_wake);
                }
            })
            .ok();
        cx.notify();
    }

    pub(super) fn revoke_computer_app(&mut self, key: &str, cx: &mut Context<Self>) {
        self.state
            .computer_use_allowed_apps
            .retain(|grant| grant.key() != key);
        self.save();
        cx.notify();
    }

    pub(super) fn computer_use_app_icon(
        &self,
        bundle_id: &str,
        cx: &mut Context<Self>,
    ) -> Option<std::sync::Arc<gpui::Image>> {
        if let Some(icon) = self.settings.permissions.app_icons.borrow().get(bundle_id) {
            return icon.clone();
        }

        let bundle_id = bundle_id.to_owned();
        if self
            .settings
            .permissions
            .app_icon_loads
            .borrow_mut()
            .insert(bundle_id.clone())
        {
            cx.spawn(async move |this, cx| {
                let load_bundle_id = bundle_id.clone();
                let icon =
                    cx.background_executor()
                        .spawn(async move {
                            crate::platform::load_app_icon_for_bundle_id(&load_bundle_id)
                        })
                        .await;
                let _ = this.update(cx, |this, cx| {
                    this.settings
                        .permissions
                        .app_icon_loads
                        .borrow_mut()
                        .remove(&bundle_id);
                    this.settings
                        .permissions
                        .app_icons
                        .borrow_mut()
                        .insert(bundle_id, icon);
                    cx.notify();
                });
            })
            .detach();
        }
        None
    }

    pub(in crate::app) fn drain_computer_permission_events(&mut self) -> bool {
        let mut changed = false;
        while let Ok(result) = self.settings.permissions.events.try_recv() {
            self.settings.permissions.request_pending = false;
            match result {
                Ok(permissions) => self.settings.permissions.snapshot = permissions,
                Err(error) => self.show_toast(error),
            }
            changed = true;
        }
        changed
    }
}
