//! The egui application: repository picker, status panel, and the
//! fetch / pull / commit / push buttons.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use crate::git::{self, CommandOutput, RepoStatus};

const APP_KEY: &str = "abg_git_ui";
const AUTO_REFRESH: Duration = Duration::from_secs(5);
const MAX_LOG: usize = 300;

/// Messages sent from the worker thread back to the UI.
pub enum Msg {
    Log(CommandOutput),
    Status(Result<RepoStatus, String>),
    /// A clone finished; switch the UI to this working copy.
    Repo(PathBuf),
    Done,
}

#[derive(serde::Deserialize, serde::Serialize, Clone)]
#[serde(default)]
pub struct Persisted {
    pub repo_path: String,
    pub clone_url: String,
    pub clone_parent: String,
    pub remote_url: String,
    pub pull_rebase: bool,
    pub auto_refresh: bool,
    pub recent: Vec<String>,
}

impl Default for Persisted {
    fn default() -> Self {
        Self {
            repo_path: String::new(),
            clone_url: String::new(),
            clone_parent: String::new(),
            remote_url: String::new(),
            pull_rebase: true,
            auto_refresh: true,
            recent: Vec::new(),
        }
    }
}

pub struct GitApp {
    p: Persisted,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    status: Option<RepoStatus>,
    status_err: Option<String>,
    log: Vec<CommandOutput>,
    busy: bool,
    commit_msg: String,
    last_refresh: Instant,
    show_clone: bool,
}

impl GitApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let p = cc
            .storage
            .and_then(|s| eframe::get_value::<Persisted>(s, APP_KEY))
            .unwrap_or_default();

        let (tx, rx) = channel();
        let mut app = Self {
            p,
            tx,
            rx,
            status: None,
            status_err: None,
            log: Vec::new(),
            busy: false,
            commit_msg: String::new(),
            last_refresh: Instant::now(),
            show_clone: false,
        };
        if !app.p.repo_path.trim().is_empty() {
            app.refresh(&cc.egui_ctx);
        }
        app
    }

    fn repo_path(&self) -> PathBuf {
        PathBuf::from(self.p.repo_path.trim())
    }

    fn has_repo(&self) -> bool {
        self.status.is_some()
    }

    /// Run `job` on a worker thread. Only one job runs at a time.
    fn spawn<F>(&mut self, ctx: &egui::Context, job: F)
    where
        F: FnOnce(&Sender<Msg>) + Send + 'static,
    {
        if self.busy {
            return;
        }
        self.busy = true;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            job(&tx);
            let _ = tx.send(Msg::Done);
            ctx.request_repaint();
        });
    }

    /// Run a sequence of git commands in the repo, stopping at the first
    /// failure, then re-read the status.
    fn repo_job(&mut self, ctx: &egui::Context, steps: Vec<Vec<String>>) {
        let repo = self.repo_path();
        self.last_refresh = Instant::now();
        self.spawn(ctx, move |tx| {
            for step in steps {
                let args: Vec<&str> = step.iter().map(String::as_str).collect();
                let out = git::run(&repo, &args);
                let ok = out.success;
                let _ = tx.send(Msg::Log(out));
                if !ok {
                    break;
                }
            }
            let _ = tx.send(Msg::Status(git::status(&repo)));
        });
    }

    fn refresh(&mut self, ctx: &egui::Context) {
        self.repo_job(ctx, Vec::new());
    }

    fn remember_recent(&mut self, path: &str) {
        let path = path.trim().to_string();
        if path.is_empty() {
            return;
        }
        self.p.recent.retain(|p| p != &path);
        self.p.recent.insert(0, path);
        self.p.recent.truncate(8);
    }

    fn drain_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Log(out) => {
                    self.log.push(out);
                    if self.log.len() > MAX_LOG {
                        let drop = self.log.len() - MAX_LOG;
                        self.log.drain(0..drop);
                    }
                }
                Msg::Status(Ok(st)) => {
                    self.p.repo_path = st.root.display().to_string();
                    self.remember_recent(&self.p.repo_path.clone());
                    self.status = Some(st);
                    self.status_err = None;
                }
                Msg::Status(Err(err)) => {
                    self.status = None;
                    self.status_err = Some(err);
                }
                Msg::Repo(path) => {
                    self.p.repo_path = path.display().to_string();
                }
                Msg::Done => self.busy = false,
            }
        }
    }
}

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
}

