#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use mhf_iel::{MezFesStall, MhfConfig, Notification};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use ureq::Response;

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct User {
    rights: u32,
    token: String,
}

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
struct Character {
    id: u32,
    name: String,
    #[serde(default)]
    is_new: bool,
    is_female: bool,
    weapon: u32,
    hr: u32,
    gr: u32,
    last_login: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MezFes {
    id: u32,
    start: u32,
    end: u32,
    solo_tickets: u32,
    group_tickets: u32,
    stalls: Vec<u32>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AuthData {
    current_ts: u32,
    expiry_ts: u32,
    entrance_count: u32,
    notifications: Vec<String>,
    user: User,
    characters: Vec<Character>,
    mez_fes: Option<MezFes>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Empty {}

#[derive(Default, PartialEq)]
enum Host {
    #[default]
    LocalHost,
    Custom,
}

impl Host {
    fn label(&self) -> &str {
        match self {
            Host::LocalHost => "Local Server",
            Host::Custom => "Custom",
        }
    }
}

enum CharacterOp {
    Start,
    Delete,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UserRequest<'a> {
    password: &'a str,
    username: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateCharRequest<'a> {
    token: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteCharRequest<'a> {
    token: &'a str,
    char_id: u32,
}

#[derive(Default)]
enum MhfState {
    #[default]
    Login,
    Character,
}

#[derive(Default)]
struct MhfLauncher {
    state: MhfState,
    username: String,
    password: String,
    custom_host: String,
    auth_data: AuthData,
    error_message: Option<String>,
    host: Host,
}

impl MhfLauncher {
    fn get_host(&self) -> &str {
        match self.host {
            Host::LocalHost => "http://127.0.0.1:8080",
            Host::Custom => &self.custom_host,
        }
    }

    fn handle_resposne<T: DeserializeOwned>(
        &mut self,
        response: Result<Response, ureq::Error>,
    ) -> Option<T> {
        match response {
            Ok(r) => {
                match r.into_json() {
                    Ok(data) => {
                        self.error_message = None;
                        return Some(data);
                    }
                    Err(e) => {
                        self.error_message = Some(format!("Failed to decode JSON response: {e}"))
                    }
                };
            }
            Err(ureq::Error::Status(_, r)) => {
                let mut text = r.into_string().unwrap();
                if text.is_empty() {
                    text = "Unable to connect to server, try again later".into();
                }
                self.error_message = Some(text)
            }
            Err(_) => self.error_message = Some("Failed to connect to server".to_owned()),
        };
        None
    }

    fn request_login(&mut self) {
        let result = self.handle_resposne(
            ureq::post(&format!("{}/login", self.get_host())).send_json(UserRequest {
                username: &self.username,
                password: &self.password,
            }),
        );
        if let Some(auth_data) = result {
            self.auth_data = auth_data;
        }
    }

    fn request_register(&mut self) {
        let result = self.handle_resposne(
            ureq::post(&format!("{}/register", self.get_host())).send_json(UserRequest {
                username: &self.username,
                password: &self.password,
            }),
        );
        if let Some(auth_data) = result {
            self.auth_data = auth_data;
        }
    }

    fn request_create_character(&mut self) {
        let result: Option<Character> = self.handle_resposne(
            ureq::post(&format!("{}/character/create", self.get_host())).send_json(
                CreateCharRequest {
                    token: &self.auth_data.user.token,
                },
            ),
        );
        if let Some(character) = result {
            self.handle_start(character);
        };
    }

    fn request_delete_character(&mut self, character: Character) {
        let result: Option<Empty> = self.handle_resposne(
            ureq::post(&format!("{}/character/delete", self.get_host())).send_json(
                DeleteCharRequest {
                    token: &self.auth_data.user.token,
                    char_id: character.id,
                },
            ),
        );
        if let Some(Empty) = result {
            self.auth_data.characters.retain(|c| c.id != character.id);
        };
    }

    fn handle_start(&mut self, character: Character) {
        let auth_data = &self.auth_data;
        let char_ids: Vec<u32> = auth_data.characters.iter().map(|c| c.id).collect();
        let mut config = MhfConfig {
            entrance_count: auth_data.entrance_count,
            current_ts: auth_data.current_ts,
            expiry_ts: auth_data.expiry_ts,
            notifications: auth_data
                .notifications
                .iter()
                .map(|n| Notification { data: n, flags: 0 })
                .collect(),
            char_id: character.id,
            char_new: character.is_new,
            char_name: &character.name,
            char_hr: character.hr,
            char_gr: character.gr,
            char_ids,
            user_name: &self.username,
            user_password: &self.password,
            user_rights: auth_data.user.rights,
            user_token: &auth_data.user.token,
            ..Default::default()
        };
        if let Some(mez_fes) = &auth_data.mez_fes {
            config.mez_event_id = mez_fes.id;
            config.mez_start = mez_fes.start;
            config.mez_end = mez_fes.end;
            config.mez_solo_tickets = mez_fes.solo_tickets;
            config.mez_group_tickets = mez_fes.group_tickets;
            config.mez_stalls = mez_fes
                .stalls
                .iter()
                .map(|v| <u32 as TryInto<MezFesStall>>::try_into(*v).unwrap())
                .collect();
        }
        config.mhf_folder = Some("F:/Games/Monster Hunter Frontier Online".into());
        mhf_iel::run(config).unwrap();
    }

    fn render_login(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Sample MHF Launcher");
            ui.text_edit_singleline(&mut self.username)
                .labelled_by(ui.label("Username").id);
            ui.text_edit_singleline(&mut self.password)
                .labelled_by(ui.label("Password").id);
            ui.separator();

            egui::ComboBox::from_label("Host")
                .selected_text(self.host.label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.host, Host::LocalHost, Host::LocalHost.label());
                    ui.selectable_value(&mut self.host, Host::Custom, Host::Custom.label());
                });
            if self.host == Host::Custom {
                ui.text_edit_singleline(&mut self.custom_host)
                    .labelled_by(ui.label("Custom Host").id);
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Login").clicked() {
                    self.request_login();
                    self.state = MhfState::Character;
                }
                if ui.button("Register").clicked() {
                    self.request_register();
                    self.state = MhfState::Character;
                }
            });
            if let Some(error_message) = &self.error_message {
                ui.label(error_message);
            }
        });
    }

    fn render_characters(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut selected = None;
            for character in self.auth_data.characters.iter() {
                ui.horizontal(|ui| {
                    ui.label("ID:");
                    ui.label(&character.id.to_string());
                    ui.label("Name:");
                    ui.label(&character.name);
                    ui.separator();
                    ui.label("HR");
                    ui.label(&character.hr.to_string());
                    ui.separator();
                    ui.label("GR");
                    ui.label(&character.gr.to_string());
                    if ui.button("Start").clicked() {
                        selected = Some((character.clone(), CharacterOp::Start));
                    }
                    if ui.button("Deleted").clicked() {
                        selected = Some((character.clone(), CharacterOp::Delete));
                    }
                });
                ui.separator();
            }
            if let Some((character, op)) = selected {
                match op {
                    CharacterOp::Start => self.handle_start(character),
                    CharacterOp::Delete => self.request_delete_character(character),
                };
            }
            ui.horizontal(|ui| {
                if ui.button("Create").clicked() {
                    self.request_create_character();
                }
                if ui.button("Logout").clicked() {
                    self.error_message = None;
                    self.state = MhfState::Login;
                }
            });
            if let Some(error_message) = &self.error_message {
                ui.label(error_message);
            }
        });
    }
}

impl eframe::App for MhfLauncher {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.style_mut(|style| {
            for (_, font_id) in style.text_styles.iter_mut() {
                font_id.size = 24.0;
            }
        });
        match self.state {
            MhfState::Login => self.render_login(ctx),
            MhfState::Character => self.render_characters(ctx),
        }
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(640.0, 480.0)),
        ..Default::default()
    };
    eframe::run_native(
        "My egui App",
        options,
        Box::new(|_cc| {
            let mut l = Box::<MhfLauncher>::default();
            l.username = "rockisch".into();
            l.password = "abcdef".into();
            l
        }),
    )
}