impl eframe::App for GitApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, APP_KEY, &self.p);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_messages();

        if self.busy {
            ctx.request_repaint_after(Duration::from_millis(100));
        } else if self.p.auto_refresh
            && self.has_repo()
            && self.last_refresh.elapsed() > AUTO_REFRESH
        {
            self.refresh(ctx);
        }

        self.top_bar(ctx);
        self.log_panel(ctx);
        self.side_panel(ctx);
        self.central_panel(ctx);
    }
}

impl GitApp {
    fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading("Action-Based-Generation");
                ui.label(egui::RichText::new("git").weak());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.busy {
                        ui.spinner();
                        ui.label("working…");
                    }
                    ui.checkbox(&mut self.p.auto_refresh, "auto-refresh");
                });
            });

            ui.horizontal(|ui| {
                ui.label("Folder:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.p.repo_path)
                        .desired_width(420.0)
                        .hint_text(r"C:\path\to\Action-Based-Generation"),
                );
                if ui.button("Browse…").clicked()
                    && let Some(dir) = rfd::FileDialog::new().pick_folder()
                {
                    self.p.repo_path = dir.display().to_string();
                    self.refresh(ctx);
                }
                if ui.button("Open").clicked() {
                    self.refresh(ctx);
                }
                if !self.p.recent.is_empty() {
                    let mut pick: Option<String> = None;
                    egui::ComboBox::from_id_salt("recent")
                        .selected_text("Recent")
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            for path in self.p.recent.clone() {
                                if ui.selectable_label(false, &path).clicked() {
                                    pick = Some(path);
                                }
                            }
                        });
                    if let Some(path) = pick {
                        self.p.repo_path = path;
                        self.refresh(ctx);
                    }
                }
                ui.toggle_value(&mut self.show_clone, "Clone…");
            });

            if self.show_clone {
                self.clone_form(ui, ctx);
            }
            ui.add_space(4.0);
        });
    }

    fn clone_form(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("GitHub URL:");
            ui.add(
                egui::TextEdit::singleline(&mut self.p.clone_url)
                    .desired_width(380.0)
                    .hint_text("https://github.com/owner/Action-Based-Generation.git"),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Into folder:");
            ui.add(
                egui::TextEdit::singleline(&mut self.p.clone_parent)
                    .desired_width(380.0)
                    .hint_text(r"C:\Users\you\Desktop"),
            );
            if ui.button("Browse…").clicked()
                && let Some(dir) = rfd::FileDialog::new().pick_folder()
            {
                self.p.clone_parent = dir.display().to_string();
            }
            let ready = !self.p.clone_url.trim().is_empty()
                && PathBuf::from(self.p.clone_parent.trim()).is_dir();
            if ui
                .add_enabled(ready && !self.busy, egui::Button::new("Clone"))
                .clicked()
            {
                let url = self.p.clone_url.trim().to_string();
                let parent = PathBuf::from(self.p.clone_parent.trim());
                let name = url
                    .trim_end_matches('/')
                    .trim_end_matches(".git")
                    .rsplit('/')
                    .next()
                    .unwrap_or("repo")
                    .to_string();
                let dest = parent.join(&name);
                self.spawn(ctx, move |tx| {
                    let out = git::run(&parent, &["clone", &url, &name]);
                    let ok = out.success;
                    let _ = tx.send(Msg::Log(out));
                    if ok {
                        let _ = tx.send(Msg::Repo(dest.clone()));
                        let _ = tx.send(Msg::Status(git::status(&dest)));
                    }
                });
                self.show_clone = false;
            }
        });
    }

    fn side_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("side")
            .resizable(true)
            .default_width(290.0)
            .show(ctx, |ui| {
                ui.add_space(6.0);

                let Some(st) = self.status.clone() else {
                    ui.heading("No repository");
                    ui.add_space(6.0);
                    if let Some(err) = &self.status_err {
                        ui.colored_label(egui::Color32::from_rgb(220, 110, 110), err);
                    } else {
                        ui.label("Pick a folder above, or clone one from GitHub.");
                    }
                    ui.add_space(10.0);
                    self.connect_form(ui, ctx);
                    return;
                };

                ui.horizontal(|ui| {
                    ui.label("Branch:");
                    ui.strong(&st.branch);
                });
                ui.horizontal(|ui| {
                    ui.label("Tracking:");
                    match &st.upstream {
                        Some(u) => ui.strong(u),
                        None => ui.colored_label(
                            egui::Color32::from_rgb(220, 170, 90),
                            "no upstream yet",
                        ),
                    };
                });
                ui.horizontal(|ui| {
                    if st.behind > 0 {
                        ui.colored_label(
                            egui::Color32::from_rgb(120, 180, 240),
                            format!("↓ {} to pull", st.behind),
                        );
                    }
                    if st.ahead > 0 {
                        ui.colored_label(
                            egui::Color32::from_rgb(140, 210, 140),
                            format!("↑ {} to push", st.ahead),
                        );
                    }
                    if st.ahead == 0 && st.behind == 0 && !st.is_dirty() {
                        ui.label("up to date");
                    }
                });

                if let Some(url) = &st.remote_url {
                    ui.horizontal(|ui| {
                        ui.label("origin:");
                        ui.add(egui::Label::new(egui::RichText::new(url).small()).truncate());
                    });
                }

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);

                let busy = self.busy;
                let big = egui::vec2(ui.available_width(), 30.0);

                if ui
                    .add_enabled(
                        !busy,
                        egui::Button::new(
                            egui::RichText::new("⟲  Sync (pull, then push)").strong(),
                        )
                        .min_size(big),
                    )
                    .on_hover_text(
                        "Commits staged changes if a message is set, pulls, then pushes.",
                    )
                    .clicked()
                {
                    let mut steps: Vec<Vec<String>> = Vec::new();
                    if st.is_dirty() && !self.commit_msg.trim().is_empty() {
                        steps.push(argv(&["add", "-A"]));
                        steps.push(argv(&["commit", "-m", self.commit_msg.trim()]));
                    }
                    steps.push(self.pull_args());
                    steps.push(self.push_args(&st));
                    self.commit_msg.clear();
                    self.repo_job(ctx, steps);
                }

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let w = (ui.available_width() - 8.0) / 2.0;
                    if ui
                        .add_enabled(
                            !busy,
                            egui::Button::new("↓  Pull").min_size(egui::vec2(w, 28.0)),
                        )
                        .clicked()
                    {
                        let args = self.pull_args();
                        self.repo_job(ctx, vec![args]);
                    }
                    if ui
                        .add_enabled(
                            !busy,
                            egui::Button::new("↑  Push").min_size(egui::vec2(w, 28.0)),
                        )
                        .clicked()
                    {
                        let args = self.push_args(&st);
                        self.repo_job(ctx, vec![args]);
                    }
                });

                ui.horizontal(|ui| {
                    let w = (ui.available_width() - 8.0) / 2.0;
                    if ui
                        .add_enabled(
                            !busy,
                            egui::Button::new("Fetch").min_size(egui::vec2(w, 24.0)),
                        )
                        .clicked()
                    {
                        self.repo_job(ctx, vec![argv(&["fetch", "--all", "--prune"])]);
                    }
                    if ui
                        .add_enabled(
                            !busy,
                            egui::Button::new("Refresh").min_size(egui::vec2(w, 24.0)),
                        )
                        .clicked()
                    {
                        self.refresh(ctx);
                    }
                });

                ui.add_space(4.0);
                ui.checkbox(&mut self.p.pull_rebase, "Pull with rebase (recommended)");

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                // Branch switcher.
                if !st.branches.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label("Switch to:");
                        let mut pick: Option<String> = None;
                        egui::ComboBox::from_id_salt("branches")
                            .selected_text(&st.branch)
                            .width(ui.available_width().min(170.0))
                            .show_ui(ui, |ui| {
                                for b in &st.branches {
                                    if ui.selectable_label(*b == st.branch, b).clicked()
                                        && *b != st.branch
                                    {
                                        pick = Some(b.clone());
                                    }
                                }
                            });
                        if let Some(b) = pick {
                            self.repo_job(ctx, vec![argv(&["checkout", &b])]);
                        }
                    });
                }

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Open folder").clicked() {
                        git::open_folder(&st.root);
                    }
                    if let Some(url) = st.remote_url.as_deref().and_then(git::web_url)
                        && ui.button("Open on GitHub").clicked()
                    {
                        git::open_url(&url);
                    }
                });

                ui.add_space(10.0);
                ui.label(egui::RichText::new("Recent commits").weak());
                egui::ScrollArea::vertical()
                    .id_salt("commits")
                    .max_height(160.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for line in &st.recent_commits {
                            ui.label(egui::RichText::new(line).monospace().small());
                        }
                    });
            });
    }

    /// Shown when the chosen folder is not a git repository yet: turn it into
    /// one and point it at a GitHub remote.
    fn connect_form(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let folder = self.repo_path();
        if !folder.is_dir() {
            return;
        }
        ui.label(egui::RichText::new("Connect this folder to GitHub").strong());
        ui.add_space(4.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.p.remote_url)
                .desired_width(ui.available_width())
                .hint_text("https://github.com/owner/repo.git"),
        );
        ui.add_space(4.0);
        let ready = !self.p.remote_url.trim().is_empty();
        if ui
            .add_enabled(
                ready && !self.busy,
                egui::Button::new("Initialise + add origin"),
            )
            .on_hover_text("Runs: git init, git remote add origin <url>, git fetch origin")
            .clicked()
        {
            let url = self.p.remote_url.trim().to_string();
            self.repo_job(
                ctx,
                vec![
                    argv(&["init"]),
                    argv(&["remote", "add", "origin", &url]),
                    argv(&["fetch", "origin"]),
                ],
            );
        }
    }

    fn pull_args(&self) -> Vec<String> {
        if self.p.pull_rebase {
            argv(&["pull", "--rebase", "--autostash"])
        } else {
            argv(&["pull", "--ff-only"])
        }
    }

    fn push_args(&self, st: &RepoStatus) -> Vec<String> {
        if st.upstream.is_none() && !st.detached {
            argv(&["push", "-u", "origin", &st.branch])
        } else {
            argv(&["push"])
        }
    }

    fn central_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(st) = self.status.clone() else {
                ui.centered_and_justified(|ui| {
                    ui.label("Open a repository to get started.");
                });
                return;
            };

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading("Changes");
                ui.label(egui::RichText::new(format!("({})", st.files.len())).weak());
            });
            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .id_salt("files")
                .auto_shrink([false, false])
                .max_height(ui.available_height() - 110.0)
                .show(ui, |ui| {
                    if st.files.is_empty() {
                        ui.label(egui::RichText::new("Working tree is clean.").weak());
                    }
                    for f in &st.files {
                        ui.horizontal(|ui| {
                            let color = match f.label() {
                                "new" => egui::Color32::from_rgb(140, 210, 140),
                                "deleted" => egui::Color32::from_rgb(220, 110, 110),
                                "conflict" => egui::Color32::from_rgb(240, 150, 60),
                                _ => egui::Color32::from_rgb(220, 190, 100),
                            };
                            ui.add_sized(
                                [70.0, 16.0],
                                egui::Label::new(
                                    egui::RichText::new(f.label()).color(color).small(),
                                ),
                            );
                            ui.label(egui::RichText::new(&f.path).monospace());
                            if f.staged {
                                ui.label(egui::RichText::new("staged").small().weak().italics());
                            }
                        });
                    }
                });

            ui.add_space(6.0);
            ui.separator();
            ui.label("Commit message");
            ui.add(
                egui::TextEdit::multiline(&mut self.commit_msg)
                    .desired_rows(2)
                    .desired_width(f32::INFINITY)
                    .hint_text("What changed?"),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let ready = st.is_dirty() && !self.commit_msg.trim().is_empty() && !self.busy;
                if ui
                    .add_enabled(ready, egui::Button::new("Commit all"))
                    .clicked()
                {
                    let msg = self.commit_msg.trim().to_string();
                    self.commit_msg.clear();
                    self.repo_job(
                        ctx,
                        vec![argv(&["add", "-A"]), argv(&["commit", "-m", &msg])],
                    );
                }
                if ui
                    .add_enabled(ready, egui::Button::new("Commit all + Push"))
                    .clicked()
                {
                    let msg = self.commit_msg.trim().to_string();
                    self.commit_msg.clear();
                    self.repo_job(
                        ctx,
                        vec![
                            argv(&["add", "-A"]),
                            argv(&["commit", "-m", &msg]),
                            self.push_args(&st),
                        ],
                    );
                }
            });
            ui.add_space(6.0);
        });
    }

    fn log_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("log")
            .resizable(true)
            .default_height(170.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Git output").weak());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("clear").clicked() {
                            self.log.clear();
                        }
                    });
                });
                egui::ScrollArea::vertical()
                    .id_salt("log")
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for out in &self.log {
                            ui.label(
                                egui::RichText::new(format!("$ {}", out.command))
                                    .monospace()
                                    .color(egui::Color32::from_rgb(130, 170, 220)),
                            );
                            let body = out.text();
                            if !body.is_empty() {
                                let color = if out.success {
                                    egui::Color32::GRAY
                                } else {
                                    egui::Color32::from_rgb(230, 120, 120)
                                };
                                ui.label(egui::RichText::new(body).monospace().color(color));
                            }
                            ui.add_space(4.0);
                        }
                    });
            });
    }
}
